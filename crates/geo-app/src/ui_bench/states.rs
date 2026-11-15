//! The scripted states and actions. Each state starts from a settled
//! window with the move tool and leaves it that way.

use super::{adapter, button, key, moved, timed, Frame, Runner};
use crate::tools::Tool;
use eframe::egui::{vec2, Event, Key, Modifiers, MouseWheelUnit, Pos2, TouchPhase};
use geo_kernel::kernel::Value;
use std::collections::BTreeSet;

/// Repeats of each action per round.
const ACTION_REPEATS: usize = 3;

pub fn run_all(r: &mut Runner, n: usize) {
    idle(r, n, "idle", 1.0);
    idle(r, n, "idle_zoomed_4x", 4.0);
    hover(r, n);
    if adapter::HAS_KEYBOARD {
        keyboard(r, n);
    }
    algebra_scrolled(r, n);
    tool_preview(r, n);
    zoom(r, n);
    pan(r, n);
    drag(r, n);
    actions(r);
    helpers(r);
}

fn idle(r: &mut Runner, n: usize, state: &'static str, zoom: f64) {
    let mut notes = BTreeSet::new();
    if zoom != 1.0 {
        r.pin_camera(zoom);
    }
    r.settle_note(&mut notes);
    let frames = r.frames(n, |_, _| Vec::new(), |_, f| f.ok = true);
    r.record(state, frames, notes);
    if zoom != 1.0 {
        r.pin_camera(1.0);
        r.settle();
    }
}

fn circle(center: Pos2, radius: f32, i: usize) -> Pos2 {
    let a = i as f32 * 0.7;
    center + radius * vec2(a.cos(), a.sin())
}

fn hover(r: &mut Runner, n: usize) {
    let center = r.point(&r.scene.drag);
    let frames = r.frames(
        n,
        |_, i| vec![moved(circle(center, 6.0, i))],
        |r, f| f.ok = r.app.hovered.is_some(),
    );
    r.record("hover_move", frames, BTreeSet::new());
    r.step(vec![Event::PointerGone]);
    r.settle();
}

fn keyboard(r: &mut Runner, n: usize) {
    let mut notes = BTreeSet::new();
    adapter::open_keyboard(&mut r.app);
    for _ in 0..5 {
        r.step(Vec::new());
    }
    r.settle_note(&mut notes);
    let frames = r.frames(
        n,
        |_, _| Vec::new(),
        |r, f| f.ok = adapter::keyboard_open(&r.app),
    );
    r.record("keyboard_open", frames, notes);
    r.step(vec![key(Key::Escape, true)]);
    r.step(vec![key(Key::Escape, false)]);
    r.settle();
    assert!(
        !adapter::keyboard_open(&r.app),
        "the keyboard did not close"
    );
}

fn wheel(dy: f32) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: vec2(0.0, dy),
        phase: TouchPhase::Move,
        source: eframe::egui::MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    }
}

fn algebra_scrolled(r: &mut Runner, n: usize) {
    let mut notes = BTreeSet::new();
    let area = adapter::algebra(&r.app, r.screen);
    let rows = r.app.wb.rows().to_vec();
    let mid = rows.len() / 2;
    let above: f32 = rows[..mid].iter().map(adapter::row_height).sum();
    let offset = (above - area.height() / 2.0 + adapter::row_height(&rows[mid]) / 2.0).max(0.0);
    let at = area.center();
    r.step(vec![moved(at)]);
    r.step(vec![moved(at), wheel(-offset)]);
    r.settle_note(&mut notes);
    r.texts = Some(Vec::new());
    r.step(Vec::new());
    let texts = r.texts.take().unwrap_or_default();
    let visible = texts.iter().any(|t| *t == rows[mid].head);
    if !visible {
        notes.insert(format!("row {mid} ({}) not visible", rows[mid].head));
    }
    let frames = r.frames(n, |_, _| Vec::new(), |_, f| f.ok = visible);
    r.record("algebra_scrolled_middle", frames, notes);
    r.step(vec![moved(at), wheel(offset + 1000.0)]);
    r.settle();
    r.step(vec![Event::PointerGone]);
    r.settle();
}

fn press_release(r: &mut Runner, p: Pos2) -> Frame {
    r.step(vec![moved(p)]);
    r.step(vec![button(p, true)]);
    r.step(vec![button(p, false)])
}

fn back_to_move(r: &mut Runner) {
    r.step(vec![Event::PointerGone, key(Key::Escape, true)]);
    r.step(vec![key(Key::Escape, false)]);
    adapter::select_tool(&mut r.app, Tool::Move);
    r.settle();
}

fn tool_preview(r: &mut Runner, n: usize) {
    adapter::select_tool(&mut r.app, Tool::Segment);
    r.settle();
    let p = r.point(&r.scene.pick);
    press_release(r, p);
    let center = p + vec2(60.0, -40.0);
    let frames = r.frames(
        n,
        |_, i| vec![moved(circle(center, 8.0, i))],
        |r, f| f.ok = r.app.tool.picks().len() == 1,
    );
    r.record("tool_preview_segment", frames, BTreeSet::new());
    back_to_move(r);
}

fn zoom(r: &mut Runner, n: usize) {
    let at = r.canvas().center();
    let mut last = r.app.camera.map(|c| c.scale);
    let frames = r.frames(
        n,
        |_, i| {
            let f = if i % 2 == 0 { 1.03 } else { 1.0 / 1.03 };
            vec![moved(at), Event::Zoom(f)]
        },
        |r, f| {
            let now = r.app.camera.map(|c| c.scale);
            f.ok = now != last;
            last = now;
        },
    );
    r.record("zoom", frames, BTreeSet::new());
    r.step(vec![Event::PointerGone]);
    r.pin_camera(1.0);
    r.settle();
}

