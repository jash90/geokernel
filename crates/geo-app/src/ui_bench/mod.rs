//! Headless UI frame benchmark: the real window (`App::show`) driven by
//! scripted input through `egui::Context::run_ui`, then `tessellate`, with no
//! window and no GPU. It records per frame the CPU time of the UI pass and
//! of tessellation, shape, vertex, index, text and glyph counts, heap
//! allocations, rows rebuilt, kernel objects recomputed and the repaint the
//! frame asked for; and the cost of single actions (add, undo, redo, delete,
//! a tool click) and of per-object helpers.
//!
//! Run it in release mode and keep the output:
//!
//! ```text
//! UI_BENCH_JSON=ui-bench.json cargo test --release -p geo-app ui_frame_bench -- --ignored --nocapture
//! ```
//!
//! Environment: `UI_BENCH_JSON` (write JSON there), `UI_BENCH_FRAMES`
//! (measured frames per state and round, default 10), `UI_BENCH_ROUNDS`
//! (default 3; scenes are interleaved per round to spread drift),
//! `UI_BENCH_SCENES` (comma-separated scene names), `UI_BENCH_TREE` (a
//! label stored in the JSON). Counters are deterministic; times are not.
//! Everything that depends on the app's layout is in `adapter.rs`.

mod adapter;
mod alloc;
mod report;
mod scene;

use crate::app::App;
use crate::headless;
pub use crate::headless::Repaint;
use crate::interact::ViewTransform;
use crate::theme;
use crate::view::{self, Camera};
use crate::workbench::Workbench;
use eframe::egui::{self, vec2, Event, Key, Modifiers, PointerButton, Pos2, Rect, Shape};
use geo_kernel::kernel::{Kind, Value};
use scene::Scene;
use std::collections::{BTreeSet, HashSet};
use std::time::Instant;

// The test binary only: counts the allocations of the benchmark thread
// while a measurement is running and is a plain pass-through otherwise.
#[global_allocator]
static GLOBAL: alloc::Counting = alloc::Counting;

const SCREEN: (f32, f32) = (1400.0, 900.0);
const DT: f64 = 1.0 / 60.0;
const WARMUP: usize = 3;

/// What one frame cost.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub ui_ns: u64,
    pub tess_ns: u64,
    pub ui_allocs: u64,
    pub ui_bytes: u64,
    pub tess_allocs: u64,
    pub tess_bytes: u64,
    /// Leaf shapes (nested `Shape::Vec` flattened).
    pub shapes: u64,
    pub text_shapes: u64,
    pub glyphs: u64,
    /// Text shapes whose galley was not painted in the previous frame,
    /// that is, laid out anew (or re-fetched after leaving the cache).
    pub new_galleys: u64,
    pub meshes: u64,
    pub vertices: u64,
    pub indices: u64,
    /// egui passes in this frame (more than one means a discarded pass).
    pub passes: u64,
    pub texture_updates: u64,
    pub rows_built: u64,
    pub rebuilds: u64,
    /// Kernel objects recomputed (drag only, from a shadow construction).
    pub recomputed: u64,
    pub repaint: Repaint,
    /// The state the frame was meant to measure really held.
    pub ok: bool,
}

/// The frames of one state of one scene.
pub struct Measured {
    pub scene: String,
    pub objects: usize,
    pub on_screen: usize,
    pub state: &'static str,
    pub frames: Vec<Frame>,
    pub notes: BTreeSet<String>,
}

/// One timing of an action or of a helper over all objects.
pub struct Sample {
    pub scene: String,
    pub name: String,
    pub ns: u64,
    pub allocs: u64,
    pub bytes: u64,
}

struct Runner<'s> {
    scene: &'s Scene,
    ctx: egui::Context,
    app: App,
    screen: Rect,
    time: f64,
    galleys: HashSet<usize>,
    texts: Option<Vec<String>>,
    measured: Vec<Measured>,
    samples: Vec<Sample>,
}

fn moved(p: Pos2) -> Event {
    Event::PointerMoved(p)
}

fn button(p: Pos2, pressed: bool) -> Event {
    headless::press(p, PointerButton::Primary, pressed)
}

