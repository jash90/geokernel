//! Pointer behaviour of the graphics view that comes from egui's own click
//! and drag rules: a press held still for long is not a click, a press that
//! leaves the area starts a drag at once, and a drag with another button
//! pans. These pin the behaviour of a mouse on the desktop.

use super::*;

/// Runs one frame at `time` seconds with `events`.
fn frame_at(h: &mut Harness<'_, App>, time: f64, events: Vec<Event>) {
    h.input_mut().time = Some(time);
    h.input_mut().events.extend(events);
    h.step();
}

fn press(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn a_long_press_is_not_a_tool_click() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().tool.set_tool(Tool::Point);
    let at = world(1.0, 1.0);
    frame_at(&mut h, 10.0, vec![Event::PointerMoved(at)]);
    frame_at(&mut h, 10.1, vec![press(at, PointerButton::Primary, true)]);
    frame_at(&mut h, 11.0, vec![]);
    frame_at(&mut h, 11.1, vec![press(at, PointerButton::Primary, false)]);
    h.run();
    assert_eq!(labels(&h), ["A"], "no point is placed");
}

#[test]
fn after_a_long_press_the_view_pans_by_the_whole_movement() {
    let mut h = harness("A = (0, 0)\n");
    let at = world(-4.0, -3.0);
    let cx0 = h.state().camera.unwrap().cx;
    frame_at(&mut h, 10.0, vec![Event::PointerMoved(at)]);
    frame_at(&mut h, 10.1, vec![press(at, PointerButton::Primary, true)]);
    // Still for longer than a click may take: the drag starts in place.
    frame_at(&mut h, 11.0, vec![]);
    let to = at + egui::vec2(3.0, 0.0);
    frame_at(&mut h, 11.1, vec![Event::PointerMoved(to)]);
    frame_at(&mut h, 11.2, vec![press(to, PointerButton::Primary, false)]);
    h.run();
    let cam = h.state().camera.unwrap();
    assert!((cam.cx - cx0 + 3.0 / 50.0).abs() < 1e-12, "{}", cam.cx);
    assert_eq!(labels(&h), ["A"]);
}

#[test]
fn leaving_the_area_before_the_click_distance_starts_a_pan() {
    // From half a point inside the right edge to five and a half outside
    // (beyond egui's interaction radius): six points, not yet a drag by
    // distance, but the pointer is no longer over the area.
    let mut h = harness("A = (0, 0)\n");
    let from = at(SIZE.0 - 0.5, 500.0);
    let to = at(SIZE.0 + 5.5, 500.0);
    let cx0 = h.state().camera.unwrap().cx;
    moved(&mut h, from);
    button(&mut h, from, true);
    moved(&mut h, to);
    button(&mut h, to, false);
    h.run();
    let cam = h.state().camera.unwrap();
    assert!(
        (cam.cx - cx0 + 6.0 / 50.0).abs() < 1e-12,
        "{}",
        cam.cx - cx0
    );
}

#[test]
fn a_drag_with_the_secondary_button_pans_and_leaves_points_alone() {
    let mut h = harness("A = (1, 1)\n");
    let from = world(1.0, 1.0);
    let cx0 = h.state().camera.unwrap().cx;
    moved(&mut h, from);
    h.input_mut()
        .events
        .push(press(from, PointerButton::Secondary, true));
    h.step();
    for i in 1..=10 {
        moved(&mut h, from + egui::vec2(3.0 * i as f32, 0.0));
    }
    h.input_mut().events.push(press(
        from + egui::vec2(30.0, 0.0),
        PointerButton::Secondary,
        false,
    ));
    h.step();
    h.run();
    let app = h.state();
    let a = app.wb.construction().id("A").unwrap();
    assert_eq!(
        app.wb.construction().value(a),
        Value::Point { x: 1.0, y: 1.0 }
    );
    // The drag starts once the pointer is more than 6 points from the press
    // (at 9); from then on every frame's movement pans: 3 + 7 * 3.
    let cam = app.camera.unwrap();
    assert!((cam.cx - cx0 + 24.0 / 50.0).abs() < 1e-12, "{}", cam.cx);
}

#[test]
fn a_secondary_click_does_nothing() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().tool.set_tool(Tool::Point);
    let at = world(1.0, 1.0);
    moved(&mut h, at);
    h.input_mut()
        .events
        .push(press(at, PointerButton::Secondary, true));
    h.step();
    h.input_mut()
        .events
        .push(press(at, PointerButton::Secondary, false));
    h.step();
    h.run();
    assert_eq!(labels(&h), ["A"]);
}

#[test]
fn a_press_and_a_far_release_in_the_next_frame_do_nothing() {
    // egui decides neither a click (the pointer moved too far) nor a drag
    // (the button is already up) for such a flick.
    let mut h = harness("A = (1, 1)\n");
    h.state_mut().tool.set_tool(Tool::Point);
    let from = world(-3.0, -3.0);
    let cx0 = h.state().camera.unwrap().cx;
    moved(&mut h, from);
    button(&mut h, from, true);
    let to = from + egui::vec2(40.0, 0.0);
    h.input_mut().events.push(Event::PointerMoved(to));
    h.input_mut()
        .events
        .push(press(to, PointerButton::Primary, false));
    h.step();
    h.run();
    assert_eq!(labels(&h), ["A"]);
    assert_eq!(h.state().camera.unwrap().cx, cx0);
}
