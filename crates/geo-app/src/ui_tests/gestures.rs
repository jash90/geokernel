//! Pointer behaviour of the graphics view decided by the gesture automaton
//! (egui's click rules take no part in the drawing): the reference app's
//! 100 ms delay before moves count, no distance a press must go before it
//! drags, a 10 pixel threshold that decides only the click, no long press
//! for the mouse, and a drag with another button pans.

use super::*;
use crate::geom::ToEgui;
use geo_app_core::metrics::Metrics;

/// Runs one frame at `time` seconds with `events`.
fn frame_at(h: &mut Harness<'_, App>, time: f64, events: Vec<Event>) {
    h.input_mut().time = Some(time);
    h.input_mut().events.extend(events);
    h.step();
}

fn press(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// Was `a_long_press_is_not_a_tool_click` (egui's 0.8 s click time). The
/// reference app has no long press for the mouse (no timer in
/// `EuclidianController`; `wrapMouseReleasedND` runs the click path for any
/// release that did not drag beyond the threshold).
#[test]
fn a_long_press_is_still_a_tool_click() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().tool.set_tool(Tool::Point);
    let at = world(1.0, 1.0);
    frame_at(&mut h, 10.0, vec![Event::PointerMoved(at)]);
    frame_at(&mut h, 10.1, vec![press(at, PointerButton::Primary, true)]);
    frame_at(&mut h, 11.0, vec![]);
    frame_at(&mut h, 11.1, vec![press(at, PointerButton::Primary, false)]);
    h.run();
    assert_eq!(labels(&h), ["A", "B"], "the point is placed");
    assert_eq!(point(&h, "B"), (1.0, 1.0));
}

#[test]
fn after_a_long_press_the_view_pans_by_the_whole_movement() {
    let mut h = harness("A = (0, 0)\n");
    let at = world(-4.0, -3.0);
    let cx0 = h.state().camera.unwrap().cx;
    frame_at(&mut h, 10.0, vec![Event::PointerMoved(at)]);
    frame_at(&mut h, 10.1, vec![press(at, PointerButton::Primary, true)]);
    // Held still for long: a move after it is an ordinary drag, which
    // follows the pointer from the press.
    frame_at(&mut h, 11.0, vec![]);
    let to = at + egui::vec2(3.0, 0.0);
    frame_at(&mut h, 11.1, vec![Event::PointerMoved(to)]);
    frame_at(&mut h, 11.2, vec![press(to, PointerButton::Primary, false)]);
    h.run();
    let cam = h.state().camera.unwrap();
    assert!((cam.cx - cx0 + 3.0 / 50.0).abs() < 1e-12, "{}", cam.cx);
    assert_eq!(labels(&h), ["A"]);
}

/// Was `leaving_the_area_before_the_click_distance_starts_a_pan`: with no
/// distance a press must go before it drags, a move that leaves the area
/// pans by all of it, like any other.
#[test]
fn a_drag_that_leaves_the_area_pans_by_the_whole_movement() {
    // From half a point inside the right edge to five and a half outside
    // (beyond egui's interaction radius).
    let mut h = harness("A = (0, 0)\n");
    let from = at(SIZE.0 - 0.5, 500.0);
    let to = at(SIZE.0 + 5.5, 500.0);
    let cx0 = h.state().camera.unwrap().cx;
    moved(&mut h, from);
    button(&mut h, from, true);
    moved(&mut h, to);
    button(&mut h, to, false);
    h.run();
    let cam = h.state().camera.unwrap();
    assert!(
        (cam.cx - cx0 + 6.0 / 50.0).abs() < 1e-12,
        "{}",
        cam.cx - cx0
    );
}

#[test]
fn a_drag_with_the_secondary_button_pans_and_leaves_points_alone() {
    let mut h = harness("A = (1, 1)\n");
    let from = world(1.0, 1.0);
    let cx0 = h.state().camera.unwrap().cx;
    moved(&mut h, from);
    h.input_mut()
        .events
        .push(press(from, PointerButton::Secondary, true));
    h.step();
    for i in 1..=10 {
        moved(&mut h, from + egui::vec2(3.0 * i as f32, 0.0));
    }
    h.input_mut().events.push(press(
        from + egui::vec2(30.0, 0.0),
        PointerButton::Secondary,
        false,
    ));
    h.step();
    h.run();
    let app = h.state();
    let a = app.wb.construction().id("A").unwrap();
    assert_eq!(
        app.wb.construction().value(a),
        Value::Point { x: 1.0, y: 1.0 }
    );
    // The pan follows the pointer from the press (`snapMoveView` pans by
    // the offset from the press): all 30 points, no slop held back.
    let cam = app.camera.unwrap();
    assert!((cam.cx - cx0 + 30.0 / 50.0).abs() < 1e-12, "{}", cam.cx);
}

