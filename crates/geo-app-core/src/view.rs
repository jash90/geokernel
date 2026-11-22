//! Pan, zoom and fit: the camera over the plane, with no UI types.
//!
//! A [`Camera`] is a world-space centre and a scale in pixels per world unit.
//! It is turned into an [`crate::interact::ViewTransform`] for a given drawing
//! area size with [`Camera::transform`].

use crate::interact::ViewTransform;
use geo_kernel::kernel::text::TextPosition;
use geo_kernel::kernel::{Construction, Value};
use geo_kernel::svg::View;

/// Smallest and largest allowed scale, in pixels per world unit.
pub const MIN_SCALE: f64 = 1e-6;
pub const MAX_SCALE: f64 = 1e9;
/// Scale of an empty construction.
const DEFAULT_SCALE: f64 = 50.0;
/// Fraction of the area kept free around the content by "fit".
const FIT_MARGIN: f64 = 0.08;

/// An axis-aligned rectangle of the plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub xmin: f64,
    pub ymin: f64,
    pub xmax: f64,
    pub ymax: f64,
}

impl Bounds {
    fn grow(&mut self, x: f64, y: f64) {
        self.xmin = self.xmin.min(x);
        self.ymin = self.ymin.min(y);
        self.xmax = self.xmax.max(x);
        self.ymax = self.ymax.max(y);
    }
}

/// The bounding box of points, segment ends, circles, loci and world text anchors.
/// Lines are unbounded and are ignored. `None` when nothing is bounded.
pub fn content_bounds(c: &Construction) -> Option<Bounds> {
    let mut b = Bounds {
        xmin: f64::INFINITY,
        ymin: f64::INFINITY,
        xmax: f64::NEG_INFINITY,
        ymax: f64::NEG_INFINITY,
    };
    let mut any = false;
    let mut add = |b: &mut Bounds, x: f64, y: f64| {
        if x.is_finite() && y.is_finite() {
            b.grow(x, y);
            any = true;
        }
    };
    for id in c.ids() {
        match c.value(id) {
            Value::Point { x, y } => add(&mut b, x, y),
            Value::Segment { x1, y1, x2, y2 } => {
                add(&mut b, x1, y1);
                add(&mut b, x2, y2);
            }
            Value::Circle { cx, cy, r } => {
                add(&mut b, cx - r, cy - r);
                add(&mut b, cx + r, cy + r);
            }
            Value::Locus { .. } => {
                if let Some([xmin, xmax, ymin, ymax]) = c.locus(id).and_then(|l| l.bounds()) {
                    add(&mut b, xmin, ymin);
                    add(&mut b, xmax, ymax);
                }
            }
            Value::Image => {
                if let Some(points) = c
                    .image(id)
                    .filter(|image| !image.placement.is_screen())
                    .and_then(|_| c.image_quad(id, DEFAULT_SCALE))
                {
                    for [x, y] in points {
                        add(&mut b, x, y);
                    }
                }
            }
            Value::Text => {
                if let Some(TextPosition::World { x, y }) = c.text_position(id) {
                    add(&mut b, x, y);
                }
            }
            _ => {}
        }
    }
    for (_, chart) in c.charts() {
        if let Some([xmin, ymin, xmax, ymax]) = chart.bounds() {
            add(&mut b, xmin, ymin);
            add(&mut b, xmax, ymax);
        }
    }
    any.then_some(b)
}

/// Which part of the plane is visible, independent of the window size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// World point shown at the centre of the area.
    pub cx: f64,
    pub cy: f64,
    /// Pixels per world unit.
    pub scale: f64,
}

impl Camera {
    /// The standard view: origin in the middle, 50 pixels per unit.
    pub const STANDARD: Camera = Camera {
        cx: 0.0,
        cy: 0.0,
        scale: DEFAULT_SCALE,
    };

    /// The view a document opens with: the standard view when all of the
    /// content fits into it, otherwise fitted to the content.
    pub fn initial(bounds: Option<Bounds>, width: f64, height: f64) -> Camera {
        let std = Camera::STANDARD;
        let Some(b) = bounds else { return std };
        let v = std.transform(width, height).view();
        if b.xmin >= v.xmin && b.xmax <= v.xmax && b.ymin >= v.ymin && b.ymax <= v.ymax {
            std
        } else {
            Camera::fit(Some(b), width, height)
        }
    }

