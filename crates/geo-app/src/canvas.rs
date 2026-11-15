//! The graphics view: camera, pointer interaction, tools, the buttons over it.

use crate::app::{App, ViewRequest};
use crate::describe;
use crate::draw::{self, Scene};
use crate::floating;
use crate::geom::{ToCore, ToEgui};
use crate::gesture::{Button, Event as GestureEvent, Gesture, Grab, Intents, TouchPhase, MOUSE};
use crate::grid;
use crate::hit;
use crate::icons::Icon;
use crate::interact::{Drag, ViewTransform};
use crate::theme;
use crate::tools::{Pick, Tool};
use crate::view::{self, Camera};
use eframe::egui::{self, pos2, vec2, Rect, Sense, Stroke, Ui};
use geo_app_core::text::{self, Label};
use geo_kernel::kernel::{Id, Kind};

const ZOOM_BUTTON: f64 = 1.25;
const ZOOM_TOOL: f64 = 1.5;
/// How many places where a press or a finger went down in one frame are
/// checked against what lies over the drawing; any further one is not the
/// drawing's.
const STARTS: usize = 8;

pub fn show(app: &mut App, ui: &mut Ui) {
    let size = ui.available_size();
    let (response, painter) = ui.allocate_painter(size, Sense::click_and_drag());
    let rect = response.rect;
    let (w, h) = (rect.width() as f64, rect.height() as f64);
    // A press, or a finger going down, belongs to the area when it lands
    // on it, no area (a menu, a dialog) lies over it there and no widget
    // over the drawing (a button, the notice, the tool hint) is there:
    // decided for each one where it happened (egui's own hit test is
    // private and made for one position a frame). The widgets are those
    // painted in the last frame, which is what the user saw. egui's lock is
    // held while the events are read, so the places are read first (into a
    // fixed array, nothing when no press came) and asked about outside it.
    // The first finger's touch and the press egui-winit makes of it share
    // one place.
    let mut starts: [Option<egui::Pos2>; STARTS] = [None; STARTS];
    ui.input(|i| {
        let found = i.events.iter().filter_map(|e| match *e {
            egui::Event::PointerButton {
                pos, pressed: true, ..
            }
            | egui::Event::Touch {
                pos,
                phase: egui::TouchPhase::Start,
                ..
            } => Some(pos),
            _ => None,
        });
        let mut n = 0;
        for pos in found {
            if n == STARTS {
                break;
            }
            if !starts[..n].contains(&Some(pos)) {
                starts[n] = Some(pos);
                n += 1;
            }
        }
    });
    let owned = starts.map(|p| {
        p.is_some_and(|p| {
            rect.contains(p)
                && ui.ctx().layer_id_at(p) == Some(response.layer_id)
                && !app.overlays.contains(p.core())
        })
    });
    // A place beyond the array (more than `STARTS` in one frame, never seen
    // in practice) is not found and so is not the drawing's: the safe side,
    // as nothing was checked there.
    let owns = |pos: egui::Pos2| {
        starts
            .iter()
            .position(|s| *s == Some(pos))
            .is_some_and(|i| owned[i])
    };
    let mut gesture = app.gesture;
    let fed = ui.input(|i| {
        gesture.begin_frame(i.time);
        feed(&mut gesture, &i.events, owns)
    });
    if w < 1.0 || h < 1.0 {
        // Nothing can be pressed on an area this small; a press held when
        // it shrank ends as a cancel.
        let intents = gesture.cancel();
        app.gesture = gesture;
        app.overlays.clear();
        end_drag(app, &intents);
        if fed.released {
            app.press_view = None;
        }
        return;
    }
    // Opening or closing the keyboard, or resizing the window, keeps the
    // top left corner of the drawing in place, not its centre. A view
    // request is made for the new size, so it comes after.
    if let (Some(cam), Some((old, _))) = (app.camera, app.view) {
        let old = old.egui();
        if old.size() != rect.size() {
            app.camera = Some(cam.resized((old.width() as f64, old.height() as f64), (w, h)));
        }
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
        gesture.forget_target();
        app.tool.reset();
    }
    let mut cam = app.camera.unwrap_or(Camera::STANDARD);
    let to_local = |pos: egui::Pos2| {
        let d = pos - rect.min;
        (d.x as f64, d.y as f64)
    };

    let zoom = if response.hovered() {
        let (scroll, pinch, touch) = ui.input(|i| {
            (
                i.smooth_scroll_delta().y as f64,
                i.zoom_delta() as f64,
                i.multi_touch(),
            )
        });
        let factor = view::zoom_factor(scroll, pinch);
        match touch {
            Some(t) => gesture.pinch(
                factor,
                t.center_pos.core(),
                (t.translation_delta.x, t.translation_delta.y),
            ),
            None => {
                let center = response.hover_pos().unwrap_or(rect.center());
                gesture.zoom(factor, center.core())
            }
        }
    } else {
        None
    };
    if let Some(z) = zoom {
        let (sx, sy) = to_local(z.center.egui());
        if z.factor != 1.0 {
            cam.zoom_at(z.factor, sx, sy, w, h);
        }
        if z.pan != (0.0, 0.0) {
            cam.pan_pixels(z.pan.0 as f64, z.pan.1 as f64);
        }
    }

    let tool = app.tool.tool();
    let move_tool = tool == Tool::Move;
    // The show/hide tool shows hidden objects faintly and lets them be picked.
    let show_all = tool == Tool::ShowHide;
    let t = cam.transform(w, h);
    // The hit radii of the profile (three numbers read once per frame).
    let (point_r, curve_r, grab_r) = (
        app.metrics.point_radius,
        app.metrics.curve_radius,
        app.metrics.grab_radius,
    );
    let shown = |app: &App, id: Id| show_all || app.wb.is_visible(id);

    // What the pointer is over (for highlighting and for picking).
    let pick_at = |app: &App, pos: (f64, f64)| -> Option<Id> {
        let c = app.wb.construction();
        hit::pick(c, &t, pos, point_r, curve_r, |id| {
            shown(app, id) && (move_tool || app.tool.accepts_object(c.label(id), c.kind(id)))
        })
    };
    app.hovered = response.hover_pos().and_then(|p| pick_at(app, to_local(p)));

    // Remember what the user saw when the button went down (the area may
    // change in this very frame, e.g. the keyboard closes on a press). The
    // press is read from this frame's events: when press and release come
    // in one frame (a touch tap) egui keeps no press origin.
    // A tap in the frame of a later press (release, then press) belongs to
    // the earlier press and its view.
    let tap_view = app.press_view;
    let tap_before_press = fed.pressed.is_some() && !fed.released;
    if let Some(pos) = fed.pressed {
        app.press_view = app
            .view
            .filter(|(r, _)| r.contains(pos.core()))
            .or_else(|| rect.contains(pos).then_some((rect.core(), t)));
    }
    // A press position in the area as it was at the press, and that
    // transform.
    let at_view = |view: Option<(geo_app_core::geom::Rect, ViewTransform)>, pos: egui::Pos2| {
        let (r, pt) = view.unwrap_or((rect.core(), t));
        let d = pos - r.min.egui();
        ((d.x as f64, d.y as f64), pt)
    };
    let at_press = |app: &App, pos: egui::Pos2| at_view(app.press_view, pos);

    // A drag that starts takes hold of what was under the pointer when the
    // button went down: a free point (with the move tool), or the view. A
    // press on a point never pans: a dependent point stays where it is,
    // unless the move-view tool pans from anywhere.
    let mut grabbed = None;
    let intents = gesture.end_frame(response.contains_pointer(), |origin| {
        let ((sx, sy), pt) = at_press(app, origin.egui());
        if move_tool {
            grabbed = Drag::begin(app.wb.construction(), &pt, sx, sy, grab_r, |id| {
                app.wb.is_visible(id)
            });
        }
        let c = app.wb.construction();
        let on_point = hit::pick(c, &pt, (sx, sy), point_r, curve_r, |id| {
            shown(app, id) && c.kind(id) == Kind::Point
        })
        .is_some();
        if grabbed.is_some() {
            Grab::Point
        } else if !on_point || tool == Tool::MoveView {
            Grab::Pan
        } else {
            Grab::Nothing
        }
    });
    app.gesture = gesture;
    end_drag(app, &intents);
    if intents.started.is_some() {
        app.drag = grabbed;
        if let Some(drag) = grabbed {
            app.wb.begin_gesture();
            app.wb.selected = Some(drag.id());
        }
    }
    if let (Some(pos), Some(drag)) = (intents.drag_to, app.drag) {
        let (sx, sy) = to_local(pos.egui());
        let (x, y) = drag.target(&t, sx, sy);
        app.wb.move_point(drag.id(), x, y);
    }
    if let Some((dx, dy)) = intents.pan {
        cam.pan_pixels(dx as f64, dy as f64);
    }
    // egui decides which widget a click belongs to as well; a press on a
    // widget over the area made no tap here in the first place.
    let tap = intents.tap.filter(|_| response.clicked());
    if let Some(pos) = tap.map(ToEgui::egui) {
        let local = to_local(pos);
        match tool {
            Tool::ZoomIn => cam.zoom_at(ZOOM_TOOL, local.0, local.1, w, h),
            Tool::ZoomOut => cam.zoom_at(1.0 / ZOOM_TOOL, local.0, local.1, w, h),
            Tool::MoveView => {}
            _ => {
                let view = if tap_before_press {
                    tap_view
                } else {
                    app.press_view
                };
                let (local, pt) = at_view(view, pos);
                let c = app.wb.construction();
                let hit = hit::pick(c, &pt, local, point_r, curve_r, |id| shown(app, id));
                click(app, &pt, local, hit);
            }
        }
    }
    let cursor = match tool {
        _ if app.drag.is_some() || app.gesture.panning() => Some(egui::CursorIcon::Grabbing),
        Tool::MoveView => Some(egui::CursorIcon::Grab),
        Tool::ZoomIn => Some(egui::CursorIcon::ZoomIn),
        Tool::ZoomOut => Some(egui::CursorIcon::ZoomOut),
        Tool::Move => response
            .hover_pos()
            .filter(|&p| hit_free_point(app, &t, to_local(p), grab_r))
            .map(|_| egui::CursorIcon::Grab),
        _ => response.hover_pos().map(|_| egui::CursorIcon::Crosshair),
    };
    if let (Some(c), true) = (cursor, response.hovered() || app.drag.is_some()) {
        ui.ctx().set_cursor_icon(c);
    }

    app.camera = Some(cam);
    let t = cam.transform(w, h);
    // Whoever got the press (this area, a button over it, a dialog), it
    // ends with the release.
    if fed.released {
        app.press_view = None;
    }
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
            palette: app.wb.palette(),
            cull: true,
            generation: app.wb.rebuilds(),
        },
    );
    preview(app, &painter, rect, &t, response.hover_pos());
    overlay(app, ui, rect, w, h, &mut cam);
    app.camera = Some(cam);
    app.view = Some((rect.core(), cam.transform(w, h)));
}

