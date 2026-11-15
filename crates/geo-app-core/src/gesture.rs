//! The pointer automaton of the graphics view: presses, moves and releases
//! (and pinches) go in, intents (tap, start a drag, drag to, pan by, end or
//! cancel the drag, zoom about) come out. It holds a few numbers and no
//! collections, and it knows nothing about the construction: what a drag
//! takes hold of is asked from the caller once, when the drag starts.
//!
//! With the desktop metrics it follows egui's rules for a widget that senses
//! clicks and drags, which the window used before:
//!
//! - a press becomes a drag once the pointer is more than `drag_slop` from
//!   the press, or the press is older than `click_time`, but never in the
//!   frame of the press itself; it also becomes one at once when the
//!   pointer is no longer over the area;
//! - a release is a tap when the press neither moved that far nor lasted
//!   that long, and it was made with the primary button;
//! - a drag follows the pointer every frame; a pan moves by the pointer's
//!   movement since the previous frame (so the slop itself is not panned);
//! - the release frame ends the drag without applying its last movement;
//! - the pointer leaving the window does not end a press (a captured mouse
//!   keeps reporting moves outside it), but its position is forgotten, so a
//!   pan does not jump by the way back in.
//!
//! On top of that: a second pointer going down, or a pinch, turns the press
//! into a pinch that never moves a point (a point drag in progress is
//! cancelled and the point goes back); a cancel of the pointer, or the window
//! losing focus, ends the press and puts a dragged point back; and a press
//! of the same button while one is held (its release was lost) ends the old
//! press the same way before it starts.
//!
//! A frame is `begin_frame`, the frame's events in order, then `end_frame`.
//! Each event is decided when it comes, so a release and a new press in one
//! frame end the old drag and start the new press in that order.

use crate::geom::Point;
use crate::metrics::Metrics;

/// Tells pointers apart: the mouse, or one finger.
pub type PointerId = u64;

/// The id of the mouse (and of the first finger, which the platform layer
/// reports as the mouse).
pub const MOUSE: PointerId = 0;

/// The id of a finger other than the first one, from the platform's touch id.
fn finger(touch: u64) -> PointerId {
    touch | 1 << 63
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Primary,
    /// The secondary or middle button: it can pan but never taps.
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Down {
        pos: Point,
        id: PointerId,
        button: Button,
    },
    Move {
        pos: Point,
        id: PointerId,
    },
    Up {
        pos: Point,
        id: PointerId,
        button: Button,
    },
    /// The platform took this pointer away (a cancelled touch): its press
    /// ends without a tap and a dragged point goes back.
    Cancel {
        id: PointerId,
    },
    /// The pointer left the window. A held press goes on; the position is
    /// unknown until the pointer moves again.
    Gone,
    /// The window lost the focus: a held press ends as with `Cancel`.
    FocusLost,
}

/// The phase of a touch, as the platform reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Start,
    Move,
    End,
    Cancel,
}

/// What a drag takes hold of, decided by the caller from the press position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grab {
    /// A free point, which follows the pointer.
    Point,
    /// The view, which pans with the pointer.
    Pan,
    /// Nothing: the drag does nothing until it ends.
    Nothing,
}

/// What a frame asks the caller to do, in this order: put back or end the
/// drag that ended, start the new one, follow the pointer, pan, tap.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Intents {
    /// Put a dragged point back where the drag started.
    pub cancel_drag: bool,
    /// The drag ended (the point stays where it is).
    pub end_drag: bool,
    /// A drag started this frame and took hold of this.
    pub started: Option<Grab>,
    /// Move the dragged point to follow the pointer, now here.
    pub drag_to: Option<Point>,
    /// Pan the view by this many points.
    pub pan: Option<(f32, f32)>,
    /// A tap here: a tool picks what is under it, or places something.
    pub tap: Option<Point>,
}

/// A change of scale from the wheel, a trackpad or two fingers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zoom {
    /// Zoom by this factor about `center`.
    pub factor: f64,
    pub center: Point,
    /// Then pan by this many points (two fingers moving together).
    pub pan: (f32, f32),
    /// Put a dragged point back first: a second finger took over.
    pub cancel_drag: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Point,
    Pan,
    /// A drag whose target is gone (the construction was rebuilt).
    Inert,
    Pinch,
}

