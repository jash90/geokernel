//! Pan, zoom and fit: the camera over the plane, with no UI types.
//!
//! A [`Camera`] is a world-space centre and one scale per axis, in pixels
//! per world unit, as the reference app's view (`EuclidianView.xscale` and
//! `.yscale`): the scales come apart in an axis rescale, a zoom rectangle
//! or a zoom to fit, and every world/pixel conversion uses both. It is
//! turned into an [`crate::interact::ViewTransform`] for a given drawing
//! area size with [`Camera::transform`].
//!
//! Parts of `fit` and `zoom_at` are translated from GeoGebra (EUPL 1.2),
//! commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//! `shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianView.java`
//! (`calculateRectangleOfAllObjects`, `setRealWorldCoordSystemVisible`,
//! `CoordSystemAnimation`), see `camera.rs` for the rest.

use crate::interact::ViewTransform;
use geo_kernel::kernel::text::TextPosition;
use geo_kernel::kernel::{Construction, Id, Value};
use geo_kernel::svg::View;

/// The smallest and largest scale the view allows, in pixels per world
/// unit. The reference app allows 1e-15 .. 1e15 (`setCoordSystem` with
/// `Kernel.MAX_DOUBLE_PRECISION`), far past the precision the drawing
/// works at; ours keeps its own bounds (the owner's choice, spec D15).
/// Which one applies is an owner's rule (`OwnerRules::scale_limits`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaleLimits {
    pub min: f64,
    pub max: f64,
}

impl ScaleLimits {
    /// The owner's bounds (spec D15).
    pub const OURS: ScaleLimits = ScaleLimits {
        min: 1e-6,
        max: 1e9,
    };
    /// The reference app's bounds.
    pub const REFERENCE: ScaleLimits = ScaleLimits {
        min: 1e-15,
        max: 1e15,
    };

    #[inline]
    pub(crate) fn clamp(self, scale: f64) -> f64 {
        scale.clamp(self.min, self.max)
    }
}

/// Scale of an empty construction (`EuclidianView.SCALE_STANDARD`).
pub const STANDARD_SCALE: f64 = 50.0;
/// List entries available to both camera fitting and painting in one desktop frame.
pub const MAX_DESKTOP_FRAME_LIST_ITEMS: usize = 512;
/// The fraction of the content's extent that "fit" keeps free on every
/// side (`calculateRectangleOfAllObjects`: `xGap = (x1 - x0) * 0.03`).
const FIT_MARGIN: f64 = 0.03;
/// The radius of the rectangle a point covers on the screen
/// (`DrawPoint.getBounds`: the selection circle's box, at least the mouse
/// capturing threshold 3 plus `SELECTION_RADIUS_MIN` 12).
const POINT_BOUNDS_RADIUS: f64 = 15.0;

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
    visible_content_bounds(c, |_| true)
}

/// Content bounds for objects enabled in the graphics view.
pub fn visible_content_bounds(c: &Construction, visible: impl Fn(Id) -> bool) -> Option<Bounds> {
    visible_content_bounds_with_list_budget(c, visible, &mut {
        geo_kernel::expr::MAX_FRAME_CANVAS_LIST_ITEMS
    })
}