/// Puts back or ends the drag that the automaton ended.
fn end_drag(app: &mut App, intents: &Intents) {
    if intents.cancel_drag && app.drag.take().is_some() {
        app.wb.cancel_gesture();
    }
    if intents.end_drag {
        app.wb.end_gesture();
        app.drag = None;
    }
}

/// What `feed` saw of the primary button.
struct Fed {
    /// The position of the last primary press among the events.
    pressed: Option<egui::Pos2>,
    /// The last primary button event was a release.
    released: bool,
}

/// Gives this frame's pointer events to the automaton, in order. Each press,
/// and each finger going down, is the area's when `owns` says so at its
/// position; any other press only moves the pointer.
fn feed(gesture: &mut Gesture, events: &[egui::Event], owns: impl Fn(egui::Pos2) -> bool) -> Fed {
    let mut fed = Fed {
        pressed: None,
        released: false,
    };
    for event in events {
        match *event {
            egui::Event::PointerMoved(pos) => gesture.event(GestureEvent::Move {
                pos: pos.core(),
                id: MOUSE,
            }),
            egui::Event::PointerButton {
                pos,
                button,
                pressed: down,
                ..
            } => {
                let primary = button == egui::PointerButton::Primary;
                if primary {
                    if down {
                        fed.pressed = Some(pos);
                    }
                    fed.released = !down;
                }
                let button = if primary {
                    Button::Primary
                } else {
                    Button::Other
                };
                let core = pos.core();
                gesture.event(if !down {
                    GestureEvent::Up {
                        pos: core,
                        id: MOUSE,
                        button,
                    }
                } else if owns(pos) {
                    GestureEvent::Down {
                        pos: core,
                        id: MOUSE,
                        button,
                    }
                } else {
                    GestureEvent::Move {
                        pos: core,
                        id: MOUSE,
                    }
                });
            }
            egui::Event::PointerGone => gesture.event(GestureEvent::Gone),
            egui::Event::WindowFocused(false) => gesture.event(GestureEvent::FocusLost),
            egui::Event::Touch { id, phase, pos, .. } => {
                let phase = match phase {
                    egui::TouchPhase::Start => TouchPhase::Start,
                    egui::TouchPhase::Move => TouchPhase::Move,
                    egui::TouchPhase::End => TouchPhase::End,
                    egui::TouchPhase::Cancel => TouchPhase::Cancel,
                };
                gesture.touch(id.0, phase, phase == TouchPhase::Start && owns(pos));
            }
            _ => {}
        }
    }
    fed
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

/// The world position of a picked point.
fn pick_position(app: &App, pick: &Pick) -> Option<(f64, f64)> {
    let c = app.wb.construction();
    match c.value(c.id(pick.label())?) {
        geo_kernel::kernel::Value::Point { x, y } => Some((x, y)),
        _ => None,
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
    app.overlays.clear();
    style_bar(app, ui, rect, cam);
    let right = rect.right() - 35.0;
    let bottom = rect.bottom() - 33.0;
    let buttons = [
        (Icon::Fit, text::label(Label::ShowAllObjects)),
        (Icon::ZoomIn, text::label(Label::ZoomIn)),
        (Icon::ZoomOut, text::label(Label::ZoomOut)),
        (Icon::Fullscreen, text::label(Label::Fullscreen)),
    ];
    for (i, (icon, tip)) in buttons.into_iter().enumerate() {
        let center = pos2(right, bottom - 44.0 * (3 - i) as f32);
        if !floating::round_button(ui, &mut app.overlays, tip, center, icon, true, tip).clicked() {
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
    // The hint of the move tool is for a pointer that cannot hover; any
    // other tool shows its hint with a button that leaves it.
    let other_tool = app.tool.tool() != Tool::Move;
    let tip = other_tool || !app.metrics.hover;
    if let Some(notice) = app.wb.notice.clone() {
        let lift = if tip { 66.0 } else { 0.0 };
        if floating::snackbar(ui, &mut app.overlays, rect, &notice, lift) {
            app.wb.notice = None;
        }
    }
    if tip {
        let close = floating::tool_tip(
            ui,
            &mut app.overlays,
            rect,
            app.tool.tool().caption(),
            app.tool.hint(),
            other_tool,
        );
        if other_tool {
            if let Some(e) = app.tool_error.clone() {
                floating::hint_pill(ui, &mut app.overlays, rect, &e, theme::ERROR, 1.0);
            }
        }
        if close.is_some_and(|c| c.clicked()) {
            app.leave_tool();
        }
    }
}

/// The toggle in the top right corner and, when open, the bar with the view
/// settings to its left.
fn style_bar(app: &mut App, ui: &mut Ui, rect: Rect, cam: &mut Camera) {
    let toggle = Rect::from_min_size(pos2(rect.right() - 44.0, rect.top()), vec2(44.0, 40.0));
    let response = floating::square_button(
        ui,
        &mut app.overlays,
        "style_bar",
        toggle,
        Icon::StyleBar,
        app.style_bar,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, text::label(Label::StyleBar))
    });
    if response.on_hover_text("Pasek stylu").clicked() {
        app.style_bar = !app.style_bar;
    }
    if !app.style_bar {
        return;
    }
    let items = [
        (Icon::Axes, text::label(Label::Axes), app.decor.axes),
        (Icon::Grid, text::label(Label::Grid), app.decor.grid),
        (Icon::Home, text::label(Label::StandardView), false),
    ];
    for (i, (icon, tip, on)) in items.into_iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(
                toggle.left() - 44.0 * (items.len() - i) as f32 - 6.0,
                rect.top(),
            ),
            vec2(44.0, 40.0),
        );
        let response = floating::square_button(ui, &mut app.overlays, tip, r, icon, on);
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
