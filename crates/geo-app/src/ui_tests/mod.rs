//! Headless interaction tests: synthetic pointer events through the real UI.

use crate::app::App;
use crate::layout::SPLITTER;
use crate::tools::{Tool, GROUPS};
use crate::workbench::Workbench;
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use geo_app_core::app::OwnerRules;
use geo_app_core::metrics::Metrics;
use geo_kernel::kernel::Value;

mod chrome;
mod gestures;
mod pointer;
mod tangents;
mod tools;

/// The harness puts this margin around the root ui.
const MARGIN: f32 = 8.0;
const SIZE: (f32, f32) = (1400.0, 900.0);
const TOOLBAR_HEIGHT: f32 = Metrics::desktop().toolbar_height;
const ALGEBRA_WIDTH: f32 = Metrics::desktop().algebra_width;
/// Heights of the algebra rows of a free and a dependent object.
const ROW: f32 = 50.0;
const ROW_DEPENDENT: f32 = 102.0;

fn harness(script: &str) -> Harness<'static, App> {
    let app = App::new(Workbench::from_script(script).unwrap(), None);
    let mut h = Harness::builder()
        .with_size(egui::vec2(SIZE.0 + 2.0 * MARGIN, SIZE.1 + 2.0 * MARGIN))
        .with_pixels_per_point(1.0)
        .build_ui_state(|ui, app: &mut App| crate::app::show(app, ui), app);
    h.run();
    h
}

/// Window coordinates (as in a 1400 x 900 window) to harness coordinates.
fn at(x: f32, y: f32) -> Pos2 {
    Pos2::new(x + MARGIN, y + MARGIN)
}

/// Window position of the world point `(x, y)` in the standard view, with
/// the on-screen keyboard closed.
fn world(x: f64, y: f64) -> Pos2 {
    let left = ALGEBRA_WIDTH + SPLITTER;
    let (w, h) = (SIZE.0 - left, SIZE.1 - TOOLBAR_HEIGHT);
    at(
        left + w / 2.0 + (x * 50.0) as f32,
        TOOLBAR_HEIGHT + h / 2.0 - (y * 50.0) as f32,
    )
}

/// The middle of the text of the algebra row that starts `top` pixels below
/// the toolbar.
fn row_body(top: f32) -> Pos2 {
    at(120.0, TOOLBAR_HEIGHT + top + ROW / 2.0)
}

/// The marker of the algebra row that starts `top` pixels below the toolbar.
fn row_marker(top: f32) -> Pos2 {
    at(28.0, TOOLBAR_HEIGHT + top + ROW / 2.0)
}

/// The menu button of a free row, or of a dependent one (`dependent`).
fn row_menu(top: f32, dependent: bool) -> Pos2 {
    let y = if dependent { 30.0 } else { ROW / 2.0 };
    at(ALGEBRA_WIDTH - 28.0, TOOLBAR_HEIGHT + top + y)
}

/// The centre of toolbar group button `i`.
fn group_button(i: usize) -> Pos2 {
    at(5.0 + 45.0 * i as f32 + 20.0, TOOLBAR_HEIGHT / 2.0)
}

fn button(h: &mut Harness<'_, App>, pos: Pos2, pressed: bool) {
    h.input_mut().events.push(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    });
    h.step();
}

fn moved(h: &mut Harness<'_, App>, pos: Pos2) {
    h.input_mut().events.push(Event::PointerMoved(pos));
    h.step();
}

fn click(h: &mut Harness<'_, App>, pos: Pos2) {
    moved(h, pos);
    button(h, pos, true);
    button(h, pos, false);
    h.run();
}

fn labels(h: &Harness<'_, App>) -> Vec<String> {
    let c = h.state().wb.construction();
    c.ids().map(|id| c.label(id).to_owned()).collect()
}

