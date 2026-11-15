//! The application state and the window layout: toolbar, algebra view,
//! graphics view and the on-screen keyboard.

use crate::algebra;
use crate::canvas;
use crate::errors;
use crate::grid::Decor;
use crate::interact::Drag;
use crate::keyboard::{self, Keyboard};
use crate::layout::Regions;
use crate::theme;
use crate::toolbar;
use crate::tools::{Outcome, Tool, ToolState, GROUPS};
use crate::view::Camera;
use crate::workbench::Workbench;
use eframe::egui::{self, Rect, Stroke, Ui, UiBuilder};
use geo_kernel::kernel::Id;
use std::path::Path;

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
    pub tool: ToolState,
    /// The tool each toolbar group shows (the last one chosen in it).
    pub group_tools: Vec<Tool>,
    pub input: String,
    pub input_error: Option<String>,
    pub tool_error: Option<String>,
    pub load_error: Option<String>,
    pub decor: Decor,
    /// The object under the pointer in the drawing area.
    pub hovered: Option<Id>,
    pub focus_input: bool,
    /// The input row's cursor (in characters), kept while the on-screen
    /// keyboard takes the focus for a moment.
    pub input_cursor: usize,
    /// Put the input row's cursor here when it next has the focus.
    pub cursor_request: Option<usize>,
    pub keyboard: Keyboard,
    /// The graphics style bar is open.
    pub style_bar: bool,
    /// The text of the number dialog.
    pub number_text: String,
    /// Where the regions were in the last frame.
    pub regions: Regions,
    /// `Workbench::rebuilds` as the drawing last saw it.
    pub seen_rebuilds: u64,
    /// A text field had the keyboard in the previous frame.
    was_typing: bool,
}

