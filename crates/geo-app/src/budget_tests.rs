//! The frame budget as counters: what one frame of the window produces at
//! 100 and 1000 objects, in the states the frame benchmark measures (idle,
//! zoomed in, keyboard open, tool preview, pan, drag) and in the frames
//! after an undo, a redo and a deletion. Times are not
//! asserted (they depend on the machine); these counters stand in for
//! them, and a change that makes a frame do work for every object, or
//! rebuild the algebra list while dragging, fails here.
//!
//! The counters: leaf shapes and text shapes painted, vertices after
//! tessellation, heap allocations and bytes of the frame (the UI pass,
//! counted by the benchmark's allocator), algebra rows made
//! (`rows_formatted`) and list changes (`rows_built`), and whether the
//! frame asked for an immediate repaint.

use crate::app::App;
use crate::geom::ToEgui;
use crate::headless::{self, Repaint};
use crate::style;
use crate::theme;
use crate::tools::Tool;
use crate::ui_bench::alloc;
use crate::view::{self, Camera};
use crate::workbench::Workbench;
use eframe::egui::{self, epaint, vec2, Event, Modifiers, PointerButton, Pos2, Rect, Shape};
use geo_kernel::kernel::{Kind, Value};

const SCREEN: (f32, f32) = (1400.0, 900.0);
const DT: f64 = 1.0 / 60.0;

/// The benchmark's grid cell: three free points, a free segment, a line, a
/// circle, a triangle with its three sides, a midpoint and one indexed intersection.
fn grid(cells: usize) -> (String, String) {
    let cols = (cells as f64).sqrt().ceil() as usize;
    let half = (cols as f64 - 1.0) * 5.0 / 2.0;
    let mut s = String::new();
    for i in 0..cells {
        let x = (i % cols) as f64 * 5.0 - half;
        let y = (i / cols) as f64 * 5.0 - half;
        let p = |n: &str| format!("{n}_{{{i}}}");
        let (pp, q, r) = (p("P"), p("Q"), p("R"));
        for line in [
            format!("{pp} = ({x}, {y})"),
            format!("{q} = ({}, {y})", x + 2.0),
            format!("{r} = ({x}, {})", y + 2.0),
            format!("{} = Segment({pp}, {r})", p("s")),
            format!("{} = Line({q}, {r})", p("l")),
            format!("{} = Circle({pp}, {q})", p("k")),
            format!("{} = Polygon({pp}, {q}, {r})", p("t")),
            format!("{} = Midpoint({q}, {r})", p("M")),
            format!("{} = Intersect({}, {}, 1)", p("I"), p("l"), p("k")),
        ] {
            s += &line;
            s.push('\n');
        }
    }
    let mid = ((cols / 2) * cols + cols / 2).min(cells - 1);
    (s, format!("{{{mid}}}"))
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Counts {
    shapes: u64,
    texts: u64,
    vertices: u64,
    raster_quads: u64,
    allocs: u64,
    bytes: u64,
    /// Objects looked through for polygon sides (`style::objects_scanned`).
    colour_scans: u64,
    rows_formatted: u64,
    rows_built: u64,
    immediate: bool,
}

struct Window {
    ctx: egui::Context,
    app: App,
    time: f64,
    /// The label suffix of the cell in the middle, `{n}`.
    mid: String,
    /// The window's size in points.
    screen: (f32, f32),
}

fn leaves(shape: &Shape, c: &mut Counts) {
    match shape {
        Shape::Noop => {}
        Shape::Vec(v) => v.iter().for_each(|s| leaves(s, c)),
        Shape::Text(_) => {
            c.shapes += 1;
            c.texts += 1;
        }
        _ => c.shapes += 1,
    }
}

impl Window {
    fn new(cells: usize) -> Window {
        let (script, mid) = grid(cells);
        Window::with(&script, mid)
    }

    fn with(script: &str, mid: String) -> Window {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut w = Window {
            ctx,
            app: App::new(Workbench::from_script(script).unwrap(), None),
            time: 0.0,
            mid,
            screen: SCREEN,
        };
        w.frames(3);
        w.fit(1.0);
        w.frames(3);
        w
    }

    fn step(&mut self, events: Vec<Event>) -> Counts {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(self.screen.0, self.screen.1));
        let input = headless::raw_input(screen, self.time, DT as f32, events);
        self.time += DT;
        let made = self.app.wb.rows_formatted();
        let scans = style::objects_scanned();
        // The UI pass only, as the benchmark's `ui_allocs` / `ui_bytes`.
        alloc::start();
        let (mut out, pass) = headless::run(&self.ctx, &mut self.app, input, crate::app::show);
        let (allocs, bytes) = alloc::stop();
        let colour_scans = style::objects_scanned() - scans;
        out.textures_delta.clear();
        let mut c = Counts {
            allocs,
            bytes,
            colour_scans,
            rows_formatted: self.app.wb.rows_formatted() - made,
            rows_built: pass.rows_built,
            immediate: headless::repaint(&out) == Repaint::Immediate,
            ..Counts::default()
        };
        for s in &out.shapes {
            leaves(&s.shape, &mut c);
        }
        for p in self.ctx.tessellate(out.shapes, out.pixels_per_point) {
            if let epaint::Primitive::Mesh(m) = p.primitive {
                c.vertices += m.vertices.len() as u64;
                if m.texture_id != epaint::TextureId::default() {
                    c.raster_quads += (m.indices.len() / 6) as u64;
                }
            }
        }
        c
    }

    fn frames(&mut self, n: usize) -> Counts {
        (0..n).fold(Counts::default(), |_, _| self.step(Vec::new()))
    }

    /// Empty frames until two in a row ask for no immediate repaint (as
    /// the benchmark settles), then `n` frames: the last one's counts.
    fn settled(&mut self, n: usize) -> Counts {
        let mut quiet = 0;
        for _ in 0..40 {
            quiet = if self.step(Vec::new()).immediate {
                0
            } else {
                quiet + 1
            };
            if quiet == 2 {
                return self.frames(n);
            }
        }
        panic!("the window keeps asking for repaints");
    }

    /// All objects in the drawing area, zoomed in `zoom` times about its
    /// middle (the benchmark's camera).
    fn fit(&mut self, zoom: f64) {
        let r = self.app.view.unwrap().0;
        let (w, h) = (r.width() as f64, r.height() as f64);
        let mut cam = Camera::fit(
            view::content_bounds(self.app.wb.construction()),
            w,
            h,
            view::ScaleLimits::OURS,
        );
        cam.zoom_at(zoom, w / 2.0, h / 2.0, w, h, view::ScaleLimits::OURS);
        self.app.camera = Some(cam);
    }

    fn point(&self, label: &str) -> Pos2 {
        let c = self.app.wb.construction();
        let Value::Point { x, y } = c.value(c.id(label).unwrap()) else {
            panic!("{label} is not a point");
        };
        let (rect, t) = self.app.view.unwrap();
        let (sx, sy) = t.world_to_screen(x, y);
        Pos2::new(rect.min.x + sx as f32, rect.min.y + sy as f32)
    }

    /// A spot in the drawing that hits nothing, no label either: in the
    /// Move tool only a press on nothing pans.
    fn empty_spot(&self) -> Pos2 {
        let (rect, t) = self.app.view.unwrap();
        let c = self.app.wb.construction();
        let center = Pos2::new(
            rect.min.x + rect.width() / 2.0,
            rect.min.y + rect.height() / 2.0,
        );
        let inside = rect.egui().shrink(45.0);
        let spiral =
            (0..400).map(|i| center + 3.0 * i as f32 * vec2((i as f32).cos(), (i as f32).sin()));
        let grid = (0..30).flat_map(|row| {
            (0..30).map(move |col| {
                inside.min
                    + vec2(
                        inside.width() * col as f32 / 30.0,
                        inside.height() * row as f32 / 30.0,
                    )
            })
        });
        spiral
            .chain(grid)
            .filter(|p| inside.contains(*p))
            .find(|p| {
                let local = ((p.x - rect.min.x) as f64, (p.y - rect.min.y) as f64);
                // Wider than the mouse's 3 px square: a point within 16 px
                // (4 + 12, `DrawPoint.hit`) is hit.
                let reach = crate::hit::Reach::square(4.0);
                crate::hit::pick(c, &t, local, &reach, None, |_| true).is_none()
                    && self.app.label_at(local).is_none()
            })
            .expect("an empty spot")
    }
}

