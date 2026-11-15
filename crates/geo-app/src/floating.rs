//! Buttons floating over the drawing area, the tool tooltip and notices.

use crate::icons::{self, Icon};
use crate::theme;
use eframe::egui::{
    self, vec2, Align2, Color32, CursorIcon, FontId, Painter, Pos2, Rect, Response, Sense, Ui,
};
use geo_app_core::text::{self, Label};

const RADIUS: f32 = 19.0;

/// Paints a soft shadow under `rect` by stacking faint rounded rectangles.
pub fn soft_shadow(painter: &Painter, rect: Rect, rounding: f32) {
    for i in 1..=5 {
        let grow = i as f32 * 1.6;
        let alpha = (22.0 / i as f32) as u8;
        painter.rect_filled(
            rect.expand(grow).translate(vec2(0.0, 1.5)),
            rounding + grow,
            Color32::from_black_alpha(alpha / 2),
        );
    }
}

/// A round white button with a glyph. Returns the click response.
pub fn round_button(
    ui: &mut Ui,
    key: &str,
    center: Pos2,
    icon: Icon,
    enabled: bool,
    tip: &str,
) -> Response {
    let rect = Rect::from_center_size(center, vec2(RADIUS * 2.0, RADIUS * 2.0));
    // A disabled button still takes the click so it does not reach the drawing.
    let response = ui.interact(rect, ui.scope_id().with(("floating", key)), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, tip));
    let painter = ui.painter();
    soft_shadow(painter, rect, RADIUS);
    let fill = if enabled && response.hovered() {
        Color32::from_rgb(0xf2, 0xf2, 0xf2)
    } else {
        theme::WHITE
    };
    painter.circle_filled(center, RADIUS, fill);
    painter.circle_stroke(center, RADIUS, egui::Stroke::new(1.0, theme::GRID_MINOR));
    let color = if enabled {
        theme::ICON
    } else {
        theme::ICON_DISABLED
    };
    icons::paint(painter, icon, center, 9.0, color);
    if enabled {
        response
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(tip)
    } else {
        response
    }
}

/// A white pill with one line of text, centred at the top of `area`.
pub fn hint_pill(ui: &Ui, area: Rect, text: &str, color: Color32, row: f32) {
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(text.to_owned(), FontId::proportional(14.0), color);
    let size = galley.size() + vec2(28.0, 14.0);
    let center = Pos2::new(area.center().x, area.top() + 24.0 + row * 40.0);
    let rect = Rect::from_center_size(center, size);
    soft_shadow(painter, rect, size.y / 2.0);
    painter.rect_filled(rect, size.y / 2.0, theme::WHITE);
    painter.text(
        center,
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(14.0),
        color,
    );
}

