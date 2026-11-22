//! The graphics view: camera, pointer interaction, tools, the buttons over it.

use crate::app::{App, ViewRequest};
use crate::draw::{self, Scene};
use crate::floating;
use crate::geom::{ToCore, ToEgui};
use crate::gesture::{Button, Event as GestureEvent, Gesture, Grab, Intents, TouchPhase, MOUSE};
use crate::grid;
use crate::hit;
use crate::icons::Icon;
use crate::interact::{self, Drag, Hold, Step, ViewTransform};
use crate::theme;
use crate::tools::{Pick, Tool};
use crate::view::{self, Camera};
use eframe::egui::{self, pos2, vec2, Rect, Sense, Stroke, Ui};
use geo_app_core::text::{self, Label};
use geo_kernel::kernel::{Id, Kind};

const ZOOM_BUTTON: f64 = 1.25;
const ZOOM_TOOL: f64 = 1.5;
const LOCUS_SCALE_RATIO: f64 = 1.5;
const LOCUS_SAMPLE_INTERVAL: f64 = 0.08;
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
    let request = app.view_request.take();
    if app.camera.is_none() || request.is_some() {
        let bounds = view::content_bounds(app.wb.construction());
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
        app.tool_press = None;
        app.label_rects.borrow_mut().clear();
        app.text_rects.borrow_mut().clear();
        gesture.forget_target();
        // A tool's picks are labels: they outlive the rebuild unless one
        // of them went (a change from outside takes an unfinished tool use
        // back, a failed one puts it back as it was).
        let c = app.wb.construction();
        if app.tool.picks().iter().any(|p| c.id(p.label()).is_none()) {
            app.reset_tool();
        }
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
    // A tool that builds from picks takes them when the button goes down
    // (spec E1); the delete and show/hide tools act on a click.
    let press_tool = !matches!(
        tool,
        Tool::Move | Tool::MoveView | Tool::ZoomIn | Tool::ZoomOut | Tool::Delete | Tool::ShowHide
    );
    // The show/hide tool shows hidden objects faintly and lets them be picked.
    let show_all = tool == Tool::ShowHide;
    let t = cam.transform(w, h);
    // How far a pointer reaches (`App.getCapturingThreshold` by the
    // pointer's type): the hover is the mouse's; a press is its pointer's.
    let hover_reach = hit::Reach::of(&app.metrics, false);
    let press_reach = hit::Reach::of(&app.metrics, gesture.touch_press());
    let shown = |app: &App, id: Id| show_all || app.wb.is_visible(id);

    // What the pointer is over (for highlighting and for picking).
    let pick_at = |app: &App, pos: (f64, f64)| -> Option<Id> {
        let c = app.wb.construction();
        let accept = |id| shown(app, id) && (move_tool || app.accepts_tool_hit(id));
        let text = app.text_at(pos, app.wb.selected, accept);
        hit::pick_with_text(c, &t, pos, &hover_reach, app.wb.selected, text, accept)
    };
    // What a press of the move tool at `pos` (in the area as it was, with
    // the transform `pt`) takes hold of.
    let target = |app: &App, pt: &ViewTransform, pos: (f64, f64)| -> Target {
        let c = app.wb.construction();
        let accept = |id| shown(app, id);
        let text = app.text_at(pos, app.wb.selected, accept);
        let Some(id) = hit::pick_with_text(c, pt, pos, &press_reach, app.wb.selected, text, accept)
        else {
            return Target::Nothing;
        };
        // A side of a polygon under the pointer: the owner's rule
        // (`hit::SIDE_TAKES_THE_POLYGON`) takes the polygon; the reference
        // app does nothing with such a press.
        if let Some(p) = app.wb.polygon_of_side(id).filter(|&p| shown(app, p)) {
            let (t0, retry) = (press_reach.threshold, press_reach.retry);
            let hits = |th: f64| hit::hit(c, p, pt, pos, th, &press_reach).is_some();
            if hits(t0) || retry.is_some_and(hits) {
                return if app.rules.side_takes_the_polygon {
                    Target::Object(p)
                } else {
                    Target::Inert
                };
            }
        }
        Target::Object(id)
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

    // The move tool selects what a press takes hold of when the button goes
    // down (`handleMousePressedForMoveMode`), whether it then drags or not;
    // a press on a label takes the label and selects nothing.
    if let (true, Some(pos)) = (move_tool, fed.owned) {
        let (local, pt) = at_press(app, pos);
        if app.label_at(local).is_none() {
            if let Target::Object(id) = target(app, &pt, local) {
                app.wb.selected = Some(id);
            }
        }
    }

    // A press of this frame picks at once (spec E1, E2); the release of an
    // earlier press ends that one first (spec E3). Without a new press, the
    // release waits until the drag it ends took hold of what that press
    // made.
    if let (Some(pos), Some(_)) = (fed.early_release, fed.owned) {
        release_tool(app, &t, to_local(pos), &press_reach);
    }
    if let (true, Some(pos)) = (press_tool, fed.owned) {
        let (local, pt) = at_press(app, pos);
        app.tool_press = Some(app.press_tool(&pt, local, &press_reach));
    }

    // A drag that starts takes hold of what was under the pointer when the
    // button went down. With the move tool: a label first
    // (`handleMousePressedForMoveMode` asks `getLabelHit` before anything
    // else), then what the press chose, if it can move; a press on any
    // object never pans, only one that hits nothing does
    // (`hasNoHitsDisablingModeForShallMoveView`). The move-view tool
    // pans from anywhere. The delete tool's drag erases (spec E11). The
    // point tool drags the point its press made or found (spec E4); a press
    // of another tool that picked does not pan; else a press on a point
    // does not pan.
    let mut grabbed = None;
    let mut refused = None;
    let mut erase = false;
    let intents = gesture.end_frame(|origin| {
        let ((sx, sy), pt) = at_press(app, origin.egui());
        let c = app.wb.construction();
        if move_tool {
            if let Some(id) = app.label_at((sx, sy)) {
                let old = app.wb.label_offset(id).unwrap_or((0.0, 0.0));
                grabbed = Some(Drag::label(id, old, (sx, sy)));
                return Grab::Point;
            }
            let id = match target(app, &pt, (sx, sy)) {
                Target::Object(id) => id,
                Target::Nothing => return Grab::Pan,
                Target::Inert => return Grab::Nothing,
            };
            // The press position, captured to the grid with no offset
            // (`wrapMousePressed` calls `transformCoords`).
            let press = app
                .snap(pt.scale())
                .capture(pt.screen_to_world(sx, sy), (0.0, 0.0));
            grabbed = if c.kind(id) == Kind::Text {
                Drag::text(c, id, (sx, sy), press)
            } else {
                Drag::begin(c, id, press)
            };
            if grabbed.is_some() {
                return Grab::Point;
            }
            refused = Some(id);
            return Grab::Nothing;
        }
        if tool == Tool::MoveView {
            return Grab::Pan;
        }
        if tool == Tool::Delete {
            erase = true;
            return Grab::Point;
        }
        if let (true, Some(press)) = (press_tool, app.tool_press) {
            if let Some(id) = press.point {
                let at = app
                    .snap(pt.scale())
                    .capture(pt.screen_to_world(sx, sy), (0.0, 0.0));
                grabbed = Drag::begin(c, id, at);
                if grabbed.is_some() {
                    return Grab::Point;
                }
            }
            if press.acted {
                return Grab::Nothing;
            }
        }
        let on_point = hit::pick(c, &pt, (sx, sy), &press_reach, None, |id| {
            shown(app, id) && c.kind(id) == Kind::Point
        })
        .is_some();
        if on_point {
            Grab::Nothing
        } else {
            Grab::Pan
        }
    });
    app.gesture = gesture;
    // A refused drag of a dependent point says why, once per press (the
    // owner's rule, `interact::REFUSED_DRAG_NOTICE`).
    if let Some(id) = refused.filter(|_| app.rules.refused_drag_notice) {
        if app.wb.construction().kind(id) == Kind::Point {
            app.wb.notice = Some(text::label(Label::PointDependsOnOthers).to_owned());
        }
    }
    // The intents in their order: a drag that starts (perhaps one whose
    // press already ended this frame), its motion, then the end of a drag,
    // then the click.
    if intents.started.is_some() {
        if erase {
            app.erasing = true;
            app.wb.begin_gesture();
        }
        app.drag = grabbed;
        if let Some(drag) = grabbed {
            app.wb.begin_gesture();
            if !matches!(drag.hold(), Hold::Label { .. }) {
                app.wb.selected = Some(drag.id());
            }
        }
    }
    if let (Some(pos), Some(mut drag)) = (intents.drag_to, app.drag) {
        let (sx, sy) = to_local(pos.egui());
        match drag.step(&t, &app.snap(t.scale()), sx, sy) {
            Step::Point { x, y } => {
                app.wb.move_point(drag.id(), x, y);
            }
            Step::Translate { dx, dy } => {
                let points = interact::free_input_points(app.wb.construction(), drag.id());
                app.wb.translate(&points, (dx, dy));
            }
            Step::Label { x, y } => app.wb.set_label_offset(drag.id(), (x, y)),
            Step::Text(movement) => app.wb.move_text(drag.id(), movement),
        }
        app.drag = Some(drag);
    }
    // The eraser deletes what it meets once the pointer went beyond the
    // click threshold (`draggingBeyondThreshold`), at each position the
    // frame delivers.
    if let (Some(pos), true) = (intents.drag_to, app.erasing) {
        if app.gesture.beyond_threshold() {
            let radius = f64::from(draw::FREE_POINT_RADIUS);
            if app.erase_at(&t, to_local(pos.egui()), radius) {
                // The eraser holds no ids, so its own deletions leave the
                // stroke going (unlike a rebuild under a drag).
                app.seen_rebuilds = app.wb.rebuilds();
                app.label_rects.borrow_mut().clear();
                app.text_rects.borrow_mut().clear();
                app.hovered = None;
            }
        }
    }
    if let Some((dx, dy)) = intents.pan {
        cam.pan_pixels(dx as f64, dy as f64);
    }
    end_drag(app, &intents);
    // The automaton alone decides a click (egui's click rules take no part
    // in the drawing); the release must still be over the drawing, not over
    // a button on it.
    let tap = intents
        .tap
        .filter(|p| rect.contains(p.egui()) && !app.overlays.contains(*p));
    if let Some(pos) = tap.map(ToEgui::egui) {
        let local = to_local(pos);
        match tool {
            Tool::ZoomIn => cam.zoom_at(ZOOM_TOOL, local.0, local.1, w, h),
            Tool::ZoomOut => cam.zoom_at(1.0 / ZOOM_TOOL, local.0, local.1, w, h),
            Tool::MoveView => {}
            // Its press picked already (spec E1).
            _ if press_tool => {}
            _ => {
                let view = if tap_before_press {
                    tap_view
                } else {
                    app.press_view
                };
                let (local, pt) = at_view(view, pos);
                let hit = if move_tool {
                    target(app, &pt, local)
                } else {
                    let c = app.wb.construction();
                    let accept = |id| shown(app, id);
                    let text = app.text_at(local, None, accept);
                    match hit::pick_with_text(c, &pt, local, &press_reach, None, text, accept) {
                        Some(id) => Target::Object(id),
                        None => Target::Nothing,
                    }
                };
                match hit {
                    Target::Object(id) => click(app, &pt, local, Some(id)),
                    Target::Nothing => click(app, &pt, local, None),
                    Target::Inert => {}
                }
            }
        }
    }
    let early = fed.early_release.filter(|_| fed.owned.is_none());
    for pos in early.into_iter().chain(fed.late_release) {
        release_tool(app, &t, to_local(pos), &press_reach);
    }
    let cursor = match tool {
        _ if app.drag.is_some() || app.gesture.panning() => Some(egui::CursorIcon::Grabbing),
        Tool::MoveView => Some(egui::CursorIcon::Grab),
        Tool::ZoomIn => Some(egui::CursorIcon::ZoomIn),
        Tool::ZoomOut => Some(egui::CursorIcon::ZoomOut),
        Tool::Move => app
            .hovered
            .filter(|&id| interact::movable(app.wb.construction(), id))
            .map(|_| egui::CursorIcon::Grab),
        _ => response.hover_pos().map(|_| egui::CursorIcon::Crosshair),
    };
    if let (Some(c), true) = (cursor, response.hovered() || app.drag.is_some()) {
        ui.ctx().set_cursor_icon(c);
    }

    app.camera = Some(cam);
    let t = cam.transform(w, h);
    sample_loci(app, ui, t);
    // Whoever got the press (this area, a button over it, a dialog), it
    // ends with the release.
    if fed.released {
        app.press_view = None;
    }
    painter.rect_filled(rect, 0.0, theme::WHITE);
    let c = app.wb.construction();
    let visible = |id: Id| show_all || app.wb.is_visible(id);
    let ghost = |id: Id| show_all && !app.wb.is_visible(id);
    draw::image::paint_background(&painter, rect, c, &t, &visible, &ghost);
    grid::paint(&painter, rect, &t, app.decor, app.metrics.numbering_px);
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
            label_offset: &|id| app.wb.label_offset(id),
            label_rects: Some(&app.label_rects),
            text_rects: Some(&app.text_rects),
        },
    );
    preview(app, &painter, rect, &t, response.hover_pos());
    overlay(app, ui, rect, w, h, &mut cam);
    app.camera = Some(cam);
    app.view = Some((rect.core(), cam.transform(w, h)));
}

