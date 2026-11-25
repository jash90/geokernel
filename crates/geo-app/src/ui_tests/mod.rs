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
mod upstream_tools;
mod zoom;

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
    let wb = &h.state().wb;
    wb.object_ids()
        .map(|id| wb.construction().label(id).to_owned())
        .collect()
}

#[test]
fn a_line_tool_use_with_its_two_new_points_is_one_undo_step() {
    // Spec F1 (`storeUndoInfoAndStateForModeStarting`): the points are made
    // at once but are no steps of their own.
    let mut h = harness("A = (-5.64, 4.44)\n");
    h.state_mut().tool.set_tool(Tool::Line);
    click(&mut h, world(1.0, 1.0));
    assert_eq!(labels(&h), ["A", "B"], "the first point is made at once");
    assert_eq!(h.state().wb.undo_depth(), (0, 0), "and is no step");
    click(&mut h, world(3.0, -2.0));
    assert_eq!(labels(&h), ["A", "B", "C", "f"]);
    let app = h.state_mut();
    assert_eq!(app.wb.undo_depth(), (1, 0));
    assert!(app.undo());
    assert_eq!(labels(&h), ["A"]);
    assert!(!h.state_mut().undo());
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
    // The pan is one undo step right after it (the owner's rule,
    // `OwnerRules::view_changes_are_steps`; the reference app stores a view
    // change only at the next press).
    let app = h.state();
    assert_eq!(app.wb.undo_depth(), (1, 0));
    let cam = app.camera.unwrap();
    assert!(
        (cam.cx + 60.0 / 50.0).abs() < 0.05,
        "camera moved left: {}",
        cam.cx
    );
    assert!(h.state_mut().undo());
    h.run();
    let cam = h.state().camera.unwrap();
    assert!(cam.cx.abs() < 1e-9, "undo put the view back: {}", cam.cx);
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
        h.state_mut().set_rules(rules);
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
        h.state_mut().set_rules(rules);
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

/// A line that fails in the middle of a tool use changes nothing: the
/// pending point and the pick survive the rebuilds, and the use still
/// finishes as one step. Fails when the line made the point permanent or
/// when the rebuild dropped the picks.
#[test]
fn a_failed_line_in_the_middle_of_a_tool_use_lets_the_use_go_on() {
    let mut h = harness("Z = (-3, -3)\n");
    h.state_mut().select_tool(Tool::Segment);
    click(&mut h, world(1.0, 1.0));
    assert_eq!(labels(&h), ["Z", "A"]);
    type_command(&mut h, 2.0 * ROW, "Nonsense((");
    assert!(h.state().input_error.is_some());
    assert_eq!(labels(&h), ["Z", "A"]);
    assert_eq!(h.state().tool.picks().len(), 1);
    assert_eq!(h.state().wb.undo_depth(), (0, 0));
    click(&mut h, world(2.0, 1.0));
    assert_eq!(labels(&h), ["Z", "A", "B", "f"]);
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
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
fn a_second_click_on_the_same_point_deselects_without_creating_a_duplicate() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().tool.set_tool(Tool::Line);
    click(&mut h, world(0.0, 0.0));
    click(&mut h, world(0.0, 0.0));
    assert_eq!(labels(&h), ["A"], "nothing created yet");
    assert!(h.state().tool.picks().is_empty());
    click(&mut h, world(3.0, 0.0));
    assert_eq!(labels(&h), ["A", "B"]);
    click(&mut h, world(0.0, 0.0));
    assert_eq!(labels(&h), ["A", "B", "f"]);
    assert_eq!(h.state().wb.definition("f"), Some("Line(B, A)"));
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

#[test]
fn a_saved_camera_with_every_point_offscreen_fits_the_construction() {
    let mut h = harness("A = (0, 0)");
    let source = br#"<geogebra><euclidianView>
      <size width="800" height="600"/>
      <coordSystem xZero="-15000" yZero="-15000" scale="50"/>
    </euclidianView><construction>
      <element type="point" label="A"><coords x="1" y="2" z="1"/></element>
      <element type="point" label="B"><coords x="4" y="3" z="1"/></element>
    </construction></geogebra>"#;
    h.state_mut().wb.load_bytes(source, "distant.ggb").unwrap();
    h.state_mut().view_request = Some(crate::app::ViewRequest::Initial);
    h.run();
    for label in ["A", "B"] {
        let app = h.state();
        let id = app.wb.construction().id(label).unwrap();
        assert!(app
            .label_rects
            .borrow()
            .iter()
            .any(|(drawn, _)| *drawn == id));
    }
}

#[test]
fn slider_tool_opens_on_click_and_places_the_default_slider() {
    // SliderToolTest.sliderTool and sliderToolPreviewDoesNotShowDialog.
    let mut h = harness("");
    h.state_mut().select_tool(Tool::Slider);
    let at = at(ALGEBRA_WIDTH + SPLITTER + 100.0, TOOLBAR_HEIGHT + 100.0);
    moved(&mut h, at);
    assert!(h.state().tool.control_request().is_none());
    click(&mut h, at);
    assert_eq!(h.state().tool.control_request(), Some((100.0, 100.0)));
    assert!(labels(&h).is_empty());
    h.get_by_label("OK").click();
    h.run();
    let c = h.state().wb.construction();
    let id = c.ids().next().unwrap();
    assert_eq!(c.value(id), Value::Number(1.0));
    assert_eq!(c.slider_interval(id), Some((-5.0, 5.0)));
    assert_eq!(c.slider_style(id).step, Some(0.1));
    assert_eq!(c.slider_style(id).position, Some((100.0, 100.0)));
    assert!(h.state_mut().undo());
    assert!(labels(&h).is_empty());
    assert!(h.state_mut().redo());
    let c = h.state().wb.construction();
    assert_eq!(
        c.slider_style(c.ids().next().unwrap()).position,
        Some((100.0, 100.0))
    );
}

#[test]
fn slider_drag_snaps_updates_dependents_and_is_one_undo_step() {
    let mut h = harness("a = Slider(0, 8, 0.5)\nb = 2 a");
    let from = at(ALGEBRA_WIDTH + SPLITTER + 30.0, TOOLBAR_HEIGHT + 50.0);
    drag(&mut h, from, from + egui::vec2(94.0, 0.0));
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(4.0));
    assert_eq!(c.value(c.id("b").unwrap()), Value::Number(8.0));
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    assert!(h.state_mut().undo());
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(0.0));
    assert!(h.state_mut().redo());
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(4.0));
}

