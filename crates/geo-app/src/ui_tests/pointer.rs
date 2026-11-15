//! Presses, clicks and drags in the graphics view while the on-screen
//! keyboard is open: the press closes the keyboard and the area grows in
//! that very frame, which must not move the drawing or the hit.

use super::*;
use crate::geom::ToEgui;

/// Opens the keyboard the way a user does: a click in the input row, which
/// starts `top` pixels below the toolbar.
fn open_keyboard(h: &mut Harness<'_, App>, top: f32) {
    click(h, row_body(top));
    assert!(h.state().keyboard.open, "the input row opens the keyboard");
}

/// The world point at the top left corner of the drawing area.
fn top_left(h: &Harness<'_, App>) -> (f64, f64) {
    h.state().view.unwrap().1.screen_to_world(0.0, 0.0)
}

/// Where the world point `(x, y)` is drawn now (harness coordinates).
fn on_screen(h: &Harness<'_, App>, x: f64, y: f64) -> Pos2 {
    let (rect, t) = h.state().view.unwrap();
    let (sx, sy) = t.world_to_screen(x, y);
    rect.min.egui() + egui::vec2(sx as f32, sy as f32)
}

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

#[test]
fn opening_and_closing_the_keyboard_keeps_the_drawing_in_place() {
    let mut h = harness("A = (1, 1)\n");
    let corner = top_left(&h);
    open_keyboard(&mut h, ROW);
    assert!(close(top_left(&h), corner), "opening moved the drawing");
    h.state_mut().keyboard.open = false;
    h.run();
    assert!(close(top_left(&h), corner), "closing moved the drawing");
}

#[test]
fn a_drag_with_the_keyboard_open_moves_the_point_not_the_view() {
    let mut h = harness("A = (1, 1)\n");
    open_keyboard(&mut h, ROW);
    let corner = top_left(&h);
    let (from, to) = (on_screen(&h, 1.0, 1.0), on_screen(&h, 3.0, 2.0));
    drag(&mut h, from, to);
    assert!(!h.state().keyboard.open, "the press put the keyboard away");
    assert!(close(top_left(&h), corner), "the view panned");
    assert!(close(point(&h, "A"), (3.0, 2.0)), "{:?}", point(&h, "A"));
}

#[test]
fn a_tool_click_with_the_keyboard_open_hits_what_is_under_the_pointer() {
    let mut h = harness("A = (1, 1)\n");
    h.state_mut().select_tool(Tool::Segment);
    open_keyboard(&mut h, ROW);
    // On the existing point: picked, no new point on top of it.
    let at = on_screen(&h, 1.0, 1.0);
    click(&mut h, at);
    assert_eq!(labels(&h), ["A"]);
    assert_eq!(h.state().tool.picks().len(), 1);
    // On empty space: the new point is where the pointer was.
    open_keyboard(&mut h, ROW);
    let at = on_screen(&h, -2.0, 3.0);
    click(&mut h, at);
    assert_eq!(labels(&h), ["A", "B", "f"]);
    assert!(close(point(&h, "B"), (-2.0, 3.0)), "{:?}", point(&h, "B"));
}

#[test]
fn a_press_on_a_dependent_point_neither_moves_it_nor_pans() {
    let mut h = harness("A = (0, 0)\nB = (2, 0)\nM = Midpoint(A, B)\n");
    open_keyboard(&mut h, 2.0 * ROW + ROW_DEPENDENT);
    let corner = top_left(&h);
    let (from, to) = (on_screen(&h, 1.0, 0.0), on_screen(&h, 3.0, 2.0));
    drag(&mut h, from, to);
    assert!(close(top_left(&h), corner), "the view panned");
    assert!(close(point(&h, "M"), (1.0, 0.0)));
    assert!(!h.state().wb.can_undo());
    // With a construction tool a press on a free point does not pan either.
    h.state_mut().select_tool(Tool::Line);
    let (from, to) = (on_screen(&h, 0.0, 0.0), on_screen(&h, 2.0, 3.0));
    drag(&mut h, from, to);
    assert!(close(top_left(&h), corner), "the view panned");
    assert!(close(point(&h, "A"), (0.0, 0.0)));
    // The move-view tool pans even from a point.
    h.state_mut().select_tool(Tool::MoveView);
    let (from, to) = (on_screen(&h, 0.0, 0.0), on_screen(&h, 1.0, 0.0));
    drag(&mut h, from, to);
    assert!(close(point(&h, "A"), (0.0, 0.0)));
    assert!(
        !close(top_left(&h), corner),
        "the move-view tool did not pan"
    );
    let corner = top_left(&h);
    h.state_mut().select_tool(Tool::Line);
    // Away from points a drag still pans.
    let (from, to) = (on_screen(&h, -3.0, 3.0), on_screen(&h, -1.0, 3.0));
    drag(&mut h, from, to);
    assert!(!close(top_left(&h), corner));
}