fn sample_loci(app: &mut App, ui: &Ui, t: ViewTransform) {
    let c = app.wb.construction();
    if !c.has_loci() {
        return;
    }
    let now = ui.input(|i| i.time);
    if let Some([xmin, xmax, ymin, ymax, scale, _]) = c.locus_view() {
        // The sampler already covers half a view past each edge.
        let (mx, my) = ((xmax - xmin) * 0.5, (ymax - ymin) * 0.5);
        let v = t.view();
        if v.xmin >= xmin - mx
            && v.xmax <= xmax + mx
            && v.ymin >= ymin - my
            && v.ymax <= ymax + my
            && (1.0 / LOCUS_SCALE_RATIO..=LOCUS_SCALE_RATIO).contains(&(t.scale() / scale))
        {
            return;
        }
        if let Some(last) = app.locus_sample_time {
            let remaining = LOCUS_SAMPLE_INTERVAL - (now - last);
            if remaining > 0.0 {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(remaining));
                return;
            }
        }
    }
    app.wb.set_locus_view(t);
    app.locus_sample_time = Some(now);
}

/// What a press of the move tool takes hold of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    /// Nothing is hit: the press pans, a click deselects.
    Nothing,
    Object(Id),
    /// Something is hit that the press must leave alone: it selects
    /// nothing, drags nothing and does not pan (a polygon's side under the
    /// reference app's rule, `OwnerRules::side_takes_the_polygon` off).
    Inert,
}