    /// Frames `bounds` in a `width` x `height` area with a margin. `None`, an
    /// empty box and a single point get a fixed scale around the box centre.
    pub fn fit(bounds: Option<Bounds>, width: f64, height: f64) -> Camera {
        let Some(b) = bounds else {
            return Camera {
                cx: 0.0,
                cy: 0.0,
                scale: DEFAULT_SCALE,
            };
        };
        let (w, h) = (b.xmax - b.xmin, b.ymax - b.ymin);
        let usable = 1.0 - 2.0 * FIT_MARGIN;
        let sx = width.max(1.0) * usable / w;
        let sy = height.max(1.0) * usable / h;
        // A zero extent gives an infinite ratio, which `min` discards.
        let scale = sx.min(sy);
        let scale = if scale.is_finite() {
            scale.clamp(MIN_SCALE, MAX_SCALE)
        } else {
            DEFAULT_SCALE
        };
        Camera {
            cx: 0.5 * (b.xmin + b.xmax),
            cy: 0.5 * (b.ymin + b.ymax),
            scale,
        }
    }

    /// The transform for an area of `width` x `height` pixels.
    /// The camera after the area changed from `old` to `new` (width,
    /// height in pixels), keeping the world point at the top left corner
    /// where it is: opening the keyboard or resizing the window does not
    /// move the drawing under the pointer.
    pub fn resized(self, old: (f64, f64), new: (f64, f64)) -> Camera {
        Camera {
            cx: self.cx + 0.5 * (new.0 - old.0) / self.scale,
            cy: self.cy - 0.5 * (new.1 - old.1) / self.scale,
            ..self
        }
    }

    #[inline]
    pub fn transform(&self, width: f64, height: f64) -> ViewTransform {
        let hw = 0.5 * width / self.scale;
        let hh = 0.5 * height / self.scale;
        ViewTransform::new(
            View {
                xmin: self.cx - hw,
                ymin: self.cy - hh,
                xmax: self.cx + hw,
                ymax: self.cy + hh,
            },
            width,
            height,
        )
    }

    /// Moves the content by `(dx, dy)` screen pixels (y points down).
    #[inline]
    pub fn pan_pixels(&mut self, dx: f64, dy: f64) {
        let (cx, cy) = (self.cx - dx / self.scale, self.cy + dy / self.scale);
        if cx.is_finite() && cy.is_finite() {
            (self.cx, self.cy) = (cx, cy);
        }
    }

    /// Multiplies the scale by `factor`, keeping the world point under the
    /// screen position `(sx, sy)` fixed. The area is `width` x `height`.
    /// Non-finite or non-positive factors are ignored.
    #[inline]
    pub fn zoom_at(&mut self, factor: f64, sx: f64, sy: f64, width: f64, height: f64) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        let new_scale = (self.scale * factor).clamp(MIN_SCALE, MAX_SCALE);
        // Offset of the cursor from the area centre, in pixels, is unchanged.
        let (ox, oy) = (sx - 0.5 * width, sy - 0.5 * height);
        let (mut wx, mut wy) = self.transform(width, height).screen_to_world(sx, sy);
        if !(wx.is_finite() && wy.is_finite()) {
            // A view whose extent rounds away (far out and zoomed far in)
            // has no usable transform; the point under the cursor follows
            // from the camera alone.
            (wx, wy) = (self.cx + ox / self.scale, self.cy - oy / self.scale);
        }
        let (cx, cy) = (wx - ox / new_scale, wy + oy / new_scale);
        // Never a camera that is not finite: it stays as it was.
        if cx.is_finite() && cy.is_finite() {
            *self = Camera {
                cx,
                cy,
                scale: new_scale,
            };
        }
    }
}

