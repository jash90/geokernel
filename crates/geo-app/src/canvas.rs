//! The graphics view: camera, pointer interaction, tools, the buttons over it.

use crate::app::{App, ViewRequest};
use crate::describe;
use crate::draw::{self, Scene};
use crate::floating;
use crate::grid;
use crate::hit;
use crate::icons::Icon;
use crate::interact::{Drag, ViewTransform};
use crate::theme;
use crate::tools::{Pick, Tool};
use crate::view::{self, Camera};
use eframe::egui::{self, pos2, vec2, Rect, Sense, Stroke, Ui};
use geo_kernel::kernel::{Id, Kind};

const POINT_RADIUS: f64 = 11.0;
const CURVE_RADIUS: f64 = 7.0;
const ZOOM_BUTTON: f64 = 1.25;
const ZOOM_TOOL: f64 = 1.5;

pub fn show(app: &mut App, ui: &mut Ui) {
    let size = ui.available_size();
    let (response, painter) = ui.allocate_painter(size, Sense::click_and_drag());
    let rect = response.rect;
    let (w, h) = (rect.width() as f64, rect.height() as f64);
    if w < 1.0 || h < 1.0 {
        return;
    }
    let bounds = view::content_bounds(app.wb.construction());
    let request = app.view_request.take();
    if app.camera.is_none() || request.is_some() {
        app.camera = Some(match request {
            Some(ViewRequest::Standard) => Camera::STANDARD,
            Some(ViewRequest::Fit) => Camera::fit(bounds, w, h),
            Some(ViewRequest::Initial) | None => Camera::initial(bounds, w, h),
        });
    }
    // Ids are only valid until the construction is rebuilt (undo, redo,
    // delete, open): a drag or a half-finished tool must not outlive that.
    if app.seen_rebuilds != app.wb.rebuilds() {
        app.seen_rebuilds = app.wb.rebuilds();
        app.drag = None;
        app.panning = false;
        app.tool.reset();
    }
    let mut cam = app.camera.unwrap_or(Camera::STANDARD);
    let to_local = |pos: egui::Pos2| {
        let d = pos - rect.min;
        (d.x as f64, d.y as f64)
    };

    if response.hovered() {
        let (scroll, pinch) =
            ui.input(|i| (i.smooth_scroll_delta().y as f64, i.zoom_delta() as f64));
        let factor = view::zoom_factor(scroll, pinch);
        if factor != 1.0 {
            let (sx, sy) = response.hover_pos().map_or((0.5 * w, 0.5 * h), to_local);
            cam.zoom_at(factor, sx, sy, w, h);
        }
    }

    let tool = app.tool.tool();
    let move_tool = tool == Tool::Move;
    // The show/hide tool shows hidden objects faintly and lets them be picked.
    let show_all = tool == Tool::ShowHide;
    let t = cam.transform(w, h);
    let free_radius = (draw::FREE_POINT_RADIUS + 6.0) as f64;
    let shown = |app: &App, id: Id| show_all || app.wb.is_visible(id);

    // What the pointer is over (for highlighting and for picking).
    let pick_at = |app: &App, pos: (f64, f64)| -> Option<Id> {
        let c = app.wb.construction();
        hit::pick(c, &t, pos, POINT_RADIUS, CURVE_RADIUS, |id| {
            shown(app, id) && (move_tool || app.tool.accepts_object(c.label(id), c.kind(id)))
        })
    };
    // What a click lands on, whatever the tool: a click on an existing point
    // must never create a new one on top of it.
    let pick_any = |app: &App, pos: (f64, f64)| -> Option<Id> {
        hit::pick(
            app.wb.construction(),
            &t,
            pos,
            POINT_RADIUS,
            CURVE_RADIUS,
            |id| shown(app, id),
        )
    };
    app.hovered = response.hover_pos().and_then(|p| pick_at(app, to_local(p)));

    if response.drag_started() {
        app.drag = None;
        app.panning = false;
        if move_tool && response.dragged_by(egui::PointerButton::Primary) {
            // egui reports the drag a few pixels after the press: grab what
            // was under the pointer when the button went down.
            let origin = ui
                .input(|i| i.pointer.press_origin())
                .or(response.interact_pointer_pos());
            if let Some(pos) = origin {
                let (sx, sy) = to_local(pos);
                app.drag = Drag::begin(app.wb.construction(), &t, sx, sy, free_radius, |id| {
                    app.wb.is_visible(id)
                });
            }
        }
        if let Some(drag) = app.drag {
            app.wb.begin_gesture();
            app.wb.selected = Some(drag.id());
        }
        app.panning = app.drag.is_none();
    }
    if response.dragged() {
        if let Some(drag) = app.drag {
            if let Some(pos) = response.interact_pointer_pos() {
                let (sx, sy) = to_local(pos);
                let (x, y) = drag.target(&t, sx, sy);
                app.wb.move_point(drag.id(), x, y);
            }
        } else if app.panning {
            let d = response.drag_delta();
            cam.pan_pixels(d.x as f64, d.y as f64);
        }
    }
    if response.drag_stopped() {
        app.wb.end_gesture();
        app.drag = None;
        app.panning = false;
    }
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let local = to_local(pos);
            match tool {
                Tool::ZoomIn => cam.zoom_at(ZOOM_TOOL, local.0, local.1, w, h),
                Tool::ZoomOut => cam.zoom_at(1.0 / ZOOM_TOOL, local.0, local.1, w, h),
                Tool::MoveView => {}
                _ => click(app, &t, local, pick_any(app, local)),
            }
        }
    }
    let cursor = match tool {
        _ if app.drag.is_some() || app.panning => Some(egui::CursorIcon::Grabbing),
        Tool::MoveView => Some(egui::CursorIcon::Grab),
        Tool::ZoomIn => Some(egui::CursorIcon::ZoomIn),
        Tool::ZoomOut => Some(egui::CursorIcon::ZoomOut),
        Tool::Move => response
            .hover_pos()
            .filter(|&p| hit_free_point(app, &t, to_local(p), free_radius))
            .map(|_| egui::CursorIcon::Grab),
        _ => response.hover_pos().map(|_| egui::CursorIcon::Crosshair),
    };
    if let (Some(c), true) = (cursor, response.hovered() || app.drag.is_some()) {
        ui.ctx().set_cursor_icon(c);
    }

    app.camera = Some(cam);
    let t = cam.transform(w, h);
    painter.rect_filled(rect, 0.0, theme::WHITE);
    grid::paint(&painter, rect, &t, app.decor);
    let c = app.wb.construction();
    let visible = |id: Id| show_all || app.wb.is_visible(id);
    let ghost = |id: Id| show_all && !app.wb.is_visible(id);
    draw::paint(
        &painter,
        rect,
        c,
        &t,
        &Scene {
            visible: &visible,
            ghost: &ghost,
            selected: app.wb.selected,
            hovered: app.hovered,
        },
    );
    preview(app, &painter, rect, &t, response.hover_pos());
    overlay(app, ui, rect, w, h, &mut cam);
    app.camera = Some(cam);
}