#[test]
fn line_tool_makes_each_point_and_the_line_their_own_undo_steps() {
    let mut h = harness("A = (-5.64, 4.44)\n");
    h.state_mut().tool.set_tool(Tool::Line);
    click(&mut h, world(1.0, 1.0));
    assert_eq!(labels(&h), ["A", "B"], "the first point is made at once");
    click(&mut h, world(3.0, -2.0));
    assert_eq!(labels(&h), ["A", "B", "C", "f"]);
    let app = h.state_mut();
    for left in [["A", "B", "C"].as_slice(), &["A", "B"], &["A"]] {
        assert!(app.wb.undo());
        let c = app.wb.construction();
        let now: Vec<&str> = c.ids().map(|id| c.label(id)).collect();
        assert_eq!(now, left);
    }
    assert!(!app.wb.undo());
}

#[test]
fn clicking_existing_points_reuses_them() {
    let mut h = harness("A = (0, 0)\nB = (2, 1)\n");
    h.state_mut().tool.set_tool(Tool::Segment);
    click(&mut h, world(0.0, 0.0));
    click(&mut h, world(2.0, 1.0));
    assert_eq!(labels(&h), ["A", "B", "f"]);
    let c = h.state().wb.construction();
    let f = c.id("f").unwrap();
    assert_eq!(
        c.value(f),
        Value::Segment {
            x1: 0.0,
            y1: 0.0,
            x2: 2.0,
            y2: 1.0
        }
    );
}

#[test]
fn escape_returns_to_the_move_tool() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().tool.set_tool(Tool::Circle);
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(h.state().tool.tool(), Tool::Move);
}

#[test]
fn dragging_a_free_point_moves_it_as_one_undo_step() {
    let mut h = harness("A = (1, 1)\nB = (3, 1)\nf = Line(A, B)\n");
    let start = world(1.0, 1.0);
    moved(&mut h, start);
    button(&mut h, start, true);
    for i in 1..=10 {
        moved(&mut h, start + egui::vec2(5.0 * i as f32, -4.0 * i as f32));
    }
    button(&mut h, start + egui::vec2(50.0, -40.0), false);
    h.run();
    let app = h.state_mut();
    let a = app.wb.construction().id("A").unwrap();
    let Value::Point { x, y } = app.wb.construction().value(a) else {
        panic!("A is not a point");
    };
    // The first few pixels before egui starts the drag do not count.
    assert!((x - 2.0).abs() < 0.25 && (y - 1.8).abs() < 0.25, "{x} {y}");
    assert!(app.wb.undo());
    let a = app.wb.construction().id("A").unwrap();
    assert_eq!(
        app.wb.construction().value(a),
        Value::Point { x: 1.0, y: 1.0 }
    );
    assert!(!app.wb.can_undo(), "one step for the whole drag");
}

#[test]
fn dragging_empty_space_pans_instead_of_moving_points() {
    let mut h = harness("A = (1, 1)\n");
    let start = world(-4.0, -3.0);
    moved(&mut h, start);
    button(&mut h, start, true);
    for i in 1..=6 {
        moved(&mut h, start + egui::vec2(10.0 * i as f32, 0.0));
    }
    button(&mut h, start + egui::vec2(60.0, 0.0), false);
    h.run();
    let app = h.state();
    assert!(!app.wb.can_undo());
    let cam = app.camera.unwrap();
    assert!(
        (cam.cx + 60.0 / 50.0).abs() < 0.05,
        "camera moved left: {}",
        cam.cx
    );
}

/// The owner's rule `interact::REFUSED_DRAG_NOTICE`, under both settings
/// (`App::rules`): a drag of a point that depends on others moves nothing
/// and pans nothing (the reference app's press on an object never pans,
/// `hasNoHitsDisablingModeForShallMoveView`) and selects the point. Ours
/// says why in Polish; the reference app stays silent. Fails if the switch
/// is ignored either way.
#[test]
fn a_refused_drag_of_a_dependent_point_says_why_and_does_not_pan() {
    for rules in [OwnerRules::OURS, OwnerRules::REFERENCE] {
        let mut h = harness("A = (1, 1)\nB = (5, 1)\nM = Midpoint(A, B)\n");
        h.state_mut().rules = rules;
        drag(
            &mut h,
            world(3.0, 1.0),
            world(3.0, 1.0) + egui::vec2(60.0, -30.0),
        );
        assert_eq!(point(&h, "M"), (3.0, 1.0));
        let app = h.state();
        assert!(!app.wb.can_undo());
        let cam = app.camera.unwrap();
        assert!(cam.cx.abs() < 1e-9 && cam.cy.abs() < 1e-9, "panned");
        assert_eq!(app.wb.selected, app.wb.construction().id("M"));
        let notice = geo_app_core::text::label(geo_app_core::text::Label::PointDependsOnOthers);
        let want = rules.refused_drag_notice.then_some(notice);
        assert_eq!(app.wb.notice.as_deref(), want, "{rules:?}");
    }
}

