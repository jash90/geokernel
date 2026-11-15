//! Geometry of the selection highlight, with no UI types.
//!
//! Kept apart from `draw.rs`: it is derived from a [`Value`] and the current
//! transform only.

use crate::interact::ViewTransform;
use geo_kernel::kernel::Value;

/// A highlight in screen coordinates (pixels, origin at the drawing area's
/// top-left corner).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Highlight {
    Point {
        x: f64,
        y: f64,
    },
    /// A straight piece of the plane: a segment, or a line clipped far
    /// outside the area.
    Stroke {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
    Circle {
        x: f64,
        y: f64,
        r: f64,
    },
}

/// The highlight for `value`, or `None` for kinds without one (undefined
/// objects, numbers, kinds this module does not know about).
///
/// `width` and `height` are the drawing area size in pixels; a line is
/// extended past its edges. Line coefficients need not be normalised.
pub fn of_value(value: &Value, t: &ViewTransform, width: f64, height: f64) -> Option<Highlight> {
    let finite = |vs: &[f64]| vs.iter().all(|v| v.is_finite());
    match *value {
        Value::Point { x, y } if finite(&[x, y]) => {
            let (x, y) = t.world_to_screen(x, y);
            Some(Highlight::Point { x, y })
        }
        Value::Segment { x1, y1, x2, y2 } if finite(&[x1, y1, x2, y2]) => {
            let (x1, y1) = t.world_to_screen(x1, y1);
            let (x2, y2) = t.world_to_screen(x2, y2);
            Some(Highlight::Stroke { x1, y1, x2, y2 })
        }
        Value::Circle { cx, cy, r } if finite(&[cx, cy, r]) => {
            let (x, y) = t.world_to_screen(cx, cy);
            Some(Highlight::Circle {
                x,
                y,
                r: r.abs() * t.scale(),
            })
        }
        Value::Line { a, b, c } if finite(&[a, b, c]) => {
            let n = a.hypot(b);
            if n == 0.0 {
                return None;
            }
            let (a, b, c) = (a / n, b / n, c / n);
            // Foot of the perpendicular from the area centre, then walk
            // along the direction (b, -a) far enough to leave the area.
            let (mx, my) = t.screen_to_world(0.5 * width, 0.5 * height);
            let d = a * mx + b * my + c;
            let (fx, fy) = (mx - a * d, my - b * d);
            let reach = (width.hypot(height) + d.abs() * t.scale()) / t.scale();
            let (x1, y1) = t.world_to_screen(fx - reach * b, fy + reach * a);
            let (x2, y2) = t.world_to_screen(fx + reach * b, fy - reach * a);
            Some(Highlight::Stroke { x1, y1, x2, y2 })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::Camera;

    fn t() -> ViewTransform {
        Camera {
            cx: 0.0,
            cy: 0.0,
            scale: 10.0,
        }
        .transform(200.0, 100.0)
    }

    #[test]
    fn point_maps_to_screen() {
        let h = of_value(&Value::Point { x: 1.0, y: 2.0 }, &t(), 200.0, 100.0);
        assert_eq!(h, Some(Highlight::Point { x: 110.0, y: 30.0 }));
    }

    #[test]
    fn circle_radius_is_scaled() {
        let h = of_value(
            &Value::Circle {
                cx: 0.0,
                cy: 0.0,
                r: 3.0,
            },
            &t(),
            200.0,
            100.0,
        );
        assert_eq!(
            h,
            Some(Highlight::Circle {
                x: 100.0,
                y: 50.0,
                r: 30.0
            })
        );
    }

    #[test]
    fn line_stroke_lies_on_the_line_and_leaves_the_area() {
        // x = 2, with a deliberately unnormalised coefficient pair.
        let v = Value::Line {
            a: 4.0,
            b: 0.0,
            c: -8.0,
        };
        let Some(Highlight::Stroke { x1, y1, x2, y2 }) = of_value(&v, &t(), 200.0, 100.0) else {
            panic!("expected a stroke");
        };
        // World x = 2 is screen x = 120 for both ends.
        assert!((x1 - 120.0).abs() < 1e-9 && (x2 - 120.0).abs() < 1e-9);
        let (lo, hi) = (y1.min(y2), y1.max(y2));
        assert!(lo < 0.0 && hi > 100.0);
    }

    #[test]
    fn line_far_from_the_centre_still_crosses_the_area() {
        let v = Value::Line {
            a: 0.0,
            b: 1.0,
            c: -400.0,
        }; // y = 400, off screen
        let Some(Highlight::Stroke { x1, x2, .. }) = of_value(&v, &t(), 200.0, 100.0) else {
            panic!("expected a stroke");
        };
        assert!(x1.min(x2) < 0.0 && x1.max(x2) > 200.0);
    }

    #[test]
    fn nothing_for_undefined_numbers_and_bad_lines() {
        let t = t();
        assert_eq!(of_value(&Value::Undefined, &t, 200.0, 100.0), None);
        assert_eq!(of_value(&Value::Number(1.0), &t, 200.0, 100.0), None);
        assert_eq!(
            of_value(
                &Value::Line {
                    a: 0.0,
                    b: 0.0,
                    c: 1.0
                },
                &t,
                200.0,
                100.0
            ),
            None
        );
        assert_eq!(
            of_value(
                &Value::Point {
                    x: f64::NAN,
                    y: 0.0
                },
                &t,
                200.0,
                100.0
            ),
            None
        );
    }
}