#[test]
fn slider_click_advances_one_notch_and_drag_multiplies_before_dividing() {
    let mut h = harness("a = Slider(0, 1, 0.1)\na = 0.25\nb = Slider(0, 10, 0.1)");
    let canvas = |x, y| at(ALGEBRA_WIDTH + SPLITTER + x, TOOLBAR_HEIGHT + y);
    let knob = canvas(80.0, 50.0);
    moved(&mut h, knob);
    button(&mut h, knob, true);
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(0.35));
    button(&mut h, knob, false);
    drag(&mut h, canvas(30.0, 90.0), canvas(77.0, 90.0));
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(0.35));
    assert_eq!(c.value(c.id("b").unwrap()), Value::Number(2.4));
}

#[test]
fn dragging_a_playing_slider_pauses_the_timer() {
    let mut h = harness("a = Slider(0, 10, 0.1)\n$sliderAnimation(_61, 0.1, 1, 0, true)");
    let id = h.state().wb.construction().id("a").unwrap();
    h.state_mut().wb.set_animation_running(false);
    h.state_mut().wb.set_number(id, 0.0);
    let from = at(ALGEBRA_WIDTH + SPLITTER + 30.0, TOOLBAR_HEIGHT + 50.0);
    moved(&mut h, from);
    h.state_mut().wb.set_animation_running(true);
    assert!(h.state().wb.is_animating());
    button(&mut h, from, true);
    let to = from + egui::vec2(94.0, 0.0);
    for _ in 0..4 {
        moved(&mut h, to);
    }
    button(&mut h, to, false);
    h.run();
    assert!(!h.state().wb.is_animating());
    assert!(h.state().wb.construction().animation(id).unwrap().playing);
    assert_eq!(h.state().wb.construction().value(id), Value::Number(4.7));
}

#[test]
fn dragging_an_overlapping_track_moves_the_selected_slider() {
    let mut h = harness(
        "a = Slider(0, 10)\na = 5\nb = Slider(0, 10)\n\
         $sliderStyle(_62, true, 30, 50, 200, true, false, _, _)",
    );
    let from = at(ALGEBRA_WIDTH + SPLITTER + 130.0, TOOLBAR_HEIGHT + 50.0);
    let b = h.state().wb.construction().id("b").unwrap();
    h.state_mut().wb.selected = Some(b);
    drag(&mut h, from, from + egui::vec2(40.0, 25.0));
    let c = h.state().wb.construction();
    assert_eq!(c.slider_position(b, c.slider_style(b)), (70.0, 75.0));
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(5.0));
    assert_eq!(c.value(b), Value::Number(0.0));
}

#[test]
fn checkbox_caption_hit_rectangle_includes_the_blank_strip() {
    let caption = geo_kernel::kernel::control::encode_text("A long checkbox caption");
    let mut h = harness(&format!(
        "b = $checkBox(false, {caption})\n$checkboxStyle(_62, 80, 200, false)"
    ));
    click(
        &mut h,
        at(ALGEBRA_WIDTH + SPLITTER + 140.0, TOOLBAR_HEIGHT + 202.0),
    );
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("b").unwrap()), Value::Bool(true));
}

#[test]
fn checkbox_math_caption_uses_the_formula_renderer() {
    let caption = geo_kernel::kernel::control::encode_text(r"$\frac{1}{2}$");
    let before = crate::latex::layouts_built();
    let mut h = harness(&format!("b = $checkBox(false, {caption})"));
    assert_eq!(crate::latex::layouts_built(), before + 1);
    h.run();
    assert_eq!(crate::latex::layouts_built(), before + 1);
}

#[test]
fn checkbox_tool_creates_a_control_that_toggles_on_canvas() {
    let mut h = harness("");
    h.state_mut().select_tool(Tool::Checkbox);
    let from = at(ALGEBRA_WIDTH + SPLITTER + 100.0, TOOLBAR_HEIGHT + 100.0);
    click(&mut h, from);
    h.get_by_label("OK").click();
    h.run();
    let value = |h: &Harness<'_, App>| {
        let c = h.state().wb.construction();
        c.value(c.id("a").unwrap())
    };
    assert_eq!(value(&h), Value::Bool(true));
    h.state_mut().select_tool(Tool::Move);
    click(&mut h, from + egui::vec2(15.0, 15.0));
    assert_eq!(value(&h), Value::Bool(false));
    assert_eq!(h.state().wb.undo_depth(), (2, 0));
    assert!(h.state_mut().undo());
    assert_eq!(value(&h), Value::Bool(true));
}

#[test]
fn fixed_canvas_controls_change_value_without_moving() {
    let mut h = harness(
        "a = Slider(0, 10)\n\
         $sliderStyle(_61, true, 80, 100, 100, true, true, _, _)\n\
         b = $checkBox(true, _78)\n\
         $checkboxStyle(_62, 80, 200, true)",
    );
    let canvas = |x, y| at(ALGEBRA_WIDTH + SPLITTER + x, TOOLBAR_HEIGHT + y);
    drag(&mut h, canvas(80.0, 100.0), canvas(165.0, 100.0));
    click(&mut h, canvas(95.0, 215.0));
    let c = h.state().wb.construction();
    let (a, b) = (c.id("a").unwrap(), c.id("b").unwrap());
    assert_eq!(c.value(a), Value::Number(8.5));
    assert_eq!(c.value(b), Value::Bool(false));
    assert_eq!(c.slider_position(a, c.slider_style(a)), (80.0, 100.0));
    assert_eq!(c.checkbox_position(b), Some((80.0, 200.0)));
    assert_eq!(h.state().wb.undo_depth(), (2, 0));
    drag(&mut h, canvas(95.0, 215.0), canvas(180.0, 215.0));
    let c = h.state().wb.construction();
    assert_eq!(c.value(b), Value::Bool(true));
    assert_eq!(c.checkbox_position(b), Some((80.0, 200.0)));
    assert_eq!(h.state().wb.undo_depth(), (3, 0));
}

