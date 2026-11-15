//! Named rectangles of what the last frame put on screen. The names and
//! rectangles come from egui itself: the AccessKit tree egui builds from the
//! widgets' own descriptions (labels, roles, bounds), and egui's record of
//! the widgets of the last pass for the few areas that have no description
//! (the algebra rows and the tool hint card). The window keeps no list of
//! its own for this.

use crate::app::App;
use crate::geom::ToEgui;
use crate::theme;
use crate::tools::{Tool, GROUPS};
use eframe::egui::{accesskit, pos2, Context, Rect, Role};
use geo_app_core::text::{self, Label};

/// What a widget is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WidgetKind {
    /// A tool group button of the toolbar (`tool` is the tool it shows).
    ToolGroup,
    /// An entry of an open tool drop-down.
    ToolEntry,
    /// Undo, redo and the main menu at the right of the toolbar.
    ToolbarButton,
    /// The text of an algebra row (a click selects the object).
    Row,
    /// The visibility marker of an algebra row.
    RowMarker,
    /// The menu button of an algebra row.
    RowMenu,
    /// The value field of a free number's row.
    RowValue,
    /// The input bar under the rows.
    InputBar,
    /// The round buttons over the drawing (zoom in and out, fit, full
    /// screen).
    ViewButton,
    /// The style bar toggle and the buttons of the open style bar.
    StyleButton,
    /// A key of the on-screen keyboard, its tabs and its close button.
    Key,
    /// The card with the active tool's name and hint, and its close cross.
    HintCard,
    HintClose,
    /// The notice card and its close button.
    Notice,
    NoticeClose,
    /// Anything else with a name: entries of open menus, dialog fields and
    /// buttons, the load error's close button.
    Other,
}

/// A named rectangle on screen, in window points.
#[derive(Clone, Debug, PartialEq)]
pub struct Widget {
    pub kind: WidgetKind,
    /// The name egui reports for it (its accessible label).
    pub label: String,
    pub rect: Rect,
    pub enabled: bool,
    /// The tool of a tool group button or a drop-down entry.
    pub tool: Option<Tool>,
    /// The toolbar group of a tool group button or a drop-down entry.
    pub group: Option<usize>,
    /// The object of an algebra row, marker, menu or value field.
    pub object: Option<String>,
    /// The state of a toggle (a style bar button, a menu check box).
    pub checked: Option<bool>,
}

impl Widget {
    fn new(kind: WidgetKind, label: &str, rect: Rect) -> Widget {
        Widget {
            kind,
            label: label.to_owned(),
            rect,
            enabled: true,
            tool: None,
            group: None,
            object: None,
            checked: None,
        }
    }
}

fn tool_with_caption(caption: &str) -> Option<Tool> {
    GROUPS
        .iter()
        .flat_map(|g| g.iter().copied())
        .find(|t| t.caption() == caption)
}

fn rect_of(r: accesskit::Rect, ppp: f32) -> Rect {
    Rect::from_min_max(
        pos2(r.x0 as f32 / ppp, r.y0 as f32 / ppp),
        pos2(r.x1 as f32 / ppp, r.y1 as f32 / ppp),
    )
}

