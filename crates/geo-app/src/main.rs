//! `geokernel-app`: a desktop window where free points can be dragged.

mod draw;
mod interact;

use eframe::egui;
use geo_kernel::kernel::Construction;
use interact::{Drag, ViewTransform};

const SCRIPT: &str = include_str!("../../../examples/triangle.geo");

struct App {
    construction: Construction,
    /// Fitted once, on the first frame, and then kept so the picture does not
    /// jump while dragging. Only the pixel size of the area may change.
    view: Option<geo_kernel::svg::View>,
    drag: Option<Drag>,
    /// Objects recomputed by the last drag step.
    recomputed: usize,
}

impl App {
    fn new(construction: Construction) -> Self {
        App {
            construction,
            view: None,
            drag: None,
            recomputed: 0,
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let size = ui.available_size();
                let (response, painter) = ui.allocate_painter(size, egui::Sense::drag());
                let rect = response.rect;
                let (w, h) = (rect.width() as f64, rect.height() as f64);
                let view = *self
                    .view
                    .get_or_insert_with(|| ViewTransform::fit(&self.construction, w, h).view());
                let t = ViewTransform::new(view, w, h);
                let to_local = |pos: egui::Pos2| {
                    let d = pos - rect.min;
                    (d.x as f64, d.y as f64)
                };
                let radius = (draw::FREE_POINT_RADIUS + 5.0) as f64;

                if response.drag_started() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let (sx, sy) = to_local(pos);
                        self.drag = Drag::begin(&self.construction, &t, sx, sy, radius);
                    }
                }
                if response.dragged() {
                    if let (Some(drag), Some(pos)) = (self.drag, response.interact_pointer_pos()) {
                        let (sx, sy) = to_local(pos);
                        if let Ok(n) = drag.apply(&mut self.construction, &t, sx, sy) {
                            self.recomputed = n;
                        }
                    }
                }
                if response.drag_stopped() {
                    self.drag = None;
                }
                if self.drag.is_some() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                } else if let Some(pos) = response.hover_pos() {
                    let (sx, sy) = to_local(pos);
                    if interact::hit_test(&self.construction, &t, sx, sy, radius).is_some() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    }
                }

                draw::paint(&painter, rect, &self.construction, &t);
                painter.text(
                    rect.left_bottom() + egui::vec2(8.0, -8.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!(
                        "{} objects, last drag recomputed {}",
                        self.construction.len(),
                        self.recomputed
                    ),
                    egui::FontId::proportional(12.0),
                    egui::Color32::from_gray(0x70),
                );
            });
    }
}

fn main() -> eframe::Result {
    let construction = match geo_kernel::script::load(SCRIPT) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cannot load the built-in script: {e:?}");
            std::process::exit(1);
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("geokernel")
            .with_inner_size([900.0, 650.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "geokernel",
        options,
        Box::new(|_cc| Ok(Box::new(App::new(construction)))),
    )
}