#[test]
fn an_imported_vertical_sliders_label_moves_separately_from_its_knob() {
    let mut h = harness("");
    h.state_mut()
        .wb
        .load_for_export(
            br#"<geogebra format="5.0"><construction>
             <element type="numeric" label="a">
               <value val="2"/><show object="true" label="true"/>
               <slider min="0" max="8" absoluteScreenLocation="true"
                       x="140" y="200" width="120" horizontal="false"/>
               <animation step="0.5" type="0" playing="false"/>
               <labelOffset x="40" y="6"/>
             </element>
           </construction></geogebra>"#,
            "vertical-slider",
        )
        .unwrap();
    h.run();
    let (canvas, _) = h.state().view.unwrap();
    let canvas_at = |x, y| Pos2::new(canvas.min.x + x, canvas.min.y + y);
    let id = h.state().wb.construction().id("a").unwrap();
    let label = h
        .state()
        .label_rects
        .borrow()
        .iter()
        .find(|(label, _)| *label == id)
        .unwrap()
        .1;
    let from = canvas_at(
        label.left() + label.width() / 2.0,
        label.top() + label.height() / 2.0,
    );
    drag(&mut h, from, from + egui::vec2(22.0, 11.0));
    assert_eq!(h.state().wb.label_offset(id), Some((62.0, 17.0)));
    assert_eq!(h.state().wb.construction().value(id), Value::Number(2.0));
    assert_eq!(h.state().wb.undo_depth(), (0, 0));
    drag(&mut h, canvas_at(140.0, 170.0), canvas_at(140.0, 63.0));
    assert_eq!(h.state().wb.construction().value(id), Value::Number(8.0));
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
}

#[test]
fn imported_slider_labels_use_saved_rounding_and_label_mode_in_canvas_and_svg() {
    let third = "0.3333333333333333";
    for (settings, mode, value, expected) in [
        ("<decimals val=\"2\"/>", 1, third, "a = 0.33"),
        ("<significantfigures val=\"4\"/>", 1, third, "a = 0.3333"),
        ("<decimals val=\"2\"/>", 2, "2", "2"),
        ("<significantfigures val=\"2\"/>", 2, "2.25", "2.3"),
        ("<decimals val=\"2\"/>", 0, third, "a"),
    ] {
        let xml = format!(
            r#"<geogebra format="5.0"><kernel>{settings}</kernel><construction>
              <element type="numeric" label="a">
                <value val="{value}"/><show object="true" label="true"/>
                <slider min="0" max="3" x="160" y="130" width="200"
                        horizontal="true" absoluteScreenLocation="true"/>
                <labelMode val="{mode}"/>
              </element>
            </construction></geogebra>"#
        );
        let mut h = harness("");
        h.state_mut()
            .wb
            .load_for_export(xml.as_bytes(), "slider.xml")
            .unwrap();
        h.run();
        let canvas = h.state().view.unwrap().0;
        assert!(h.output().shapes.iter().any(|shape| {
            shape.clip_rect.left() >= canvas.min.x
                && matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == expected)
        }), "missing canvas label {expected:?}");
        let c = h.state().wb.construction();
        let svg = geo_kernel::svg::render(c, geo_kernel::svg::View::fit(c), 800, 600);
        assert!(svg.contains(&format!(">{expected}</text>")), "{svg}");
    }
}

#[test]
fn checkbox_caption_hit_rectangle_rejects_a_click_left_of_it() {
    let mut h = harness("b = $checkBox(false, _78)\n$checkboxStyle(_62, 80, 200, false)");
    click(
        &mut h,
        at(ALGEBRA_WIDTH + SPLITTER + 78.0, TOOLBAR_HEIGHT + 216.0),
    );
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("b").unwrap()), Value::Bool(false));
}

#[test]
fn checkbox_formula_accepts_spaces_after_the_closing_delimiter() {
    let caption = geo_kernel::kernel::control::encode_text("$x$ ");
    let before = crate::latex::layouts_built();
    let _h = harness(&format!("b = $checkBox(false, {caption})"));
    assert_eq!(crate::latex::layouts_built(), before + 1);
}

#[test]
fn dragging_an_unlocked_checkbox_with_a_show_condition_preserves_its_value() {
    let mut h = harness(
        "show = true\nb = $checkBox(false, _78)\n\
         $checkboxStyle(_62, 80, 200, false)\n$showObject(b, _73686f77)",
    );
    let from = at(ALGEBRA_WIDTH + SPLITTER + 95.0, TOOLBAR_HEIGHT + 215.0);
    let b = h.state().wb.construction().id("b").unwrap();
    assert!(h.state().wb.is_visible(b));
    drag(&mut h, from, from + egui::vec2(40.0, 0.0));
    let check = |h: &Harness<'_, App>, x| {
        let c = h.state().wb.construction();
        let b = c.id("b").unwrap();
        assert_eq!(c.checkbox_position(b), Some((x, 200.0)));
        assert_eq!(c.value(b), Value::Bool(false));
        assert!(h.state().wb.is_visible(b));
    };
    check(&h, 120.0);
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    assert!(h.state_mut().undo());
    check(&h, 80.0);
    assert!(h.state_mut().redo());
    check(&h, 120.0);
}

#[test]
fn a_nearby_track_press_advances_one_notch_and_then_drags_the_value() {
    for (size, pressed, dragged, position) in
        [(5, 5.0, 5.0, (70.0, 50.0)), (7, 6.0, 8.0, (30.0, 50.0))]
    {
        let mut h = harness(&format!(
            "a = Slider(0, 10, 1)\na = 5\n\
             $sliderStyle(_61, true, 30, 50, 200, true, false, {size}, 1)",
        ));
        let from = at(ALGEBRA_WIDTH + SPLITTER + 154.0, TOOLBAR_HEIGHT + 50.0);
        let to = from + egui::vec2(40.0, 0.0);
        moved(&mut h, from);
        button(&mut h, from, true);
        let number = |h: &Harness<'_, App>| {
            let c = h.state().wb.construction();
            c.value(c.id("a").unwrap())
        };
        assert_eq!(number(&h), Value::Number(pressed), "knob size {size}");
        for _ in 0..10 {
            moved(&mut h, to);
        }
        button(&mut h, to, false);
        assert_eq!(number(&h), Value::Number(dragged), "knob size {size}");
        let c = h.state().wb.construction();
        let a = c.id("a").unwrap();
        assert_eq!(c.slider_position(a, c.slider_style(a)), position);
        assert_eq!(h.state().wb.undo_depth(), (1, 0));
    }
}