/// Content bounds that charge list visits to the same allowance as drawing.
pub fn visible_content_bounds_with_list_budget(
    c: &Construction,
    visible: impl Fn(Id) -> bool,
    list_items_left: &mut usize,
) -> Option<Bounds> {
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
    for id in c.ids().filter(|&id| visible(id)) {
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
            Value::PenStroke { .. } => {
                if let Some([xmin, xmax, ymin, ymax]) = c.stroke(id).and_then(|s| s.bounds()) {
                    add(&mut b, xmin, ymin);
                    add(&mut b, xmax, ymax);
                }
            }
            Value::Image => {
                if let Some(points) = c
                    .image(id)
                    .filter(|image| !image.placement.is_screen())
                    .and_then(|_| c.image_quad(id, STANDARD_SCALE))
                {
                    for [x, y] in points {
                        add(&mut b, x, y);
                    }
                }
            }
            Value::List { .. } if *list_items_left > 0 => {
                if let Some(list) = c.elements(id) {
                    list.canvas_members(list_items_left, |object, value| {
                        if object.is_none() {
                            if let Value::Point { x, y } = value {
                                add(&mut b, x, y);
                            }
                        }
                        std::ops::ControlFlow::Continue(())
                    });
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
    for (_, chart) in c.charts().filter(|(id, _)| visible(*id)) {
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
    /// Pixels per world unit, on each axis (`EuclidianView.xscale`,
    /// `.yscale`).
    pub xscale: f64,
    pub yscale: f64,
}

impl Camera {
    /// The standard view: origin in the middle, 50 pixels per unit on both
    /// axes (`setStandardView`).
    pub const STANDARD: Camera = Camera {
        cx: 0.0,
        cy: 0.0,
        xscale: STANDARD_SCALE,
        yscale: STANDARD_SCALE,
    };

    /// A camera of one scale on both axes.
    pub const fn uniform(cx: f64, cy: f64, scale: f64) -> Camera {
        Camera {
            cx,
            cy,
            xscale: scale,
            yscale: scale,
        }
    }

    /// Recenter a file's saved pixel origin at the actual canvas size.
    pub fn from_file(view: crate::workbench::FileView, limits: ScaleLimits) -> Option<Camera> {
        let cx = (f64::from(view.size[0]) / 2.0 - view.origin[0]) / view.scale[0];
        let cy = (view.origin[1] - f64::from(view.size[1]) / 2.0) / view.scale[1];
        (cx.is_finite() && cy.is_finite()).then_some(Camera {
            cx,
            cy,
            xscale: limits.clamp(view.scale[0]),
            yscale: limits.clamp(view.scale[1]),
        })
    }

    /// The view a document opens with: the standard view when all of the
    /// content fits into it, otherwise fitted to the content.
    pub fn initial(bounds: Option<Bounds>, width: f64, height: f64, limits: ScaleLimits) -> Camera {
        let std = Camera::STANDARD;
        let Some(b) = bounds else { return std };
        let v = std.transform(width, height).view();
        if b.xmin >= v.xmin && b.xmax <= v.xmax && b.ymin >= v.ymin && b.ymax <= v.ymax {
            std
        } else {
            Camera::fit(bounds, width, height, limits)
        }
    }

    /// Frames `bounds` in a `width` x `height` area with one scale on both
    /// axes: a document opens undistorted, as the reference app opens it
    /// (a circle stays a circle; only an explicit zoom to fit
    /// [`Camera::fit_drawn`] scales the axes apart,
    /// `App.setViewShowAllObjects` with the ratio not kept). `None`, an
    /// empty box and a single point get a fixed scale around the box
    /// centre.
    pub fn fit(bounds: Option<Bounds>, width: f64, height: f64, limits: ScaleLimits) -> Camera {
        let Some(b) = bounds else {
            return Camera::STANDARD;
        };
        let (w, h) = (b.xmax - b.xmin, b.ymax - b.ymin);
        // The same free space on every side as the reference app's fit
        // (`FIT_MARGIN`, half of it on each side of the extent); the scale
        // that frames both axes, so nothing is cut off and the ratio of
        // the drawing stands.
        let sx = width.max(1.0) / (w * (1.0 + 2.0 * FIT_MARGIN));
        let sy = height.max(1.0) / (h * (1.0 + 2.0 * FIT_MARGIN));
        // A zero extent gives an infinite ratio, which `min` discards;
        // both zero is a single point at the standard scale.
        let scale = if w > 0.0 && h > 0.0 {
            sx.min(sy)
        } else if w > 0.0 {
            sx
        } else if h > 0.0 {
            sy
        } else {
            STANDARD_SCALE
        };
        let scale = if scale.is_finite() {
            limits.clamp(scale)
        } else {
            STANDARD_SCALE
        };
        Camera::uniform(0.5 * (b.xmin + b.xmax), 0.5 * (b.ymin + b.ymax), scale)
    }

    /// Zoom to fit the way the reference app does
    /// (`App.setViewShowAllObjects` with the ratio not kept): the bounds
    /// of the drawn shapes — a point by its selection circle's box, a
    /// segment by its ends, a circle by its box — as the reference measures
    /// them, in whole pixels of the view `t` now, then 3 per cent of the
    /// extent free on every side (`calculateRectangleOfAllObjects`). The
    /// new view shows exactly that rectangle (`setRealWorldCoordSystemVisible`).
    /// `None` when nothing is bounded: the view does not change.
    pub fn fit_drawn(
        c: &Construction,
        t: &ViewTransform,
        width: f64,
        height: f64,
        limits: ScaleLimits,
    ) -> Option<Camera> {
        let mut b = Bounds {
            xmin: f64::INFINITY,
            ymin: f64::INFINITY,
            xmax: f64::NEG_INFINITY,
            ymax: f64::NEG_INFINITY,
        };
        let mut any = false;
        let grow = |b: &mut Bounds, x0: f64, y0: f64, x1: f64, y1: f64| {
            b.grow(t.screen_to_world(x0, 0.0).0, t.screen_to_world(0.0, y0).1);
            b.grow(t.screen_to_world(x1, 0.0).0, t.screen_to_world(0.0, y1).1);
        };
        for id in c.ids() {
            match c.value(id) {
                // `DrawPoint.getBounds`: a whole-pixel rectangle of the
                // point's screen position (the reference truncates the
                // position to an int) with the selection radius on every
                // side.
                Value::Point { x, y } if x.is_finite() && y.is_finite() => {
                    let (sx, sy) = t.world_to_screen(x, y);
                    let (sx, sy) = (sx.trunc(), sy.trunc());
                    let r = POINT_BOUNDS_RADIUS;
                    grow(&mut b, sx - r, sy - r, sx + r, sy + r);
                    any = true;
                }
                // `DrawSegment.getBounds`: the ends, no threshold.
                Value::Segment { x1, y1, x2, y2 } => {
                    let (ax, ay) = t.world_to_screen(x1, y1);
                    let (bx, by) = t.world_to_screen(x2, y2);
                    grow(&mut b, ax.min(bx), ay.min(by), ax.max(bx), ay.max(by));
                    any = true;
                }
                // `DrawConic.getBounds`: the box the ellipse fills.
                Value::Circle { cx, cy, r } if r.is_finite() => {
                    let (x0, y0) = t.world_to_screen(cx - r, cy - r);
                    let (x1, y1) = t.world_to_screen(cx + r, cy + r);
                    grow(&mut b, x0, y0, x1, y1);
                    any = true;
                }
                _ => {}
            }
        }
        if !any {
            return None;
        }
        let gap = |lo: &mut f64, hi: &mut f64| {
            let g = (*hi - *lo) * FIT_MARGIN;
            *lo -= g;
            *hi += g;
        };
        gap(&mut b.xmin, &mut b.xmax);
        gap(&mut b.ymin, &mut b.ymax);
        Some(Camera::showing(b, width, height, limits))
    }

    /// The camera that shows exactly `b` in a `width` x `height` area
    /// (`setRealWorldCoordSystemVisible`: each axis its own scale, the
    /// bounds clamped).
    pub fn showing(b: Bounds, width: f64, height: f64, limits: ScaleLimits) -> Camera {
        let (w, h) = (b.xmax - b.xmin, b.ymax - b.ymin);
        if !(w.is_finite() && w > 0.0 && h.is_finite() && h > 0.0) {
            return Camera::fit(None, width, height, limits);
        }
        Camera {
            cx: 0.5 * (b.xmin + b.xmax),
            cy: 0.5 * (b.ymin + b.ymax),
            xscale: limits.clamp(width / w),
            yscale: limits.clamp(height / h),
        }
    }

    /// Whether this is the standard view (`isStandardView` /
    /// `isZeroStandard`): the origin at the area's centre — which a camera
    /// of a world-space centre says as the centre being the origin — and
    /// both scales 50.
    pub fn is_standard(self) -> bool {
        self.cx.abs() < 1e-9
            && self.cy.abs() < 1e-9
            && (self.xscale - STANDARD_SCALE).abs() < 1e-9
            && (self.yscale - STANDARD_SCALE).abs() < 1e-9
    }

    #[inline]
    pub fn transform(&self, width: f64, height: f64) -> ViewTransform {
        ViewTransform::with_scales(
            View {
                xmin: self.cx - 0.5 * width / self.xscale,
                ymin: self.cy - 0.5 * height / self.yscale,
                xmax: self.cx + 0.5 * width / self.xscale,
                ymax: self.cy + 0.5 * height / self.yscale,
            },
            self.xscale,
            self.yscale,
            height,
        )
    }

    /// Moves the content by `(dx, dy)` screen pixels (y points down).
    #[inline]
    pub fn pan_pixels(&mut self, dx: f64, dy: f64) {
        let (cx, cy) = (self.cx - dx / self.xscale, self.cy + dy / self.yscale);
        if cx.is_finite() && cy.is_finite() {
            (self.cx, self.cy) = (cx, cy);
        }
    }

    /// Multiplies both scales by `factor`, keeping the world point under
    /// the screen position `(sx, sy)` fixed (`CoordSystemAnimation` in its
    /// ZOOM mode: both axes by the same factor, so a scale ratio survives
    /// a zoom). The area is `width` x `height`. Non-finite or non-positive
    /// factors are ignored.
    #[inline]
    pub fn zoom_at(
        &mut self,
        factor: f64,
        sx: f64,
        sy: f64,
        width: f64,
        height: f64,
        limits: ScaleLimits,
    ) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        let nx = limits.clamp(self.xscale * factor);
        let ny = limits.clamp(self.yscale * factor);
        // Offset of the cursor from the area centre, in pixels, is unchanged.
        let (ox, oy) = (sx - 0.5 * width, sy - 0.5 * height);
        let (mut wx, mut wy) = self.transform(width, height).screen_to_world(sx, sy);
        if !(wx.is_finite() && wy.is_finite()) {
            // A view whose extent rounds away (far out and zoomed far in)
            // has no usable transform; the point under the cursor follows
            // from the camera alone.
            (wx, wy) = (self.cx + ox / self.xscale, self.cy - oy / self.yscale);
        }
        let (cx, cy) = (wx - ox / nx, wy + oy / ny);
        // Never a camera that is not finite: it stays as it was.
        if cx.is_finite() && cy.is_finite() {
            *self = Camera {
                cx,
                cy,
                xscale: nx,
                yscale: ny,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zoom_or_pan_far_out_keeps_the_camera_finite() {
        for (c, scale) in [(1e10, 1e9), (1e16, 1e3), (1e300, 1e-3)] {
            let mut cam = Camera::uniform(c, c, scale);
            for (factor, sx, sy) in [(1.25, 10.0, 590.0), (0.8, 400.0, 300.0), (1.1, 799.0, 1.0)] {
                cam.zoom_at(factor, sx, sy, 800.0, 600.0, ScaleLimits::OURS);
                assert!(
                    cam.cx.is_finite()
                        && cam.cy.is_finite()
                        && cam.xscale.is_finite()
                        && cam.yscale.is_finite(),
                    "{cam:?}"
                );
            }
            cam.pan_pixels(1e300, -1e300);
            assert!(cam.cx.is_finite() && cam.cy.is_finite(), "{cam:?}");
        }
        // Far out and zoomed in, a zoom out still works.
        let mut cam = Camera::uniform(1e10, 1e10, 1e9);
        cam.zoom_at(0.5, 10.0, 10.0, 800.0, 600.0, ScaleLimits::OURS);
        assert_eq!(cam.xscale, 5e8);
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
            Camera::initial(Some(small), 1000.0, 800.0, ScaleLimits::OURS),
            Camera::STANDARD
        );
        assert_eq!(
            Camera::initial(None, 1000.0, 800.0, ScaleLimits::OURS),
            Camera::STANDARD
        );
        let big = Bounds {
            xmin: -50.0,
            ymin: -4.0,
            xmax: 50.0,
            ymax: 4.0,
        };
        let cam = Camera::initial(Some(big), 1000.0, 800.0, ScaleLimits::OURS);
        assert_ne!(cam, Camera::STANDARD);
        assert!(cam.xscale < 10.0);
    }

    #[test]
    fn transform_has_the_camera_scales_and_centre() {
        let cam = Camera {
            cx: 3.0,
            cy: -2.0,
            xscale: 40.0,
            yscale: 25.0,
        };
        let t = cam.transform(800.0, 600.0);
        assert!(close(t.xscale(), 40.0) && close(t.yscale(), 25.0));
        let (sx, sy) = t.world_to_screen(3.0, -2.0);
        assert!(close(sx, 400.0) && close(sy, 300.0));
    }

    #[test]
    fn pan_moves_content_by_the_pixel_delta() {
        let mut cam = Camera::uniform(0.0, 0.0, 25.0);
        let before = cam.transform(400.0, 300.0).world_to_screen(1.0, 2.0);
        cam.pan_pixels(30.0, -12.0);
        let after = cam.transform(400.0, 300.0).world_to_screen(1.0, 2.0);
        assert!(close(after.0 - before.0, 30.0) && close(after.1 - before.1, -12.0));
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor_on_both_axes() {
        let mut cam = Camera {
            cx: 1.0,
            cy: 1.0,
            xscale: 30.0,
            yscale: 20.0,
        };
        let (sx, sy) = (123.0, 77.0);
        let (wx, wy) = cam.transform(500.0, 400.0).screen_to_world(sx, sy);
        for factor in [1.5, 0.3, 7.0] {
            cam.zoom_at(factor, sx, sy, 500.0, 400.0, ScaleLimits::OURS);
            let (rx, ry) = cam.transform(500.0, 400.0).screen_to_world(sx, sy);
            assert!(close(rx, wx) && close(ry, wy), "factor {factor}");
        }
    }

    #[test]
    fn zoom_scales_and_clamps() {
        let mut cam = Camera::uniform(0.0, 0.0, 10.0);
        cam.zoom_at(2.0, 0.0, 0.0, 100.0, 100.0, ScaleLimits::OURS);
        assert!(close(cam.xscale, 20.0) && close(cam.yscale, 20.0));
        cam.zoom_at(1e30, 50.0, 50.0, 100.0, 100.0, ScaleLimits::OURS);
        assert_eq!(
            (cam.xscale, cam.yscale),
            (ScaleLimits::OURS.max, ScaleLimits::OURS.max)
        );
        cam.zoom_at(1e-30, 50.0, 50.0, 100.0, 100.0, ScaleLimits::OURS);
        assert_eq!(
            (cam.xscale, cam.yscale),
            (ScaleLimits::OURS.min, ScaleLimits::OURS.min)
        );
        let kept = cam;
        cam.zoom_at(f64::NAN, 0.0, 0.0, 100.0, 100.0, ScaleLimits::OURS);
        cam.zoom_at(-1.0, 0.0, 0.0, 100.0, 100.0, ScaleLimits::OURS);
        assert_eq!(cam, kept);
    }

    /// A zoom keeps the ratio of the scales apart (`CoordSystemAnimation`:
    /// `newScale * view.getScaleRatio()`).
    #[test]
    fn a_zoom_keeps_the_scale_ratio() {
        let mut cam = Camera {
            cx: 0.0,
            cy: 0.0,
            xscale: 60.0,
            yscale: 30.0,
        };
        cam.zoom_at(1.5, 200.0, 150.0, 400.0, 300.0, ScaleLimits::OURS);
        assert!(close(cam.xscale, 90.0) && close(cam.yscale, 45.0));
    }

    #[test]
    fn closing_camera_bounds_charge_only_the_available_list_members() {
        let c = geo_kernel::script::load(
            "L0 = Sequence((k, 0), k, 1, 256)\nL1 = Sequence((k, 1), k, 1, 256)\n\
             L2 = Sequence((k, 2), k, 1, 256)",
        )
        .unwrap();
        let mut remaining = MAX_DESKTOP_FRAME_LIST_ITEMS;
        let bounds = visible_content_bounds_with_list_budget(&c, |_| true, &mut remaining).unwrap();
        assert_eq!(bounds.ymax, 1.0);
        assert_eq!(remaining, 0);
        assert_eq!(
            visible_content_bounds_with_list_budget(&c, |_| true, &mut remaining),
            None
        );
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
        let cam = Camera::fit(b, 640.0, 480.0, ScaleLimits::OURS);
        let t = cam.transform(640.0, 480.0);
        let b = b.unwrap();
        for (x, y) in [(b.xmin, b.ymin), (b.xmax, b.ymax)] {
            let (sx, sy) = t.world_to_screen(x, y);
            assert!((0.0..=640.0).contains(&sx) && (0.0..=480.0).contains(&sy));
        }
    }

    #[test]
    fn fit_degenerate_inputs_stay_finite() {
        let cam = Camera::fit(None, 100.0, 100.0, ScaleLimits::OURS);
        assert_eq!((cam.xscale, cam.yscale), (STANDARD_SCALE, STANDARD_SCALE));
        let one = Bounds {
            xmin: 2.0,
            ymin: 3.0,
            xmax: 2.0,
            ymax: 3.0,
        };
        let cam = Camera::fit(Some(one), 100.0, 100.0, ScaleLimits::OURS);
        assert_eq!(
            (cam.cx, cam.cy, cam.xscale, cam.yscale),
            (2.0, 3.0, STANDARD_SCALE, STANDARD_SCALE)
        );
        let flat = Bounds {
            xmin: 0.0,
            ymin: 1.0,
            xmax: 10.0,
            ymax: 1.0,
        };
        let cam = Camera::fit(Some(flat), 200.0, 100.0, ScaleLimits::OURS);
        // 3 per cent free on every side: 200 px over 10.6 world units, on
        // both axes (a flat extent leaves the other axis the same scale).
        assert!(close(cam.xscale, 200.0 / 10.6) && close(cam.yscale, cam.xscale));
        let cam = Camera::fit(Some(flat), 0.0, 0.0, ScaleLimits::OURS);
        assert!(cam.xscale.is_finite() && cam.xscale > 0.0);
    }

    /// A document opens undistorted: the fit at load keeps one scale on
    /// both axes however wide the content is, so a circle stays a circle
    /// (`Camera::fit_drawn`, the zoom to fit, is the one that scales the
    /// axes apart; `App.setViewShowAllObjects` with the ratio kept).
    #[test]
    fn the_fit_at_load_keeps_the_ratio() {
        let wide = Bounds {
            xmin: -10.0,
            ymin: -1.0,
            xmax: 10.0,
            ymax: 1.0,
        };
        let cam = Camera::fit(Some(wide), 800.0, 600.0, ScaleLimits::OURS);
        assert_eq!(cam.xscale, cam.yscale);
        assert!(close(cam.xscale, 800.0 / (20.0 * 1.06)), "{}", cam.xscale);
        let tall = Bounds {
            xmin: -1.0,
            ymin: -10.0,
            xmax: 1.0,
            ymax: 10.0,
        };
        let cam = Camera::fit(Some(tall), 800.0, 600.0, ScaleLimits::OURS);
        assert_eq!(cam.xscale, cam.yscale);
        assert!(close(cam.yscale, 600.0 / (20.0 * 1.06)), "{}", cam.yscale);
        // A zoom to fit still scales the axes apart (no ratio kept there).
        let mut c = Construction::new();
        geo_kernel::script::exec(&mut c, "A = (1, 1)").unwrap();
        geo_kernel::script::exec(&mut c, "B = (5, 2)").unwrap();
        let t = Camera::STANDARD.transform(892.0, 747.0);
        let cam = Camera::fit_drawn(&c, &t, 892.0, 747.0, ScaleLimits::OURS).unwrap();
        assert_ne!(cam.xscale, cam.yscale);
    }

    /// Zoom to fit as the reference app measures it
    /// (`calculateRectangleOfAllObjects` over `DrawPoint.getBounds`): the
    /// numbers of the observation B2f (points (1, 1) and (5, 2) in an
    /// 892 x 747 view with the origin at (446, 373.5), scale 50).
    #[test]
    fn fit_drawn_takes_whole_pixel_point_boxes_and_three_percent() {
        let mut c = Construction::new();
        geo_kernel::script::exec(&mut c, "A = (1, 1)").unwrap();
        geo_kernel::script::exec(&mut c, "B = (5, 2)").unwrap();
        let t = Camera::STANDARD.transform(892.0, 747.0);
        let cam = Camera::fit_drawn(&c, &t, 892.0, 747.0, ScaleLimits::OURS).unwrap();
        let v = cam.transform(892.0, 747.0).view();
        // The point boxes: floor(496) +- 15 = [481, 511] px in x, and in y
        // floor(323.5) = 323 -> [308, 338], floor(273.5) = 273 -> [258,
        // 288], whose world [0.71, 2.31] with the 3 per cent gap each side
        // is what the view shows; x has no half pixel and stays [0.7, 5.3].
        assert!(close(v.xmin, 0.562), "{}", v.xmin);
        assert!(close(v.xmax, 5.438), "{}", v.xmax);
        assert!(close(v.ymin, 0.662), "{}", v.ymin);
        assert!(close(v.ymax, 2.358), "{}", v.ymax);
        assert!(!cam.is_standard());
    }

    #[test]
    fn file_camera_clamps_both_axes_to_the_configured_limits() {
        let view = crate::workbench::FileView {
            size: [800, 600],
            origin: [400.0, 300.0],
            scale: [1e-100, 1e100],
            ..crate::workbench::FileView::default()
        };
        let limits = ScaleLimits::OURS;
        let camera = Camera::from_file(view, limits).unwrap();
        assert_eq!([camera.xscale, camera.yscale], [limits.min, limits.max]);
        assert_eq!([camera.cx, camera.cy], [0.0, 0.0]);
    }

    #[test]
    fn fit_drawn_of_an_empty_drawing_changes_nothing() {
        let t = Camera::STANDARD.transform(892.0, 747.0);
        assert_eq!(
            Camera::fit_drawn(&Construction::new(), &t, 892.0, 747.0, ScaleLimits::OURS),
            None
        );
    }

    /// The bounds of the scale are the owner's rule (`ScaleLimits`): ours
    /// stops at 1e-6 .. 1e9, the reference app at 1e-15 .. 1e15.
    #[test]
    fn showing_clamps_to_the_scale_bounds() {
        let far = |xmax: f64| Bounds {
            xmin: 0.0,
            ymin: 0.0,
            xmax,
            ymax: 1.0,
        };
        // 100 px over 1e15 world units is 1e-13: inside the reference's
        // bounds, past ours; over 1e18 units it is past both.
        let cam = Camera::showing(far(1e15), 100.0, 100.0, ScaleLimits::OURS);
        assert_eq!(cam.xscale, ScaleLimits::OURS.min);
        let cam = Camera::showing(far(1e15), 100.0, 100.0, ScaleLimits::REFERENCE);
        assert_eq!(cam.xscale, 1e-13);
        let cam = Camera::showing(far(1e18), 100.0, 100.0, ScaleLimits::REFERENCE);
        assert_eq!(cam.xscale, ScaleLimits::REFERENCE.min);
        // A zoom in past the owner's bound stops there under ours and goes
        // on under the reference's.
        let mut ours = Camera::uniform(0.0, 0.0, ScaleLimits::OURS.max);
        ours.zoom_at(8.0, 50.0, 50.0, 100.0, 100.0, ScaleLimits::OURS);
        assert_eq!(ours.xscale, ScaleLimits::OURS.max);
        let mut reference = Camera::uniform(0.0, 0.0, ScaleLimits::OURS.max);
        reference.zoom_at(8.0, 50.0, 50.0, 100.0, 100.0, ScaleLimits::REFERENCE);
        assert_eq!(reference.xscale, 8e9);
    }

    /// `isStandardView`: the origin at the centre and both scales 50, and
    /// nothing else.
    #[test]
    fn the_standard_view_is_recognised() {
        assert!(Camera::STANDARD.is_standard());
        // One pixel off the centre (0.02 world units at 50 px per unit) is
        // not the standard view any more.
        assert!(!Camera::uniform(0.02, 0.0, 50.0).is_standard());
        assert!(!Camera::uniform(0.0, 0.0, 55.0).is_standard());
        let mut cam = Camera {
            cx: 0.0,
            cy: 0.0,
            xscale: 50.0,
            yscale: 60.0,
        };
        assert!(!cam.is_standard());
        cam.zoom_at(1.0, 0.0, 0.0, 100.0, 100.0, ScaleLimits::OURS);
        assert!(!cam.is_standard());
    }

    /// A resize keeps the world point at the centre (`keepCenter`, spec
    /// D11): the camera names that point itself, so whatever the area size
    /// becomes, the centre of it still shows `cx`/`cy` — was
    /// `resizing_keeps_the_top_left_corner_in_place`.
    #[test]
    fn a_resize_keeps_the_world_point_at_the_centre() {
        let cam = Camera::uniform(1.0, -2.0, 40.0);
        for (w, h) in [(800.0, 600.0), (900.0, 378.0)] {
            let (x, y) = cam.transform(w, h).screen_to_world(0.5 * w, 0.5 * h);
            assert_eq!((x, y), (cam.cx, cam.cy));
        }
    }
}
