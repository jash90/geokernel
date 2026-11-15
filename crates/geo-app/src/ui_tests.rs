//! Headless interaction tests: synthetic pointer events through the real UI.

use crate::app::{App, Mode};
use crate::tools::Tool;
use crate::workbench::Workbench;
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use geo_kernel::kernel::Value;

/// The harness puts this margin around the root ui.
const MARGIN: f32 = 8.0;
const SIZE: (f32, f32) = (1400.0, 900.0);

fn harness(script: &str) -> Harness<'static, App> {
    let app = App::new(Workbench::from_script(script).unwrap(), None);
    let mut h = Harness::builder()
        .with_size(egui::vec2(SIZE.0 + 2.0 * MARGIN, SIZE.1 + 2.0 * MARGIN))
        .with_pixels_per_point(1.0)
        .build_ui_state(|ui, app: &mut App| app.show(ui), app);
    h.run();
    h
}

/// Window coordinates (as in a 1400 x 900 window) to harness coordinates.
fn at(x: f32, y: f32) -> Pos2 {
    Pos2::new(x + MARGIN, y + MARGIN)
}

/// Window position of the world point `(x, y)` in the standard view.
fn world(x: f64, y: f64) -> Pos2 {
    use crate::theme::{HEADER_HEIGHT, RAIL_WIDTH, SIDE_WIDTH};
    let left = RAIL_WIDTH + SIDE_WIDTH;
    let (w, h) = (SIZE.0 - left, SIZE.1 - HEADER_HEIGHT);
    at(
        left + w / 2.0 + (x * 50.0) as f32,
        HEADER_HEIGHT + h / 2.0 - (y * 50.0) as f32,
    )
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
fn line_tool_creates_points_and_a_line_in_one_undo_step() {
    let mut h = harness("A = (-5.64, 4.44)\n");
    h.state_mut().tool.set_tool(Tool::Line);
    click(&mut h, world(1.0, 1.0));
    click(&mut h, world(3.0, -2.0));
    assert_eq!(labels(&h), ["A", "B", "C", "f"]);
    let app = h.state_mut();
    assert!(app.wb.undo());
    assert_eq!(app.wb.construction().len(), 1);
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

#[test]
fn the_marker_toggles_visibility_and_clicking_the_body_selects() {
    let mut h = harness("A = (0, 0)\nB = (2, 0)\n");
    // First row: marker cell is 57 px wide, rows start under the 65 px header.
    let marker = at(72.0 + 28.0, 65.0 + 24.0);
    click(&mut h, marker);
    let a = h.state().wb.construction().id("A").unwrap();
    assert!(!h.state().wb.is_visible(a));
    click(&mut h, marker);
    assert!(h.state().wb.is_visible(a));
    // The text body of the second row selects B.
    click(&mut h, at(72.0 + 120.0, 65.0 + 48.0 + 24.0));
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

#[test]
fn the_rail_switches_between_the_object_list_and_the_tools() {
    let mut h = harness("A = (0, 0)\n");
    assert_eq!(h.state().mode, Mode::Algebra);
    click(&mut h, at(36.0, 10.0 + 65.0 + 74.0 + 30.0));
    assert_eq!(h.state().mode, Mode::Tools);
    // The first tile is "move", the second "point".
    click(&mut h, at(72.0 + 12.0 + 150.0, 65.0 + 12.0 + 45.0));
    assert_eq!(h.state().tool.tool(), Tool::Point);
    click(&mut h, world(3.0, 3.0));
    assert_eq!(labels(&h), ["A", "B"]);
}
