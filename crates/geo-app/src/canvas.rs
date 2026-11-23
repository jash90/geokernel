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
use geo_app_core::camera;
use geo_app_core::text::{self, Label};
use geo_kernel::kernel::{Id, Kind};

/// The zoom of the buttons over the drawing (`EuclidianView
/// .MODE_ZOOM_FACTOR`, 1.5, about the centre).
const ZOOM_BUTTON: f64 = 1.5;
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
    // world point at the centre of the drawing there (`keepCenter`; spec
    // D11): the camera names the centre's world point itself, so there is
    // nothing to move. A view request is made for the new size, so it
    // comes after.
    let bounds = view::content_bounds(app.wb.construction());
    let request = app.view_request.take();
    if app.camera.is_none() || request.is_some() {
        // A fit asked for (`setViewShowAllObjects`) measures the drawn
        // shapes through the view it leaves; the first frame of a document
        // has none and fits the content bounds.
        let was = app.camera;
        let t = was.map(|c| c.transform(w, h));
        app.camera = Some(match request {
            Some(ViewRequest::Standard) => Camera::STANDARD,
            Some(ViewRequest::Fit) => {
                let c = app.wb.construction();
                t.as_ref()
                    .and_then(|t| Camera::fit_drawn(c, t, w, h, app.rules.scale_limits))
                    .unwrap_or_else(|| Camera::fit(bounds, w, h, app.rules.scale_limits))
            }
            Some(ViewRequest::Initial) | None => initial_camera(app, bounds, &painter, rect),
        });
        // A fit or the standard view is one undo step
        // (`setViewShowAllObjects(storeUndo)` / `setStandardView(true)`),
        // never the view a document opens with.
        if let (Some(before), Some(now)) = (was, app.camera) {
            if matches!(request, Some(ViewRequest::Fit | ViewRequest::Standard)) {
                app.wb.set_camera(now);
                app.wb.record_view_step(before);
            }
        }
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
    // A camera that an undo or redo step restored becomes the view this
    // frame starts from.
    if let Some(restored) = app.wb.take_restored_view() {
        cam = restored;
        app.camera = Some(cam);
        app.view_gesture = Default::default();
        app.pinching = false;
    }
    let to_local = |pos: egui::Pos2| {
        let d = pos - rect.min;
        (d.x as f64, d.y as f64)
    };

    // The camera as it was before this frame changed it: where a view
    // gesture (a wheel run, a pinch) begins.
    let cam0 = cam;

    // The wheel of this frame, from its raw events: the zoom is
    // proportional to the scroll amount (the owner's rule, spec D2), so
    // egui's smoothed delta is no measure of it; the reference app zooms
    // one fixed x1.1 per event whatever its delta. The wheel belongs to a
    // text field that has the keyboard (`isTextfieldHasFocus`). It zooms about the
    // pointer while that is over the drawing; horizontal scrolling zooms
    // nothing; Alt strengthens the zoom (`wrapMouseWheelMoved`).
    let typing = ui.ctx().egui_wants_keyboard_input();
    let (notches, events, alt) = if response.hovered() && !typing {
        ui.input(|i| {
            let (mut n, mut e, mut alt) = (0.0, 0.0, false);
            for ev in &i.events {
                if let egui::Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } = *ev
                {
                    let d = camera::wheel_notches(
                        unit == egui::MouseWheelUnit::Line,
                        delta.y as f64,
                        delta.x as f64,
                    );
                    n += d;
                    // The signed count of the events that zoom: only their
                    // sign decides in or out for the reference app.
                    e += d.signum();
                    alt |= modifiers.alt;
                }
            }
            (n, e, alt)
        })
    } else {
        (0.0, 0.0, false)
    };
    let wheel = if notches != 0.0 || events != 0.0 {
        camera::wheel_zoom_factor(app.rules.wheel_zoom_fixed_per_event, notches, events, alt)
    } else {
        1.0
    };
    // A trackpad's pinch, as the platform reports it (egui's zoom events,
    // unsmoothed): like the wheel, it zooms about the pointer only when
    // no text field has focus. Two touch pointers pinch under the fingers.
    let native = if response.hovered() && !typing {
        ui.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match *e {
                    egui::Event::Zoom(f) => Some(f as f64),
                    _ => None,
                })
                .product::<f64>()
        })
    } else {
        1.0
    };
    let wheel = wheel * native;
    let mut zoomed = wheel != 1.0;

    // The zoom the keyboard asked for (Ctrl + plus / minus, spec D14):
    // about the last pointer position, by the wheel factor.
    if let Some(f) = app.key_zoom.take() {
        zoomed = true;
        let at = ui
            .input(|i| i.pointer.latest_pos())
            .unwrap_or(rect.center());
        if rect.contains(at) {
            let (sx, sy) = to_local(at);
            cam.zoom_at(f, sx, sy, w, h, app.rules.scale_limits);
        } else {
            cam.zoom_at(f, 0.5 * w, 0.5 * h, w, h, app.rules.scale_limits);
        }
    }

    // Two fingers pinch (`twoTouchStart` .. `twoTouchEnd`): each move of a
    // finger, as its event came and wherever the other one stands, is one
    // step of the pinch's own rule — the owner's tracks the fingers
    // exactly (spec H4), the reference's lags behind its dead bands
    // (`camera::Pinch`). The places of the pointers a pinch may measure
    // are kept every frame, whole pixels as the reference counts them, so
    // the pinch begins wherever its fingers stand. A trackpad's pinch
    // (the platform's zoom events) is a zoom like the wheel's, about the
    // pointer; the frame's wheel or keyboard zoom stands down while two
    // fingers pinch.
    let pinching = gesture.pinching();
    {
        let ids = pinching.then(|| gesture.pinch_ids()).flatten();
        let bands = app.rules.pinch_bands;
        let limits = app.rules.scale_limits;
        let mut fingers = std::mem::take(&mut app.fingers);
        let mut pinch = app.pinch;
        ui.input(|i| {
            for ev in &i.events {
                let (id, at, down) = match *ev {
                    egui::Event::Touch {
                        id,
                        pos,
                        phase: egui::TouchPhase::Cancel | egui::TouchPhase::End,
                        ..
                    } => (id.0, to_local(pos), false),
                    egui::Event::Touch { id, pos, .. } => (id.0, to_local(pos), true),
                    egui::Event::PointerMoved(pos) => (MOUSE, to_local(pos), true),
                    _ => continue,
                };
                if !down {
                    fingers.remove(&id);
                    continue;
                }
                fingers.insert(id, (at.0.round(), at.1.round()));
                let Some((first, second)) = ids else {
                    continue;
                };
                if id != first && id != second {
                    continue;
                }
                if let (Some(a), Some(b)) = (fingers.get(&first), fingers.get(&second)) {
                    let (a, b) = (*a, *b);
                    match pinch.as_mut() {
                        // The pinch is on: one move of a finger is one
                        // step (`twoTouchMove`).
                        Some(p) => camera::pinch_move(&mut cam, p, a, b, w, h, limits),
                        // Both fingers are down: the pinch begins here
                        // (`twoTouchStart`).
                        None => pinch = Some(camera::Pinch::start(a, b, bands)),
                    }
                }
            }
        });
        app.fingers = fingers;
        app.pinch = pinching.then_some(pinch).flatten();
    }
    if !pinching && zoomed {
        let center = response.hover_pos().unwrap_or(rect.center());
        let (sx, sy) = to_local(center);
        cam.zoom_at(wheel, sx, sy, w, h, app.rules.scale_limits);
    }
    if pinching {
        zoomed = false;
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
    //
    // A modifier press pans from anywhere (`shallMoveView` /
    // `specialMoveEvent`): Shift without Ctrl, or the Space bar held. A
    // press on an axis rescales that axis instead of panning when the view
    // can be scaled there (`moveAxesPossible`): the view-move tool, or any
    // tool with Shift. The zoom in tool drags a rectangle out (`processZoomRectangle`);
    // its zoom out kin has no rectangle.
    let shift = fed.pressed_mods.shift && !fed.pressed_mods.ctrl;
    let space = ui.input(|i| i.key_down(egui::Key::Space));
    let modifiers_pan = shift || space;
    // Which axis the press sits on, within the press's reach
    // (`HitDetector.addXAxis` / `addYAxis`: `< threshold`).
    let axis_under = |pt: &ViewTransform, (sx, sy): (f64, f64)| -> Option<bool> {
        let (ox, oy) = pt.world_to_screen(0.0, 0.0);
        let reach = press_reach.threshold;
        let x = (sy - oy).abs() < reach;
        let y = (sx - ox).abs() < reach;
        match (x, y) {
            (false, false) => None,
            (true, false) => Some(true),
            (false, true) => Some(false),
            // Both at the origin: the nearer axis.
            (true, true) => Some((sy - oy).abs() <= (sx - ox).abs()),
        }
    };
    let mut grabbed = None;
    let mut refused = None;
    let mut erase = false;
    let mut axis = None;
    let mut rect_from = None;
    let intents = gesture.end_frame(|origin| {
        let ((sx, sy), pt) = at_press(app, origin.egui());
        let c = app.wb.construction();
        if tool == Tool::ZoomIn || tool == Tool::ZoomOut {
            // A drag of the zoom in tool over empty space zooms to the
            // rectangle it draws; one that starts on an object zooms
            // nothing, and the zoom out tool has no rectangle.
            let empty =
                hit::pick(c, &pt, (sx, sy), &press_reach, None, |id| shown(app, id)).is_none();
            if tool == Tool::ZoomIn && empty {
                rect_from = Some((sx, sy));
                return Grab::Rect;
            }
            return Grab::Nothing;
        }
        // A press on an axis with the view-move tool, or with Shift held
        // (`mousePressedTranslatedView`): the drag rescales that axis.
        if let Some(x_axis) = axis_under(&pt, (sx, sy)) {
            if tool == Tool::MoveView || (modifiers_pan && tool != Tool::Delete) {
                let (wx, wy) = pt.screen_to_world(sx, sy);
                axis = Some(camera::AxisDrag {
                    x: x_axis,
                    grabbed: if x_axis { wx } else { wy },
                });
                return Grab::Axis;
            }
        }
        if modifiers_pan || tool == Tool::MoveView {
            return Grab::Pan;
        }
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
                .snap(pt.xscale(), pt.yscale())
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
        if tool == Tool::Delete {
            erase = true;
            return Grab::Point;
        }
        if let (true, Some(press)) = (press_tool, app.tool_press) {
            if let Some(id) = press.point {
                let at = app
                    .snap(pt.xscale(), pt.yscale())
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
        // A drag that only changes the view (a pan, an axis rescale, a
        // zoom rectangle) holds no object.
        app.axis = axis;
        app.rect_from = rect_from;
    }
    if let (Some(pos), Some(mut drag)) = (intents.drag_to, app.drag) {
        // A drag that a pinch suspended and that now goes on is a gesture
        // of its own (the pinch's step lies between).
        if !app.wb.gesture_open() {
            app.wb.begin_gesture();
        }
        let (sx, sy) = to_local(pos.egui());
        let now = cam.transform(w, h);
        match drag.step(&now, &app.snap(now.xscale(), now.yscale()), sx, sy) {
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
    // An axis drag rescales its axis about the origin so that the grabbed
    // tick follows the pointer (`scaleXAxis` / `scaleYAxis`).
    if let (Some(pos), Some(axis)) = (intents.drag_to, app.axis) {
        let (sx, sy) = to_local(pos.egui());
        let v = cam.transform(w, h).view();
        let (ox, oy) = cam.transform(w, h).world_to_screen(0.0, 0.0);
        cam = camera::axis_scale(
            &cam,
            axis.x,
            camera::ScreenAxis {
                zero: if axis.x { ox } else { oy },
                size: if axis.x { w } else { h },
                min: if axis.x { v.xmin } else { v.ymin },
                max: if axis.x { v.xmax } else { v.ymax },
            },
            axis.grabbed,
            if axis.x { sx } else { sy },
            app.rules.scale_limits,
        );
    }
    // The rectangle the zoom in tool drags out, for its preview.
    if let (Some(pos), true) = (intents.drag_to, app.rect_from.is_some()) {
        app.rect_to = Some(to_local(pos.egui()));
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
    // The zoom rectangle the release applies (`processZoomRectangle`):
    // only the zoom in tool's, and only from 30 px on in both directions.
    if intents.end_drag {
        if let (Some(from), Some(to)) = (app.rect_from, app.rect_to) {
            let (x0, y0, x1, y1) = camera::zoom_rectangle(from, to, w, h);
            if x1 - x0 >= camera::ZOOM_RECT_THRESHOLD && y1 - y0 >= camera::ZOOM_RECT_THRESHOLD {
                let t_now = cam.transform(w, h);
                let wx0 = t_now.screen_to_world(x0, y0).0;
                let wx1 = t_now.screen_to_world(x1, y0).0;
                let wy0 = t_now.screen_to_world(x0, y1).1;
                let wy1 = t_now.screen_to_world(x0, y0).1;
                cam = Camera::showing(
                    view::Bounds {
                        xmin: wx0,
                        ymin: wy0,
                        xmax: wx1,
                        ymax: wy1,
                    },
                    w,
                    h,
                    app.rules.scale_limits,
                );
            }
        }
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
            // The zoom tools' click is one undo step at once (the owner's
            // rule; the reference stores it when the tool is left).
            Tool::ZoomIn => zoom_click(app, &mut cam, ZOOM_TOOL, local, w, h),
            Tool::ZoomOut => zoom_click(app, &mut cam, 1.0 / ZOOM_TOOL, local, w, h),
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
    // The release of a press the area no longer knows (a tap the frame
    // after its press, or one in its very frame) reads the view the press
    // was made in: the area may have changed size in between (the keyboard
    // closing on the press keeps the centre, spec D11, not the corner).
    let press_t = app.press_view.map(|(_, t)| t).unwrap_or(t);
    for pos in early.into_iter().chain(fed.late_release) {
        release_tool(app, &press_t, to_local(pos), &press_reach);
    }

    // The undo steps of the view (`OwnerRules::view_changes_are_steps`):
    // under the owner's rule every view gesture — a pan, an axis rescale,
    // a zoom rectangle, a pinch, a run of wheel or keyboard zoom — is one
    // step right after it; under the reference's a run of wheel or
    // keyboard zooms waits for the next canvas press, which stores it —
    // one step for every view change since — exactly as the reference app
    // keeps it (`storeUndoInfoIfSetCoordSystemOccurred`, spec F3).
    if fed.owned.is_some() && !app.rules.view_changes_are_steps {
        if let Some(before) = app.view_gesture.end(cam0) {
            app.wb.record_view_step(before);
        }
    }
    app.wb.set_camera(cam);
    if intents
        .started
        .is_some_and(|g| matches!(g, Grab::Pan | Grab::Axis | Grab::Rect))
    {
        if let Some(before) = app.view_gesture.begin(cam0) {
            app.wb.record_view_step(before);
        }
    }
    if pinching && !app.pinching {
        // A drag the pinch suspends ends its own step here; the pinch is a
        // gesture of its own until the last other finger lifts.
        app.wb.end_gesture();
        if let Some(before) = app.view_gesture.begin(cam0) {
            app.wb.record_view_step(before);
        }
    }
    if !pinching && app.pinching {
        if let Some(before) = app.view_gesture.end(cam) {
            app.wb.record_view_step(before);
        }
    }
    app.pinching = pinching;
    if let Some(before) = app
        .view_gesture
        .zoom_run(cam0, zoomed, app.rules.view_changes_are_steps)
    {
        app.wb.record_view_step(before);
    }
    if intents.cancel_view {
        if let Some(back) = app.view_gesture.cancel(cam) {
            cam = back;
            app.wb.set_camera(cam);
        }
    }
    if intents.end_drag {
        if let Some(before) = app.view_gesture.end(cam) {
            app.wb.record_view_step(before);
        }
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
    animate_sliders(app, ui);
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
    // The rectangle the zoom in tool drags out, snapped to the view's ratio
    // when it is near it (`updateSelectionRectangle`).
    if let (Some(from), Some(to)) = (app.rect_from, app.rect_to) {
        let (x0, y0, x1, y1) = camera::zoom_rectangle(from, to, w, h);
        let r = Rect::from_min_max(
            rect.min + vec2(x0 as f32, y0 as f32),
            rect.min + vec2(x1 as f32, y1 as f32),
        );
        painter.rect_filled(r, 2.0, theme::POLYGON.gamma_multiply(0.08));
        painter.rect_stroke(
            r,
            2.0,
            Stroke::new(1.5, theme::CURVE),
            egui::StrokeKind::Outside,
        );
    }
    overlay(app, ui, rect, w, h, &mut cam);
    app.camera = Some(cam);
    app.view = Some((rect.core(), cam.transform(w, h)));
}

fn initial_camera(
    app: &App,
    bounds: Option<view::Bounds>,
    painter: &egui::Painter,
    rect: Rect,
) -> Camera {
    let (w, h) = (f64::from(rect.width()), f64::from(rect.height()));
    let limits = app.rules.scale_limits;
    let Some(saved) = app.wb.file_view() else {
        return Camera::initial(bounds, w, h, limits);
    };
    let c = app.wb.construction();
    let visible = |id| app.wb.is_visible(id);
    if let Some(camera) = Camera::from_file(saved, limits) {
        let t = camera.transform(w, h);
        // Probe the saved view with the actual object renderer once at open.
        // Axes, grid and selection overlays do not count as visible objects.
        let layer = egui::LayerId::new(
            egui::Order::Background,
            egui::Id::unique("saved-view-probe"),
        );
        let probe = painter.clone().with_layer_id(layer);
        draw::image::paint_background(&probe, rect, c, &t, &visible, &|_| false);
        draw::paint(
            &probe,
            rect,
            c,
            &t,
            &Scene {
                visible: &visible,
                ghost: &|_| false,
                hovered: None,
                selected: None,
                palette: app.wb.palette(),
                cull: true,
                generation: app.wb.rebuilds(),
                label_offset: &|id| app.wb.label_offset(id),
                label_rects: None,
                text_rects: None,
            },
        );
        let has_objects = painter.ctx().graphics_mut(|graphics| {
            graphics.get_mut(layer).is_some_and(|list| {
                std::mem::take(list).all_entries().any(|shape| {
                    shape
                        .shape
                        .visual_bounding_rect()
                        .intersects(rect.intersect(shape.clip_rect))
                })
            })
        });
        if has_objects || c.is_empty() {
            return camera;
        }
    }
    let visible_bounds = view::visible_content_bounds(c, visible);
    Camera::fit(visible_bounds.or(bounds), w, h, limits)
}

/// The zoom of a zoom tool's click, as one undo step at once (the owner's
/// rule, `OwnerRules::view_changes_are_steps`; the reference app stores it when
/// the tool is left, `endOfMode`).
fn zoom_click(app: &mut App, cam: &mut Camera, factor: f64, at: (f64, f64), w: f64, h: f64) {
    let before = *cam;
    cam.zoom_at(factor, at.0, at.1, w, h, app.rules.scale_limits);
    app.wb.set_camera(*cam);
    app.wb.record_view_step(before);
}

/// Plays the animated sliders: a frame every thirtieth of a second, with
/// the frame rate the time since the last frame gives
/// (`AnimationManager.sliderStep`; the first frame runs at the highest rate).
fn animate_sliders(app: &mut App, ui: &Ui) {
    use geo_kernel::kernel::animation::MAX_ANIMATION_FRAME_RATE;
    if !app.wb.is_animating() {
        app.animation_time = None;
        return;
    }
    let now = ui.input(|i| i.time);
    let delay = 1.0 / MAX_ANIMATION_FRAME_RATE;
    let rate = match app.animation_time {
        Some(last) if now - last < delay => {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(delay - (now - last)));
            return;
        }
        Some(last) => 1.0 / (now - last),
        None => MAX_ANIMATION_FRAME_RATE,
    };
    app.wb.animate(rate);
    app.animation_time = Some(now);
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs_f64(delay));
}

fn sample_loci(app: &mut App, ui: &Ui, t: ViewTransform) {
    // The view-dependent objects (`Corner`, `AxisStepX`) follow the view on
    // every change, like GeoGebra's `euclidianViewUpdate`; `set_view` skips
    // an unchanged window, so this is one comparison per frame.
    app.wb.set_view(t);
    let c = app.wb.construction();
    if !c.has_loci() {
        return;
    }
    let now = ui.input(|i| i.time);
    if let Some([xmin, xmax, ymin, ymax, xscale, yscale]) = c.locus_view() {
        // The sampler already covers half a view past each edge.
        let (mx, my) = ((xmax - xmin) * 0.5, (ymax - ymin) * 0.5);
        let v = t.view();
        if v.xmin >= xmin - mx
            && v.xmax <= xmax + mx
            && v.ymin >= ymin - my
            && v.ymax <= ymax + my
            && (1.0 / LOCUS_SCALE_RATIO..=LOCUS_SCALE_RATIO).contains(&(t.xscale() / xscale))
            && (1.0 / LOCUS_SCALE_RATIO..=LOCUS_SCALE_RATIO).contains(&(t.yscale() / yscale))
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
        app.axis = None;
        app.rect_from = None;
        app.rect_to = None;
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
    /// The modifiers held at that press.
    pressed_mods: egui::Modifiers,
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
        pressed_mods: egui::Modifiers::NONE,
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
                modifiers,
            } => {
                let primary = button == egui::PointerButton::Primary;
                if primary {
                    if down {
                        fed.pressed = Some(pos);
                        fed.pressed_mods = modifiers;
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
    let world = app
        .snap(t.xscale(), t.yscale())
        .apply(t.screen_to_world(pos.0, pos.1));
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
    // The home button sits above the others and shows only while the view
    // is not the standard one (`ZoomController`: `onHomePressed` is offered
    // when `!isStandardView()`).
    let home = !cam.is_standard();
    let mut buttons = Vec::new();
    if home {
        buttons.push((Icon::Home, text::label(Label::StandardView)));
    }
    buttons.extend([
        (Icon::Fit, text::label(Label::ShowAllObjects)),
        (Icon::ZoomIn, text::label(Label::ZoomIn)),
        (Icon::ZoomOut, text::label(Label::ZoomOut)),
        (Icon::Fullscreen, text::label(Label::Fullscreen)),
    ]);
    // The play button of the reference sits in the lower left corner while
    // a slider animates (`needToShowAnimationButton`): it pauses and plays.
    if app.wb.has_animation() {
        let running = app.wb.is_animating();
        let (icon, tip) = if running {
            (Icon::Pause, text::label(Label::PauseAnimation))
        } else {
            (Icon::Play, text::label(Label::PlayAnimation))
        };
        let center = pos2(rect.left() + 35.0, bottom);
        if floating::round_button(ui, &mut app.overlays, "animation", center, icon, true, tip)
            .clicked()
        {
            app.wb.set_animation_running(!running);
        }
    }
    let n = buttons.len();
    for (i, (icon, tip)) in buttons.into_iter().enumerate() {
        let center = pos2(right, bottom - 44.0 * (n - 1 - i) as f32);
        if !floating::round_button(ui, &mut app.overlays, tip, center, icon, true, tip).clicked() {
            continue;
        }
        match icon {
            // Zoom to fit (`onZoomToFitPressed`): the drawn shapes' bounds
            // with 3 per cent free, no ratio kept, and a stored undo step
            // (`setViewShowAllObjects(true, ...)`).
            Icon::Fit => {
                let t = cam.transform(w, h);
                if let Some(fit) =
                    Camera::fit_drawn(app.wb.construction(), &t, w, h, app.rules.scale_limits)
                {
                    let before = *cam;
                    *cam = fit;
                    app.wb.set_camera(*cam);
                    app.wb.record_view_step(before);
                }
            }
            // The zoom buttons (`zoomInOut`): x1.5 about the centre, never
            // an undo step.
            Icon::ZoomIn => {
                cam.zoom_at(ZOOM_BUTTON, 0.5 * w, 0.5 * h, w, h, app.rules.scale_limits)
            }
            Icon::ZoomOut => cam.zoom_at(
                1.0 / ZOOM_BUTTON,
                0.5 * w,
                0.5 * h,
                w,
                h,
                app.rules.scale_limits,
            ),
            // The standard view (`onHomePressed`), stored as a step.
            Icon::Home => {
                let before = *cam;
                *cam = Camera::STANDARD;
                app.wb.set_camera(*cam);
                app.wb.record_view_step(before);
            }
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
                // The standard view, stored as a step like the home button
                // over the drawing (`setStandardView(true)`).
                _ => {
                    let before = *cam;
                    *cam = Camera::STANDARD;
                    app.wb.set_camera(*cam);
                    app.wb.record_view_step(before);
                }
            }
        }
    }
}
