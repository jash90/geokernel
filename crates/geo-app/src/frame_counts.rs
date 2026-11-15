//! Counts of what one frame of the window produces: shapes, and vertices and
//! indices after tessellation, plus whether the frame asked for an immediate
//! repaint. The numbers are pinned, so that a change of the code that adds
//! drawing work per frame shows up as a failing test. A deliberate change of
//! the drawing updates them.

use crate::app::App;
use crate::workbench::Workbench;
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;

const MARGIN: f32 = 8.0;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct Counts {
    shapes: usize,
    vertices: usize,
    indices: usize,
    /// Frames out of the measured ones that asked to be repainted at once.
    immediate_repaints: usize,
}

fn harness(script: &str) -> Harness<'static, App> {
    let app = App::new(Workbench::from_script(script).unwrap(), None);
    let mut h = Harness::builder()
        .with_size(egui::vec2(1400.0 + 2.0 * MARGIN, 900.0 + 2.0 * MARGIN))
        .with_pixels_per_point(1.0)
        .with_context_setup(crate::theme::apply)
        .build_ui_state(|ui, app: &mut App| crate::app::show(app, ui), app);
    h.run();
    h
}

/// Runs `frames` frames without input and sums what the last one produced;
/// the repaint count is over all of them.
fn measure(h: &mut Harness<'_, App>, frames: usize) -> Counts {
    let mut immediate_repaints = 0;
    for _ in 0..frames {
        h.step();
        let out = h.output();
        if out
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .is_some_and(|v| v.repaint_delay.is_zero())
        {
            immediate_repaints += 1;
        }
    }
    let out = h.output();
    let primitives = h.ctx.tessellate(out.shapes.clone(), out.pixels_per_point);
    let (mut vertices, mut indices) = (0, 0);
    for p in &primitives {
        if let egui::epaint::Primitive::Mesh(mesh) = &p.primitive {
            vertices += mesh.vertices.len();
            indices += mesh.indices.len();
        }
    }
    Counts {
        shapes: out.shapes.len(),
        vertices,
        indices,
        immediate_repaints,
    }
}

fn pointer(h: &mut Harness<'_, App>, event: Event) {
    h.input_mut().events.push(event);
    h.step();
}

fn press(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// Window position of the world point `(x, y)` in the triangle example's
/// initial view; read from the app so it does not depend on the camera.
fn screen(h: &Harness<'_, App>, x: f64, y: f64) -> Pos2 {
    let (rect, t) = h.state().view.unwrap();
    let (sx, sy) = t.world_to_screen(x, y);
    Pos2::new(rect.min.x + sx as f32, rect.min.y + sy as f32)
}

const EXAMPLE: &str = include_str!("../../../examples/triangle.geo");

fn scenes() -> Vec<(&'static str, Counts)> {
    let mut out = Vec::new();

    let mut h = harness(EXAMPLE);
    out.push(("idle", measure(&mut h, 5)));

    // The pointer over a point: hover highlight.
    let a = screen(&h, 0.0, 0.0);
    pointer(&mut h, Event::PointerMoved(a));
    out.push(("hover", measure(&mut h, 5)));

    // A drag of a free point, measured while the button is held.
    pointer(&mut h, press(a, true));
    for i in 1..=5 {
        pointer(
            &mut h,
            Event::PointerMoved(a + egui::vec2(8.0 * i as f32, 0.0)),
        );
    }
    out.push(("drag", measure(&mut h, 3)));
    pointer(&mut h, press(a + egui::vec2(40.0, 0.0), false));

    // A tool with one pick and the pointer elsewhere: the preview.
    let mut h = harness(EXAMPLE);
    h.state_mut().tool.set_tool(crate::tools::Tool::Segment);
    let b = screen(&h, 6.0, 0.0);
    pointer(&mut h, Event::PointerMoved(b));
    pointer(&mut h, press(b, true));
    pointer(&mut h, press(b, false));
    pointer(&mut h, Event::PointerMoved(b + egui::vec2(-60.0, 80.0)));
    out.push(("segment preview", measure(&mut h, 5)));

    // The on-screen keyboard open.
    let mut h = harness(EXAMPLE);
    h.state_mut().keyboard.open = true;
    out.push(("keyboard", measure(&mut h, 5)));

    out
}

#[test]
fn frame_counts_are_unchanged() {
    let got = scenes();
    for (name, counts) in &got {
        println!("{name}: {counts:?}");
    }
    let expected: &[(&str, Counts)] = &[
        (
            "idle",
            Counts {
                shapes: 578,
                vertices: 12790,
                indices: 52767,
                immediate_repaints: 0,
            },
        ),
        (
            "hover",
            Counts {
                shapes: 580,
                vertices: 12930,
                indices: 53484,
                immediate_repaints: 1,
            },
        ),
        (
            "drag",
            Counts {
                shapes: 581,
                vertices: 13110,
                indices: 54243,
                immediate_repaints: 1,
            },
        ),
        (
            "segment preview",
            Counts {
                shapes: 582,
                vertices: 12998,
                indices: 53307,
                immediate_repaints: 1,
            },
        ),
        (
            "keyboard",
            Counts {
                shapes: 629,
                vertices: 14914,
                indices: 62493,
                immediate_repaints: 0,
            },
        ),
    ];
    assert_eq!(got.as_slice(), expected);
}
