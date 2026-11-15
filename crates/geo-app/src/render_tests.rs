//! Off-screen rendering of the window to a PNG, to look at the result.
//!
//! These tests need a GPU adapter, so they are ignored by default:
//! `cargo test -p geo-app render -- --ignored`. The images go to
//! `target/render/` and are not committed.

use crate::app::App;
use crate::workbench::Workbench;
use egui_kittest::Harness;
use std::path::PathBuf;

/// The harness puts a fixed margin around the root ui; the real window has none.
const MARGIN: f32 = 8.0;

fn out_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/render");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn render(script: &str, name: &str, prepare: impl FnOnce(&mut App)) -> PathBuf {
    render_sized(script, name, (1400, 900), prepare)
}

fn render_sized(
    script: &str,
    name: &str,
    (w, h): (u32, u32),
    prepare: impl FnOnce(&mut App),
) -> PathBuf {
    let mut app = App::new(Workbench::from_script(script).unwrap(), None);
    prepare(&mut app);
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(
            w as f32 + 2.0 * MARGIN,
            h as f32 + 2.0 * MARGIN,
        ))
        .with_pixels_per_point(1.0)
        .with_context_setup(crate::theme::apply)
        .wgpu()
        .build_ui(move |ui| app.show(ui));
    harness.run();
    let image = harness.render().expect("render");
    let path = out_dir().join(name);
    let m = MARGIN as u32;
    let cropped = image::imageops::crop_imm(&image, m, m, w, h).to_image();
    cropped.save(&path).unwrap();
    path
}

#[test]
#[ignore = "needs a GPU adapter"]
fn render_acceptance_scene() {
    let path = render(
        include_str!("../../../examples/four-lines.geo"),
        "four-lines.png",
        |_| {},
    );
    println!("{}", path.display());
}

#[test]
#[ignore = "needs a GPU adapter"]
fn render_tool_drop_down_and_keyboard() {
    let path = render(
        include_str!("../../../examples/four-lines.geo"),
        "keyboard.png",
        |app| {
            app.keyboard.open = true;
            app.style_bar = true;
            app.select_tool(crate::tools::Tool::Circle);
        },
    );
    println!("{}", path.display());
}

#[test]
#[ignore = "needs a GPU adapter"]
fn render_empty_reference_size() {
    let path = render_sized("", "empty.png", (1728, 1050), |app| {
        app.keyboard.open = true
    });
    println!("{}", path.display());
}

#[test]
#[ignore = "needs a GPU adapter"]
fn render_quadrilateral_reference_size() {
    // The four sides stand in for a polygon until the kernel has polygons.
    let path = render_sized(
        "A = (-8.82, 7.97)\nB = (-3.5, 2.2)\nC = (2.4, 4.1)\nD = (-1.6, 9.3)\n\
         a = Segment(A, B)\nb = Segment(B, C)\nc = Segment(C, D)\nd = Segment(D, A)\n\
         α = Angle(B, A, D)\n",
        "quadrilateral.png",
        (1728, 1050),
        |_| {},
    );
    println!("{}", path.display());
}

#[test]
#[ignore = "needs a GPU adapter"]
fn render_interaction_states() {
    let path = render(
        include_str!("../../../examples/four-lines.geo"),
        "states.png",
        |app| {
            app.wb.selected = app.wb.construction().id("g");
            let b = app.wb.construction().id("B").unwrap();
            app.wb.toggle_visible(b);
            app.input = "Line(A, Q)".into();
            app.input_error = Some("Nieznany obiekt `Q`".into());
            app.tool.set_tool(crate::tools::Tool::Line);
            app.tool_error = Some("Tych obiektów nie można przeciąć".into());
        },
    );
    println!("{}", path.display());
}

#[test]
#[ignore = "needs a GPU adapter"]
fn render_mixed_objects() {
    let path = render(
        "A = (-3, 1)\nB = (2, 2)\nr = 2.5\nc = Circle(A, r)\ns = Segment(A, B)\n\
         M = Midpoint(A, B)\np = Perpendicular(M, s)\nR = Ray(B, A)\nv = (1, -2)\n\
         k = Circle(B, A)\nP = Intersect(p, k)\n",
        "mixed.png",
        |app| app.wb.selected = app.wb.construction().id("c"),
    );
    println!("{}", path.display());
}

#[test]
#[ignore = "needs a GPU adapter"]
fn render_axes_out_of_view() {
    let path = render(
        include_str!("../../../examples/four-lines.geo"),
        "far.png",
        |app| {
            app.view_request = None;
            app.camera = Some(crate::view::Camera {
                cx: 60.0,
                cy: 40.0,
                scale: 25.0,
            });
        },
    );
    println!("{}", path.display());
}