fn hit_free_point(app: &App, t: &ViewTransform, pos: (f64, f64), radius: f64) -> bool {
    crate::interact::hit_test(app.wb.construction(), t, pos.0, pos.1, radius, |id| {
        app.wb.is_visible(id)
    })
    .is_some()
}

/// A click on the drawing area, with the object under it.
fn click(app: &mut App, t: &ViewTransform, pos: (f64, f64), hit: Option<Id>) {
    if app.tool.tool() == Tool::Move {
        app.wb.selected = hit;
        app.style_bar = false;
        return;
    }
    let world = t.screen_to_world(pos.0, pos.1);
    let hit_owned: Option<(String, Kind)> = hit.map(|id| {
        let c = app.wb.construction();
        (c.label(id).to_owned(), c.kind(id))
    });
    // A click as near to a pending new point as to an existing one hits it.
    app.tool.set_tolerance(POINT_RADIUS / t.scale());
    let outcome = {
        let c = app.wb.construction();
        app.tool.click(
            hit_owned.as_ref().map(|(l, k)| (l.as_str(), *k)),
            world,
            describe::click_decimals(t.scale()),
            &|l| c.id(l).is_some(),
        )
    };
    app.apply_outcome(outcome);
}

/// The world position of a pick (an existing point or a new one).
fn pick_position(app: &App, pick: &Pick) -> Option<(f64, f64)> {
    match pick {
        Pick::New { x, y } => Some((*x, *y)),
        Pick::Object { label, .. } => {
            let c = app.wb.construction();
            match c.value(c.id(label)?) {
                geo_kernel::kernel::Value::Point { x, y } => Some((x, y)),
                _ => None,
            }
        }
    }
}