#[test]
fn input_box_canvas_updates_slider_and_dependents() {
    let mut h = harness("a = Slider(0, 8, 0.5)\nb = 2 a\nfield = $inputBox(a, _)\n$inputBoxStyle(_6669656c64, 124, 88, 12)");
    let c = h.state().wb.construction();
    let field = c.id("field").unwrap();
    let [x, y, ..] = c.input_box_rect(field);
    let pos = at(
        ALGEBRA_WIDTH + SPLITTER + x as f32 + 10.0,
        TOOLBAR_HEIGHT + y as f32 + 13.0,
    );
    click(&mut h, pos);
    assert_eq!(h.state().wb.selected, Some(field));
    h.input_mut().events.push(Event::Text("3.5".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.run();
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(3.5));
    assert_eq!(c.value(c.id("b").unwrap()), Value::Number(7.0));
}

#[test]
fn input_box_accepting_an_unchanged_angle_keeps_its_value() {
    let mut h = harness("a = 33.33333333333333°\nfield = $inputBox(a, _)");
    let c = h.state().wb.construction();
    let angle = c.id("a").unwrap();
    let original = c.value(angle);
    let [x, y, ..] = c.input_box_rect(c.id("field").unwrap());
    let pos = at(
        ALGEBRA_WIDTH + SPLITTER + x as f32 + 10.0,
        TOOLBAR_HEIGHT + y as f32 + 13.0,
    );

    click(&mut h, pos);
    assert_eq!(h.state().input_box_edit.as_ref().unwrap().text, "33.33°");
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(h.state().wb.construction().value(angle), original);
    assert_eq!(h.state().wb.undo_depth(), (0, 0));

    click(&mut h, pos);
    h.input_mut().events.push(Event::Text("42°".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(
        h.state().wb.construction().value(angle),
        Value::Number(42.0_f64.to_radians()),
    );
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
}

#[test]
fn input_box_tool_places_a_control() {
    for link in [None, Some("a")] {
        let mut h = harness("a = 1");
        h.state_mut().select_tool(Tool::InputBox);
        click(
            &mut h,
            at(ALGEBRA_WIDTH + SPLITTER + 124.0, TOOLBAR_HEIGHT + 88.0),
        );
        if let Some(link) = link {
            h.get_by_label("Powiązany obiekt").click();
            h.step();
            h.input_mut().events.push(Event::Text(link.into()));
            h.step();
        }
        h.get_by_label("OK").click();
        h.run();
        let c = h.state().wb.construction();
        let field = c
            .ids()
            .find(|&id| c.kind(id) == geo_kernel::Kind::InputBox)
            .unwrap();
        assert_eq!(
            c.def(field),
            geo_kernel::Def::InputBox(link.and_then(|label| c.id(label)))
        );
        assert_eq!(h.state().wb.undo_depth(), (1, 0));
    }
}

#[test]
fn input_box_canvas_text_is_bounded() {
    let source = format!(
        "note = \"{}\"\nfield = $inputBox(note, _)",
        "a".repeat(16384)
    );
    let h = harness(&source);
    let c = h.state().wb.construction();
    let field = c.id("field").unwrap();
    let svg = geo_kernel::svg::render_visible(c, geo_kernel::svg::View::fit(c), 400, 300, |id| {
        id == field
    });
    assert!(svg.contains(">aaaaaaaa"));
    assert!(!svg.contains(&"a".repeat(1024)));
}

#[test]
fn input_box_canvas_edits_a_boolean_and_a_text_literal() {
    let mut h = harness(
        "b = $checkBox(true, _)\nnote = \"x < y\"\nflag = $inputBox(b, _)\nwords = $inputBox(note, _)\n\
         $inputBoxStyle(_666c6167, 124, 128, 8)\n\
         $inputBoxStyle(_776f726473, 124, 168, 20)",
    );
    for (label, data) in [("flag", "false"), ("words", "two \"quotes\"")] {
        let c = h.state().wb.construction();
        let [x, y, ..] = c.input_box_rect(c.id(label).unwrap());
        click(
            &mut h,
            at(
                ALGEBRA_WIDTH + SPLITTER + x as f32 + 10.0,
                TOOLBAR_HEIGHT + y as f32 + 13.0,
            ),
        );
        assert_eq!(
            h.state()
                .input_box_edit
                .as_ref()
                .map(|edit| edit.label.as_str()),
            Some(label),
        );
        h.input_mut().events.push(Event::Text(data.into()));
        h.step();
        assert_eq!(
            h.state()
                .input_box_edit
                .as_ref()
                .map(|edit| edit.text.as_str()),
            Some(data)
        );
        h.key_press(egui::Key::Enter);
        h.run();
        let c = h.state().wb.construction();
        assert_eq!(c.value(c.id("b").unwrap()), Value::Bool(false), "{label}");
    }
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("b").unwrap()), Value::Bool(false));
    assert_eq!(c.text(c.id("note").unwrap()), Some("two \"quotes\""));
    assert_eq!(h.state().wb.undo_depth(), (2, 0));
}

#[test]
fn canvas_list_tool_creates_a_live_list_from_the_selection_rectangle() {
    let mut h = harness("A = (0, 0)\nB = (2, 2)\nC = (5, 1)");
    let tool = Tool::all()
        .find(|tool| tool.caption() == "Utwórz listę")
        .expect("create list tool");
    h.state_mut().select_tool(tool);
    drag(&mut h, world(-0.5, 2.5), world(2.5, -0.5));
    let c = h.state().wb.construction();
    let l = c.id("l1").expect("list from the two enclosed points");
    assert_eq!(
        geo_app_core::describe::object_text(c, l),
        "{(0, 0), (2, 2)}"
    );
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    let bytes = h.state().wb.export_ggb(None).unwrap();
    let mut again = Workbench::default();
    again.load_for_export(&bytes, "list-tool").unwrap();
    let a = again.construction().id("A").unwrap();
    assert!(again.move_point(a, 1.0, 1.0));
    let c = again.construction();
    assert_eq!(
        geo_app_core::describe::object_text(c, c.id("l1").unwrap()),
        "{(1, 1), (2, 2)}"
    );
}

#[test]
fn canvas_remaining_text_tool_creates_text_at_the_click() {
    let mut h = harness("");
    let tool = Tool::all()
        .find(|tool| tool.caption() == "Tekst")
        .expect("text tool");
    h.state_mut().select_tool(tool);
    click(&mut h, world(2.0, 1.0));
    h.input_mut().events.push(Event::Text("Placed text".into()));
    h.step();
    h.get_by_label("OK").click();
    h.run();
    let c = h.state().wb.construction();
    let text = c.id("text1").unwrap();
    assert_eq!(c.text(text), Some("Placed text"));
    assert_eq!(
        c.text_position(text),
        Some(geo_kernel::kernel::text::TextPosition::World { x: 2.0, y: 1.0 })
    );
    let rect = h
        .state()
        .text_rects
        .borrow()
        .iter()
        .find(|(id, _)| *id == text)
        .unwrap()
        .1;
    h.state_mut().select_tool(Tool::Move);
    click(
        &mut h,
        at(
            ALGEBRA_WIDTH + SPLITTER + (rect.min.x + rect.max.x) * 0.5,
            TOOLBAR_HEIGHT + (rect.min.y + rect.max.y) * 0.5,
        ),
    );
    assert_eq!(h.state().wb.selected, Some(text));
}

#[test]
fn canvas_remaining_pen_tool_tracks_drag_and_is_one_undo_step() {
    let mut h = harness("");
    let tool = Tool::all()
        .find(|tool| tool.caption() == "Pióro")
        .expect("pen tool");
    h.state_mut().select_tool(tool);
    drag(&mut h, world(0.0, 0.0), world(1.0, -1.0));
    let c = h.state().wb.construction();
    let points = c.stroke(c.id("stroke1").unwrap()).unwrap();
    assert!(points.defined_points().count() >= 2);
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
}

#[test]
fn canvas_remaining_locus_tool_uses_the_dependent_point_then_its_driver() {
    let mut h =
        harness("A = (0, 0)\nB = (4, 0)\nk = Circle(A, B)\nP = Point(k)\nQ = Midpoint(A, P)");
    let tool = Tool::all()
        .find(|tool| tool.caption() == "Miejsce geometryczne")
        .expect("locus tool");
    h.state_mut().select_tool(tool);
    let (x, y) = point(&h, "Q");
    click(&mut h, world(x, y));
    let (x, y) = point(&h, "P");
    click(&mut h, world(x, y));
    let c = h.state().wb.construction();
    assert!(c.locus(c.id("locus1").unwrap()).unwrap().defined);
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
}

#[test]
fn input_box_commits_on_tab_and_canvas_click() {
    for tab in [true, false] {
        let mut h = harness("a = Slider(0, 8, 0.5)\nb = 2 a\nfield = $inputBox(a, _)\n$inputBoxStyle(_6669656c64, 124, 88, 12)");
        let c = h.state().wb.construction();
        let [x, y, ..] = c.input_box_rect(c.id("field").unwrap());
        click(
            &mut h,
            at(
                ALGEBRA_WIDTH + SPLITTER + x as f32 + 10.0,
                TOOLBAR_HEIGHT + y as f32 + 13.0,
            ),
        );
        h.input_mut().events.push(Event::Text("3.5".into()));
        h.step();
        assert_eq!(
            h.state()
                .input_box_edit
                .as_ref()
                .map(|edit| edit.text.as_str()),
            Some("3.5")
        );
        if tab {
            h.key_press(egui::Key::Tab);
            h.run();
        } else {
            click(&mut h, world(3.0, -2.0));
        }
        let c = h.state().wb.construction();
        assert_eq!(
            c.value(c.id("a").unwrap()),
            Value::Number(3.5),
            "Tab: {tab}"
        );
        assert_eq!(c.value(c.id("b").unwrap()), Value::Number(7.0));
        assert_eq!(h.state().wb.undo_depth(), (1, 0));
    }
}

#[test]
fn canvas_text_tool_follows_the_selected_point() {
    let mut h = harness("A = (2, 1)");
    h.state_mut().select_tool(Tool::Text);
    click(&mut h, world(2.0, 1.0));
    h.input_mut().events.push(Event::Text("Label".into()));
    h.step();
    h.get_by_label("OK").click();
    h.run();
    let a = h.state().wb.construction().id("A").unwrap();
    assert!(h.state_mut().wb.move_point(a, 3.0, 2.0));
    let c = h.state().wb.construction();
    assert_eq!(
        c.text_position(c.id("text1").unwrap()),
        Some(geo_kernel::kernel::text::TextPosition::Point(a))
    );
}

#[test]
fn canvas_locus_tool_accepts_the_driver_first() {
    let mut h = harness("A = (0, 0)\nk = Circle(A, 4)\nP = Point(k)\nQ = Midpoint(A, P)");
    h.state_mut().select_tool(Tool::Locus);
    for label in ["P", "Q"] {
        let (x, y) = point(&h, label);
        click(&mut h, world(x, y));
    }
    let c = h.state().wb.construction();
    assert!(
        c.locus(c.id("locus1").expect("driver-first locus"))
            .unwrap()
            .defined
    );
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
}

#[test]
fn canvas_create_list_filters_polygon_vertices_and_sides() {
    let mut h = harness("A = (0, 0)\nB = (2, 0)\nC = (0, 2)\nD = (3, 2)\npoly = Polygon(A, B, C)");
    h.state_mut().select_tool(Tool::CreateList);
    drag(&mut h, world(-0.5, 2.5), world(3.5, -0.5));
    let c = h.state().wb.construction();
    let list = c.elements(c.id("l1").unwrap()).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list.object(0), c.id("poly"));
    assert_eq!(list.object(1), c.id("D"));
}