#[test]
fn panning_to_a_distant_locus_draws_its_circle() {
    let mut h = harness(
        "A = (0, 0)\nr = 1\nc = Circle(A, r)\nP = Point(c)\nQ = (30 + 2 * x(P), 2 * y(P))\nL = Locus(Q, P)\n",
    );
    h.state_mut().camera = Some(crate::view::Camera::STANDARD);
    h.state_mut().decor.axes = false;
    h.state_mut().decor.grid = false;
    let wb = &mut h.state_mut().wb;
    for label in ["A", "c", "P", "Q"] {
        wb.toggle_visible(wb.construction().id(label).unwrap());
    }
    let from = world(1.0, 1.0);
    let to = from - egui::vec2(1500.0, 0.0);
    frame_at(&mut h, 10.0, vec![Event::PointerMoved(from)]);
    frame_at(
        &mut h,
        10.1,
        vec![press(from, PointerButton::Secondary, true)],
    );
    frame_at(&mut h, 10.3, vec![Event::PointerMoved(to)]);
    frame_at(
        &mut h,
        10.4,
        vec![press(to, PointerButton::Secondary, false)],
    );
    frame_at(&mut h, 11.0, vec![]);
    frame_at(&mut h, 11.1, vec![]);

    let cam = h.state().camera.unwrap();
    assert!((cam.cx - 30.0).abs() < 1e-9, "camera at {cam:?}");
    let center = h.state().view.unwrap().0.egui().center();
    let region = egui::Rect::from_center_size(center, egui::vec2(250.0, 250.0));
    let output = h.output();
    let meshes = h
        .ctx
        .tessellate(output.shapes.clone(), output.pixels_per_point);
    let color = crate::theme::CURVE;
    let mut bounds = egui::Rect::NOTHING;
    for primitive in meshes {
        if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
            for vertex in mesh
                .vertices
                .iter()
                .filter(|v| v.color == color && region.contains(v.pos))
            {
                bounds.extend_with(vertex.pos);
            }
        }
    }
    assert!((195.0..210.0).contains(&bounds.width()), "{bounds:?}");
    assert!((195.0..210.0).contains(&bounds.height()), "{bounds:?}");
    let c = h.state().wb.construction();
    let fit = crate::view::content_bounds(c).unwrap();
    assert!(fit.ymin <= -1.99 && fit.ymax >= 1.99, "{fit:?}");

    let drawn = |h: &Harness<'_, App>| {
        h.output().shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Mesh(mesh) if region.contains_rect(mesh.calc_bounds()) => mesh
                .vertices
                .iter()
                .any(|v| v.color == color)
                .then(|| mesh.clone()),
            _ => None,
        })
    };
    let mesh = drawn(&h).expect("locus mesh");
    frame_at(&mut h, 11.2, vec![]);
    assert!(std::sync::Arc::ptr_eq(&mesh, &drawn(&h).unwrap()));
}

#[test]
fn a_secondary_click_does_nothing() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().tool.set_tool(Tool::Point);
    let at = world(1.0, 1.0);
    moved(&mut h, at);
    h.input_mut()
        .events
        .push(press(at, PointerButton::Secondary, true));
    h.step();
    h.input_mut()
        .events
        .push(press(at, PointerButton::Secondary, false));
    h.step();
    h.run();
    assert_eq!(labels(&h), ["A"]);
}

/// Was `a_press_and_a_far_release_in_the_next_frame_do_nothing` (egui
/// decided neither a click nor a drag for such a flick). Now the move is
/// applied (it comes after the delay) and, being a single move, it is not
/// beyond the threshold yet (`isDraggingBeyondThreshold` is checked one move
/// late), so the release is still a click.
#[test]
fn a_flick_in_the_next_frame_moves_and_is_still_a_click() {
    let mut h = harness("A = (1, 1)\n");
    h.state_mut().wb.selected = h.state().wb.construction().id("A");
    let from = world(-3.0, -3.0);
    let cx0 = h.state().camera.unwrap().cx;
    moved(&mut h, from);
    button(&mut h, from, true);
    let to = from + egui::vec2(40.0, 0.0);
    h.input_mut().events.push(Event::PointerMoved(to));
    h.input_mut()
        .events
        .push(press(to, PointerButton::Primary, false));
    h.step();
    h.run();
    let cam = h.state().camera.unwrap();
    assert!((cam.cx - cx0 + 40.0 / 50.0).abs() < 1e-12, "{}", cam.cx);
    assert_eq!(
        h.state().wb.selected,
        None,
        "the click on nothing deselects"
    );
    assert_eq!(labels(&h), ["A"]);

    // With the point tool the press places the point and the flick drags
    // it (spec E1, E4): it ends 40 px right of the press and nothing pans.
    // Fails if the point were placed only on a click (the flick is none
    // then) or if the flick panned.
    let mut h = harness("A = (1, 1)\n");
    h.state_mut().tool.set_tool(Tool::Point);
    let from = world(-3.3, -2.7);
    let cx0 = h.state().camera.unwrap().cx;
    moved(&mut h, from);
    button(&mut h, from, true);
    let to = from + egui::vec2(40.0, 0.0);
    h.input_mut().events.push(Event::PointerMoved(to));
    h.input_mut()
        .events
        .push(press(to, PointerButton::Primary, false));
    h.step();
    h.run();
    let cam = h.state().camera.unwrap();
    assert_eq!(cam.cx, cx0, "the flick panned");
    assert_eq!(labels(&h), ["A", "B"]);
    assert_eq!(point(&h, "B"), (-2.5, -2.7));
}

/// A window with the touch profile, `size` points large.
fn touch_harness(script: &str, size: (f32, f32)) -> Harness<'static, App> {
    let mut app = App::new(Workbench::from_script(script).unwrap(), None);
    app.set_metrics(Metrics::touch());
    let mut h = Harness::builder()
        .with_size(egui::vec2(size.0 + 2.0 * MARGIN, size.1 + 2.0 * MARGIN))
        .with_pixels_per_point(1.0)
        .with_context_setup(|ctx| {
            ctx.options_mut(|o| {
                o.input_options.max_click_dist = Metrics::touch().widget_click_dist;
                o.input_options.max_click_duration = Metrics::touch().widget_click_time;
            })
        })
        .build_ui_state(|ui, app: &mut App| crate::app::show(app, ui), app);
    h.run();
    h
}

