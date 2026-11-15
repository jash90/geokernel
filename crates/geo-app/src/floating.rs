//! Round buttons floating over the drawing area, and the pill with the tool hint.

use crate::icons::{self, Icon};
use crate::theme;
use eframe::egui::{
    self, vec2, Align2, Color32, CursorIcon, FontId, Painter, Pos2, Rect, Response, Sense, Ui,
};

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
    let painter = ui.painter();
    soft_shadow(painter, rect, RADIUS);
    let fill = if enabled && response.hovered() {
        Color32::from_rgb(0xf2, 0xf2, 0xf2)
    } else {
        theme::WHITE
    };
    painter.circle_filled(center, RADIUS, fill);
    painter.circle_stroke(
        center,
        RADIUS,
        egui::Stroke::new(1.0, Color32::from_rgb(0xe4, 0xe4, 0xe4)),
    );
    let color = if enabled {
        Color32::from_rgb(0x55, 0x55, 0x55)
    } else {
        Color32::from_rgb(0xc4, 0xc4, 0xc4)
    };
    icons::paint(painter, icon, center, 9.5, color);
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
