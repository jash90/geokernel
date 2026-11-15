//! The algebra panel: one row per object, and the input row.

use crate::app::App;
use crate::geom::ToEgui;
use crate::icons::{self, Icon};
use crate::style;
use crate::theme;
use crate::workbench::Row;
use eframe::egui::{
    self, text::CCursor, text::CCursorRange, vec2, Align2, Color32, CursorIcon, FontId, Pos2, Rect,
    Sense, Stroke, Ui,
};
use geo_app_core::text::{self, Label};
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
    app.wb.refresh_rows();
    let mut actions = Actions::default();
    let kept: Option<String> = ui.data(|d| d.get_temp(kept_id()));
    let all = lay_out_all_rows();
    let output = egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_viewport(ui, |ui, viewport| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let origin = ui.cursor().min;
            let width = ui.available_width();
            // Only the rows in the visible part are laid out, and the row
            // that holds the keyboard focus or an open menu (egui forgets
            // both for a widget it does not see). The space of the others
            // is left empty, so the list is as tall and every row where it
            // would be with all of them laid out.
            let rows = app.wb.current_rows();
            let (mut top, mut skipped) = (0.0, 0.0);
            let mut keep = None;
            for row in rows {
                let height = row_height(row);
                let in_view = top + height > viewport.min.y && top < viewport.max.y;
                if all || in_view || kept.as_deref() == Some(row.label.as_str()) {
                    ui.add_space(skipped);
                    skipped = 0.0;
                    if paint_row(app, ui, row, &mut actions) {
                        keep = Some(row.label.clone());
                    }
                } else {
                    skipped += height;
                }
                top += height;
            }
            ui.add_space(skipped);
            input_row(app, ui);
            (origin, width, keep)
        });
    let (origin, width, keep) = output.inner;
    ui.data_mut(|d| {
        d.insert_temp(
            geometry_id(),
            ListGeometry {
                origin,
                width,
                viewport: output.inner_rect,
                content: output.content_size,
                offset: output.state.clamped_offset().y,
            },
        );
        if let Some(label) = keep {
            d.insert_temp(kept_id(), label);
        } else {
            d.remove::<String>(kept_id());
        }
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

pub(crate) fn row_height(row: &Row) -> f32 {
    if row.free {
        ROW_FREE
    } else {
        ROW_DEPENDENT
    }
}

/// The id of an object's row. Its widgets' ids derive from it (`body`,
/// `marker`, `kebab`, `value`), so they stay the same when other rows
/// scroll in or out, and the headless window names rows by them.
pub(crate) fn row_id(label: &str) -> egui::Id {
    egui::Id::unique(("algebra_row", label))
}

/// The click areas of a row laid out at `rect`: its text (`body`), its
/// visibility marker and its menu button, with the salt of each one's id.
pub(crate) fn row_parts(row: &Row, rect: Rect) -> [(&'static str, Rect); 3] {
    let body = Rect::from_min_max(rect.min + vec2(theme::MARKER_CELL, 0.0), rect.max);
    let marker = Rect::from_min_size(rect.min, vec2(theme::MARKER_CELL, rect.height()));
    let kebab = Rect::from_center_size(
        Pos2::new(rect.right() - 28.0, head_y(row, rect)),
        vec2(30.0, 30.0),
    );
    [("body", body), ("marker", marker), ("kebab", kebab)]
}

/// The middle of a row's first line.
fn head_y(row: &Row, rect: Rect) -> f32 {
    if row.free {
        rect.center().y
    } else {
        rect.top() + 30.0
    }
}

/// Where the list was laid out in the last frame, in window points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ListGeometry {
    /// The top left corner of the first row, scrolled.
    pub origin: Pos2,
    pub width: f32,
    /// The visible part of the list.
    pub viewport: Rect,
    /// The size of everything in the list (rows and the input row).
    pub content: egui::Vec2,
    /// How far the list is scrolled.
    pub offset: f32,
}

fn geometry_id() -> egui::Id {
    egui::Id::unique("algebra_list_geometry")
}

fn kept_id() -> egui::Id {
    egui::Id::unique("algebra_kept_row")
}

/// The list as the last frame laid it out.
#[cfg(any(test, feature = "headless"))]
pub(crate) fn geometry(ctx: &egui::Context) -> Option<ListGeometry> {
    ctx.data(|d| d.get_temp(geometry_id()))
}

#[cfg(test)]
thread_local! {
    /// Tests lay out every row, to compare the visible-only list with it.
    pub(crate) static ALL_ROWS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn lay_out_all_rows() -> bool {
    #[cfg(test)]
    return ALL_ROWS.with(std::cell::Cell::get);
    #[cfg(not(test))]
    false
}

/// Lays out one row; returns whether it holds the keyboard focus or an
/// open menu.
fn paint_row(app: &App, ui: &mut Ui, row: &Row, actions: &mut Actions) -> bool {
    let height = row_height(row);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let id = row_id(&row.label);
    let [(_, body), (_, cell), (_, kebab_rect)] = row_parts(row, rect);
    let mut keep = false;
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
    let body_response = ui.interact(body, id.with("body"), Sense::click());
    if body_response.clicked() {
        actions.select = Some(if selected { None } else { Some(row.id) });
    }

    // The marker toggles visibility.
    let marker = ui.interact(cell, id.with("marker"), Sense::click());
    let center = cell.center();
    let visible = app.wb.is_visible(row.id);
    let base = app.wb.palette().color(row.id);
    let (fill, ring) = style::marker_of(base);
    let (fill, ring) = (fill.egui(), ring.egui());
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
        .on_hover_text(if visible {
            text::label(Label::Hide)
        } else {
            text::label(Label::Show)
        });

    // Text.
    let font = FontId::proportional(FONT);
    let head_y = head_y(row, rect);
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
    if let Some(n) = row.free_number {
        // An angle is edited in degrees.
        let shown = if row.angle { n.to_degrees() } else { n };
        let mut v = shown;
        let speed = (shown.abs() * 0.01).max(0.01);
        let r = Rect::from_min_size(
            Pos2::new(head_pos.x + head_width + 8.0, head_y - 13.0),
            vec2(84.0, 26.0),
        );
        let drag = egui::DragValue::new(&mut v).speed(speed).max_decimals(2);
        let drag = if row.angle { drag.suffix("°") } else { drag };
        // In a child of the row that does not move the row's cursor: the
        // next row starts where this one ends, as the skipped rows are
        // counted. Its id is the object's, so the field keeps its focus and
        // text when the first drawn row changes.
        let mut field = ui.new_child(
            egui::UiBuilder::new()
                .scope_id(id.with("value"))
                .max_rect(r)
                .layout(egui::Layout::centered_and_justified(
                    egui::Direction::TopDown,
                )),
        );
        let response = field.add(drag);
        keep |= response.has_focus();
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
            actions.set_number = Some((row.id, if row.angle { v.to_radians() } else { v }));
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
    let kebab_center = kebab_rect.center();
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
    let menu = egui::Popup::menu(&kebab).show(|ui| {
        ui.set_min_width(120.0);
        let text = if row.siblings.is_empty() {
            text::label(Label::Delete).to_owned()
        } else {
            format!("Usuń razem z {}", row.siblings.join(", "))
        };
        if ui.button(text).clicked() {
            actions.delete = Some(row.id);
            ui.close();
        }
        let toggle = if visible {
            text::label(Label::HideObject)
        } else {
            text::label(Label::ShowObject)
        };
        if ui.button(toggle).clicked() {
            actions.toggle = Some(row.id);
            ui.close();
        }
    });
    keep || menu.is_some()
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
        egui::WidgetInfo::text_edit(
            true,
            String::new(),
            app.input.clone(),
            text::label(Label::InputHint),
        )
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