#[test]
fn canvas_pen_keeps_motion_before_a_same_frame_release() {
    let mut h = harness("");
    h.state_mut().select_tool(Tool::PenStroke);
    let start = world(0.0, 0.0);
    let bend = world(1.0, 1.0);
    let end = world(2.0, 0.0);
    moved(&mut h, start);
    let pointer = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    h.input_mut().events.extend([
        pointer(start, true),
        Event::PointerMoved(bend),
        Event::PointerMoved(end),
        pointer(end, false),
    ]);
    h.step();
    let c = h.state().wb.construction();
    let stroke = c.stroke(c.id("stroke1").expect("stroke")).unwrap();
    assert!(stroke.defined_points().any(|p| p.y > 0.5));
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
}

#[test]
fn input_box_tab_reaches_an_idle_box_and_uses_its_caption() {
    let mut h = harness("a = 0\nb = 0\nfirst = $inputBox(a, _)\nsecond = $inputBox(b, _45646974207365636f6e64)\n$inputBoxStyle(_6669727374, 124, 88, 12)\n$inputBoxStyle(_7365636f6e64, 124, 128, 12)");
    h.get_by_role_and_label(egui::Role::TextInput, "Edit second");
    let c = h.state().wb.construction();
    let [x, y, ..] = c.input_box_rect(c.id("first").unwrap());
    click(
        &mut h,
        at(
            ALGEBRA_WIDTH + SPLITTER + x as f32 + 10.0,
            TOOLBAR_HEIGHT + y as f32 + 13.0,
        ),
    );
    h.key_press(egui::Key::Tab);
    h.run();
    assert_eq!(
        h.state()
            .input_box_edit
            .as_ref()
            .map(|edit| edit.label.as_str()),
        Some("second")
    );
    h.input_mut().events.push(Event::Text("4".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.run();
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("b").unwrap()), Value::Number(4.0));
}

fn click_canvas_input_box(h: &mut Harness<'_, App>, label: &str) {
    let c = h.state().wb.construction();
    let [x, y, ..] = c.input_box_rect(c.id(label).unwrap());
    click(
        h,
        at(
            ALGEBRA_WIDTH + SPLITTER + x as f32 + 10.0,
            TOOLBAR_HEIGHT + y as f32 + 13.0,
        ),
    );
}

