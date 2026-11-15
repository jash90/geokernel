//! The application state and the window layout: header, rail, side panel and
//! drawing area.

use crate::algebra;
use crate::canvas;
use crate::grid::Decor;
use crate::icons::{self, Icon};
use crate::interact::Drag;
use crate::theme;
use crate::tool_panel;
use crate::tools::{Tool, ToolState};
use crate::view::Camera;
use crate::workbench::Workbench;
use eframe::egui::{self, vec2, Align2, Color32, FontId, Frame, Pos2, Rect, Sense, Stroke, Ui};
use geo_kernel::kernel::Id;
use std::path::Path;

/// What the side panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Algebra,
    Tools,
}

/// A change of view requested outside the drawing code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewRequest {
    /// The view a document opens with.
    Initial,
    Standard,
    /// Fit all objects.
    Fit,
}

pub struct App {
    pub wb: Workbench,
    /// `None` until the first frame, when the size of the area is known.
    pub camera: Option<Camera>,
    pub view_request: Option<ViewRequest>,
    pub drag: Option<Drag>,
    pub panning: bool,
    pub mode: Mode,
    pub tool: ToolState,
    pub input: String,
    pub input_error: Option<String>,
    pub tool_error: Option<String>,
    pub load_error: Option<String>,
    pub decor: Decor,
    /// The object under the pointer in the drawing area.
    pub hovered: Option<Id>,
    pub focus_input: bool,
}

impl App {
    pub fn new(wb: Workbench, load_error: Option<String>) -> Self {
        App {
            wb,
            camera: None,
            view_request: Some(ViewRequest::Initial),
            drag: None,
            panning: false,
            mode: Mode::Algebra,
            tool: ToolState::new(Tool::Move),
            input: String::new(),
            input_error: None,
            tool_error: None,
            load_error,
            decor: Decor {
                grid: true,
                axes: true,
            },
            hovered: None,
            focus_input: false,
        }
    }

    pub fn open(&mut self, path: &Path) {
        match crate::cli::load_into(&mut self.wb, path) {
            Ok(()) => {
                self.load_error = None;
                self.drag = None;
                self.view_request = Some(ViewRequest::Initial);
            }
            Err(e) => self.load_error = Some(e),
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let (redo, undo, escape, fit) = ctx.input_mut(|i| {
            (
                i.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Z,
                ),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
                false,
            )
        });
        let _ = fit;
        if redo {
            self.wb.redo();
        } else if undo {
            self.wb.undo();
        }
        if escape {
            self.tool.set_tool(Tool::Move);
            self.tool_error = None;
            self.wb.selected = None;
        }
        if !ctx.egui_wants_keyboard_input()
            && ctx.input(|i| {
                i.key_pressed(egui::Key::F)
                    && !i.modifiers.ctrl
                    && !i.modifiers.command
                    && !i.modifiers.alt
            })
        {
            self.view_request = Some(ViewRequest::Fit);
        }
    }

    /// Lays out the whole window into `ui`.
    pub fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if let Some(path) = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_owned()))
        {
            self.open(&path);
        }
        self.shortcuts(&ctx);
        let white = Frame::NONE.fill(theme::WHITE);
        egui::Panel::top("header")
            .exact_size(theme::HEADER_HEIGHT)
            .resizable(false)
            .show_separator_line(false)
            .frame(white)
            .show(ui, |ui| self.header(ui));
        egui::Panel::left("rail")
            .exact_size(theme::RAIL_WIDTH)
            .resizable(false)
            .show_separator_line(false)
            .frame(white)
            .show(ui, |ui| self.rail(ui));
        egui::Panel::left("side")
            .exact_size(theme::SIDE_WIDTH)
            .resizable(false)
            .show_separator_line(false)
            .frame(white)
            .show(ui, |ui| match self.mode {
                Mode::Algebra => algebra::show(self, ui),
                Mode::Tools => tool_panel::show(self, ui),
            });
        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(theme::WHITE))
            .show(ui, |ui| canvas::show(self, ui));
    }

    fn header(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        let painter = ui.painter().clone();
        painter.hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, theme::SEPARATOR),
        );
        let center = Pos2::new(rect.left() + theme::RAIL_WIDTH / 2.0, rect.center().y);
        let menu = ui.interact(
            Rect::from_center_size(center, vec2(44.0, 44.0)),
            ui.scope_id().with("menu"),
            Sense::click(),
        );
        if menu.hovered() {
            painter.circle_filled(center, 22.0, Color32::from_rgb(0xf0, 0xf0, 0xf0));
        }
        icons::paint(
            &painter,
            Icon::Menu,
            center,
            11.0,
            Color32::from_rgb(0x55, 0x55, 0x55),
        );
        egui::Popup::menu(&menu).show(|ui| {
            ui.set_min_width(180.0);
            if ui
                .add_enabled(self.wb.can_undo(), egui::Button::new("Cofnij"))
                .clicked()
            {
                self.wb.undo();
                ui.close();
            }
            if ui
                .add_enabled(self.wb.can_redo(), egui::Button::new("Ponów"))
                .clicked()
            {
                self.wb.redo();
                ui.close();
            }
            if ui.button("Widok standardowy").clicked() {
                self.view_request = Some(ViewRequest::Standard);
                ui.close();
            }
        });
        painter.text(
            Pos2::new(rect.left() + theme::RAIL_WIDTH + 5.0, rect.center().y),
            Align2::LEFT_CENTER,
            "geokernel",
            FontId::proportional(31.0),
            theme::TITLE_GREY,
        );
    }

    fn rail(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        let entries = [
            (Mode::Algebra, Icon::Algebra, "Algebra"),
            (Mode::Tools, Icon::Tools, "Narzędzia"),
        ];
        for (i, (mode, icon, caption)) in entries.into_iter().enumerate() {
            let cell = Rect::from_min_size(
                rect.min + vec2(0.0, 10.0 + i as f32 * 74.0),
                vec2(rect.width(), 70.0),
            );
            let response = ui.interact(cell, ui.scope_id().with(("rail", i)), Sense::click());
            let active = self.mode == mode;
            let painter = ui.painter();
            if response.hovered() && !active {
                painter.rect_filled(
                    cell.shrink2(vec2(6.0, 2.0)),
                    8.0,
                    Color32::from_rgb(0xf3, 0xf3, 0xf3),
                );
            }
            let color = if active {
                theme::ACCENT
            } else {
                Color32::from_rgb(0x6b, 0x6b, 0x6b)
            };
            icons::paint(
                painter,
                icon,
                cell.center_top() + vec2(0.0, 25.0),
                11.0,
                color,
            );
            painter.text(
                cell.center_top() + vec2(0.0, 52.0),
                Align2::CENTER_CENTER,
                caption,
                FontId::proportional(11.0),
                color,
            );
            if response.clicked() {
                self.mode = mode;
            }
        }
    }
}