/// The touch profile no longer has a wider slop of its own: the point tool
/// places its point where the finger goes down (spec E1); a wobble within
/// the 100 ms delay is dropped (`shouldCancelDrag`, for every pointer), and
/// one after the delay drags the new point with it (spec E4). Nothing pans.
/// Fails if the point were placed at the release (the first wobble) or if
/// the second wobble panned instead of moving the point.
#[test]
fn with_the_touch_profile_a_wobbly_tap_still_places_a_point() {
    for after_delay in [false, true] {
        let mut h = touch_harness("A = (0, 0)\n", SIZE);
        h.state_mut().tool.set_tool(Tool::Point);
        let from = drawn_at(&h, 1.3, 1.7);
        let cam = h.state().camera.unwrap();
        let wobble = from + egui::vec2(9.0, -4.0);
        let (t_move, t_up) = if after_delay {
            (20.25, 20.3)
        } else {
            (20.15, 20.19)
        };
        frame_at(&mut h, 20.0, vec![Event::PointerMoved(from)]);
        frame_at(
            &mut h,
            20.1,
            vec![press(from, PointerButton::Primary, true)],
        );
        frame_at(&mut h, t_move, vec![Event::PointerMoved(wobble)]);
        frame_at(
            &mut h,
            t_up,
            vec![
                Event::PointerMoved(wobble),
                press(wobble, PointerButton::Primary, false),
            ],
        );
        h.run();
        let what = format!("after the delay: {after_delay}");
        assert_eq!(labels(&h), ["A", "B"], "{what}");
        let (b, scale) = (point(&h, "B"), cam.scale);
        let want = if after_delay {
            (1.3 + 9.0 / scale, 1.7 + 4.0 / scale)
        } else {
            (1.3, 1.7)
        };
        assert!(
            (b.0 - want.0).abs() < 1e-9 && (b.1 - want.1).abs() < 1e-9,
            "{what}: {b:?}"
        );
        assert_eq!(h.state().camera.unwrap(), cam, "{what}: the wobble panned");
    }
}

/// A mouse press between the old point radius (11 px) and the new one
/// (3 + 12 = 15 px, `DrawPoint.SELECTION_RADIUS_MIN`) takes the point; a
/// finger press between its first reach (9 + 12 = 21 px) and its retry
/// (27 + 12 = 39 px, `HitDetector.setHits`) does too. Fails with
/// `point_selection_min` 8 (the old 11 px for the mouse) or with
/// `touch_retry_factor` 1 (no retry): the press then hits nothing and pans.
#[test]
fn presses_between_the_old_and_the_new_reach_take_the_point() {
    use egui::TouchPhase::{End, Move, Start};
    // The mouse, 13 px right of A.
    let mut h = harness("A = (1, 1)\n");
    let cam = h.state().camera.unwrap();
    let from = world(1.0, 1.0) + egui::vec2(13.0, 0.0);
    drag(&mut h, from, from + egui::vec2(40.0, -30.0));
    assert_ne!(point(&h, "A"), (1.0, 1.0), "the mouse press missed A");
    assert_eq!(h.state().camera.unwrap(), cam, "the press panned");
    // A finger, 30 px right of A.
    let mut h = touch_harness("A = (0, 0)\n", SIZE);
    let cam = h.state().camera.unwrap();
    let mut w = Winit::default();
    let mut f = drawn_at(&h, 0.0, 0.0) + egui::vec2(30.0, 0.0);
    frame_at(&mut h, 10.0, w.touch(0, Start, f));
    for i in 1..=4 {
        f += egui::vec2(10.0, 0.0);
        frame_at(&mut h, 10.1 + 0.05 * i as f64, w.touch(0, Move, f));
    }
    frame_at(&mut h, 10.5, w.touch(0, End, f));
    h.run();
    assert_ne!(point(&h, "A"), (0.0, 0.0), "the finger missed A");
    assert_eq!(h.state().camera.unwrap(), cam, "the finger panned");
}

#[test]
fn a_narrow_touch_window_stacks_the_views_and_still_works() {
    let mut h = touch_harness("A = (0, 0)\n", (400.0, 800.0));
    let app = h.state();
    assert!(app.regions.portrait);
    assert_eq!(app.regions.graphics.width(), 400.0);
    // A tap in the graphics view with the point tool places a point there.
    h.state_mut().tool.set_tool(Tool::Point);
    let (rect, _) = h.state().view.unwrap();
    let at = Pos2::new(rect.min.x + 50.0, rect.min.y + 50.0);
    click(&mut h, at);
    assert_eq!(labels(&h), ["A", "B"]);
}

/// The shapes of the last frame.
fn shapes(h: &Harness<'_, App>) -> usize {
    h.output().shapes.len()
}

