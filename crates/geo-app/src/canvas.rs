//! The drawing area: camera, pointer interaction, tools, floating buttons.

use crate::app::{App, ViewRequest};
use crate::describe;
use crate::draw::{self, Scene};
use crate::errors;
use crate::floating;
use crate::grid;
use crate::hit;
use crate::icons::Icon;
use crate::interact::Drag;
use crate::theme;
use crate::tools::{Outcome, Pick, Tool};
use crate::view::{self, Camera};
use eframe::egui::{self, vec2, Color32, Rect, Sense, Ui};
use geo_kernel::kernel::{Id, Kind};

const POINT_RADIUS: f64 = 11.0;
const CURVE_RADIUS: f64 = 7.0;
const ZOOM_BUTTON: f64 = 1.25;

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

    let move_tool = app.tool.tool() == Tool::Move;
    let t = cam.transform(w, h);
    let free_radius = (draw::FREE_POINT_RADIUS + 5.0) as f64;

    // What the pointer is over (for highlighting and for picking).
    let pick_at = |app: &App, pos: (f64, f64)| -> Option<Id> {
        let c = app.wb.construction();
        hit::pick(c, &t, pos, POINT_RADIUS, CURVE_RADIUS, |id| {
            app.wb.is_visible(id) && (move_tool || app.tool.accepts_object(c.label(id), c.kind(id)))
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
            |id| app.wb.is_visible(id),
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
            click(app, &t, to_local(pos), pick_any(app, to_local(pos)));
        }
    }
    if app.drag.is_some() || app.panning {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if let Some(pos) = response.hover_pos() {
        if !move_tool {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        } else if hit_free_point(app, &t, to_local(pos), free_radius) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    app.camera = Some(cam);
    let t = cam.transform(w, h);
    painter.rect_filled(rect, 0.0, theme::WHITE);
    grid::paint(&painter, rect, &t, app.decor);
    let c = app.wb.construction();
    let visible = |id: Id| app.wb.is_visible(id);
    draw::paint(
        &painter,
        rect,
        c,
        &t,
        &Scene {
            visible: &visible,
            selected: app.wb.selected,
            hovered: app.hovered,
        },
    );
    for pick in app.tool.picks() {
        if let Pick::New { x, y } = pick {
            let (sx, sy) = t.world_to_screen(*x, *y);
            painter.circle_filled(
                rect.min + vec2(sx as f32, sy as f32),
                5.0,
                theme::ACCENT.gamma_multiply(0.7),
            );
        }
    }
    shadow_edge(&painter, rect);
    overlay(app, ui, rect, w, h, &mut cam);
    app.camera = Some(cam);
}

fn hit_free_point(
    app: &App,
    t: &crate::interact::ViewTransform,
    pos: (f64, f64),
    radius: f64,
) -> bool {
    crate::interact::hit_test(app.wb.construction(), t, pos.0, pos.1, radius, |id| {
        app.wb.is_visible(id)
    })
    .is_some()
}

/// A click on the drawing area, with the object under it that the active tool
/// can use.
fn click(app: &mut App, t: &crate::interact::ViewTransform, pos: (f64, f64), hit: Option<Id>) {
    if app.tool.tool() == Tool::Move {
        app.wb.selected = hit;
        return;
    }
    let world = t.screen_to_world(pos.0, pos.1);
    let hit_owned: Option<(String, Kind)> = hit.map(|id| {
        let c = app.wb.construction();
        (c.label(id).to_owned(), c.kind(id))
    });
    let outcome = {
        let c = app.wb.construction();
        app.tool.click(
            hit_owned.as_ref().map(|(l, k)| (l.as_str(), *k)),
            world,
            describe::click_decimals(t.scale()),
            &|l| c.id(l).is_some(),
        )
    };
    match outcome {
        Outcome::Run(lines) => match app.wb.run_lines(&lines) {
            Ok(()) => {
                app.tool_error = None;
                app.wb.selected = app.wb.construction().ids().last();
            }
            Err(e) => {
                app.tool_error = Some(errors::polish(&e));
                app.tool.reset();
            }
        },
        Outcome::Delete(label) => {
            if let Some(id) = app.wb.construction().id(&label) {
                app.wb.delete(id);
            }
            app.tool_error = None;
        }
        Outcome::Select(label) => {
            app.wb.selected = app.wb.construction().id(&label);
            app.tool_error = None;
        }
        Outcome::Pending => app.tool_error = None,
        Outcome::Ignored => {}
    }
}

/// The soft shadow the side panel casts on the drawing area.
fn shadow_edge(painter: &egui::Painter, rect: Rect) {
    for i in 0..9 {
        let a = (34.0 * (1.0 - i as f32 / 9.0).powi(2)) as u8;
        let x = rect.left() + i as f32;
        painter.rect_filled(
            Rect::from_min_max(
                egui::pos2(x, rect.top()),
                egui::pos2(x + 1.0, rect.bottom()),
            ),
            0.0,
            Color32::from_black_alpha(a),
        );
    }
}

/// The floating buttons and the tool hint.
fn overlay(app: &mut App, ui: &mut Ui, rect: Rect, w: f64, h: f64, cam: &mut Camera) {
    let margin = 30.0;
    let left = rect.left() + margin + 4.0;
    let top = rect.top() + margin;
    let undo = floating::round_button(
        ui,
        "undo",
        egui::pos2(left, top),
        Icon::Undo,
        app.wb.can_undo(),
        "Cofnij",
    );
    if undo.clicked() {
        app.wb.undo();
    }
    let redo = floating::round_button(
        ui,
        "redo",
        egui::pos2(left + 44.0, top),
        Icon::Redo,
        app.wb.can_redo(),
        "Ponów",
    );
    if redo.clicked() {
        app.wb.redo();
    }
    let gear = floating::round_button(
        ui,
        "gear",
        egui::pos2(rect.right() - margin, top),
        Icon::Gear,
        true,
        "Ustawienia",
    );
    egui::Popup::menu(&gear).show(|ui| {
        ui.set_min_width(140.0);
        ui.checkbox(&mut app.decor.grid, "Siatka");
        ui.checkbox(&mut app.decor.axes, "Osie");
    });
    let right = rect.right() - margin;
    let bottom = rect.bottom() - margin;
    if floating::round_button(
        ui,
        "home",
        egui::pos2(right, bottom),
        Icon::Home,
        true,
        "Widok standardowy",
    )
    .clicked()
    {
        *cam = Camera::STANDARD;
    }
    if floating::round_button(
        ui,
        "zoom_out",
        egui::pos2(right, bottom - 44.0),
        Icon::ZoomOut,
        true,
        "Pomniejsz",
    )
    .clicked()
    {
        cam.zoom_at(1.0 / ZOOM_BUTTON, 0.5 * w, 0.5 * h, w, h);
    }
    if floating::round_button(
        ui,
        "zoom_in",
        egui::pos2(right, bottom - 88.0),
        Icon::ZoomIn,
        true,
        "Powiększ",
    )
    .clicked()
    {
        cam.zoom_at(ZOOM_BUTTON, 0.5 * w, 0.5 * h, w, h);
    }
    if let Some(notice) = app.wb.notice.clone() {
        floating::hint_pill(ui, rect, &notice, theme::ERROR, 2.0);
    }
    if app.tool.tool() != Tool::Move {
        floating::hint_pill(ui, rect, app.tool.hint(), theme::TEXT, 0.0);
        if let Some(e) = app.tool_error.clone() {
            floating::hint_pill(ui, rect, &e, theme::ERROR, 1.0);
        }
    }
}
