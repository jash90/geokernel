//! Spreadsheet surface. Only rows and columns intersecting the clip are built.

use crate::{theme, App};
use eframe::egui::{
    pos2, vec2, Align2, FontId, Key, Rect, Role, ScrollArea, Sense, Stroke, TextEdit, Ui,
    WidgetInfo,
};
use geo_app_core::text::{label, Label};
use geo_kernel::spreadsheet::{Address, MAX_COLUMNS, MAX_INPUT_BYTES, MAX_ROWS};

const COLUMN_WIDTH: f32 = 74.0;
const ROW_HEIGHT: f32 = 24.0;
const HEADER_WIDTH: f32 = 32.0;

enum GridAction {
    Select(Address),
    Fill(Address),
}

pub(crate) fn show(app: &mut App, ui: &mut Ui) {
    ui.heading(label(Label::SpreadsheetView));
    crate::app::file_notices(app, ui);

    if app.spreadsheet_editor.is_none() {
        let address = app
            .wb
            .spreadsheet_view()
            .and_then(|view| view.selected())
            .unwrap_or(Address { column: 1, row: 1 });
        app.select_spreadsheet_cell(address);
    }
    let editor = app
        .spreadsheet_editor
        .as_mut()
        .expect("spreadsheet selection");
    let selected = editor.selected;
    let focus = std::mem::take(&mut editor.focus);
    let reveal = std::mem::take(&mut editor.reveal);
    let error = editor.error.as_deref();
    let input_id = ui.make_persistent_id("spreadsheet_input");
    if focus {
        ui.memory_mut(|memory| memory.request_focus(input_id));
    }
    let response = ui
        .horizontal(|ui| {
            ui.label(selected.label());
            let mut text = TextEdit::load_state(ui.ctx(), input_id).unwrap_or_default();
            if focus {
                use eframe::egui::text::{CCursor, CCursorRange};
                text.cursor.set_char_range(Some(CCursorRange::two(
                    CCursor::new(0),
                    CCursor::new(editor.draft.chars().count()),
                )));
                text.store(ui.ctx(), input_id);
            }
            ui.add(
                TextEdit::singleline(&mut editor.draft)
                    .id(input_id)
                    .hint_text(label(Label::SpreadsheetInput))
                    .char_limit(MAX_INPUT_BYTES)
                    .desired_width(f32::INFINITY),
            )
        })
        .inner;
    response
        .widget_info(|| WidgetInfo::labeled(Role::TextInput, true, label(Label::SpreadsheetInput)));
    let enter = (response.has_focus() || response.lost_focus())
        && ui.input(|input| input.key_pressed(Key::Enter));
    if let Some(error) = error {
        ui.colored_label(
            theme::ERROR,
            format!("{} {error}", label(Label::SpreadsheetError)),
        );
    }
    let clicked = grid(app, ui, selected, reveal);
    if enter {
        app.submit_spreadsheet_input();
        ui.ctx().request_repaint();
    } else if let Some(action) = clicked {
        match action {
            GridAction::Select(address) => {
                if app.commit_spreadsheet_input() {
                    app.select_spreadsheet_cell(address);
                }
            }
            GridAction::Fill(target) => app.drag_spreadsheet_cell(selected, target),
        }
        ui.ctx().request_repaint();
    }
}