#[test]
fn without_hover_the_tool_preview_waits_for_the_pointer_and_never_goes_stale() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().tool.set_tool(Tool::Segment);
    // A touch tap: the press and release, then the pointer is gone.
    let first = world(2.0, 2.0);
    button(&mut h, first, true);
    button(&mut h, first, false);
    h.input_mut().events.push(Event::PointerGone);
    h.run();
    assert_eq!(h.state().tool.picks().len(), 1);
    let without = shapes(&h);
    // A pointer over the view draws the segment to it ...
    moved(&mut h, world(-2.0, 1.0));
    h.run();
    assert!(shapes(&h) > without);
    // ... and once it is gone nothing of it stays.
    h.input_mut().events.push(Event::PointerGone);
    h.run();
    assert_eq!(shapes(&h), without);
}

#[test]
fn without_hover_the_move_tool_shows_its_hint() {
    let shown = |hover: bool| {
        let mut app = App::new(Workbench::from_script("A = (0, 0)\n").unwrap(), None);
        app.set_metrics(Metrics {
            hover,
            ..Metrics::desktop()
        });
        let mut h = Harness::builder()
            .with_size(egui::vec2(SIZE.0 + 2.0 * MARGIN, SIZE.1 + 2.0 * MARGIN))
            .with_pixels_per_point(1.0)
            .build_ui_state(|ui, app: &mut App| crate::app::show(app, ui), app);
        h.run();
        shapes(&h)
    };
    // The panel and the two lines of text, and no close button.
    assert_eq!(shown(false), shown(true) + 3);
}

/// A touch event of finger `id`.
fn touch(h: &mut Harness<'_, App>, id: u64, phase: egui::TouchPhase, pos: Pos2) {
    h.input_mut().events.push(Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(id),
        phase,
        pos,
        force: None,
    });
}

#[test]
fn a_second_finger_puts_a_dragged_point_back_and_pinches_the_view() {
    use egui::TouchPhase::{End, Move, Start};
    let mut h = harness("A = (0, 0)\nB = (3, 1)\n");
    let scale = h.state().camera.unwrap().scale;
    // The first finger takes A and drags it (the platform also sends the
    // pointer events of the first finger, as egui-winit does).
    let mut f0 = world(0.0, 0.0);
    touch(&mut h, 0, Start, f0);
    moved(&mut h, f0);
    button(&mut h, f0, true);
    for _ in 0..4 {
        f0 += egui::vec2(10.0, 0.0);
        touch(&mut h, 0, Move, f0);
        moved(&mut h, f0);
    }
    assert!(point_x(&h, "A") > 0.5, "the drag moved A");
    // A second finger: the drag is undone and the two fingers pinch.
    let mut f1 = f0 + egui::vec2(0.0, 120.0);
    touch(&mut h, 1, Start, f1);
    h.run();
    assert_eq!(point_x(&h, "A"), 0.0, "A is back");
    for _ in 0..4 {
        f0 += egui::vec2(0.0, -10.0);
        f1 += egui::vec2(0.0, 10.0);
        touch(&mut h, 0, Move, f0);
        touch(&mut h, 1, Move, f1);
        moved(&mut h, f0);
    }
    touch(&mut h, 1, End, f1);
    touch(&mut h, 0, End, f0);
    button(&mut h, f0, false);
    h.run();
    assert_eq!(point_x(&h, "A"), 0.0, "the pinch never moves A");
    assert!(!h.state().wb.can_undo(), "no undo step is left");
    assert!(
        h.state().camera.unwrap().scale > scale * 1.05,
        "the view zoomed in"
    );
}

/// Runs one frame with `events`.
fn frame(h: &mut Harness<'_, App>, events: Vec<Event>) {
    h.input_mut().events.extend(events);
    h.step();
}

/// The platform's touch emulation (egui-winit 0.37), shared with the
/// headless server.
type Winit = crate::headless::TouchEmulation;

/// Where the world point `(x, y)` is drawn now.
fn drawn_at(h: &Harness<'_, App>, x: f64, y: f64) -> Pos2 {
    let (rect, t) = h.state().view.unwrap();
    let (sx, sy) = t.world_to_screen(x, y);
    rect.min.egui() + egui::vec2(sx as f32, sy as f32)
}

fn camera(h: &Harness<'_, App>) -> (f64, f64, f64) {
    let c = h.state().camera.unwrap();
    (c.cx, c.cy, c.scale)
}

/// With the earlier code (one ownership flag a frame, from egui's
/// `is_pointer_button_down_on`, which the release clears) the new press was
/// not the drawing's and never panned.
#[test]
fn a_release_and_a_press_in_one_frame_end_the_drag_and_start_a_new_press() {
    let mut h = harness("A = (0, 0)\n");
    let from = world(0.0, 0.0);
    moved(&mut h, from);
    button(&mut h, from, true);
    let to = from + egui::vec2(100.0, 0.0);
    for i in 1..=5 {
        moved(&mut h, from + (to - from) * (i as f32 / 5.0));
    }
    // The release of the drag and a press on empty space in one frame.
    let empty = world(-6.0, 4.0);
    frame(
        &mut h,
        vec![
            press(to, PointerButton::Primary, false),
            Event::PointerMoved(empty),
            press(empty, PointerButton::Primary, true),
        ],
    );
    assert_eq!(point(&h, "A").0, 2.0, "the drag ended where it was");
    let cam = camera(&h);
    for i in 1..=5 {
        moved(&mut h, empty + egui::vec2(20.0 * i as f32, 0.0));
    }
    button(&mut h, empty + egui::vec2(100.0, 0.0), false);
    h.run();
    assert_ne!(camera(&h).0, cam.0, "the second press panned the view");
    assert_eq!(point(&h, "A").0, 2.0);
    // The drag of A is one undo step of its own.
    undo(&mut h);
    assert_eq!(point(&h, "A").0, 0.0);
    assert!(!h.state().wb.can_undo());
}