/// Puts back or ends the drag that the automaton ended.
fn end_drag(app: &mut App, intents: &Intents) {
    if intents.cancel_drag {
        app.cancel_drag();
        if app.erasing {
            app.wb.cancel_gesture();
        }
        app.erasing = false;
    }
    if intents.end_drag {
        // The reference app keeps no step for a drag within the click
        // threshold (spec F2); the owner keeps every drag
        // (`interact::EVERY_DRAG_IS_A_STEP`).
        let short = intents.released.is_some_and(|r| !r.beyond_threshold);
        if short && !app.rules.every_drag_is_a_step {
            app.wb.end_gesture_without_step();
        } else {
            app.wb.end_gesture();
        }
        app.drag = None;
        app.erasing = false;
    }
}

/// The release that ends a construction tool's press, at `pos` in the area.
fn release_tool(app: &mut App, t: &ViewTransform, pos: (f64, f64), reach: &hit::Reach) {
    if let Some(press) = app.tool_press.take() {
        app.release_tool(press, t, pos, reach);
    }
}

/// What `feed` saw of the primary button.
struct Fed {
    /// The position of the last primary press among the events.
    pressed: Option<egui::Pos2>,
    /// The last primary button event was a release.
    released: bool,
    /// The position of the last primary press that was the area's.
    owned: Option<egui::Pos2>,
    /// A primary release before any primary press of the frame.
    early_release: Option<egui::Pos2>,
    /// A primary release after the last primary press of the frame.
    late_release: Option<egui::Pos2>,
}