/// New points picked so far, and the object the tool is about to make,
/// drawn from the first pick to the pointer.
fn preview(
    app: &App,
    painter: &egui::Painter,
    rect: Rect,
    t: &ViewTransform,
    pointer: Option<egui::Pos2>,
) {
    let screen = |(x, y): (f64, f64)| {
        let (sx, sy) = t.world_to_screen(x, y);
        rect.min + vec2(sx as f32, sy as f32)
    };
    let picks = app.tool.picks();
    for pick in picks {
        if let Pick::New { x, y } = pick {
            draw::point_shape(
                painter,
                screen((*x, *y)),
                theme::FREE_POINT,
                draw::FREE_POINT_RADIUS,
            );
        }
    }
    if app.tool.tool() == Tool::Polygon && !picks.is_empty() {
        // The sides so far and the one following the pointer, lightly filled.
        let mut pts: Vec<egui::Pos2> = picks
            .iter()
            .filter_map(|p| pick_position(app, p).map(screen))
            .collect();
        pts.extend(pointer.filter(|p| rect.contains(*p)));
        if pts.len() >= 3 {
            let tris: Vec<(f64, f64)> = pts.iter().map(|p| (p.x as f64, p.y as f64)).collect();
            let mut mesh = egui::Mesh::default();
            for &p in &pts {
                mesh.colored_vertex(p, theme::POLYGON.gamma_multiply(0.1));
            }
            for [a, b, c] in crate::shape::triangulate(&tris) {
                mesh.add_triangle(a as u32, b as u32, c as u32);
            }
            painter.add(egui::Shape::mesh(mesh));
        }
        let stroke = Stroke::new(draw::CURVE_WIDTH, theme::POLYGON.gamma_multiply(0.6));
        painter.add(egui::Shape::line(pts, stroke));
        return;
    }
    let (Some(to), [first]) = (pointer.filter(|p| rect.contains(*p)), picks) else {
        return;
    };
    let Some(from) = pick_position(app, first).map(screen) else {
        return;
    };
    let stroke = Stroke::new(draw::CURVE_WIDTH, theme::CURVE.gamma_multiply(0.6));
    let d = to - from;
    match app.tool.tool() {
        Tool::Segment | Tool::Vector => {
            painter.line_segment([from, to], stroke);
        }
        Tool::Line | Tool::Ray if d.length() > 0.5 => {
            let far = d / d.length() * (rect.width() + rect.height());
            let start = if app.tool.tool() == Tool::Line {
                from - far
            } else {
                from
            };
            painter.line_segment([start, from + far], stroke);
        }
        Tool::Circle => {
            painter.circle_stroke(from, d.length(), stroke);
        }
        _ => {}
    }
}

/// The buttons over the drawing area and the tool hint.
fn overlay(app: &mut App, ui: &mut Ui, rect: Rect, w: f64, h: f64, cam: &mut Camera) {
    style_bar(app, ui, rect, cam);
    let right = rect.right() - 35.0;
    let bottom = rect.bottom() - 33.0;
    let buttons = [
        (Icon::Fit, "Pokaż wszystkie obiekty"),
        (Icon::ZoomIn, "Powiększ"),
        (Icon::ZoomOut, "Pomniejsz"),
        (Icon::Fullscreen, "Pełny ekran"),
    ];
    for (i, (icon, tip)) in buttons.into_iter().enumerate() {
        let center = pos2(right, bottom - 44.0 * (3 - i) as f32);
        if !floating::round_button(ui, tip, center, icon, true, tip).clicked() {
            continue;
        }
        match icon {
            Icon::Fit => {
                *cam = Camera::fit(view::content_bounds(app.wb.construction()), w, h);
            }
            Icon::ZoomIn => cam.zoom_at(ZOOM_BUTTON, 0.5 * w, 0.5 * h, w, h),
            Icon::ZoomOut => cam.zoom_at(1.0 / ZOOM_BUTTON, 0.5 * w, 0.5 * h, w, h),
            _ => {
                let full = ui.ctx().input(|i| i.viewport().fullscreen.unwrap_or(false));
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
            }
        }
    }
    if let Some(notice) = app.wb.notice.clone() {
        floating::hint_pill(ui, rect, &notice, theme::ERROR, 0.0);
    }
    if app.tool.tool() != Tool::Move {
        floating::tool_tip(ui, rect, app.tool.tool().caption(), app.tool.hint());
        if let Some(e) = app.tool_error.clone() {
            floating::hint_pill(ui, rect, &e, theme::ERROR, 1.0);
        }
    }
}

/// The toggle in the top right corner and, when open, the bar with the view
/// settings to its left.
fn style_bar(app: &mut App, ui: &mut Ui, rect: Rect, cam: &mut Camera) {
    let toggle = Rect::from_min_size(pos2(rect.right() - 44.0, rect.top()), vec2(44.0, 40.0));
    let response = floating::square_button(ui, "style_bar", toggle, Icon::StyleBar, app.style_bar);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Pasek stylu"));
    if response.on_hover_text("Pasek stylu").clicked() {
        app.style_bar = !app.style_bar;
    }
    if !app.style_bar {
        return;
    }
    let items = [
        (Icon::Axes, "Osie", app.decor.axes),
        (Icon::Grid, "Siatka", app.decor.grid),
        (Icon::Home, "Widok standardowy", false),
    ];
    for (i, (icon, tip, on)) in items.into_iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(
                toggle.left() - 44.0 * (items.len() - i) as f32 - 6.0,
                rect.top(),
            ),
            vec2(44.0, 40.0),
        );
        let response = floating::square_button(ui, tip, r, icon, on);
        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, tip));
        if response.on_hover_text(tip).clicked() {
            match icon {
                Icon::Axes => app.decor.axes = !app.decor.axes,
                Icon::Grid => app.decor.grid = !app.decor.grid,
                _ => *cam = Camera::STANDARD,
            }
        }
    }
}