/// With the earlier code (the release decided at the frame's end, at the
/// last pointer position) the tap was placed where the new press went down.
#[test]
fn a_tap_and_a_press_in_one_frame_place_the_point_where_the_tap_was() {
    let mut h = harness("");
    h.state_mut().select_tool(Tool::Point);
    h.run();
    let (first, second) = (world(1.0, 1.0), world(-3.0, 2.0));
    moved(&mut h, first);
    button(&mut h, first, true);
    // The tap's release, then a press somewhere else, in one frame.
    frame(
        &mut h,
        vec![
            press(first, PointerButton::Primary, false),
            Event::PointerMoved(second),
            press(second, PointerButton::Primary, true),
        ],
    );
    button(&mut h, second, false);
    h.run();
    assert_eq!(labels(&h), ["A", "B"]);
    assert_eq!(
        point(&h, "A"),
        (1.0, 1.0),
        "the tap was placed at the press"
    );
    assert_eq!(point(&h, "B"), (-3.0, 2.0));
}

#[test]
fn a_press_on_the_toolbar_right_after_a_click_in_the_drawing_is_not_the_drawings() {
    let mut h = harness("");
    h.state_mut().select_tool(Tool::Point);
    h.run();
    let spot = world(1.0, 1.0);
    moved(&mut h, spot);
    button(&mut h, spot, true);
    // The click's release and a press on a tool group button in one frame.
    let group = group_button(0);
    frame(
        &mut h,
        vec![
            press(spot, PointerButton::Primary, false),
            Event::PointerMoved(group),
            press(group, PointerButton::Primary, true),
        ],
    );
    let cam = camera(&h);
    // Held and dragged down over the drawing: the drawing does not pan.
    for i in 1..=6 {
        moved(&mut h, group + egui::vec2(0.0, 40.0 * i as f32));
    }
    button(&mut h, group + egui::vec2(0.0, 240.0), false);
    h.run();
    assert_eq!(labels(&h), ["A"], "the click placed one point");
    assert_eq!(point(&h, "A"), (1.0, 1.0));
    assert_eq!(camera(&h), cam, "the press on the toolbar panned nothing");
}

/// Pins behaviour the window already had (the earlier code reached it by
/// another way: it ignored the touch events and the finger that stays sends
/// no press), now with every pointer event egui-winit really sends,
/// including the moves and the release without a press of a second finger
/// that outlives the first.
#[test]
fn after_a_pinch_the_finger_that_stays_neither_drags_nor_pans_as_before() {
    use egui::TouchPhase::{End, Move, Start};
    for first_lifts in [false, true] {
        let mut h = touch_harness("A = (0, 0)\nB = (3, 1)\n", SIZE);
        let mut w = Winit::default();
        // The first finger on A, the second one far from it.
        let mut f0 = drawn_at(&h, 0.0, 0.0);
        frame(&mut h, w.touch(0, Start, f0));
        let mut f1 = f0 + egui::vec2(0.0, 150.0);
        frame(&mut h, w.touch(1, Start, f1));
        for _ in 0..4 {
            f0 += egui::vec2(0.0, -10.0);
            f1 += egui::vec2(0.0, 10.0);
            let mut events = w.touch(0, Move, f0);
            events.extend(w.touch(1, Move, f1));
            frame(&mut h, events);
        }
        // One finger lifts; the other one moves on, far.
        let mut stays = if first_lifts {
            frame(&mut h, w.touch(0, End, f0));
            (1, f1)
        } else {
            frame(&mut h, w.touch(1, End, f1));
            (0, f0)
        };
        h.run();
        let cam = camera(&h);
        for _ in 0..6 {
            stays.1 += egui::vec2(25.0, 10.0);
            let events = w.touch(stays.0, Move, stays.1);
            // The second finger's moves now come as the mouse's.
            assert!(events.contains(&Event::PointerMoved(stays.1)));
            frame(&mut h, events);
        }
        let events = w.touch(stays.0, End, stays.1);
        assert!(events.contains(&press(stays.1, PointerButton::Primary, false)));
        frame(&mut h, events);
        h.run();
        let what = format!("first lifts: {first_lifts}");
        assert_eq!(camera(&h), cam, "{what}: the finger that stayed panned");
        assert_eq!(point(&h, "A"), (0.0, 0.0), "{what}: A moved");
        assert!(!h.state().wb.can_undo(), "{what}");
        assert_eq!(labels(&h), ["A", "B"], "{what}");
    }
}

/// Pins behaviour the window already had (the earlier code cancelled the
/// press on a cancel of any finger); the next test tells the two apart.
#[test]
fn a_cancelled_touch_puts_the_dragged_point_back_as_before() {
    use egui::TouchPhase::{Cancel, Move, Start};
    let mut h = touch_harness("A = (0, 0)\n", SIZE);
    let mut w = Winit::default();
    let mut f = drawn_at(&h, 0.0, 0.0);
    frame(&mut h, w.touch(4, Start, f));
    for _ in 0..4 {
        f += egui::vec2(15.0, 0.0);
        frame(&mut h, w.touch(4, Move, f));
    }
    assert!(point(&h, "A").0 > 0.5, "the drag moved A");
    frame(&mut h, w.touch(4, Cancel, f));
    h.run();
    assert_eq!(point(&h, "A"), (0.0, 0.0), "A is back");
    assert!(!h.state().wb.can_undo(), "no undo step is left");
    assert!(h.state().drag.is_none() && !h.state().gesture.pressed());
}

