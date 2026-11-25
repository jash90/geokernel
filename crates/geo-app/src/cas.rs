//! The CAS panel uses virtual rows so a frame touches only the visible cells.

use crate::{latex::FormulaCache, theme, App};
use eframe::egui::{
    self, pos2, vec2, Align2, Button, FontId, Key, Rect, Role, ScrollArea, Sense, Stroke,
    StrokeKind, TextEdit, Ui, WidgetInfo,
};
use geo_app_core::cas::{MAX_INPUT_BYTES, MAX_ROWS};
use geo_app_core::text::{label, Label};

const ROW_HEIGHT: f32 = 76.0;
const FONT_SIZE: f32 = 15.0;

enum Action {
    Edit(usize),
    Delete(usize),
    Submit,
}

pub(crate) fn show(app: &mut App, ui: &mut Ui) {
    let bounds = ui.max_rect();
    ui.painter().rect_filled(bounds, 0.0, theme::WHITE);
    ui.heading(label(Label::CasView));
    crate::app::file_notices(app, ui);
    if app.cas_editor.as_ref().is_some_and(|editor| editor.refused) {
        ui.colored_label(theme::ERROR, label(Label::CasLimit));
    }

    let rows = app.wb.cas_rows().len();
    let count = rows + usize::from(rows < MAX_ROWS);
    let mut action = None;
    let formulas = FormulaCache::new(ui.painter());
    ui.spacing_mut().item_spacing.y = 0.0;
    ScrollArea::vertical()
        .id_salt("cas_rows")
        .auto_shrink([false, false])
        .animated(false)
        .show_rows(ui, ROW_HEIGHT, count, |ui, visible| {
            if let Some(editor) = &app.cas_editor {
                if editor.focus {
                    let offset = editor.row as f32 - visible.start as f32;
                    let y = ui.max_rect().top() + offset * ROW_HEIGHT;
                    let target =
                        Rect::from_min_size(pos2(ui.min_rect().left(), y), vec2(1.0, 28.0));
                    let clip = ui.clip_rect();
                    if clip.height() >= target.height()
                        && (target.top() < clip.top() || target.bottom() > clip.bottom())
                    {
                        ui.scroll_to_rect(target, None);
                        ui.ctx().request_repaint();
                    }
                }
            }
            for row in visible {
                let (_, rect) = ui.allocate_space(vec2(ui.available_width(), ROW_HEIGHT));
                let mut cell = ui.new_child(egui::UiBuilder::new().id_salt(row).max_rect(rect));
                cell.shrink_clip_rect(rect);
                if let Some(event) = draw_row(app, &mut cell, &formulas, row) {
                    action = Some(event);
                }
            }
        });

    match action {
        Some(Action::Edit(row)) => app.edit_cas_row(row),
        Some(Action::Submit) => app.submit_cas_input(),
        Some(Action::Delete(row)) => {
            app.wb.delete_cas(row);
            app.cancel_cas_edit();
        }
        None => return,
    }
    ui.ctx().request_repaint();
}