/// A dark card at the bottom centre of `area` with a notice that may run
/// over several lines (what an import left out, a failed rebuild), and a
/// button that closes it. `lift` raises it above the tool tip. Returns
/// true when the user closed it.
pub fn snackbar(ui: &mut Ui, area: Rect, text: &str, lift: f32) -> bool {
    const PAD: f32 = 16.0;
    const CLOSE: &str = text::label(Label::Close);
    let font = FontId::proportional(14.0);
    let close_galley = ui
        .painter()
        .layout_no_wrap(CLOSE.to_owned(), font.clone(), theme::WHITE);
    let width = (area.width() - 32.0).clamp(120.0, 600.0);
    let text_width = (width - 3.0 * PAD - close_galley.size().x).max(40.0);
    let galley = ui
        .painter()
        .layout(text.to_owned(), font.clone(), theme::WHITE, text_width);
    let height = galley.size().y + 2.0 * 12.0;
    let width = galley.size().x + 3.0 * PAD + close_galley.size().x;
    let rect = Rect::from_min_size(
        Pos2::new(
            area.center().x - 0.5 * width,
            area.bottom() - 16.0 - lift - height,
        ),
        vec2(width, height),
    );
    let close_rect = Rect::from_min_size(
        Pos2::new(rect.right() - PAD - close_galley.size().x, rect.top()),
        vec2(close_galley.size().x, height),
    )
    .expand2(vec2(PAD / 2.0, 0.0));
    // The whole card takes clicks and drags, so none reach the drawing.
    let notice = ui.interact(
        rect,
        ui.scope_id().with("snackbar"),
        Sense::click_and_drag(),
    );
    notice.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, text));
    let close = ui
        .interact(
            close_rect,
            ui.scope_id().with("snackbar_close"),
            Sense::click(),
        )
        .on_hover_cursor(CursorIcon::PointingHand);
    close.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, text::label(Label::CloseNotice))
    });
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        6.0,
        Color32::from_rgba_unmultiplied(0x33, 0x33, 0x33, 240),
    );
    painter.galley(rect.min + vec2(PAD, 12.0), galley, theme::WHITE);
    let close_color = if close.hovered() {
        theme::WHITE
    } else {
        Color32::from_gray(0xc8)
    };
    painter.galley(
        Pos2::new(
            rect.right() - PAD - close_galley.size().x,
            rect.center().y - 0.5 * close_galley.size().y,
        ),
        close_galley,
        close_color,
    );
    close.clicked()
}

/// A square white button at the edge of the drawing area (the style bar).
pub fn square_button(ui: &mut Ui, key: &str, rect: Rect, icon: Icon, on: bool) -> Response {
    let response = ui
        .interact(rect, ui.scope_id().with(("square", key)), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let painter = ui.painter();
    soft_shadow(painter, rect, 0.0);
    let fill = if on {
        theme::SELECTED_ROW
    } else if response.hovered() {
        theme::HOVER_ROW
    } else {
        theme::WHITE
    };
    painter.rect_filled(rect, 0.0, fill);
    icons::paint(painter, icon, rect.center(), 11.0, theme::ICON);
    response
}

/// The active tool's name and what to click next, at the bottom of `area`.
/// With `closable` it ends in a close button (the counterpart of Escape for
/// a pointer without a keyboard), whose response is returned.
pub fn tool_tip(ui: &Ui, area: Rect, name: &str, hint: &str, closable: bool) -> Option<Response> {
    const CLOSE: f32 = 36.0;
    let painter = ui.painter();
    let bold = FontId::new(15.0, egui::FontFamily::Proportional);
    let name_galley = painter.layout_no_wrap(name.to_owned(), bold.clone(), theme::WHITE);
    let hint_galley = painter.layout_no_wrap(
        hint.to_owned(),
        FontId::proportional(14.0),
        Color32::from_gray(0xe0),
    );
    let close_room = if closable { CLOSE } else { 0.0 };
    let width = name_galley.size().x.max(hint_galley.size().x) + 32.0 + close_room;
    let rect = Rect::from_min_size(
        Pos2::new(area.left() + 16.0, area.bottom() - 16.0 - 54.0),
        vec2(width, 54.0),
    );
    painter.rect_filled(
        rect,
        6.0,
        Color32::from_rgba_unmultiplied(0x33, 0x33, 0x33, 235),
    );
    painter.galley(rect.min + vec2(16.0, 8.0), name_galley, theme::WHITE);
    painter.galley(rect.min + vec2(16.0, 29.0), hint_galley, theme::WHITE);
    if !closable {
        return None;
    }
    let close = Rect::from_center_size(
        Pos2::new(rect.right() - CLOSE / 2.0 - 4.0, rect.center().y),
        vec2(CLOSE, CLOSE),
    );
    let response = ui
        .interact(close, ui.scope_id().with("tool_tip_close"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, text::label(Label::LeaveTool))
    });
    let k = close.center();
    for d in [vec2(6.0, 6.0), vec2(6.0, -6.0)] {
        painter.line_segment([k - d, k + d], egui::Stroke::new(1.6, theme::WHITE));
    }
    Some(response)
}
