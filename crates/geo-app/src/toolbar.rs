//! The toolbar across the top: one button per tool group, each with a
//! drop-down of the group's tools; undo, redo and the main menu at the right.

use crate::app::{App, ViewRequest};
use crate::icons::{self, Icon};
use crate::theme;
use crate::tool_icons;
use crate::tools::{Tool, GROUPS};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Rect, Response, Sense, Stroke, Ui,
};
use geo_app_core::metrics::Metrics;
use geo_app_core::text::{self, Label};

const FIRST_LEFT: f32 = 5.0;
const ITEM_HEIGHT: f32 = 44.0;
/// The room the undo, redo and menu buttons take at the right.
const RIGHT_BUTTONS: f32 = 146.0;

/// The rectangle of group button `i` in the strip `bar`.
pub fn button_rect(bar: Rect, i: usize, m: &Metrics) -> Rect {
    Rect::from_min_size(
        pos2(
            bar.left() + FIRST_LEFT + m.toolbar_pitch * i as f32,
            bar.center().y - m.toolbar_button / 2.0,
        ),
        vec2(m.toolbar_button, m.toolbar_button),
    )
}

/// Centres of the undo, redo and menu buttons at the right of `bar`.
pub fn right_buttons(bar: Rect) -> [egui::Pos2; 3] {
    let y = bar.center().y;
    [
        pos2(bar.right() - 126.0, y),
        pos2(bar.right() - 86.0, y),
        pos2(bar.right() - 32.0, y),
    ]
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let bar = ui.max_rect();
    let painter = ui.painter().clone();
    painter.rect_filled(bar, 0.0, theme::TOOLBAR_BG);
    painter.hline(
        bar.x_range(),
        bar.bottom() - 0.5,
        Stroke::new(1.0, theme::BORDER),
    );
    // With a profile that adapts to narrow windows (touch), the tool groups
    // scroll sideways when they do not fit beside the buttons at the right;
    // otherwise (always on the desktop) they are laid out straight into the
    // bar, as they always were.
    let strip = FIRST_LEFT + app.metrics.toolbar_pitch * GROUPS.len() as f32;
    let room = bar.width() - RIGHT_BUTTONS;
    if app.metrics.portrait_below > 0.0 && strip > room && room > 0.0 {
        let area = Rect::from_min_size(bar.min, vec2(room, bar.height()));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area).id_salt("groups"));
        child.set_clip_rect(area.intersect(ui.clip_rect()));
        egui::ScrollArea::horizontal()
            .id_salt("groups")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(&mut child, |ui| {
                let (inner, _) = ui.allocate_exact_size(vec2(strip, bar.height()), Sense::hover());
                group_buttons(app, ui, inner);
            });
    } else {
        group_buttons(app, ui, bar);
    }
    let [undo, redo, menu] = right_buttons(bar);
    if flat_button(
        ui,
        "undo",
        undo,
        Icon::Undo,
        app.can_undo(),
        text::label(Label::Undo),
    )
    .clicked()
    {
        app.undo();
    }
    if flat_button(
        ui,
        "redo",
        redo,
        Icon::Redo,
        app.wb.can_redo(),
        text::label(Label::Redo),
    )
    .clicked()
    {
        app.redo();
    }
    let menu = flat_button(ui, "menu", menu, Icon::Menu, true, text::label(Label::Menu));
    main_menu(app, &menu);
}

/// The tool group buttons laid out from the left of `bar`.
fn group_buttons(app: &mut App, ui: &mut Ui, bar: Rect) {
    let painter = ui.painter().clone();
    for (i, group) in GROUPS.iter().enumerate() {
        let shown = app.group_tools[i];
        let rect = button_rect(bar, i, &app.metrics);
        let response = ui
            .interact(rect, ui.scope_id().with(("group", i)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, shown.caption()));
        let active = group.contains(&app.tool.tool());
        painter.rect_filled(rect, 2.0, theme::WHITE);
        if active {
            painter.rect_stroke(
                rect.shrink(1.0),
                2.0,
                Stroke::new(2.0, theme::TOOL_ACTIVE),
                egui::StrokeKind::Inside,
            );
        } else {
            painter.rect_stroke(
                rect,
                2.0,
                Stroke::new(1.0, theme::BORDER),
                egui::StrokeKind::Inside,
            );
        }
        tool_icons::paint(&painter, shown, rect.center(), 15.0);
        if response.clicked() {
            app.select_tool(shown);
        }
        if group.len() > 1 {
            // A small corner mark: the button opens a list.
            let c = rect.right_bottom() - vec2(5.0, 5.0);
            painter.add(egui::Shape::convex_polygon(
                vec![c, c - vec2(5.0, 0.0), c - vec2(0.0, 5.0)],
                Color32::from_gray(0x90),
                Stroke::NONE,
            ));
        }
        if group.len() > 1 {
            dropdown(app, &response, group);
        }
    }
}

