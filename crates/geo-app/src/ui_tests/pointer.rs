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

/// The world point at the centre of the drawing area: what a resize keeps
/// (`keepCenter`, spec D11) and a pan moves.
fn centre(h: &Harness<'_, App>) -> (f64, f64) {
    let (r, t) = h.state().view.unwrap();
    let (w, h) = ((r.max.x - r.min.x) as f64, (r.max.y - r.min.y) as f64);
    t.screen_to_world(0.5 * w, 0.5 * h)
}

/// The world point at the top left corner of the drawing area: it stands
/// still while a drag moves an object and not the view.
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
    // Opening or closing the keyboard keeps the world point at the
    // centre (`keepCenter`, spec D11): the drawing stays under the
    // viewer.
    let middle = centre(&h);
    open_keyboard(&mut h, ROW);
    assert!(close(centre(&h), middle), "opening moved the drawing");
    h.state_mut().keyboard.open = false;
    h.run();
    assert!(close(centre(&h), middle), "closing moved the drawing");
}

#[test]
fn a_drag_with_the_keyboard_open_moves_the_point_not_the_view() {
    let mut h = harness("A = (1, 1)\n");
    open_keyboard(&mut h, ROW);
    let middle = centre(&h);
    let (from, to) = (on_screen(&h, 1.0, 1.0), on_screen(&h, 3.0, 2.0));
    drag(&mut h, from, to);
    assert!(!h.state().keyboard.open, "the press put the keyboard away");
    assert!(close(centre(&h), middle), "the view panned");
    // The press closed the keyboard, so the area grew mid-drag; the point
    // stays under the finger it ended at.
    let (r, t) = h.state().view.unwrap();
    let (sx, sy) = t.world_to_screen(point(&h, "A").0, point(&h, "A").1);
    let at = r.min.egui() + egui::vec2(sx as f32, sy as f32);
    assert!(
        (at - to).length() < 1e-3,
        "A is at {at:?}, the finger at {to:?}"
    );
}

fn text_presentation(h: &Harness<'_, App>) -> geo_kernel::kernel::text::TextPresentation {
    let c = h.state().wb.construction();
    c.text_presentation(c.id("t").unwrap()).unwrap()
}

#[test]
fn text_canvas_picks_and_drags_screen_and_world_text_with_undo() {
    use geo_kernel::kernel::text::TextPosition;

    for (placement, before, after) in [
        (
            "screen, 150, 180",
            TextPosition::Screen { x: 150.0, y: 180.0 },
            TextPosition::Screen { x: 210.0, y: 145.0 },
        ),
        (
            "world, 1, 1",
            TextPosition::World { x: 1.0, y: 1.0 },
            TextPosition::World { x: 2.2, y: 1.7 },
        ),
    ] {
        let mut h = harness(&format!(
            "t = \"Canvas text\"\n$textProperties(t, -, -, -, -, -, {placement})\n"
        ));
        h.state_mut().decor.grid = false;
        h.state_mut().decor.axes = false;
        let at = match before {
            TextPosition::Screen { x, y } => {
                h.state().view.unwrap().0.min.egui() + egui::vec2(x as f32, y as f32)
            }
            TextPosition::World { x, y } => on_screen(&h, x, y),
            _ => unreachable!(),
        };
        let id = h.state().wb.construction().id("t").unwrap();
        moved(&mut h, at + egui::vec2(20.0, 10.0));
        assert_eq!(h.state().hovered, None, "outside the text rectangle");
        let from = at + egui::vec2(20.0, -6.0);
        moved(&mut h, from);
        assert_eq!(h.state().hovered, Some(id));
        click(&mut h, from);
        assert_eq!(h.state().wb.selected, Some(id));
        assert!(!h.state().wb.can_undo());
        let corner = top_left(&h);
        drag(&mut h, from, from + egui::vec2(60.0, -35.0));
        match (text_presentation(&h).position, after) {
            (
                TextPosition::World { x, y },
                TextPosition::World {
                    x: want_x,
                    y: want_y,
                },
            ) => {
                assert!(close((x, y), (want_x, want_y)), "{x}, {y}");
            }
            (actual, expected) => assert_eq!(actual, expected),
        }
        assert!(close(top_left(&h), corner));
        assert_eq!(h.state().wb.undo_depth(), (1, 0));
        assert!(h.state_mut().undo());
        assert_eq!(text_presentation(&h).position, before);
        assert!(h.state_mut().redo());
        assert_eq!(h.state().wb.undo_depth(), (1, 0));
    }
}