/// A press on a side of a polygon under both settings of the owner's rule
/// `hit::SIDE_TAKES_THE_POLYGON` (`App::rules`). Ours takes the polygon (a
/// side is a segment of free points, so the side itself would drag two
/// vertices and deform the polygon). The reference app, as observed in
/// scenario `press_on_polygon_edge_does_nothing`, selects nothing, moves
/// nothing and does not pan, for a click and for a drag. Fails if the
/// switch is ignored either way.
#[test]
fn a_press_on_a_side_takes_the_polygon() {
    for rules in [OwnerRules::OURS, OwnerRules::REFERENCE] {
        let mut h = harness("A = (1, 1)\nB = (5, 1)\nC = (1, 4)\np = Polygon(A, B, C)\n");
        h.state_mut().rules = rules;
        let p = h.state().wb.construction().id("p");
        assert!(p.is_some());
        click(&mut h, world(3.0, 1.0));
        let want = if rules.side_takes_the_polygon {
            p
        } else {
            None
        };
        assert_eq!(h.state().wb.selected, want, "{rules:?}: the click");
        h.state_mut().wb.selected = None;
        let cam = h.state().camera.unwrap();
        drag(
            &mut h,
            world(3.0, 1.0),
            world(3.0, 1.0) + egui::vec2(60.0, -30.0),
        );
        assert_eq!(h.state().wb.selected, want, "{rules:?}: the drag");
        let moved = point(&h, "A") != (1.0, 1.0);
        assert_eq!(moved, rules.side_takes_the_polygon, "{rules:?}");
        if !rules.side_takes_the_polygon {
            assert_eq!((point(&h, "B"), point(&h, "C")), ((5.0, 1.0), (1.0, 4.0)));
            assert_eq!(h.state().camera.unwrap(), cam, "the press panned");
            assert!(!h.state().wb.can_undo());
        }
    }
}

/// A drag on a side moves the polygon (the owner's rule above, with
/// `moveDependent`): every free vertex follows the pointer, as one undo
/// step. The press at (3, 1) is a grid crossing, so the motion counts from
/// there.
#[test]
fn a_drag_on_a_side_moves_the_polygon_as_one_undo_step() {
    let mut h = harness("A = (1, 1)\nB = (5, 1)\nC = (1, 4)\np = Polygon(A, B, C)\n");
    drag(
        &mut h,
        world(3.0, 1.0),
        world(3.0, 1.0) + egui::vec2(60.0, -30.0),
    );
    let near = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9;
    for (l, at) in [("A", (2.2, 1.6)), ("B", (6.2, 1.6)), ("C", (2.2, 4.6))] {
        assert!(near(point(&h, l), at), "{l} at {:?}", point(&h, l));
    }
    assert_eq!(h.state().wb.selected, h.state().wb.construction().id("p"));
    undo(&mut h);
    assert_eq!(point(&h, "A"), (1.0, 1.0));
    assert_eq!(point(&h, "C"), (1.0, 4.0));
    assert!(!h.state().wb.can_undo(), "one step for the whole drag");
}