impl From<Grab> for Mode {
    fn from(g: Grab) -> Mode {
        match g {
            Grab::Point => Mode::Point,
            Grab::Pan => Mode::Pan,
            Grab::Nothing => Mode::Inert,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Press {
    id: PointerId,
    button: Button,
    origin: Point,
    start: f64,
    /// The pointer went further than the slop from the press.
    moved: bool,
    /// Escape ended what the press did: it can no longer start a drag.
    no_drag: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Gesture {
    slop: f32,
    click_time: f64,
    time: f64,
    /// The last known pointer position, and that at the start of the frame.
    latest: Option<Point>,
    frame_start: Option<Point>,
    press: Option<Press>,
    /// What the press is doing once it is a drag (or a pinch).
    mode: Option<Mode>,
    pressed_now: bool,
    /// The touch id of the finger that the platform reports as the mouse.
    primary_finger: Option<u64>,
    /// Intents decided by events, handed out by `end_frame`.
    pending: Intents,
}

fn distance(a: Point, b: Point) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

impl Gesture {
    pub fn new(m: &Metrics) -> Gesture {
        Gesture {
            slop: m.drag_slop,
            click_time: m.click_time,
            time: 0.0,
            latest: None,
            frame_start: None,
            press: None,
            mode: None,
            pressed_now: false,
            primary_finger: None,
            pending: Intents::default(),
        }
    }

    /// A drag is in progress that pans the view.
    pub fn panning(&self) -> bool {
        self.mode == Some(Mode::Pan)
    }

    /// A press is held (whether or not it is a drag yet).
    pub fn pressed(&self) -> bool {
        self.press.is_some()
    }

    /// Starts a frame at `time` seconds.
    pub fn begin_frame(&mut self, time: f64) {
        self.time = time;
        self.frame_start = self.latest;
        self.pressed_now = false;
    }

    pub fn event(&mut self, event: Event) {
        match event {
            Event::Down { pos, id, button } => match self.press {
                Some(p) if p.id != id => self.start_pinch(),
                // Another button of the same mouse: the first press goes on,
                // as in egui (the first release of either ends it).
                Some(p) if p.button != button => {}
                // The release of the held press never came.
                Some(_) => {
                    self.abort();
                    self.begin(pos, id, button);
                }
                None => self.begin(pos, id, button),
            },
            Event::Move { pos, id } => {
                // Only the pointer of the press moves it; with no press, only
                // the mouse (the first finger) has a position worth keeping.
                if self.press.map_or(id != MOUSE, |p| p.id != id) {
                    return;
                }
                self.latest = Some(pos);
                if let Some(p) = &mut self.press {
                    p.moved |= distance(p.origin, pos) > self.slop;
                }
            }
            Event::Up { pos, id, button } => {
                let Some(p) = self.press else {
                    if id == MOUSE {
                        self.latest = Some(pos);
                    }
                    return;
                };
                if p.id != id {
                    // A second finger lifted: the pinch goes on until the
                    // first one does.
                    return;
                }
                self.latest = Some(pos);
                let long = self.time - p.start > self.click_time;
                let tap = !p.moved
                    && !long
                    && button == Button::Primary
                    && self.mode != Some(Mode::Pinch);
                if matches!(self.mode, Some(Mode::Point | Mode::Pan | Mode::Inert)) {
                    self.pending.end_drag = true;
                }
                if tap {
                    self.pending.tap = Some(pos);
                }
                self.mode = None;
                self.press = None;
            }
            Event::Cancel { id } => {
                if self.press.is_some_and(|p| p.id == id) {
                    self.abort();
                    self.forget_position();
                }
            }
            Event::Gone => self.forget_position(),
            Event::FocusLost => {
                self.abort();
                self.forget_position();
            }
        }
    }

    /// A touch of the finger with the platform's id `touch`. The platform
    /// layer also reports the first finger down as the mouse (egui-winit
    /// sends its pointer events right after the touch event), so that
    /// finger's touches count only for a cancel, which the mouse events do
    /// not carry. Any other finger is a pointer of its own; it matters only
    /// while a press is held, which it turns into a pinch.
    pub fn touch(&mut self, touch: u64, phase: TouchPhase, pos: Point) {
        match self.primary_finger {
            None if phase == TouchPhase::Start => {
                self.primary_finger = Some(touch);
                return;
            }
            Some(f) if f == touch => {
                if matches!(phase, TouchPhase::End | TouchPhase::Cancel) {
                    self.primary_finger = None;
                }
                if phase == TouchPhase::Cancel {
                    self.event(Event::Cancel { id: MOUSE });
                }
                return;
            }
            _ => {}
        }
        let id = finger(touch);
        match phase {
            TouchPhase::Start if self.press.is_some() => self.event(Event::Down {
                pos,
                id,
                button: Button::Primary,
            }),
            TouchPhase::Start => {}
            TouchPhase::Move => self.event(Event::Move { pos, id }),
            TouchPhase::End => self.event(Event::Up {
                pos,
                id,
                button: Button::Primary,
            }),
            TouchPhase::Cancel => self.event(Event::Cancel { id }),
        }
    }

    fn begin(&mut self, pos: Point, id: PointerId, button: Button) {
        self.latest = Some(pos);
        self.press = Some(Press {
            id,
            button,
            origin: pos,
            start: self.time,
            moved: false,
            no_drag: false,
        });
        self.pressed_now = true;
        self.mode = None;
    }

    /// Ends the press without a tap; a dragged point goes back.
    fn abort(&mut self) {
        if self.mode == Some(Mode::Point) {
            self.pending.cancel_drag = true;
        }
        self.mode = None;
        self.press = None;
    }

    fn forget_position(&mut self) {
        self.latest = None;
        self.frame_start = None;
    }

    /// Ends whatever is held, as a cancel, and hands out what is still to do
    /// (the area went away, so no `end_frame` follows).
    pub fn cancel(&mut self) -> Intents {
        self.abort();
        std::mem::take(&mut self.pending)
    }

    /// Escape: the drag in progress ends where it is, and the press can no
    /// longer start one (a release may still be a tap).
    pub fn stop(&mut self) {
        if matches!(self.mode, Some(Mode::Point | Mode::Pan | Mode::Inert)) {
            self.pending.end_drag = true;
            self.mode = None;
        }
        if let Some(p) = &mut self.press {
            p.no_drag = true;
        }
    }

    /// The construction was rebuilt: what a drag holds is gone, so the drag
    /// does nothing more until it ends.
    pub fn forget_target(&mut self) {
        if matches!(self.mode, Some(Mode::Point | Mode::Pan)) {
            self.mode = Some(Mode::Inert);
        }
    }

    /// The wheel or a trackpad asks to zoom by `factor` about `center`.
    pub fn zoom(&self, factor: f64, center: Point) -> Option<Zoom> {
        (factor != 1.0).then_some(Zoom {
            factor,
            center,
            pan: (0.0, 0.0),
            cancel_drag: false,
        })
    }

    /// Two or more fingers are down: zoom about their centre and pan with
    /// them. The press becomes a pinch; it moves no point any more.
    pub fn pinch(&mut self, factor: f64, center: Point, pan: (f32, f32)) -> Zoom {
        let cancel_drag = self.mode == Some(Mode::Point);
        if self.press.is_some() {
            self.mode = Some(Mode::Pinch);
        }
        Zoom {
            factor,
            center,
            pan,
            cancel_drag,
        }
    }

    fn start_pinch(&mut self) {
        if self.mode == Some(Mode::Point) {
            self.pending.cancel_drag = true;
        }
        self.mode = Some(Mode::Pinch);
    }

    /// Ends the frame. `inside` tells whether the pointer is over the area;
    /// `grab` is asked, with the press position, what a drag of the primary
    /// button takes hold of when one starts.
    pub fn end_frame(&mut self, inside: bool, grab: impl FnOnce(Point) -> Grab) -> Intents {
        let mut out = std::mem::take(&mut self.pending);
        let Some(p) = self.press else {
            return out;
        };
        if self.mode.is_none() && !p.no_drag {
            let long = self.time - p.start > self.click_time;
            let decided = !self.pressed_now && (p.moved || long);
            if decided || !inside {
                let g = match p.button {
                    Button::Primary => grab(p.origin),
                    Button::Other => Grab::Pan,
                };
                self.mode = Some(g.into());
                out.started = Some(g);
            }
        }
        match self.mode {
            Some(Mode::Point) => out.drag_to = self.latest,
            Some(Mode::Pan) => {
                out.pan = Some(match (self.frame_start, self.latest) {
                    (Some(a), Some(b)) => (b.x - a.x, b.y - a.y),
                    _ => (0.0, 0.0),
                });
            }
            _ => {}
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f32, y: f32) -> Point {
        Point::new(x, y)
    }

    fn down(x: f32, y: f32) -> Event {
        Event::Down {
            pos: p(x, y),
            id: MOUSE,
            button: Button::Primary,
        }
    }

    fn mv(x: f32, y: f32) -> Event {
        Event::Move {
            pos: p(x, y),
            id: MOUSE,
        }
    }

    fn up(x: f32, y: f32) -> Event {
        Event::Up {
            pos: p(x, y),
            id: MOUSE,
            button: Button::Primary,
        }
    }

    /// Runs one frame and returns its intents; `grab` answers a drag start.
    fn frame(g: &mut Gesture, time: f64, events: &[Event], grab: Grab) -> Intents {
        g.begin_frame(time);
        for &e in events {
            g.event(e);
        }
        g.end_frame(true, |_| grab)
    }

    fn mouse() -> Gesture {
        Gesture::new(&Metrics::desktop())
    }

    #[test]
    fn a_click_is_a_tap_at_the_release() {
        let mut g = mouse();
        assert_eq!(
            frame(&mut g, 0.0, &[mv(5.0, 5.0)], Grab::Point),
            Intents::default()
        );
        assert_eq!(
            frame(&mut g, 0.1, &[down(5.0, 5.0)], Grab::Point),
            Intents::default()
        );
        assert_eq!(
            frame(&mut g, 0.2, &[mv(8.0, 5.0)], Grab::Point),
            Intents::default()
        );
        let out = frame(&mut g, 0.3, &[up(8.0, 5.0)], Grab::Point);
        assert_eq!(out.tap, Some(p(8.0, 5.0)));
        assert!(!out.end_drag && out.started.is_none());
    }

    #[test]
    fn a_press_and_release_in_one_frame_is_a_tap() {
        let mut g = mouse();
        let out = frame(
            &mut g,
            0.0,
            &[mv(1.0, 2.0), down(1.0, 2.0), up(1.0, 2.0)],
            Grab::Point,
        );
        assert_eq!(out.tap, Some(p(1.0, 2.0)));
    }

    #[test]
    fn a_drag_starts_beyond_the_slop_and_follows_the_pointer() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        // Exactly the slop away is still a click.
        assert_eq!(
            frame(&mut g, 0.1, &[mv(6.0, 0.0)], Grab::Point),
            Intents::default()
        );
        let out = frame(&mut g, 0.2, &[mv(7.0, 0.0)], Grab::Point);
        assert_eq!(out.started, Some(Grab::Point));
        assert_eq!(out.drag_to, Some(p(7.0, 0.0)));
        let out = frame(&mut g, 0.3, &[mv(9.0, 1.0)], Grab::Point);
        assert_eq!((out.started, out.drag_to), (None, Some(p(9.0, 1.0))));
        // The release ends the drag and does not apply its own movement.
        let out = frame(&mut g, 0.4, &[mv(30.0, 1.0), up(30.0, 1.0)], Grab::Point);
        assert!(out.end_drag);
        assert_eq!((out.drag_to, out.tap), (None, None));
    }

    #[test]
    fn a_pan_moves_by_each_frame_but_not_by_the_slop() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Pan);
        let mut panned = 0.0;
        for (i, x) in [3.0, 6.0, 9.0, 12.0].into_iter().enumerate() {
            let out = frame(&mut g, 0.1 * (i + 1) as f64, &[mv(x, 0.0)], Grab::Pan);
            panned += out.pan.map_or(0.0, |d| d.0);
        }
        assert_eq!(panned, 6.0);
    }

    #[test]
    fn no_drag_starts_in_the_frame_of_the_press() {
        let mut g = mouse();
        let out = frame(&mut g, 0.0, &[down(0.0, 0.0), mv(20.0, 0.0)], Grab::Pan);
        assert_eq!(out, Intents::default());
        let out = frame(&mut g, 0.1, &[], Grab::Pan);
        assert_eq!((out.started, out.pan), (Some(Grab::Pan), Some((0.0, 0.0))));
    }

    #[test]
    fn a_long_press_becomes_a_drag_in_place_and_is_no_tap() {
        let mut g = mouse();
        frame(&mut g, 1.0, &[down(0.0, 0.0)], Grab::Pan);
        assert_eq!(frame(&mut g, 1.5, &[], Grab::Pan), Intents::default());
        let out = frame(&mut g, 1.9, &[], Grab::Pan);
        assert_eq!(out.started, Some(Grab::Pan));
        let out = frame(&mut g, 2.0, &[mv(3.0, 0.0)], Grab::Pan);
        assert_eq!(out.pan, Some((3.0, 0.0)));
        // Released without a frame in between: still no tap.
        let mut g = mouse();
        frame(&mut g, 1.0, &[down(0.0, 0.0)], Grab::Point);
        let out = frame(&mut g, 1.9, &[up(0.0, 0.0)], Grab::Point);
        assert_eq!(out, Intents::default());
    }

    #[test]
    fn leaving_the_area_starts_the_drag_at_once() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Pan);
        g.begin_frame(0.1);
        g.event(mv(3.0, 0.0));
        let out = g.end_frame(false, |_| Grab::Pan);
        assert_eq!((out.started, out.pan), (Some(Grab::Pan), Some((3.0, 0.0))));
    }

