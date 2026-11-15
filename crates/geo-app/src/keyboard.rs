//! The on-screen keyboard panel: the tabs, the close button and the keys of
//! the model in `geo_app_core::keyboard`.
//!
//! The layout of the keys is our own, laid out by eye from a screenshot of
//! the reference keyboard; no code or artwork of the reference was used.

use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme;
use eframe::egui::{self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Rect, Sense, Ui};
pub use geo_app_core::keyboard::*;
use geo_app_core::text::{self, Label};

const KEY: egui::Vec2 = vec2(66.0, 40.0);
const PITCH: egui::Vec2 = vec2(70.0, 44.0);
const BLOCK_GAP: f32 = 18.0;

pub fn show(app: &mut App, ui: &mut Ui) {
    let panel = ui.max_rect();
    let painter = ui.painter().clone();
    painter.rect_filled(panel, 0.0, theme::KEYBOARD_BG);
    painter.hline(
        panel.x_range(),
        panel.top() + 0.5,
        egui::Stroke::new(1.0, theme::BORDER),
    );
    let rows = rows(app.keyboard.tab);
    let width = |row: &Vec<Option<(Key, f32)>>| -> f32 {
        row.iter()
            .map(|k| k.map_or(BLOCK_GAP, |(_, w)| PITCH.x * w))
            .sum::<f32>()
            - (PITCH.x - KEY.x)
    };
    let widest = rows.iter().map(width).fold(0.0, f32::max);
    let left = panel.center().x - widest / 2.0;

    // Tabs.
    let tab_y = panel.top() + 22.0;
    let mut x = left + 2.0;
    for (tab, caption) in [(Tab::Numbers, "123"), (Tab::Letters, "ABC")] {
        let galley =
            painter.layout_no_wrap(caption.into(), FontId::proportional(16.0), theme::TEXT);
        let rect = Rect::from_min_size(pos2(x, tab_y - 13.0), vec2(galley.size().x + 22.0, 26.0));
        let response = ui
            .interact(rect, ui.scope_id().with(("tab", caption)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Tab, true, caption));
        let active = app.keyboard.tab == tab;
        if active {
            painter.rect_filled(rect, 13.0, theme::KEY_TAB_ACTIVE);
        }
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            caption,
            FontId::proportional(16.0),
            if active { theme::WHITE } else { theme::TEXT },
        );
        if response.clicked() {
            app.keyboard.tab = tab;
            app.focus_input = true;
        }
        x = rect.right() + 14.0;
    }
    let close = Rect::from_center_size(pos2(panel.right() - 20.0, tab_y), vec2(30.0, 30.0));
    let close_response = ui
        .interact(close, ui.scope_id().with("close"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text("Zamknij klawiaturę");
    close_response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, text::label(Label::CloseKeyboard))
    });
    let k = close.center();
    for d in [vec2(7.0, 7.0), vec2(7.0, -7.0)] {
        painter.line_segment([k - d, k + d], egui::Stroke::new(1.6, theme::ICON));
    }
    if close_response.clicked() {
        app.keyboard.open = false;
    }

    // Keys.
    let top = panel.top() + 43.0;
    for (r, row) in rows.iter().enumerate() {
        let mut x = left;
        let y = top + r as f32 * PITCH.y;
        for entry in row {
            let Some((key, w)) = *entry else {
                x += BLOCK_GAP;
                continue;
            };
            let rect =
                Rect::from_min_size(pos2(x, y), vec2(PITCH.x * w - (PITCH.x - KEY.x), KEY.y));
            x += PITCH.x * w;
            let text = cap(key, app.keyboard.shift);
            let response = ui
                .interact(
                    rect,
                    ui.scope_id().with(("key", r, rect.left() as i32)),
                    Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand);
            let name = match key {
                Key::Backspace => text::label(Label::KeyBackspace).to_owned(),
                Key::Enter => text::label(Label::KeyEnter).to_owned(),
                Key::Space => text::label(Label::KeySpace).to_owned(),
                Key::Left => text::label(Label::KeyLeft).to_owned(),
                Key::Right => text::label(Label::KeyRight).to_owned(),
                Key::Shift => text::label(Label::KeyShift).to_owned(),
                _ => text.clone(),
            };
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
            let special = matches!(key, Key::Backspace | Key::Left | Key::Right | Key::Enter)
                || (key == Key::Shift && app.keyboard.shift);
            let fill = match (special, response.hovered()) {
                (true, _) => Color32::from_rgb(0xd0, 0xd0, 0xd6),
                (false, true) => Color32::from_rgb(0xf6, 0xf6, 0xf8),
                (false, false) => theme::WHITE,
            };
            painter.rect_filled(rect, 6.0, fill);
            let icon = match key {
                Key::Backspace => Some(Icon::Backspace),
                Key::Left => Some(Icon::Left),
                Key::Right => Some(Icon::Right),
                Key::Enter => Some(Icon::Enter),
                Key::Shift => Some(Icon::Shift),
                _ => None,
            };
            if let Some(icon) = icon {
                icons::paint(&painter, icon, rect.center(), 10.0, theme::TEXT);
            } else {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    &text,
                    FontId::proportional(19.0),
                    theme::TEXT,
                );
            }
            if response.clicked() {
                press(app, key);
            }
        }
    }
}