/// Escape during a drag of a line puts both its points back.
#[test]
fn escape_cancels_a_drag_of_a_line() {
    let mut h = harness("A = (1, 1)\nB = (3, 2)\nf = Line(A, B)\n");
    let start = world(2.0, 1.5);
    moved(&mut h, start);
    button(&mut h, start, true);
    for i in 1..=8 {
        moved(&mut h, start + egui::vec2(5.0 * i as f32, 5.0 * i as f32));
    }
    assert!(point_x(&h, "A") > 1.2 && point_x(&h, "B") > 3.2);
    h.key_press(egui::Key::Escape);
    h.run();
    button(&mut h, start + egui::vec2(40.0, 40.0), false);
    h.run();
    assert_eq!((point(&h, "A"), point(&h, "B")), ((1.0, 1.0), (3.0, 2.0)));
    assert!(!h.state().wb.can_undo());
}

/// `moveLabel`: a press on a point's label drags the label, not the point,
/// selects nothing and makes no undo step; Escape puts it back.
#[test]
fn a_drag_on_a_label_moves_only_the_label() {
    let mut h = harness("A = (1, 1)\n");
    // The label starts 6 px right of and 6 px above the point.
    let on_label = world(1.0, 1.0) + egui::vec2(10.0, -16.0);
    drag(&mut h, on_label, on_label + egui::vec2(40.0, 30.0));
    let a = h.state().wb.construction().id("A").unwrap();
    assert_eq!(point(&h, "A"), (1.0, 1.0));
    assert_eq!(h.state().wb.label_offset(a), Some((40.0, 30.0)));
    assert_eq!(h.state().wb.selected, None);
    assert!(!h.state().wb.can_undo());
    // A second drag starts from the moved label; Escape restores it.
    let on_label = on_label + egui::vec2(40.0, 30.0);
    moved(&mut h, on_label);
    button(&mut h, on_label, true);
    for i in 1..=8 {
        moved(&mut h, on_label + egui::vec2(-5.0 * i as f32, 0.0));
    }
    assert_ne!(h.state().wb.label_offset(a), Some((40.0, 30.0)));
    h.key_press(egui::Key::Escape);
    h.run();
    button(&mut h, on_label + egui::vec2(-40.0, 0.0), false);
    h.run();
    assert_eq!(h.state().wb.label_offset(a), Some((40.0, 30.0)));
}

#[test]
fn the_marker_toggles_visibility_and_clicking_the_body_selects() {
    let mut h = harness("A = (0, 0)\nB = (2, 0)\n");
    click(&mut h, row_marker(0.0));
    let a = h.state().wb.construction().id("A").unwrap();
    assert!(!h.state().wb.is_visible(a));
    click(&mut h, row_marker(0.0));
    assert!(h.state().wb.is_visible(a));
    // The text of the second row selects B.
    click(&mut h, row_body(ROW));
    let b = h.state().wb.construction().id("B").unwrap();
    assert_eq!(h.state().wb.selected, Some(b));
}

#[test]
fn selecting_in_the_drawing_selects_the_object() {
    let mut h = harness("A = (0, 0)\nB = (2, 0)\n");
    click(&mut h, world(2.0, 0.0));
    let b = h.state().wb.construction().id("B").unwrap();
    assert_eq!(h.state().wb.selected, Some(b));
    click(&mut h, world(-5.0, 4.0));
    assert_eq!(h.state().wb.selected, None);
}