fn drop_saved_input_box(h: &mut Harness<'_, App>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../golden/ggb/saved/saved-input-box-001.xml");
    #[derive(Debug)]
    struct Dropped(std::path::PathBuf);
    impl egui::DroppedFile for Dropped {
        fn path(&self) -> &std::path::Path {
            &self.0
        }

        fn bytes(&self) -> Result<Vec<u8>, String> {
            std::fs::read(&self.0).map_err(|error| error.to_string())
        }
    }
    h.input_mut()
        .dropped_files
        .push(std::sync::Arc::new(Dropped(path)));
    h.step();
    h.input_mut().dropped_files.clear();
}

#[test]
fn closing_input_box_draft_stays_with_its_worksheet() {
    let mut h = harness(
        "a = Slider(0, 8, 0.5)\nfield = $inputBox(a, _)\n$inputBoxStyle(_6669656c64, 124, 88, 12)",
    );
    click_canvas_input_box(&mut h, "field");
    h.input_mut().events.push(Event::Text("5.5".into()));
    h.step();
    assert_eq!(h.state().input_box_edit.as_ref().unwrap().text, "5.5");
    drop_saved_input_box(&mut h);
    h.key_press(egui::Key::Tab);
    h.run();
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(3.5));
    assert_eq!(h.state().wb.undo_depth(), (0, 0));
}

#[test]
fn closing_input_box_commits_when_hidden() {
    let mut h = harness("a = Slider(0, 8, 0.5)\nb = 2 a\nfield = $inputBox(a, _)\n$inputBoxStyle(_6669656c64, 124, 88, 12)");
    click_canvas_input_box(&mut h, "field");
    assert!(h.state().input_box_edit.is_some());
    h.input_mut().events.push(Event::Text("3.5".into()));
    h.step();
    let field = h.state().wb.construction().id("field").unwrap();
    click(&mut h, row_marker(ROW + ROW_DEPENDENT));
    let c = h.state().wb.construction();
    assert!(!h.state().wb.is_visible(field));
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(3.5));
    assert_eq!(c.value(c.id("b").unwrap()), Value::Number(7.0));
}

#[test]
fn closing_input_box_commits_text_in_the_click_frame() {
    let mut h = harness("a = Slider(0, 8, 0.5)\nb = 2 a\nfield = $inputBox(a, _)\n$inputBoxStyle(_6669656c64, 124, 88, 12)");
    click_canvas_input_box(&mut h, "field");
    assert!(h.state().input_box_edit.is_some());
    let pos = world(3.0, -2.0);
    h.input_mut().events.push(Event::Text("3.5".into()));
    h.input_mut().events.push(Event::PointerMoved(pos));
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    h.run();
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(3.5));
    assert_eq!(c.value(c.id("b").unwrap()), Value::Number(7.0));
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    assert!(h.state().input_box_edit.is_none());
}

#[test]
fn closing_input_box_tab_visits_three_editors_in_order() {
    let mut h = harness(
        "a = 0\nb = 0\nc = 0\nfirst = $inputBox(a, _)\nsecond = $inputBox(b, _)\nthird = $inputBox(c, _)\n\
         $inputBoxStyle(_6669727374, 124, 88, 12)\n$inputBoxStyle(_7365636f6e64, 124, 128, 12)\n\
         $inputBoxStyle(_7468697264, 124, 168, 12)",
    );
    click_canvas_input_box(&mut h, "first");
    for expected in ["second", "third"] {
        h.key_press(egui::Key::Tab);
        h.run();
        assert_eq!(
            h.state()
                .input_box_edit
                .as_ref()
                .map(|edit| edit.label.as_str()),
            Some(expected)
        );
    }
    h.input_mut().events.push(Event::Text("3.5".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.run();
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("c").unwrap()), Value::Number(3.5));
}

#[test]
fn closing_pen_keeps_both_gestures_in_one_frame() {
    let mut h = harness("");
    h.state_mut().select_tool(Tool::PenStroke);
    moved(&mut h, world(0.0, 0.0));
    for y in [0.0, -2.0] {
        let start = world(0.0, y);
        let end = world(2.0, y);
        let pointer = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        h.input_mut().events.extend([
            pointer(start, true),
            Event::PointerMoved(world(1.0, y + 1.0)),
            Event::PointerMoved(end),
            pointer(end, false),
        ]);
    }
    h.step();
    let c = h.state().wb.construction();
    let points = || {
        c.ids()
            .filter_map(|id| c.stroke(id))
            .flat_map(|s| s.defined_points())
    };
    assert!(points().any(|p| p.y > 0.5));
    assert!(points().any(|p| p.y < -1.5));
    assert_eq!(h.state().wb.undo_depth(), (2, 0));
}

#[test]
fn closing_rectangle_aborts_if_containment_exhausts_the_work_budget() {
    let mut h = harness("poly = PolyLine(Sequence((k/7000,0),k,1,7000))\nD = (1, 1)");
    h.state_mut().select_tool(Tool::CreateList);
    drag(&mut h, world(-0.5, 1.5), world(1.5, -0.5));
    assert!(h.state().wb.construction().id("l1").is_none());
    assert_eq!(h.state().wb.undo_depth(), (0, 0));
}

#[test]
fn closing_settled_canvas_skips_the_bounds_pass() {
    let mut h = harness(
        "L0 = Sequence((k, 0), k, 1, 256)\nL1 = Sequence((k, 1), k, 1, 256)\n\
         L2 = Sequence((k, 2), k, 1, 256)",
    );
    assert_eq!(h.state().camera.unwrap().cy, 0.5);
    let before = crate::canvas::BOUNDS_PASSES.with(std::cell::Cell::get);
    h.step();
    assert_eq!(
        crate::canvas::BOUNDS_PASSES.with(std::cell::Cell::get),
        before
    );
}

#[test]
fn closing_input_box_blurs_on_a_slider_drag_press() {
    let mut h = harness(
        "a = Slider(0, 8, 0.5)\na = 2\nb = 2 a\nfield = $inputBox(a, _)\n\
         $inputBoxStyle(_6669656c64, 124, 88, 12)",
    );
    click_canvas_input_box(&mut h, "field");
    assert_eq!(h.state().input_box_edit.as_ref().unwrap().text, "2");

    let from = at(ALGEBRA_WIDTH + SPLITTER + 80.0, TOOLBAR_HEIGHT + 50.0);
    let to = from + egui::vec2(75.0, 0.0);
    moved(&mut h, from);
    button(&mut h, from, true);
    assert!(h.state().input_box_edit.is_none());
    for i in 1..=4 {
        moved(&mut h, from.lerp(to, i as f32 / 4.0));
    }
    button(&mut h, to, false);
    click(&mut h, world(3.0, -2.0));
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(5.0));
    assert_eq!(c.value(c.id("b").unwrap()), Value::Number(10.0));
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
}

