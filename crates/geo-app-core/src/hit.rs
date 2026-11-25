// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/HitDetector.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/Hits.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianController.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawSlider.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawBoolean.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawLine.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawRay.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawVector.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawConic.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawConicPart.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawLocus.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawPieChart.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawPolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/DefaultGeoPriorityComparator.java
//   source/shared/common/src/main/java/org/geogebra/common/plugin/GeoClass.java
//   source/shared/common/src/main/java/org/geogebra/common/main/App.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Hit testing of the visible objects under a screen position, with no UI
//! types, following the reference app:
//!
//! - every object is tested against a square of half-width `threshold`
//!   around the pointer (`App.getCapturingThreshold`: 3 px for the mouse,
//!   9 px for a finger; a finger that hits nothing tries again at 27 px,
//!   `HitDetector.setHits(GPoint, PointerEventType)`);
//! - a point is hit within `r = max(size, threshold + 12)`
//!   (`DrawPoint.hit`, `SELECTION_RADIUS_MIN`); a line, segment, ray or
//!   vector when its line crosses the square (`Line2D.intersects`); a circle
//!   when its stroked outline crosses the square (`DrawConic.hit`; circles
//!   are not filled, so their inside is never hit). Other curves use the
//!   same stroked-boundary square; a polygon is hit when the
//!   square crosses its outline, or lies inside it (it is filled; for a
//!   filled polygon wider than 20 px the square is the mouse's,
//!   `DrawPolygon.hit`, `getFillingHitThreshold`);
//! - a text is hit within its painted rectangle (`DrawText.hit`), without
//!   a capture threshold.
//! - of the objects hit, one is chosen (`EuclidianController.chooseGeo` on
//!   `Hits.getTopHits` with the polygons removed when anything else is hit,
//!   as `handleMousePressedForMoveMode` does): a single selected object that
//!   is hit stays chosen; points win over everything (the free point made
//!   last, else a single point, else the point made first); then a segment
//!   (the one made first); then the object drawn on top (outlines before
//!   fillings, then the type's drawing priority, then the one made last).
//!
//! Choosing streams over the objects once with a few numbers, so a hit test
//! allocates nothing (the window asks it for the hover every frame).

use crate::interact::ViewTransform;
use crate::metrics::Metrics;
use geo_kernel::algo::{arc::ArcData, vector};
use geo_kernel::kernel::{Construction, Id, Kind, Value};
use geo_kernel::{ConicData, ConicType, LocusPoint};
use std::f64::consts::TAU;

/// The width the window draws curves with (`draw::CURVE_WIDTH` in the
/// window), which is part of a circle's stroked outline.
pub const CURVE_STROKE: f64 = 2.5;

/// `DrawPolygon.MIN_HITTING_SIZE`: a filled polygon wider than this is hit
/// with the mouse's square whatever the pointer.
const MIN_HITTING_SIZE: f64 = 20.0;

/// The owner's choice where the reference app is not followed: a press of
/// the move tool on a side of a polygon takes hold of the polygon. `false`
/// follows the reference app as observed (scenario
/// `press_on_polygon_edge_does_nothing`; the source path was never found):
/// such a press selects nothing, moves nothing and does not pan. The window
/// reads it through `app::OwnerRules`.
pub const SIDE_TAKES_THE_POLYGON: bool = true;

/// How far from the pointer an object is still hit, for one press.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    /// The half-width of the square around the pointer.
    pub threshold: f64,
    /// A second, larger square when the first one hits nothing (a finger).
    pub retry: Option<f64>,
    /// Added to the threshold for the radius of a point.
    pub point_min: f64,
    /// The mouse's threshold (big filled polygons use it for any pointer).
    pub mouse: f64,
}

impl Reach {
    /// The reach of a press by a finger (`touch`) or the mouse, from the
    /// profile's numbers (`App.getCapturingThreshold`).
    pub fn of(m: &Metrics, touch: bool) -> Reach {
        let threshold = if touch {
            m.touch_threshold
        } else {
            m.mouse_threshold
        };
        Reach {
            threshold,
            retry: touch.then_some(threshold * m.touch_retry_factor),
            point_min: m.point_selection_min,
            mouse: m.mouse_threshold,
        }
    }

    /// A square of half-width `threshold`, without a retry.
    pub fn square(threshold: f64) -> Reach {
        Reach {
            threshold,
            retry: None,
            point_min: Metrics::desktop().point_selection_min,
            mouse: threshold,
        }
    }
}

/// How an object is hit (`GeoElement.HitType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HitType {
    /// Its inside (a polygon is always hit this way, `GeoPolygon
    /// .getLastHitType`).
    Filling,
    /// A point, or the outline of a curve.
    Boundary,
    SliderKnob,
}

fn point_hit(s: (f64, f64), p: (f64, f64), radius: f64) -> bool {
    let (dx, dy) = (s.0 - p.0, s.1 - p.1);
    dx.abs() < radius && dx * dx + dy * dy <= radius * radius
}

/// Whether the closed square of half-width `t` around `p` meets the segment
/// `a`-`b` (Liang-Barsky clipping).
fn segment_meets_square(a: (f64, f64), b: (f64, f64), p: (f64, f64), t: f64) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for (q, d) in [
        (a.0 - (p.0 - t), -dx),
        ((p.0 + t) - a.0, dx),
        (a.1 - (p.1 - t), -dy),
        ((p.1 + t) - a.1, dy),
    ] {
        if d == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let r = q / d;
            if d < 0.0 {
                lo = lo.max(r);
            } else {
                hi = hi.min(r);
            }
        }
    }
    lo <= hi
}

/// Whether the infinite line through `a` and `b` meets the square of
/// half-width `t` around `p`: the line's distance to the centre, measured
/// along its normal, is at most the square's extent along it.
fn line_meets_square(a: (f64, f64), b: (f64, f64), p: (f64, f64), t: f64) -> bool {
    let (nx, ny) = (a.1 - b.1, b.0 - a.0);
    if nx == 0.0 && ny == 0.0 {
        return false;
    }
    (nx * (p.0 - a.0) + ny * (p.1 - a.1)).abs() <= t * (nx.abs() + ny.abs())
}

/// How the object `id` is hit by the square of half-width `t` around `p`
/// (`mouse` is the mouse's threshold), or `None`.
pub fn hit(
    c: &Construction,
    id: Id,
    tr: &ViewTransform,
    p: (f64, f64),
    t: f64,
    reach: &Reach,
) -> Option<HitType> {
    hit_with_list_budget(c, id, tr, p, t, reach, &mut {
        geo_kernel::expr::MAX_CANVAS_LIST_ITEMS
    })
}

