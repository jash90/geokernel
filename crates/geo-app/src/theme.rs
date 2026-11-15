//! Colours, sizes and the global egui style: a light theme modelled on the
//! classic desktop look of the reference application.

use eframe::egui::{self, Color32, FontFamily, FontId, TextStyle};

pub const ACCENT: Color32 = Color32::from_rgb(0x65, 0x57, 0xd2);
pub const WHITE: Color32 = Color32::WHITE;
pub const TEXT: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1f);
pub const MUTED: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);
pub const ERROR: Color32 = Color32::from_rgb(0xd9, 0x30, 0x25);

/// The toolbar's and the splitter's background.
pub const TOOLBAR_BG: Color32 = Color32::from_rgb(0xf7, 0xf7, 0xf7);
pub const BORDER: Color32 = Color32::from_rgb(0xd7, 0xd7, 0xd7);
pub const SEPARATOR: Color32 = Color32::from_rgb(0xe3, 0xe3, 0xe3);
/// The border of the active tool group.
pub const TOOL_ACTIVE: Color32 = Color32::from_rgb(0x56, 0x57, 0xf8);
pub const ICON: Color32 = Color32::from_rgb(0x66, 0x66, 0x66);
pub const ICON_DISABLED: Color32 = Color32::from_rgb(0xc8, 0xc8, 0xc8);
pub const HOVER_ROW: Color32 = Color32::from_rgb(0xf0, 0xf0, 0xf2);
pub const SELECTED_ROW: Color32 = Color32::from_rgb(0xe9, 0xe7, 0xfb);

pub const GRID_MINOR: Color32 = Color32::from_rgb(0xea, 0xea, 0xec);
pub const GRID_MAJOR: Color32 = Color32::from_rgb(0xab, 0xaa, 0xb1);
pub const AXIS: Color32 = Color32::from_rgb(0x1a, 0x1a, 0x1c);
pub const TICK_TEXT: Color32 = Color32::from_rgb(0x1a, 0x1a, 0x1c);

/// Default object colours.
pub const FREE_POINT: Color32 = Color32::from_rgb(77, 77, 255);
pub const DEPENDENT_POINT: Color32 = Color32::from_rgb(0x6e, 0x6d, 0x73);
pub const POINT_RING: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1f);
pub const CURVE: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1f);
pub const POLYGON: Color32 = Color32::from_rgb(153, 51, 0);
pub const ANGLE: Color32 = Color32::from_rgb(0, 100, 0);

pub const KEYBOARD_BG: Color32 = Color32::from_rgb(0xf1, 0xf0, 0xf5);
pub const KEY_TAB_ACTIVE: Color32 = Color32::from_rgb(0x98, 0x8c, 0xf7);

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
    v.widgets.hovered.weak_bg_fill = HOVER_ROW;
    v.widgets.hovered.bg_fill = HOVER_ROW;
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
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
        // The fade takes its colour from the parent, which our regions do not set.
        style.spacing.scroll.fade.strength = 0.0;
    });
}