    #[test]
    fn the_other_buttons_pan_and_never_tap() {
        let mut g = mouse();
        let other = |x: f32, pressed: bool| {
            let pos = p(x, 0.0);
            if pressed {
                Event::Down {
                    pos,
                    id: MOUSE,
                    button: Button::Other,
                }
            } else {
                Event::Up {
                    pos,
                    id: MOUSE,
                    button: Button::Other,
                }
            }
        };
        frame(&mut g, 0.0, &[other(0.0, true)], Grab::Point);
        assert_eq!(
            frame(&mut g, 0.1, &[other(0.0, false)], Grab::Point),
            Intents::default()
        );
        frame(&mut g, 0.2, &[other(0.0, true)], Grab::Point);
        // `grab` is not asked: a drag of another button always pans.
        let out = frame(&mut g, 0.3, &[mv(10.0, 0.0)], Grab::Point);
        assert_eq!(out.started, Some(Grab::Pan));
    }

    #[test]
    fn a_touch_tap_may_wobble_within_the_larger_slop() {
        let mut g = Gesture::new(&Metrics::touch());
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        assert_eq!(
            frame(&mut g, 0.05, &[mv(10.0, 0.0)], Grab::Point),
            Intents::default()
        );
        let out = frame(&mut g, 0.1, &[up(10.0, 0.0)], Grab::Point);
        assert_eq!(out.tap, Some(p(10.0, 0.0)));
        // Further than the touch slop it is a drag.
        frame(&mut g, 1.0, &[down(0.0, 0.0)], Grab::Point);
        let out = frame(&mut g, 1.05, &[mv(13.0, 0.0)], Grab::Point);
        assert_eq!(out.started, Some(Grab::Point));
    }