/// A shape hit sharing the caller's list budget with the other hits in its pass.
pub fn hit_with_list_budget(
    c: &Construction,
    id: Id,
    tr: &ViewTransform,
    p: (f64, f64),
    t: f64,
    reach: &Reach,
    list_items_left: &mut usize,
) -> Option<HitType> {
    let to_screen = |x: f64, y: f64| tr.world_to_screen(x, y);
    let boundary = |hit: bool| hit.then_some(HitType::Boundary);
    match c.value(id) {
        Value::List { .. } => {
            use std::ops::ControlFlow;
            let mut result = None;
            c.elements(id)?.canvas_members(list_items_left, |member| {
                let found = if let Some(object) = member.object {
                    hit(c, object, tr, p, t, reach)
                } else {
                    boundary(match member.value {
                        Value::Vector { x, y } => {
                            segment_meets_square(to_screen(0.0, 0.0), to_screen(x, y), p, t)
                        }
                        Value::Conic { .. } => member
                            .conic(c)
                            .is_some_and(|d| conic_meets_square(&d, tr, p, t + CURVE_STROKE / 2.0)),
                        value => primitive_meets_square(value, tr, p, t, reach),
                    })
                };
                if found.is_some() {
                    result = found;
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            });
            result
        }
        value @ (Value::Point { .. }
        | Value::Line { .. }
        | Value::Segment { .. }
        | Value::Ray { .. }
        | Value::Circle { .. }) => boundary(primitive_meets_square(value, tr, p, t, reach)),
        Value::Vector { x, y } => {
            let (x0, y0) = vector::start(c, id)?;
            boundary(segment_meets_square(
                to_screen(x0, y0),
                to_screen(x0 + x, y0 + y),
                p,
                t,
            ))
        }
        // Degenerate parts may have drawable rays despite an infinite measure.
        Value::Arc { .. } | Value::Sector { .. } | Value::Undefined => {
            let part = c.arc(id)?;
            if part_meets_square(&part, tr, p, t + CURVE_STROKE / 2.0) {
                Some(HitType::Boundary)
            } else {
                (!c.drawing_style(id).sector_outline_only
                    && sector_contains(&part, tr.screen_to_world(p.0, p.1)))
                .then_some(HitType::Filling)
            }
        }
        Value::Conic { .. } => {
            let d = c.conic(id)?;
            boundary(conic_meets_square(&d, tr, p, t + CURVE_STROKE / 2.0))
        }
        Value::ImplicitCurve => {
            let coefficients = c.implicit_coefficients(id)?;
            let t = t + CURVE_STROKE / 2.0;
            let lo = tr.screen_to_world(p.0 - t, p.1 + t);
            let hi = tr.screen_to_world(p.0 + t, p.1 - t);
            boundary(geo_kernel::plot::implicit::meets_box(coefficients, lo, hi))
        }
        Value::Function => boundary(function_meets_square(c, id, tr, p, t + CURVE_STROKE / 2.0)),
        Value::Number(_) if c.kind(id) == Kind::Angle => {
            let angle = c.angle_geometry(id)?;
            let center = to_screen(angle.center.0, angle.center.1);
            angle
                .contains(
                    (p.0 - center.0, center.1 - p.1),
                    angle.radius((tr.xscale() * tr.yscale()).sqrt()),
                )
                .then_some(HitType::Filling)
        }
        Value::Number(_) if c.kind(id) == Kind::Number => {
            let slider =
                c.slider_geometry(id, (tr.xscale(), tr.yscale()), |(x, y)| to_screen(x, y))?;
            if knob_hit(slider, p, t) {
                Some(HitType::SliderKnob)
            } else {
                boundary(slider_track_hit(c, id, slider, p, t))
            }
        }
        Value::Text => {
            let [x0, y0, x1, y1] = c.text_pixel_rect(id, to_screen)?.map(f64::from);
            ((x0..x1).contains(&p.0) && (y0..y1).contains(&p.1)).then_some(HitType::Filling)
        }
        Value::Image => {
            let image = c.image(id)?;
            image.source_rect()?;
            if image.style.background {
                return None;
            }
            let quad = c.image_quad_with_scales(id, tr.xscale(), tr.yscale())?;
            let positions = quad.map(|[x, y]| {
                if image.placement.is_screen() {
                    (x, y)
                } else {
                    to_screen(x, y)
                }
            });
            crate::shape::contains(&positions, p).then_some(HitType::Filling)
        }
        Value::PieChart => {
            let pie = c.chart(id)?.pie.as_ref()?;
            let (x, y) = tr.screen_to_world(p.0, p.1);
            let (cx, cy) = pie.center;
            ((x - cx).hypot(y - cy) <= pie.radius).then_some(HitType::Filling)
        }
        Value::Locus { .. } => {
            let locus = c.locus(id)?;
            let half = t + f64::from(c.drawing_style(id).line_thickness) * 0.25;
            boundary(locus_meets_square(&locus.points, to_screen, p, half))
        }
        Value::InputBox => {
            let [x, y, width, height] = c.input_box_rect(id);
            ((x..x + width).contains(&p.0) && (y..y + height).contains(&p.1))
                .then_some(HitType::Filling)
        }
        Value::Bool(_) => {
            use geo_kernel::kernel::control::CHECKBOX_HIT_SIZE;
            let (x, y) = c.checkbox_position(id)?;
            let extent = CHECKBOX_HIT_SIZE;
            ((x..x + extent).contains(&p.0) && (y..y + extent).contains(&p.1))
                .then_some(HitType::Filling)
        }
        Value::Polygon { .. } => {
            let pts = screen_vertices(c, id, tr)?;
            // `DrawPolygon.hit`: polygons are drawn filled.
            let (mut xmin, mut xmax) = (f64::INFINITY, f64::NEG_INFINITY);
            for q in &pts {
                xmin = xmin.min(q.0);
                xmax = xmax.max(q.0);
            }
            let eps = if xmax - xmin > MIN_HITTING_SIZE {
                reach.mouse
            } else {
                t
            };
            let n = pts.len();
            let edge = (0..n).any(|i| segment_meets_square(pts[i], pts[(i + 1) % n], p, eps));
            let corners = [
                (p.0 - eps, p.1 - eps),
                (p.0 + eps, p.1 - eps),
                (p.0 + eps, p.1 + eps),
                (p.0 - eps, p.1 + eps),
            ];
            let inside = corners
                .iter()
                .filter(|&&q| crate::shape::contains(&pts, q))
                .count();
            let contains = inside == 4 && !edge;
            let intersects = inside > 0 || edge;
            // Filled and the square inside: hit; else the square crosses
            // the outline. Either way a polygon counts as a filling.
            (contains || intersects).then_some(HitType::Filling)
        }
        Value::PolyLine { .. } => {
            let pts = screen_vertices(c, id, tr)?;
            boundary(
                pts.windows(2)
                    .any(|e| segment_meets_square(e[0], e[1], p, t)),
            )
        }
        Value::PenStroke { .. } => {
            let runs = c.stroke(id)?.runs(STROKE_STEPS);
            let t = t + CURVE_STROKE / 2.0;
            boundary(runs.iter().any(|run| {
                let pts: Vec<_> = run.iter().map(|&(x, y)| tr.world_to_screen(x, y)).collect();
                match pts.as_slice() {
                    // a lone point is drawn as the round cap of the pen
                    [q] => (q.0 - p.0).abs() <= t && (q.1 - p.1).abs() <= t,
                    _ => pts
                        .windows(2)
                        .any(|e| segment_meets_square(e[0], e[1], p, t)),
                }
            }))
        }
        _ => None,
    }
}

fn primitive_meets_square(
    value: Value,
    tr: &ViewTransform,
    p: (f64, f64),
    t: f64,
    reach: &Reach,
) -> bool {
    let to_screen = |x, y| tr.world_to_screen(x, y);
    match value {
        Value::Point { x, y } => {
            // `DrawPoint.hit`: r = max(pointSize, threshold + 12); the
            // window's points are never larger than that.
            let s = to_screen(x, y);
            let r = t + reach.point_min;
            point_hit(s, p, r)
        }
        v @ Value::Line { .. } => {
            let Some((a, b, k)) = v.unit_line() else {
                return false;
            };
            // Two points of the line, a unit apart, on the screen.
            let (x0, y0) = (-a * k, -b * k);
            let s0 = to_screen(x0, y0);
            let s1 = to_screen(x0 - b, y0 + a);
            line_meets_square(s0, s1, p, t)
        }
        Value::Segment { x1, y1, x2, y2 } => {
            segment_meets_square(to_screen(x1, y1), to_screen(x2, y2), p, t)
        }
        Value::Ray { x, y, a, b, .. } => {
            let len = a.hypot(b);
            if len == 0.0 {
                return false;
            }
            // A point far along the ray stands in for infinity: far on
            // the coarser axis, so it clears the area on both.
            let far = 1e7 / tr.xscale().min(tr.yscale()).max(1e-9);
            let end = to_screen(x + far * b / len, y - far * a / len);
            segment_meets_square(to_screen(x, y), end, p, t)
        }
        Value::Circle { cx, cy, r } => {
            // The stroked outline (`DrawConic.hit`): the ring between the
            // radii r - w/2 and r + w/2 meets the square when the square's
            // nearest point is inside the outer circle and its farthest
            // corner outside the inner one.
            let s = to_screen(cx, cy);
            // With the scales apart the drawn shape is an ellipse; the
            // ring test keeps one radius, the geometric mean of the scales
            // (exact whenever they are equal).
            let rad = r * (tr.xscale() * tr.yscale()).sqrt();
            let (dx, dy) = ((s.0 - p.0).abs(), (s.1 - p.1).abs());
            let near = (dx - t).max(0.0).hypot((dy - t).max(0.0));
            let far = (dx + t).hypot(dy + t);
            let w = 0.5 * CURVE_STROKE;
            near <= rad + w && far >= rad - w
        }
        _ => false,
    }
}

/// The chords a cubic piece of a pen stroke is hit with.
const STROKE_STEPS: usize = 16;

/// A conic's quadratic in its own coordinate system avoids cancellation
/// from the expanded world coefficients of a translated curve.
fn conic_meets_square(d: &ConicData, tr: &ViewTransform, p: (f64, f64), t: f64) -> bool {
    if !d.defined {
        return false;
    }
    let (x, y) = tr.screen_to_world(p.0, p.1);
    let [mx, my] = d.midpoint;
    let h = t / (tr.xscale() * tr.yscale()).sqrt();
    let [e0, e1] = d.eigenvec;
    let [a, b] = d.half_axes;
    let (dx, dy) = (x - mx, y - my);
    let (u2, v2, u1, constant) = match d.ty {
        ConicType::Circle | ConicType::Ellipse => {
            if dx.abs() > (a * e0[0]).abs() + (b * e1[0]).abs() + h
                || dy.abs() > (a * e0[1]).abs() + (b * e1[1]).abs() + h
            {
                return false;
            }
            (1.0 / a / a, 1.0 / b / b, 0.0, -1.0)
        }
        ConicType::Hyperbola => (1.0 / a / a, -1.0 / b / b, 0.0, -1.0),
        ConicType::Parabola => (0.0, 1.0, -2.0 * d.p, 0.0),
        ConicType::SinglePoint => return dx.hypot(dy) <= h,
        ty if ty.has_lines() => {
            return d.lines.iter().any(|&[a, b, c]| {
                (a != 0.0 || b != 0.0) && (a * x + b * y + c).abs() <= h * (a.abs() + b.abs())
            });
        }
        _ => return false,
    };
    let eigen = |x, y| (x * e0[0] + y * e0[1], x * e1[0] + y * e1[1]);
    let (u, v) = eigen(dx, dy);
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut include = |f: f64| {
        min = min.min(f);
        max = max.max(f);
    };
    let value = |u: f64, v: f64| (u2 * u + u1) * u + (v2 * v) * v + constant;
    // Along each side, the extrema occur at the corners or at the
    // stationary point of a one-variable quadratic.
    for (du, dv, eu, ev) in [(e0[0], e1[0], e0[1], e1[1]), (e0[1], e1[1], e0[0], e1[0])] {
        let q = (u2 * du) * du + (v2 * dv) * dv;
        for sign in [-1.0, 1.0] {
            let (u, v) = (u + sign * h * eu, v + sign * h * ev);
            include(value(u - h * du, v - h * dv));
            include(value(u + h * du, v + h * dv));
            let linear = (u2 * u + u1 / 2.0) * du + (v2 * v) * dv;
            let extremum = -linear / q;
            if extremum.abs() <= h {
                include(value(u + extremum * du, v + extremum * dv));
            }
        }
    }
    if matches!(d.ty, ConicType::Circle | ConicType::Ellipse) && dx.abs() <= h && dy.abs() <= h {
        include(constant);
    }
    min <= 0.0 && max >= 0.0
}

/// A point on a curved conic part, using its conic's parameter.
pub fn part_at(part: &ArcData, angle: f64) -> (f64, f64) {
    let [a, b] = part.conic.half_axes;
    let [e0, e1] = part.conic.eigenvec;
    let [x, y] = part.conic.midpoint;
    let (sin, cos) = angle.sin_cos();
    let (u, v) = (a * cos, b * sin);
    (x + u * e0[0] + v * e1[0], y + u * e0[1] + v * e1[1])
}

/// Parameters where the rim crosses a vertical (axis 0) or horizontal
/// (axis 1) line at `offset` from the centre.
pub fn part_axis_crossings(part: &ArcData, axis: usize, offset: f64) -> impl Iterator<Item = f64> {
    let [e0, e1] = part.conic.eigenvec;
    let [a, b] = part.conic.half_axes;
    let (a, b) = (a * e0[axis], b * e1[axis]);
    let r = a.hypot(b);
    let angles = (offset.abs() <= r && r > 0.0).then(|| {
        let base = b.atan2(a);
        let half = (offset / r).acos();
        [base - half, base + half]
    });
    let (start, extent) = (part.start, part.extent);
    angles.into_iter().flatten().filter_map(move |angle| {
        let t = (angle - start).rem_euclid(TAU);
        (t <= extent).then_some(start + t)
    })
}

/// Drawing and picking use the same prefix of a locus.
pub const MAX_CANVAS_LOCUS_SAMPLES: usize = 10_001;

pub fn canvas_locus_samples(points: &[LocusPoint]) -> &[LocusPoint] {
    &points[..points.len().min(MAX_CANVAS_LOCUS_SAMPLES)]
}

fn locus_meets_square(
    points: &[LocusPoint],
    to_screen: impl Fn(f64, f64) -> (f64, f64),
    p: (f64, f64),
    half: f64,
) -> bool {
    let mut previous = None;
    for point in canvas_locus_samples(points) {
        if !point.x.is_finite() || !point.y.is_finite() {
            previous = None;
            continue;
        }
        let at = to_screen(point.x, point.y);
        if previous.filter(|_| point.line_to).map_or_else(
            || (at.0 - p.0).abs() <= half && (at.1 - p.1).abs() <= half,
            |from| segment_meets_square(from, at, p, half),
        ) {
            return true;
        }
        previous = Some(at);
    }
    false
}

#[cfg(test)]
#[test]
fn canvas_remaining_locus_picking_stops_at_the_edge_limit() {
    let points: Vec<_> = (0..=MAX_CANVAS_LOCUS_SAMPLES)
        .map(|i| LocusPoint {
            x: i as f64 * 10.0,
            y: 0.0,
            line_to: true,
        })
        .collect();
    let end = (MAX_CANVAS_LOCUS_SAMPLES - 1) as f64 * 10.0;
    assert!(locus_meets_square(
        &points,
        |x, y| (x, y),
        (end - 5.0, 0.0),
        1.0
    ));
    assert!(!locus_meets_square(
        &points,
        |x, y| (x, y),
        (end + 5.0, 0.0),
        1.0
    ));
}

fn sector_contains(part: &ArcData, (x, y): (f64, f64)) -> bool {
    if !part.sector || !part.defined || !part.is_curved() {
        return false;
    }
    let [mx, my] = part.conic.midpoint;
    let [[ux, uy], [vx, vy]] = part.conic.eigenvec;
    let [a, b] = part.conic.half_axes;
    let (x, y) = (x - mx, y - my);
    let u = (ux * x + uy * y) / a;
    let v = (vx * x + vy * y) / b;
    u.hypot(v) < 1.0 && (v.atan2(u) - part.start).rem_euclid(TAU) < part.extent
}

fn part_meets_square(part: &ArcData, tr: &ViewTransform, p: (f64, f64), t: f64) -> bool {
    if !part.defined {
        return false;
    }
    let screen = |(x, y)| tr.world_to_screen(x, y);
    if let Some([a, b]) = part.segment() {
        if part.positive {
            return segment_meets_square(screen(a), screen(b), p, t);
        }
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length = dx.hypot(dy);
        if length == 0.0 || !length.is_finite() {
            return false;
        }
        let far = 1e7 / (tr.xscale().min(tr.yscale()) * length);
        return segment_meets_square(screen(a), screen((a.0 - far * dx, a.1 - far * dy)), p, t)
            || segment_meets_square(screen(b), screen((b.0 + far * dx, b.1 + far * dy)), p, t);
    }
    if !part.is_curved() || !conic_meets_square(&part.conic, tr, p, t) && !part.sector {
        return false;
    }
    let start = screen(part_at(part, part.start));
    let end = screen(part_at(part, part.start + part.extent));
    let inside = |(x, y): (f64, f64)| (x - p.0).abs() <= t && (y - p.1).abs() <= t;
    if inside(start) || inside(end) {
        return true;
    }
    let [mx, my] = part.conic.midpoint;
    if part.sector {
        let center = screen((mx, my));
        if segment_meets_square(center, start, p, t) || segment_meets_square(center, end, p, t) {
            return true;
        }
    }
    // Intersections of the ellipse with each edge x = k or y = k satisfy
    // A cos(angle) + B sin(angle) = k, relative to the ellipse's centre.
    let (px, py) = tr.screen_to_world(p.0, p.1);
    let (hx, hy) = (t / tr.xscale(), t / tr.yscale());
    for (axis, d, other, h, other_h) in [(0, px - mx, py, hx, hy), (1, py - my, px, hy, hx)] {
        for k in [d - h, d + h] {
            if part_axis_crossings(part, axis, k).any(|angle| {
                let (x, y) = part_at(part, angle);
                let coordinate = if axis == 0 { y } else { x };
                (coordinate - other).abs() <= other_h
            }) {
                return true;
            }
        }
    }
    false
}

fn function_meets_square(
    c: &Construction,
    id: Id,
    tr: &ViewTransform,
    p: (f64, f64),
    t: f64,
) -> bool {
    let (x, y) = tr.screen_to_world(p.0, p.1);
    let h = t / (tr.xscale() * tr.yscale()).sqrt();
    let eval = |x: f64| c.function_value(id, x).map_or(f64::NAN, |v| v - y);
    let center = eval(x);
    if center.abs() <= h {
        return true;
    }
    // Probe the small horizontal interval under the pointer. In a steep
    // crossing, bisect until an actual graph point is inside the square.
    // A jump or asymptote can straddle it without ever reaching it.
    for direction in [-1.0, 1.0] {
        let mut a = x;
        let mut fa = center;
        for fraction in [0.25, 0.5, 0.75, 1.0] {
            let b = x + direction * h * fraction;
            let fb = eval(b);
            if fb.abs() <= h {
                return true;
            }
            if fa.is_finite() && fb.is_finite() && fa.is_sign_positive() != fb.is_sign_positive() {
                let (mut l, mut r) = (a, b);
                for _ in 0..32 {
                    let m = f64::midpoint(l, r);
                    let fm = eval(m);
                    if fm.abs() <= h {
                        return true;
                    }
                    if !fm.is_finite() || m == l || m == r {
                        break;
                    }
                    if fm.is_sign_positive() == fa.is_sign_positive() {
                        l = m;
                    } else {
                        r = m;
                    }
                }
            }
            (a, fa) = (b, fb);
        }
    }
    false
}

fn knob_hit(
    slider: geo_kernel::kernel::control::SliderGeometry,
    at: (f64, f64),
    threshold: f64,
) -> bool {
    let r = threshold + slider_blob_diameter(slider);
    let (dx, dy) = (at.0 - slider.knob.0, at.1 - slider.knob.1);
    dx.abs() < r && dx * dx + dy * dy <= r * r
}

fn slider_blob_diameter(slider: geo_kernel::kernel::control::SliderGeometry) -> f64 {
    (2.0 * (slider.radius - 0.5).trunc() + 1.0).max(5.0)
}

fn slider_track_hit(
    c: &Construction,
    id: Id,
    slider: geo_kernel::kernel::control::SliderGeometry,
    at: (f64, f64),
    threshold: f64,
) -> bool {
    let thickness = c.drawing_style(id).line_thickness.max(10);
    segment_meets_square(
        slider.start,
        slider.end,
        at,
        threshold + f64::from(thickness),
    )
}

/// DrawSlider.hitSliderNotBlob uses a larger, square exclusion zone to
/// decide what moves after the hit detector has picked the slider.
pub(crate) fn slider_track_grab(
    c: &Construction,
    id: Id,
    slider: geo_kernel::kernel::control::SliderGeometry,
    at: (f64, f64),
    threshold: f64,
) -> bool {
    let r = 3.0 * threshold + slider_blob_diameter(slider);
    ((at.0 - slider.knob.0).abs() > r || (at.1 - slider.knob.1).abs() > r)
        && slider_track_hit(c, id, slider, at, threshold)
}

/// The vertices of a polygon or polyline on the screen, `None` when one of
/// them is undefined.
pub fn screen_vertices(c: &Construction, id: Id, t: &ViewTransform) -> Option<Vec<(f64, f64)>> {
    let pts: Option<Vec<(f64, f64)>> = c
        .vertices(id)
        .map(|v| v.map(|(x, y)| t.world_to_screen(x, y)))
        .collect();
    pts.filter(|p| p.len() >= 2)
}

/// `GeoClass` drawing priorities (higher is drawn on top), used by
/// `DefaultGeoPriorityComparator`.
fn type_priority(kind: Kind) -> u8 {
    match kind {
        Kind::Text => 150,
        Kind::Point => 140,
        Kind::Vector => 120,
        Kind::Segment | Kind::Ray => 110,
        Kind::Line => 100,
        Kind::Function => 90,
        Kind::Circle | Kind::Conic | Kind::Arc | Kind::Sector => 70,
        Kind::PolyLine => 51,
        Kind::Polygon => 50,
        Kind::Angle => 49,
        _ => 0,
    }
}

/// What `pick` keeps of the hits while it streams over the objects.
#[derive(Default)]
struct Choice {
    points: u32,
    /// The free point made last, and the point made first.
    last_free: Option<Id>,
    first_point: Option<Id>,
    /// Hits that are neither points nor polygons, and polygons.
    others: u32,
    polygons: u32,
    the_other: Option<Id>,
    the_polygon: Option<Id>,
    /// The segment made first.
    first_segment: Option<Id>,
    /// The object drawn on top among the others, and among the polygons.
    top_knob: Option<Id>,
    layer: Option<u8>,
    top_other: Option<((HitType, u8, usize), Id)>,
    top_polygon: Option<((HitType, u8, usize), Id)>,
    /// How the selected object was hit, if it was.
    selected: Option<Kind>,
}

impl Choice {
    fn add(&mut self, c: &Construction, id: Id, index: usize, how: HitType, selected: bool) {
        let kind = c.kind(id);
        // HitDetector keeps the top layer before Hits.getTopHits prefers points.
        let layer = c.drawing_style(id).layer;
        if self.layer.is_some_and(|top| layer < top) {
            return;
        }
        if self.layer != Some(layer) {
            *self = Self {
                layer: Some(layer),
                ..Self::default()
            };
        }
        if selected {
            self.selected = Some(kind);
        }
        if how == HitType::SliderKnob && self.top_knob.is_none_or(|old| id > old) {
            self.top_knob = Some(id);
        }
        let key = (how, type_priority(kind), index);
        match kind {
            Kind::Point => {
                self.points += 1;
                self.first_point.get_or_insert(id);
                if c.def(id).is_free() {
                    self.last_free = Some(id);
                }
            }
            Kind::Polygon => {
                self.polygons += 1;
                self.the_polygon = Some(id);
                if self.top_polygon.is_none_or(|(k, _)| key > k) {
                    self.top_polygon = Some((key, id));
                }
            }
            _ => {
                self.others += 1;
                self.the_other = Some(id);
                if kind == Kind::Segment {
                    self.first_segment.get_or_insert(id);
                }
                if self.top_other.is_none_or(|(k, _)| key > k) {
                    self.top_other = Some((key, id));
                }
            }
        }
    }

    fn hits(&self) -> bool {
        self.points + self.others + self.polygons > 0
    }

    /// `handleMousePressedForMoveMode`: the top hits (only the points when a
    /// point is hit, `Hits.getTopHits`), without the polygons when anything
    /// else is left (`removePolygonsIfNotOnlyCS2D`); the selected object if
    /// it is among them; else `chooseGeo`. The candidates are in the top layer.
    fn choose(&self, selected: Option<Id>) -> Option<Id> {
        let kept = |k: Kind| -> bool {
            if self.points > 0 {
                k == Kind::Point
            } else if self.others > 0 {
                k != Kind::Polygon
            } else {
                k == Kind::Polygon
            }
        };
        if let (Some(id), Some(k)) = (selected, self.selected) {
            if kept(k) {
                return Some(id);
            }
        }
        if self.points > 0 {
            // A free point (the one made last), else a single point, else
            // the point made first (`chooseGeo` returns `retIndex`, the
            // lowest construction index, though its comment says the
            // most recent).
            return self.last_free.or(self.first_point);
        }
        if let Some(knob) = self.top_knob {
            return Some(knob);
        }
        if self.others > 0 {
            if self.others == 1 {
                return self.the_other;
            }
            // Segments first: in one layer, the one made first.
            return self.first_segment.or(self.top_other.map(|(_, id)| id));
        }
        if self.polygons == 1 {
            return self.the_polygon;
        }
        self.top_polygon.map(|(_, id)| id)
    }
}

/// The object a press at `(sx, sy)` takes hold of, among those `accept`
/// allows; `selected` is the selected object (kept when it is hit).
pub fn pick(
    c: &Construction,
    t: &ViewTransform,
    position: (f64, f64),
    reach: &Reach,
    selected: Option<Id>,
    accept: impl Fn(Id) -> bool,
) -> Option<Id> {
    pick_with_text(c, t, position, reach, selected, None, accept)
}

/// Includes a hit on a painted text rectangle alongside geometric hits.
pub fn pick_with_text(
    c: &Construction,
    t: &ViewTransform,
    position: (f64, f64),
    reach: &Reach,
    selected: Option<Id>,
    text: Option<Id>,
    accept: impl Fn(Id) -> bool,
) -> Option<Id> {
    let mut list_items_left = geo_kernel::expr::MAX_FRAME_CANVAS_LIST_ITEMS;
    let mut pass = |threshold: f64| {
        let mut choice = Choice::default();
        if let Some(id) = text {
            choice.add(c, id, id.index(), HitType::Filling, Some(id) == selected);
        }
        for (index, id) in c.ids().enumerate() {
            if c.kind(id) == Kind::Text || !accept(id) {
                continue;
            }
            if let Some(how) =
                hit_with_list_budget(c, id, t, position, threshold, reach, &mut list_items_left)
            {
                choice.add(c, id, index, how, Some(id) == selected);
            }
        }
        choice
    };
    let mut choice = pass(reach.threshold);
    if let (false, Some(retry)) = (choice.hits(), reach.retry) {
        choice = pass(retry);
    }
    choice.choose(selected)
}

/// Every object among those `accept` allows that a press at `(sx, sy)`
/// hits, in construction order (`EuclidianView.setHits`: the square of the
/// threshold, then the finger's larger one when it hits nothing). Made for
/// one press, not for every frame: it allocates.
pub fn hits(
    c: &Construction,
    t: &ViewTransform,
    (sx, sy): (f64, f64),
    reach: &Reach,
    accept: impl Fn(Id) -> bool,
) -> Vec<Id> {
    let mut list_items_left = geo_kernel::expr::MAX_FRAME_CANVAS_LIST_ITEMS;
    let mut pass = |threshold: f64| -> Vec<Id> {
        c.ids()
            .filter(|&id| {
                accept(id)
                    && hit_with_list_budget(
                        c,
                        id,
                        t,
                        (sx, sy),
                        threshold,
                        reach,
                        &mut list_items_left,
                    )
                    .is_some()
            })
            .collect()
    };
    let found = pass(reach.threshold);
    match reach.retry {
        Some(retry) if found.is_empty() => pass(retry),
        _ => found,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_kernel::svg::View;

    /// 100 x 100 px area showing the square [-5, 5]^2: 10 px per unit.
    fn setup() -> (Construction, ViewTransform) {
        let c = geo_kernel::script::load(
            "A = (0, 0)\nB = (4, 0)\nf = Line(A, B)\nk = Circle(A, B)\ns = Segment(A, B)\nN = 3\n",
        )
        .unwrap();
        let t = ViewTransform::new(
            View {
                xmin: -5.0,
                ymin: -5.0,
                xmax: 5.0,
                ymax: 5.0,
            },
            100.0,
            100.0,
        );
        (c, t)
    }

    fn label(c: &Construction, id: Option<Id>) -> Option<String> {
        id.map(|i| c.label(i).to_owned())
    }

    fn mouse() -> Reach {
        Reach::of(&Metrics::desktop(), false)
    }

    fn touch() -> Reach {
        Reach::of(&Metrics::touch(), true)
    }

    fn only<'a>(c: &'a Construction, name: &'a str) -> impl Fn(Id) -> bool + 'a {
        move |id| c.label(id) == name
    }

    #[test]
    fn an_angle_hits_the_square_or_sector_instead_of_its_bounding_box() {
        let (mut c, t) = setup();
        let script = geo_kernel::script::exec;
        script(&mut c, "C = (0, 4)").unwrap();
        let right = script(&mut c, "right = Angle(B, A, C)").unwrap().unwrap();
        let reflex = script(&mut c, "reflex = Angle(C, A, B)").unwrap().unwrap();
        let (x, y) = t.world_to_screen(0.0, 0.0);
        let test = |id, dx, dy| hit(&c, id, &t, (x + dx, y + dy), 3.0, &mouse());
        assert_eq!(test(right, 10.0, -10.0), Some(HitType::Filling));
        assert_eq!(test(right, 16.0, -1.0), None);
        assert_eq!(test(reflex, 10.0, -10.0), None);
        assert_eq!(test(reflex, 10.0, 10.0), Some(HitType::Filling));
    }

    #[test]
    fn a_point_wins_over_curves_through_it() {
        let (c, t) = setup();
        let (sx, sy) = t.world_to_screen(0.0, 0.0);
        let id = pick(&c, &t, (sx + 2.0, sy), &mouse(), None, |_| true);
        assert_eq!(label(&c, id).as_deref(), Some("A"));
    }

    /// `DrawPoint.hit`: r = 3 + 12 = 15 for the mouse, `abs(dx) < r`.
    #[test]
    fn a_point_is_hit_within_the_selection_radius() {
        let (c, t) = setup();
        let (sx, sy) = t.world_to_screen(0.0, 0.0);
        let at = |dx: f64, dy: f64, reach: &Reach| {
            pick(&c, &t, (sx + dx, sy + dy), reach, None, only(&c, "A")).is_some()
        };
        assert!(at(-14.0, 0.0, &mouse()) && !at(-15.0, 0.0, &mouse()));
        assert!(at(0.0, -15.0, &mouse()) && !at(0.0, -15.5, &mouse()));
        // A finger: 9 + 12, then the retry at 27 + 12.
        assert!(at(-38.0, 0.0, &touch()) && !at(-39.0, 0.0, &touch()));
    }

    /// `Line2D.intersects` with the square of half-width 3 (mouse) or 9
    /// then 27 (touch).
    #[test]
    fn curves_are_hit_within_the_square() {
        let (c, t) = setup();
        // Above the line f (y = 0) between A and B.
        let (sx, sy) = t.world_to_screen(2.0, 0.0);
        let line = |dy: f64, reach: &Reach| {
            pick(&c, &t, (sx, sy + dy), reach, None, only(&c, "f")).is_some()
        };
        assert!(line(-3.0, &mouse()) && !line(-4.0, &mouse()));
        assert!(line(-27.0, &touch()) && !line(-28.0, &touch()));
        // The circle of radius 4 around the origin: its outline is 2.5 wide.
        let (cx, cy) = t.world_to_screen(0.0, 4.0);
        let circle = |dy: f64| pick(&c, &t, (cx, cy + dy), &mouse(), None, only(&c, "k"));
        assert!(circle(-4.0).is_some() && circle(4.0).is_some());
        assert!(circle(-4.5).is_none() && circle(4.5).is_none());
    }

    #[test]
    fn segments_end_at_their_end_points() {
        let (c, t) = setup();
        let (sx, sy) = t.world_to_screen(4.0, 0.0);
        let beyond = pick(&c, &t, (sx + 4.0, sy), &mouse(), None, only(&c, "s"));
        assert_eq!(beyond, None);
        let near = pick(&c, &t, (sx + 3.0, sy), &mouse(), None, only(&c, "s"));
        assert_eq!(label(&c, near).as_deref(), Some("s"));
    }

    #[test]
    fn accept_filters_and_numbers_are_never_hit() {
        let (c, t) = setup();
        let (sx, sy) = t.world_to_screen(0.0, 0.0);
        assert_eq!(pick(&c, &t, (sx, sy), &mouse(), None, |_| false), None);
        assert_eq!(pick(&c, &t, (sx, sy), &mouse(), None, only(&c, "N")), None);
    }

    /// Was `the_nearest_curve_wins`: `chooseGeo` does not measure distances.
    /// Of a line and a circle both hit, the line is drawn on top
    /// (`GeoClass` priorities), and a segment wins over both.
    #[test]
    fn a_segment_then_the_object_drawn_on_top_wins() {
        let (c, t) = setup();
        let (sx, sy) = t.world_to_screen(-4.0, 0.0);
        let pick_of = |names: &[&str]| {
            label(
                &c,
                pick(&c, &t, (sx, sy + 1.0), &mouse(), None, |id| {
                    names.contains(&c.label(id))
                }),
            )
        };
        assert_eq!(pick_of(&["f", "k"]).as_deref(), Some("f"));
        let (sx, sy) = t.world_to_screen(2.0, 0.0);
        let id = pick(&c, &t, (sx, sy), &mouse(), None, |id| {
            matches!(c.label(id), "f" | "s")
        });
        assert_eq!(label(&c, id).as_deref(), Some("s"));
    }

    /// `chooseGeo`: of several free points, the one made last; of
    /// dependent points only, the one made first.
    #[test]
    fn of_several_points_the_free_one_made_last_wins() {
        let c = geo_kernel::script::load(
            "A = (0, 0)\nB = (0.1, 0)\nC = (4, 0)\nM = Midpoint(A, C)\nN = Midpoint(A, C)\n",
        )
        .unwrap();
        let (_, t) = setup();
        let (sx, sy) = t.world_to_screen(0.0, 0.0);
        let id = pick(&c, &t, (sx, sy), &mouse(), None, |_| true);
        assert_eq!(label(&c, id).as_deref(), Some("B"), "not the nearest, A");
        let (mx, my) = t.world_to_screen(2.0, 0.0);
        let id = pick(&c, &t, (mx, my), &mouse(), None, |id| c.label(id) != "C");
        assert_eq!(label(&c, id).as_deref(), Some("M"));
        // The selected object stays chosen while it is hit.
        let id = pick(&c, &t, (sx, sy), &mouse(), c.id("A"), |_| true);
        assert_eq!(label(&c, id).as_deref(), Some("A"));
    }

    /// `DrawConic.hit`: an unfilled circle's inside is not hit;
    /// `DrawPolygon.hit`: a filled polygon's inside is; a polygon is
    /// dropped when anything else is hit (`removePolygonsIfNotOnlyCS2D`).
    #[test]
    fn fillings_count_only_for_polygons_and_lose_to_anything_else() {
        let c = geo_kernel::script::load(
            "A = (-3, -3)\nB = (3, -3)\nC = (0, 3)\nq = Polygon(A, B, C)\nO = (-4, 4)\nr = 3\nk = Circle(O, r)\n",
        )
        .unwrap();
        let (_, t) = setup();
        let centre = t.world_to_screen(0.0, -0.5);
        let id = pick(&c, &t, centre, &mouse(), None, |_| true);
        assert_eq!(label(&c, id).as_deref(), Some("q"));
        let outside = t.world_to_screen(4.0, 4.0);
        assert_eq!(pick(&c, &t, outside, &mouse(), None, |_| true), None);
        // Inside the circle, away from its centre point: nothing.
        let inside_circle = t.world_to_screen(-4.0, 2.0);
        assert_eq!(pick(&c, &t, inside_circle, &mouse(), None, |_| true), None);
        let on_circle = t.world_to_screen(-4.0, 1.0);
        let id = pick(&c, &t, on_circle, &mouse(), None, |_| true);
        assert_eq!(label(&c, id).as_deref(), Some("k"));
    }

    #[test]
    fn a_finger_retries_with_a_larger_square_only_when_nothing_is_hit() {
        let (c, t) = setup();
        let (sx, sy) = t.world_to_screen(2.0, 0.0);
        // 20 px above f: beyond 9, within 27.
        let id = pick(&c, &t, (sx, sy - 20.0), &touch(), None, only(&c, "f"));
        assert_eq!(label(&c, id).as_deref(), Some("f"));
        assert_eq!(
            pick(&c, &t, (sx, sy - 20.0), &mouse(), None, only(&c, "f")),
            None
        );
    }

    /// `removePolygonsIfNotOnlyCS2D`: a press on a polygon's side hits the
    /// polygon and its side (a segment); the polygon is dropped and the side
    /// chosen, and a line across the inside wins over the filling. Fails if
    /// the polygon is kept among other hits.
    #[test]
    fn a_polygon_is_dropped_when_anything_else_is_hit() {
        let c = geo_kernel::script::load(
            "A = (-3, -3)\nB = (3, -3)\nC = (0, 3)\nq = Polygon(A, B, C)\nD = (-4, 0)\nE = (4, 0)\nf = Line(D, E)\n",
        )
        .unwrap();
        let (_, t) = setup();
        let side = t.world_to_screen(1.0, -3.0);
        let id = pick(&c, &t, side, &mouse(), None, |_| true);
        let kind = id.map(|id| c.kind(id));
        assert_eq!(kind, Some(Kind::Segment), "{:?}", label(&c, id));
        let on_line = t.world_to_screen(0.5, 0.0);
        let id = pick(&c, &t, on_line, &mouse(), None, |_| true);
        assert_eq!(label(&c, id).as_deref(), Some("f"));
        // Alone, the polygon is hit through its filling and on its side.
        let q = |pos| label(&c, pick(&c, &t, pos, &mouse(), None, only(&c, "q")));
        assert_eq!(q(side).as_deref(), Some("q"));
        assert_eq!(q(on_line).as_deref(), Some("q"));
    }

    /// `chooseGeo`: a free point wins over a dependent point at the same
    /// place, even when the free one was made first. Fails if the point
    /// made last wins whether it is free or not.
    #[test]
    fn a_free_point_wins_over_a_dependent_point_on_it() {
        let c =
            geo_kernel::script::load("P = (2, 0)\nA = (0, 0)\nB = (4, 0)\nM = Midpoint(A, B)\n")
                .unwrap();
        let (_, t) = setup();
        let at = t.world_to_screen(2.0, 0.0);
        let id = pick(&c, &t, at, &mouse(), None, |_| true);
        assert_eq!(label(&c, id).as_deref(), Some("P"));
    }

    /// `chooseGeo` takes a segment before the type priority: a vector is
    /// drawn above a segment (`GeoClass` 120 against 110), yet the segment
    /// wins, whichever was made first. Fails if the type priority decides.
    #[test]
    fn a_segment_wins_over_a_vector_drawn_above_it() {
        for script in [
            "A = (0, 0)\nB = (4, 0)\ns = Segment(A, B)\nu = Vector(A, B)\n",
            "A = (0, 0)\nB = (4, 0)\nu = Vector(A, B)\ns = Segment(A, B)\n",
        ] {
            let c = geo_kernel::script::load(script).unwrap();
            let (_, t) = setup();
            let at = t.world_to_screen(2.0, 0.0);
            let both = |id: Id| matches!(c.label(id), "s" | "u");
            assert_eq!(
                label(
                    &c,
                    pick(&c, &t, at, &mouse(), None, |id| c.label(id) == "u")
                )
                .as_deref(),
                Some("u"),
                "the vector is hit"
            );
            let id = pick(&c, &t, at, &mouse(), None, both);
            assert_eq!(label(&c, id).as_deref(), Some("s"), "{script}");
        }
    }

    /// The finger's retry runs only when the first square hit nothing: a
    /// line 5 px away is hit at once, so a point 30 px away (within the
    /// retry's 27 + 12, beyond the first 9 + 12) is not tried, though a
    /// point would win. Fails if the retry always runs.
    #[test]
    fn a_finger_does_not_retry_when_something_is_hit() {
        let (c, t) = setup();
        let (sx, sy) = t.world_to_screen(0.0, 0.0);
        let near = |id: Id| matches!(c.label(id), "A" | "f");
        let id = pick(&c, &t, (sx + 30.0, sy - 5.0), &touch(), None, near);
        assert_eq!(label(&c, id).as_deref(), Some("f"));
        // Away from the line, the retry finds the point.
        let id = pick(&c, &t, (sx + 30.0, sy - 12.0), &touch(), None, near);
        assert_eq!(label(&c, id).as_deref(), Some("A"));
    }
}

#[cfg(test)]
mod control_tests {
    use super::*;

    #[test]
    fn a_higher_layer_slider_knob_wins_over_a_point() {
        let mut c = geo_kernel::script::load("a = Slider(0, 10)\na = 5").unwrap();
        let slider = c.id("a").unwrap();
        let view = ViewTransform::new(geo_kernel::svg::View::fit(&c), 400.0, 300.0);
        let at = (130.0, 50.0);
        let (x, y) = view.screen_to_world(at.0, at.1);
        let point = c.add("P", geo_kernel::Def::FreePoint { x, y }).unwrap();
        let reach = Reach::square(3.0);
        assert_eq!(pick(&c, &view, at, &reach, None, |_| true), Some(point));
        c.set_drawing_style(
            slider,
            geo_kernel::kernel::DrawingStyle {
                layer: 1,
                ..Default::default()
            },
        );
        assert_eq!(
            pick(&c, &view, at, &reach, Some(point), |_| true),
            Some(slider)
        );
    }

    #[test]
    fn a_selected_track_wins_over_an_overlapping_knob() {
        let c = geo_kernel::script::load(
            "a = Slider(0, 10)\na = 5\nb = Slider(0, 10)\n\
             $sliderStyle(_62, true, 30, 50, 200, true, false, _, _)",
        )
        .unwrap();
        let (a, b) = (c.id("a").unwrap(), c.id("b").unwrap());
        let view = ViewTransform::new(geo_kernel::svg::View::fit(&c), 400.0, 300.0);
        let reach = Reach::square(3.0);
        assert_eq!(
            hit(&c, a, &view, (130.0, 50.0), 3.0, &reach),
            Some(HitType::SliderKnob)
        );
        assert_eq!(
            hit(&c, b, &view, (130.0, 50.0), 3.0, &reach),
            Some(HitType::Boundary)
        );
        assert_eq!(
            pick(&c, &view, (130.0, 50.0), &reach, Some(b), |_| true),
            Some(b)
        );
        let mut c = c;
        let (x, y) = view.screen_to_world(130.0, 50.0);
        let point = c.add("P", geo_kernel::Def::FreePoint { x, y }).unwrap();
        assert_eq!(
            pick(&c, &view, (130.0, 50.0), &reach, Some(b), |_| true),
            Some(point)
        );
    }
}

#[cfg(test)]
#[test]
fn canvas_list_hits_member_shapes() {
    let c = geo_kernel::script::load(
        "A = (0, 0)\nB = (3, 0)\ns = Segment(A, B)\nl = {(0, 2), s}\nn = {0, 2}",
    )
    .unwrap();
    let tr = ViewTransform::new(geo_kernel::svg::View::fit(&c), 400.0, 300.0);
    let reach = Reach::square(3.0);
    let test = |label, x, y| {
        hit(
            &c,
            c.id(label).unwrap(),
            &tr,
            tr.world_to_screen(x, y),
            3.0,
            &reach,
        )
    };
    assert_eq!(test("l", 0.0, 2.0), Some(HitType::Boundary));
    assert_eq!(test("l", 1.5, 0.0), Some(HitType::Boundary));
    assert_eq!(test("l", 1.5, 2.0), None);
    assert_eq!(test("n", 0.0, 0.0), None);
}

#[cfg(test)]
#[test]
fn canvas_remaining_sector_chart_and_locus_hits_use_their_shapes() {
    let c = geo_kernel::script::load(
        "A = (0, 0)\nB = (2, 0)\nC = (0, 2)\na = CircularArc(A, B, C)\ns = CircularSector(A, B, C)\n\
         chart = PieChart({1, 2}, (4, 0), 2)\nk = Circle(A, B)\nP = Point(k)\nQ = Midpoint(P, A)\nl = Locus(Q, P)",
    )
    .unwrap();
    let view = ViewTransform::new(geo_kernel::svg::View::fit(&c), 800.0, 600.0);
    let reach = Reach::square(3.0);
    let at = |label, x, y| {
        hit(
            &c,
            c.id(label).unwrap(),
            &view,
            view.world_to_screen(x, y),
            3.0,
            &reach,
        )
    };
    assert_eq!(at("a", 0.5, 0.5), None);
    assert_eq!(at("s", 0.5, 0.5), Some(HitType::Filling));
    assert_eq!(at("chart", 4.5, 0.5), Some(HitType::Filling));
    assert_eq!(at("chart", 6.1, 0.0), None);
    assert_eq!(at("l", 0.0, 1.0), Some(HitType::Boundary));
    assert_eq!(at("l", 0.0, 0.0), None);
}

#[cfg(test)]
#[test]
fn canvas_remaining_image_picking_uses_the_raster_quad() {
    let file = include_bytes!("../../../golden/ggb/saved/saved-images-001.ggb");
    let imported = geo_import::import(file).unwrap();
    let c = geo_kernel::script::load(&imported.script.join("\n")).unwrap();
    let view = ViewTransform::new(geo_kernel::svg::View::fit(&c), 800.0, 600.0);
    let reach = Reach::square(3.0);
    let mut checked = false;
    for (id, image) in c.images() {
        if image.style.background {
            continue;
        }
        let Some([a, _, b, _]) = c.image_quad_with_scales(id, view.xscale(), view.yscale()) else {
            continue;
        };
        let (x, y) = (0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1]));
        let pos = if image.placement.is_screen() {
            (x, y)
        } else {
            view.world_to_screen(x, y)
        };
        assert_eq!(hit(&c, id, &view, pos, 3.0, &reach), Some(HitType::Filling));
        assert_eq!(hit(&c, id, &view, (-10000.0, -10000.0), 3.0, &reach), None);
        checked = true;
    }
    assert!(checked);
}

