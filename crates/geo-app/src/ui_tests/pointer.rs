//! Presses, clicks and drags in the graphics view while the on-screen
//! keyboard is open: the press closes the keyboard and the area grows in
//! that very frame, which must not move the drawing or the hit.

use super::*;
use crate::tools::Outcome;

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
    rect.min + egui::vec2(sx as f32, sy as f32)
}

fn point(h: &Harness<'_, App>, label: &str) -> (f64, f64) {
    let c = h.state().wb.construction();
    let Value::Point { x, y } = c.value(c.id(label).unwrap()) else {
        panic!("{label} is not a point");
    };
    (x, y)
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
fn a_placed_point_survives_a_failed_final_step() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().select_tool(Tool::Segment);
    h.state_mut().apply_outcome(Outcome::Place {
        line: "B = (1, 1)".into(),
        then: Box::new(Outcome::Run(vec!["f = Segment(B, Nope)".into()])),
    });
    h.run();
    assert_eq!(labels(&h), ["A", "B"]);
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
