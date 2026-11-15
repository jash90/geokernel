// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianController.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoElement.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoElement.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Dragging logic with no UI types: view transform, what can move, drag.
//!
//! Screen coordinates are in pixels with the origin at the top-left corner of
//! the drawing area and the y axis pointing down. World coordinates have the
//! y axis pointing up.

use crate::snap::Snap;
use geo_kernel::algo::vector::VectorDef;
use geo_kernel::kernel::{Construction, Def, Id, Kind, Value};
use geo_kernel::svg::View;

/// Maps between world and screen coordinates with one uniform scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransform {
    view: View,
    scale: f64,
    ox: f64,
    oy: f64,
    height: f64,
}

impl ViewTransform {
    /// Fits `view` into a `width` x `height` area, centred, keeping the aspect ratio.
    #[inline]
    pub fn new(view: View, width: f64, height: f64) -> Self {
        let vw = (view.xmax - view.xmin).max(f64::MIN_POSITIVE);
        let vh = (view.ymax - view.ymin).max(f64::MIN_POSITIVE);
        let scale = (width / vw).min(height / vh);
        ViewTransform {
            view,
            scale,
            ox: 0.5 * (width - vw * scale),
            oy: 0.5 * (height - vh * scale),
            height,
        }
    }

    /// A transform fitted to the whole construction, with a margin.
    #[cfg(test)]
    pub fn fit(c: &Construction, width: f64, height: f64) -> Self {
        Self::new(View::fit(c), width, height)
    }

    /// The visible rectangle of the plane this transform was built from.
    #[inline]
    pub fn view(&self) -> View {
        self.view
    }

    /// Pixels per world unit.
    #[inline]
    pub fn scale(&self) -> f64 {
        self.scale
    }

    #[inline]
    pub fn world_to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.ox + (x - self.view.xmin) * self.scale,
            self.height - self.oy - (y - self.view.ymin) * self.scale,
        )
    }

    #[inline]
    pub fn screen_to_world(&self, sx: f64, sy: f64) -> (f64, f64) {
        (
            self.view.xmin + (sx - self.ox) / self.scale,
            self.view.ymin + (self.height - self.oy - sy) / self.scale,
        )
    }
}

/// Whether `id` is a free point (`GeoElement.isFreeInputPoint`; the kernel
/// has no points on paths, which the reference app also counts).
fn free_point(c: &Construction, id: Id) -> bool {
    matches!(c.def(id), Def::FreePoint { .. })
}

/// Whether the move tool can move the dependent object `id` by its free
/// input points (`GeoElement.hasMoveableInputPoints`, with
/// `EuclidianController.isElementAllowedToMove`):
///
/// - lines, segments, rays, circles and conics through points: every input
///   is a free point (`hasOnlyFreeInputPoints`);
/// - a circle with a centre and a radius (`AlgoCirclePointRadiusInterface`),
///   polygons and polylines: at least one input is a free point, and only
///   those move;
/// - a vector from point to point (`Vector(A, B)`): both are free points.
///
/// An expression has no inputs (`DependentAlgo` gives no free input
/// points), so it does not move this way. The reference app's other
/// branches of `handleMovedElementDependent` (a translated image moved by
/// its vector, `Vector(A, B)` with a dependent end moved by its start) are
/// not followed: those objects are refused.
fn moves_by_input_points(c: &Construction, id: Id) -> bool {
    let def = c.def(id);
    if def.is_free() {
        return false;
    }
    let all_free = || c.inputs(id).next().is_some() && c.inputs(id).all(|p| free_point(c, p));
    let any_free = || c.inputs(id).any(|p| free_point(c, p));
    match c.kind(id) {
        Kind::Line | Kind::Segment | Kind::Ray => all_free(),
        Kind::Circle | Kind::Conic => match def {
            Def::CircleRadius { .. } => any_free(),
            _ => all_free(),
        },
        Kind::Polygon | Kind::PolyLine => any_free(),
        Kind::Vector => matches!(def, Def::Vector(VectorDef::Between(..))) && all_free(),
        _ => false,
    }
}