/// The list of a group's tools under its button.
fn dropdown(app: &mut App, button: &Response, group: &[Tool]) {
    egui::Popup::menu(button)
        .gap(4.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            for &tool in group {
                let (rect, response) =
                    ui.allocate_exact_size(vec2(260.0, ITEM_HEIGHT), Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, true, tool.caption())
                });
                let painter = ui.painter();
                if app.tool.tool() == tool {
                    painter.rect_filled(rect, 0.0, theme::SELECTED_ROW);
                } else if response.hovered() {
                    painter.rect_filled(rect, 0.0, theme::HOVER_ROW);
                }
                tool_icons::paint(painter, tool, rect.left_center() + vec2(26.0, 0.0), 14.0);
                painter.text(
                    rect.left_center() + vec2(56.0, 0.0),
                    Align2::LEFT_CENTER,
                    tool.caption(),
                    FontId::proportional(16.0),
                    theme::TEXT,
                );
                if response.clicked() {
                    app.select_tool(tool);
                    ui.close();
                }
            }
        });
}

/// A borderless icon button.
fn flat_button(
    ui: &mut Ui,
    key: &str,
    center: egui::Pos2,
    icon: Icon,
    enabled: bool,
    tip: &str,
) -> Response {
    let rect = Rect::from_center_size(center, vec2(36.0, 36.0));
    let response = ui.interact(rect, ui.scope_id().with(key), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, tip));
    let painter = ui.painter();
    if enabled && response.hovered() {
        painter.circle_filled(center, 18.0, theme::HOVER_ROW);
    }
    let color = if enabled {
        theme::ICON
    } else {
        theme::ICON_DISABLED
    };
    icons::paint(painter, icon, center, 13.0, color);
    if enabled {
        response
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(tip)
    } else {
        response
    }
}

fn main_menu(app: &mut App, button: &Response) {
    egui::Popup::menu(button).show(|ui| {
        ui.set_min_width(200.0);
        ui.menu_button(text::label(Label::File), |ui| {
            if ui.button(text::label(Label::SaveAsGgb)).clicked() {
                app.save_path = Some(crate::app::default_save_path(app.source_path.as_deref()));
                ui.close();
            }
        });
        let view = if app.show_cas {
            Label::AlgebraView
        } else {
            Label::CasView
        };
        if ui.button(text::label(view)).clicked() {
            app.show_cas = !app.show_cas;
            app.show_spreadsheet = false;
            app.cancel_spreadsheet_range_extension();
            app.cancel_cas_edit();
            app.keyboard.open = false;
            ui.close();
        }
        let view = if app.show_spreadsheet {
            Label::AlgebraView
        } else {
            Label::SpreadsheetView
        };
        if ui.button(text::label(view)).clicked() {
            app.show_spreadsheet = !app.show_spreadsheet;
            app.show_cas = false;
            app.cancel_cas_edit();
            app.cancel_spreadsheet_edit();
            app.keyboard.open = false;
            ui.close();
        }
        ui.checkbox(&mut app.decor.axes, text::label(Label::Axes));
        ui.checkbox(&mut app.decor.grid, text::label(Label::Grid));
        if ui.button("Widok standardowy").clicked() {
            app.view_request = Some(ViewRequest::Standard);
            ui.close();
        }
        if ui.button("Pokaż wszystkie obiekty").clicked() {
            app.view_request = Some(ViewRequest::Fit);
            ui.close();
        }
        ui.separator();
        let label = if app.keyboard.open {
            text::label(Label::CloseKeyboard)
        } else {
            text::label(Label::OpenKeyboard)
        };
        if ui.button(label).clicked() {
            app.keyboard.open = !app.keyboard.open;
            ui.close();
        }
    });
}
