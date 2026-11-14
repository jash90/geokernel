//! Dragging logic with no UI types: view transform, hit test and drag.
//!
//! Screen coordinates are in pixels with the origin at the top-left corner of
//! the drawing area and the y axis pointing down. World coordinates have the
//! y axis pointing up.

use geo_kernel::kernel::{Construction, Def, Error, Id, Value};
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
    pub fn fit(c: &Construction, width: f64, height: f64) -> Self {
        Self::new(View::fit(c), width, height)
    }

    /// The visible rectangle of the plane this transform was built from.
    pub fn view(&self) -> View {
        self.view
    }

    /// Pixels per world unit.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn world_to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.ox + (x - self.view.xmin) * self.scale,
            self.height - self.oy - (y - self.view.ymin) * self.scale,
        )
    }

    pub fn screen_to_world(&self, sx: f64, sy: f64) -> (f64, f64) {
        (
            self.view.xmin + (sx - self.ox) / self.scale,
            self.view.ymin + (self.height - self.oy - sy) / self.scale,
        )
    }
}

/// The free point nearest to the screen position, if it lies within `radius` pixels.
/// Dependent points, numbers and undefined points are never returned.
pub fn hit_test(c: &Construction, t: &ViewTransform, sx: f64, sy: f64, radius: f64) -> Option<Id> {
    let mut best: Option<(Id, f64)> = None;
    for id in c.ids() {
        if !matches!(c.def(id), Def::FreePoint { .. }) {
            continue;
        }
        let Value::Point { x, y } = c.value(id) else {
            continue;
        };
        let (px, py) = t.world_to_screen(x, y);
        let d = (px - sx).hypot(py - sy);
        if d <= radius && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((id, d));
        }
    }
    best.map(|(id, _)| id)
}

/// A drag in progress: which point is held and where it was grabbed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    id: Id,
    /// World offset from the cursor to the point, so the point does not jump.
    dx: f64,
    dy: f64,
}

impl Drag {
    /// Starts dragging the free point under the cursor, if there is one.
    pub fn begin(
        c: &Construction,
        t: &ViewTransform,
        sx: f64,
        sy: f64,
        radius: f64,
    ) -> Option<Drag> {
        let id = hit_test(c, t, sx, sy, radius)?;
        let Value::Point { x, y } = c.value(id) else {
            return None;
        };
        let (wx, wy) = t.screen_to_world(sx, sy);
        Some(Drag {
            id,
            dx: x - wx,
            dy: y - wy,
        })
    }

    /// The point being dragged.
    #[cfg(test)]
    pub fn id(&self) -> Id {
        self.id
    }

    /// Moves the held point to follow the cursor. Returns how many dependent
    /// objects were recomputed.
    pub fn apply(
        &self,
        c: &mut Construction,
        t: &ViewTransform,
        sx: f64,
        sy: f64,
    ) -> Result<usize, Error> {
        let (wx, wy) = t.screen_to_world(sx, sy);
        c.move_point(self.id, wx + self.dx, wy + self.dy)
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
    fn hit_test_picks_nearest_free_point() {
        let c = triangle();
        let t = ViewTransform::fit(&c, 800.0, 600.0);
        let (ax, ay) = screen_of(&c, &t, "A");
        assert_eq!(hit_test(&c, &t, ax + 3.0, ay - 2.0, 10.0), c.id("A"));
        let (bx, by) = screen_of(&c, &t, "B");
        assert_eq!(hit_test(&c, &t, bx, by, 10.0), c.id("B"));
        assert_eq!(hit_test(&c, &t, ax + 200.0, ay - 200.0, 10.0), None);
    }

    #[test]
    fn hit_test_ignores_dependent_points() {
        let c = triangle();
        let t = ViewTransform::fit(&c, 800.0, 600.0);
        let (ox, oy) = screen_of(&c, &t, "O");
        assert!(!c.def(c.id("O").unwrap()).is_free());
        assert!(hit_test(&c, &t, ox, oy, 4.0).is_none());
        let (mx, my) = screen_of(&c, &t, "Mc");
        assert_eq!(hit_test(&c, &t, mx, my, 4.0), None);
    }

    #[test]
    fn drag_moves_point_and_dependents() {
        let mut c = triangle();
        let t = ViewTransform::fit(&c, 800.0, 600.0);
        let mc = c.id("Mc").unwrap();
        let before = c.value(mc);
        let (ax, ay) = screen_of(&c, &t, "A");
        let drag = Drag::begin(&c, &t, ax + 2.0, ay + 1.0, 10.0).expect("grab A");
        assert_eq!(Some(drag.id()), c.id("A"));
        let recomputed = drag
            .apply(&mut c, &t, ax + 2.0 - 60.0, ay + 1.0 - 40.0)
            .unwrap();
        assert!(recomputed > 0);
        // The point keeps its offset from the cursor: it moved by the cursor delta.
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
    fn drag_does_not_start_on_empty_space_or_dependent_point() {
        let c = triangle();
        let t = ViewTransform::fit(&c, 800.0, 600.0);
        let (ox, oy) = screen_of(&c, &t, "O");
        assert!(Drag::begin(&c, &t, ox, oy, 4.0).is_none());
    }
}