/// The widgets of the last frame, ordered top to bottom, then left to
/// right. `update` is the AccessKit tree of that frame.
pub(super) fn collect(
    ctx: &Context,
    app: &App,
    update: Option<&accesskit::TreeUpdate>,
) -> Vec<Widget> {
    let ppp = ctx.pixels_per_point();
    let regions = app.regions;
    let toolbar = regions.toolbar.egui();
    let keyboard = regions.keyboard.map(|k| k.egui());
    let graphics = regions.graphics.egui();
    let notice = app.wb.notice.as_deref();
    let mut out = Vec::new();
    for (_, node) in update.map_or(&[][..], |u| &u.nodes[..]) {
        let role = node.role();
        // Unnamed areas (`Unknown`: the drawing, the algebra rows, the hint
        // card) are found below from egui's widget rectangles.
        if matches!(
            role,
            Role::Window | Role::GenericContainer | Role::Unknown | Role::TextRun
        ) {
            continue;
        }
        let Some(bounds) = node.bounds() else {
            continue;
        };
        // A text field is named by its hint, which egui does not report.
        let label = match node.label() {
            Some(l) => l,
            None if role == Role::TextInput => "",
            None => continue,
        };
        let rect = rect_of(bounds, ppp);
        let center = rect.center();
        let mut w = Widget::new(WidgetKind::Other, label, rect);
        w.enabled = !node.is_disabled();
        w.checked = node.toggled().map(|t| t != accesskit::Toggled::False);
        let in_keyboard = keyboard.is_some_and(|k| k.contains(center));
        // A tool's caption names its group button in the toolbar and its
        // entry in an open drop-down, a wide row; the round zoom buttons
        // over the drawing share two captions with tools.
        let tool = tool_with_caption(label).filter(|_| role == Role::Button);
        let tool = tool.filter(|_| toolbar.contains(center) || rect.width() > 2.0 * rect.height());
        if let Some(tool) = tool {
            w.kind = if toolbar.contains(center) {
                WidgetKind::ToolGroup
            } else {
                WidgetKind::ToolEntry
            };
            w.tool = Some(tool);
            w.group = Some(tool.group());
        } else if role == Role::Button
            && toolbar.contains(center)
            && [Label::Undo, Label::Redo, Label::Menu]
                .iter()
                .any(|&l| text::label(l) == label)
        {
            w.kind = WidgetKind::ToolbarButton;
        } else if in_keyboard {
            w.kind = WidgetKind::Key;
        } else if role == Role::Button
            && [
                Label::ZoomIn,
                Label::ZoomOut,
                Label::ShowAllObjects,
                Label::Fullscreen,
            ]
            .iter()
            .any(|&l| text::label(l) == label)
        {
            w.kind = WidgetKind::ViewButton;
        } else if role == Role::Button
            && graphics.contains(center)
            && [
                Label::StyleBar,
                Label::Axes,
                Label::Grid,
                Label::StandardView,
            ]
            .iter()
            .any(|&l| text::label(l) == label)
        {
            w.kind = WidgetKind::StyleButton;
        } else if label == text::label(Label::LeaveTool) {
            w.kind = WidgetKind::HintClose;
        } else if label == text::label(Label::CloseNotice) {
            w.kind = WidgetKind::NoticeClose;
        } else if Some(label) == notice && graphics.contains(center) {
            w.kind = WidgetKind::Notice;
        } else if role == Role::SpinButton {
            if let Some(object) = label.strip_prefix("Wartość ") {
                w.kind = WidgetKind::RowValue;
                w.object = Some(object.to_owned());
            }
        } else if role == Role::TextInput && regions.algebra.egui().contains(center) {
            w.kind = WidgetKind::InputBar;
            w.label = text::label(Label::InputHint).to_owned();
        }
        out.push(w);
    }
    rows(ctx, app, &mut out);
    hint_card(ctx, app, &mut out);
    out.sort_by(|a, b| {
        (a.rect.top(), a.rect.left(), a.kind, &a.label)
            .partial_cmp(&(b.rect.top(), b.rect.left(), b.kind, &b.label))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// The rectangles egui recorded in the last pass that sense clicks.
fn clickable(ctx: &Context) -> Vec<Rect> {
    ctx.viewport(|v| {
        v.prev_pass
            .widgets
            .layers()
            .flat_map(|(_, ws)| ws.iter())
            .filter(|w| w.sense.senses_click())
            .map(|w| w.rect)
            .collect()
    })
}

/// The algebra rows: the click areas of each row's text, marker and menu
/// button, which egui records with the scroll offset applied. Rows are
/// matched in display order.
fn rows(ctx: &Context, app: &App, out: &mut Vec<Widget>) {
    let area = app.regions.algebra.egui();
    let rects = clickable(ctx);
    let in_column = |r: &Rect, left: f32| (r.left() - left).abs() < 0.5;
    let mut bodies: Vec<Rect> = rects
        .iter()
        .copied()
        .filter(|r| in_column(r, area.left() + theme::MARKER_CELL) && r.right() <= area.right())
        .collect();
    let mut markers: Vec<Rect> = rects
        .iter()
        .copied()
        .filter(|r| in_column(r, area.left()) && (r.width() - theme::MARKER_CELL).abs() < 0.5)
        .collect();
    bodies.sort_by(|a, b| a.top().total_cmp(&b.top()));
    markers.sort_by(|a, b| a.top().total_cmp(&b.top()));
    // One row per object, in construction order (`Workbench::rows`).
    let c = app.wb.construction();
    let labels: Vec<&str> = c.ids().map(|id| c.label(id)).collect();
    if bodies.len() != labels.len() || markers.len() != labels.len() {
        return;
    }
    for ((&label, body), marker) in labels.iter().zip(&bodies).zip(&markers) {
        let mut w = Widget::new(WidgetKind::Row, label, *body);
        w.object = Some(label.to_owned());
        out.push(w);
        let mut m = Widget::new(WidgetKind::RowMarker, label, *marker);
        m.object = Some(label.to_owned());
        out.push(m);
        // The menu button: 30 x 30, 28 points left of the row's right
        // edge, centred on the first line.
        if let Some(kebab) = rects.iter().find(|r| {
            (r.width() - 30.0).abs() < 0.5
                && (r.center().x - (body.right() - 28.0)).abs() < 0.5
                && body.contains(r.center())
        }) {
            let mut k = Widget::new(WidgetKind::RowMenu, label, *kebab);
            k.object = Some(label.to_owned());
            out.push(k);
        }
    }
}

/// The tool hint card at the bottom left of the drawing: the click area
/// egui recorded for it.
fn hint_card(ctx: &Context, app: &App, out: &mut Vec<Widget>) {
    let Some((area, _)) = app.view else {
        return;
    };
    let area = area.egui();
    let (left, top) = (area.left() + 16.0, area.bottom() - 16.0 - 54.0);
    if let Some(card) = clickable(ctx).into_iter().find(|r| {
        (r.left() - left).abs() < 0.5
            && (r.top() - top).abs() < 0.5
            && (r.height() - 54.0).abs() < 0.5
    }) {
        out.push(Widget::new(
            WidgetKind::HintCard,
            app.tool.tool().caption(),
            card,
        ));
    }
}