#[test]
fn a_finger_on_the_toolbar_leaves_a_point_drag_alone_even_when_cancelled() {
    use egui::TouchPhase::{Cancel, End, Move, Start};
    let mut h = touch_harness("A = (0, 0)\nB = (3, 1)\n", SIZE);
    let scale = camera(&h).2;
    let mut w = Winit::default();
    let mut f = drawn_at(&h, 0.0, 0.0);
    frame(&mut h, w.touch(0, Start, f));
    for _ in 0..3 {
        f += egui::vec2(15.0, 0.0);
        frame(&mut h, w.touch(0, Move, f));
    }
    let x = point(&h, "A").0;
    assert!(x > 0.5, "the drag moved A");
    // A second finger lands on the toolbar, rests there and wobbles while
    // the first one drags on (egui reports two fingers on the screen).
    let other = at(100.0, 6.0);
    frame(&mut h, w.touch(1, Start, other));
    for i in 0..4 {
        f += egui::vec2(15.0, 0.0);
        let mut events = w.touch(0, Move, f);
        events.extend(w.touch(1, Move, other + egui::vec2(i as f32, 0.0)));
        frame(&mut h, events);
    }
    assert!(point(&h, "A").0 > x + 0.5, "A stopped following the finger");
    // The platform cancels the second finger: the drag still goes on.
    frame(&mut h, w.touch(1, Cancel, other));
    let x = point(&h, "A").0;
    f += egui::vec2(15.0, 0.0);
    frame(&mut h, w.touch(0, Move, f));
    assert!(point(&h, "A").0 > x + 0.1, "the cancel ended the drag");
    frame(&mut h, w.touch(0, End, f));
    h.run();
    let a = point(&h, "A");
    assert!(a.0 > 2.0, "A went back: {a:?}");
    assert_eq!(camera(&h).2, scale, "the two fingers zoomed");
    assert_eq!(labels(&h), ["A", "B"]);
    // The drag is one undo step.
    undo(&mut h);
    assert_eq!(point(&h, "A"), (0.0, 0.0));
    assert!(!h.state().wb.can_undo());
}

#[test]
fn a_rebuild_during_a_pan_leaves_the_pan_inert_until_the_release() {
    let mut h = harness("A = (1, 1)\n");
    h.state_mut().wb.run_line("B = (4, 4)").unwrap();
    let start = world(-5.0, -3.0);
    moved(&mut h, start);
    button(&mut h, start, true);
    for i in 1..=5 {
        moved(&mut h, start + egui::vec2(10.0 * i as f32, 0.0));
    }
    assert!(h.state().gesture.panning(), "the drag pans");
    undo(&mut h);
    let cam = camera(&h);
    for i in 6..=12 {
        moved(&mut h, start + egui::vec2(10.0 * i as f32, 0.0));
    }
    button(&mut h, start + egui::vec2(120.0, 0.0), false);
    h.run();
    assert_eq!(camera(&h), cam, "the view no longer follows");
    assert_eq!(labels(&h), ["A"]);
}

/// The last move of a running drag that shares a frame with the release
/// still counts (`wrapMouseDraggedND` runs before `wrapMouseReleasedND`):
/// the point ends at the last move, as one undo step. The same for a pan
/// and for a finger. Fails if the release drops the motion of its frame.
#[test]
fn the_last_move_of_a_drag_counts_when_the_release_shares_its_frame() {
    // A drag of a point with the mouse.
    let mut h = harness("A = (0, 0)\n");
    let from = world(0.0, 0.0);
    frame_at(&mut h, 10.0, vec![Event::PointerMoved(from)]);
    frame_at(
        &mut h,
        10.05,
        vec![press(from, PointerButton::Primary, true)],
    );
    frame_at(
        &mut h,
        10.2,
        vec![Event::PointerMoved(from + egui::vec2(30.0, 0.0))],
    );
    let to = from + egui::vec2(60.0, -40.0);
    frame_at(
        &mut h,
        10.3,
        vec![
            Event::PointerMoved(to),
            press(to, PointerButton::Primary, false),
        ],
    );
    h.run();
    assert_eq!(point(&h, "A"), (1.2, 0.8));
    undo(&mut h);
    assert_eq!(point(&h, "A"), (0.0, 0.0));
    assert!(!h.state().wb.can_undo(), "one step for the whole drag");

    // A pan.
    let mut h = harness("A = (0, 0)\n");
    let (cx0, cy0, _) = camera(&h);
    let from = world(-4.0, -3.0);
    frame_at(&mut h, 10.0, vec![Event::PointerMoved(from)]);
    frame_at(
        &mut h,
        10.05,
        vec![press(from, PointerButton::Primary, true)],
    );
    frame_at(
        &mut h,
        10.2,
        vec![Event::PointerMoved(from + egui::vec2(-20.0, 0.0))],
    );
    let to = from + egui::vec2(-50.0, 20.0);
    frame_at(
        &mut h,
        10.3,
        vec![
            Event::PointerMoved(to),
            press(to, PointerButton::Primary, false),
        ],
    );
    h.run();
    let (cx, cy, _) = camera(&h);
    assert!((cx - cx0 - 1.0).abs() < 1e-12, "{}", cx - cx0);
    assert!(((cy - cy0).abs() - 0.4).abs() < 1e-12, "{}", cy - cy0);
    assert!(!h.state().wb.can_undo());

    // A drag of a point with a finger.
    use egui::TouchPhase::{End, Move, Start};
    let mut h = touch_harness("A = (0, 0)\n", SIZE);
    let scale = camera(&h).2;
    let mut w = Winit::default();
    let f = drawn_at(&h, 0.0, 0.0);
    frame_at(&mut h, 10.0, w.touch(0, Start, f));
    frame_at(&mut h, 10.2, w.touch(0, Move, f + egui::vec2(30.0, 0.0)));
    let to = f + egui::vec2(60.0, -40.0);
    let mut events = w.touch(0, Move, to);
    events.extend(w.touch(0, End, to));
    frame_at(&mut h, 10.3, events);
    h.run();
    let a = point(&h, "A");
    assert!(
        (a.0 - 60.0 / scale).abs() < 1e-9 && (a.1 - 40.0 / scale).abs() < 1e-9,
        "{a:?}"
    );
    undo(&mut h);
    assert_eq!(point(&h, "A"), (0.0, 0.0));
    assert!(!h.state().wb.can_undo());
}

