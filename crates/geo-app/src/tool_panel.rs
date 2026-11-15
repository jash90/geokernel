//! The tools panel: a grid of tool buttons.

use crate::app::App;
use crate::theme;
use crate::tool_icons;
use crate::tools::Tool;
use eframe::egui::{vec2, Align2, Color32, CursorIcon, FontId, Rect, Sense, Ui};

const COLUMNS: usize = 3;
const PAD: f32 = 12.0;
const TILE_HEIGHT: f32 = 94.0;

pub fn show(app: &mut App, ui: &mut Ui) {
    ui.add_space(PAD);
    let tile_width = (ui.available_width() - 2.0 * PAD) / COLUMNS as f32;
    for chunk in Tool::ALL.chunks(COLUMNS) {
        let (row, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), TILE_HEIGHT), Sense::hover());
        for (i, &tool) in chunk.iter().enumerate() {
            let rect = Rect::from_min_size(
                row.min + vec2(PAD + i as f32 * tile_width, 0.0),
                vec2(tile_width, TILE_HEIGHT),
            )
            .shrink(3.0);
            let response = ui
                .interact(
                    rect,
                    ui.scope_id().with(("tool", i, tool as u8)),
                    Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand);
            let active = app.tool.tool() == tool;
            let painter = ui.painter();
            if active {
                painter.rect_filled(rect, 10.0, theme::ACCENT_TINT);
            } else if response.hovered() {
                painter.rect_filled(rect, 10.0, Color32::from_rgb(0xf3, 0xf3, 0xf3));
            }
            let (ink, accent) = if active {
                (theme::ACCENT, theme::ACCENT)
            } else {
                (Color32::from_rgb(0x55, 0x55, 0x55), theme::FREE_POINT_FILL)
            };
            tool_icons::paint(
                painter,
                tool,
                rect.center_top() + vec2(0.0, 32.0),
                17.0,
                ink,
                accent,
            );
            painter.text(
                rect.center_top() + vec2(0.0, 70.0),
                Align2::CENTER_CENTER,
                tool.caption(),
                FontId::proportional(13.0),
                if active { theme::ACCENT } else { theme::TEXT },
            );
            if response.clicked() {
                app.tool.set_tool(tool);
                app.tool_error = None;
            }
        }
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(PAD + 4.0);
        ui.add(eframe::egui::Label::new(
            eframe::egui::RichText::new(app.tool.hint())
                .size(14.0)
                .color(theme::MUTED),
        ));
    });
}