#[test]
fn placed_points_survive_a_failed_final_step() {
    // The kernel refuses a regular polygon of a million vertices.
    let mut h = harness("A = (-3, -3)\n");
    h.state_mut().select_tool(Tool::RegularPolygon);
    click(&mut h, world(0.0, 2.0));
    click(&mut h, world(1.0, 2.0));
    assert!(h.state().tool.asking().is_some());
    h.state_mut().number_text = "1000000".into();
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(labels(&h), ["A", "B", "C"], "the points stay, nothing else");
    assert!(h.state().tool_error.is_some());
    assert!(h.state().tool.picks().is_empty());
}

#[test]
fn a_second_click_on_a_just_placed_point_hits_it_and_makes_no_second_point() {
    let mut h = harness("A = (-3, -3)\n");
    h.state_mut().select_tool(Tool::Circle);
    click(&mut h, world(1.0, 1.0));
    click(&mut h, world(1.0, 1.0));
    assert_eq!(labels(&h), ["A", "B"], "no zero-radius circle");
    assert_eq!(h.state().tool.picks().len(), 1);
    click(&mut h, world(3.0, 1.0));
    assert_eq!(labels(&h), ["A", "B", "C", "c"]);
}

/// A press and its release in one frame, as a touch tap or a frame hitch
/// delivers them (egui then keeps no press origin).
fn tap(h: &mut Harness<'_, App>, pos: Pos2) {
    for event in [
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ] {
        h.input_mut().events.push(event);
    }
    h.run();
}

#[test]
fn a_one_frame_tap_after_a_zoom_button_hits_the_zoomed_drawing() {
    let mut h = harness("A = (2, 1)\n");
    h.state_mut().select_tool(Tool::Segment);
    h.get_by_label("Powiększ").click();
    h.run();
    // A is drawn somewhere else now; tap it there.
    let at = on_screen(&h, 2.0, 1.0);
    assert!((at - world(2.0, 1.0)).length() > 20.0, "the zoom moved A");
    tap(&mut h, at);
    assert_eq!(labels(&h), ["A"], "the tap hit A, no new point");
    assert_eq!(h.state().tool.picks().len(), 1);
}

#[test]
fn a_tap_is_hit_tested_with_the_view_on_screen_not_one_changed_in_its_frame() {
    // The standard view comes in the same frame as the tap: the tap is
    // meant for what was on screen when it happened.
    let mut h = harness("A = (2, 1)\n");
    h.state_mut().camera.as_mut().unwrap().cx = 3.0;
    h.run();
    h.state_mut().select_tool(Tool::Segment);
    let at = on_screen(&h, 2.0, 1.0);
    h.state_mut().view_request = Some(crate::app::ViewRequest::Standard);
    tap(&mut h, at);
    assert_eq!(labels(&h), ["A"]);
    assert_eq!(h.state().tool.picks().len(), 1, "A was picked");
}

#[test]
fn the_notice_card_takes_clicks_on_its_text_and_drags_from_its_button() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().select_tool(Tool::Point);
    h.state_mut().wb.notice = Some("Pominięte obiekty: 1".into());
    h.run();
    let text = h.get_by_label("Pominięte obiekty: 1").rect().center();
    let text = Pos2::new(text.x - 30.0, text.y);
    click(&mut h, text);
    assert_eq!(labels(&h), ["A"], "a click on the text made a point");
    let corner = top_left(&h);
    let button = h.get_by_label("Zamknij komunikat").rect().center();
    drag(&mut h, button, button + egui::vec2(-120.0, -80.0));
    assert!(close(top_left(&h), corner), "a drag from the button panned");
}