fn draw_row(app: &mut App, ui: &mut Ui, formulas: &FormulaCache, row: usize) -> Option<Action> {
    let rect = ui.max_rect();
    let field = Rect::from_min_max(
        pos2(rect.left() + 42.0, rect.top() + 3.0),
        pos2(
            (rect.right() - 32.0).max(rect.left() + 43.0),
            rect.top() + 28.0,
        ),
    );
    let row_name = format!("{} {}", label(Label::CasRow), row + 1);
    ui.painter().text(
        pos2(rect.left() + 8.0, field.center().y),
        Align2::LEFT_CENTER,
        (row + 1).to_string(),
        FontId::proportional(FONT_SIZE),
        theme::MUTED,
    );
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, theme::SEPARATOR),
    );
    let entry = app.wb.cas_rows().get(row);
    let mut action = None;
    if let Some(editor) = app.cas_editor.as_mut().filter(|editor| editor.row == row) {
        let id = ui.make_persistent_id("input");
        if std::mem::take(&mut editor.focus) {
            ui.memory_mut(|m| m.request_focus(id));
        }
        if ui.memory(|m| m.has_focus(id)) {
            if let Some(cursor) = app.cursor_request.take() {
                use egui::text::{CCursor, CCursorRange};
                let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
                state
                    .cursor
                    .set_char_range(Some(CCursorRange::one(CCursor::new(cursor))));
                state.store(ui.ctx(), id);
            }
        }
        let output = ui
            .scope_builder(egui::UiBuilder::new().max_rect(field), |ui| {
                TextEdit::singleline(&mut editor.draft)
                    .id(id)
                    .char_limit(MAX_INPUT_BYTES)
                    .desired_width(field.width())
                    .show(ui)
            })
            .inner;
        output
            .response
            .widget_info(|| WidgetInfo::labeled(Role::TextInput, true, &row_name));
        if output.response.has_focus() {
            if let Some(range) = output.cursor_range {
                app.input_cursor = range.primary.index.into();
            }
        }
        if output.response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            action = Some(Action::Submit);
        }
    } else {
        let preview = entry.map_or(label(Label::InputHint), |entry| entry.input.as_str());
        // Text editing has its own bounded input buffer. Inactive rows need
        // only a short preview to avoid shaping whole lines on every frame.
        let preview = preview
            .char_indices()
            .nth(160)
            .map_or(preview, |(end, _)| &preview[..end]);
        let editable =
            row < MAX_ROWS && entry.is_none_or(|entry| entry.input.len() <= MAX_INPUT_BYTES);
        let button = ui
            .add_enabled_ui(editable, |ui| {
                ui.put(field, Button::new(preview).truncate().frame(false))
            })
            .inner;
        button.widget_info(|| WidgetInfo::labeled(Role::Button, true, &row_name));
        if button.clicked() {
            action = Some(Action::Edit(row));
        }
    }
    if let Some(entry) = entry {
        let mut area = Rect::from_min_max(
            pos2(field.left(), field.bottom() + 2.0),
            pos2(rect.right() - 8.0, rect.bottom() - 2.0),
        );
        if entry.unevaluated {
            area.max.y -= 16.0;
            ui.painter().text(
                pos2(area.left(), area.bottom()),
                Align2::LEFT_TOP,
                label(Label::CasUnevaluated),
                FontId::proportional(12.0),
                theme::MUTED,
            );
        }
        let output = &entry.output.text;
        let visible_output = if output.len() > geo_app_core::cas::MAX_OUTPUT_BYTES {
            output
                .char_indices()
                .nth(256)
                .map_or(output.as_str(), |(end, _)| &output[..end])
        } else {
            output.as_str()
        };
        if !output.is_empty() || entry.unevaluated {
            let result = ui.interact(
                area,
                ui.make_persistent_id("result"),
                Sense::focusable_noninteractive(),
            );
            result.widget_info(|| {
                let prefix = format!("{} {}", label(Label::CasResult), row + 1);
                let name = if entry.unevaluated {
                    format!(
                        "{prefix}: {visible_output}. {}",
                        label(Label::CasUnevaluated)
                    )
                } else {
                    format!("{prefix}: {visible_output}")
                };
                WidgetInfo::labeled(Role::Group, true, name)
            });
            if result.has_focus() {
                ui.painter().rect_stroke(
                    area,
                    2.0,
                    ui.visuals().selection.stroke,
                    StrokeKind::Inside,
                );
            }
        }
        if !output.is_empty() && entry.output.latex.is_empty() {
            ui.put(area, egui::Label::new(visible_output).truncate());
        } else if !output.is_empty() {
            let formula = formulas.get(
                ui.painter(),
                &entry.output.latex,
                FONT_SIZE,
                geo_latex::Style::Text,
                geo_latex::Face::default(),
            );
            if formula.size.x <= area.width() && formula.size.y <= area.height() {
                formula.paint(ui.painter(), area.min, theme::TEXT);
            } else {
                ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
                    ui.shrink_clip_rect(area);
                    ScrollArea::both()
                        .id_salt("output")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let (_, body) = ui.allocate_space(formula.size);
                            formula.paint(ui.painter(), body.min, theme::TEXT);
                        });
                });
            }
        }
        let delete_rect = Rect::from_min_size(
            pos2(rect.right() - 28.0, field.top()),
            vec2(24.0, field.height()),
        );
        let delete_label = format!("{} {}", label(Label::DeleteCasRow), row + 1);
        let delete = ui
            .put(delete_rect, Button::new("×").frame(false))
            .on_hover_text(&delete_label);
        delete.widget_info(|| WidgetInfo::labeled(Role::Button, true, &delete_label));
        if delete.clicked() {
            action = Some(Action::Delete(row));
        }
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headless::{self, Headless, Profile, DT};
    use crate::Workbench;

    #[test]
    fn desktop_editor_accepts_a_full_input() {
        let mut window = Headless::new(
            Workbench::default(),
            None,
            Profile::Desktop,
            (1000.0, 480.0),
            1.0,
        );
        window.app.show_cas = true;
        window.app.edit_cas_row(0);
        window.frame(DT, Vec::new());
        let expression = format!("1{}+2", " ".repeat(MAX_INPUT_BYTES - 3));
        window.frame(DT, vec![egui::Event::Text(expression.clone())]);
        window.frame(
            DT,
            vec![headless::key(Key::Enter, true, egui::Modifiers::NONE)],
        );
        assert_eq!(window.app.wb.cas_rows()[0].input, expression);
        assert_eq!(window.app.wb.cas_rows()[0].output.text, "3");
    }

    #[test]
    fn result_is_labelled_and_follows_the_input_in_tab_order() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut wb = Workbench::default();
        assert!(wb.submit_cas(0, "2/3 * 5/7"));
        let mut app = App::new(wb, None);
        app.edit_cas_row(0);
        let frame = |app: &mut App, events| {
            let screen = Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 350.0));
            let mut output = ctx.run_ui(headless::raw_input(screen, DT, DT as f32, events), |ui| {
                show(app, ui)
            });
            output.textures_delta.clear();
            output.platform_output.accesskit_update.unwrap()
        };
        let tree = frame(&mut app, Vec::new());
        let node = |name: &str| {
            tree.nodes
                .iter()
                .find(|(_, node)| node.label() == Some(name))
                .unwrap_or_else(|| panic!("missing node {name}"))
                .0
        };
        let input = node("Wiersz 1");
        let result = node("Wynik wiersza CAS 1: 10 / 21");
        let delete = node("Usuń wiersz 1");
        assert_eq!(tree.focus, input);
        let tab = || vec![headless::key(Key::Tab, true, egui::Modifiers::NONE)];
        assert_eq!(frame(&mut app, tab()).focus, result);
        frame(
            &mut app,
            vec![headless::key(Key::Tab, false, egui::Modifiers::NONE)],
        );
        assert_eq!(frame(&mut app, tab()).focus, delete);
    }

    #[test]
    fn enter_keeps_the_next_editor_in_view() {
        let mut window = Headless::new(
            Workbench::default(),
            None,
            Profile::Desktop,
            (1000.0, 480.0),
            1.0,
        );
        window.app.show_cas = true;
        window.app.edit_cas_row(0);
        window.frame(DT, Vec::new());
        for row in 1..=20 {
            window.frame(DT, vec![egui::Event::Text(row.to_string())]);
            window.frame(
                DT,
                vec![headless::key(Key::Enter, true, egui::Modifiers::NONE)],
            );
            window.frame(
                DT,
                vec![headless::key(Key::Enter, false, egui::Modifiers::NONE)],
            );
            window.settle(DT, 12);
            assert_eq!(window.app.wb.cas_rows().len(), row);
            let ctx = window.ctx();
            let focus = ctx
                .memory(|m| m.focused())
                .expect("next CAS editor is focused");
            let rect = ctx.viewport(|v| {
                v.prev_pass
                    .widgets
                    .get(focus)
                    .expect("editor rectangle")
                    .rect
            });
            assert!(
                window.screen().contains_rect(rect),
                "row {}: {rect:?}",
                row + 1
            );
        }
    }
}
