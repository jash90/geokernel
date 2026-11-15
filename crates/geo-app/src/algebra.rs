//! The algebra panel: one row per object, and the input row.

use crate::app::App;
use crate::icons::{self, Icon};
use crate::style;
use crate::theme;
use crate::workbench::Row;
use eframe::egui::{
    self, text::CCursor, text::CCursorRange, vec2, Align2, Color32, CursorIcon, FontId, Pos2, Rect,
    Sense, Stroke, Ui,
};
use geo_kernel::kernel::Id;

/// Row heights: one line for a free object, two for a dependent one.
const ROW_FREE: f32 = 50.0;
const ROW_DEPENDENT: f32 = 102.0;
const FONT: f32 = 22.0;
const TEXT_X: f32 = theme::MARKER_CELL + 13.0;

/// What the user did in the list this frame.
#[derive(Default)]
struct Actions {
    select: Option<Option<Id>>,
    toggle: Option<Id>,
    delete: Option<Id>,
    set_number: Option<(Id, f64)>,
    number_drag: Option<bool>,
}

pub fn show(app: &mut App, ui: &mut Ui) {
    if let Some(e) = app.load_error.clone() {
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(theme::ERROR, format!("Nie można wczytać pliku: {e}"));
            if ui.small_button("Zamknij").clicked() {
                app.load_error = None;
            }
        });
    }
    let rows = app.wb.rows().to_vec();
    let mut actions = Actions::default();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            for row in &rows {
                paint_row(app, ui, row, &mut actions);
            }
            input_row(app, ui);
        });
    if let Some(s) = actions.select {
        app.wb.selected = s;
    }
    if let Some(id) = actions.toggle {
        app.wb.toggle_visible(id);
    }
    // A drag of a number field is one undo step.
    if actions.number_drag == Some(true) {
        app.wb.begin_gesture();
    }
    if let Some((id, v)) = actions.set_number {
        app.wb.set_number(id, v);
    }
    if actions.number_drag == Some(false) {
        app.wb.end_gesture();
    }
    // Last: deleting rebuilds the construction and invalidates every id.
    if let Some(id) = actions.delete {
        app.wb.delete(id);
    }
}

fn paint_row(app: &App, ui: &mut Ui, row: &Row, actions: &mut Actions) {
    let height = if row.free { ROW_FREE } else { ROW_DEPENDENT };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let id = ui.scope_id().with(("row", row.id.index()));
    let selected = app.wb.selected == Some(row.id);
    let hovered = app.hovered == Some(row.id);
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect, 0.0, theme::SELECTED_ROW);
    } else if hovered {
        painter.rect_filled(rect, 0.0, theme::HOVER_ROW);
    }
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, theme::SEPARATOR),
    );
    painter.vline(
        rect.left() + theme::MARKER_CELL,
        rect.y_range(),
        Stroke::new(1.0, theme::SEPARATOR),
    );

    // The body selects the object.
    let body = Rect::from_min_max(rect.min + vec2(theme::MARKER_CELL, 0.0), rect.max);
    let body_response = ui.interact(body, id.with("body"), Sense::click());
    if body_response.clicked() {
        actions.select = Some(if selected { None } else { Some(row.id) });
    }

    // The marker toggles visibility.
    let cell = Rect::from_min_size(rect.min, vec2(theme::MARKER_CELL, height));
    let marker = ui.interact(cell, id.with("marker"), Sense::click());
    let center = cell.center();
    let visible = app.wb.is_visible(row.id);
    let (fill, ring) = style::marker(app.wb.construction(), row.id);
    if visible {
        painter.circle(center, 10.0, fill, Stroke::new(1.5, ring));
    } else {
        painter.circle(center, 10.0, theme::WHITE, Stroke::new(1.5, ring));
    }
    if marker.clicked() {
        actions.toggle = Some(row.id);
    }
    marker
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(if visible { "Ukryj" } else { "Pokaż" });

    // Text.
    let font = FontId::proportional(FONT);
    let head_y = if row.free {
        rect.center().y
    } else {
        rect.top() + 30.0
    };
    let head_pos = Pos2::new(rect.left() + TEXT_X, head_y);
    let head_galley = painter.layout_no_wrap(row.head.clone(), font.clone(), theme::TEXT);
    let head_width = head_galley.size().x;
    painter.text(
        head_pos,
        Align2::LEFT_CENTER,
        &row.head,
        font.clone(),
        theme::TEXT,
    );
    if let (Some(n), true) = (row.free_number, true) {
        let mut v = n;
        let speed = (n.abs() * 0.01).max(0.01);
        let r = Rect::from_min_size(
            Pos2::new(head_pos.x + head_width + 8.0, head_y - 13.0),
            vec2(84.0, 26.0),
        );
        let response = ui.put(r, egui::DragValue::new(&mut v).speed(speed).max_decimals(2));
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::SpinButton,
                true,
                format!("Wartość {}", row.label),
            )
        });
        if response.drag_started() {
            actions.number_drag = Some(true);
        } else if response.drag_stopped() {
            actions.number_drag = Some(false);
        }
        if response.changed() {
            actions.set_number = Some((row.id, v));
        }
    }
    if let Some(value) = &row.value {
        let y = rect.top() + 72.0;
        painter.text(
            Pos2::new(rect.left() + TEXT_X, y),
            Align2::LEFT_CENTER,
            "=",
            FontId::proportional(FONT),
            theme::TEXT,
        );
        painter.text(
            Pos2::new(rect.left() + TEXT_X + 22.0, y),
            Align2::LEFT_CENTER,
            value,
            font,
            theme::TEXT,
        );
    }

    // The menu button.
    let kebab_center = Pos2::new(rect.right() - 28.0, head_y);
    let kebab_rect = Rect::from_center_size(kebab_center, vec2(30.0, 30.0));
    let kebab = ui.interact(kebab_rect, id.with("kebab"), Sense::click());
    if kebab.hovered() {
        painter.circle_filled(kebab_center, 15.0, Color32::from_rgb(0xec, 0xec, 0xec));
    }
    icons::paint(
        &painter,
        Icon::Kebab,
        kebab_center,
        9.0,
        Color32::from_rgb(0x55, 0x55, 0x55),
    );
    egui::Popup::menu(&kebab).show(|ui| {
        ui.set_min_width(120.0);
        let text = if row.siblings.is_empty() {
            "Usuń".to_owned()
        } else {
            format!("Usuń razem z {}", row.siblings.join(", "))
        };
        if ui.button(text).clicked() {
            actions.delete = Some(row.id);
            ui.close();
        }
        let toggle = if visible {
            "Ukryj obiekt"
        } else {
            "Pokaż obiekt"
        };
        if ui.button(toggle).clicked() {
            actions.toggle = Some(row.id);
            ui.close();
        }
    });
}

