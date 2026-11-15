//! Dragging logic with no UI types: view transform, what can move, drag.
//!
//! Screen coordinates are in pixels with the origin at the top-left corner of
//! the drawing area and the y axis pointing down. World coordinates have the
//! y axis pointing up.

use geo_kernel::kernel::{Construction, Def, Id, Value};
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

/// Whether the move tool can drag the object `id`: a free point
/// (`EuclidianController.handleMovedElement` moves a free point as
/// `MoveMode.POINT`). What the press takes hold of is chosen by
/// `hit::pick`; an object it chose that cannot move is refused.
pub fn movable(c: &Construction, id: Id) -> bool {
    id.index() < c.len()
        && matches!(c.def(id), Def::FreePoint { .. })
        && matches!(c.value(id), Value::Point { .. })
}

/// The owner's choice where the reference app is not followed (it refuses
/// such a drag without a word, `isElementAllowedToMove`): a drag of a point
/// that depends on other objects shows a short notice, once per press, and
/// does not pan. `false` would refuse silently.
pub const REFUSED_DRAG_NOTICE: bool = true;

/// A drag in progress: which object is held.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    id: Id,
}

impl Drag {
    /// Starts dragging `id`, when the move tool can drag it.
    pub fn begin(c: &Construction, id: Id) -> Option<Drag> {
        movable(c, id).then_some(Drag { id })
    }

    /// The object being dragged.
    #[inline]
    pub fn id(&self) -> Id {
        self.id
    }

    /// Where the held point goes for the pointer at `(sx, sy)`: under the
    /// pointer, wherever it was grabbed (`handleMovedElementFreePoint` sets
    /// no grab offset, and `movePoint` takes the pointer's coordinates).
    #[inline]
    pub fn target(&self, t: &ViewTransform, sx: f64, sy: f64) -> (f64, f64) {
        t.screen_to_world(sx, sy)
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
        let drag = Drag::begin(&c, c.id("A").unwrap()).expect("grab A");
        let (tx, ty) = drag.target(&t, ax - 60.0, ay - 40.0);
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
        assert!(Drag::begin(&c, c.id("O").unwrap()).is_none());
    }

    /// Was `target_keeps_the_grab_offset`: the point jumps to the pointer
    /// (`handleMovedElementFreePoint`, no grab offset).
    #[test]
    fn the_target_is_under_the_pointer() {
        let c = triangle();
        let t = ViewTransform::fit(&c, 800.0, 600.0);
        let (ax, ay) = screen_of(&c, &t, "A");
        let drag = Drag::begin(&c, c.id("A").unwrap()).unwrap();
        let (x, y) = drag.target(&t, ax + 3.0, ay - 2.0);
        let (wx, wy) = t.screen_to_world(ax + 3.0, ay - 2.0);
        assert!((x - wx).abs() < 1e-12 && (y - wy).abs() < 1e-12);
        assert!(x.abs() > 1e-6, "not where A was");
    }
}
