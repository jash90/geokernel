//! Pointer behaviour of the graphics view that comes from egui's own click
//! and drag rules: a press held still for long is not a click, a press that
//! leaves the area starts a drag at once, and a drag with another button
//! pans. These pin the behaviour of a mouse on the desktop.

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

#[test]
fn a_long_press_is_not_a_tool_click() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().tool.set_tool(Tool::Point);
    let at = world(1.0, 1.0);
    frame_at(&mut h, 10.0, vec![Event::PointerMoved(at)]);
    frame_at(&mut h, 10.1, vec![press(at, PointerButton::Primary, true)]);
    frame_at(&mut h, 11.0, vec![]);
    frame_at(&mut h, 11.1, vec![press(at, PointerButton::Primary, false)]);
    h.run();
    assert_eq!(labels(&h), ["A"], "no point is placed");
}

#[test]
fn after_a_long_press_the_view_pans_by_the_whole_movement() {
    let mut h = harness("A = (0, 0)\n");
    let at = world(-4.0, -3.0);
    let cx0 = h.state().camera.unwrap().cx;
    frame_at(&mut h, 10.0, vec![Event::PointerMoved(at)]);
    frame_at(&mut h, 10.1, vec![press(at, PointerButton::Primary, true)]);
    // Still for longer than a click may take: the drag starts in place.
    frame_at(&mut h, 11.0, vec![]);
    let to = at + egui::vec2(3.0, 0.0);
    frame_at(&mut h, 11.1, vec![Event::PointerMoved(to)]);
    frame_at(&mut h, 11.2, vec![press(to, PointerButton::Primary, false)]);
    h.run();
    let cam = h.state().camera.unwrap();
    assert!((cam.cx - cx0 + 3.0 / 50.0).abs() < 1e-12, "{}", cam.cx);
    assert_eq!(labels(&h), ["A"]);
}

#[test]
fn leaving_the_area_before_the_click_distance_starts_a_pan() {
    // From half a point inside the right edge to five and a half outside
    // (beyond egui's interaction radius): six points, not yet a drag by
    // distance, but the pointer is no longer over the area.
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
    // The drag starts once the pointer is more than 6 points from the press
    // (at 9); from then on every frame's movement pans: 3 + 7 * 3.
    let cam = app.camera.unwrap();
    assert!((cam.cx - cx0 + 24.0 / 50.0).abs() < 1e-12, "{}", cam.cx);
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

#[test]
fn a_press_and_a_far_release_in_the_next_frame_do_nothing() {
    // egui decides neither a click (the pointer moved too far) nor a drag
    // (the button is already up) for such a flick.
    let mut h = harness("A = (1, 1)\n");
    h.state_mut().tool.set_tool(Tool::Point);
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
    assert_eq!(labels(&h), ["A"]);
    assert_eq!(h.state().camera.unwrap().cx, cx0);
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
                o.input_options.max_click_dist = Metrics::touch().drag_slop;
                o.input_options.max_click_duration = Metrics::touch().click_time;
            })
        })
        .build_ui_state(|ui, app: &mut App| crate::app::show(app, ui), app);
    h.run();
    h
}

#[test]
fn with_the_touch_profile_a_wobbly_tap_still_places_a_point() {
    let mut h = touch_harness("A = (0, 0)\n", SIZE);
    h.state_mut().tool.set_tool(Tool::Point);
    let (rect, _) = h.state().view.unwrap();
    let from = Pos2::new(rect.min.x + 100.0, rect.min.y + 100.0);
    moved(&mut h, from);
    button(&mut h, from, true);
    moved(&mut h, from + egui::vec2(10.0, 0.0));
    button(&mut h, from + egui::vec2(10.0, 0.0), false);
    h.run();
    assert_eq!(labels(&h), ["A", "B"]);
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

/// What egui-winit 0.37 makes of touches (`State::on_touch`): every touch
/// event, and pointer events for the finger it follows as the mouse, which
/// is the one that went down while no finger was followed. After that finger
/// lifts no finger is followed, so the moves of a finger that stays are
/// reported as the mouse's, and its end as a release (at the last pointer
/// position, if one is known) that no press came before.
#[derive(Default)]
struct Winit {
    pointer: Option<u64>,
    pos: Option<Pos2>,
}

impl Winit {
    fn touch(&mut self, id: u64, phase: egui::TouchPhase, pos: Pos2) -> Vec<Event> {
        use egui::TouchPhase::{Cancel, End, Move, Start};
        let mut events = vec![Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(id),
            phase,
            pos,
            force: None,
        }];
        if self.pointer.is_some_and(|p| p != id) {
            return events;
        }
        match phase {
            Start => {
                self.pointer = Some(id);
                self.pos = Some(pos);
                events.extend([
                    Event::PointerMoved(pos),
                    press(pos, PointerButton::Primary, true),
                ]);
            }
            Move => {
                self.pos = Some(pos);
                events.push(Event::PointerMoved(pos));
            }
            End => {
                self.pointer = None;
                events.extend(
                    self.pos
                        .take()
                        .map(|p| press(p, PointerButton::Primary, false)),
                );
                events.push(Event::PointerGone);
            }
            Cancel => {
                self.pointer = None;
                self.pos = None;
                events.push(Event::PointerGone);
            }
        }
        events
    }
}

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