#[test]
fn text_canvas_keeps_point_anchors_when_dragged() {
    use geo_kernel::kernel::text::TextPosition;

    for (script, offset) in [
        (
            "A = (1, 1)\nt = \"Canvas text\"\n$textProperties(t, -, -, -, -, -, point, A)\n",
            [60, -35],
        ),
        ("A = (1, 1)\nt = Text(\"Canvas text\", A)\n", [0, 0]),
        (
            "A = (150, 180)\nt = \"Canvas text\"\n$textProperties(t, -, -, -, -, -, screenPoint, A)\n",
            [0, 0],
        ),
    ] {
        let mut h = harness(script);
        let before = text_presentation(&h);
        let at = if matches!(before.position, TextPosition::ScreenPoint(_)) {
            h.state().view.unwrap().0.min.egui() + egui::vec2(150.0, 180.0)
        } else {
            on_screen(&h, 1.0, 1.0)
        };
        let anchor = point(&h, "A");
        let corner = top_left(&h);
        let from = at + egui::vec2(24.0, -6.0);
        drag(&mut h, from, from + egui::vec2(60.0, -35.0));
        let c = h.state().wb.construction();
        assert_eq!(h.state().wb.selected, c.id("t"));
        assert!(close(top_left(&h), corner));
        assert_eq!(point(&h, "A"), anchor);
        assert_eq!(text_presentation(&h).position, before.position);
        assert_eq!(text_presentation(&h).offset, offset);
        assert_eq!(h.state().wb.undo_depth(), (usize::from(offset != [0, 0]), 0));

        if offset != [0, 0] {
            assert!(h.state_mut().undo());
            assert_eq!(text_presentation(&h).offset, [0, 0]);
            assert!(h.state_mut().redo());
            assert_eq!(text_presentation(&h).offset, offset);
        }
        let id = h.state().wb.construction().id("A").unwrap();
        assert!(h.state_mut().wb.move_point(id, anchor.0 + 2.0, anchor.1 - 1.0));
        h.run();
        let shift = if matches!(before.position, TextPosition::ScreenPoint(_)) {
            egui::vec2(2.0, -1.0)
        } else {
            egui::vec2(100.0, 50.0)
        };
        moved(&mut h, from + shift + egui::vec2(offset[0] as f32, offset[1] as f32));
        assert_eq!(h.state().hovered, h.state().wb.construction().id("t"));
    }
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
    let corner = centre(&h);
    let (from, to) = (on_screen(&h, 1.0, 0.0), on_screen(&h, 3.0, 2.0));
    drag(&mut h, from, to);
    assert!(close(centre(&h), corner), "the view panned");
    assert!(close(point(&h, "M"), (1.0, 0.0)));
    assert!(!h.state().wb.can_undo());
    // With a construction tool a press on a free point does not pan either.
    h.state_mut().select_tool(Tool::Line);
    let (from, to) = (on_screen(&h, 0.0, 0.0), on_screen(&h, 2.0, 3.0));
    drag(&mut h, from, to);
    assert!(close(centre(&h), corner), "the view panned");
    assert!(close(point(&h, "A"), (0.0, 0.0)));
    // The press picked A and the release made the line's second point.
    assert_eq!(labels(&h), ["A", "B", "M", "C", "f"]);
    // The move-view tool pans even from a point: the drag starts on C
    // (2, 3), a point off both axes — a press on an axis rescales that
    // axis instead (`scaleXAxis`, spec D10) — and neither moves C nor
    // makes anything.
    h.state_mut().select_tool(Tool::MoveView);
    let (from, to) = (on_screen(&h, 2.0, 3.0), on_screen(&h, 3.0, 3.5));
    drag(&mut h, from, to);
    assert!(close(point(&h, "C"), (2.0, 3.0)), "the point moved");
    assert_eq!(labels(&h), ["A", "B", "M", "C", "f"]);
    assert!(!close(centre(&h), corner), "the move-view tool did not pan");
    let corner = centre(&h);
    // Away from objects the line tool's drag does not pan either: the press
    // places the first point, the release the second, and the line is made
    // (spec E1, `createNewPoint` on press, `wrapMouseReleased`).
    h.state_mut().select_tool(Tool::Line);
    let (from, to) = (on_screen(&h, -3.0, 3.0), on_screen(&h, -1.0, 3.0));
    drag(&mut h, from, to);
    assert!(close(centre(&h), corner), "the view panned");
    assert_eq!(labels(&h), ["A", "B", "M", "C", "f", "D", "E", "g"]);
    assert!(close(point(&h, "D"), (-3.0, 3.0)), "{:?}", point(&h, "D"));
    assert!(close(point(&h, "E"), (-1.0, 3.0)), "{:?}", point(&h, "E"));
    assert!(h.state().tool.picks().is_empty());
    // A construction tool whose press there picks nothing still pans, and
    // makes nothing.
    h.state_mut().select_tool(Tool::Intersect);
    let (from, to) = (on_screen(&h, -3.0, -1.0), on_screen(&h, -1.0, -1.0));
    drag(&mut h, from, to);
    assert!(!close(centre(&h), corner), "the drag did not pan");
    assert_eq!(labels(&h), ["A", "B", "M", "C", "f", "D", "E", "g"]);
    assert!(h.state().tool.picks().is_empty());
}