/// Types `text` into the input row below the rows that take `top` pixels
/// and presses Enter. The on-screen keyboard that comes up with the field is
/// put away again, so that `world` stays true.
fn type_command(h: &mut Harness<'_, App>, top: f32, text: &str) {
    click(h, row_body(top));
    h.input_mut().events.push(Event::Text(text.to_owned()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.run();
}

#[test]
fn the_input_row_runs_commands_in_both_label_forms() {
    let mut h = harness("A = (0, 0)\n");
    type_command(&mut h, ROW, "B = (2, 3)");
    assert_eq!(labels(&h), ["A", "B"]);
    type_command(&mut h, 2.0 * ROW, "f: Line(A, B)");
    assert_eq!(labels(&h), ["A", "B", "f"]);
    assert_eq!(h.state().input_error, None);
    assert!(h.state().input.is_empty());
}

#[test]
fn the_input_row_shows_errors_in_polish() {
    let mut h = harness("A = (0, 0)\n");
    type_command(&mut h, ROW, "f = Line(A, Z)");
    assert_eq!(labels(&h), ["A"]);
    assert_eq!(
        h.state().input_error.as_deref(),
        Some("Nieznany obiekt `Z`")
    );
}

#[test]
fn the_row_menu_deletes_the_object_and_its_dependents() {
    let mut h = harness("A = (0, 0)\nB = (2, 0)\nf = Line(A, B)\n");
    click(&mut h, row_menu(0.0, false));
    h.get_by_label("Usuń").click();
    h.run();
    assert_eq!(labels(&h), ["B"]);
    assert!(h.state_mut().wb.undo());
    assert_eq!(labels(&h), ["A", "B", "f"]);
}

#[test]
fn the_row_menu_hides_the_object() {
    let mut h = harness("A = (0, 0)\nB = (2, 0)\n");
    click(&mut h, row_menu(ROW, false));
    h.get_by_label("Ukryj obiekt").click();
    h.run();
    let b = h.state().wb.construction().id("B").unwrap();
    assert!(!h.state().wb.is_visible(b));
}

#[test]
fn cmd_z_undoes_and_shift_cmd_z_redoes() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().wb.run_line("B = (1, 1)").unwrap();
    h.state_mut().wb.run_line("C = (2, 2)").unwrap();
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert_eq!(labels(&h), ["A", "B"]);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert_eq!(labels(&h), ["A"]);
    // Shift+Cmd+Z is a redo, not a second undo.
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, egui::Key::Z);
    h.run();
    assert_eq!(labels(&h), ["A", "B"]);
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, egui::Key::Z);
    h.run();
    assert_eq!(labels(&h), ["A", "B", "C"]);
}

#[test]
fn the_object_under_the_pointer_is_hovered() {
    let mut h = harness("A = (0, 0)\nB = (2, 0)\n");
    moved(&mut h, world(2.0, 0.0));
    let b = h.state().wb.construction().id("B").unwrap();
    assert_eq!(h.state().hovered, Some(b));
    moved(&mut h, world(-5.0, 4.0));
    assert_eq!(h.state().hovered, None);
}

fn drag(h: &mut Harness<'_, App>, from: Pos2, to: Pos2) {
    moved(h, from);
    button(h, from, true);
    for i in 1..=10 {
        moved(h, from + (to - from) * (i as f32 / 10.0));
    }
    button(h, to, false);
    h.run();
}

fn point_x(h: &Harness<'_, App>, label: &str) -> f64 {
    let c = h.state().wb.construction();
    let Value::Point { x, .. } = c.value(c.id(label).unwrap()) else {
        panic!("{label} is not a point");
    };
    x
}

fn point(h: &Harness<'_, App>, label: &str) -> (f64, f64) {
    let c = h.state().wb.construction();
    let Value::Point { x, y } = c.value(c.id(label).unwrap()) else {
        panic!("{label} is not a point");
    };
    (x, y)
}

fn redo(h: &mut Harness<'_, App>) {
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, egui::Key::Z);
    h.run();
}

fn undo(h: &mut Harness<'_, App>) {
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
    h.run();
}

#[test]
fn the_point_tool_on_an_existing_point_selects_it() {
    let mut h = harness("A = (0, 0)\nB = (2, 0)\n");
    h.state_mut().tool.set_tool(Tool::Point);
    click(&mut h, world(0.0, 0.0));
    assert_eq!(labels(&h), ["A", "B"]);
    let a = h.state().wb.construction().id("A").unwrap();
    assert_eq!(h.state().wb.selected, Some(a));
    assert!(!h.state().wb.can_undo());
}

#[test]
fn a_second_click_on_the_same_point_does_not_create_a_duplicate() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().tool.set_tool(Tool::Line);
    click(&mut h, world(0.0, 0.0));
    click(&mut h, world(0.0, 0.0));
    assert_eq!(labels(&h), ["A"], "nothing created yet");
    click(&mut h, world(3.0, 0.0));
    assert_eq!(labels(&h), ["A", "B", "f"]);
}