/// The free input points of `id`, each once, in the order of its inputs
/// (`AlgoElement.getFreeInputPoints`): what a drag of the dependent object
/// translates. The first one is captured to the grid.
pub fn free_input_points(c: &Construction, id: Id) -> Vec<Id> {
    let mut points: Vec<Id> = Vec::new();
    for p in c.inputs(id) {
        if free_point(c, p) && !points.contains(&p) {
            points.push(p);
        }
    }
    points
}

/// Whether the move tool can drag the object `id`
/// (`EuclidianController.handleMovedElement`): a free point moves itself
/// (`MoveMode.POINT`), a dependent object moves its free input points
/// (`MoveMode.DEPENDENT`, see `moves_by_input_points`). What the press takes
/// hold of is chosen by `hit::pick`; an object it chose that cannot move is
/// refused.
pub fn movable(c: &Construction, id: Id) -> bool {
    id.index() < c.len()
        && match c.def(id) {
            Def::FreePoint { .. } => matches!(c.value(id), Value::Point { .. }),
            _ => moves_by_input_points(c, id),
        }
}

/// The owner's choice where the reference app is not followed (it refuses
/// such a drag without a word, `isElementAllowedToMove`): a drag of a point
/// that depends on other objects shows a short notice, once per press, and
/// does not pan. `false` would refuse silently.
pub const REFUSED_DRAG_NOTICE: bool = true;

/// What a drag holds, and the state its moves need.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hold {
    /// A free point: it goes under the pointer, captured to the grid
    /// (`MoveMode.POINT`; `handleMovedElementFreePoint` sets no offset).
    Point,
    /// A dependent object: its free input points are translated by the
    /// pointer's motion from `start` (`MoveMode.DEPENDENT`, `moveDependent`),
    /// with the first of them captured to the grid through `offset` (that
    /// point minus the press position, `initOffsetFrom`).
    Dependent {
        start: (f64, f64),
        offset: (f64, f64),
    },
    /// The object's label: its offset in pixels is the one it had (`old`)
    /// plus the pointer's motion from the press (`start`, in pixels)
    /// (`MoveMode.LABEL`, `moveLabel`). No undo step, no selection.
    Label { old: (f64, f64), start: (f64, f64) },
}

/// A drag in progress: which object is held, and how.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    id: Id,
    hold: Hold,
}

/// What one move of a drag does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Put the held free point at `(x, y)`.
    Point { x: f64, y: f64 },
    /// Translate the free input points of the held object by `(dx, dy)`.
    Translate { dx: f64, dy: f64 },
    /// Set the held object's label offset to `(x, y)` pixels.
    Label { x: f64, y: f64 },
}

impl Drag {
    /// Starts dragging `id`, when the move tool can drag it. `press` is the
    /// world position of the press, captured to the grid with no offset
    /// (`wrapMousePressed` calls `transformCoords` first; the offset is reset
    /// at every release).
    pub fn begin(c: &Construction, id: Id, press: (f64, f64)) -> Option<Drag> {
        if !movable(c, id) {
            return None;
        }
        if free_point(c, id) {
            return Some(Drag {
                id,
                hold: Hold::Point,
            });
        }
        // `handleMovedElementDependentInitMode`: the offset comes from the
        // first free input point, and the start is the press position.
        let first = free_input_points(c, id).first().copied()?;
        let Value::Point { x, y } = c.value(first) else {
            return None;
        };
        Some(Drag {
            id,
            hold: Hold::Dependent {
                start: press,
                offset: (x - press.0, y - press.1),
            },
        })
    }

    /// Starts dragging the label of `id`, whose offset is `old`, from the
    /// press at `press` (pixels).
    pub fn label(id: Id, old: (f64, f64), press: (f64, f64)) -> Drag {
        Drag {
            id,
            hold: Hold::Label { old, start: press },
        }
    }