/// The delete tool's drag is an eraser: what the stroke crosses is deleted
/// (here two segments, their end points stay), the whole stroke one undo
/// step, and it never pans (spec E11, `handleMouseDraggedForDelete`). A
/// stroke within the click threshold erases nothing and does not pan
/// either. Fails on a drag that pans.
#[test]
fn the_delete_tools_stroke_erases_what_it_crosses_as_one_step() {
    let src = "A = (-2, 0)\nB = (2, 0)\nf = Segment(A, B)\n\
               C = (-2, 2)\nD = (2, 2)\ng = Segment(C, D)\n";
    let mut h = harness(src);
    h.state_mut().select_tool(Tool::Delete);
    let cam = camera(&h);
    let from = world(0.3, -1.0);
    frame_at(&mut h, 10.0, vec![Event::PointerMoved(from)]);
    frame_at(
        &mut h,
        10.05,
        vec![press(from, PointerButton::Primary, true)],
    );
    let mut pos = from;
    for i in 0..16 {
        pos = world(0.3, -1.0 + 0.25 * f64::from(i + 1));
        frame_at(
            &mut h,
            10.2 + 0.02 * f64::from(i),
            vec![Event::PointerMoved(pos)],
        );
    }
    frame_at(
        &mut h,
        10.6,
        vec![press(pos, PointerButton::Primary, false)],
    );
    h.run();
    assert_eq!(labels(&h), ["A", "B", "C", "D"]);
    assert_eq!(camera(&h), cam, "the stroke panned");
    undo(&mut h);
    assert_eq!(labels(&h), ["A", "B", "f", "C", "D", "g"]);
    assert!(!h.state().wb.can_undo(), "one step for the stroke");

    let from = world(-3.3, -2.7);
    frame_at(&mut h, 20.0, vec![Event::PointerMoved(from)]);
    frame_at(
        &mut h,
        20.05,
        vec![press(from, PointerButton::Primary, true)],
    );
    let to = from + egui::vec2(6.0, 0.0);
    frame_at(&mut h, 20.2, vec![Event::PointerMoved(to)]);
    frame_at(&mut h, 20.3, vec![press(to, PointerButton::Primary, false)]);
    h.run();
    assert_eq!(labels(&h), ["A", "B", "f", "C", "D", "g"]);
    assert_eq!(camera(&h), cam, "the short stroke panned");
}

