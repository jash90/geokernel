//! `geokernel-app`: a desktop workbench for the construction kernel.
//!
//! Logic lives in plain modules (`view`, `interact`, `workbench`, `history`,
//! `format`, `highlight`, `cli`); this file only wires egui events to them.

mod cli;
mod draw;
mod format;
mod highlight;
mod history;
mod interact;
mod view;
mod workbench;

use eframe::egui::{self, text::CCursor, text::CCursorRange};
use geo_kernel::kernel::{Construction, Id};
use highlight::Highlight;
use interact::Drag;
use std::path::Path;
use view::Camera;
use workbench::Workbench;

const SCRIPT: &str = include_str!("../../../examples/triangle.geo");
const HIGHLIGHT: egui::Color32 = egui::Color32::from_rgb(0xf5, 0x7c, 0x00);
const ERROR: egui::Color32 = egui::Color32::from_rgb(0xd9, 0x30, 0x25);

struct App {
    wb: Workbench,
    /// `None` until the first frame, when the size of the area is known.
    camera: Option<Camera>,
    fit_requested: bool,
    drag: Option<Drag>,
    panning: bool,
    input: String,
    input_error: Option<String>,
    load_error: Option<String>,
    /// Status bar text and the version it was built for.
    status: String,
    status_version: Option<u64>,
}

impl App {
    fn new(construction: Construction, load_error: Option<String>) -> Self {
        App {
            wb: Workbench::new(construction),
            camera: None,
            fit_requested: true,
            drag: None,
            panning: false,
            input: String::new(),
            input_error: None,
            load_error,
            status: String::new(),
            status_version: None,
        }
    }

    fn open(&mut self, path: &Path) {
        match cli::load_file(path) {
            Ok(c) => {
                self.wb.replace(c);
                self.load_error = None;
                self.drag = None;
                self.fit_requested = true;
            }
            Err(e) => self.load_error = Some(e),
        }
    }

    fn refresh_status(&mut self) {
        if self.status_version == Some(self.wb.version()) {
            return;
        }
        let s = self.wb.stats;
        self.status = format!(
            "{} objects | last change recomputed {} | took {}",
            self.wb.construction().len(),
            s.recomputed,
            format::duration(s.elapsed)
        );
        self.status_version = Some(self.wb.version());
    }

    fn input_bar(&mut self, ui: &mut egui::Ui) {
        let id = egui::Id::unique("input_line");
        ui.horizontal(|ui| {
            ui.label(">");
            let mut text_changed_by_history = false;
            if ui.memory(|m| m.has_focus(id)) {
                let (up, down) = ui.input_mut(|i| {
                    (
                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                    )
                });
                let recalled = if up {
                    self.wb.history.up(&self.input)
                } else if down {
                    self.wb.history.down()
                } else {
                    None
                };
                if let Some(t) = recalled {
                    self.input = t.to_owned();
                    text_changed_by_history = true;
                }
            }
            let width = if self.input_error.is_some() {
                ui.available_width() * 0.55
            } else {
                ui.available_width()
            };
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.input)
                    .id(id)
                    .hint_text("name = Definition(...), for example M = Midpoint(A, B)")
                    .desired_width(width),
            );
            if text_changed_by_history {
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
                    let end = CCursor::new(self.input.chars().count());
                    state.cursor.set_char_range(Some(CCursorRange::one(end)));
                    egui::TextEdit::store_state(ui.ctx(), id, state);
                }
            } else if response.changed() {
                self.wb.history.stop_browsing();
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                match self.wb.run_line(&self.input) {
                    Ok(()) => {
                        self.input.clear();
                        self.input_error = None;
                    }
                    Err(e) => self.input_error = Some(e),
                }
                response.request_focus();
            }
            if let Some(e) = &self.input_error {
                ui.colored_label(ERROR, e.as_str());
            }
        });
    }

    fn object_list(&mut self, ui: &mut egui::Ui) {
        let mut fit = false;
        ui.horizontal(|ui| {
            ui.strong("Objects");
            if ui.button("Fit (F)").clicked() {
                fit = true;
            }
        });
        ui.separator();
        let selected = self.wb.selected;
        let mut new_selection: Option<Option<Id>> = None;
        let mut edit: Option<(Id, f64)> = None;
        let rows = self.wb.rows();
        let row_height = ui.spacing().interact_size.y;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, row_height, rows.len(), |ui, range| {
                for row in &rows[range] {
                    ui.horizontal(|ui| {
                        let is_selected = selected == Some(row.id);
                        let mut clicked = ui
                            .selectable_label(is_selected, row.label.as_str())
                            .clicked();
                        clicked |= ui
                            .add(egui::Label::new(row.kind.as_str()).sense(egui::Sense::click()))
                            .clicked();
                        if let Some(n) = row.free_number {
                            let mut v = n;
                            let speed = (n.abs() * 0.01).max(0.01);
                            let r =
                                ui.add(egui::DragValue::new(&mut v).speed(speed).max_decimals(4));
                            if r.changed() {
                                edit = Some((row.id, v));
                            }
                        } else {
                            clicked |= ui
                                .add(
                                    egui::Label::new(row.value.as_str())
                                        .sense(egui::Sense::click()),
                                )
                                .clicked();
                        }
                        if clicked {
                            new_selection = Some(if is_selected { None } else { Some(row.id) });
                        }
                    });
                }
            });
        if let Some(s) = new_selection {
            self.wb.selected = s;
        }
        if let Some((id, v)) = edit {
            self.wb.set_number(id, v);
        }
        if fit {
            self.fit_requested = true;
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let size = ui.available_size();
        let (response, painter) = ui.allocate_painter(size, egui::Sense::drag());
        let rect = response.rect;
        let (w, h) = (rect.width() as f64, rect.height() as f64);
        if w < 1.0 || h < 1.0 {
            return;
        }
        if self.fit_requested || self.camera.is_none() {
            let bounds = view::content_bounds(self.wb.construction());
            self.camera = Some(Camera::fit(bounds, w, h));
            self.fit_requested = false;
        }
        let mut cam = self.camera.unwrap_or(Camera {
            cx: 0.0,
            cy: 0.0,
            scale: 50.0,
        });
        let to_local = |pos: egui::Pos2| {
            let d = pos - rect.min;
            (d.x as f64, d.y as f64)
        };

        if response.hovered() {
            let (scroll, pinch) =
                ui.input(|i| (i.smooth_scroll_delta().y as f64, i.zoom_delta() as f64));
            let factor = view::zoom_factor(scroll, pinch);
            if factor != 1.0 {
                let (sx, sy) = response.hover_pos().map_or((0.5 * w, 0.5 * h), to_local);
                cam.zoom_at(factor, sx, sy, w, h);
            }
        }

        let radius = (draw::FREE_POINT_RADIUS + 5.0) as f64;
        let t = cam.transform(w, h);
        if response.drag_started() {
            self.drag = None;
            self.panning = false;
            if response.dragged_by(egui::PointerButton::Primary) {
                if let Some(pos) = response.interact_pointer_pos() {
                    let (sx, sy) = to_local(pos);
                    self.drag = Drag::begin(self.wb.construction(), &t, sx, sy, radius);
                }
            }
            self.panning = self.drag.is_none();
        }
        if response.dragged() {
            if let Some(drag) = self.drag {
                if let Some(pos) = response.interact_pointer_pos() {
                    let (sx, sy) = to_local(pos);
                    let (x, y) = drag.target(&t, sx, sy);
                    self.wb.move_point(drag.id(), x, y);
                }
            } else if self.panning {
                let d = response.drag_delta();
                cam.pan_pixels(d.x as f64, d.y as f64);
            }
        }
        if response.drag_stopped() {
            self.drag = None;
            self.panning = false;
        }
        if self.drag.is_some() || self.panning {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if let Some(pos) = response.hover_pos() {
            let (sx, sy) = to_local(pos);
            if interact::hit_test(self.wb.construction(), &t, sx, sy, radius).is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
        }

        self.camera = Some(cam);
        let t = cam.transform(w, h);
        draw::paint(&painter, rect, self.wb.construction(), &t);
        if let Some(id) = self.wb.selected {
            let value = self.wb.construction().value(id);
            if let Some(hl) = highlight::of_value(&value, &t, w, h) {
                paint_highlight(&painter, rect, hl);
            }
        }
    }
}

