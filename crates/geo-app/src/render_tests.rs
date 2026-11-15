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
    let mut app = App::new(Workbench::from_script(script).unwrap(), None);
    prepare(&mut app);
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(
            1400.0 + 2.0 * MARGIN,
            900.0 + 2.0 * MARGIN,
        ))
        .with_pixels_per_point(1.0)
        .with_context_setup(crate::theme::apply)
        .wgpu()
        .build_ui(move |ui| app.show(ui));
    harness.run();
    let image = harness.render().expect("render");
    let path = out_dir().join(name);
    let m = MARGIN as u32;
    let cropped = image::imageops::crop_imm(&image, m, m, 1400, 900).to_image();
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
fn render_tools_panel() {
    let path = render(
        include_str!("../../../examples/four-lines.geo"),
        "tools.png",
        |app| app.mode = crate::app::Mode::Tools,
    );
    println!("{}", path.display());
}
