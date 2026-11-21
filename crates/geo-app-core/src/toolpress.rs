// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianController.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/modes/ModeDeleteLocus.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! How a construction tool takes the pointer: it picks when the button goes
//! down, a drag-creating tool takes its second point at the release, and
//! the new points go on the objects under the pointer. No UI types: the
//! window hands in positions in the drawing area and the view transform it
//! painted.
//!
//! - The press picks at once (`switchModeForMousePressedND`; spec E1). A
//!   new point is resolved by `new_point_at` (spec E2).
//! - The line, segment, ray, vector, circle and regular polygon tools also
//!   make their object by press, drag and release
//!   (`isModeCreatingObjectsByDrag`; spec E3): with the first point picked,
//!   a press waits for its release, and a release that ends a drag away
//!   from the press (beyond the point selection distance) picks there.
//! - The point tool's press makes or finds a point that a drag then moves
//!   (`wrapMouseDraggedND`; spec E4).
//! - The delete tool's drag beyond the click threshold is an eraser: every
//!   object that a square of 20 px around the pointer meets is deleted, the
//!   whole stroke one undo step (`ModeDeleteLocus.handleMouseDraggedForDelete`,
//!   `storeUndoAfterDrag`; spec E11).

use crate::app::App;
use crate::hit::{self, Reach};
use crate::interact::{self, ViewTransform};
use crate::tools::{creates_by_drag, new_points_on_objects, NewPoint, Outcome, Tool};
use geo_kernel::kernel::{Id, Kind, Value};

/// The side of the eraser's square, in pixels
/// (`EuclidianConstants.DEFAULT_ERASER_SIZE`).
pub const ERASER_SIZE: f64 = 20.0;

/// What a construction tool's press did, kept until its release.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolPress {
    /// Where the press was, in the drawing area.
    pub at: (f64, f64),
    /// The press picks nothing: a drag-creating tool with its first point
    /// waits for the release.
    pub deferred: bool,
    /// The press picked or placed something (a drag after it does not pan).
    pub acted: bool,
    /// The point the press made or found with the point tool, which a drag
    /// moves.
    pub point: Option<Id>,
}

/// The kinds a new point can be put on (`Path` in the reference; polygons
/// are left out, `hits.removePolygons`, their sides are segments).
fn is_path(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Line | Kind::Segment | Kind::Ray | Kind::Circle | Kind::PolyLine
    )
}

/// Whether the kernel intersects `a` with `b`, and whether that makes two
/// points (one of which is chosen by its index).
fn intersection(a: Kind, b: Kind) -> Option<bool> {
    match (a, b) {
        (Kind::Line, Kind::Line) => Some(false),
        (Kind::Line, Kind::Circle) | (Kind::Circle, Kind::Line) | (Kind::Circle, Kind::Circle) => {
            Some(true)
        }
        _ => None,
    }
}

impl App {
    /// Whether the object `id` is drawn (and so can be hit); the show/hide
    /// tool shows hidden objects too.
    fn hittable(&self, id: Id) -> bool {
        self.tool.tool() == Tool::ShowHide || self.wb.is_visible(id)
    }

    /// What a new point made by a click at `pos` (in the area, `t` the view
    /// it was made in) is (`EuclidianController.getNewPoint`; spec E2): the
    /// crossing of the first two paths under the pointer when the kernel
    /// intersects them and the point is defined
    /// (`getSingleIntersectionPoint`, of two points the one nearer the
    /// pointer), else a point on the path under the pointer (`chooseGeo`
    /// among the path hits), else a free point; the position is captured to
    /// the grid (`transformCoords`). Only the tools that allow it put points
    /// on objects (`new_points_on_objects`). A point under the pointer is
    /// not this function's business: the tool picks it.
    pub fn new_point_at(&mut self, t: &ViewTransform, pos: (f64, f64), reach: &Reach) -> NewPoint {
        let (x, y) = self.snap(t.scale()).apply(t.screen_to_world(pos.0, pos.1));
        if !new_points_on_objects(self.tool.tool(), self.tool.picks()) {
            return NewPoint::Free(x, y);
        }
        let c = self.wb.construction();
        let paths = hit::hits(c, t, pos, reach, |id| {
            self.hittable(id) && is_path(c.kind(id))
        });
        if let [a, b, ..] = paths[..] {
            if let Some(two) = intersection(c.kind(a), c.kind(b)) {
                let (a, b) = (c.label(a).to_owned(), c.label(b).to_owned());
                let raw = t.screen_to_world(pos.0, pos.1);
                if let Some(index) = self.nearest_intersection(&a, &b, two, raw) {
                    return NewPoint::Intersection { a, b, index };
                }
            }
        }
        let c = self.wb.construction();
        let path = hit::pick(c, t, pos, reach, None, |id| {
            self.hittable(id) && is_path(c.kind(id))
        });
        match path {
            Some(p) => NewPoint::OnPath {
                path: c.label(p).to_owned(),
                x,
                y,
            },
            None => NewPoint::Free(x, y),
        }
    }