#[test]
fn closing_input_box_untouched_draft_keeps_a_new_link_value() {
    let mut h = harness(
        "a = Slider(0, 8, 0.5)\na = 2\nb = 2 a\nfield = $inputBox(a, _)\n\
         $inputBoxStyle(_6669656c64, 124, 88, 12)",
    );
    click_canvas_input_box(&mut h, "field");
    assert_eq!(h.state().input_box_edit.as_ref().unwrap().text, "2");
    let a = h.state().wb.construction().id("a").unwrap();
    assert!(h.state_mut().wb.set_number(a, 5.0));

    click(&mut h, world(3.0, -2.0));
    let c = h.state().wb.construction();
    assert_eq!(c.value(a), Value::Number(5.0));
    assert_eq!(c.value(c.id("b").unwrap()), Value::Number(10.0));
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
}

#[test]
fn last_round_input_box_edits_the_full_linked_text() {
    let original = format!("{}Z", "x".repeat(256));
    let mut h = harness(&format!(
        "note = \"{original}\"\nfield = $inputBox(note, _)"
    ));
    click_canvas_input_box(&mut h, "field");
    h.key_press(egui::Key::Home);
    h.step();
    h.key_press(egui::Key::Delete);
    h.step();
    h.key_press(egui::Key::Enter);
    h.run();
    let c = h.state().wb.construction();
    assert_eq!(c.text(c.id("note").unwrap()), Some(&original[1..]));
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
}

#[test]
fn last_round_input_box_refuses_an_oversized_value_without_truncating_it() {
    let original = format!("{}Z", "é".repeat(16_384));
    let mut h = harness(&format!(
        "note = \"{original}\"\nfield = $inputBox(note, _)"
    ));
    click_canvas_input_box(&mut h, "field");
    assert!(h.state().input_box_edit.is_none());
    let c = h.state().wb.construction();
    assert_eq!(c.text(c.id("note").unwrap()), Some(original.as_str()));
    assert_eq!(h.state().wb.undo_depth(), (0, 0));

    let fits = original.strip_prefix('é').unwrap();
    let mut h = harness(&format!("note = \"{fits}\"\nfield = $inputBox(note, _)"));
    click_canvas_input_box(&mut h, "field");
    h.key_press(egui::Key::Home);
    h.step();
    h.input_mut().events.push(Event::Text("Q".into()));
    h.step();
    assert_eq!(h.state().input_box_edit.as_ref().unwrap().text, fits);
    h.key_press(egui::Key::Enter);
    h.run();
    let c = h.state().wb.construction();
    assert_eq!(c.text(c.id("note").unwrap()), Some(fits));
}

#[test]
fn last_round_rectangle_charges_stretch_vector_parents() {
    let mut script = String::from("p = Polygon({(0,0),(1,0),(0,1)})\nv0 = Vector((1,0))");
    for k in 1..=70 {
        use std::fmt::Write as _;
        write!(script, "\nv{k} = UnitVector(v{})", k - 1).unwrap();
    }
    for copies in [3, 70] {
        let mut script = script.clone();
        for k in 1..=copies {
            use std::fmt::Write as _;
            write!(script, "\np{k} = Stretch(p, v70)").unwrap();
        }
        let mut h = harness(&script);
        h.state_mut().select_tool(Tool::CreateList);
        drag(&mut h, world(-0.5, 1.5), world(1.5, -0.5));
        assert_eq!(h.state().wb.construction().id("l1").is_some(), copies == 3);
        assert_eq!(h.state().wb.undo_depth(), (usize::from(copies == 3), 0));
    }
}

#[test]
fn last_round_input_box_undo_stays_with_its_worksheet() {
    let mut h = harness("");
    drop_saved_input_box(&mut h);
    h.run();
    click_canvas_input_box(&mut h, "field");
    h.input_mut().events.push(Event::Text("5.5".into()));
    h.step();
    assert_eq!(h.state().input_box_edit.as_ref().unwrap().text, "5.5");
    for _ in 0..80 {
        h.step();
    }
    drop_saved_input_box(&mut h);
    h.run();
    click_canvas_input_box(&mut h, "field");
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
    h.step();
    h.key_press(egui::Key::Tab);
    h.run();
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(3.5));
    assert_eq!(h.state().wb.undo_depth(), (0, 0));
}

