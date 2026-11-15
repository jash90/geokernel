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
        .build_ui(move |ui| crate::app::show(&mut app, ui));
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
    // The scene of the reference snapshot: a quadrilateral with its sides.
    let path = render_sized(
        "A = (-8.82, 7.97)\nB = (-4.56, 6.63)\nC = (-10.46, 1.45)\nD = (-12.16, 4.93)\n\
         q1 = Polygon(A, B, C, D)\n",
        "quadrilateral.png",
        (1920, 1050),
        |app| app.wb.selected = app.wb.construction().id("B"),
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

#[test]
#[ignore = "needs a GPU adapter"]
fn render_touch_portrait() {
    let path = render_sized(
        include_str!("../../../examples/triangle.geo"),
        "touch-portrait.png",
        (400, 800),
        |app| app.set_metrics(geo_app_core::metrics::Metrics::touch()),
    );
    println!("{}", path.display());
}

/// Rendered on the GPU, `tan(x)` and `1 / (x - 2)` leave dark curve pixels
/// next to sampled points of their graphs and none along their poles (away
/// from the x axis and its numbers, which are dark as well).
#[test]
#[ignore = "needs a GPU adapter"]
fn render_function_graphs_and_their_poles() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let mut app = App::new(
        Workbench::from_script("f(x) = tan(x)\ng(x) = 1 / (x - 2)\n").unwrap(),
        None,
    );
    app.view_request = None;
    app.camera = Some(crate::view::Camera::STANDARD);
    let app = Rc::new(RefCell::new(app));
    let shown = app.clone();
    let (w, h) = (1400u32, 900u32);
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(
            w as f32 + 2.0 * MARGIN,
            h as f32 + 2.0 * MARGIN,
        ))
        .with_pixels_per_point(1.0)
        .with_context_setup(crate::theme::apply)
        .wgpu()
        .build_ui(move |ui| crate::app::show(&mut shown.borrow_mut(), ui));
    harness.run();
    let image = harness.render().expect("render");
    image.save(out_dir().join("function-graphs.png")).unwrap();
    // The harness's coordinates, margin included, are the image's.
    let (rect, t) = app.borrow().view.expect("the canvas was drawn");
    let px = |x: f64, y: f64| {
        let (sx, sy) = t.world_to_screen(x, y);
        (rect.min.x as f64 + sx, rect.min.y as f64 + sy)
    };
    let dark = |x: i64, y: i64| {
        if x < 0 || y < 0 || x >= image.width() as i64 || y >= image.height() as i64 {
            return false;
        }
        let p = image.get_pixel(x as u32, y as u32).0;
        p[0] < 120 && p[1] < 120 && p[2] < 120
    };
    let inside = |(sx, sy): (f64, f64)| {
        sx > rect.min.x as f64 + 30.0
            && sx < rect.max.x as f64 - 30.0
            && sy > rect.min.y as f64 + 30.0
            && sy < rect.max.y as f64 - 30.0
    };
    let (_, axis_y) = px(0.0, 0.0);
    let mut checked = 0;
    let graphs = [f64::tan as fn(f64) -> f64, |x| 1.0 / (x - 2.0)];
    for f in graphs {
        for i in 0..4000 {
            let x = -12.0 + 24.0 * i as f64 / 4000.0;
            let (y, slope) = (f(x), (f(x + 1e-4) - f(x - 1e-4)) / 2e-4);
            let p = px(x, y);
            if !y.is_finite() || slope.abs() > 5.0 || !inside(p) {
                continue;
            }
            let (cx, cy) = (p.0.round() as i64, p.1.round() as i64);
            let near = (-2..=2).any(|dx| (-2..=2).any(|dy| dark(cx + dx, cy + dy)));
            assert!(near, "no curve pixel near ({x}, {y})");
            checked += 1;
        }
    }
    assert!(checked > 1000, "{checked} points checked");
    for pole in [
        2.0,
        -std::f64::consts::FRAC_PI_2,
        std::f64::consts::FRAC_PI_2,
        3.0 * std::f64::consts::FRAC_PI_2,
    ] {
        let (sx, _) = px(pole, 0.0);
        let sx = sx.round() as i64;
        let (top, bottom) = (rect.min.y as i64 + 40, rect.max.y as i64);
        for y in top..bottom {
            if (y as f64 - axis_y).abs() < 20.0 {
                continue;
            }
            // Another branch may cross the pole's column; only a join is wrong.
            let crossed = graphs.iter().any(|f| {
                (-24..=24).any(|k| {
                    let x = pole + k as f64 / (4.0 * t.scale());
                    let (cx, cy) = px(x, f(x));
                    (cx - sx as f64).abs() < 6.0 && (cy - y as f64).abs() < 4.0
                })
            });
            if crossed {
                continue;
            }
            assert!(
                !dark(sx, y),
                "a curve pixel on the pole x = {pole} at row {y}"
            );
        }
    }
}