    /// The intersection of `a` and `b` to use: `Some(None)` for the one
    /// point of two lines, `Some(Some(n))` for the defined point of two that
    /// is nearer `(x, y)`, `None` when none is defined (`!showInEuclidianView`).
    fn nearest_intersection(
        &mut self,
        a: &str,
        b: &str,
        two: bool,
        (x, y): (f64, f64),
    ) -> Option<Option<u8>> {
        if !two {
            return self
                .wb
                .probe_point(&format!("Intersect({a}, {b})"))
                .map(|_| None);
        }
        let mut best: Option<(f64, u8)> = None;
        for n in [1u8, 2] {
            if let Some((px, py)) = self.wb.probe_point(&format!("Intersect({a}, {b}, {n})")) {
                let d = (px - x).hypot(py - y);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, n));
                }
            }
        }
        best.map(|(_, n)| Some(n))
    }

    /// The object a construction tool's click at `pos` lands on: the one the
    /// press takes hold of (`hit::pick`), whatever the tool can do with it.
    fn tool_hit(
        &self,
        t: &ViewTransform,
        pos: (f64, f64),
        reach: &Reach,
    ) -> Option<(String, Kind)> {
        let c = self.wb.construction();
        hit::pick(c, t, pos, reach, None, |id| self.hittable(id))
            .map(|id| (c.label(id).to_owned(), c.kind(id)))
    }

    /// One click of a construction tool at `pos`: the pick, with a new
    /// point resolved there; returns what it did.
    fn tool_click(&mut self, t: &ViewTransform, pos: (f64, f64), reach: &Reach) -> Outcome {
        let hit = self.tool_hit(t, pos, reach);
        let new_point = self.new_point_at(t, pos, reach);
        let outcome = {
            let c = self.wb.construction();
            self.tool.click_with(
                hit.as_ref().map(|(l, k)| (l.as_str(), *k)),
                &new_point,
                &|l| c.id(l).is_some(),
            )
        };
        let shown = outcome.clone();
        self.apply_outcome(outcome);
        shown
    }

    /// A press of a construction tool (one that builds from picks) at `pos`
    /// in the area, in the view `t` the user saw. Picks at once, except the
    /// second point of a drag-creating tool, which waits for the release.
    pub fn press_tool(&mut self, t: &ViewTransform, pos: (f64, f64), reach: &Reach) -> ToolPress {
        let tool = self.tool.tool();
        let mut press = ToolPress {
            at: pos,
            deferred: false,
            acted: false,
            point: None,
        };
        if creates_by_drag(tool) && self.tool.picks().len() == 1 && self.tool.asking().is_none() {
            press.deferred = true;
            press.acted = true;
            return press;
        }
        let before = self.wb.construction().len();
        let outcome = self.tool_click(t, pos, reach);
        press.acted = !matches!(outcome, Outcome::Ignored);
        if tool == Tool::Point {
            let c = self.wb.construction();
            press.point = match outcome {
                // Only a placement that succeeded made the last object.
                Outcome::Place { .. } if c.len() > before => c.ids().last(),
                Outcome::Select(label) => c.id(&label),
                _ => None,
            }
            .filter(|&id| interact::movable(c, id));
        }
        press
    }

    /// The release that ends a construction tool's press, at `pos` in the
    /// area (`t` the view now). A drag-creating tool holding its first point
    /// picks here when the press waited for the release, or when the
    /// pointer went beyond the point selection distance from the press
    /// (`withinPointSelectionDistance`, Euclidean, threshold + 12 px)
    /// (`EuclidianController.wrapMouseReleased`).
    pub fn release_tool(
        &mut self,
        press: ToolPress,
        t: &ViewTransform,
        pos: (f64, f64),
        reach: &Reach,
    ) {
        if !creates_by_drag(self.tool.tool()) || self.tool.picks().len() != 1 {
            return;
        }
        let distance = (pos.0 - press.at.0).hypot(pos.1 - press.at.1);
        let within = distance < reach.threshold + reach.point_min;
        if press.deferred || !within {
            self.tool_click(t, pos, reach);
        }
    }

    /// Where the free end of a tool's preview is drawn for the pointer at
    /// `pos` (in the area): on the point under the pointer, by the hit rule
    /// a click uses, else at the pointer (spec E6).
    pub fn preview_end(&self, t: &ViewTransform, pos: (f64, f64), reach: &Reach) -> (f64, f64) {
        let c = self.wb.construction();
        hit::pick(c, t, pos, reach, None, |id| self.hittable(id))
            .filter(|&id| c.kind(id) == Kind::Point)
            .and_then(|id| match c.value(id) {
                Value::Point { x, y } => Some(t.world_to_screen(x, y)),
                _ => None,
            })
            .unwrap_or(pos)
    }

    /// One move of the eraser's stroke at `pos` in the area: deletes every
    /// object that the square of `ERASER_SIZE` around it meets
    /// (`HitDetector.setIntersectionHits`; a point by its drawn disc).
    /// Returns whether something was deleted.
    pub fn erase_at(&mut self, t: &ViewTransform, pos: (f64, f64), point_radius: f64) -> bool {
        let half = ERASER_SIZE / 2.0;
        let reach = Reach {
            threshold: half,
            retry: None,
            point_min: point_radius,
            mouse: half,
        };
        let mut deleted = false;
        loop {
            let c = self.wb.construction();
            let found = hit::hits(c, t, pos, &reach, |id| self.wb.is_visible(id));
            // One at a time: a deletion takes dependents along and renumbers
            // the rest.
            let Some(&id) = found.first() else {
                break;
            };
            if !self.wb.delete(id) {
                break;
            }
            deleted = true;
        }
        deleted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::Camera;
    use crate::workbench::Workbench;

    /// The origin at (400, 300), 50 pixels per unit.
    fn t() -> ViewTransform {
        Camera {
            cx: 0.0,
            cy: 0.0,
            scale: 50.0,
        }
        .transform(800.0, 600.0)
    }

    fn at(x: f64, y: f64) -> (f64, f64) {
        t().world_to_screen(x, y)
    }

    fn app(src: &str, tool: Tool) -> App {
        let mut app = App::new(Workbench::from_script(src).unwrap(), None);
        app.select_tool(tool);
        app
    }

    fn reach(app: &App) -> Reach {
        Reach::of(&app.metrics, false)
    }

    fn labels(app: &App) -> Vec<String> {
        let c = app.wb.construction();
        c.ids().map(|id| c.label(id).to_owned()).collect()
    }

    const CIRCLE_AND_LINE: &str =
        "O = (0, 0)\nP = (2, 0)\nc = Circle(O, P)\nQ = (-3, 1)\nR = (3, 1)\nf = Line(Q, R)\n";

    /// A new point where a circle and a line cross is their intersection,
    /// the one of the two nearer the pointer (spec E2,
    /// `getSingleIntersectionPoint`). Fails if new points were free.
    #[test]
    fn a_new_point_on_a_crossing_is_the_nearer_intersection() {
        let mut app = app(CIRCLE_AND_LINE, Tool::Point);
        let r = reach(&app);
        for x in [3f64.sqrt(), -(3f64.sqrt())] {
            let NewPoint::Intersection { a, b, index } = app.new_point_at(&t(), at(x, 1.0), &r)
            else {
                panic!("not an intersection at {x}");
            };
            let n = index.expect("two points");
            let p = app
                .wb
                .probe_point(&format!("Intersect({a}, {b}, {n})"))
                .unwrap();
            assert!((p.0 - x).abs() < 1e-9 && (p.1 - 1.0).abs() < 1e-9, "{p:?}");
        }
    }

    /// On one path the new point is a point on it; the parallel line tool
    /// puts it there only once a line is picked, the angle tool never
    /// (spec E2, the per-mode cases of `getNewPoint`).
    #[test]
    fn new_points_go_on_paths_only_for_the_tools_that_allow_it() {
        let on_f = at(-2.5, 1.0);
        let mut app = app(CIRCLE_AND_LINE, Tool::Point);
        let r = reach(&app);
        assert!(matches!(
            app.new_point_at(&t(), on_f, &r),
            NewPoint::OnPath { path, .. } if path == "f"
        ));
        app.select_tool(Tool::Angle);
        assert!(matches!(
            app.new_point_at(&t(), on_f, &r),
            NewPoint::Free(..)
        ));
        app.select_tool(Tool::Parallel);
        assert!(matches!(
            app.new_point_at(&t(), on_f, &r),
            NewPoint::Free(..)
        ));
        app.press_tool(&t(), on_f, &r);
        assert_eq!(app.tool.picks().len(), 1, "the line is picked");
        assert!(matches!(
            app.new_point_at(&t(), at(0.0, 2.0), &r),
            NewPoint::OnPath { path, .. } if path == "c"
        ));
    }

    /// The segment tool makes its first point when the button goes down
    /// and, after a drag, its second one where the button goes up (spec
    /// E1, E3); a release near the press makes nothing more, the next press
    /// waits for its release. Fails if a press picked nothing.
    #[test]
    fn the_segment_tool_picks_on_the_press_and_after_a_drag_on_the_release() {
        let mut app = app("", Tool::Segment);
        let r = reach(&app);
        let press = app.press_tool(&t(), at(-2.3, 1.7), &r);
        assert!(press.acted && !press.deferred);
        assert_eq!(labels(&app), ["A"]);
        app.release_tool(press, &t(), at(1.7, -1.3), &r);
        assert_eq!(labels(&app), ["A", "B", "f"]);
        assert!(app.wb.can_undo());
        app.wb.undo();
        assert!(labels(&app).is_empty(), "one undo step for the whole use");

        let press = app.press_tool(&t(), at(-2.3, 1.7), &r);
        let near = (press.at.0 + 5.0, press.at.1);
        app.release_tool(press, &t(), near, &r);
        assert_eq!(labels(&app), ["A"], "a click picks once");
        let press = app.press_tool(&t(), at(1.7, -1.3), &r);
        assert!(press.deferred);
        assert_eq!(labels(&app), ["A"]);
        app.release_tool(press, &t(), at(1.7, -1.3), &r);
        assert_eq!(labels(&app), ["A", "B", "f"]);
    }

    /// The point tool's press makes the point that a drag then moves, or
    /// takes an existing free one (spec E4).
    #[test]
    fn the_point_tools_press_holds_the_point_it_made_or_found() {
        let mut app = app("A = (1, 1)\n", Tool::Point);
        let r = reach(&app);
        let press = app.press_tool(&t(), at(-2.3, 1.7), &r);
        let c = app.wb.construction();
        assert_eq!(press.point, c.id("B"));
        let press = app.press_tool(&t(), at(1.0, 1.0), &r);
        assert_eq!(press.point, app.wb.construction().id("A"));
        assert_eq!(labels(&app), ["A", "B"]);
    }

    /// The preview's free end jumps onto a point under the pointer, by the
    /// hit rule of a click, and stays at the pointer elsewhere (spec E6).
    /// Fails if the end followed the pointer only.
    #[test]
    fn the_preview_end_snaps_to_a_point_under_the_pointer() {
        let app = app("A = (1, 1)\nB = (-2, 1)\nf = Line(A, B)\n", Tool::Segment);
        let r = reach(&app);
        let a = at(1.0, 1.0);
        let near = (a.0 + 6.0, a.1 - 5.0);
        assert_eq!(app.preview_end(&t(), near, &r), a);
        let on_line = at(-0.5, 1.0);
        assert_eq!(app.preview_end(&t(), on_line, &r), on_line);
        let far = (a.0 + 40.0, a.1);
        assert_eq!(app.preview_end(&t(), far, &r), far);
    }

    /// The eraser deletes what the square around the pointer meets, points
    /// by their drawn disc (spec E11).
    #[test]
    fn the_eraser_deletes_what_its_square_meets() {
        let src = "A = (0, 0)\nB = (4, 0)\nf = Segment(A, B)\nC = (0, 3)\n";
        let mut app = app(src, Tool::Delete);
        let (x, y) = at(2.0, 0.0);
        assert!(!app.erase_at(&t(), (x, y - 11.0), 5.0), "10 px is the edge");
        assert!(app.erase_at(&t(), (x, y - 9.0), 5.0));
        assert_eq!(labels(&app), ["A", "B", "C"]);
        let (x, y) = at(0.0, 3.0);
        assert!(app.erase_at(&t(), (x + 14.0, y), 5.0));
        assert_eq!(labels(&app), ["A", "B"]);
    }

    fn picks(app: &App) -> Vec<String> {
        app.tool
            .picks()
            .iter()
            .map(|p| p.label().to_owned())
            .collect()
    }

    fn type_line(app: &mut App, line: &str) {
        app.input = line.to_owned();
        app.submit_input();
    }

    /// The segment tool holding its first point A (pending), with one step
    /// to redo (a typed `Y`, undone).
    fn segment_use_with_a_redo() -> App {
        let mut app = app("Z = (9, 9)\n", Tool::Segment);
        let r = reach(&app);
        type_line(&mut app, "Y = (8, 8)");
        assert!(app.undo());
        assert_eq!(app.wb.undo_depth(), (0, 1));
        app.press_tool(&t(), at(1.0, 1.0), &r);
        assert_eq!(picks(&app), ["A"]);
        assert_eq!(labels(&app), ["Z", "A"]);
        app
    }

    /// Undo in the middle of a use takes only the use back and leaves the
    /// redo stack as it was (`Kernel.undo`, `restoreStateForModeStarting`).
    #[test]
    fn undo_mid_use_takes_only_the_use_back() {
        let mut app = segment_use_with_a_redo();
        assert!(app.can_undo());
        assert!(app.undo());
        assert!(picks(&app).is_empty());
        assert_eq!(labels(&app), ["Z"]);
        assert_eq!(app.wb.undo_depth(), (0, 1));
        assert!(!app.can_undo());
        assert!(app.redo());
        assert_eq!(labels(&app), ["Z", "Y"]);
        assert_eq!(app.wb.undo_depth(), (1, 0));
    }

    /// Redo in the middle of a use ends it unfinished, then redoes.
    #[test]
    fn redo_mid_use_takes_the_use_back_then_redoes() {
        let mut app = segment_use_with_a_redo();
        assert!(app.redo());
        assert!(picks(&app).is_empty());
        assert_eq!(labels(&app), ["Z", "Y"]);
        assert_eq!(app.wb.undo_depth(), (1, 0));
        assert!(app.undo());
        assert_eq!(labels(&app), ["Z"]);
        assert!(!app.can_undo());
    }

    /// A line typed in the middle of a use: one that fails changes nothing
    /// and the use goes on; one that runs takes the use back (its point does
    /// not survive as an orphan) and is the only step.
    #[test]
    fn a_line_typed_mid_use_keeps_the_use_only_when_it_fails() {
        let mut app = app("Z = (9, 9)\n", Tool::Segment);
        let r = reach(&app);
        app.press_tool(&t(), at(1.0, 1.0), &r);
        type_line(&mut app, "Nonsense((");
        assert!(app.input_error.is_some());
        assert_eq!(picks(&app), ["A"]);
        assert_eq!(labels(&app), ["Z", "A"]);
        assert_eq!(app.wb.undo_depth(), (0, 0));
        // The use can still finish, as one step.
        let press = app.press_tool(&t(), at(2.0, 2.0), &r);
        app.release_tool(press, &t(), at(2.0, 2.0), &r);
        assert!(picks(&app).is_empty());
        assert_eq!(labels(&app), ["Z", "A", "B", "f"]);
        assert_eq!(app.wb.undo_depth(), (1, 0));
        assert!(app.undo());
        assert_eq!(labels(&app), ["Z"]);

        app.press_tool(&t(), at(1.0, 1.0), &r);
        type_line(&mut app, "Y = (8, 8)");
        assert!(picks(&app).is_empty());
        assert_eq!(labels(&app), ["Z", "Y"]);
        app.select_tool(Tool::Move);
        assert_eq!(labels(&app), ["Z", "Y"]);
        assert_eq!(app.wb.undo_depth(), (1, 0));
        assert!(app.undo());
        assert_eq!(labels(&app), ["Z"]);
    }

    /// A tool switch in the middle of the eraser's stroke stops it, and
    /// what it deleted is stored as one step (`storeUndoAfterDrag` runs at
    /// the release whatever the mode). Escape reaches the same end through
    /// the gesture's stop; this is any other switch.
    #[test]
    fn a_tool_switch_mid_stroke_ends_the_eraser() {
        let mut app = app(CIRCLE_AND_LINE, Tool::Delete);
        app.erasing = true;
        app.wb.begin_gesture();
        assert!(app.erase_at(&t(), at(0.0, 1.0), 4.0));
        assert_eq!(labels(&app), ["O", "P", "c", "Q", "R"]);
        app.select_tool(Tool::Move);
        assert!(!app.erasing);
        assert_eq!(app.wb.undo_depth(), (1, 0));
        assert!(app.undo());
        assert_eq!(labels(&app), ["O", "P", "c", "Q", "R", "f"]);
    }
}
