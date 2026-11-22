// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianController.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianConstants.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The pointer automaton of the graphics view: presses, moves and releases
//! (and pinches) go in, intents (tap, start a drag, drag to, pan by, end or
//! cancel the drag, zoom about) come out. It holds a few numbers and no
//! collections, and it knows nothing about the construction: what a drag
//! takes hold of is asked from the caller once, when the drag starts.
//!
//! It owns the whole decision between a click and a drag (egui's click
//! rules take no part in the drawing) and follows the reference app's
//! `EuclidianController` (GeoGebra, `common/.../euclidian/
//! EuclidianController.java`):
//!
//! - moves that come less than `drag_delay` (100 ms) after the press are
//!   dropped (`shouldCancelDrag`, called from `wrapMouseDragged` and
//!   `wrapMouseDraggedND`), for every pointer; there is no distance a press
//!   must go before it drags;
//! - the first move after the delay starts the drag and takes the dragged
//!   object straight to the pointer, so motion made during the delay is not
//!   lost (`handleMovedElementFreePoint` sets no grab offset); a pan follows
//!   the pointer exactly, by its movement from the press (`moveView`,
//!   `snapMoveView`);
//! - a drag is "beyond the threshold" once the pointer went more than
//!   `drag_threshold` (10 px) from the press on either axis
//!   (`isDraggingBeyondThreshold`). Upstream checks it before it takes the
//!   new position (`wrapMouseDraggedND`), so it tests the previous accepted
//!   move: one move late. It decides only the click: a release of the
//!   primary button whose drag is not beyond it is a tap
//!   (`wrapMouseReleasedND` runs the click path, `switchModeForMouseReleased`),
//!   even when the motion moved something; the motion is applied either way;
//! - there is no long press for the mouse: a still press released after any
//!   time is a tap;
//! - the release ends the drag and does not move anything itself
//!   (`wrapMouseReleasedND`);
//! - the pointer leaving the window does not end a press (a captured mouse
//!   keeps reporting moves outside it), but its position is forgotten, so a
//!   pan does not jump by the way back in.
//!
//! Kept from before: a second finger landing in the area turns the press
//! into a pinch that never moves a point (a point drag in progress is
//! cancelled and the point goes back), and so does a press made while such a
//! finger is down; a finger landing anywhere else (the toolbar, the keyboard,
//! a button over the area) is ignored, and a drag goes on. A cancel of the
//! pointer, or the window losing focus, ends the press and puts a dragged
//! point back; and a press of the same button while one is held (its
//! release was lost) ends the old press the same way before it starts.
//!
//! A frame is `begin_frame`, the frame's events in order, then `end_frame`.
//! Each event is decided when it comes, at the frame's time, so a release
//! and a new press in one frame end the old drag and start the new press in
//! that order.

use crate::geom::Point;
use crate::metrics::Metrics;

/// Tells pointers apart: the mouse, or one finger.
pub type PointerId = u64;

/// The id of the mouse (and of the first finger, which the platform layer
/// reports as the mouse).
pub const MOUSE: PointerId = 0;

/// How many fingers besides the first one are followed; one more is ignored.
const FINGERS: usize = 4;

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
    /// An object, which follows the pointer.
    Point,
    /// The view, which pans with the pointer.
    Pan,
    /// Nothing: the drag does nothing until it ends.
    Nothing,
    /// A rectangle the zoom in tool will zoom to (`processZoomRectangle`);
    /// the drag only shows it.
    Rect,
    /// An axis the drag rescales (`scaleXAxis` / `scaleYAxis`): the view
    /// move tool, or Shift with the move tool, pressing on an axis.
    Axis,
}

/// How a press that ended went, for the undo rules of a drag: whether it
/// went beyond the click threshold, how many moves were accepted (after the
/// delay) and how far, on either axis, the pointer got from the press.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Release {
    pub beyond_threshold: bool,
    pub moves: u32,
    pub max_distance: f32,
}

/// What a frame asks the caller to do, in this order: start the new drag,
/// follow the pointer, pan, put back or end the drag that ended, tap.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Intents {
    /// A drag started this frame and took hold of this.
    pub started: Option<Grab>,
    /// Move the dragged object to follow the pointer, now here.
    pub drag_to: Option<Point>,
    /// Pan the view by this many points.
    pub pan: Option<(f32, f32)>,
    /// Put a dragged object back where the drag started.
    pub cancel_drag: bool,
    /// Put the view back where the drag or pinch that moved it started
    /// (the press was taken away).
    pub cancel_view: bool,
    /// The drag ended (the object stays where it is).
    pub end_drag: bool,
    /// A press ended with a release (not a cancel): how it went.
    pub released: Option<Release>,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Point,
    Pan,
    /// A drag whose target is gone (the construction was rebuilt).
    Inert,
    /// A rectangle the zoom in tool drags out.
    Rect,
    /// An axis a drag rescales.
    Axis,
    Pinch,
}

impl Mode {
    /// Whether a drag this mode stands for ends at the release of the
    /// press.
    fn is_drag(self) -> bool {
        matches!(
            self,
            Mode::Point | Mode::Pan | Mode::Inert | Mode::Rect | Mode::Axis
        )
    }
}