#[test]
fn canvas_picking_shares_the_list_allowance() {
    use std::fmt::Write as _;
    let mut script = String::new();
    for i in 0..16 {
        writeln!(script, "L{i} = Sequence((k, 10), k, 1, 256)").unwrap();
    }
    script.push_str("L16 = {(0, 0)}\n");
    let c = geo_kernel::script::load(&script).unwrap();
    let tr = ViewTransform::new(
        geo_kernel::svg::View {
            xmin: -5.0,
            xmax: 5.0,
            ymin: -5.0,
            ymax: 5.0,
        },
        400.0,
        400.0,
    );
    let at = tr.world_to_screen(0.0, 0.0);
    let reach = Reach::square(3.0);
    assert!(hits(&c, &tr, at, &reach, |_| true).is_empty());
    assert_eq!(pick(&c, &tr, at, &reach, None, |_| true), None);
    let last = c.id("L16").unwrap();
    assert_eq!(pick(&c, &tr, at, &reach, None, |id| id == last), Some(last));
}

#[test]
fn canvas_locus_picks_a_lone_sample_without_joining_gaps() {
    let points = [
        LocusPoint {
            x: 0.0,
            y: 0.0,
            line_to: false,
        },
        LocusPoint {
            x: 10.0,
            y: 0.0,
            line_to: false,
        },
    ];
    assert!(locus_meets_square(&points, |x, y| (x, y), (0.0, 0.0), 1.0));
    assert!(!locus_meets_square(&points, |x, y| (x, y), (5.0, 0.0), 1.0));
}

