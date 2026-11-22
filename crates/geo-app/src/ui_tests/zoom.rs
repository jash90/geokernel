//! The zoom tools, ported from the reference app's zoom tool tests
//! (GeoGebra, EUPL 1.2):
//! `shared/common-jre/src/test/java/org/geogebra/common/euclidian/tools/ZoomToolTest.java`
//! at commit 112919f. Where the owner keeps our own rule (view changes are
//! undo steps right after the gesture, `OwnerRules::view_changes_are_steps`) the
//! expectation says so; everything else follows the reference.

use super::*;
use crate::geom::ToEgui;
use crate::interact::ViewTransform;

fn press(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// One frame `dt` seconds after the last one, with `events`: the harness
/// clock only runs while it is asked to, so a drag crosses its gates in
/// the steps the test makes.
fn frame_later(h: &mut Harness<'_, App>, dt: f64, events: Vec<Event>) {
    let now = h.ctx.input(|i| i.time);
    h.input_mut().time = Some(now + dt);
    h.input_mut().events.extend(events);
    h.step();
}

/// The camera's scales, x then y.
fn scales(h: &Harness<'_, App>) -> (f64, f64) {
    let c = h.state().camera.unwrap();
    (c.xscale, c.yscale)
}

/// The world rectangle the drawing area shows.
fn bounds(h: &Harness<'_, App>) -> (f64, f64, f64, f64) {
    let (_, t) = h.state().view.unwrap();
    let v = t.view();
    (v.xmin, v.xmax, v.ymin, v.ymax)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

/// Where the world point `(x, y)` is drawn, in window points.
fn drawn(h: &Harness<'_, App>, x: f64, y: f64) -> Pos2 {
    let (r, t) = h.state().view.unwrap();
    let (sx, sy) = t.world_to_screen(x, y);
    r.min.egui() + egui::vec2(sx as f32, sy as f32)
}

/// `assertClickZoom`: a click of the tool multiplies both scales by
/// `factor` about the clicked point, which stays at its pixel.
fn assert_click_zoom(h: &mut Harness<'_, App>, tool: Tool, x: f64, y: f64, factor: f64) {
    h.state_mut().tool.set_tool(tool);
    let (sx, sy) = scales(h);
    let at = drawn(h, x, y);
    click(h, at);
    let (nx, ny) = scales(h);
    assert!(close(nx, sx * factor), "{nx} instead of {}", sx * factor);
    assert!(close(ny, sy * factor), "{ny} instead of {}", sy * factor);
    let now = drawn(h, x, y);
    assert!(
        (now - at).length() < 1e-4,
        "the clicked point moved from {at:?} to {now:?}"
    );
}

/// Was `zoomInClickShouldIncreaseScaleAroundClickedPoint`: the click zooms
/// by `MODE_ZOOM_FACTOR` (1.5) about the clicked point.
#[test]
fn a_zoom_in_click_increases_both_scales_about_the_click() {
    let mut h = harness("");
    assert_click_zoom(&mut h, Tool::ZoomIn, 2.0, -2.0, 1.5);
}

#[test]
fn a_zoom_out_click_decreases_both_scales_about_the_click() {
    let mut h = harness("");
    assert_click_zoom(&mut h, Tool::ZoomOut, 2.0, -2.0, 1.0 / 1.5);
}

/// `repeatedZoomInClicksShouldApplyFactorEachTime`.
#[test]
fn repeated_zoom_in_clicks_apply_the_factor_each_time() {
    let mut h = harness("");
    h.state_mut().tool.set_tool(Tool::ZoomIn);
    let (sx, sy) = scales(&h);
    let at = drawn(&h, 2.0, -2.0);
    click(&mut h, at);
    click(&mut h, at);
    let (nx, ny) = scales(&h);
    assert!(close(nx, sx * 1.5 * 1.5));
    assert!(close(ny, sy * 1.5 * 1.5));
}

/// A drag of the zoom in tool over a rectangle: the view becomes exactly
/// that rectangle (`processZoomRectangle`, which does not keep the aspect
/// ratio), and undo steps back to the view before it
/// (`zoomInSelectionRectangleShouldSetViewBoundsAndBeUndoable`).
#[test]
fn a_zoom_in_rectangle_sets_the_view_bounds_and_is_undoable() {
    let mut h = harness("");
    h.state_mut().tool.set_tool(Tool::ZoomIn);
    let was = bounds(&h);
    let from = drawn(&h, 1.0, -1.0);
    let to = drawn(&h, 5.0, -4.0);
    moved(&mut h, from);
    button(&mut h, from, true);
    // Beyond the 100 ms gate, in steps like a pointer moves.
    for i in 1..=4 {
        let at = from + (to - from) * (i as f32 / 4.0);
        frame_later(&mut h, 0.05, vec![Event::PointerMoved(at)]);
    }
    frame_later(&mut h, 0.1, vec![press(to, PointerButton::Primary, false)]);
    h.run();
    let got = bounds(&h);
    // The rectangle snaps to the view's ratio (`keepScreenRatio`): its
    // width stands (200 px) and its height grows to 200 px over the view's
    // ratio, from the corner the drag began at.
    let (rw, rh) = {
        let (r, _) = h.state().view.unwrap();
        (r.width() as f64, r.height() as f64)
    };
    let ymin = -1.0 - 200.0 / (rw / rh) / 50.0;
    for (a, b) in [(got.0, 1.0), (got.1, 5.0), (got.2, ymin), (got.3, -1.0)] {
        assert!(close(a, b), "bounds {got:?}, ymin {ymin}");
    }
    assert_eq!(h.state().wb.undo_depth().0, 1, "one step for the rectangle");
    h.state_mut().undo();
    h.run();
    let back = bounds(&h);
    for (a, b) in [
        (back.0, was.0),
        (back.1, was.1),
        (back.2, was.2),
        (back.3, was.3),
    ] {
        assert!(close(a, b), "bounds {back:?} instead of {was:?}");
    }
}

/// `narrowZoomInSelectionRectangleShouldNotChangeView`: under
/// `ZOOM_RECT_THRESHOLD` (30 px) on either side the rectangle zooms nothing.
#[test]
fn a_narrow_zoom_in_rectangle_changes_nothing() {
    let mut h = harness("");
    h.state_mut().tool.set_tool(Tool::ZoomIn);
    let was = (bounds(&h), scales(&h));
    let from = drawn(&h, 1.0, -1.0);
    let to = drawn(&h, 1.4, -4.0);
    moved(&mut h, from);
    button(&mut h, from, true);
    // A pointer moves in steps: the threshold is crossed by the second
    // move, which makes the release a drag's, not a click.
    frame_later(
        &mut h,
        0.2,
        vec![Event::PointerMoved(from + (to - from) * 0.5)],
    );
    frame_later(&mut h, 0.05, vec![Event::PointerMoved(to)]);
    frame_later(&mut h, 0.1, vec![press(to, PointerButton::Primary, false)]);
    h.run();
    assert_eq!(bounds(&h), was.0);
    assert_eq!(scales(&h), was.1);
}

/// `shortZoomInSelectionRectangleShouldNotChangeView`.
#[test]
fn a_short_zoom_in_rectangle_changes_nothing() {
    let mut h = harness("");
    h.state_mut().tool.set_tool(Tool::ZoomIn);
    let was = (bounds(&h), scales(&h));
    let from = drawn(&h, 1.0, -1.0);
    let to = drawn(&h, 5.0, -1.4);
    moved(&mut h, from);
    button(&mut h, from, true);
    // A pointer moves in steps: the threshold is crossed by the second
    // move, which makes the release a drag's, not a click.
    frame_later(
        &mut h,
        0.2,
        vec![Event::PointerMoved(from + (to - from) * 0.5)],
    );
    frame_later(&mut h, 0.05, vec![Event::PointerMoved(to)]);
    frame_later(&mut h, 0.1, vec![press(to, PointerButton::Primary, false)]);
    h.run();
    assert_eq!(bounds(&h), was.0);
    assert_eq!(scales(&h), was.1);
}

/// `zoomInDragStartingOnObjectShouldNotChangeView`: a drag that starts on
/// an object drags nothing and zooms nothing.
#[test]
fn a_zoom_in_drag_starting_on_an_object_changes_nothing() {
    let mut h = harness("A = (1, -1)\n");
    h.state_mut().tool.set_tool(Tool::ZoomIn);
    let was = (bounds(&h), scales(&h));
    let from = drawn(&h, 1.0, -1.0);
    let to = drawn(&h, 5.0, -4.0);
    moved(&mut h, from);
    button(&mut h, from, true);
    // A pointer moves in steps: the threshold is crossed by the second
    // move, which makes the release a drag's, not a click.
    frame_later(
        &mut h,
        0.2,
        vec![Event::PointerMoved(from + (to - from) * 0.5)],
    );
    frame_later(&mut h, 0.05, vec![Event::PointerMoved(to)]);
    frame_later(&mut h, 0.1, vec![press(to, PointerButton::Primary, false)]);
    h.run();
    assert_eq!(bounds(&h), was.0);
    assert_eq!(scales(&h), was.1);
    assert_eq!(point(&h, "A"), (1.0, -1.0));
}

/// `zoomOutDragShouldNotChangeView`: the zoom out tool has no rectangle.
#[test]
fn a_zoom_out_drag_changes_nothing() {
    let mut h = harness("");
    h.state_mut().tool.set_tool(Tool::ZoomOut);
    let was = (bounds(&h), scales(&h));
    let from = drawn(&h, 1.0, -1.0);
    let to = drawn(&h, 5.0, -4.0);
    moved(&mut h, from);
    button(&mut h, from, true);
    // A pointer moves in steps: the threshold is crossed by the second
    // move, which makes the release a drag's, not a click.
    frame_later(
        &mut h,
        0.2,
        vec![Event::PointerMoved(from + (to - from) * 0.5)],
    );
    frame_later(&mut h, 0.05, vec![Event::PointerMoved(to)]);
    frame_later(&mut h, 0.1, vec![press(to, PointerButton::Primary, false)]);
    h.run();
    assert_eq!(bounds(&h), was.0);
    assert_eq!(scales(&h), was.1);
}

/// Was `leavingZoomToolShouldMakeClicksUndoableAsOneAction`: the reference
/// stores the zoom clicks of a tool use at the moment the tool is left
/// (`endOfMode`), so any number of them is one undo step. The owner keeps
/// view changes as undo steps right after the gesture
/// (`OwnerRules::view_changes_are_steps`): each click is a step of its own.
#[test]
fn each_zoom_in_click_is_its_own_undo_step() {
    let mut h = harness("");
    h.state_mut().tool.set_tool(Tool::ZoomIn);
    let was = scales(&h);
    let at = drawn(&h, 2.0, -2.0);
    click(&mut h, at);
    click(&mut h, at);
    h.state_mut().select_tool(Tool::Move);
    h.run();
    assert_eq!(h.state().wb.undo_depth().0, 2);
    h.state_mut().undo();
    h.run();
    let (sx, _) = scales(&h);
    assert!(close(sx, was.0 * 1.5), "one click back: {sx}");
    h.state_mut().undo();
    h.run();
    let (sx, _) = scales(&h);
    assert!(close(sx, was.0), "two clicks back: {sx}");
}

/// The undo of a view step leaves the construction alone (the reference's
/// undo state carries the view settings, `AppState`; ours carries the
/// camera next to the journal).
#[test]
fn the_undo_of_a_view_step_keeps_the_objects() {
    let mut h = harness("A = (1, 1)\n");
    h.state_mut().tool.set_tool(Tool::ZoomIn);
    let at = drawn(&h, 2.0, -2.0);
    click(&mut h, at);
    assert_eq!(labels(&h), ["A"]);
    assert!(h.state_mut().undo());
    h.run();
    assert_eq!(labels(&h), ["A"]);
    assert_eq!(point(&h, "A"), (1.0, 1.0));
    assert_eq!(scales(&h), (50.0, 50.0));
    assert!(!h.state().wb.can_undo());
    assert!(h.state().wb.can_redo());
}

/// The transform the camera hands out carries both scales (spec D1).
#[test]
fn the_transform_carries_both_scales() {
    let h = harness("");
    let (r, t) = h.state().view.unwrap();
    assert!(close(t.xscale(), 50.0));
    assert!(close(t.yscale(), 50.0));
    let (w, hgt) = (r.width() as f64, r.height() as f64);
    // A camera with two different scales maps each axis by its own.
    let c = crate::view::Camera {
        cx: 0.0,
        cy: 0.0,
        xscale: 100.0,
        yscale: 50.0,
    };
    let t: ViewTransform = c.transform(w, hgt);
    let v = t.view();
    assert!(close(v.xmax - v.xmin, w / 100.0));
    assert!(close(v.ymax - v.ymin, hgt / 50.0));
}

/// A run of wheel zooms is one undo step right after it under the owner's
/// rule; under the reference's the wheel alone adds no step — the next
/// canvas press stores it, one step for every view change since
/// (`OwnerRules::view_changes_are_steps`, spec F3, the recorded
/// observation B1 counted none). Fails if the switch does nothing.
#[test]
fn a_wheel_zoom_steps_at_once_or_at_the_next_press() {
    for rules in [OwnerRules::OURS, OwnerRules::REFERENCE] {
        let what = format!("{rules:?}");
        let mut h = harness("");
        h.state_mut().set_rules(rules);
        let (sx, _) = scales(&h);
        let at = drawn(&h, 2.0, -2.0);
        let wheel = |h: &mut Harness<'_, App>, dy: f32| {
            h.input_mut().events.push(Event::PointerMoved(at));
            h.input_mut().events.push(Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, dy),
                modifiers: Modifiers::NONE,
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Wheel,
            });
            h.step();
        };
        // The frame of the run: nothing stored yet, either way.
        wheel(&mut h, 1.0);
        assert!(close(scales(&h).0, sx * 1.1), "{what}");
        assert_eq!(h.state().wb.undo_depth().0, 0, "{what}: the run is open");
        h.run();
        let want = if rules.view_changes_are_steps { 1 } else { 0 };
        assert_eq!(
            h.state().wb.undo_depth().0,
            want,
            "{what}: the run ended, its step is {}",
            if want == 1 { "stored" } else { "still waiting" }
        );
        // Another notch starts from what the first left: one step more for
        // the owner, still the one pending step for the reference.
        wheel(&mut h, 1.0);
        h.run();
        assert_eq!(
            h.state().wb.undo_depth().0,
            if rules.view_changes_are_steps { 2 } else { 0 },
            "{what}"
        );
        if !rules.view_changes_are_steps {
            // The next canvas press stores the one pending step and undo
            // takes the view back to where the wheel began.
            let at = drawn(&h, 2.0, -2.0);
            click(&mut h, at);
            assert_eq!(h.state().wb.undo_depth().0, 1, "{what}");
            assert!(h.state_mut().undo());
            h.run();
            assert!(close(scales(&h).0, sx), "{what}");
        }
    }
}