fn escape_key() -> Event {
    Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// A press at `from`, a drag to `to` well beyond the click threshold after
/// the reference app's move delay, and the release there.
fn slow_drag(h: &mut Harness<'_, App>, time: f64, from: Pos2, to: Pos2) {
    frame_at(h, time, vec![Event::PointerMoved(from)]);
    frame_at(
        h,
        time + 0.05,
        vec![press(from, PointerButton::Primary, true)],
    );
    for i in 1..=8 {
        let pos = from + (to - from) * (i as f32 / 8.0);
        frame_at(
            h,
            time + 0.2 + 0.02 * f64::from(i),
            vec![Event::PointerMoved(pos)],
        );
    }
    frame_at(
        h,
        time + 0.5,
        vec![press(to, PointerButton::Primary, false)],
    );
    h.run();
}

/// Escape in the middle of the eraser's stroke switches to the move tool
/// and the stroke stops there: what it crossed before stays deleted, as one
/// undo step (the reference deletes as the stroke goes and stores the
/// stroke at the release whatever the mode, `storeUndoAfterDrag`); what it
/// crosses afterwards stays, and the rest of the drag does not pan. Fails if
/// the stroke went on erasing after Escape.
#[test]
fn escape_mid_stroke_stops_the_eraser_and_keeps_what_it_deleted() {
    let src = "A = (-2, 0)\nB = (2, 0)\nf = Segment(A, B)\n\
               C = (-2, 2)\nD = (2, 2)\ng = Segment(C, D)\n";
    let mut h = harness(src);
    h.state_mut().select_tool(Tool::Delete);
    let cam = camera(&h);
    let from = world(0.3, -1.0);
    frame_at(&mut h, 10.0, vec![Event::PointerMoved(from)]);
    frame_at(
        &mut h,
        10.05,
        vec![press(from, PointerButton::Primary, true)],
    );
    let mut pos = from;
    for i in 0..6 {
        pos = world(0.3, -1.0 + 0.25 * f64::from(i + 1));
        frame_at(
            &mut h,
            10.2 + 0.02 * f64::from(i),
            vec![Event::PointerMoved(pos)],
        );
    }
    assert_eq!(labels(&h), ["A", "B", "C", "D", "g"], "f was crossed");
    frame_at(&mut h, 10.4, vec![escape_key()]);
    assert_eq!(h.state().tool.tool(), Tool::Move);
    for i in 0..10 {
        pos = world(0.3, 0.5 + 0.25 * f64::from(i + 1));
        frame_at(
            &mut h,
            10.5 + 0.02 * f64::from(i),
            vec![Event::PointerMoved(pos)],
        );
    }
    frame_at(
        &mut h,
        10.8,
        vec![press(pos, PointerButton::Primary, false)],
    );
    h.run();
    assert_eq!(labels(&h), ["A", "B", "C", "D", "g"]);
    assert_eq!(camera(&h), cam, "the rest of the drag panned");
    assert!(!h.state().erasing);
    assert_eq!(h.state().wb.undo_depth(), (1, 0), "the stroke is stored");
    undo(&mut h);
    assert_eq!(labels(&h), ["A", "B", "f", "C", "D", "g"]);
    assert!(!h.state().wb.can_undo(), "one step for the stroke");
    // The move tool's next drag across g from empty space pans, it erases
    // nothing.
    slow_drag(&mut h, 20.0, world(0.3, 1.0), world(0.3, 3.0));
    assert_eq!(labels(&h), ["A", "B", "f", "C", "D", "g"]);
    assert_ne!(camera(&h), cam, "the move tool's drag did not pan");
}

/// The point tool's press-drag-release is two undo steps, the new point,
/// then its move, as in the reference (the press makes the point and
/// `processMode` stores it; the release beyond the threshold stores the
/// drag, `modeTriggersUndoOnDragGeo`). The segment tool's is one: the press
/// and the release are picks of one use, stored at its end. Fails if the
/// two tools' steps were made alike.
#[test]
fn a_press_drag_release_is_two_steps_for_the_point_tool_and_one_for_the_segment_tool() {
    let mut h = harness("Z = (-3, -3)\n");
    h.state_mut().select_tool(Tool::Point);
    slow_drag(&mut h, 10.0, world(1.0, 1.0), world(3.0, 1.0));
    assert_eq!(labels(&h), ["Z", "A"]);
    assert_eq!(point(&h, "A"), (3.0, 1.0));
    assert_eq!(h.state().wb.undo_depth(), (2, 0));
    undo(&mut h);
    assert_eq!(labels(&h), ["Z", "A"]);
    assert_eq!(
        point(&h, "A"),
        (1.0, 1.0),
        "the first undo takes the move back"
    );
    undo(&mut h);
    assert_eq!(labels(&h), ["Z"]);

    h.state_mut().select_tool(Tool::Segment);
    slow_drag(&mut h, 20.0, world(1.0, 1.0), world(3.0, 1.0));
    assert_eq!(labels(&h), ["Z", "A", "B", "f"]);
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    undo(&mut h);
    assert_eq!(labels(&h), ["Z"]);
}

/// Deleting from the algebra row in the middle of a tool use: the use does
/// not finish, so its point goes and the picks with it; the deletion is the
/// only step. Fails when the pending point stayed as a step of its own.
#[test]
fn a_row_deletion_mid_use_takes_the_use_back() {
    let mut h = harness("Z = (-3, -3)\n");
    h.state_mut().select_tool(Tool::Segment);
    click(&mut h, world(1.0, 1.0));
    assert_eq!(labels(&h), ["Z", "A"]);
    assert_eq!(h.state().tool.picks().len(), 1);
    click(&mut h, row_menu(0.0, false));
    h.get_by_label("Usuń").click();
    h.run();
    assert!(labels(&h).is_empty());
    assert!(h.state().tool.picks().is_empty());
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    undo(&mut h);
    assert_eq!(labels(&h), ["Z"]);
    assert!(!h.state().wb.can_undo());
}

/// A drag of a point that stays within the click threshold is an undo
/// step under the owner's rule; the reference app keeps its motion but no
/// step (spec F2, `interact::EVERY_DRAG_IS_A_STEP`). Fails if the switch
/// did nothing.
#[test]
fn a_drag_within_the_click_threshold_is_a_step_only_under_the_owners_rule() {
    for rules in [OwnerRules::OURS, OwnerRules::REFERENCE] {
        let mut h = harness("A = (0.3, 0.3)\n");
        h.state_mut().rules = rules;
        let from = world(0.3, 0.3);
        frame_at(&mut h, 10.0, vec![Event::PointerMoved(from)]);
        frame_at(
            &mut h,
            10.05,
            vec![press(from, PointerButton::Primary, true)],
        );
        let to = from + egui::vec2(6.0, 0.0);
        frame_at(&mut h, 10.2, vec![Event::PointerMoved(to)]);
        frame_at(&mut h, 10.3, vec![press(to, PointerButton::Primary, false)]);
        h.run();
        let what = format!("{rules:?}");
        assert!((point(&h, "A").0 - 0.42).abs() < 1e-9, "{what}");
        assert_eq!(
            h.state().wb.can_undo(),
            rules.every_drag_is_a_step,
            "{what}"
        );
    }
}