fn key(k: Key, pressed: bool) -> Event {
    headless::key(k, pressed, Modifiers::NONE)
}

fn collect_texts(shape: &Shape, out: &mut Vec<String>) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| collect_texts(s, out)),
        Shape::Text(t) => out.push(t.galley.job.text.clone()),
        _ => {}
    }
}

fn timed<R>(f: impl FnOnce() -> R) -> (R, u64, u64, u64) {
    alloc::start();
    let t = Instant::now();
    let r = f();
    let ns = t.elapsed().as_nanos() as u64;
    let (allocs, bytes) = alloc::stop();
    (r, ns, allocs, bytes)
}

impl<'s> Runner<'s> {
    fn new(scene: &'s Scene) -> Self {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let wb = Workbench::from_script(&scene.script).expect("the scene loads");
        let mut r = Runner {
            scene,
            ctx,
            app: App::new(wb, None),
            screen: Rect::from_min_size(Pos2::ZERO, vec2(SCREEN.0, SCREEN.1)),
            time: 0.0,
            galleys: HashSet::new(),
            texts: None,
            measured: Vec::new(),
            samples: Vec::new(),
        };
        r.settle();
        r.pin_camera(1.0);
        r.settle();
        r
    }

    fn step(&mut self, events: Vec<Event>) -> Frame {
        let input = headless::raw_input(self.screen, self.time, DT as f32, events);
        self.time += DT;
        let mut f = Frame::default();
        let (ctx, app) = (&self.ctx, &mut self.app);
        let ((mut out, pass), ns, allocs, bytes) =
            timed(|| headless::run(ctx, app, input, adapter::show));
        (f.ui_ns, f.ui_allocs, f.ui_bytes) = (ns, allocs, bytes);
        (f.passes, f.rows_built, f.rebuilds) = (pass.passes, pass.rows_built, pass.rebuilds);
        f.repaint = headless::repaint(&out);
        f.texture_updates = out.textures_delta.set.len() as u64;
        out.textures_delta.clear();
        let shapes = headless::count_shapes(&out, &mut self.galleys);
        (f.shapes, f.text_shapes, f.glyphs, f.new_galleys) = (
            shapes.shapes,
            shapes.text_shapes,
            shapes.glyphs,
            shapes.new_galleys,
        );
        if let Some(texts) = &mut self.texts {
            for clipped in &out.shapes {
                collect_texts(&clipped.shape, texts);
            }
        }
        let shapes = std::mem::take(&mut out.shapes);
        let ppp = out.pixels_per_point;
        let (prims, ns, allocs, bytes) = timed(|| self.ctx.tessellate(shapes, ppp));
        (f.tess_ns, f.tess_allocs, f.tess_bytes) = (ns, allocs, bytes);
        let meshes = headless::count_meshes(&prims);
        (f.meshes, f.vertices, f.indices) = (meshes.meshes, meshes.vertices, meshes.indices);
        f
    }

    /// Runs empty frames until two in a row ask for no repaint; returns how
    /// many it took, or `None` if the window keeps repainting.
    fn settle(&mut self) -> Option<usize> {
        let mut quiet = 0;
        for i in 0..40 {
            if self.step(Vec::new()).repaint == Repaint::None {
                quiet += 1;
                if quiet == 2 {
                    return Some(i + 1);
                }
            } else {
                quiet = 0;
            }
        }
        None
    }

    fn canvas(&self) -> Rect {
        adapter::canvas(&self.app, self.screen)
    }

    fn transform(&self) -> ViewTransform {
        let r = self.canvas();
        let cam = self.app.camera.unwrap_or(Camera::STANDARD);
        cam.transform(r.width() as f64, r.height() as f64)
    }

    fn to_screen(&self, x: f64, y: f64) -> Pos2 {
        let (sx, sy) = self.transform().world_to_screen(x, y);
        self.canvas().min + vec2(sx as f32, sy as f32)
    }

    fn point(&self, label: &str) -> Pos2 {
        let c = self.app.wb.construction();
        match c.value(c.id(label).unwrap_or_else(|| panic!("no object {label}"))) {
            Value::Point { x, y } => self.to_screen(x, y),
            v => panic!("{label} is not a point: {v:?}"),
        }
    }