    #[test]
    fn a_pinch_while_a_point_is_held_puts_the_point_back_and_moves_it_no_more() {
        let mut g = Gesture::new(&Metrics::touch());
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        let out = frame(&mut g, 0.1, &[mv(20.0, 0.0)], Grab::Point);
        assert_eq!(out.started, Some(Grab::Point));
        g.begin_frame(0.2);
        let zoom = g.pinch(1.1, p(30.0, 30.0), (2.0, 0.0));
        assert!(zoom.cancel_drag);
        assert_eq!((zoom.factor, zoom.pan), (1.1, (2.0, 0.0)));
        assert_eq!(g.end_frame(true, |_| Grab::Point), Intents::default());
        let out = frame(&mut g, 0.3, &[mv(40.0, 0.0)], Grab::Point);
        assert_eq!(out, Intents::default());
        // The point drag was cancelled, so the release ends nothing.
        let out = frame(&mut g, 0.4, &[up(40.0, 0.0)], Grab::Point);
        assert_eq!(out, Intents::default());
    }

    #[test]
    fn a_second_finger_turns_a_press_into_a_pinch() {
        // The first finger comes as a touch and as the mouse; only the
        // mouse events of it count.
        let first = |g: &mut Gesture, phase| g.touch(3, phase, p(0.0, 0.0));
        let mut g = Gesture::new(&Metrics::touch());
        g.begin_frame(0.0);
        first(&mut g, TouchPhase::Start);
        g.event(down(0.0, 0.0));
        g.end_frame(true, |_| Grab::Point);
        frame(&mut g, 0.1, &[mv(20.0, 0.0)], Grab::Point);
        g.begin_frame(0.2);
        g.touch(8, TouchPhase::Start, p(100.0, 0.0));
        let out = g.end_frame(true, |_| Grab::Point);
        assert!(out.cancel_drag);
        assert_eq!(out.drag_to, None);
        // Before the slop the press would have been a tap; now it is not.
        let mut g = Gesture::new(&Metrics::touch());
        g.begin_frame(0.0);
        first(&mut g, TouchPhase::Start);
        g.event(down(0.0, 0.0));
        g.touch(8, TouchPhase::Start, p(100.0, 0.0));
        g.end_frame(true, |_| Grab::Point);
        g.begin_frame(0.1);
        first(&mut g, TouchPhase::End);
        g.event(up(0.0, 0.0));
        assert_eq!(g.end_frame(true, |_| Grab::Point), Intents::default());
    }