/// Scale factor for one frame of wheel scrolling: `scroll_y` is the scrolled
/// distance in points (positive = content moves down), `pinch` is the
/// multiplicative pinch factor of the frame (1 = none).
#[inline]
pub fn zoom_factor(scroll_y: f64, pinch: f64) -> f64 {
    (scroll_y * 0.002).exp() * pinch
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zoom_or_pan_far_out_keeps_the_camera_finite() {
        for (c, scale) in [(1e10, 1e9), (1e16, 1e3), (1e300, 1e-3)] {
            let mut cam = Camera {
                cx: c,
                cy: c,
                scale,
            };
            for (factor, sx, sy) in [(1.25, 10.0, 590.0), (0.8, 400.0, 300.0), (1.1, 799.0, 1.0)] {
                cam.zoom_at(factor, sx, sy, 800.0, 600.0);
                assert!(
                    cam.cx.is_finite() && cam.cy.is_finite() && cam.scale.is_finite(),
                    "{cam:?}"
                );
            }
            cam.pan_pixels(1e300, -1e300);
            assert!(cam.cx.is_finite() && cam.cy.is_finite(), "{cam:?}");
        }
        // Far out and zoomed in, a zoom out still works.
        let mut cam = Camera {
            cx: 1e10,
            cy: 1e10,
            scale: 1e9,
        };
        cam.zoom_at(0.5, 10.0, 10.0, 800.0, 600.0);
        assert_eq!(cam.scale, 5e8);
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9 * (1.0 + a.abs().max(b.abs()))
    }

    fn triangle() -> Construction {
        geo_kernel::script::load(include_str!("../../../examples/triangle.geo")).unwrap()
    }

    #[test]
    fn initial_view_is_standard_when_the_content_fits() {
        let small = Bounds {
            xmin: -5.0,
            ymin: -4.0,
            xmax: 5.0,
            ymax: 4.0,
        };
        assert_eq!(
            Camera::initial(Some(small), 1000.0, 800.0),
            Camera::STANDARD
        );
        assert_eq!(Camera::initial(None, 1000.0, 800.0), Camera::STANDARD);
        let big = Bounds {
            xmin: -50.0,
            ymin: -4.0,
            xmax: 50.0,
            ymax: 4.0,
        };
        let cam = Camera::initial(Some(big), 1000.0, 800.0);
        assert_ne!(cam, Camera::STANDARD);
        assert!(cam.scale < 10.0);
    }

    #[test]
    fn transform_has_the_camera_scale_and_centre() {
        let cam = Camera {
            cx: 3.0,
            cy: -2.0,
            scale: 40.0,
        };
        let t = cam.transform(800.0, 600.0);
        assert!(close(t.scale(), 40.0));
        let (sx, sy) = t.world_to_screen(3.0, -2.0);
        assert!(close(sx, 400.0) && close(sy, 300.0));
    }

    #[test]
    fn pan_moves_content_by_the_pixel_delta() {
        let mut cam = Camera {
            cx: 0.0,
            cy: 0.0,
            scale: 25.0,
        };
        let before = cam.transform(400.0, 300.0).world_to_screen(1.0, 2.0);
        cam.pan_pixels(30.0, -12.0);
        let after = cam.transform(400.0, 300.0).world_to_screen(1.0, 2.0);
        assert!(close(after.0 - before.0, 30.0) && close(after.1 - before.1, -12.0));
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let mut cam = Camera {
            cx: 1.0,
            cy: 1.0,
            scale: 30.0,
        };
        let (sx, sy) = (123.0, 77.0);
        let (wx, wy) = cam.transform(500.0, 400.0).screen_to_world(sx, sy);
        for factor in [1.5, 0.3, 7.0] {
            cam.zoom_at(factor, sx, sy, 500.0, 400.0);
            let (rx, ry) = cam.transform(500.0, 400.0).screen_to_world(sx, sy);
            assert!(close(rx, wx) && close(ry, wy), "factor {factor}");
        }
    }

    #[test]
    fn zoom_scales_and_clamps() {
        let mut cam = Camera {
            cx: 0.0,
            cy: 0.0,
            scale: 10.0,
        };
        cam.zoom_at(2.0, 0.0, 0.0, 100.0, 100.0);
        assert!(close(cam.scale, 20.0));
        cam.zoom_at(1e30, 50.0, 50.0, 100.0, 100.0);
        assert_eq!(cam.scale, MAX_SCALE);
        cam.zoom_at(1e-30, 50.0, 50.0, 100.0, 100.0);
        assert_eq!(cam.scale, MIN_SCALE);
        let kept = cam;
        cam.zoom_at(f64::NAN, 0.0, 0.0, 100.0, 100.0);
        cam.zoom_at(-1.0, 0.0, 0.0, 100.0, 100.0);
        assert_eq!(cam, kept);
    }

    #[test]
    fn wheel_factor_is_neutral_without_input_and_monotone() {
        assert!(close(zoom_factor(0.0, 1.0), 1.0));
        assert!(zoom_factor(10.0, 1.0) > 1.0);
        assert!(zoom_factor(-10.0, 1.0) < 1.0);
        assert!(close(zoom_factor(0.0, 1.25), 1.25));
    }

    #[test]
    fn bounds_cover_points_segments_and_circles() {
        let c = triangle();
        let b = content_bounds(&c).expect("triangle has bounded objects");
        for id in c.ids() {
            if let Value::Point { x, y } = c.value(id) {
                assert!(b.xmin <= x && x <= b.xmax && b.ymin <= y && y <= b.ymax);
            }
        }
    }

    #[test]
    fn bounds_include_circle_extent_and_skip_lines() {
        let mut c = Construction::new();
        geo_kernel::script::exec(&mut c, "O = (0, 0)").unwrap();
        geo_kernel::script::exec(&mut c, "P = (1, 0)").unwrap();
        geo_kernel::script::exec(&mut c, "k = Circle(O, P)").unwrap();
        geo_kernel::script::exec(&mut c, "l = Line(O, P)").unwrap();
        let b = content_bounds(&c).unwrap();
        assert_eq!(
            b,
            Bounds {
                xmin: -1.0,
                ymin: -1.0,
                xmax: 1.0,
                ymax: 1.0
            }
        );
    }

    #[test]
    fn bounds_of_empty_construction_is_none() {
        assert_eq!(content_bounds(&Construction::new()), None);
    }

    #[test]
    fn fit_frames_all_content_inside_the_area() {
        let c = triangle();
        let b = content_bounds(&c);
        let cam = Camera::fit(b, 640.0, 480.0);
        let t = cam.transform(640.0, 480.0);
        let b = b.unwrap();
        for (x, y) in [(b.xmin, b.ymin), (b.xmax, b.ymax)] {
            let (sx, sy) = t.world_to_screen(x, y);
            assert!((0.0..=640.0).contains(&sx) && (0.0..=480.0).contains(&sy));
        }
    }

    #[test]
    fn fit_degenerate_inputs_stay_finite() {
        let cam = Camera::fit(None, 100.0, 100.0);
        assert_eq!(cam.scale, DEFAULT_SCALE);
        let one = Bounds {
            xmin: 2.0,
            ymin: 3.0,
            xmax: 2.0,
            ymax: 3.0,
        };
        let cam = Camera::fit(Some(one), 100.0, 100.0);
        assert_eq!((cam.cx, cam.cy, cam.scale), (2.0, 3.0, DEFAULT_SCALE));
        let flat = Bounds {
            xmin: 0.0,
            ymin: 1.0,
            xmax: 10.0,
            ymax: 1.0,
        };
        let cam = Camera::fit(Some(flat), 200.0, 100.0);
        assert!(close(cam.scale, 200.0 * 0.84 / 10.0));
        let cam = Camera::fit(Some(flat), 0.0, 0.0);
        assert!(cam.scale.is_finite() && cam.scale > 0.0);
    }

    #[test]
    fn resizing_keeps_the_top_left_corner_in_place() {
        let cam = Camera {
            cx: 1.0,
            cy: -2.0,
            scale: 40.0,
        };
        let (old, new) = ((800.0, 600.0), (900.0, 378.0));
        let a = cam.transform(old.0, old.1).screen_to_world(0.0, 0.0);
        let moved = cam.resized(old, new);
        let b = moved.transform(new.0, new.1).screen_to_world(0.0, 0.0);
        assert!(
            (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12,
            "{a:?} {b:?}"
        );
        assert_eq!(moved.scale, cam.scale);
        // A point on screen stays at the same pixel.
        let p = cam.transform(old.0, old.1).world_to_screen(2.0, 1.0);
        let q = moved.transform(new.0, new.1).world_to_screen(2.0, 1.0);
        assert!((p.0 - q.0).abs() < 1e-9 && (p.1 - q.1).abs() < 1e-9);
    }
}