fn input_row(app: &mut App, ui: &mut Ui) {
    let error_height = if app.input_error.is_some() { 26.0 } else { 0.0 };
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), ROW_FREE + error_height),
        Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    let row = Rect::from_min_size(rect.min, vec2(rect.width(), ROW_FREE));
    painter.vline(
        rect.left() + theme::MARKER_CELL,
        row.y_range(),
        Stroke::new(1.0, theme::SEPARATOR),
    );
    painter.hline(
        rect.x_range(),
        row.bottom() - 0.5,
        Stroke::new(1.0, theme::SEPARATOR),
    );
    icons::paint(
        &painter,
        Icon::Plus,
        Pos2::new(rect.left() + theme::MARKER_CELL / 2.0, row.center().y),
        9.0,
        theme::MUTED,
    );

    let id = egui::Id::unique("algebra_input");
    let mut recalled_by_history = false;
    if ui.memory(|m| m.has_focus(id)) {
        let (up, down) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            )
        });
        let recalled = if up {
            app.wb.history.up(&app.input)
        } else if down {
            app.wb.history.down()
        } else {
            None
        };
        if let Some(t) = recalled {
            app.input = t.to_owned();
            recalled_by_history = true;
        }
    }
    let field = Rect::from_min_max(
        Pos2::new(rect.left() + TEXT_X, row.top() + 8.0),
        Pos2::new(rect.right() - 8.0, row.bottom() - 8.0),
    );
    let response = ui.put(
        field,
        egui::TextEdit::singleline(&mut app.input)
            .id(id)
            .hint_text("Wprowadź…")
            .text_color(theme::TEXT)
            .frame(egui::Frame::NONE)
            .font(FontId::proportional(FONT))
            .align(egui::Align2::LEFT_CENTER)
            .desired_width(field.width()),
    );
    response.widget_info(|| {
        egui::WidgetInfo::text_edit(true, String::new(), app.input.clone(), "Wprowadź…")
    });
    if app.focus_input {
        response.request_focus();
        app.focus_input = false;
    }
    if response.gained_focus() {
        // As in the reference, the keyboard comes up with the input row.
        app.keyboard.open = true;
    }
    let jump = if recalled_by_history {
        Some(app.input.chars().count())
    } else if response.has_focus() {
        app.cursor_request.take()
    } else {
        None
    };
    if let Some(at) = jump {
        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
            let at = CCursor::new(at.min(app.input.chars().count()));
            state.cursor.set_char_range(Some(CCursorRange::one(at)));
            egui::TextEdit::store_state(ui.ctx(), id, state);
        }
        app.input_cursor = at;
    } else if response.changed() {
        app.wb.history.stop_browsing();
    }
    if response.has_focus() && jump.is_none() {
        if let Some(range) =
            egui::TextEdit::load_state(ui.ctx(), id).and_then(|s| s.cursor.char_range())
        {
            app.input_cursor = range.primary.index.into();
        }
    }
    if response.has_focus() {
        painter.hline(
            row.x_range(),
            row.bottom() - 1.0,
            Stroke::new(2.0, theme::TOOL_ACTIVE),
        );
    }
    if response.has_focus()
        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        // Escape leaves the field and puts the keyboard away.
        app.input.clear();
        app.input_error = None;
        app.keyboard.open = false;
        response.surrender_focus();
        return;
    }
    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        app.submit_input();
    }
    if let Some(e) = &app.input_error {
        painter.text(
            Pos2::new(rect.left() + TEXT_X, row.bottom() + 13.0),
            Align2::LEFT_CENTER,
            e,
            FontId::proportional(13.0),
            theme::ERROR,
        );
    }
}
