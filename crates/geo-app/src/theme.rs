//! Colours, sizes and the global egui style: a light theme with a violet-blue
//! accent.

use eframe::egui::{self, Color32, FontFamily, FontId, TextStyle};

pub const ACCENT: Color32 = Color32::from_rgb(0x65, 0x57, 0xd2);
pub const ACCENT_TINT: Color32 = Color32::from_rgb(0xee, 0xec, 0xfb);
pub const WHITE: Color32 = Color32::WHITE;
pub const TEXT: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
pub const MUTED: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);
pub const TITLE_GREY: Color32 = Color32::from_rgb(0x9a, 0x9a, 0x9a);
pub const SEPARATOR: Color32 = Color32::from_rgb(0xdc, 0xdc, 0xdc);
pub const ERROR: Color32 = Color32::from_rgb(0xd9, 0x30, 0x25);

pub const GRID_MINOR: Color32 = Color32::from_rgb(0xee, 0xee, 0xee);
pub const GRID_MAJOR: Color32 = Color32::from_rgb(0xd2, 0xd2, 0xd2);
pub const AXIS: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
pub const TICK_TEXT: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);

pub const FREE_POINT_FILL: Color32 = Color32::from_rgb(0x4a, 0x90, 0xe2);
pub const FREE_POINT_RING: Color32 = Color32::from_rgb(0x1b, 0x4b, 0x91);
pub const FREE_POINT_LABEL: Color32 = Color32::from_rgb(0x15, 0x65, 0xc0);
pub const DEPENDENT_FILL: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);
pub const DEPENDENT_RING: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
pub const CURVE: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
pub const LABEL: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);

pub const HEADER_HEIGHT: f32 = 65.0;
pub const RAIL_WIDTH: f32 = 72.0;
pub const SIDE_WIDTH: f32 = 324.0;
pub const MARKER_CELL: f32 = 57.0;

/// Installs the light theme and the text sizes.
pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Light);
    let mut v = egui::Visuals::light();
    v.panel_fill = WHITE;
    v.window_fill = WHITE;
    v.extreme_bg_color = WHITE;
    v.selection.bg_fill = ACCENT.gamma_multiply(0.35);
    v.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, SEPARATOR);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0xf0, 0xf0, 0xf0);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0xf0, 0xf0, 0xf0);
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.all_styles_mut(|style| {
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(16.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(15.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        );
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    });
}