/// Gives this frame's pointer events to the automaton, in order. Each press,
/// and each finger going down, is the area's when `owns` says so at its
/// position; any other press only moves the pointer.
fn feed(gesture: &mut Gesture, events: &[egui::Event], owns: impl Fn(egui::Pos2) -> bool) -> Fed {
    let mut fed = Fed {
        pressed: None,
        released: false,
        owned: None,
        early_release: None,
        late_release: None,
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
                        fed.late_release = None;
                    } else if fed.pressed.is_none() {
                        fed.early_release.get_or_insert(pos);
                    } else {
                        fed.late_release = Some(pos);
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
                    if primary {
                        fed.owned = Some(pos);
                    }
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

/// A click on the drawing area, with the object under it.
fn click(app: &mut App, t: &ViewTransform, pos: (f64, f64), hit: Option<Id>) {
    if app.tool.tool() == Tool::Move {
        app.wb.selected = hit;
        app.style_bar = false;
        return;
    }
    // A new point takes the position captured to the grid
    // (`transformCoords` in `updateNewPoint`).
    let world = app.snap(t.scale()).apply(t.screen_to_world(pos.0, pos.1));
    let hit_owned: Option<(String, Kind)> = hit.map(|id| {
        let c = app.wb.construction();
        (c.label(id).to_owned(), c.kind(id))
    });
    let outcome = {
        let c = app.wb.construction();
        app.tool.click(
            hit_owned.as_ref().map(|(l, k)| (l.as_str(), *k)),
            world,
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
/// drawn from the first pick to the pointer, in the object's own style
/// (spec E7). The free end jumps onto a point under the pointer, by the
/// rule a click uses (spec E6).
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
    if picks.is_empty() {
        return;
    }
    let pointer = pointer.filter(|p| rect.contains(*p)).map(|p| {
        let local = ((p.x - rect.min.x) as f64, (p.y - rect.min.y) as f64);
        let (x, y) = app.preview_end(t, local, &hit::Reach::of(&app.metrics, false));
        rect.min + vec2(x as f32, y as f32)
    });
    if app.tool.tool() == Tool::Polygon && !picks.is_empty() {
        // The sides so far and the one following the pointer, lightly filled.
        let mut pts: Vec<egui::Pos2> = picks
            .iter()
            .filter_map(|p| pick_position(app, p).map(screen))
            .collect();
        pts.extend(pointer);
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
        let stroke = Stroke::new(draw::CURVE_WIDTH, theme::POLYGON);
        painter.add(egui::Shape::line(pts, stroke));
        return;
    }
    let (Some(to), [first]) = (pointer, picks) else {
        return;
    };
    let Some(from) = pick_position(app, first).map(screen) else {
        return;
    };
    let stroke = Stroke::new(draw::CURVE_WIDTH, theme::CURVE);
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