fn grid(app: &App, ui: &mut Ui, selected: Address, reveal: bool) -> Option<GridAction> {
    let mut clicked = None;
    let c = app.wb.construction();
    let font = FontId::proportional(13.0);
    ScrollArea::both()
        .id_salt("spreadsheet_grid")
        .auto_shrink([false, false])
        .animated(false)
        .show_viewport(ui, |ui, viewport| {
            let origin = ui.cursor().min;
            ui.set_min_size(vec2(
                HEADER_WIDTH + COLUMN_WIDTH * f32::from(MAX_COLUMNS),
                ROW_HEIGHT * (1.0 + f32::from(MAX_ROWS)),
            ));
            let col_at = |x: f32| ((x - HEADER_WIDTH) / COLUMN_WIDTH).floor().max(0.0) as u16 + 1;
            let row_at = |y: f32| (y / ROW_HEIGHT).floor().max(1.0) as u16;
            let cols = col_at(viewport.left())..=col_at(viewport.right()).min(MAX_COLUMNS);
            let rows = row_at(viewport.top())..=row_at(viewport.bottom()).min(MAX_ROWS);
            let cell_rect = |column: u16, row: u16| {
                Rect::from_min_size(
                    origin
                        + vec2(
                            HEADER_WIDTH + COLUMN_WIDTH * f32::from(column - 1),
                            ROW_HEIGHT * f32::from(row),
                        ),
                    vec2(COLUMN_WIDTH, ROW_HEIGHT),
                )
            };
            let clip = ui.clip_rect();
            let selected_rect = cell_rect(selected.column, selected.row);
            if reveal && !clip.contains_rect(selected_rect) {
                ui.scroll_to_rect(selected_rect, None);
                ui.ctx().request_repaint();
            }
            let header_y = clip.top();
            let header_x = clip.left();
            let painter = ui.painter().clone();
            painter.rect_filled(
                Rect::from_min_size(pos2(clip.left(), header_y), vec2(clip.width(), ROW_HEIGHT)),
                0.0,
                theme::TOOLBAR_BG,
            );
            painter.rect_filled(
                Rect::from_min_size(
                    pos2(header_x, clip.top()),
                    vec2(HEADER_WIDTH, clip.height()),
                ),
                0.0,
                theme::TOOLBAR_BG,
            );
            for column in cols.clone() {
                let header = cell_rect(column, 0);
                let pos = pos2(header.center().x, header_y + ROW_HEIGHT / 2.0);
                if pos.x >= header_x + HEADER_WIDTH {
                    painter.text(
                        pos,
                        Align2::CENTER_CENTER,
                        Address::column_name(column),
                        font.clone(),
                        theme::MUTED,
                    );
                }
            }
            let body = Rect::from_min_max(clip.min + vec2(HEADER_WIDTH, ROW_HEIGHT), clip.max);
            let painter = painter.with_clip_rect(body);
            for row in rows {
                let y = cell_rect(1, row).center().y;
                if y >= header_y + ROW_HEIGHT {
                    ui.painter().text(
                        pos2(header_x + HEADER_WIDTH / 2.0, y),
                        Align2::CENTER_CENTER,
                        row.to_string(),
                        font.clone(),
                        theme::MUTED,
                    );
                }
                for column in cols.clone() {
                    let address = Address { column, row };
                    let rect = cell_rect(column, row);
                    let hit = rect.intersect(body);
                    if !hit.is_positive() {
                        continue;
                    }
                    let response =
                        ui.interact(hit, ui.make_persistent_id((column, row)), Sense::click());
                    let active = selected == address;
                    let shown = c.spreadsheet_display(address);
                    response.widget_info(|| {
                        let mut info =
                            WidgetInfo::selected(Role::Button, true, active, address.label());
                        info.current_text_value = Some(shown.clone());
                        info
                    });
                    if response.clicked() {
                        clicked = Some(GridAction::Select(address));
                    }
                    if active {
                        painter.rect_filled(rect, 0.0, theme::TOOLBAR_BG);
                    }
                    let edge = Stroke::new(1.0, theme::SEPARATOR);
                    painter.hline(rect.x_range(), rect.bottom() - 0.5, edge);
                    painter.vline(rect.right() - 0.5, rect.y_range(), edge);
                    if !shown.is_empty() {
                        painter.with_clip_rect(hit).text(
                            pos2(rect.right() - 5.0, rect.center().y),
                            Align2::RIGHT_CENTER,
                            shown,
                            font.clone(),
                            theme::TEXT,
                        );
                    }
                }
            }
            if c.spreadsheet_cell(selected).is_some() {
                let corner = selected_rect.right_bottom() - vec2(5.0, 5.0);
                let handle = Rect::from_center_size(corner, vec2(8.0, 8.0));
                if body.contains_rect(handle) {
                    let response = ui.interact(
                        handle,
                        ui.make_persistent_id("spreadsheet_fill_handle"),
                        Sense::drag(),
                    );
                    response.widget_info(|| {
                        WidgetInfo::labeled(Role::Button, true, label(Label::SpreadsheetFill))
                    });
                    if response.drag_stopped() {
                        if let Some(pos) =
                            ui.ctx().pointer_latest_pos().filter(|p| body.contains(*p))
                        {
                            let delta = pos - origin;
                            if let Some(target) = Address::new(col_at(delta.x), row_at(delta.y)) {
                                clicked = Some(GridAction::Fill(target));
                            }
                        }
                    }
                    painter.rect_filled(handle, 1.0, theme::TEXT);
                }
            }
        });
    clicked
}