#[test]
fn a_notice_shows_over_the_drawing_and_its_close_button_takes_the_click() {
    let long = "Pominięte obiekty: 3 (jądro ich nie obsługuje), zależne od nich: 2, \
                wartości różne od zapisanych w pliku: 1, wczytane linie: 40. \
                Uwagi: a long note that has to wrap over more than one line";
    for tool in [Tool::Move, Tool::Point] {
        let mut h = harness("A = (0, 0)\n");
        h.state_mut().select_tool(tool);
        h.state_mut().wb.notice = Some(long.into());
        h.run();
        let _ = h.get_by_label(long);
        h.get_by_label("Zamknij komunikat").click();
        h.run();
        assert!(h.state().wb.notice.is_none(), "{tool:?}");
        assert_eq!(labels(&h), ["A"], "{tool:?}: the click reached the drawing");
    }
}

/// The drawing area as last painted.
fn area(h: &Harness<'_, App>) -> egui::Rect {
    h.state().view.unwrap().0.egui()
}

/// The centre of the round zoom-in button over the drawing.
fn zoom_in_button(h: &Harness<'_, App>) -> Pos2 {
    let r = area(h);
    Pos2::new(r.right() - 35.0, r.bottom() - 33.0 - 2.0 * 44.0)
}

/// Puts a free point `label` under the window position `pos`.
fn point_under(h: &mut Harness<'_, App>, label: &str, pos: Pos2) {
    let (r, t) = h.state().view.unwrap();
    let local = pos - r.min.egui();
    let (x, y) = t.screen_to_world(local.x as f64, local.y as f64);
    h.state_mut()
        .wb
        .run_line(&format!("{label} = ({x}, {y})"))
        .unwrap();
    h.run();
}

#[test]
fn a_drag_from_a_view_button_neither_pans_nor_takes_the_point_under_it() {
    let mut h = harness("");
    let button = zoom_in_button(&h);
    point_under(&mut h, "A", button);
    let (a, corner) = (point(&h, "A"), top_left(&h));
    let scale = h.state().camera.unwrap().scale;
    drag(&mut h, button, button + egui::vec2(-200.0, -120.0));
    assert!(close(top_left(&h), corner), "the drag panned the view");
    assert_eq!(
        point(&h, "A"),
        a,
        "the drag took the point under the button"
    );
    assert_eq!(h.state().camera.unwrap().scale, scale, "it was a click");
}

#[test]
fn a_press_just_outside_a_view_button_is_the_drawings() {
    let mut h = harness("");
    let beside = zoom_in_button(&h) + egui::vec2(-19.0 - 12.0, 0.0);
    let corner = top_left(&h);
    drag(&mut h, beside, beside + egui::vec2(-200.0, -120.0));
    assert!(!close(top_left(&h), corner), "the drag did not pan");
    h.state_mut().select_tool(Tool::Point);
    h.run();
    tap(&mut h, beside);
    assert_eq!(labels(&h), ["A"], "the tap placed no point");
}

#[test]
fn a_tap_on_the_tool_hint_places_no_point_under_it() {
    let mut h = harness("");
    h.state_mut().select_tool(Tool::Point);
    h.run();
    // The hint text: the second line of the card at the bottom left.
    let r = area(&h);
    let card_top = r.bottom() - 16.0 - 54.0;
    let hint = Pos2::new(r.left() + 16.0 + 16.0 + 20.0, card_top + 29.0 + 8.0);
    tap(&mut h, hint);
    click(&mut h, hint + egui::vec2(10.0, 0.0));
    assert!(labels(&h).is_empty(), "a tap on the hint placed a point");
    // Just above the card the drawing takes the tap.
    tap(&mut h, Pos2::new(hint.x, card_top - 12.0));
    assert_eq!(labels(&h), ["A"]);
}