#[test]
fn hidden_points_cannot_be_dragged_or_picked() {
    let mut h = harness("A = (1, 1)\nB = (3, 1)\n");
    let a = h.state().wb.construction().id("A").unwrap();
    h.state_mut().wb.toggle_visible(a);
    // A drag from the hidden point pans the view instead of moving it.
    drag(&mut h, world(1.0, 1.0), world(2.0, 2.0));
    assert_eq!(point_x(&h, "A"), 1.0);
    let mut h = harness("A = (1, 1)\nB = (3, 1)\n");
    let a = h.state().wb.construction().id("A").unwrap();
    h.state_mut().wb.toggle_visible(a);
    // The segment tool puts a new point where the hidden one is.
    h.state_mut().tool.set_tool(Tool::Segment);
    click(&mut h, world(1.0, 1.0));
    click(&mut h, world(3.0, 1.0));
    assert_eq!(labels(&h), ["A", "B", "C", "f"]);
}

#[test]
fn undo_during_a_drag_ends_the_drag() {
    let mut h = harness("A = (1, 1)\n");
    h.state_mut().wb.run_line("B = (4, 4)").unwrap();
    let start = world(1.0, 1.0);
    moved(&mut h, start);
    button(&mut h, start, true);
    for i in 1..=5 {
        moved(&mut h, start + egui::vec2(4.0 * i as f32, 0.0));
    }
    undo(&mut h);
    let x_after_undo = point_x(&h, "A");
    for i in 6..=12 {
        moved(&mut h, start + egui::vec2(4.0 * i as f32, 0.0));
    }
    button(&mut h, start + egui::vec2(48.0, 0.0), false);
    h.run();
    assert_eq!(point_x(&h, "A"), x_after_undo, "no further movement");
    assert_eq!(labels(&h).len(), 2);
}

#[test]
fn escape_cancels_a_drag_and_restores_the_point() {
    let mut h = harness("A = (1, 1)\n");
    let start = world(1.0, 1.0);
    moved(&mut h, start);
    button(&mut h, start, true);
    for i in 1..=8 {
        moved(&mut h, start + egui::vec2(5.0 * i as f32, 0.0));
    }
    assert!(point_x(&h, "A") > 1.2);
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(point_x(&h, "A"), 1.0);
    button(&mut h, start + egui::vec2(40.0, 0.0), false);
    h.run();
    assert_eq!(point_x(&h, "A"), 1.0);
    assert!(
        !h.state().wb.can_undo(),
        "the cancelled drag leaves no step"
    );
}

#[test]
fn cmd_z_and_escape_belong_to_the_input_row_while_it_has_focus() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().wb.run_line("B = (1, 1)").unwrap();
    h.state_mut().tool.set_tool(Tool::Circle);
    // The input row is the third one.
    click(&mut h, row_body(2.0 * ROW));
    h.input_mut().events.push(Event::Text("abc".to_owned()));
    h.step();
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert_eq!(
        labels(&h),
        ["A", "B"],
        "Cmd+Z did not undo the construction"
    );
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(
        h.state().tool.tool(),
        Tool::Circle,
        "Esc only left the field"
    );
    assert!(h.state().input.is_empty());
}

#[test]
fn a_bare_command_returning_a_point_gets_a_point_label() {
    let mut h = harness("A = (0, 0)\nB = (2, 0)\n");
    h.state_mut().wb.run_line("Midpoint(A, B)").unwrap();
    assert_eq!(labels(&h), ["A", "B", "C"]);
}

#[test]
fn hiding_through_the_marker_is_one_undo_step() {
    let mut h = harness("A = (0, 0)\nB = (2, 0)\n");
    let a = h.state().wb.construction().id("A").unwrap();
    click(&mut h, row_marker(0.0));
    assert!(!h.state().wb.is_visible(a));
    undo(&mut h);
    assert!(h.state().wb.is_visible(a));
    redo(&mut h);
    assert!(!h.state().wb.is_visible(a));
}
