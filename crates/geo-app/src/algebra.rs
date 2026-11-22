//! The algebra panel: one row per object, and the input row.

use crate::app::App;
use crate::geom::ToEgui;
use crate::icons::{self, Icon};
use crate::latex::{self, Formula, FormulaCache};
use crate::style;
use crate::theme;
use crate::workbench::Row;
use eframe::egui::{
    self, text::CCursor, text::CCursorRange, vec2, Align2, Color32, CursorIcon, FontId, Pos2, Rect,
    Sense, Stroke, Ui,
};
use geo_app_core::text::{self, Label};
use geo_kernel::kernel::{Construction, Def, Id, Kind, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
    set_boolean: Option<(Id, bool)>,
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
    let measures = measures(ui.ctx());
    let output = egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_viewport(ui, |ui, viewport| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let origin = ui.cursor().min;
            let width = ui.available_width();
            // Only the rows in the visible part are laid out, and the row
            // that holds the keyboard focus or an open menu (egui forgets
            // both for a widget it does not see). The heights of the others
            // come from the measures of their last layout, so the list is
            // as tall and every row where it would be with all of them
            // laid out.
            let rows = app.wb.current_rows();
            let mut measures = measures.lock().unwrap_or_else(|e| e.into_inner());
            // The measures of rows long gone (a mass deletion) are dropped
            // at once; the next frame measures the rows again.
            if measures.map.len() > 4 * rows.len() + 64 {
                measures.map.clear();
            }
            let (mut top, mut skipped) = (0.0, 0.0);
            let mut keep = None;
            let mut formulas = None;
            for row in rows {
                let height = measured_height(
                    app.wb.construction(),
                    ui.painter(),
                    row,
                    &mut measures,
                    &mut formulas,
                );
                let in_view = top + height > viewport.min.y && top < viewport.max.y;
                if all || in_view || kept.as_deref() == Some(row.label.as_str()) {
                    ui.add_space(skipped);
                    skipped = 0.0;
                    let math = row_math(app.wb.construction(), ui.painter(), row, &mut formulas);
                    if paint_row(app, ui, row, math.as_ref(), height, &mut actions) {
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
    if let Some((id, value)) = actions.set_boolean {
        app.wb.set_boolean(id, value);
    }
    if actions.number_drag == Some(false) {
        app.wb.end_gesture();
    }
    // Last: deleting rebuilds the construction and invalidates every id.
    if let Some(id) = actions.delete {
        app.wb.delete(id);
    }
}

pub(crate) struct RowMath {
    formula: Arc<Formula>,
    show_definition: bool,
    show_label: bool,
}

pub(crate) fn row_math(
    c: &Construction,
    painter: &egui::Painter,
    row: &Row,
    cache: &mut Option<FormulaCache>,
) -> Option<RowMath> {
    let (source, face, show_definition, show_label) = if row.kind == Kind::Text {
        let presentation = c.text_presentation(row.id).filter(|style| style.latex)?;
        let source = c.text(row.id).filter(|text| !text.is_empty())?;
        (source, latex::face(presentation), !row.free, row.free)
    } else {
        let source = row.formula.as_deref()?;
        let function = matches!(row.kind, Kind::Function | Kind::FunctionNVar);
        (
            source,
            geo_latex::Face::default(),
            !function && !row.free,
            !function && row.free,
        )
    };
    let cache = cache.get_or_insert_with(|| FormulaCache::new(painter));
    Some(RowMath {
        formula: cache.get(painter, source, FONT, geo_latex::Style::Text, face),
        show_definition,
        show_label,
    })
}

pub(crate) fn row_height(row: &Row, math: Option<&RowMath>) -> f32 {
    if let Some(math) = math {
        let block = math.formula.size.y + 20.0;
        if math.show_definition {
            ROW_DEPENDENT.max(48.0 + block)
        } else {
            ROW_FREE.max(block)
        }
    } else if row.free {
        ROW_FREE
    } else {
        ROW_DEPENDENT
    }
}

/// One row's height, with what it was measured against: the workbench's
/// counters move with content edits and rebuilds, and the stored source
/// and text-style bits move with a saved presentation override (a
/// `$textProperties` line changes no row). Until one of these changes, an
/// off-screen row needs no layout at all.
struct RowMeasure {
    kind: Kind,
    free: bool,
    /// The latex / bold / italic bits of a text's presentation; `None` for
    /// every row whose math does not come from one.
    style: Option<(bool, bool, bool)>,
    source: Option<Box<str>>,
    height: f32,
}

/// The measured heights of the list's rows, shared by every placement of
/// them (the list, the harness's row model, the benchmark's scroll).
#[derive(Default)]
pub(crate) struct RowMeasures {
    map: HashMap<Box<str>, RowMeasure>,
}

/// What a row's math is made of this frame: the presentation bits of a
/// LaTeX text, and the source its formula comes from.
fn row_fingerprint<'a>(
    c: &'a Construction,
    row: &'a Row,
) -> (Option<(bool, bool, bool)>, Option<&'a str>) {
    if row.kind == Kind::Text {
        match (
            c.text_presentation(row.id).filter(|style| style.latex),
            c.text(row.id).filter(|text| !text.is_empty()),
        ) {
            (Some(style), Some(source)) => {
                (Some((style.latex, style.bold, style.italic)), Some(source))
            }
            _ => (None, None),
        }
    } else {
        (None, row.formula.as_deref())
    }
}

/// The shared row measures of a context, as the list's last frames left
/// them.
pub(crate) fn measures(ctx: &egui::Context) -> Arc<Mutex<RowMeasures>> {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Arc<Mutex<RowMeasures>>>(row_measures_id())
            .clone()
    })
}

fn row_measures_id() -> egui::Id {
    egui::Id::unique("algebra_row_measures")
}

/// A row's height, measured on the frame its content or text style
/// changed and kept until one of them changes again.
pub(crate) fn measured_height(
    c: &Construction,
    painter: &egui::Painter,
    row: &Row,
    measures: &mut RowMeasures,
    formulas: &mut Option<FormulaCache>,
) -> f32 {
    let (style, source) = row_fingerprint(c, row);
    if let Some(measure) = measures.map.get(row.label.as_str()) {
        if measure.kind == row.kind
            && measure.free == row.free
            && measure.style == style
            && measure.source.as_deref() == source
        {
            return measure.height;
        }
    }
    let math = row_math(c, painter, row, formulas);
    let height = row_height(row, math.as_ref());
    measures.map.insert(
        row.label.clone().into_boxed_str(),
        RowMeasure {
            kind: row.kind,
            free: row.free,
            style,
            source: source.map(Box::from),
            height,
        },
    );
    height
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
fn paint_row(
    app: &App,
    ui: &mut Ui,
    row: &Row,
    math: Option<&RowMath>,
    height: f32,
    actions: &mut Actions,
) -> bool {
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

    if app.wb.construction().control(row.id).is_some() {
        let contents = Rect::from_min_max(
            Pos2::new(rect.left() + TEXT_X, rect.top() + 8.0),
            Pos2::new(
                kebab_rect.left().max(rect.left() + TEXT_X),
                rect.bottom() - 8.0,
            ),
        );
        keep |= paint_control(app, ui, row, id, contents, actions);
    } else if let Some(math) = math {
        paint_math(&painter, row, math, rect, kebab_rect.left());
    } else {
        keep |= paint_value(ui, &painter, row, id, rect, actions);
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

fn paint_math(painter: &egui::Painter, row: &Row, math: &RowMath, rect: Rect, right: f32) {
    let x = rect.left() + TEXT_X;
    let mut start = x;
    let y = if math.show_definition {
        painter.text(
            Pos2::new(x, rect.top() + 30.0),
            Align2::LEFT_CENTER,
            &row.head,
            FontId::proportional(FONT),
            theme::TEXT,
        );
        rect.top() + 48.0 + (rect.height() - 48.0) / 2.0
    } else {
        rect.center().y
    };
    if math.show_label || math.show_definition {
        let label = if math.show_label {
            format!("{} =", row.label)
        } else {
            "=".into()
        };
        let placed = painter.text(
            Pos2::new(x, y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(FONT),
            theme::TEXT,
        );
        start = placed.right() + 7.0;
    }
    if start < right {
        let clip = Rect::from_min_max(rect.min, Pos2::new(right, rect.bottom()));
        math.formula.paint(
            &painter.with_clip_rect(clip),
            Pos2::new(start, y - math.formula.size.y / 2.0),
            theme::TEXT,
        );
    }
}

fn paint_value(
    ui: &mut Ui,
    painter: &egui::Painter,
    row: &Row,
    id: egui::Id,
    rect: Rect,
    actions: &mut Actions,
) -> bool {
    let mut keep = false;
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

    keep
}

fn paint_control(
    app: &App,
    ui: &mut Ui,
    row: &Row,
    id: egui::Id,
    rect: Rect,
    actions: &mut Actions,
) -> bool {
    let c = app.wb.construction();
    let Some(control) = c.control(row.id) else {
        return false;
    };
    let caption = if control.caption.is_empty() {
        &row.label
    } else {
        &control.caption
    };
    let mut field = ui.new_child(
        egui::UiBuilder::new()
            .scope_id(id.with("control"))
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let def = c.def(row.id);
    if !matches!(def, Def::Boolean(_)) && !app.wb.is_visible(row.id) {
        field.disable();
    }
    let response = match def {
        Def::Boolean(mut value) => {
            let response = field.checkbox(&mut value, caption);
            if response.changed() {
                actions.set_boolean = Some((row.id, value));
            }
            response
        }
        Def::Button => field.add_enabled(false, egui::Button::new(caption)),
        Def::InputBox(target) => {
            field.label(caption);
            match target.map(|id| (id, c.value(id))) {
                Some((id, Value::Number(value))) if c.def(id).is_free() => {
                    let angle = c.kind(id) == Kind::Angle;
                    let mut number = if angle { value.to_degrees() } else { value };
                    let response = field.add(egui::DragValue::new(&mut number).suffix(if angle {
                        "°"
                    } else {
                        ""
                    }));
                    if response.drag_started() {
                        actions.number_drag = Some(true);
                    } else if response.drag_stopped() {
                        actions.number_drag = Some(false);
                    }
                    if response.changed() {
                        actions.set_number =
                            Some((id, if angle { number.to_radians() } else { number }));
                    }
                    response
                }
                Some((id, _)) => field.label(geo_app_core::describe::object_text(c, id)),
                None => field.add_enabled(
                    false,
                    egui::TextEdit::singleline(&mut String::new()).desired_width(84.0),
                ),
            }
        }
        _ => return false,
    };
    response.has_focus()
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