    #[test]
    fn a_finger_without_a_press_is_ignored() {
        let mut g = Gesture::new(&Metrics::touch());
        g.begin_frame(0.0);
        g.touch(3, TouchPhase::Start, p(0.0, 0.0));
        g.touch(3, TouchPhase::End, p(0.0, 0.0));
        // A second finger alone (the first one lifted) does nothing.
        g.touch(5, TouchPhase::Start, p(50.0, 0.0));
        assert!(!g.pressed());
        g.touch(5, TouchPhase::Move, p(90.0, 0.0));
        g.touch(5, TouchPhase::End, p(90.0, 0.0));
        assert_eq!(g.end_frame(true, |_| Grab::Pan), Intents::default());
        assert_eq!(g.latest, None);
    }

    #[test]
    fn after_a_pinch_the_finger_that_stays_moves_nothing() {
        for first_lifts in [false, true] {
            let mut g = Gesture::new(&Metrics::touch());
            g.begin_frame(0.0);
            g.touch(1, TouchPhase::Start, p(0.0, 0.0));
            g.event(down(0.0, 0.0));
            g.end_frame(true, |_| Grab::Pan);
            g.begin_frame(0.1);
            g.touch(2, TouchPhase::Start, p(80.0, 0.0));
            g.pinch(1.2, p(40.0, 0.0), (0.0, 0.0));
            g.end_frame(true, |_| Grab::Pan);
            g.begin_frame(0.2);
            if first_lifts {
                g.touch(1, TouchPhase::End, p(0.0, 0.0));
                g.event(up(0.0, 0.0));
                g.event(Event::Gone);
            } else {
                g.touch(2, TouchPhase::End, p(80.0, 0.0));
            }
            assert_eq!(g.end_frame(true, |_| Grab::Pan), Intents::default());
            // The finger that stays moves far from where the press began.
            for (i, x) in [30.0, 60.0, 90.0].into_iter().enumerate() {
                g.begin_frame(0.3 + 0.1 * i as f64);
                if first_lifts {
                    g.touch(2, TouchPhase::Move, p(80.0 + x, 0.0));
                } else {
                    g.touch(1, TouchPhase::Move, p(x, 0.0));
                    g.event(mv(x, 0.0));
                }
                let out = g.end_frame(true, |_| Grab::Pan);
                assert_eq!(out, Intents::default(), "first lifts: {first_lifts}");
            }
        }
    }