impl App {
    pub fn new(wb: Workbench, load_error: Option<String>) -> Self {
        App {
            wb,
            camera: None,
            view_request: Some(ViewRequest::Initial),
            drag: None,
            panning: false,
            tool: ToolState::new(Tool::Move),
            group_tools: GROUPS.iter().map(|g| g[0]).collect(),
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
            input_cursor: 0,
            cursor_request: None,
            keyboard: Keyboard::default(),
            style_bar: false,
            number_text: String::new(),
            regions: Regions::new(Rect::ZERO, false),
            seen_rebuilds: 0,
            was_typing: false,
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

    /// Makes `tool` the active tool and the one its group shows.
    pub fn select_tool(&mut self, tool: Tool) {
        self.tool.set_tool(tool);
        self.group_tools[tool.group()] = tool;
        self.tool_error = None;
    }

    /// Runs the input row's text.
    pub fn submit_input(&mut self) {
        match self.wb.run_line(&self.input) {
            Ok(()) => {
                self.input.clear();
                self.input_cursor = 0;
                self.input_error = None;
                self.wb.selected = None;
            }
            Err(e) => self.input_error = Some(errors::polish(&e)),
        }
        self.focus_input = true;
    }

    /// Runs what a finished tool asks for.
    pub fn apply_outcome(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Run(lines) => match self.wb.run_lines(&lines) {
                Ok(()) => {
                    self.tool_error = None;
                    self.wb.selected = self.wb.construction().ids().last();
                }
                Err(e) => {
                    self.tool_error = Some(errors::polish(&e));
                    self.tool.reset();
                }
            },
            Outcome::AskNumber(request) => {
                self.number_text = crate::describe::n2(request.default);
                self.tool_error = None;
            }
            Outcome::Delete(label) => {
                if let Some(id) = self.wb.construction().id(&label) {
                    self.wb.delete(id);
                }
                self.tool_error = None;
            }
            Outcome::ToggleVisible(label) => {
                if let Some(id) = self.wb.construction().id(&label) {
                    self.wb.toggle_visible(id);
                }
            }
            Outcome::Select(label) => {
                self.wb.selected = self.wb.construction().id(&label);
                self.tool_error = None;
            }
            Outcome::Pending => self.tool_error = None,
            Outcome::Ignored => {}
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        // While a text field has focus these keys belong to it: Cmd+Z undoes
        // the text, Escape leaves the field.
        // egui drops the focus itself when Escape arrives, so the field
        // counts as focused if it was a frame ago.
        let typing = ctx.egui_wants_keyboard_input();
        let was_typing = std::mem::replace(&mut self.was_typing, typing);
        let typing = typing || was_typing;
        if self.tool.asking().is_some() {
            return;
        }
        if typing {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.input.clear();
                self.input_error = None;
                self.keyboard.open = false;
                ctx.memory_mut(|m| {
                    if let Some(id) = m.focused() {
                        m.surrender_focus(id);
                    }
                });
            }
        } else {
            let (redo, undo, escape) = ctx.input_mut(|i| {
                (
                    i.consume_key(
                        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                        egui::Key::Z,
                    ),
                    i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
                )
            });
            if redo {
                self.wb.redo();
            } else if undo {
                self.wb.undo();
            }
            if escape {
                if self.drag.is_some() {
                    // Abandon the drag and put the point back.
                    self.wb.cancel_gesture();
                    self.drag = None;
                    self.panning = false;
                } else {
                    self.select_tool(Tool::Move);
                    self.wb.selected = None;
                    self.style_bar = false;
                }
            }
            if ctx.input(|i| {
                i.key_pressed(egui::Key::F)
                    && !i.modifiers.ctrl
                    && !i.modifiers.command
                    && !i.modifiers.alt
            }) {
                self.view_request = Some(ViewRequest::Fit);
            }
        }
    }

    /// Runs `f` in a child ui that covers exactly `rect`.
    fn region(&mut self, ui: &mut Ui, rect: Rect, salt: &str, f: impl FnOnce(&mut Self, &mut Ui)) {
        let mut child = ui.new_child(
            UiBuilder::new()
                .id_salt(salt)
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        f(self, &mut child);
    }

    /// Lays out the whole window into `ui`.
    pub fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if let Some(path) = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_owned()))
        {
            self.open(&path);
        }
        self.shortcuts(&ctx);
        // A press in the graphics view puts the keyboard away.
        let pressed_at = ctx.input(|i| {
            i.pointer
                .primary_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten()
        });
        if pressed_at.is_some_and(|p| self.regions.graphics.contains(p)) {
            self.keyboard.open = false;
        }
        let regions = Regions::new(ui.max_rect(), self.keyboard.open);
        self.regions = regions;
        let painter = ui.painter().clone();
        painter.rect_filled(regions.splitter, 0.0, theme::TOOLBAR_BG);
        for x in [
            regions.splitter.left() + 0.5,
            regions.splitter.right() - 0.5,
        ] {
            painter.vline(
                x,
                regions.splitter.y_range(),
                Stroke::new(1.0, theme::BORDER),
            );
        }
        self.region(ui, regions.algebra, "algebra", |app, ui| {
            algebra::show(app, ui)
        });
        self.region(ui, regions.graphics, "graphics", |app, ui| {
            canvas::show(app, ui)
        });
        if let Some(rect) = regions.keyboard {
            self.region(ui, rect, "keyboard", keyboard::show);
        }
        // Last, so that its drop-downs are above the views.
        self.region(ui, regions.toolbar, "toolbar", |app, ui| {
            toolbar::show(app, ui)
        });
        self.number_dialog(&ctx);
    }

    /// The dialog of a tool that needs a number.
    fn number_dialog(&mut self, ctx: &egui::Context) {
        let Some(request) = self.tool.asking() else {
            return;
        };
        let mut answer: Option<Option<f64>> = None;
        let modal = egui::Modal::new(egui::Id::unique("number_dialog")).show(ctx, |ui| {
            ui.set_width(300.0);
            ui.heading(request.title);
            ui.add_space(8.0);
            let prompt = ui.label(request.prompt);
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.number_text)
                    .id(egui::Id::unique("number_field"))
                    .desired_width(f32::INFINITY),
            );
            let field = field.labelled_by(prompt.id);
            if !field.has_focus() && !field.lost_focus() {
                field.request_focus();
            }
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let parsed = parse_number(&self.number_text);
            if parsed.is_none() {
                ui.colored_label(theme::ERROR, "To nie jest liczba");
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Anuluj").clicked() {
                    answer = Some(None);
                }
                let ok = ui.add_enabled(parsed.is_some(), egui::Button::new("OK"));
                if (ok.clicked() || enter) && parsed.is_some() {
                    answer = Some(parsed);
                }
            });
        });
        if modal.should_close() && answer.is_none() {
            answer = Some(None);
        }
        match answer {
            Some(Some(v)) => {
                let outcome = {
                    let c = self.wb.construction();
                    let decimals = self
                        .camera
                        .map_or(2, |cam| crate::describe::click_decimals(cam.scale));
                    self.tool.finish_number(v, decimals, &|l| c.id(l).is_some())
                };
                self.apply_outcome(outcome);
            }
            Some(None) => self.tool.reset(),
            None => {}
        }
    }
}

/// A number typed by a person: a decimal comma is accepted, a degree sign
/// is ignored.
pub fn parse_number(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches('°').trim().replace(',', ".");
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::parse_number;

    #[test]
    fn typed_numbers_accept_a_comma_and_a_degree_sign() {
        assert_eq!(parse_number(" 2,5 "), Some(2.5));
        assert_eq!(parse_number("45°"), Some(45.0));
        assert_eq!(parse_number("-3"), Some(-3.0));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number("inf"), None);
    }
}