fn press(p: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// The counts of every state, for one size.
fn states(cells: usize) -> Vec<(&'static str, Counts)> {
    let mut w = Window::new(cells);
    let mut out = vec![("idle", w.settled(2))];

    w.fit(4.0);
    out.push(("idle_zoomed_4x", w.settled(2)));
    w.fit(1.0);
    w.frames(3);

    w.app.focus_input = true;
    w.frames(4);
    assert!(w.app.keyboard.open, "the keyboard did not open");
    out.push(("keyboard_open", w.settled(2)));
    w.step(vec![Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    w.frames(3);

    w.app.select_tool(Tool::Segment);
    w.frames(2);
    let q = w.point(&format!("Q_{}", w.mid));
    w.step(vec![Event::PointerMoved(q)]);
    w.step(vec![press(q, true)]);
    w.step(vec![press(q, false)]);
    w.step(vec![Event::PointerMoved(q + vec2(60.0, -40.0))]);
    let preview = w.step(vec![Event::PointerMoved(q + vec2(64.0, -44.0))]);
    assert_eq!(w.app.tool.picks().len(), 1, "no tool preview");
    out.push(("tool_preview_segment", preview));
    w.step(vec![Event::PointerGone]);
    w.app.select_tool(Tool::Move);
    w.frames(3);

    let from = w.empty_spot();
    w.step(vec![Event::PointerMoved(from)]);
    w.step(vec![press(from, true)]);
    // Moves within the drag delay after a press are dropped.
    let wait = (w.app.metrics.drag_delay / DT).ceil() as usize + 1;
    w.frames(wait);
    w.step(vec![Event::PointerMoved(from + vec2(14.0, 4.0))]);
    let pan = w.step(vec![Event::PointerMoved(from + vec2(10.0, 4.0))]);
    assert!(w.app.gesture.panning() && w.app.drag.is_none(), "no pan");
    out.push(("pan", pan));
    w.step(vec![press(from + vec2(10.0, 4.0), false)]);
    w.step(vec![Event::PointerGone]);
    w.fit(1.0);
    w.frames(3);

    let p = w.point(&format!("P_{}", w.mid));
    w.step(vec![Event::PointerMoved(p)]);
    w.step(vec![press(p, true)]);
    w.frames(wait);
    w.step(vec![Event::PointerMoved(p + vec2(14.0, 4.0))]);
    w.step(vec![Event::PointerMoved(p + vec2(10.0, 4.0))]);
    assert!(w.app.drag.is_some(), "no drag");
    w.step(vec![Event::PointerMoved(p + vec2(10.0, 4.0))]);
    // A drag frame, and how many objects it changed. The algebra list is
    // laid out before the drawing, which moves the point, so the rows
    // follow one frame later: the next frame, with the pointer held still,
    // makes them.
    let c = w.app.wb.construction();
    let before: Vec<Value> = c.ids().map(|id| c.value(id)).collect();
    let mut drag = w.step(vec![Event::PointerMoved(p + vec2(14.0, 4.0))]);
    let c = w.app.wb.construction();
    let changed = c
        .ids()
        .zip(&before)
        .filter(|(id, v)| c.value(*id) != **v)
        .count() as u64;
    assert!(changed > 0, "the drag moved nothing");
    let next = w.step(vec![Event::PointerMoved(p + vec2(14.0, 4.0))]);
    assert!(
        next.rows_formatted <= changed,
        "{} rows made for {changed} changed objects",
        next.rows_formatted
    );
    drag.rows_formatted += next.rows_formatted;
    drag.rows_built += next.rows_built;
    // The heavier of the two frames: the one that moves, or the one that
    // makes the rows.
    drag.allocs = drag.allocs.max(next.allocs);
    drag.bytes = drag.bytes.max(next.bytes);
    out.push(("drag_point", drag));
    // The pointer held still once more: nothing changed, nothing is made.
    let still = w.step(vec![Event::PointerMoved(p + vec2(14.0, 4.0))]);
    out.push(("drag_point_still", still));
    w.step(vec![press(p + vec2(14.0, 4.0), false)]);
    w.step(vec![Event::PointerGone]);
    w.settled(1);

    // Undo and redo of that drag (one step), then a deletion: each rebuilds
    // the construction, which renumbers the objects. The frame after makes
    // the rows of the objects whose value changed, and keeps the others.
    let before = w.values();
    assert!(w.app.undo(), "nothing to undo");
    let undo = w.step(Vec::new());
    let changed = w.changed_since(&before);
    assert!(changed > 0, "the undo moved nothing back");
    assert!(
        undo.rows_formatted <= changed,
        "undo: {} rows made for {changed} changed objects",
        undo.rows_formatted
    );
    out.push(("undo", undo));
    w.settled(1);
    let before = w.values();
    assert!(w.app.redo(), "nothing to redo");
    let redo = w.step(Vec::new());
    let changed = w.changed_since(&before);
    assert!(
        redo.rows_formatted <= changed,
        "redo: {} rows made for {changed} changed objects",
        redo.rows_formatted
    );
    out.push(("redo", redo));
    w.settled(1);
    // The first cell's first point, with its segment, circle, triangle
    // and intersection: every object after them is renumbered.
    let first = w.app.wb.construction().id("P_{0}").unwrap();
    let before = w.app.wb.construction().len();
    assert!(w.app.wb.delete(first));
    assert!(w.app.wb.construction().len() < before, "nothing deleted");
    let delete = w.step(Vec::new());
    assert_eq!(delete.rows_formatted, 0, "delete: rows made");
    out.push(("delete", delete));
    w.settled(1);
    out
}

impl Window {
    /// The value of every object, by label.
    fn values(&self) -> Vec<(String, Value)> {
        let c = self.app.wb.construction();
        c.ids()
            .map(|id| (c.label(id).to_owned(), c.value(id)))
            .collect()
    }

    /// How many objects hold another value than in `before` (by label; an
    /// object that is new counts).
    fn changed_since(&self, before: &[(String, Value)]) -> u64 {
        let c = self.app.wb.construction();
        let old: std::collections::HashMap<&str, Value> =
            before.iter().map(|(l, v)| (l.as_str(), *v)).collect();
        c.ids()
            .filter(|&id| old.get(c.label(id)) != Some(&c.value(id)))
            .count() as u64
    }
}

#[test]
fn frame_counters_stay_within_the_budget() {
    let small = states(9); // 108 objects
    let large = states(84); // 1008 objects
    for ((name, s), (_, l)) in small.iter().zip(&large) {
        println!("{name}: n100 {s:?}\n{name}: n1000 {l:?}");
    }
    let get = |v: &[(&str, Counts)], name: &str| v.iter().find(|(n, _)| *n == name).unwrap().1;
    // What each threshold protects:
    //
    // - growth from n100 to n1000 (9.3 times the objects): in the fitted
    //   view every object is on screen, so the drawing grows with the
    //   objects; the list paints its 15 or so visible rows at both sizes and
    //   colours and rows cost nothing per object, which keeps the frame at
    //   3 to 6 times. A list of all rows again (about five shapes, three
    //   texts and three allocations per row) or work per object for
    //   colours or rows goes past these. Zoomed in, culling keeps the
    //   drawing to the objects in view: 2 to 3.5 times.
    // - ceilings at n1000: the counts when they were set plus about 5%
    //   (shapes, texts, vertices) or 8% (allocations, bytes), against slow
    //   creep. Lower them with any change that lowers the counts.
    // - colour scans: none in any frame (the palette's cache); one call of
    //   `style::color` per object would scan all objects for each.
    // - rows: none made outside a drag or an edit; in a drag, an undo or a
    //   redo only those of the objects it changed, none after a deletion
    //   (checked in `states`).
    //
    // Growth caps in tenths: (shapes, texts, allocations, bytes).
    let growth = |name: &str| match name {
        "idle_zoomed_4x" => (25, 40, 30, 30),
        _ => (40, 70, 65, 50),
    };
    for (name, l) in &large {
        let s = get(&small, name);
        // No frame without input asks for an immediate repaint.
        if matches!(*name, "idle" | "idle_zoomed_4x" | "keyboard_open") {
            assert!(!s.immediate && !l.immediate, "{name}: immediate repaint");
        }
        let (shapes, texts, allocs, bytes) = growth(name);
        for (what, cap, l, s) in [
            ("shapes", shapes, l.shapes, s.shapes),
            ("texts", texts, l.texts, s.texts),
            ("allocations", allocs, l.allocs, s.allocs),
            ("bytes", bytes, l.bytes, s.bytes),
        ] {
            assert!(
                10 * l <= cap * s,
                "{name}: {what} {l} at n1000 against {s} at n100 (cap {cap} tenths)"
            );
        }
        if matches!(*name, "undo" | "redo" | "delete") {
            // A rebuild renumbers the objects: the polygons' sides are
            // found again, in one scan of the 108 or 1008 objects.
            assert!(
                s.colour_scans <= 108 && l.colour_scans <= 1008,
                "{name}: colours scanned"
            );
        } else {
            assert_eq!(
                (s.colour_scans, l.colour_scans),
                (0, 0),
                "{name}: colours scanned"
            );
        }
        // A drag and an edit change the list; their rows are checked in
        // `states`.
        if !matches!(*name, "drag_point" | "undo" | "redo" | "delete") {
            assert_eq!(
                (l.rows_formatted, l.rows_built),
                (0, 0),
                "{name}: rows made"
            );
        }
    }
    // Before the visible-only list and the culling the idle frame had
    // 10 887 shapes and 3467 texts.
    let ceilings = [
        ("idle", 2_550, 1_030, 94_000),
        ("idle_zoomed_4x", 840, 210, 27_000),
        ("keyboard_open", 2_250, 880, 78_000),
        ("tool_preview_segment", 2_550, 1_030, 94_000),
        ("pan", 2_550, 1_030, 94_000),
        ("drag_point", 2_550, 1_030, 94_000),
        ("undo", 2_550, 1_030, 94_000),
        ("redo", 2_550, 1_030, 94_000),
        ("delete", 2_550, 1_030, 94_000),
    ];
    for (name, shapes, texts, vertices) in ceilings {
        let l = get(&large, name);
        assert!(l.shapes <= shapes, "{name}: {} shapes", l.shapes);
        assert!(l.texts <= texts, "{name}: {} texts", l.texts);
        assert!(l.vertices <= vertices, "{name}: {} vertices", l.vertices);
    }
    // Before the shared colours, the row cache and the visible-only list
    // the idle frame made 16 696 allocations of 2.08 MB and a drag frame
    // 27 725 of 2.43 MB; a copy of all rows per frame adds about 4000
    // allocations, making them all again about 10 000.
    let ceilings = [
        ("idle", 2_900, 565_000),
        ("idle_zoomed_4x", 680, 170_000),
        ("keyboard_open", 2_600, 520_000),
        ("tool_preview_segment", 4_000, 570_000),
        ("pan", 3_100, 580_000),
        ("drag_point", 3_100, 580_000),
        ("drag_point_still", 3_100, 580_000),
        // The frames after an undo, a redo and a deletion. When a rebuild
        // made every row again (1008, or 1000 after the deletion) they made
        // 13 423 allocations of 0.88 MB (undo, redo) and 14 067 of 1.13 MB
        // (the deletion); now the rows that changed are made, and the
        // deletion lays out the rows that come into view.
        ("undo", 2_950, 635_000),
        ("redo", 2_950, 635_000),
        ("delete", 3_700, 905_000),
    ];
    for (name, allocs, bytes) in ceilings {
        let l = get(&large, name);
        assert!(l.allocs <= allocs, "{name}: {} allocations", l.allocs);
        assert!(l.bytes <= bytes, "{name}: {} bytes", l.bytes);
    }
    let still = get(&large, "drag_point_still");
    assert_eq!((still.rows_formatted, still.rows_built), (0, 0));
    assert!(get(&large, "drag_point").rows_formatted <= 12);
    // Zoomed in four times, a sixteenth of the area: the shapes follow.
    assert!(get(&large, "idle_zoomed_4x").shapes * 3 < get(&large, "idle").shapes);
}

#[test]
fn locus_frames_stay_within_the_budget() {
    let (mut script, mid) = grid(83);
    script +=
        "A = (0, 0)\nr = 0\nc = Circle(A, r)\nP = Point(c)\nQ = (x(P), y(P))\nL = Locus(Q, P)\n";
    let mut w = Window::with(&script, mid);
    let c = w.app.wb.construction();
    assert_eq!(c.len(), 1002);
    assert_eq!(c.value(c.id("L").unwrap()), Value::Locus { len: 10_001 });
    let idle = w.settled(1);
    println!("locus idle: {idle:?}");
    // The cached stroke cap measured 2395 shapes, 964 texts, 88811
    // vertices and 2655 allocations of 518425 bytes. Drawing each repeated
    // sample added about 80000 vertices and 262 kB of allocations.
    assert!(idle.shapes <= 2_450 && idle.texts <= 1_000, "{idle:?}");
    assert!(idle.vertices <= 92_000, "{idle:?}");
    assert!(idle.allocs <= 2_850 && idle.bytes <= 560_000, "{idle:?}");
    assert_eq!((idle.rows_formatted, idle.rows_built), (0, 0));
    assert!(!idle.immediate);
}

#[test]
fn large_chart_frames_stay_within_the_budget() {
    let (mut script, mid) = grid(83);
    script += "edges = Sequence(k / 250, k, -1000, 1000)\n";
    script += "samples = Sequence((k + 0.5) / 250, k, -1000, 999)\n";
    script += "histogram = Histogram(edges, samples, false)\n";
    script += "dots = DotPlot(samples)\n";
    let mut w = Window::with(&script, mid);
    let c = w.app.wb.construction();
    assert_eq!(
        c.ids().filter(|&id| !c.label(id).starts_with('\0')).count(),
        1000
    );
    for label in ["histogram", "dots"] {
        let chart = c.chart(c.id(label).unwrap()).unwrap();
        assert!(chart.marks.len() >= 2000, "{label}: {}", chart.marks.len());
    }
    w.settled(1);
    let builds = crate::draw::charts::builds();
    let mut measure = |panning| {
        let mut times = [0.0_f64; 31];
        let mut counts = Counts::default();
        for (i, ms) in times.iter_mut().enumerate() {
            if panning {
                let dx = if i % 2 == 0 { 6.0 } else { -6.0 };
                w.app.camera.as_mut().unwrap().pan_pixels(dx, 0.0);
            }
            let start = std::time::Instant::now();
            counts = w.frames(1);
            *ms = start.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(crate::draw::charts::builds(), builds);
            assert_eq!(counts.raster_quads, 2, "{counts:?}");
            assert!(
                counts.shapes <= 2_600 && counts.texts <= 1_040,
                "{counts:?}"
            );
            assert!(counts.vertices <= 100_000, "{counts:?}");
            assert!(
                counts.allocs <= 3_100 && counts.bytes <= 850_000,
                "{counts:?}"
            );
            assert_eq!((counts.rows_formatted, counts.rows_built), (0, 0));
        }
        times.sort_by(f64::total_cmp);
        (counts, times[15], times[29])
    };
    let (idle, idle_median, idle_p95) = measure(false);
    let (pan, pan_median, pan_p95) = measure(true);
    println!(
        "charts idle (median {idle_median:.2} ms, p95 {idle_p95:.2} ms): {idle:?}; \
         pan (median {pan_median:.2} ms, p95 {pan_p95:.2} ms): {pan:?}"
    );
}

#[test]
fn raster_frames_stay_within_the_budget() {
    use geo_kernel::kernel::image::{ImagePlacement, ImageStyle};
    use geo_kernel::script::image::definition;

    let (mut script, mid) = grid(82);
    let mut fixture = Workbench::default();
    fixture
        .load_bytes(
            include_bytes!("../../../golden/ggb/saved/saved-images-001.ggb"),
            "raster",
        )
        .unwrap();
    let snapshot = fixture.snapshot_text();
    script += snapshot
        .lines()
        .find(|line| line.starts_with("$imageAsset("))
        .unwrap();
    script.push('\n');
    let name = &fixture
        .construction()
        .image(fixture.construction().id("fixedImage").unwrap())
        .unwrap()
        .name;
    for i in 0..16 {
        let placement = ImagePlacement::Screen([30.0 + f64::from(i) * 14.0, 60.0]);
        let style = ImageStyle {
            background: i % 2 == 0,
            ..ImageStyle::default()
        };
        script += &format!("image_{{{i}}} = {}\n", definition(name, placement, style));
    }

    let mut w = Window::with(&script, mid);
    assert_eq!(w.app.wb.construction().len(), 1000);
    let idle = w.settled(1);
    w.app.camera.as_mut().unwrap().cx += 1.0;
    let panned = w.frames(1);
    for counts in [idle, panned] {
        assert_eq!(counts.raster_quads, 16, "{counts:?}");
        assert!(
            counts.shapes <= 2_600 && counts.texts <= 1_040,
            "{counts:?}"
        );
        assert!(counts.vertices <= 95_000, "{counts:?}");
        assert!(
            counts.allocs <= 2_900 && counts.bytes <= 575_000,
            "{counts:?}"
        );
        assert_eq!((counts.rows_formatted, counts.rows_built), (0, 0));
    }
}

/// The benchmark's grid of 45 cells (540 objects, with the triangles'
/// sides) with 50 groups of three
/// free points and four conics on them (two ellipses, two hyperbolas: 350
/// objects, 200 conics) and 100 functions: 990 objects. The conics and the
/// functions do not read the grid's points.
fn curve_scene() -> (String, String) {
    let (mut s, mid) = grid(45);
    for j in 0..50 {
        let (x, y) = ((j % 10) as f64 * 4.0 - 18.0, (j / 10) as f64 * 8.0 - 18.0);
        let p = |n: &str| format!("{n}_{{{j}}}");
        let (a, b, c) = (p("A"), p("B"), p("C"));
        for line in [
            format!("{a} = ({x}, {y})"),
            format!("{b} = ({}, {y})", x + 1.5),
            format!("{c} = ({}, {})", x + 0.5, y + 1.2),
            format!("{} = Ellipse({a}, {b}, {c})", p("e")),
            format!("{} = Ellipse({b}, {c}, {a})", p("E")),
            format!("{} = Hyperbola({a}, {b}, {c})", p("h")),
            format!("{} = Hyperbola({b}, {c}, {a})", p("H")),
        ] {
            s += &line;
            s.push('\n');
        }
    }
    for j in 1..=100 {
        s += &format!("f{j}(x) = sin(x + {j}) + {} / 4\n", j as f64 - 50.0);
    }
    (s, mid)
}

/// What a frame of the curve scene does: its counts, and how many curves
/// it plotted and tessellated.
#[derive(Clone, Copy, Debug)]
struct CurveFrame {
    counts: Counts,
    plotted: u64,
    /// Vertices copied to place kept meshes before handing them to egui.
    moved: u64,
    /// What the plots counted against `REPLOTS_PER_FRAME`.
    cost: u64,
    tessellated: u64,
    /// Not asserted: printed to compare by hand (release builds only mean
    /// anything).
    ms: f64,
}

fn curve_step(w: &mut Window, events: Vec<Event>) -> CurveFrame {
    let (p, t, k) = (
        crate::curves::curves_plotted(),
        crate::curves::curves_tessellated(),
        crate::curves::curves_plot_cost(),
    );
    let moved = crate::curves::curves_moved_vertices();
    let start = std::time::Instant::now();
    let counts = w.step(events);
    CurveFrame {
        ms: start.elapsed().as_secs_f64() * 1e3,
        counts,
        plotted: crate::curves::curves_plotted() - p,
        cost: crate::curves::curves_plot_cost() - k,
        moved: crate::curves::curves_moved_vertices() - moved,
        tessellated: crate::curves::curves_tessellated() - t,
    }
}

impl Window {
    /// A grid-free spot on the point `name` that no label covers.
    fn bare(&self, name: &str) -> bool {
        let (rect, _) = self.app.view.unwrap();
        let q = self.point(name);
        self.app
            .label_at(((q.x - rect.min.x) as f64, (q.y - rect.min.y) as f64))
            .is_none()
    }

    /// Drags the point `name` by 50 points in `steps` frames of 50 points
    /// each after the press: the frames of the drag after the first move.
    fn drag_point(&mut self, name: &str, steps: usize) -> Vec<CurveFrame> {
        let wait = (self.app.metrics.drag_delay / DT).ceil() as usize + 1;
        let p = self.point(name);
        self.step(vec![Event::PointerMoved(p)]);
        self.step(vec![press(p, true)]);
        self.frames(wait);
        // Steps of a grid cell or more: grid snapping must not keep the
        // point where it was.
        self.step(vec![Event::PointerMoved(p + vec2(14.0, 4.0))]);
        self.step(vec![Event::PointerMoved(p + vec2(40.0, 4.0))]);
        assert!(self.app.drag.is_some(), "no drag of {name}");
        let mut out = Vec::new();
        let mut at = p + vec2(40.0, 4.0);
        for _ in 0..steps {
            let before = self.point(name);
            at += vec2(50.0, 0.0);
            out.push(curve_step(self, vec![Event::PointerMoved(at)]));
            let moved = (self.point(name) - before).length();
            assert!(moved > 20.0, "{name} did not move");
        }
        // The frame after the last move.
        out.push(curve_step(self, vec![Event::PointerMoved(at)]));
        self.step(vec![press(at, false)]);
        self.frames(3);
        out
    }
}

#[test]
fn curve_frames_stay_within_the_budget() {
    use crate::curves::REPLOTS_PER_FRAME;
    let (script, mid) = curve_scene();
    let mut w = Window::with(&script, mid);
    let c = w.app.wb.construction();
    assert_eq!(c.len(), 990);
    assert_eq!(c.ids().filter(|&id| c.kind(id) == Kind::Conic).count(), 200);
    assert_eq!(
        c.ids().filter(|&id| c.kind(id) == Kind::Function).count(),
        100
    );
    w.settled(1);
    let idle = curve_step(&mut w, Vec::new());

    // A drag of a grid point that no conic or function reads.
    let mid: usize = w
        .mid
        .trim_matches(|ch| ch == '{' || ch == '}')
        .parse()
        .unwrap();
    let grid = (mid..)
        .map(|k| format!("P_{{{k}}}"))
        .find(|n| w.bare(n))
        .unwrap();
    let unrelated = w.drag_point(&grid, 2);
    // A drag of a focus of two ellipses and two hyperbolas.
    let focus = (0..50)
        .map(|j| format!("A_{{{j}}}"))
        .find(|n| w.bare(n))
        .unwrap();
    let related = w.drag_point(&focus, 2);
    w.settled(1);

    // A pan by 4 points a frame, 60 frames: the kept drawings move; past
    // half the margin they are plotted again, a bounded number a frame.
    let wait = (w.app.metrics.drag_delay / DT).ceil() as usize + 1;
    let from = w.empty_spot();
    w.step(vec![Event::PointerMoved(from)]);
    w.step(vec![press(from, true)]);
    w.frames(wait);
    w.step(vec![Event::PointerMoved(from + vec2(4.0, 0.0))]);
    assert!(w.app.gesture.panning() && w.app.drag.is_none(), "no pan");
    let pans: Vec<CurveFrame> = (2..62)
        .map(|k| {
            curve_step(
                &mut w,
                vec![Event::PointerMoved(from + vec2(4.0 * k as f32, 0.0))],
            )
        })
        .collect();
    w.step(vec![press(from + vec2(244.0, 0.0), false)]);
    w.step(vec![Event::PointerGone]);
    w.settled(1);

    // A zoom step (a quarter in) about the middle of the area: at most
    // `REPLOTS_PER_FRAME` curves plotted, the others drawn from their kept
    // polylines, and the next frames finish the job.
    let r = w.app.view.unwrap().0;
    let (aw, ah) = (r.width() as f64, r.height() as f64);
    let mut cam = w.app.camera.unwrap();
    cam.zoom_at(1.25, aw / 2.0, ah / 2.0, aw, ah, view::ScaleLimits::OURS);
    w.app.camera = Some(cam);
    let zoom = curve_step(&mut w, Vec::new());
    let mut after_zoom = Vec::new();
    for _ in 0..60 {
        let f = curve_step(&mut w, Vec::new());
        after_zoom.push(f);
        if !f.counts.immediate {
            break;
        }
    }

    w.settled(1);

    // Edits that change no curve's drawing: the frame after each plots
    // nothing. Undo, redo and a deletion rebuild the construction, which
    // renumbers the objects; the kept drawings are found by label.
    let mut edits = Vec::new();
    w.app.wb.run_line("Z = (1, 1)").unwrap();
    edits.push(("add_point", curve_step(&mut w, Vec::new())));
    w.settled(1);
    assert!(w.app.wb.undo());
    edits.push(("undo", curve_step(&mut w, Vec::new())));
    w.settled(1);
    assert!(w.app.wb.redo());
    edits.push(("redo", curve_step(&mut w, Vec::new())));
    w.settled(1);
    // An early object, so that every curve after it is renumbered.
    let first = w.app.wb.construction().id("P_{0}").unwrap();
    assert!(w.app.wb.delete(first));
    edits.push(("delete", curve_step(&mut w, Vec::new())));
    w.settled(1);
    let f1 = w.app.wb.construction().id("f1").unwrap();
    w.app.wb.selected = Some(f1);
    edits.push(("select", curve_step(&mut w, Vec::new())));
    w.app.wb.selected = None;
    w.settled(1);
    // The window grows by 10 points: the kept drawings move at once; the
    // functions are plotted for the new area over the next frames. The
    // view this starts from is the fit the document opened with, one
    // scale on both axes.
    w.screen = (SCREEN.0 + 10.0, SCREEN.1 + 10.0);
    let mut resize = vec![curve_step(&mut w, Vec::new())];
    while resize.last().unwrap().counts.immediate && resize.len() < 60 {
        resize.push(curve_step(&mut w, Vec::new()));
    }
    w.screen = SCREEN;
    w.settled(1);

    // The same two gestures from an anisotropic view — the scales an axis
    // drag or a zoom rectangle leaves apart (spec D1), here x at 40 px a
    // unit against y at 60. The plot frame letterboxes on y then (its
    // scale is the smaller 40), so the zoom frame places every kept mesh
    // through the frame-to-canvas affine, and the resize — the scales
    // standing apart but still — moves them as a whole through the
    // stretch, its letterbox offset included.
    let r = w.app.view.unwrap().0;
    let (aw, ah) = (r.width() as f64, r.height() as f64);
    let mut cam = w.app.camera.unwrap();
    cam.xscale = 40.0;
    cam.yscale = 60.0;
    w.app.camera = Some(cam);
    while curve_step(&mut w, Vec::new()).counts.immediate {}
    let mut cam = w.app.camera.unwrap();
    cam.zoom_at(1.25, aw / 2.0, ah / 2.0, aw, ah, view::ScaleLimits::OURS);
    w.app.camera = Some(cam);
    let aniso_zoom = curve_step(&mut w, Vec::new());
    while curve_step(&mut w, Vec::new()).counts.immediate {}
    w.settled(1);
    w.screen = (SCREEN.0 + 10.0, SCREEN.1 + 10.0);
    let mut aniso_resize = vec![curve_step(&mut w, Vec::new())];
    while aniso_resize.last().unwrap().counts.immediate && aniso_resize.len() < 60 {
        aniso_resize.push(curve_step(&mut w, Vec::new()));
    }
    w.screen = SCREEN;
    w.settled(1);

    let show = |name: &str, f: &CurveFrame| {
        println!(
            "curves {name}: {:?}, plotted {} (cost {}), tessellated {}, moved {}, {:.2} ms",
            f.counts, f.plotted, f.cost, f.tessellated, f.moved, f.ms
        );
    };
    show("idle", &idle);
    show("drag_unrelated", &unrelated[0]);
    show("drag_unrelated_next", &unrelated[2]);
    show("drag_related", &related[0]);
    show("pan", &pans[0]);
    let worst_pan = pans.iter().max_by_key(|f| f.plotted).unwrap();
    show("pan_worst", worst_pan);
    let slowest = pans.iter().map(|f| f.ms).fold(0.0, f64::max);
    if let Some(f) = pans.iter().max_by(|a, b| a.ms.total_cmp(&b.ms)) {
        show("pan_slowest_frame", f);
    }
    println!("curves pan_slowest: {slowest:.2} ms");
    show("zoom", &zoom);
    for (i, f) in after_zoom.iter().enumerate() {
        show(&format!("after_zoom_{i}"), f);
    }
    for (name, f) in &edits {
        show(name, f);
    }
    for (i, f) in resize.iter().enumerate() {
        show(&format!("resize_{i}"), f);
    }
    show("aniso_zoom", &aniso_zoom);
    for (i, f) in aniso_resize.iter().enumerate() {
        show(&format!("aniso_resize_{i}"), f);
    }

    // Idle: nothing plotted, no rows, no repaint asked for.
    assert_eq!((idle.plotted, idle.tessellated), (0, 0), "idle");
    assert!(!idle.counts.immediate, "idle: immediate repaint");
    assert_eq!((idle.counts.rows_formatted, idle.counts.rows_built), (0, 0));
    // A drag no curve depends on: nothing plotted.
    for f in &unrelated {
        assert_eq!((f.plotted, f.tessellated), (0, 0), "unrelated drag");
        assert!(
            f.counts.rows_formatted <= 12,
            "{} rows made",
            f.counts.rows_formatted
        );
    }
    // A drag of a focus: its four conics, each frame it moves.
    for f in &related[..2] {
        assert_eq!((f.plotted, f.tessellated), (4, 4), "drag of {focus}");
    }
    assert_eq!((related[2].plotted, related[2].tessellated), (0, 0));
    // The first pan frames move every kept drawing: nothing plotted or
    // tessellated. Later frames spend at most `REPLOTS_PER_PAN_FRAME` on
    // plots, a function counting as `FUNCTION_COST`.
    let quiet = pans.iter().take_while(|f| f.plotted == 0).count();
    println!("curves pan_quiet_frames: {quiet}");
    assert!(quiet >= 20, "only {quiet} pan frames plotted nothing");
    for f in &pans {
        assert_eq!(f.plotted, f.tessellated, "a pan re-tessellated a curve");
        assert!(
            f.cost <= crate::curves::REPLOTS_PER_PAN_FRAME as u64,
            "{} plotted, costing {}",
            f.plotted,
            f.cost
        );
    }
    assert!(
        pans.iter().any(|f| f.plotted > 0),
        "the pan never plotted again"
    );
    // The zoom frame: the most it may plot, the rest moved and scaled.
    // 258 curves meet the zoomed view: 24 plotted, 234 drawn from their
    // kept meshes placed point by point, none tessellated again.
    assert_eq!((zoom.plotted, zoom.tessellated), (24, 24));
    assert!(
        zoom.cost <= REPLOTS_PER_FRAME as u64,
        "zoom cost {}",
        zoom.cost
    );
    assert_eq!(REPLOTS_PER_FRAME, 24);
    assert!(zoom.counts.immediate, "the zoom asks for the next frame");
    let last = after_zoom.last().unwrap();
    assert!(
        !last.counts.immediate,
        "the frames after a zoom never settle"
    );
    for f in &after_zoom {
        assert!(f.cost <= REPLOTS_PER_FRAME as u64, "cost {}", f.cost);
        assert_eq!(f.plotted, f.tessellated);
    }
    println!("curves after_zoom_frames: {}", after_zoom.len());
    // The load's fit keeps 3% free around the content (the reference's
    // `calculateRectangleOfAllObjects`), where the port's first fit kept
    // 8%: the closer view meets one curve fewer, and the zoom settles a
    // frame earlier than it did there.
    assert_eq!(after_zoom.len(), 23, "frames until the zoom is done");
    for (name, f) in &edits {
        assert_eq!((f.plotted, f.tessellated), (0, 0), "{name}");
    }
    // The rows: the new point's after it was added and after the redo,
    // none after the undo and the deletion (the other rows are kept).
    let rows: Vec<u64> = edits.iter().map(|(_, f)| f.counts.rows_formatted).collect();
    assert_eq!(rows, [1, 0, 1, 0, 0], "rows made by the edits");
    assert!(
        !resize.last().unwrap().counts.immediate,
        "the resize never settles"
    );
    println!("curves resize_frames: {}", resize.len());
    // The frame of the resize moves the kept drawings, re-tessellating
    // none; a curve that comes into the grown area with a plot kept from
    // before the zoom (one hyperbola here) is plotted.
    assert_eq!(resize[0].plotted, resize[0].tessellated);
    assert_eq!(resize[0].moved, 0, "resize copied kept vertices");
    for f in &resize {
        assert!(f.cost <= REPLOTS_PER_FRAME as u64, "cost {}", f.cost);
    }
    assert!(
        !aniso_resize.last().unwrap().counts.immediate,
        "the anisotropic resize never settles"
    );
    println!("curves aniso_resize_frames: {}", aniso_resize.len());
    // The same placement, no new tessellation, at the replot cap.
    assert!(
        aniso_zoom.counts.immediate,
        "the zoom asks for the next frame"
    );
    assert_eq!(aniso_zoom.plotted, aniso_zoom.tessellated);
    assert_eq!(aniso_resize[0].plotted, aniso_resize[0].tessellated);
    assert_eq!(
        aniso_resize[0].moved, 0,
        "anisotropic resize copied kept vertices"
    );
    for f in &aniso_resize {
        assert!(f.cost <= REPLOTS_PER_FRAME as u64, "cost {}", f.cost);
    }
    let heaviest = *after_zoom.iter().max_by_key(|f| f.counts.bytes).unwrap();
    let heaviest_resize = *resize.iter().max_by_key(|f| f.counts.bytes).unwrap();
    let heaviest_aniso = *aniso_resize.iter().max_by_key(|f| f.counts.bytes).unwrap();
    // Ceilings: the counts when they were set plus about 8%. The zoom and the
    // resize start from the view the document opened with, one scale on
    // both axes: the zoom frame places the kept meshes again, scaled the
    // same way, at the port's old ceiling (3 700 allocations of 7.2 MB;
    // 3 218 of 7.1 MB here — the per-axis fit this branch first tried,
    // which stretched every mesh, measured 6 000 of 7.6 MB). The frame
    // after a rebuild (undo, redo, delete) finds what each function
    // depends on again (about 0.25 ms, most of it formatting the
    // definitions for their fingerprints: about 850 allocations of
    // 300 kB). When it also made the 990 rows again it made 17 400 to
    // 18 200 allocations of 1.1 to 1.4 MB.
    //
    // A resize keeps the centre of the view (spec D11), moving the kept
    // meshes by (5, 5) points through the painter's transform. The first
    // frame measured 2 260 allocations and 518 500 bytes. Peak bytes were
    // 1 560 636 while replotting up to six functions per frame, within
    // the original 2 550 allocations and 1 750 000 bytes. The anisotropic
    // resize measured peaks of 1 414 allocations and 1 720 723 bytes.
    let most_moved = pans.iter().map(|f| f.moved).max().unwrap();
    println!("curves pan_most_moved: {most_moved}");
    assert!(
        most_moved <= 210_000,
        "a pan frame moved {most_moved} vertices"
    );
    assert!(
        zoom.moved <= 160_000,
        "the zoom placed {} vertices",
        zoom.moved
    );
    for (name, f, allocs, bytes) in [
        ("idle", idle, 2_650, 545_000),
        ("drag_unrelated", unrelated[0], 2_800, 555_000),
        ("drag_related", related[0], 3_000, 750_000),
        ("pan", pans[0], 4_050, 8_250_000),
        ("pan_worst", *worst_pan, 4_200, 8_400_000),
        ("zoom", zoom, 3_700, 7_200_000),
        ("after_zoom", heaviest, 2_800, 4_150_000),
        ("resize", heaviest_resize, 2_550, 1_750_000),
        ("aniso_zoom", aniso_zoom, 2_450, 4_600_000),
        ("aniso_resize", heaviest_aniso, 1_550, 1_900_000),
        ("add_point", edits[0].1, 2_200, 505_000),
        ("undo", edits[1].1, 3_250, 870_000),
        ("redo", edits[2].1, 3_300, 875_000),
        ("delete", edits[3].1, 4_050, 1_150_000),
        ("select", edits[4].1, 2_100, 480_000),
    ] {
        let c = f.counts;
        assert!(c.allocs <= allocs, "{name}: {} allocations", c.allocs);
        assert!(c.bytes <= bytes, "{name}: {} bytes", c.bytes);
    }
    // A fast pan, 24 points a frame for 40 frames: printed, only the cap
    // asserted.
    let from = w.empty_spot();
    w.step(vec![Event::PointerMoved(from)]);
    w.step(vec![press(from, true)]);
    w.frames(wait);
    let uncovered = crate::curves::curves_uncovered();
    let mut fast_ms = 0.0f64;
    for k in 1..=40 {
        let f = curve_step(
            &mut w,
            vec![Event::PointerMoved(from - vec2(24.0 * k as f32, 0.0))],
        );
        fast_ms = fast_ms.max(f.ms);
        assert!(
            f.cost <= crate::curves::REPLOTS_PER_PAN_FRAME as u64,
            "fast pan cost {}",
            f.cost
        );
    }
    println!(
        "curves fast_pan: {} curve draws past the margin in 40 frames, slowest {fast_ms:.2} ms",
        crate::curves::curves_uncovered() - uncovered
    );
    w.step(vec![press(from - vec2(960.0, 0.0), false)]);
    w.step(vec![Event::PointerGone]);
    w.settled(1);
}

/// `n` LaTeX texts, every anchor inside the viewport (screen anchors, so
/// fitting the view to the content cannot hide most of the load).
fn formula_script(n: usize) -> String {
    use geo_kernel::script::text::literal;
    use std::fmt::Write;

    let mut script = String::new();
    for i in 0..n {
        let formula = literal(&format!(r"\frac{{{i}}}{{x^2}}"));
        writeln!(script, "t{i} = {formula}").unwrap();
        writeln!(
            script,
            "$textProperties(t{i}, true, 0.65, 0, -, -, screen, {}, {})",
            8 + i % 25 * 39,
            8 + i / 25 * 19
        )
        .unwrap();
    }
    script
}

#[test]
fn a_thousand_visible_formulas_reuse_their_layouts() {
    let mut w = Window::with(&formula_script(1000), String::new());
    assert_eq!(w.app.wb.construction().len(), 1000);
    let idle = w.settled(5);
    let built = crate::latex::layouts_built();
    let mut elapsed = Vec::new();
    for _ in 0..15 {
        let start = std::time::Instant::now();
        let counts = w.step(Vec::new());
        elapsed.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!((counts.rows_formatted, counts.rows_built), (0, 0));
        assert!(!counts.immediate);
    }
    elapsed.sort_by(f64::total_cmp);
    println!(
        "formula frame median {:.3} ms, p95 {:.3} ms, {idle:?}",
        elapsed[7], elapsed[14]
    );
    assert_eq!(crate::latex::layouts_built(), built);
    assert!(
        idle.vertices >= 14_000 && idle.vertices <= 40_000,
        "{idle:?}"
    );
    assert!(
        idle.shapes <= 4700 && idle.allocs <= 450 && idle.bytes <= 600_000,
        "{idle:?}"
    );
}

/// The cold first frame of the 1000-formula scene: every formula is parsed
/// and laid out once, the drawing's thousand and the algebra rows' own
/// thousand. Measured at 512 ms alone and 1.1 s beside the other budget
/// tests on an M-series debug build (the settled frames after it run at
/// 10 ms in a debug build, 0.9 ms in the release benchmark); the bound
/// holds both with room for slower machines, and catches a cache that
/// re-lays-out formulas it should already hold.
#[test]
fn the_first_formula_frame_fits_its_budget() {
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    let mut w = Window {
        ctx,
        app: App::new(Workbench::from_script(&formula_script(1000)).unwrap(), None),
        time: 0.0,
        mid: String::new(),
        screen: SCREEN,
    };
    let start = std::time::Instant::now();
    let counts = w.step(Vec::new());
    let cold_ms = start.elapsed().as_secs_f64() * 1000.0;
    let built = crate::latex::layouts_built();
    println!("formula cold frame {cold_ms:.2} ms: {counts:?}, {built} layouts");
    // The drawing's formulas and the algebra rows' own, one per text
    // each, and nothing else: the next frame builds none of them again.
    assert_eq!(built, 2000);
    assert!(cold_ms <= 2000.0, "cold frame {cold_ms:.2} ms, cap 2000 ms");
}

/// 3000 LaTeX texts, past the 2048-entry clear-on-full cliff the layout
/// cache had: a settled frame builds no layout again and paints within
/// the formula scene's caps (only the rows that reach the drawing paint;
/// every one of the 3000 is laid out and cached).
#[test]
fn three_thousand_formulas_stay_cached_above_the_old_cliff() {
    let mut w = Window::with(&formula_script(3000), String::new());
    assert_eq!(w.app.wb.construction().len(), 3000);
    let idle = w.settled(5);
    let built = crate::latex::layouts_built();
    let mut elapsed = Vec::new();
    for _ in 0..15 {
        let start = std::time::Instant::now();
        let counts = w.step(Vec::new());
        elapsed.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!((counts.rows_formatted, counts.rows_built), (0, 0));
        assert!(!counts.immediate);
    }
    elapsed.sort_by(f64::total_cmp);
    println!(
        "3000-formula frame median {:.3} ms, p95 {:.3} ms, {idle:?}",
        elapsed[7], elapsed[14]
    );
    assert_eq!(crate::latex::layouts_built(), built);
    assert!(
        idle.vertices >= 14_000 && idle.vertices <= 46_000,
        "{idle:?}"
    );
    assert!(
        idle.shapes <= 5300 && idle.allocs <= 300 && idle.bytes <= 640_000,
        "{idle:?}"
    );
}

/// One 30 KB formula, past the quarter of the weight budget the cache
/// once refused to store at all: it is kept by handle like any other, and
/// a settled frame builds nothing.
#[test]
fn a_thirty_kilobyte_formula_is_cached_like_any_other() {
    use geo_kernel::script::text::literal;
    use std::fmt::Write;

    let mut script = String::new();
    let formula = literal(&format!(r"\text{{{}}}", "a".repeat(30_000)));
    writeln!(script, "t = {formula}").unwrap();
    writeln!(
        script,
        "$textProperties(t, true, 0.65, 0, -, -, screen, 40, 40)"
    )
    .unwrap();
    let mut w = Window::with(&script, String::new());
    let idle = w.settled(5);
    let built = crate::latex::layouts_built();
    let mut elapsed = Vec::new();
    for _ in 0..15 {
        let start = std::time::Instant::now();
        let counts = w.step(Vec::new());
        elapsed.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!((counts.rows_formatted, counts.rows_built), (0, 0));
        assert!(!counts.immediate);
    }
    elapsed.sort_by(f64::total_cmp);
    println!(
        "30 KB formula frame median {:.3} ms, p95 {:.3} ms, {idle:?}",
        elapsed[7], elapsed[14]
    );
    assert_eq!(crate::latex::layouts_built(), built);
    // Thirty thousand glyphs are a quarter million vertices by nature; the
    // caps document that size (measured 249 196) and the settled frame's
    // own cost (417 shapes, 174 allocations of 62 912 bytes).
    assert!(idle.vertices <= 260_000, "{idle:?}");
    assert!(
        idle.shapes <= 460 && idle.allocs <= 220 && idle.bytes <= 80_000,
        "{idle:?}"
    );
}

#[test]
fn a_thousand_canvas_sliders_and_checkboxes_fit_the_frame_budget() {
    use geo_kernel::kernel::control::{CheckboxStyle, SliderStyle};
    use geo_kernel::script::saved::Property;
    use std::fmt::Write;

    for checkboxes in [false, true] {
        let mut script = String::new();
        let mut hidden = String::new();
        for i in 0..1000 {
            let label = format!("v{i}");
            let x = 12 + 28 * (i % 32);
            let y = 24 + 23 * (i / 32);
            let property = if checkboxes {
                writeln!(script, "{label} = $checkBox(true, _78)").unwrap();
                Property::CheckboxStyle {
                    label: label.clone(),
                    style: CheckboxStyle {
                        position: Some([x, y]),
                        fixed: false,
                    },
                }
            } else {
                writeln!(script, "{label} = Slider(-1, 1)").unwrap();
                Property::SliderStyle {
                    label: label.clone(),
                    style: SliderStyle {
                        position: Some((f64::from(x), f64::from(y))),
                        width: 25.0,
                        ..SliderStyle::default()
                    },
                }
            };
            writeln!(script, "{property}").unwrap();
            writeln!(hidden, "SetVisibleInView({label}, 1, false)").unwrap();
        }
        let mut visible = Window::with(&script, String::new());
        assert_eq!(visible.app.wb.construction().len(), 1000);
        let (rect, _) = visible.app.view.unwrap();
        assert!(rect.width() > 960.0 && rect.height() > 775.0, "{rect:?}");
        let shown = visible.settled(2);
        script.push_str(&hidden);
        let before = Window::with(&script, String::new()).settled(2);
        let mut elapsed = [0.0_f64; 21];
        for ms in &mut elapsed {
            let start = std::time::Instant::now();
            visible.step(Vec::new());
            *ms = start.elapsed().as_secs_f64() * 1000.0;
        }
        elapsed.sort_by(f64::total_cmp);
        let name = if checkboxes { "checkboxes" } else { "sliders" };
        let median = elapsed[elapsed.len() / 2];
        println!("{name}: {median:.3} ms, hidden {before:?}, visible {shown:?}");
        let at = rect.min.egui()
            + if checkboxes {
                vec2(25.0, 37.0)
            } else {
                // The last row has no slider label from the row below it.
                vec2(24.5, 737.0)
            };
        let label = if checkboxes { "v0" } else { "v992" };
        let value = |window: &Window| {
            let c = window.app.wb.construction();
            c.value(c.id(label).unwrap())
        };
        visible.step(vec![Event::PointerMoved(at), press(at, true)]);
        if !checkboxes {
            let wait = (visible.app.metrics.drag_delay / DT).ceil() as usize + 1;
            visible.frames(wait);
        }
        let mut active_times = [0.0_f64; 21];
        let mut active = Counts::default();
        for (i, ms) in active_times.iter_mut().enumerate() {
            let events = if checkboxes {
                vec![press(at, false)]
            } else {
                let dx = if i % 2 == 0 { 16.0 } else { 2.0 };
                vec![Event::PointerMoved(at + vec2(dx, -2.0))]
            };
            let old = value(&visible);
            let start = std::time::Instant::now();
            let counts = visible.step(events);
            *ms = start.elapsed().as_secs_f64() * 1000.0;
            assert_ne!(
                value(&visible),
                old,
                "{name}: the control did not change in active frame {i}"
            );
            assert_eq!(
                counts.raster_quads, shown.raster_quads,
                "{name}: {counts:?}"
            );
            assert_eq!(counts.texts, shown.texts, "{name}: {counts:?}");
            assert!(
                counts.shapes <= 4700
                    && counts.vertices <= 60_000
                    && counts.allocs <= 3500
                    && counts.bytes <= 1_500_000,
                "{name}: {counts:?}"
            );
            assert!(
                counts.rows_formatted <= 2 && counts.rows_built <= 2,
                "{counts:?}"
            );
            assert_eq!(counts.colour_scans, 0, "{counts:?}");
            if i == 0 {
                active = counts;
            }
            if checkboxes {
                visible.step(vec![press(at, true)]);
            }
        }
        active_times.sort_by(f64::total_cmp);
        let active_median = active_times[active_times.len() / 2];
        println!("{name}: active {active_median:.3} ms, {active:?}");
        assert_eq!(shown.texts - before.texts, 1000, "{shown:?}");
        assert!(
            shown.vertices > before.vertices && shown.vertices <= 60_000,
            "{shown:?}"
        );
        assert!(
            shown.shapes <= 4700 && shown.allocs <= 3500 && shown.bytes <= 1_500_000,
            "{shown:?}"
        );
        assert_eq!(
            (shown.rows_formatted, shown.rows_built, shown.colour_scans),
            (0, 0, 0)
        );
        if !cfg!(debug_assertions) {
            assert!(median <= 4.0, "{name}: median {median:.3} ms (budget 4 ms)");
            assert!(
                active_median <= 4.0,
                "{name}: active median {active_median:.3} ms (budget 4 ms)"
            );
        }
    }
}

#[test]
fn a_thousand_cas_rows_only_render_the_visible_range() {
    let mut w = Window::with("", String::new());
    w.app.show_cas = true;
    for i in 0..1000 {
        assert!(w.app.wb.submit_cas(i, &format!("{i} / 1009")));
    }
    let counts = w.settled(5);
    let layouts = crate::latex::layouts_built();
    let start = std::time::Instant::now();
    assert_eq!(w.frames(60), counts);
    let mean_ms = start.elapsed().as_secs_f64() * 1000.0 / 60.0;
    println!("CAS idle frame: {mean_ms:.2} ms, {counts:?}");
    assert_eq!(crate::latex::layouts_built(), layouts);
    assert!(counts.texts <= 120, "{counts:?}");
    assert!(counts.shapes < 800, "{counts:?}");
    assert!(counts.allocs < 500, "{counts:?}");
    assert!(counts.bytes < 200_000, "{counts:?}");
    assert!(counts.vertices < 17_000, "{counts:?}");
    assert!(!counts.immediate, "{counts:?}");
}

#[test]
fn a_thousand_spreadsheet_cells_fit_the_frame_budget() {
    let mut w = Window::with("FillCells(A1:J100, 1)", String::new());
    w.app.show_spreadsheet = true;
    assert_eq!(w.app.wb.construction().spreadsheet_cells().count(), 1_000);
    let counts = w.settled(5);
    let start = std::time::Instant::now();
    assert_eq!(w.frames(60), counts);
    let mean_ms = start.elapsed().as_secs_f64() * 1000.0 / 60.0;
    println!("Spreadsheet idle frame: {mean_ms:.2} ms, {counts:?}");
    assert!(counts.texts <= 240, "{counts:?}");
    assert!(counts.shapes < 800, "{counts:?}");
    assert_eq!(counts.rows_formatted, 0, "{counts:?}");
    assert_eq!(counts.rows_built, 0, "{counts:?}");
    assert!(!counts.immediate, "{counts:?}");
    if !cfg!(debug_assertions) {
        assert!(mean_ms <= 4.0, "{mean_ms:.2} ms exceeds the 4 ms budget");
    }
}