    /// All objects fitted into the drawing area, then zoomed in `zoom` times
    /// about its middle. Both trees get the same rule.
    fn pin_camera(&mut self, zoom: f64) {
        let r = self.canvas();
        let (w, h) = (r.width() as f64, r.height() as f64);
        let mut cam = Camera::fit(view::content_bounds(self.app.wb.construction()), w, h);
        if zoom != 1.0 {
            cam.zoom_at(zoom, w / 2.0, h / 2.0, w, h);
        }
        self.app.camera = Some(cam);
    }

    /// Drawn objects whose shape reaches into the drawing area.
    fn on_screen(&self) -> usize {
        let c = self.app.wb.construction();
        let t = self.transform();
        let r = self.canvas();
        let (w, h) = (r.width() as f64, r.height() as f64);
        let inside = |pts: &[(f64, f64)]| {
            let s: Vec<(f64, f64)> = pts.iter().map(|&(x, y)| t.world_to_screen(x, y)).collect();
            let (x0, x1) = s
                .iter()
                .fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
            let (y0, y1) = s
                .iter()
                .fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
            x1 >= 0.0 && x0 <= w && y1 >= 0.0 && y0 <= h
        };
        c.ids()
            .filter(|&id| match c.value(id) {
                Value::Point { x, y } => inside(&[(x, y)]),
                Value::Segment { x1, y1, x2, y2 } => inside(&[(x1, y1), (x2, y2)]),
                Value::Circle { cx, cy, r } => inside(&[(cx - r, cy - r), (cx + r, cy + r)]),
                v @ Value::Line { .. } => v.unit_line().is_some_and(|(a, b, k)| {
                    let (mx, my) = t.screen_to_world(w / 2.0, h / 2.0);
                    (a * mx + b * my + k).abs() * t.scale() <= w.hypot(h) / 2.0
                }),
                Value::Polygon { .. } => {
                    let pts: Vec<(f64, f64)> = c.vertices(id).flatten().collect();
                    !pts.is_empty() && inside(&pts)
                }
                _ => false,
            })
            .count()
    }

    /// A spot in the drawing area with nothing (`points_only`: no point)
    /// near it, searched outwards from the middle.
    fn empty_spot(&self, points_only: bool) -> Pos2 {
        let r = self.canvas();
        let t = self.transform();
        let c = self.app.wb.construction();
        let inner = r.shrink(90.0);
        for ring in 0..120 {
            let n = (8 * ring).max(1);
            for k in 0..n {
                let a = k as f32 / n as f32 * std::f32::consts::TAU;
                let p = r.center() + 7.0 * ring as f32 * vec2(a.cos(), a.sin());
                if !inner.contains(p) {
                    continue;
                }
                let d = p - r.min;
                let hit = crate::hit::pick(c, &t, (d.x as f64, d.y as f64), 16.0, 12.0, |id| {
                    !points_only || c.kind(id) == Kind::Point
                });
                if hit.is_none() {
                    return p;
                }
            }
        }
        panic!("no empty spot in {}", self.scene.name)
    }

    fn record(&mut self, state: &'static str, frames: Vec<Frame>, notes: BTreeSet<String>) {
        let objects = self.app.wb.construction().len();
        let on_screen = self.on_screen();
        self.measured.push(Measured {
            scene: self.scene.name.clone(),
            objects,
            on_screen,
            state,
            frames,
            notes,
        });
    }

    fn sample(&mut self, name: &str, ns: u64, allocs: u64, bytes: u64) {
        self.samples.push(Sample {
            scene: self.scene.name.clone(),
            name: name.to_owned(),
            ns,
            allocs,
            bytes,
        });
    }

    fn settle_note(&mut self, notes: &mut BTreeSet<String>) {
        if self.settle().is_none() {
            notes.insert("never settles: keeps asking for repaints".to_owned());
        }
    }

