//! The frame budget as counters: what one frame of the window produces at
//! 100 and 1000 objects, in the states the frame benchmark measures (idle,
//! zoomed in, keyboard open, tool preview, pan, drag). Times are not
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
use crate::headless::{self, Repaint};
use crate::style;
use crate::theme;
use crate::tools::Tool;
use crate::ui_bench::alloc;
use crate::view::{self, Camera};
use crate::workbench::Workbench;
use eframe::egui::{self, epaint, vec2, Event, Modifiers, PointerButton, Pos2, Rect, Shape};
use geo_kernel::kernel::Value;

const SCREEN: (f32, f32) = (1400.0, 900.0);
const DT: f64 = 1.0 / 60.0;

/// The benchmark's grid cell: three free points, a free segment, a line, a
/// circle, a triangle with its three sides, a midpoint and an intersection.
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
            format!("{} = Intersect({}, {})", p("I"), p("l"), p("k")),
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
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut w = Window {
            ctx,
            app: App::new(Workbench::from_script(&script).unwrap(), None),
            time: 0.0,
            mid,
        };
        w.frames(3);
        w.fit(1.0);
        w.frames(3);
        w
    }

    fn step(&mut self, events: Vec<Event>) -> Counts {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(SCREEN.0, SCREEN.1));
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
        let mut cam = Camera::fit(view::content_bounds(self.app.wb.construction()), w, h);
        cam.zoom_at(zoom, w / 2.0, h / 2.0, w, h);
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
        (0..400)
            .map(|i| center + 3.0 * i as f32 * vec2((i as f32).cos(), (i as f32).sin()))
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
    out
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
    // - rows: none made outside a drag; in a drag only those of the objects
    //   it changed (checked in `states`).
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
        assert_eq!(
            (s.colour_scans, l.colour_scans),
            (0, 0),
            "{name}: colours scanned"
        );
        if *name != "drag_point" {
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