#[test]
fn a_failed_final_step_takes_the_placed_points_away() {
    // The kernel refuses a regular polygon of a million vertices. The tool
    // use ends unfinished, so its pending points go with it (spec E5/F1:
    // they belong to the use, not to steps of their own).
    let mut h = harness("A = (-3, -3)\n");
    h.state_mut().select_tool(Tool::RegularPolygon);
    click(&mut h, world(0.0, 2.0));
    click(&mut h, world(1.0, 2.0));
    assert!(h.state().tool.asking().is_some());
    h.state_mut().number_text = "1000000".into();
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(labels(&h), ["A"], "nothing of the use stays");
    assert!(h.state().tool_error.is_some());
    assert!(h.state().tool.picks().is_empty());
    assert_eq!(h.state().wb.undo_depth(), (0, 0));
}

#[test]
fn a_second_click_deselects_a_pending_point_and_keeps_it_in_the_tool_use() {
    let mut h = harness("A = (-3, -3)\n");
    h.state_mut().select_tool(Tool::Circle);
    click(&mut h, world(1.0, 1.0));
    click(&mut h, world(1.0, 1.0));
    assert_eq!(labels(&h), ["A", "B"], "no zero-radius circle");
    assert!(h.state().tool.picks().is_empty());
    assert_eq!(h.state().wb.undo_depth(), (0, 0));
    click(&mut h, world(3.0, 1.0));
    assert_eq!(labels(&h), ["A", "B", "C"]);
    click(&mut h, world(1.0, 1.0));
    assert_eq!(labels(&h), ["A", "B", "C", "c"]);
    assert_eq!(h.state().wb.definition("c"), Some("Circle(C, B)"));
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    undo(&mut h);
    assert_eq!(labels(&h), ["A"]);
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
    let corner = centre(&h);
    let button = h.get_by_label("Zamknij komunikat").rect().center();
    drag(&mut h, button, button + egui::vec2(-120.0, -80.0));
    assert!(close(centre(&h), corner), "a drag from the button panned");
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
    let (a, corner) = (point(&h, "A"), centre(&h));
    let scale = h.state().camera.unwrap().xscale;
    drag(&mut h, button, button + egui::vec2(-200.0, -120.0));
    assert!(close(centre(&h), corner), "the drag panned the view");
    assert_eq!(
        point(&h, "A"),
        a,
        "the drag took the point under the button"
    );
    assert_eq!(h.state().camera.unwrap().xscale, scale, "it was a click");
}

#[test]
fn a_press_just_outside_a_view_button_is_the_drawings() {
    let mut h = harness("");
    let beside = zoom_in_button(&h) + egui::vec2(-19.0 - 12.0, 0.0);
    let corner = centre(&h);
    drag(&mut h, beside, beside + egui::vec2(-200.0, -120.0));
    assert!(!close(centre(&h), corner), "the drag did not pan");
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

#[test]
fn formula_text_and_functions_paint_on_their_normal_surfaces() {
    use eframe::egui::Shape;
    use geo_kernel::script::text::literal;

    let source = format!(
        "f(x) = x^2 / 7\nt = {}\n$textProperties(t, true, -, -, -, -, screen, 60, 65)\n",
        literal(r"\begin{bmatrix}a&b\\c&d\\e&f\end{bmatrix}")
    );
    let h = harness(&source);
    let output = h.output();
    let canvas = h.state().view.unwrap().0;
    let (mut matrix_fences, mut fraction) = (0, false);
    for clipped in &output.shapes {
        match &clipped.shape {
            Shape::Text(text) if clipped.clip_rect.left() >= canvas.min.x => {
                if matches!(text.galley.text(), "[" | "]") {
                    matrix_fences += 1;
                    assert!(text.galley.mesh_bounds.height() > 30.0);
                }
                assert!(!text.galley.text().contains(r"\begin{bmatrix}"));
            }
            Shape::Text(text) if clipped.clip_rect.right() < canvas.min.x => {
                assert!(!text.galley.text().contains("x^2 / 7"));
                if text.galley.text() == "7" {
                    fraction = true;
                }
            }
            _ => {}
        }
    }
    assert_eq!(matrix_fences, 2);
    assert!(
        fraction,
        "the algebra row paints the denominator as a glyph run"
    );
}