    /// Runs one empty frame and notes why the frame before it asked for a
    /// repaint (egui reports the causes one pass late).
    fn note_causes(&mut self, notes: &mut BTreeSet<String>) {
        self.step(Vec::new());
        for c in self.ctx.repaint_causes() {
            let file = c.file.rsplit(['/', '\\']).next().unwrap_or(c.file);
            notes.insert(format!("repaint asked at {file}:{} {}", c.line, c.reason));
        }
    }

    /// `n` frames of `events(i)`; `ok` says whether frame `i` was valid.
    fn frames(
        &mut self,
        n: usize,
        mut events: impl FnMut(&Self, usize) -> Vec<Event>,
        mut after: impl FnMut(&mut Self, &mut Frame),
    ) -> Vec<Frame> {
        for i in 0..WARMUP {
            let ev = events(self, i);
            let mut f = self.step(ev);
            after(self, &mut f);
        }
        (0..n)
            .map(|i| {
                let ev = events(self, WARMUP + i);
                let mut f = self.step(ev);
                after(self, &mut f);
                f
            })
            .collect()
    }
}

mod states;

/// Runs every state of `scene` once with `n` measured frames each.
fn run_scene(scene: &Scene, n: usize, measured: &mut Vec<Measured>, samples: &mut Vec<Sample>) {
    let mut r = Runner::new(scene);
    states::run_all(&mut r, n);
    let [w, h] = r.ctx.fonts(|f| f.font_image_size());
    let galleys = r.ctx.fonts(|f| f.num_galleys_in_cache());
    if let Some(last) = r.measured.last_mut() {
        last.notes
            .insert(format!("font atlas {w}x{h} px, {galleys} galleys in cache"));
    }
    measured.append(&mut r.measured);
    samples.append(&mut r.samples);
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Runs the scenes named in `filter` (all if empty), interleaved per round.
fn run(filter: &[String], frames: usize, rounds: usize) -> (Vec<Measured>, Vec<Sample>) {
    let scenes: Vec<Scene> = scene::all()
        .into_iter()
        .filter(|s| filter.is_empty() || filter.contains(&s.name))
        .collect();
    let mut measured = Vec::new();
    let mut samples = Vec::new();
    for round in 0..rounds {
        // Alternate the order so no scene always runs first.
        let mut order: Vec<&Scene> = scenes.iter().collect();
        if round % 2 == 1 {
            order.reverse();
        }
        for s in order {
            eprintln!("ui_bench: round {} scene {}", round + 1, s.name);
            run_scene(s, frames, &mut measured, &mut samples);
        }
    }
    (measured, samples)
}

#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn ui_frame_bench() {
    let filter: Vec<String> = std::env::var("UI_BENCH_SCENES")
        .map(|s| s.split(',').map(|p| p.trim().to_owned()).collect())
        .unwrap_or_default();
    let frames = env_usize("UI_BENCH_FRAMES", 10);
    let rounds = env_usize("UI_BENCH_ROUNDS", 3);
    let (measured, samples) = run(&filter, frames, rounds);
    let tree = std::env::var("UI_BENCH_TREE").unwrap_or_else(|_| adapter::TREE.to_owned());
    let table = report::table(&measured, &samples);
    println!("{table}");
    if let Ok(path) = std::env::var("UI_BENCH_JSON") {
        let json = report::json(&tree, frames, rounds, &measured, &samples);
        std::fs::write(&path, json).expect("write the JSON report");
        println!("ui_bench: JSON written to {path}");
    }
}

/// A quick pass over the smallest scene: every scripted state must really
/// happen (a drag that does not drag would silently measure a pan).
#[test]
fn ui_bench_states_are_real() {
    let (measured, samples) = run(&["four-lines".to_owned()], 2, 1);
    for m in &measured {
        assert!(!m.frames.is_empty(), "{}: no frames", m.state);
        let bad = m.frames.iter().filter(|f| !f.ok).count();
        assert_eq!(bad, 0, "{}: {bad} frames not in the state", m.state);
        assert!(m.frames.iter().all(|f| f.shapes > 0 && f.vertices > 0));
    }
    let drag = measured.iter().find(|m| m.state == "drag_point").unwrap();
    assert!(drag.frames.iter().all(|f| f.recomputed > 0));
    assert!(samples.iter().any(|s| s.name == "undo"));
}
