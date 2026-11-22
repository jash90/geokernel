//! The window layout: toolbar, algebra view, graphics view and the on-screen
//! keyboard, the keyboard shortcuts and the number dialog. The state lives in
//! `geo_app_core::app`.

use crate::algebra;
use crate::canvas;
use crate::geom::{ToCore, ToEgui};
use crate::keyboard;
use crate::layout::Regions;
use crate::theme;
use crate::toolbar;
use eframe::egui::{self, Rect, Stroke, Ui, UiBuilder};
pub use geo_app_core::app::{parse_number, App, ViewRequest};
use geo_app_core::text::{self, Label};
use std::path::Path;

/// Opens a script or construction file into the window.
pub fn open(app: &mut App, path: &Path) {
    match crate::cli::load_into(&mut app.wb, path) {
        Ok(()) => {
            app.load_error = None;
            app.drag = None;
            // The tool's picks belonged to the other document.
            app.reset_tool();
            app.view_request = Some(ViewRequest::Initial);
        }
        Err(e) => app.load_error = Some(e),
    }
}

fn shortcuts(app: &mut App, ctx: &egui::Context) {
    // While a text field has focus these keys belong to it: Cmd+Z undoes
    // the text, Escape leaves the field.
    // egui drops the focus itself when Escape arrives, so the field
    // counts as focused if it was a frame ago.
    let typing = ctx.egui_wants_keyboard_input();
    let was_typing = std::mem::replace(&mut app.was_typing, typing);
    let typing = typing || was_typing;
    // egui ends any drag on Escape, whoever takes the key afterwards.
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.gesture.stop();
    }
    if app.tool.asking().is_some() {
        return;
    }
    if typing {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            app.input.clear();
            app.input_error = None;
            app.keyboard.open = false;
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
            app.redo();
        } else if undo {
            app.undo();
        }
        if escape {
            if app.drag.is_some() {
                // Abandon the drag and put the point (or the label) back.
                app.cancel_drag();
            } else {
                app.leave_tool();
            }
        }
        if ctx.input(|i| {
            i.key_pressed(egui::Key::F)
                && !i.modifiers.ctrl
                && !i.modifiers.command
                && !i.modifiers.alt
        }) {
            app.view_request = Some(ViewRequest::Fit);
        }
        // Ctrl + plus / equals zooms in, Ctrl + minus out, by the wheel
        // factor about the last pointer position (`GlobalKeyDispatcher`;
        // spec D14). The drawing applies it, with the view's own area.
        let key = ctx.input_mut(|i| {
            [
                (egui::Key::Plus, 1.0),
                (egui::Key::Equals, 1.0),
                (egui::Key::Minus, -1.0),
            ]
            .into_iter()
            .find_map(|(k, dir)| i.consume_key(egui::Modifiers::COMMAND, k).then_some(dir))
        });
        if let Some(dir) = key {
            app.key_zoom = Some(geo_app_core::camera::KEY_ZOOM_FACTOR.powf(dir));
        }
    }
}

/// Runs `f` in a child ui that covers exactly `rect`.
fn region(app: &mut App, ui: &mut Ui, rect: Rect, salt: &str, f: impl FnOnce(&mut App, &mut Ui)) {
    let mut child = ui.new_child(
        UiBuilder::new()
            .id_salt(salt)
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    f(app, &mut child);
}

/// Lays out the whole window into `ui`.
pub fn show(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    if let Some(path) = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_owned())) {
        open(app, &path);
    }
    shortcuts(app, &ctx);
    // A press in the graphics view puts the keyboard away.
    let pressed_at = ctx.input(|i| {
        i.pointer
            .primary_pressed()
            .then(|| i.pointer.interact_pos())
            .flatten()
    });
    if pressed_at.is_some_and(|p| app.regions.graphics.egui().contains(p)) {
        app.keyboard.open = false;
    }
    let regions = Regions::new(ui.max_rect().core(), app.keyboard.open, &app.metrics);
    app.regions = regions;
    let splitter = regions.splitter.egui();
    let painter = ui.painter().clone();
    // The root ui of a real window has no fill of its own.
    painter.rect_filled(ui.max_rect(), 0.0, theme::WHITE);
    painter.rect_filled(splitter, 0.0, theme::TOOLBAR_BG);
    if regions.portrait {
        for y in [splitter.top() + 0.5, splitter.bottom() - 0.5] {
            painter.hline(splitter.x_range(), y, Stroke::new(1.0, theme::BORDER));
        }
    } else {
        for x in [splitter.left() + 0.5, splitter.right() - 0.5] {
            painter.vline(x, splitter.y_range(), Stroke::new(1.0, theme::BORDER));
        }
    }
    region(app, ui, regions.algebra.egui(), "algebra", |app, ui| {
        algebra::show(app, ui)
    });
    region(app, ui, regions.graphics.egui(), "graphics", |app, ui| {
        canvas::show(app, ui)
    });
    if let Some(rect) = regions.keyboard {
        region(app, ui, rect.egui(), "keyboard", keyboard::show);
    }
    // Last, so that its drop-downs are above the views.
    region(app, ui, regions.toolbar.egui(), "toolbar", |app, ui| {
        toolbar::show(app, ui)
    });
    number_dialog(app, &ctx);
}

/// The dialog of a tool that needs a number.
fn number_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(request) = app.tool.asking() else {
        return;
    };
    let mut answer: Option<Option<f64>> = None;
    let modal = egui::Modal::new(egui::Id::unique("number_dialog")).show(ctx, |ui| {
        ui.set_width(300.0);
        ui.heading(request.title);
        ui.add_space(8.0);
        let prompt = ui.label(request.prompt);
        let field = ui.add(
            egui::TextEdit::singleline(&mut app.number_text)
                .id(egui::Id::unique("number_field"))
                .desired_width(f32::INFINITY),
        );
        let field = field.labelled_by(prompt.id);
        if !field.has_focus() && !field.lost_focus() {
            field.request_focus();
        }
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let parsed = parse_number(&app.number_text);
        if parsed.is_none() {
            ui.colored_label(theme::ERROR, text::label(Label::NotANumber));
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
                let c = app.wb.construction();
                app.tool.finish_number(v, &|l| c.id(l).is_some())
            };
            app.apply_outcome(outcome);
        }
        Some(None) => app.reset_tool(),
        None => {}
    }
}