impl From<Grab> for Mode {
    fn from(g: Grab) -> Mode {
        match g {
            Grab::Point => Mode::Point,
            Grab::Pan => Mode::Pan,
            Grab::Nothing => Mode::Inert,
            Grab::Rect => Mode::Rect,
            Grab::Axis => Mode::Axis,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Press {
    id: PointerId,
    button: Button,
    origin: Point,
    start: f64,
    /// The position of the last accepted move (the press at first): the
    /// threshold is checked against it before it is replaced.
    last: Point,
    beyond: bool,
    moves: u32,
    max_distance: f32,
    /// Where a pan goes on from: the last accepted position, or nothing
    /// after the pointer left the window (the next move only re-anchors).
    anchor: Option<Point>,
    /// Escape ended what the press did: it can no longer start a drag.
    no_drag: bool,
}

impl Press {
    fn release(&self) -> Release {
        Release {
            beyond_threshold: self.beyond,
            moves: self.moves,
            max_distance: self.max_distance,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Gesture {
    drag_delay: f64,
    drag_threshold: f32,
    time: f64,
    press: Option<Press>,
    /// The latest press was a finger's.
    touch_last: bool,
    /// What the press is doing once it is a drag (or a pinch).
    mode: Option<Mode>,
    /// What the press held before a pinch suspended it: the drag the
    /// pressing finger goes on with when the other finger lifts
    /// (`twoTouchStart` / `twoTouchEnd`; `None` while no pinch is going).
    before_pinch: Option<Mode>,
    /// A press that ended (released) with accepted moves before its drag
    /// was decided: `end_frame` decides it from this press position (and
    /// button), applies its motion and ends it.
    unresolved: Option<(Point, Button)>,
    /// A running drag that ended (released) this frame: `end_frame` still
    /// applies the motion accepted before the release, as the reference app
    /// handles each drag event before the release
    /// (`wrapMouseDraggedND`, then `wrapMouseReleasedND`).
    ended: Option<Mode>,
    /// Motion accepted this frame: the newest position, and the pan since
    /// the last frame.
    moved_to: Option<Point>,
    panned: (f32, f32),
    /// The touch id of the finger that the platform reports as the mouse.
    primary_finger: Option<u64>,
    /// The touch ids of the other fingers that are down and landed in the
    /// area (the ones that make a pinch).
    fingers: [Option<u64>; FINGERS],
    /// Intents decided by events, handed out by `end_frame`.
    pending: Intents,
}

/// The distance on the farther axis (`isDraggingBeyondThreshold` compares
/// each axis on its own).
fn chebyshev(a: Point, b: Point) -> f32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

impl Gesture {
    pub fn new(m: &Metrics) -> Gesture {
        Gesture {
            drag_delay: m.drag_delay,
            drag_threshold: m.drag_threshold,
            time: 0.0,
            press: None,
            touch_last: false,
            mode: None,
            before_pinch: None,
            unresolved: None,
            ended: None,
            moved_to: None,
            panned: (0.0, 0.0),
            primary_finger: None,
            fingers: [None; FINGERS],
            pending: Intents::default(),
        }
    }

    /// A drag is in progress that pans the view.
    pub fn panning(&self) -> bool {
        self.mode == Some(Mode::Pan)
    }

    /// Two fingers of the area pinch the view.
    pub fn pinching(&self) -> bool {
        self.mode == Some(Mode::Pinch)
    }

    /// A press is held (whether or not it is a drag yet).
    pub fn pressed(&self) -> bool {
        self.press.is_some()
    }

    /// The latest press (held or just released) was made by a finger, not
    /// the mouse (a finger is first reported as a touch, then as the
    /// mouse).
    pub fn touch_press(&self) -> bool {
        self.touch_last
    }

    /// The held press went beyond the click threshold.
    pub fn beyond_threshold(&self) -> bool {
        self.press.is_some_and(|p| p.beyond)
    }

    /// Starts a frame at `time` seconds.
    pub fn begin_frame(&mut self, time: f64) {
        self.time = time;
    }

    pub fn event(&mut self, event: Event) {
        match event {
            Event::Down { pos, id, button } => match self.press {
                Some(p) if p.id != id => self.start_pinch(),
                // Another button of the same mouse: the first press goes on
                // (the first release of either ends it).
                Some(p) if p.button != button => {}
                // The release of the held press never came.
                Some(_) => {
                    self.abort();
                    self.begin(pos, id, button);
                }
                None => self.begin(pos, id, button),
            },
            Event::Move { pos, id } => {
                let Some(p) = &mut self.press else {
                    return;
                };
                // Only the pointer of the press moves it, and only once the
                // delay after the press is over (`shouldCancelDrag`: dropped
                // while `now < draggingDelay + lastMousePressedTime`).
                if p.id != id || self.time < p.start + self.drag_delay {
                    return;
                }
                if p.no_drag || self.mode == Some(Mode::Pinch) {
                    return;
                }
                // `wrapMouseDraggedND`: the threshold is checked against the
                // previous accepted position, then the new one is taken.
                p.beyond |= chebyshev(p.last, p.origin) > self.drag_threshold;
                p.last = pos;
                p.moves += 1;
                p.max_distance = p.max_distance.max(chebyshev(pos, p.origin));
                if let Some(a) = p.anchor {
                    self.panned.0 += pos.x - a.x;
                    self.panned.1 += pos.y - a.y;
                }
                p.anchor = Some(pos);
                self.moved_to = Some(pos);
            }
            Event::Up { pos, id, button } => {
                let Some(p) = self.press else {
                    return;
                };
                if p.id != id {
                    // A second finger lifted: the pinch goes on until the
                    // first one does.
                    return;
                }
                let tap = !p.beyond && button == Button::Primary && self.mode != Some(Mode::Pinch);
                match self.mode {
                    Some(m) if m.is_drag() => {
                        self.ended = Some(m);
                        self.pending.end_drag = true;
                    }
                    // A pinch suspended a drag: the release ends it where
                    // it is (`twoTouchEnd` set `draggingBeyondThreshold`,
                    // so the release is no click).
                    Some(Mode::Pinch) => {
                        if let Some(m) = self.before_pinch.filter(|m| m.is_drag()) {
                            self.ended = Some(m);
                            self.pending.end_drag = true;
                        }
                    }
                    // Moves were accepted but the drag is decided only at
                    // the end of the frame: it is decided, applied and ended
                    // there.
                    None if p.moves > 0 && !p.no_drag && self.moved_to.is_some() => {
                        self.unresolved = Some((p.origin, p.button));
                        self.pending.end_drag = true;
                    }
                    _ => {}
                }
                self.pending.released = Some(p.release());
                if tap {
                    // `wrapMouseReleasedND` takes the release position
                    // (`setMouseLocation`, `transformCoords`) before it runs
                    // the click path.
                    self.pending.tap = Some(pos);
                }
                self.mode = None;
                self.before_pinch = None;
                self.press = None;
            }
            Event::Cancel { id } => {
                if self.press.is_some_and(|p| p.id == id) {
                    self.abort();
                }
            }
            Event::Gone => {
                if let Some(p) = &mut self.press {
                    p.anchor = None;
                }
            }
            Event::FocusLost => {
                self.abort();
                self.forget_fingers();
            }
        }
    }

    /// A touch of the finger with the platform's id `touch`; `in_area` tells,
    /// for a start, whether the finger landed in the area (the caller's rule
    /// for a press). The platform layer also reports the first finger down
    /// as the mouse (egui-winit sends its pointer events right after the
    /// touch event), so that finger's touches count only for a cancel, which
    /// the mouse events do not carry, and to tell a finger's press from the
    /// mouse's. Any other finger that lands in the area is followed until it
    /// lifts: it turns a held press, or the next one, into a pinch. A finger
    /// that lands elsewhere, or one more than the automaton follows, is
    /// ignored.
    pub fn touch(&mut self, touch: u64, phase: TouchPhase, in_area: bool) {
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
        match phase {
            TouchPhase::Start if in_area => {
                if let Some(slot) = self.fingers.iter_mut().find(|f| f.is_none()) {
                    *slot = Some(touch);
                    if self.press.is_some() {
                        self.start_pinch();
                    }
                }
            }
            // Its moves are the pinch's, which egui measures.
            TouchPhase::Start | TouchPhase::Move => {}
            TouchPhase::End | TouchPhase::Cancel => {
                for f in &mut self.fingers {
                    if *f == Some(touch) {
                        *f = None;
                    }
                }
                // The last other finger lifted: the pinch is over and the
                // pressing finger goes on with what it held.
                if self.fingers.iter().all(Option::is_none) {
                    self.end_pinch();
                }
            }
        }
    }

    /// The pinch is over (`twoTouchEnd`): the drag it suspended resumes.
    fn end_pinch(&mut self) {
        if self.mode == Some(Mode::Pinch) {
            self.mode = self.before_pinch.take();
        }
        self.before_pinch = None;
    }

    /// Forgets which fingers are down: the platform may recreate its touch
    /// state (an app that comes back to the front), so ids from before it
    /// must not linger.
    fn forget_fingers(&mut self) {
        self.primary_finger = None;
        self.fingers = [None; FINGERS];
    }

    fn begin(&mut self, pos: Point, id: PointerId, button: Button) {
        // `wrapMousePressed` sets `lastMousePressedTime`, and
        // `resetSelectionFlags` the start point of the threshold.
        self.touch_last = id == MOUSE && self.primary_finger.is_some();
        self.press = Some(Press {
            id,
            button,
            origin: pos,
            start: self.time,
            last: pos,
            beyond: false,
            moves: 0,
            max_distance: 0.0,
            anchor: Some(pos),
            no_drag: false,
        });
        self.mode = None;
        self.before_pinch = None;
        // Another finger is already down in the area: two fingers pinch.
        if self.fingers.iter().any(Option::is_some) {
            self.mode = Some(Mode::Pinch);
        }
    }

    /// Ends the press without a tap; a dragged point goes back.
    fn abort(&mut self) {
        if self.mode == Some(Mode::Point)
            || (self.mode == Some(Mode::Pinch) && self.before_pinch == Some(Mode::Point))
        {
            self.pending.cancel_drag = true;
        }
        if matches!(
            self.mode,
            Some(Mode::Pan | Mode::Axis | Mode::Rect | Mode::Pinch)
        ) {
            self.pending.cancel_view = true;
        }
        self.mode = None;
        self.before_pinch = None;
        self.press = None;
        self.moved_to = None;
        self.panned = (0.0, 0.0);
    }

    /// Ends whatever is held, as a cancel, and hands out what is still to do
    /// (the area went away, so no `end_frame` follows).
    pub fn cancel(&mut self) -> Intents {
        self.abort();
        self.forget_fingers();
        self.unresolved = None;
        self.ended = None;
        let mut out = std::mem::take(&mut self.pending);
        // Nothing is moved or panned any more.
        out.drag_to = None;
        out.pan = None;
        out
    }

    /// Escape: the drag in progress ends where it is, and the press can no
    /// longer start one (a release may still be a tap).
    pub fn stop(&mut self) {
        if matches!(self.mode, Some(m) if m.is_drag()) {
            self.pending.end_drag = true;
            self.mode = None;
        } else if let Some(m) = self.before_pinch.filter(|m| m.is_drag()) {
            // Escape during a pinch ends the drag it suspended.
            self.ended = Some(m);
            self.pending.end_drag = true;
            self.mode = None;
        }
        if let Some(p) = &mut self.press {
            p.no_drag = true;
        }
        self.moved_to = None;
        self.panned = (0.0, 0.0);
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
        })
    }

    /// Two or more fingers are down (egui counts every finger on the
    /// screen): zoom about their centre and pan with them, but only when the
    /// press is a pinch, that is when a second finger landed in the area. A
    /// finger on the toolbar or the keyboard leaves a drag alone.
    pub fn pinch(&self, factor: f64, center: Point, pan: (f32, f32)) -> Option<Zoom> {
        (self.mode == Some(Mode::Pinch)).then_some(Zoom {
            factor,
            center,
            pan,
        })
    }

    /// A second finger landed in the area (`twoTouchStart`): whatever the
    /// press held is suspended — not taken back, the drag goes on when the
    /// other finger lifts — and the two fingers pinch. The pinch marks the
    /// press, so its release is never a click (`twoTouchEnd` sets
    /// `draggingBeyondThreshold`).
    fn start_pinch(&mut self) {
        if let Some(p) = &mut self.press {
            p.beyond = true;
            // The pinch moves the view itself: a pan that resumes must not
            // jump by the motion of the pinch, so it re-anchors first.
            p.anchor = None;
        }
        self.before_pinch = self.mode.take().filter(|m| m.is_drag());
        self.mode = Some(Mode::Pinch);
        self.moved_to = None;
        self.panned = (0.0, 0.0);
    }

    /// Ends the frame. `grab` is asked, with the press position, what a
    /// drag of the primary button takes hold of when one starts (with the
    /// first move accepted after the delay).
    pub fn end_frame(&mut self, grab: impl FnOnce(Point) -> Grab) -> Intents {
        let mut out = std::mem::take(&mut self.pending);
        let moved_to = self.moved_to.take();
        let panned = std::mem::take(&mut self.panned);
        // The press to decide: one that ended this frame before it was, or
        // the held one with its first accepted moves.
        let undecided = self
            .unresolved
            .take()
            .map(|(o, b)| (o, b, false))
            .or_else(|| {
                // Only a press with accepted moves of its own: a press that
                // follows a release in the same frame never has any (the
                // delay), and the motion is the released drag's.
                self.press
                    .filter(|p| {
                        self.mode.is_none() && !p.no_drag && p.moves > 0 && moved_to.is_some()
                    })
                    .map(|p| (p.origin, p.button, true))
            });
        let mut mode = self.ended.take().or(self.mode);
        if let Some((origin, button, held)) = undecided {
            let g = match button {
                Button::Primary => grab(origin),
                Button::Other => Grab::Pan,
            };
            mode = Some(g.into());
            if held {
                self.mode = mode;
            }
            out.started = Some(g);
        }
        if moved_to.is_some() {
            match mode {
                Some(Mode::Point | Mode::Rect | Mode::Axis) => out.drag_to = moved_to,
                Some(Mode::Pan) => out.pan = Some(panned),
                _ => {}
            }
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
        g.end_frame(|_| grab)
    }

    fn mouse() -> Gesture {
        Gesture::new(&Metrics::desktop())
    }

    fn released(beyond_threshold: bool, moves: u32, max_distance: f32) -> Option<Release> {
        Some(Release {
            beyond_threshold,
            moves,
            max_distance,
        })
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
        let out = frame(&mut g, 0.3, &[up(5.0, 5.0)], Grab::Point);
        assert_eq!(out.tap, Some(p(5.0, 5.0)));
        assert_eq!(out.released, released(false, 0, 0.0));
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

    /// `EuclidianController.shouldCancelDrag`: moves within
    /// `DRAGGING_DELAY` (100 ms) of the press are dropped, for every pointer.
    #[test]
    fn moves_within_the_delay_after_the_press_are_dropped() {
        for m in [Metrics::desktop(), Metrics::touch()] {
            let mut g = Gesture::new(&m);
            frame(&mut g, 1.0, &[down(0.0, 0.0)], Grab::Point);
            assert_eq!(
                frame(&mut g, 1.05, &[mv(20.0, 0.0)], Grab::Point),
                Intents::default()
            );
            assert_eq!(
                frame(&mut g, 1.09, &[mv(30.0, 0.0)], Grab::Point),
                Intents::default()
            );
            // Without a move after the delay nothing moved and the release
            // is a click (where it is released, as `wrapMouseReleasedND`).
            let out = frame(&mut g, 1.3, &[up(30.0, 0.0)], Grab::Point);
            assert_eq!((out.tap, out.end_drag), (Some(p(30.0, 0.0)), false));
            assert_eq!(out.released, released(false, 0, 0.0));
        }
    }

    /// No motion threshold (`handleMovedElementFreePoint` sets no grab
    /// offset): the first move after the delay takes the object straight to
    /// the pointer, however small or large the motion was.
    #[test]
    fn the_first_move_after_the_delay_starts_the_drag_at_the_pointer() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        // The motion made during the delay is not lost.
        frame(&mut g, 0.005, &[mv(10.0, 0.0)], Grab::Point);
        let out = frame(&mut g, 0.1, &[mv(11.0, 0.0)], Grab::Point);
        assert_eq!(out.started, Some(Grab::Point));
        assert_eq!(out.drag_to, Some(p(11.0, 0.0)));
        // A move of one pixel moves the object one pixel.
        let out = frame(&mut g, 0.2, &[mv(12.0, 0.0)], Grab::Point);
        assert_eq!((out.started, out.drag_to), (None, Some(p(12.0, 0.0))));
        // Idle frames move nothing.
        assert_eq!(frame(&mut g, 0.3, &[], Grab::Point), Intents::default());
        // The release ends the drag and moves nothing itself.
        let out = frame(&mut g, 0.4, &[up(30.0, 1.0)], Grab::Point);
        assert!(out.end_drag);
        assert_eq!(out.drag_to, None);
    }

    /// `isDraggingBeyondThreshold`: more than `DRAG_THRESHOLD` (10 px) on
    /// either axis, checked one move late (`wrapMouseDraggedND` checks before
    /// it takes the new position). A release not beyond it is a click, even
    /// when the motion moved the object (`wrapMouseReleasedND`).
    #[test]
    fn the_click_threshold_is_checked_one_move_late() {
        // One move of any length: still a click, but the object moved.
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        let out = frame(&mut g, 0.2, &[mv(40.0, 0.0)], Grab::Point);
        assert_eq!(out.drag_to, Some(p(40.0, 0.0)));
        assert!(!g.beyond_threshold());
        let out = frame(&mut g, 0.3, &[up(40.0, 0.0)], Grab::Point);
        assert!(out.end_drag);
        assert_eq!(out.tap, Some(p(40.0, 0.0)));
        assert_eq!(out.released, released(false, 1, 40.0));
        // Two moves: the second one sees the first beyond the threshold.
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.2, &[mv(20.0, 0.0)], Grab::Point);
        frame(&mut g, 0.3, &[mv(40.0, 0.0)], Grab::Point);
        assert!(g.beyond_threshold());
        let out = frame(&mut g, 0.4, &[up(40.0, 0.0)], Grab::Point);
        assert_eq!((out.end_drag, out.tap), (true, None));
        assert_eq!(out.released, released(true, 2, 40.0));
        // Exactly the threshold away (on the farther axis) is not beyond it.
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.2, &[mv(10.0, -4.0), mv(10.0, 10.0)], Grab::Point);
        let out = frame(&mut g, 0.3, &[mv(10.0, 10.0), up(10.0, 10.0)], Grab::Point);
        assert_eq!(out.tap, Some(p(10.0, 10.0)));
        assert_eq!(out.released, released(false, 3, 10.0));
        // One pixel more is beyond it. With a threshold of 6 the case above
        // fails, with one of 20 this one does.
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.2, &[mv(11.0, 3.0)], Grab::Point);
        let out = frame(&mut g, 0.3, &[mv(11.0, 3.0), up(11.0, 3.0)], Grab::Point);
        assert_eq!(out.tap, None);
        assert_eq!(out.released, released(true, 2, 11.0));
    }

    #[test]
    fn a_pan_follows_the_pointer_exactly() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Pan);
        let mut panned = (0.0, 0.0);
        for (i, x) in [3.0, 6.0, 9.0, -50.0].into_iter().enumerate() {
            let out = frame(&mut g, 0.15 + 0.025 * i as f64, &[mv(x, x)], Grab::Pan);
            let d = out.pan.unwrap();
            panned = (panned.0 + d.0, panned.1 + d.1);
        }
        assert_eq!(panned, (-50.0, -50.0));
        assert!(g.panning());
    }