fn paint_highlight(painter: &egui::Painter, rect: egui::Rect, hl: Highlight) {
    let p = |x: f64, y: f64| rect.min + egui::vec2(x as f32, y as f32);
    let glow = egui::Stroke::new(6.0, HIGHLIGHT.gamma_multiply(0.35));
    let core = egui::Stroke::new(2.5, HIGHLIGHT);
    match hl {
        Highlight::Point { x, y } => {
            painter.circle_stroke(p(x, y), 9.0, core);
            painter.circle_stroke(p(x, y), 12.0, glow);
        }
        Highlight::Stroke { x1, y1, x2, y2 } => {
            painter.line_segment([p(x1, y1), p(x2, y2)], glow);
            painter.line_segment([p(x1, y1), p(x2, y2)], core);
        }
        Highlight::Circle { x, y, r } => {
            painter.circle_stroke(p(x, y), r as f32, glow);
            painter.circle_stroke(p(x, y), r as f32, core);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(path) = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_owned()))
        {
            self.open(&path);
        }
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::F)) {
            self.fit_requested = true;
        }

        self.refresh_status();
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(self.status.as_str());
        });
        egui::Panel::bottom("input").show(ui, |ui| self.input_bar(ui));
        let mut dismiss = false;
        if let Some(e) = &self.load_error {
            egui::Panel::top("load_error").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(ERROR, format!("Cannot load script: {e}"));
                    dismiss = ui.button("Dismiss").clicked();
                });
            });
        }
        if dismiss {
            self.load_error = None;
        }
        egui::Panel::left("objects")
            .default_size(280.0)
            .min_size(180.0)
            .show(ui, |ui| self.object_list(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.canvas(ui));
    }
}

fn main() -> eframe::Result {
    let command = match cli::parse_args(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}\n{}", cli::USAGE);
            std::process::exit(2);
        }
    };
    let cli::Command::Run { script } = command else {
        println!("{}", cli::USAGE);
        return Ok(());
    };
    let (construction, load_error) = match script {
        Some(path) => match cli::load_file(&path) {
            Ok(c) => (c, None),
            Err(e) => (Construction::new(), Some(e)),
        },
        None => match cli::load_text(SCRIPT) {
            Ok(c) => (c, None),
            Err(e) => {
                eprintln!("cannot load the built-in script: {e}");
                std::process::exit(1);
            }
        },
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("geokernel")
            .with_inner_size([1100.0, 700.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "geokernel",
        options,
        Box::new(|_cc| Ok(Box::new(App::new(construction, load_error)))),
    )
}