#[test]
fn canvas_sector_picking_respects_saved_fill() {
    let imported = geo_import::import(include_bytes!(
        "../../../golden/ggb/saved/saved-canvas-sector-001.xml"
    ))
    .unwrap();
    let c = geo_kernel::script::load(&imported.script.join("\n")).unwrap();
    let tr = ViewTransform::new(geo_kernel::svg::View::fit(&c), 800.0, 600.0);
    let at = |label, x, y| {
        hit(
            &c,
            c.id(label).unwrap(),
            &tr,
            tr.world_to_screen(x, y),
            3.0,
            &Reach::square(3.0),
        )
    };
    assert_eq!(at("s", 0.5, 0.5), None);
    assert_eq!(at("s", 1.0, 0.0), Some(HitType::Boundary));
    assert_eq!(at("shaded", 0.5, 0.5), Some(HitType::Filling));
}

#[test]
fn closing_dot_plot_first_member_uses_a_single_budget_unit() {
    let mut script = "L = Sequence(k, k, 1, 256)\nM = Sequence(k, k, 1, 257)\n".to_owned();
    for i in 0..17 {
        use std::fmt::Write as _;
        writeln!(script, "dots{i} = DotPlot(L)").unwrap();
    }
    script.push_str("longdots = DotPlot(M)");
    let c = geo_kernel::script::load(&script).unwrap();
    let tr = ViewTransform::new(geo_kernel::svg::View::fit(&c), 400.0, 300.0);
    let p = tr.world_to_screen(1.0, 1.0);
    let reach = Reach::square(3.0);
    let mut remaining = 18;
    for id in c.ids().filter(|&id| c.chart(id).is_some()) {
        assert!(hit_with_list_budget(&c, id, &tr, p, 3.0, &reach, &mut remaining).is_some());
    }
    assert_eq!(remaining, 0);
    let id = c.id("longdots").unwrap();
    assert!(hit_with_list_budget(&c, id, &tr, p, 3.0, &reach, &mut remaining).is_none());
}