/// Presses at `from`, moves until egui reports a drag, then oscillates.
fn press_and_drag(
    r: &mut Runner,
    n: usize,
    from: Pos2,
    after: impl FnMut(&mut Runner, &mut Frame),
) -> Vec<Frame> {
    r.step(vec![moved(from)]);
    r.step(vec![button(from, true)]);
    let frames = r.frames(
        n,
        |_, i| {
            let dx = if i % 2 == 0 { 14.0 } else { 10.0 };
            vec![moved(from + vec2(dx, 4.0))]
        },
        after,
    );
    let end = from + vec2(10.0, 4.0);
    r.step(vec![button(end, false)]);
    r.step(vec![Event::PointerGone]);
    frames
}

fn pan(r: &mut Runner, n: usize) {
    let from = r.empty_spot(true);
    let frames = press_and_drag(r, n, from, |r, f| {
        f.ok = r.app.panning && r.app.drag.is_none();
    });
    r.record("pan", frames, BTreeSet::new());
    r.pin_camera(1.0);
    r.settle();
}

fn drag(r: &mut Runner, n: usize) {
    let label = r.scene.drag.clone();
    // The window drops the kernel's recompute count; a shadow construction
    // repeats every move to get it.
    let mut shadow = geo_kernel::script::load(&r.scene.script).expect("the scene loads");
    let sid = shadow.id(&label).expect("dragged point");
    let from = r.point(&label);
    let frames = press_and_drag(r, n, from, |r, f| {
        f.ok = r.app.drag.is_some();
        let c = r.app.wb.construction();
        let v = c.value(c.id(&label).expect("dragged point"));
        if let (Value::Point { x, y }, true) = (v, v != shadow.value(sid)) {
            f.recomputed = shadow.move_point(sid, x, y).unwrap_or(0) as u64;
        }
    });
    r.record("drag_point", frames, BTreeSet::new());
    r.settle();
}

fn actions(r: &mut Runner) {
    let mut after_add = Vec::new();
    let mut after_undo = Vec::new();
    let (a, b) = r.scene.midpoint.clone();
    for _ in 0..ACTION_REPEATS {
        let line = format!("Z = Midpoint({a}, {b})");
        let (ok, ns, al, by) = timed(|| r.app.wb.run_line(&line));
        ok.expect("the added line runs");
        r.sample("add (run_line)", ns, al, by);
        let mut f = r.step(Vec::new());
        f.ok = r.app.wb.construction().id("Z").is_some();
        after_add.push(f);
        r.settle();

        let (_, ns, al, by) = timed(|| r.app.wb.undo());
        r.sample("undo", ns, al, by);
        let mut f = r.step(Vec::new());
        f.ok = r.app.wb.construction().id("Z").is_none();
        after_undo.push(f);
        r.settle();

        let (_, ns, al, by) = timed(|| r.app.wb.redo());
        r.sample("redo", ns, al, by);
        r.settle();
        r.app.wb.undo();
        r.settle();

        let c = r.app.wb.construction();
        let id = c.id(&r.scene.delete).expect("object to delete");
        let (ok, ns, al, by) = timed(|| r.app.wb.delete(id));
        assert!(ok, "delete failed");
        r.sample("delete", ns, al, by);
        r.settle();
        r.app.wb.undo();
        r.settle();

        let (_, ns, al, by) = timed(|| {
            r.app.wb.begin_gesture();
            r.app.wb.end_gesture();
        });
        r.sample("snapshot (begin_gesture)", ns, al, by);

        // One step of the point tool through the window: the click frame.
        adapter::select_tool(&mut r.app, Tool::Point);
        r.settle();
        let spot = r.empty_spot(false);
        let before = r.app.wb.construction().len();
        let f = press_release(r, spot);
        assert_eq!(r.app.wb.construction().len(), before + 1, "no point made");
        r.sample("tool click frame (point)", f.ui_ns, f.ui_allocs, f.ui_bytes);
        back_to_move(r);
        r.app.wb.undo();
        r.settle();
    }
    r.record("after_add_object", after_add, BTreeSet::new());
    r.record("after_undo", after_undo, BTreeSet::new());
}

/// Per-object helpers timed alone, over the whole construction.
fn helpers(r: &mut Runner) {
    let reps = if r.app.wb.construction().len() > 5_000 {
        3
    } else {
        20
    };
    let canvas = r.canvas();
    let t = r.transform();
    for _ in 0..reps {
        let c = r.app.wb.construction();
        let mid = (canvas.width() as f64 / 2.0, canvas.height() as f64 / 2.0);
        let mut done = Vec::new();
        let (_, ns, al, by) = timed(|| crate::hit::pick(c, &t, mid, 11.0, 7.0, |_| true));
        done.push(("hit::pick (one pointer event)", ns, al, by));
        let (_, ns, al, by) = timed(|| crate::view::content_bounds(c));
        done.push(("view::content_bounds", ns, al, by));
        for (name, work) in adapter::per_object_work() {
            let (_, ns, al, by) = timed(|| work(c));
            done.push((name, ns, al, by));
        }
        // Force the algebra rows to be rebuilt, as every drag frame does.
        let id = c.id(&r.scene.drag).expect("free point");
        if let Value::Point { x, y } = c.value(id) {
            r.app.wb.move_point(id, x + 1e-9, y);
        }
        let (_, ns, al, by) = timed(|| r.app.wb.rows().len());
        done.push(("Workbench::rows rebuild", ns, al, by));
        for (name, ns, al, by) in done {
            r.sample(name, ns, al, by);
        }
    }
}