#[test]
fn last_round_input_box_commits_before_the_linked_slider_click() {
    for same_frame in [false, true] {
        let mut h = harness(
            "a = Slider(0, 8, 0.5)\na = 2\nfield = $inputBox(a, _)\n\
             $sliderStyle(_61, true, 80, 60, 100, true, true, 5, 1)\n\
             $inputBoxStyle(_6669656c64, 124, 104, 12)",
        );
        click_canvas_input_box(&mut h, "field");
        h.input_mut().events.push(Event::Text("3.5".into()));
        if !same_frame {
            h.step();
        }
        let pos = at(ALGEBRA_WIDTH + SPLITTER + 178.0, TOOLBAR_HEIGHT + 60.0);
        h.input_mut().events.push(Event::PointerMoved(pos));
        for pressed in [true, false] {
            h.input_mut().events.push(Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        h.run();
        let c = h.state().wb.construction();
        assert_eq!(c.value(c.id("a").unwrap()), Value::Number(4.0));
        assert_eq!(h.state().wb.undo_depth(), (2, 0));
    }
}

#[test]
fn label_tool_toggles_hits_and_groups_changes_until_mode_exit() {
    let mut h = harness("A = (2, 1)\nB = (4, 1)");
    h.state_mut().select_tool(Tool::ShowHideLabel);
    click(&mut h, world(2.0, 1.0));
    click(&mut h, world(4.0, 1.0));
    let shown = |app: &App, label| {
        let c = app.wb.construction();
        c.drawing_style(c.id(label).unwrap()).show_label
    };
    assert!(!shown(h.state(), "A"));
    assert!(!shown(h.state(), "B"));
    assert_eq!(h.state().wb.undo_depth().0, 0);
    h.state_mut().select_tool(Tool::Move);
    assert!(h.state_mut().undo());
    assert!(shown(h.state(), "A"));
    assert!(shown(h.state(), "B"));
}

#[test]
fn copy_style_uses_the_same_source_until_it_is_clicked_again() {
    let mut h = harness("A = (2, 1)\nB = (4, 1)\nC = (6, 1)\n$drawingStyle(_41, 2, false, 7)");
    h.state_mut().select_tool(Tool::CopyVisualStyle);
    for x in [2.0, 4.0, 6.0, 2.0] {
        click(&mut h, world(x, 1.0));
    }
    let style = |app: &App, label| {
        let c = app.wb.construction();
        c.drawing_style(c.id(label).unwrap())
    };
    let source = style(h.state(), "A");
    for label in ["B", "C"] {
        let target = style(h.state(), label);
        assert_eq!(
            source,
            geo_kernel::kernel::DrawingStyle {
                color: None,
                label_edited: false,
                ..target
            }
        );
        assert!(target.color.is_some());
    }
    assert!(h.state_mut().undo());
    assert_ne!(style(h.state(), "A"), style(h.state(), "B"));
    assert_ne!(style(h.state(), "A"), style(h.state(), "C"));
}

#[test]
fn move_rotate_picks_existing_center_and_rotates_in_place_as_one_step() {
    let mut h = harness("A = (2, 1)\nB = (4, 1)");
    h.state_mut().select_tool(Tool::MoveRotate);
    click(&mut h, world(2.0, 1.0));
    button(&mut h, world(4.0, 1.0), true);
    moved(&mut h, world(3.0, 2.0));
    moved(&mut h, world(2.0, 3.0));
    button(&mut h, world(2.0, 3.0), false);
    let at = |app: &App| {
        let c = app.wb.construction();
        c.value(c.id("B").unwrap())
    };
    let Value::Point { x, y } = at(h.state()) else {
        panic!("point disappeared")
    };
    assert!((x - 2.0).abs() < 1e-9 && (y - 3.0).abs() < 1e-9);
    assert_eq!(h.state().wb.undo_depth().0, 1);
    assert!(h.state_mut().undo());
    assert_eq!(at(h.state()), Value::Point { x: 4.0, y: 1.0 });
}

#[test]
fn function_inspector_selects_a_graph_and_returns_to_move_without_undo() {
    let mut h = harness("f(x) = x^2");
    h.state_mut().select_tool(Tool::FunctionInspector);
    click(&mut h, world(1.0, 1.0));
    let app = h.state();
    assert_eq!(app.tool.tool(), Tool::Move);
    let inspector = app.inspector.as_ref().unwrap();
    assert_eq!(inspector.label, "f");
    assert_eq!(
        inspector.rows(&app.wb, 0.0, 2.0, 1.0),
        vec![(0.0, 0.0), (1.0, 1.0), (2.0, 4.0)]
    );
    assert!(inspector.rows(&app.wb, 0.0, 1e100, 0.001).len() <= 256);
    assert_eq!(app.wb.undo_depth().0, 0);
}

#[test]
fn image_tool_opens_placement_request_and_inserts_one_undo_step() {
    let mut h = harness("A = (2, 1)");
    h.state_mut().select_tool(Tool::Image);
    click(&mut h, world(2.0, 1.0));
    let app = h.state_mut();
    assert!(app.image_request.is_some());
    use image::ImageEncoder as _;
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&[0, 0, 0, 255], 1, 1, image::ExtendedColorType::Rgba8)
        .unwrap();
    app.finish_image(png).unwrap();
    let c = app.wb.construction();
    let (id, _) = c.images().next().unwrap();
    let quad = c.image_quad(id, 50.0).unwrap();
    assert_eq!(quad[3], [2.0, 1.0]);
    assert!(app.undo());
    assert_eq!(app.wb.construction().images().count(), 0);
    assert!(app.redo());
    assert_eq!(app.wb.construction().images().count(), 1);
}

#[test]
fn freehand_shape_recognizes_the_reference_diagonal_as_a_segment() {
    let mut h = harness("");
    h.state_mut().select_tool(Tool::FreehandShape);
    button(&mut h, at(500.0, 400.0), true);
    moved(&mut h, at(505.0, 405.0));
    moved(&mut h, at(510.0, 410.0));
    button(&mut h, at(510.0, 410.0), false);
    assert!(h
        .state()
        .wb
        .object_ids()
        .any(|id| h.state().wb.construction().kind(id) == geo_kernel::Kind::Segment));
    assert!(h.state_mut().undo());
    assert!(labels(&h).is_empty());
}

#[test]
fn freehand_function_interpolates_the_reference_diagonal_as_one_step() {
    let mut h = harness("");
    h.state_mut().select_tool(Tool::FreehandFunction);
    button(&mut h, world(1.0, 1.0), true);
    moved(&mut h, world(2.0, 2.0));
    moved(&mut h, world(3.0, 3.0));
    button(&mut h, world(3.0, 3.0), false);
    let c = h.state().wb.construction();
    let f = c.id("f").unwrap();
    assert_eq!(c.function_value(f, 2.5), Some(2.5));
    assert!(h.state_mut().undo());
    assert!(labels(&h).is_empty());
}

#[test]
fn round_one_imported_line_thickness_uses_the_reference_baseline() {
    use crate::draw::{line_width, CURVE_WIDTH};

    let mut wb = Workbench::default();
    wb.load_bytes(
        br#"<geogebra><construction>
          <expression label="l" exp="y = 1"/>
          <element type="line" label="l"><coords x="0" y="1" z="-1"/>
            <lineStyle thickness="5" type="0"/></element>
          <expression label="m" exp="y = x - 1"/>
          <element type="line" label="m"><coords x="-1" y="1" z="1"/>
            <lineStyle thickness="10" type="0"/></element>
        </construction></geogebra>"#,
        "line widths",
    )
    .unwrap();
    let widths = |wb: &Workbench| {
        let c = wb.construction();
        ["l", "m"].map(|label| line_width(c, c.id(label).unwrap(), CURVE_WIDTH))
    };
    assert_eq!(widths(&wb), [CURVE_WIDTH, 2.0 * CURVE_WIDTH]);
    let mut restored = Workbench::default();
    assert!(restored
        .restore_text(&wb.snapshot_text())
        .unwrap()
        .is_empty());
    assert_eq!(widths(&restored), widths(&wb));
    let native = Workbench::from_script("l: y = 1\nm: y = x - 1").unwrap();
    assert_eq!(widths(&native), [CURVE_WIDTH; 2]);
    let archive = native.export_ggb(None).unwrap();
    let mut round_trip = Workbench::default();
    round_trip.load_bytes(&archive, "native export").unwrap();
    assert_eq!(widths(&round_trip), widths(&native));
}