    #[test]
    fn a_cancel_counts_only_for_the_finger_of_the_press() {
        let mut g = Gesture::new(&Metrics::touch());
        g.begin_frame(0.0);
        g.touch(1, TouchPhase::Start, p(0.0, 0.0));
        g.event(down(0.0, 0.0));
        g.end_frame(true, |_| Grab::Point);
        let out = frame(&mut g, 0.1, &[mv(30.0, 0.0)], Grab::Point);
        assert_eq!(out.started, Some(Grab::Point));
        g.begin_frame(0.2);
        g.touch(2, TouchPhase::Start, p(90.0, 0.0));
        g.touch(2, TouchPhase::Cancel, p(90.0, 0.0));
        assert!(g.pressed());
        g.end_frame(true, |_| Grab::Point);
        g.begin_frame(0.3);
        g.touch(1, TouchPhase::Cancel, p(30.0, 0.0));
        g.event(Event::Gone);
        assert!(!g.pressed());
        assert_eq!(g.end_frame(true, |_| Grab::Point), Intents::default());
    }

    #[test]
    fn a_press_of_the_held_button_ends_the_old_press_first() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.1, &[mv(20.0, 0.0)], Grab::Point);
        // The release was lost: the point goes back, the new press starts
        // fresh and is a tap of its own.
        let out = frame(&mut g, 0.2, &[down(50.0, 50.0)], Grab::Point);
        assert!(out.cancel_drag && !out.end_drag);
        assert_eq!((out.started, out.drag_to), (None, None));
        let out = frame(&mut g, 0.3, &[up(50.0, 50.0)], Grab::Point);
        assert_eq!(out.tap, Some(p(50.0, 50.0)));
        // Another button while one is held: the first press goes on.
        frame(&mut g, 1.0, &[down(0.0, 0.0)], Grab::Point);
        let other = Event::Down {
            pos: p(0.0, 0.0),
            id: MOUSE,
            button: Button::Other,
        };
        assert_eq!(
            frame(&mut g, 1.1, &[other], Grab::Point),
            Intents::default()
        );
        let out = frame(&mut g, 1.2, &[mv(20.0, 0.0)], Grab::Point);
        assert_eq!(out.started, Some(Grab::Point));
    }

    #[test]
    fn a_release_and_a_press_in_one_frame_end_the_drag_then_start_the_press() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.1, &[mv(20.0, 0.0)], Grab::Point);
        let out = frame(&mut g, 0.2, &[up(20.0, 0.0), down(60.0, 0.0)], Grab::Point);
        assert!(out.end_drag && !out.cancel_drag);
        assert_eq!((out.started, out.drag_to, out.tap), (None, None, None));
        assert!(g.pressed());
        // The new press drags from where it went down.
        let mut origin = None;
        g.begin_frame(0.3);
        g.event(mv(70.0, 0.0));
        let out = g.end_frame(true, |o| {
            origin = Some(o);
            Grab::Point
        });
        assert_eq!(origin, Some(p(60.0, 0.0)));
        assert_eq!(out.drag_to, Some(p(70.0, 0.0)));
        // A tap and a press in one frame: the tap is at its release.
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(5.0, 5.0)], Grab::Point);
        let out = frame(&mut g, 0.1, &[up(5.0, 5.0), down(9.0, 9.0)], Grab::Point);
        assert_eq!(out.tap, Some(p(5.0, 5.0)));
        assert!(g.pressed());
    }

    #[test]
    fn leaving_the_window_keeps_the_press_and_the_pan_does_not_jump() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Pan);
        let out = frame(&mut g, 0.1, &[mv(10.0, 0.0)], Grab::Pan);
        assert_eq!(out.pan, Some((10.0, 0.0)));
        // Out of the window and back in somewhere else in one frame.
        let out = frame(&mut g, 0.2, &[Event::Gone, mv(200.0, 90.0)], Grab::Pan);
        assert_eq!(out.pan, Some((0.0, 0.0)));
        let out = frame(&mut g, 0.3, &[mv(205.0, 90.0)], Grab::Pan);
        assert_eq!(out.pan, Some((5.0, 0.0)));
        // Moves outside the window still pan; the release ends the drag.
        let out = frame(&mut g, 0.4, &[Event::Gone], Grab::Pan);
        assert_eq!(out.pan, Some((0.0, 0.0)));
        let out = frame(&mut g, 0.5, &[mv(-40.0, 0.0)], Grab::Pan);
        assert_eq!(out.pan, Some((0.0, 0.0)));
        let out = frame(&mut g, 0.6, &[mv(-50.0, 0.0)], Grab::Pan);
        assert_eq!(out.pan, Some((-10.0, 0.0)));
        assert!(frame(&mut g, 0.7, &[up(-50.0, 0.0)], Grab::Pan).end_drag);
    }

    #[test]
    fn losing_the_focus_cancels_the_press() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.1, &[mv(20.0, 0.0)], Grab::Point);
        let out = frame(&mut g, 0.2, &[Event::FocusLost], Grab::Point);
        assert!(out.cancel_drag && !out.end_drag);
        assert!(!g.pressed());
        assert_eq!(
            frame(&mut g, 0.3, &[mv(40.0, 0.0), up(40.0, 0.0)], Grab::Point),
            Intents::default()
        );
        // Without a frame end (the area went away) the cancel is handed out
        // at once.
        frame(&mut g, 1.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 1.1, &[mv(20.0, 0.0)], Grab::Point);
        let out = g.cancel();
        assert!(out.cancel_drag && !g.pressed());
    }

    #[test]
    fn cancel_puts_the_point_back_and_ends_the_press() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.1, &[mv(20.0, 0.0)], Grab::Point);
        let out = frame(&mut g, 0.2, &[Event::Cancel { id: MOUSE }], Grab::Point);
        assert!(out.cancel_drag && !out.end_drag);
        assert_eq!(
            frame(&mut g, 0.3, &[mv(30.0, 0.0)], Grab::Point),
            Intents::default()
        );
        assert_eq!(
            frame(&mut g, 0.4, &[up(30.0, 0.0)], Grab::Point),
            Intents::default()
        );
    }

    #[test]
    fn escape_ends_the_drag_and_the_press_starts_no_other() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Pan);
        frame(&mut g, 0.1, &[mv(20.0, 0.0)], Grab::Pan);
        g.begin_frame(0.2);
        g.stop();
        g.event(mv(25.0, 0.0));
        let out = g.end_frame(true, |_| Grab::Pan);
        assert!(out.end_drag);
        assert_eq!((out.started, out.pan), (None, None));
        assert_eq!(
            frame(&mut g, 0.3, &[mv(40.0, 0.0)], Grab::Pan),
            Intents::default()
        );
        assert_eq!(
            frame(&mut g, 0.4, &[up(40.0, 0.0)], Grab::Pan),
            Intents::default()
        );
    }

    #[test]
    fn a_rebuild_leaves_the_drag_inert_until_it_ends() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.1, &[mv(20.0, 0.0)], Grab::Point);
        g.forget_target();
        assert!(!g.panning());
        assert_eq!(
            frame(&mut g, 0.2, &[mv(30.0, 0.0)], Grab::Point),
            Intents::default()
        );
        assert!(frame(&mut g, 0.3, &[up(30.0, 0.0)], Grab::Point).end_drag);
    }

    #[test]
    fn the_wheel_zooms_only_when_it_changes_the_scale() {
        let g = mouse();
        assert_eq!(g.zoom(1.0, p(0.0, 0.0)), None);
        assert_eq!(g.zoom(1.25, p(3.0, 4.0)).map(|z| z.factor), Some(1.25));
    }
}