    /// The object being dragged.
    #[inline]
    pub fn id(&self) -> Id {
        self.id
    }

    /// What the drag holds.
    #[inline]
    pub fn hold(&self) -> Hold {
        self.hold
    }

    /// The move for the pointer at `(sx, sy)` (`wrapMouseDraggedND` calls
    /// `transformCoords`, then moves what is held).
    pub fn step(&mut self, t: &ViewTransform, snap: &Snap, sx: f64, sy: f64) -> Step {
        match &mut self.hold {
            Hold::Point => {
                let (x, y) = snap.apply(t.screen_to_world(sx, sy));
                Step::Point { x, y }
            }
            Hold::Dependent { start, offset } => {
                let (x, y) = snap.capture(t.screen_to_world(sx, sy), *offset);
                let (dx, dy) = (x - start.0, y - start.1);
                *start = (x, y);
                Step::Translate { dx, dy }
            }
            Hold::Label { old, start } => Step::Label {
                x: old.0 + sx - start.0,
                y: old.1 + sy - start.1,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> Construction {
        geo_kernel::script::load(include_str!("../../../examples/triangle.geo")).unwrap()
    }

    fn screen_of(c: &Construction, t: &ViewTransform, label: &str) -> (f64, f64) {
        let Value::Point { x, y } = c.value(c.id(label).unwrap()) else {
            panic!("{label} is not a point");
        };
        t.world_to_screen(x, y)
    }

    #[test]
    fn transform_round_trips() {
        let c = triangle();
        let t = ViewTransform::fit(&c, 800.0, 500.0);
        for (x, y) in [(0.0, 0.0), (6.0, 5.0), (-1.5, 3.25)] {
            let (sx, sy) = t.world_to_screen(x, y);
            let (rx, ry) = t.screen_to_world(sx, sy);
            assert!((rx - x).abs() < 1e-9 && (ry - y).abs() < 1e-9);
        }
    }

    #[test]
    fn y_axis_points_up() {
        let t = ViewTransform::new(
            View {
                xmin: 0.0,
                ymin: 0.0,
                xmax: 10.0,
                ymax: 10.0,
            },
            100.0,
            100.0,
        );
        assert_eq!(t.world_to_screen(0.0, 0.0), (0.0, 100.0));
        assert_eq!(t.world_to_screen(10.0, 10.0), (100.0, 0.0));
    }

    #[test]
    fn fit_contains_all_points() {
        let c = triangle();
        let t = ViewTransform::fit(&c, 640.0, 480.0);
        for id in c.ids() {
            if let Value::Point { x, y } = c.value(id) {
                let (sx, sy) = t.world_to_screen(x, y);
                assert!((0.0..=640.0).contains(&sx) && (0.0..=480.0).contains(&sy));
            }
        }
    }

    /// Where a point drag puts the point for the pointer at `(sx, sy)`.
    fn point_step(drag: &mut Drag, t: &ViewTransform, snap: &Snap, sx: f64, sy: f64) -> (f64, f64) {
        match drag.step(t, snap, sx, sy) {
            Step::Point { x, y } => (x, y),
            other => panic!("not a point drag: {other:?}"),
        }
    }

    #[test]
    fn free_points_are_movable_and_dependent_points_are_not() {
        let c = triangle();
        assert!(movable(&c, c.id("A").unwrap()));
        assert!(!c.def(c.id("O").unwrap()).is_free());
        assert!(!movable(&c, c.id("O").unwrap()));
        assert!(!movable(&c, c.id("Mc").unwrap()));
    }

    #[test]
    fn drag_moves_point_and_dependents() {
        let mut c = triangle();
        let t = ViewTransform::fit(&c, 800.0, 600.0);
        let mc = c.id("Mc").unwrap();
        let before = c.value(mc);
        let (ax, ay) = screen_of(&c, &t, "A");
        let mut drag = Drag::begin(&c, c.id("A").unwrap(), (0.0, 0.0)).expect("grab A");
        let (tx, ty) = point_step(&mut drag, &t, &Snap::OFF, ax - 60.0, ay - 40.0);
        let recomputed = c.move_point(drag.id(), tx, ty).unwrap();
        assert!(recomputed > 0);
        let (nx, ny) = screen_of(&c, &t, "A");
        assert!((nx - (ax - 60.0)).abs() < 1e-6 && (ny - (ay - 40.0)).abs() < 1e-6);
        assert_ne!(c.value(mc), before);
        let Value::Point { x, y } = c.value(mc) else {
            panic!("Mc must stay a point");
        };
        let Value::Point { x: bx, y: by } = c.value(c.id("B").unwrap()) else {
            panic!();
        };
        let Value::Point { x: axw, y: ayw } = c.value(c.id("A").unwrap()) else {
            panic!();
        };
        assert!((x - 0.5 * (axw + bx)).abs() < 1e-9 && (y - 0.5 * (ayw + by)).abs() < 1e-9);
    }

    #[test]
    fn drag_does_not_start_on_a_dependent_point() {
        let c = triangle();
        assert!(Drag::begin(&c, c.id("O").unwrap(), (0.0, 0.0)).is_none());
    }

    /// Was `target_keeps_the_grab_offset`: the point jumps to the pointer
    /// (`handleMovedElementFreePoint`, no grab offset).
    #[test]
    fn the_target_is_under_the_pointer() {
        let c = triangle();
        let t = ViewTransform::fit(&c, 800.0, 600.0);
        let (ax, ay) = screen_of(&c, &t, "A");
        let mut drag = Drag::begin(&c, c.id("A").unwrap(), (0.0, 0.0)).unwrap();
        let (x, y) = point_step(&mut drag, &t, &Snap::OFF, ax + 3.0, ay - 2.0);
        let (wx, wy) = t.screen_to_world(ax + 3.0, ay - 2.0);
        assert!((x - wx).abs() < 1e-12 && (y - wy).abs() < 1e-12);
        assert!(x.abs() > 1e-6, "not where A was");
    }

    /// `transformCoords` runs on every drag move: near a grid crossing the
    /// point lands on it.
    #[test]
    fn the_target_is_captured_to_the_grid() {
        let c = triangle();
        let t = crate::view::Camera::STANDARD.transform(800.0, 600.0);
        let mut drag = Drag::begin(&c, c.id("A").unwrap(), (0.0, 0.0)).unwrap();
        let snap = Snap::new(&crate::metrics::Metrics::desktop(), t.scale(), true);
        let (sx, sy) = t.world_to_screen(3.0, 2.0);
        assert_eq!(
            point_step(&mut drag, &t, &snap, sx + 5.0, sy - 5.0),
            (3.0, 2.0)
        );
        // 8 px is outside the 6.25 px box.
        let (x, _) = point_step(&mut drag, &t, &snap, sx + 8.0, sy);
        assert!((x - 3.16).abs() < 1e-9, "{x}");
    }

    fn load(src: &str) -> Construction {
        geo_kernel::script::load(src).unwrap()
    }

    /// `hasMoveableInputPoints`: lines, segments and circles through free
    /// points, polygons with a free vertex and circles with a free centre
    /// move by their points; a line through a point perpendicular to
    /// another, an expression and a dependent point do not.
    #[test]
    fn dependent_objects_move_by_their_free_input_points() {
        let c = load(
            "A = (1, 1)\nB = (3, 2)\nC = (1, 4)\nf = Line(A, B)\ns = Segment(A, B)\n\
             k = Circle(A, B)\nr = 2\nm = Circle(A, r)\nM = Midpoint(A, B)\n\
             p = Polygon(A, B, C)\nq = Polygon(M, B, C)\nn = Polygon(M, M, M)\n\
             g = PerpendicularLine(C, f)\nh = Line(M, C)\nv = Vector(A, B)\nw = Vector(A)\n",
        );
        let id = |l: &str| c.id(l).unwrap();
        for l in ["f", "s", "k", "m", "p", "q", "v"] {
            assert!(movable(&c, id(l)), "{l} does not move");
        }
        for l in ["g", "h", "M", "n", "w", "r"] {
            assert!(!movable(&c, id(l)), "{l} moves");
        }
        assert_eq!(free_input_points(&c, id("q")), [id("B"), id("C")]);
        assert_eq!(free_input_points(&c, id("m")), [id("A")]);
    }

    fn translate(drag: &mut Drag, t: &ViewTransform, snap: &Snap, at: (f64, f64)) -> (f64, f64) {
        let (sx, sy) = t.world_to_screen(at.0, at.1);
        match drag.step(t, snap, sx, sy) {
            Step::Translate { dx, dy } => (dx, dy),
            other => panic!("not a dependent drag: {other:?}"),
        }
    }

    fn near(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    /// `moveDependent`: the free points follow the pointer's motion from the
    /// press, captured to the grid with no offset (the circle's press at
    /// (1, 3.01) counts from (1, 3); scenario A5), and every move counts
    /// from the last.
    #[test]
    fn a_dependent_drag_counts_from_the_captured_press() {
        let c = load("A = (1, 1.01)\nB = (3, 1.01)\nk = Circle(A, B)\n");
        let t = crate::view::Camera::STANDARD.transform(892.0, 747.0);
        let snap = Snap::new(&crate::metrics::Metrics::desktop(), t.scale(), true);
        let press = snap.capture((1.0, 3.01), (0.0, 0.0));
        assert_eq!(press, (1.0, 3.0));
        let mut drag = Drag::begin(&c, c.id("k").unwrap(), press).unwrap();
        let d = translate(&mut drag, &t, &snap, (1.6, 2.6));
        assert!(near(d, (0.6, -0.4)), "{d:?}");
        let d = translate(&mut drag, &t, &snap, (2.2, 2.21));
        assert!(near(d, (0.6, -0.39)), "{d:?}");
    }

    /// `initOffsetFrom`: the first free input point is captured to the grid
    /// (per the source; not measured). The line through A = (1, 1.01) and B
    /// pressed at (2, 1.51): at the pointer (2.08, 1.456667) A would be at
    /// (1.08, 0.956667), within the box of (1, 1), so A lands on (1, 1).
    #[test]
    fn a_dependent_drag_captures_its_first_free_point() {
        let c = load("A = (1, 1.01)\nB = (3, 2.01)\nf = Line(A, B)\n");
        let t = crate::view::Camera::STANDARD.transform(892.0, 747.0);
        let snap = Snap::new(&crate::metrics::Metrics::desktop(), t.scale(), true);
        let press = snap.capture((2.0, 1.51), (0.0, 0.0));
        assert_eq!(press, (2.0, 1.51));
        let mut drag = Drag::begin(&c, c.id("f").unwrap(), press).unwrap();
        let d = translate(&mut drag, &t, &snap, (2.08, 1.456667));
        assert!(near((1.0 + d.0, 1.01 + d.1), (1.0, 1.0)), "{d:?}");
        // Out of the box again: A follows the pointer exactly.
        let d2 = translate(&mut drag, &t, &snap, (3.2, 0.71));
        let a = (1.0 + d.0 + d2.0, 1.01 + d.1 + d2.1);
        assert!(near(a, (2.2, 0.21)), "{a:?}");
    }

    /// `moveLabel`: the offset is the old one plus the pointer's motion in
    /// pixels, whatever the zoom.
    #[test]
    fn a_label_drag_moves_the_offset_by_the_pointer() {
        let c = load("A = (1, 1)\n");
        let t = crate::view::Camera::STANDARD.transform(892.0, 747.0);
        let mut drag = Drag::label(c.id("A").unwrap(), (5.0, -2.0), (100.0, 200.0));
        assert_eq!(
            drag.step(&t, &Snap::OFF, 140.0, 230.0),
            Step::Label { x: 45.0, y: 28.0 }
        );
    }
}