/// The wheel belongs to a text field that has the keyboard: it scrolls the
/// field and zooms nothing (`wrapMouseWheelMoved` begins with
/// `isTextfieldHasFocus`).
#[test]
fn the_wheel_zooms_nothing_while_a_text_field_has_the_focus() {
    let mut h = harness("A = (1, 1)\n");
    let was = scales(&h);
    // The input row takes the focus, as clicking it does.
    h.state_mut().focus_input = true;
    h.run();
    let at = drawn(&h, 2.0, -2.0);
    h.input_mut().events.push(Event::PointerMoved(at));
    h.input_mut().events.push(Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, 3.0),
        modifiers: Modifiers::NONE,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    });
    h.step();
    h.run();
    assert_eq!(scales(&h), was, "the wheel zoomed over the field");
}

#[test]
fn the_trackpad_pinch_zooms_only_without_text_field_focus() {
    let mut h = harness("A = (1, 1)\n");
    h.state_mut().focus_input = true;
    h.run();
    assert!(h.ctx.egui_wants_keyboard_input());

    let at = drawn(&h, 2.0, -2.0);
    let before = scales(&h);
    let pinch = |h: &mut Harness<'_, App>| {
        h.input_mut()
            .events
            .extend([Event::PointerMoved(at), Event::Zoom(1.25)]);
        h.step();
    };
    pinch(&mut h);
    assert_eq!(scales(&h), before, "the pinch zoomed while typing");

    h.ctx.memory_mut(|m| {
        if let Some(id) = m.focused() {
            m.surrender_focus(id);
        }
    });
    let (sx, sy) = scales(&h);
    pinch(&mut h);
    let (nx, ny) = scales(&h);
    assert!(close(nx, sx * 1.25) && close(ny, sy * 1.25));
}