    /// A running drag whose last moves share a frame with the release: the
    /// moves are applied before the drag ends (`wrapMouseDraggedND` runs
    /// for each move before `wrapMouseReleasedND`). Fails if `Event::Up`
    /// drops the motion of the frame (the mode cleared before `end_frame`).
    #[test]
    fn the_last_moves_of_a_running_drag_count_when_the_release_comes_in_their_frame() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.2, &[mv(20.0, 0.0)], Grab::Point);
        let out = frame(
            &mut g,
            0.3,
            &[mv(30.0, 5.0), mv(40.0, 6.0), up(40.0, 6.0)],
            Grab::Point,
        );
        assert_eq!(out.started, None);
        assert_eq!(out.drag_to, Some(p(40.0, 6.0)));
        assert!(out.end_drag && out.tap.is_none());
        assert_eq!(out.released, released(true, 3, 40.0));
        assert_eq!(frame(&mut g, 0.4, &[], Grab::Point), Intents::default());
        // The same for a pan: the view pans by the whole movement.
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Pan);
        let first = frame(&mut g, 0.2, &[mv(-20.0, 0.0)], Grab::Pan)
            .pan
            .unwrap();
        let out = frame(&mut g, 0.3, &[mv(-50.0, 4.0), up(-50.0, 4.0)], Grab::Pan);
        let last = out.pan.unwrap();
        assert_eq!((first.0 + last.0, first.1 + last.1), (-50.0, 4.0));
        assert!(out.end_drag);
        assert!(!g.panning());
        // A press right after the release, in the same frame, takes none of
        // the released drag's motion.
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.2, &[mv(20.0, 0.0)], Grab::Point);
        let out = frame(
            &mut g,
            0.3,
            &[mv(30.0, 0.0), up(30.0, 0.0), down(60.0, 0.0)],
            Grab::Pan,
        );
        assert_eq!(
            (out.started, out.drag_to, out.pan),
            (None, Some(p(30.0, 0.0)), None)
        );
        assert!(out.end_drag && g.pressed() && !g.panning());
    }

    #[test]
    fn a_still_press_is_a_tap_however_long_it_lasts() {
        let mut g = mouse();
        frame(&mut g, 1.0, &[down(0.0, 0.0)], Grab::Pan);
        assert_eq!(frame(&mut g, 2.5, &[], Grab::Pan), Intents::default());
        let out = frame(&mut g, 3.0, &[up(0.0, 0.0)], Grab::Pan);
        assert_eq!(out.tap, Some(p(0.0, 0.0)));
        // Held long and then moved: a normal drag.
        frame(&mut g, 4.0, &[down(0.0, 0.0)], Grab::Pan);
        let out = frame(&mut g, 5.5, &[mv(3.0, 0.0)], Grab::Pan);
        assert_eq!((out.started, out.pan), (Some(Grab::Pan), Some((3.0, 0.0))));
    }

    #[test]
    fn moves_and_the_release_in_one_frame_start_apply_and_end_the_drag() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        let mut asked = None;
        g.begin_frame(0.5);
        g.event(mv(5.0, 0.0));
        g.event(mv(30.0, 0.0));
        g.event(up(30.0, 0.0));
        let out = g.end_frame(|o| {
            asked = Some(o);
            Grab::Point
        });
        assert_eq!(asked, Some(p(0.0, 0.0)));
        assert_eq!(out.started, Some(Grab::Point));
        assert_eq!(out.drag_to, Some(p(30.0, 0.0)));
        assert!(out.end_drag && !g.pressed());
        assert_eq!(out.released, released(false, 2, 30.0));
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
        let out = frame(&mut g, 0.1, &[other(0.0, false)], Grab::Point);
        assert_eq!(out.tap, None);
        frame(&mut g, 0.2, &[other(0.0, true)], Grab::Point);
        // `grab` is not asked: a drag of another button always pans.
        let out = frame(&mut g, 0.4, &[mv(10.0, 0.0)], Grab::Point);
        assert_eq!(out.started, Some(Grab::Pan));
    }

    /// The touch profile has no wider click wobble any more: the same delay
    /// and threshold as the mouse (`DRAGGING_DELAY`, `DRAG_THRESHOLD`).
    #[test]
    fn a_touch_tap_may_wobble_within_the_threshold() {
        let mut g = Gesture::new(&Metrics::touch());
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        assert_eq!(
            frame(&mut g, 0.05, &[mv(10.0, 0.0)], Grab::Point),
            Intents::default()
        );
        let out = frame(&mut g, 0.1, &[up(10.0, 0.0)], Grab::Point);
        assert_eq!(out.tap, Some(p(10.0, 0.0)));
        // A wobble after the delay is applied, and a tap while it stays
        // within the threshold (fails with a threshold under 9).
        frame(&mut g, 0.5, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.65, &[mv(9.0, -3.0)], Grab::Point);
        let out = frame(&mut g, 0.7, &[mv(9.0, -3.0), up(9.0, -3.0)], Grab::Point);
        assert_eq!(out.drag_to, Some(p(9.0, -3.0)));
        assert_eq!(out.tap, Some(p(9.0, -3.0)));
        // Moves beyond it after the delay make it a drag that is no tap.
        frame(&mut g, 1.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 1.15, &[mv(13.0, 0.0)], Grab::Point);
        frame(&mut g, 1.2, &[mv(14.0, 0.0)], Grab::Point);
        assert_eq!(frame(&mut g, 1.3, &[up(14.0, 0.0)], Grab::Point).tap, None);
    }

    /// The first finger goes down at the origin: its touch, then the press
    /// the platform reports for it.
    fn first_finger(g: &mut Gesture, time: f64, grab: Grab) {
        g.begin_frame(time);
        g.touch(1, TouchPhase::Start, true);
        g.event(down(0.0, 0.0));
        g.end_frame(|_| grab);
    }

    #[test]
    fn a_press_tells_a_finger_from_the_mouse() {
        let mut g = Gesture::new(&Metrics::touch());
        first_finger(&mut g, 0.0, Grab::Point);
        assert!(g.touch_press());
        g.begin_frame(0.1);
        g.touch(1, TouchPhase::End, true);
        g.event(up(0.0, 0.0));
        g.end_frame(|_| Grab::Point);
        assert!(g.touch_press(), "still after the release");
        frame(&mut g, 0.2, &[down(0.0, 0.0)], Grab::Point);
        assert!(!g.touch_press());
    }

    /// The second finger of a pinch (`twoTouchStart`, `twoTouchEnd`): a
    /// point drag in progress is suspended, not taken back — the point
    /// stays where the drag left it — and when the second finger lifts, the
    /// first one drags it again. The pinch marks the press, so its release
    /// is never a tap (`twoTouchEnd` sets `draggingBeyondThreshold`).
    #[test]
    fn a_second_finger_suspends_the_point_drag_and_the_pinch_resumes_it() {
        let mut g = Gesture::new(&Metrics::touch());
        first_finger(&mut g, 0.0, Grab::Point);
        let out = frame(&mut g, 0.2, &[mv(20.0, 0.0)], Grab::Point);
        assert_eq!(out.started, Some(Grab::Point));
        g.begin_frame(0.3);
        g.touch(2, TouchPhase::Start, true);
        let zoom = g.pinch(1.1, p(30.0, 30.0), (2.0, 0.0)).unwrap();
        assert_eq!((zoom.factor, zoom.pan), (1.1, (2.0, 0.0)));
        let out = g.end_frame(|_| Grab::Point);
        assert!(!out.cancel_drag, "the point stays where the drag left it");
        assert_eq!(out.drag_to, None);
        // While both fingers are down the moves are the pinch's.
        let out = frame(&mut g, 0.4, &[mv(40.0, 0.0)], Grab::Point);
        assert_eq!(out, Intents::default());
        // The second finger lifts: the first one drags the point again.
        g.begin_frame(0.5);
        g.touch(2, TouchPhase::End, true);
        let out = g.end_frame(|_| Grab::Point);
        assert_eq!(out, Intents::default());
        let out = frame(&mut g, 0.6, &[mv(50.0, 0.0)], Grab::Point);
        assert_eq!(out.drag_to, Some(p(50.0, 0.0)));
        // The pinch marked the press: the release ends the drag, no tap.
        let out = frame(&mut g, 0.7, &[up(50.0, 0.0)], Grab::Point);
        assert!(out.end_drag && out.tap.is_none());
    }

    #[test]
    fn a_second_finger_turns_a_press_into_a_pinch() {
        // The first finger comes as a touch and as the mouse; only the
        // mouse events of it count.
        let first = |g: &mut Gesture, phase| g.touch(3, phase, true);
        let mut g = Gesture::new(&Metrics::touch());
        g.begin_frame(0.0);
        first(&mut g, TouchPhase::Start);
        g.event(down(0.0, 0.0));
        g.end_frame(|_| Grab::Point);
        frame(&mut g, 0.2, &[mv(20.0, 0.0)], Grab::Point);
        g.begin_frame(0.3);
        g.touch(8, TouchPhase::Start, true);
        let out = g.end_frame(|_| Grab::Point);
        assert!(!out.cancel_drag, "the drag is only suspended");
        assert_eq!(out.drag_to, None);
        // A still press would have been a tap; now it is not.
        let mut g = Gesture::new(&Metrics::touch());
        g.begin_frame(0.0);
        first(&mut g, TouchPhase::Start);
        g.event(down(0.0, 0.0));
        g.touch(8, TouchPhase::Start, true);
        g.end_frame(|_| Grab::Point);
        g.begin_frame(0.1);
        first(&mut g, TouchPhase::End);
        g.event(up(0.0, 0.0));
        let out = g.end_frame(|_| Grab::Point);
        assert_eq!((out.tap, out.end_drag, out.started), (None, false, None));
    }

    #[test]
    fn a_finger_without_a_press_is_ignored() {
        let mut g = Gesture::new(&Metrics::touch());
        g.begin_frame(0.0);
        g.touch(3, TouchPhase::Start, true);
        g.touch(3, TouchPhase::End, true);
        // A second finger alone (the first one lifted) does nothing.
        g.touch(5, TouchPhase::Start, true);
        assert!(!g.pressed());
        g.touch(5, TouchPhase::Move, true);
        g.touch(5, TouchPhase::End, true);
        assert_eq!(g.end_frame(|_| Grab::Pan), Intents::default());
    }

    /// After a pinch the pressing finger goes on with what it held
    /// (`twoTouchEnd`): the press's drag resumes when the other finger
    /// lifts. A finger that never pressed moves nothing, however the
    /// platform reports it as the mouse.
    #[test]
    fn after_a_pinch_the_pressing_finger_goes_on_dragging() {
        for first_lifts in [false, true] {
            let mut g = Gesture::new(&Metrics::touch());
            first_finger(&mut g, 0.0, Grab::Pan);
            // The press drags first, so the pinch suspends a live drag:
            // lifting the pressing finger in it ends that drag, while the
            // other finger lifting resumes it.
            let out = frame(&mut g, 0.12, &[mv(10.0, 0.0)], Grab::Pan);
            assert_eq!(out.started, Some(Grab::Pan));
            g.begin_frame(0.2);
            g.touch(2, TouchPhase::Start, true);
            assert!(g.pinch(1.2, p(40.0, 0.0), (0.0, 0.0)).is_some());
            g.end_frame(|_| Grab::Pan);
            g.begin_frame(0.3);
            if first_lifts {
                g.touch(1, TouchPhase::End, true);
                g.event(up(0.0, 0.0));
                g.event(Event::Gone);
            } else {
                g.touch(2, TouchPhase::End, true);
            }
            let out = g.end_frame(|_| Grab::Pan);
            assert_eq!(
                (out.started, out.pan, out.tap, out.end_drag),
                (None, None, None, first_lifts),
                "first lifts: {first_lifts}"
            );
            // The finger that stays moves far from where the press began.
            // Once the first finger is up, egui-winit reports the other one
            // as the mouse, though it never pressed: moves, then a release.
            let mut panned = (0.0, 0.0);
            for (i, x) in [30.0, 60.0, 90.0].into_iter().enumerate() {
                g.begin_frame(0.4 + 0.1 * i as f64);
                if first_lifts {
                    g.touch(2, TouchPhase::Move, true);
                    g.event(mv(80.0 + x, 0.0));
                } else {
                    g.touch(1, TouchPhase::Move, true);
                    g.event(mv(x, 0.0));
                }
                let out = g.end_frame(|_| Grab::Pan);
                if first_lifts {
                    // Its press ended with the first finger's release.
                    assert_eq!(out, Intents::default(), "first lifts: {first_lifts}");
                } else {
                    let pan = out.pan.unwrap_or((0.0, 0.0));
                    panned = (panned.0 + pan.0, panned.1 + pan.1);
                }
            }
            if !first_lifts {
                // The moves 30 -> 60 -> 90 pan 60 of it: the first move
                // after the pinch re-anchors, as its motion went into the
                // pinch's own translation of the view.
                assert_eq!(panned, (60.0, 0.0), "the pan resumed");
            }
            g.begin_frame(0.8);
            if first_lifts {
                g.touch(2, TouchPhase::End, true);
                g.event(up(170.0, 0.0));
                g.event(Event::Gone);
            } else {
                g.touch(1, TouchPhase::End, true);
                g.event(up(90.0, 0.0));
                g.event(Event::Gone);
            }
            let out = g.end_frame(|_| Grab::Pan);
            assert_eq!(
                (out.started, out.pan, out.tap),
                (None, None, None),
                "first lifts: {first_lifts}"
            );
            assert!(!g.pressed());
        }
    }

    #[test]
    fn a_finger_off_the_area_neither_pinches_nor_ends_the_drag() {
        let mut g = Gesture::new(&Metrics::touch());
        first_finger(&mut g, 0.0, Grab::Point);
        let out = frame(&mut g, 0.2, &[mv(30.0, 0.0)], Grab::Point);
        assert_eq!(out.started, Some(Grab::Point));
        // A finger lands on the toolbar: egui sees two fingers, the drag
        // goes on.
        g.begin_frame(0.3);
        g.touch(7, TouchPhase::Start, false);
        assert_eq!(g.pinch(1.3, p(15.0, -40.0), (0.0, 4.0)), None);
        g.event(mv(40.0, 0.0));
        let out = g.end_frame(|_| Grab::Point);
        assert!(!out.cancel_drag);
        assert_eq!(out.drag_to, Some(p(40.0, 0.0)));
        // It moves and is cancelled by the platform: still nothing.
        g.begin_frame(0.4);
        g.touch(7, TouchPhase::Move, false);
        g.touch(7, TouchPhase::Cancel, false);
        g.event(mv(50.0, 0.0));
        let out = g.end_frame(|_| Grab::Point);
        assert_eq!((out.cancel_drag, out.drag_to), (false, Some(p(50.0, 0.0))));
        assert!(frame(&mut g, 0.5, &[up(50.0, 0.0)], Grab::Point).end_drag);
    }

    #[test]
    fn a_press_while_another_finger_rests_in_the_area_is_a_pinch() {
        // Two fingers pinch; the first one lifts and goes down again while
        // the second one stays.
        let mut g = Gesture::new(&Metrics::touch());
        first_finger(&mut g, 0.0, Grab::Pan);
        g.begin_frame(0.1);
        g.touch(2, TouchPhase::Start, true);
        g.end_frame(|_| Grab::Pan);
        g.begin_frame(0.2);
        g.touch(1, TouchPhase::End, true);
        g.event(up(0.0, 0.0));
        g.event(Event::Gone);
        g.end_frame(|_| Grab::Pan);
        g.begin_frame(0.3);
        g.touch(3, TouchPhase::Start, true);
        g.event(down(10.0, 0.0));
        assert!(g.pinch(0.9, p(40.0, 0.0), (0.0, 0.0)).is_some());
        g.end_frame(|_| Grab::Pan);
        for (i, x) in [40.0, 80.0].into_iter().enumerate() {
            let out = frame(&mut g, 0.5 + 0.1 * i as f64, &[mv(x, 0.0)], Grab::Pan);
            assert_eq!(out, Intents::default());
        }
        g.touch(3, TouchPhase::End, true);
        let out = frame(&mut g, 0.7, &[up(80.0, 0.0)], Grab::Pan);
        assert_eq!((out.tap, out.end_drag), (None, false));
        // Once the resting finger lifts, a press is a press again.
        g.touch(2, TouchPhase::End, true);
        first_finger(&mut g, 1.0, Grab::Pan);
        assert_eq!(g.pinch(1.1, p(0.0, 0.0), (0.0, 0.0)), None);
    }

    #[test]
    fn losing_the_focus_or_cancelling_forgets_the_fingers() {
        for focus in [true, false] {
            let mut g = Gesture::new(&Metrics::touch());
            first_finger(&mut g, 0.0, Grab::Pan);
            g.touch(2, TouchPhase::Start, true);
            if focus {
                g.event(Event::FocusLost);
            } else {
                g.cancel();
            }
            assert_eq!(g.primary_finger, None, "focus: {focus}");
            // After the platform recreated its touches, the next finger is
            // the first one and a press of it is no pinch.
            g.begin_frame(1.0);
            g.touch(9, TouchPhase::Start, true);
            assert_eq!(g.primary_finger, Some(9));
            g.event(down(0.0, 0.0));
            assert_eq!(g.pinch(1.1, p(0.0, 0.0), (0.0, 0.0)), None);
            g.end_frame(|_| Grab::Pan);
            let out = frame(&mut g, 1.1, &[up(0.0, 0.0)], Grab::Pan);
            assert_eq!(out.tap, Some(p(0.0, 0.0)), "focus: {focus}");
        }
    }

    #[test]
    fn a_cancel_counts_only_for_the_finger_of_the_press() {
        let mut g = Gesture::new(&Metrics::touch());
        first_finger(&mut g, 0.0, Grab::Point);
        let out = frame(&mut g, 0.2, &[mv(30.0, 0.0)], Grab::Point);
        assert_eq!(out.started, Some(Grab::Point));
        g.begin_frame(0.3);
        g.touch(2, TouchPhase::Start, false);
        g.touch(2, TouchPhase::Cancel, false);
        assert!(g.pressed());
        let out = g.end_frame(|_| Grab::Point);
        assert!(!out.cancel_drag);
        g.begin_frame(0.4);
        g.touch(1, TouchPhase::Cancel, true);
        g.event(Event::Gone);
        assert!(!g.pressed());
        let out = g.end_frame(|_| Grab::Point);
        assert!(out.cancel_drag && !out.end_drag);
        assert_eq!(out.released, None);
    }

    #[test]
    fn a_press_of_the_held_button_ends_the_old_press_first() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.2, &[mv(20.0, 0.0)], Grab::Point);
        // The release was lost: the point goes back, the new press starts
        // fresh and is a tap of its own.
        let out = frame(&mut g, 0.3, &[down(50.0, 50.0)], Grab::Point);
        assert!(out.cancel_drag && !out.end_drag);
        assert_eq!((out.started, out.drag_to), (None, None));
        let out = frame(&mut g, 0.4, &[up(50.0, 50.0)], Grab::Point);
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
        frame(&mut g, 0.2, &[mv(20.0, 0.0)], Grab::Point);
        let out = frame(&mut g, 0.3, &[up(20.0, 0.0), down(60.0, 0.0)], Grab::Point);
        assert!(out.end_drag && !out.cancel_drag);
        assert_eq!((out.started, out.drag_to), (None, None));
        assert!(g.pressed());
        // The new press drags from where it went down.
        let mut origin = None;
        g.begin_frame(0.5);
        g.event(mv(70.0, 0.0));
        let out = g.end_frame(|o| {
            origin = Some(o);
            Grab::Point
        });
        assert_eq!(origin, Some(p(60.0, 0.0)));
        assert_eq!(out.drag_to, Some(p(70.0, 0.0)));
        // A tap and a press in one frame: the tap is the earlier press's.
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
        let out = frame(&mut g, 0.2, &[mv(10.0, 0.0)], Grab::Pan);
        assert_eq!(out.pan, Some((10.0, 0.0)));
        // Out of the window and back in somewhere else in one frame.
        let out = frame(&mut g, 0.3, &[Event::Gone, mv(200.0, 90.0)], Grab::Pan);
        assert_eq!(out.pan, Some((0.0, 0.0)));
        let out = frame(&mut g, 0.4, &[mv(205.0, 90.0)], Grab::Pan);
        assert_eq!(out.pan, Some((5.0, 0.0)));
        // Moves outside the window still pan; the release ends the drag.
        let out = frame(&mut g, 0.5, &[Event::Gone], Grab::Pan);
        assert_eq!(out.pan, None);
        let out = frame(&mut g, 0.6, &[mv(-40.0, 0.0)], Grab::Pan);
        assert_eq!(out.pan, Some((0.0, 0.0)));
        let out = frame(&mut g, 0.7, &[mv(-50.0, 0.0)], Grab::Pan);
        assert_eq!(out.pan, Some((-10.0, 0.0)));
        assert!(frame(&mut g, 0.8, &[up(-50.0, 0.0)], Grab::Pan).end_drag);
    }

    #[test]
    fn losing_the_focus_cancels_the_press() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.2, &[mv(20.0, 0.0)], Grab::Point);
        let out = frame(&mut g, 0.3, &[Event::FocusLost], Grab::Point);
        assert!(out.cancel_drag && !out.end_drag);
        assert!(!g.pressed());
        assert_eq!(
            frame(&mut g, 0.4, &[mv(40.0, 0.0), up(40.0, 0.0)], Grab::Point),
            Intents::default()
        );
        // Without a frame end (the area went away) the cancel is handed out
        // at once.
        frame(&mut g, 1.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 1.2, &[mv(20.0, 0.0)], Grab::Point);
        let out = g.cancel();
        assert!(out.cancel_drag && !g.pressed());
    }

    #[test]
    fn cancel_puts_the_point_back_and_ends_the_press() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.2, &[mv(20.0, 0.0)], Grab::Point);
        let out = frame(&mut g, 0.3, &[Event::Cancel { id: MOUSE }], Grab::Point);
        assert!(out.cancel_drag && !out.end_drag);
        assert_eq!(
            frame(&mut g, 0.4, &[mv(30.0, 0.0)], Grab::Point),
            Intents::default()
        );
        assert_eq!(
            frame(&mut g, 0.5, &[up(30.0, 0.0)], Grab::Point),
            Intents::default()
        );
    }

    #[test]
    fn escape_ends_the_drag_and_the_press_starts_no_other() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Pan);
        frame(&mut g, 0.2, &[mv(20.0, 0.0)], Grab::Pan);
        frame(&mut g, 0.25, &[mv(25.0, 0.0)], Grab::Pan);
        g.begin_frame(0.3);
        g.stop();
        g.event(mv(30.0, 0.0));
        let out = g.end_frame(|_| Grab::Pan);
        assert!(out.end_drag);
        assert_eq!((out.started, out.pan), (None, None));
        assert_eq!(
            frame(&mut g, 0.4, &[mv(40.0, 0.0)], Grab::Pan),
            Intents::default()
        );
        // The drag went beyond the threshold: the release is no tap.
        let out = frame(&mut g, 0.5, &[up(40.0, 0.0)], Grab::Pan);
        assert_eq!((out.tap, out.end_drag), (None, false));
    }

    #[test]
    fn a_rebuild_leaves_the_drag_inert_until_it_ends() {
        let mut g = mouse();
        frame(&mut g, 0.0, &[down(0.0, 0.0)], Grab::Point);
        frame(&mut g, 0.2, &[mv(20.0, 0.0)], Grab::Point);
        g.forget_target();
        assert!(!g.panning());
        assert_eq!(
            frame(&mut g, 0.3, &[mv(30.0, 0.0)], Grab::Point),
            Intents::default()
        );
        assert!(frame(&mut g, 0.4, &[up(30.0, 0.0)], Grab::Point).end_drag);
    }

    #[test]
    fn the_wheel_zooms_only_when_it_changes_the_scale() {
        let g = mouse();
        assert_eq!(g.zoom(1.0, p(0.0, 0.0)), None);
        assert_eq!(g.zoom(1.25, p(3.0, 4.0)).map(|z| z.factor), Some(1.25));
    }
}
