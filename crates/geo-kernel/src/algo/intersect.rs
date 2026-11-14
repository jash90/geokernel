//! Intersection points of lines and circles.
//!
//! Line–circle and circle–circle intersections follow GeoGebra with its
//! continuity setting off: the two points keep the order in which they were
//! computed when the intersection was created, a tangent point is only
//! intersection 1, and a point that lies on both objects by construction is
//! always intersection 1.

// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersect.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectConics.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectLineConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectSingle.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCircleTwoPoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectLines.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoJoinPoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLinePointLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoOrthoLinePointLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec3D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoQuadricND.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::Parents;
use crate::kernel::{Construction, Def, Id, Value};
use crate::tol::{EPS, MAX_PRECISION, MIN_PRECISION, STANDARD_PRECISION};

/// Points that an intersection reads besides its line and circles: a point
/// on both objects by construction becomes intersection 1 with its own
/// coordinates, so the intersection must be recomputed when that point
/// moves even if the line and the circles keep their values.
pub(crate) fn hidden_parents(c: &Construction, def: &Def) -> Parents {
    match *def {
        Def::IntersectLineCircle { line, circle, .. } => {
            match existing_line_circle_point(c, line, c.def(line), circle) {
                Some(s) => Parents::of(&[s]),
                None => Parents::none(),
            }
        }
        Def::IntersectCircles { c1, c2, .. } => {
            let mut ids = [Id(0); 4];
            let mut n = 0;
            let [a, b] = circle_points_on_other(c, c1, c2);
            let [d, e] = circle_points_on_other(c, c2, c1);
            for s in [a, b, d, e].into_iter().flatten() {
                if !ids[..n].contains(&s) {
                    ids[n] = s;
                    n += 1;
                }
            }
            Parents::of(&ids[..n])
        }
        _ => Parents::none(),
    }
}

pub(crate) fn lines(c: &Construction, l: Id, m: Id) -> Option<Value> {
    let (a1, b1, c1) = c.line(l)?;
    let (a2, b2, c2) = c.line(m)?;
    let det = a1 * b2 - a2 * b1;
    if det.abs() < EPS {
        return None; // parallel
    }
    Some(Value::Point {
        x: (b1 * c2 - b2 * c1) / det,
        y: (a2 * c1 - a1 * c2) / det,
    })
}

/// A point, or `None` where GeoGebra's point is undefined or at infinity.
type Pt = Option<(f64, f64)>;

/// `which` is 0 or 1: GeoGebra's intersection 1 or 2 (`AlgoIntersectLineConic`).
pub(crate) fn line_circle(c: &Construction, line: Id, circle: Id, which: u8) -> Option<Value> {
    let (cx, cy, r) = c.circle(circle)?;
    let line_def = c.def(line);
    let g = raw_line(c, line, line_def)?;
    let conic = Conic::new(cx, cy, r);

    let p = match existing_line_circle_point(c, line, line_def, circle) {
        // A point on both by construction is intersection 1 (`handleSpecialCase`).
        Some(s) => {
            let first = c.point(s);
            let q = intersect_line_conic(g, &conic);
            [first, other_point(&q, first)]
        }
        None => {
            let mut p = intersect_line_conic(g, &conic);
            avoid_double_tangent_point(&mut p);
            p
        }
    };
    to_value(p[usize::from(which)])
}

/// `which` is 0 or 1: GeoGebra's intersection 1 or 2 (`AlgoIntersectConics`).
pub(crate) fn circles(c: &Construction, c1: Id, c2: Id, which: u8) -> Option<Value> {
    let (x1, y1, r1) = c.circle(c1)?;
    let (x2, y2, r2) = c.circle(c2)?;
    let a = Conic::new(x1, y1, r1);
    let b = Conic::new(x2, y2, r2);

    let p = match shared_circles_point(c, c1, c2, &a, &b) {
        Some(s) => {
            let first = c.point(s);
            let q = shared_point_case(&a, &b);
            [first, other_point(&q, first)]
        }
        None => {
            let mut p = intersect_conics(&a, &b);
            match_existing_intersections(c, c1, c2, &mut p);
            avoid_double_tangent_point(&mut p);
            p
        }
    };
    to_value(p[usize::from(which)])
}

fn to_value(p: Pt) -> Option<Value> {
    p.map(|(x, y)| Value::Point { x, y })
}

/// The first of `q` that differs from `first`, or `first` when there is none.
/// An undefined point differs from every point.
fn other_point(q: &[Pt; 2], first: Pt) -> Pt {
    for &p in q {
        if !points_equal(p, first) {
            return p;
        }
    }
    first
}

/// `AlgoIntersect.avoidDoubleTangentPoint`: a tangent point is only intersection 1.
fn avoid_double_tangent_point(p: &mut [Pt; 2]) {
    if points_equal(p[0], p[1]) {
        p[1] = None;
    }
}

/// `GeoPoint.isEqual` for finite points.
fn points_equal(p: Pt, q: Pt) -> bool {
    match (p, q) {
        (Some((x1, y1)), Some((x2, y2))) => is_equal(x1, x2) && is_equal(y1, y2),
        _ => false,
    }
}

// ------------------------------------------------------------ DoubleUtil

fn is_zero(v: f64) -> bool {
    -STANDARD_PRECISION < v && v < STANDARD_PRECISION
}

fn is_zero_eps(v: f64, eps: f64) -> bool {
    -eps < v && v < eps
}

fn is_equal(a: f64, b: f64) -> bool {
    a == b || ((a - STANDARD_PRECISION) <= b && b <= (a + STANDARD_PRECISION))
}

fn is_equal_eps(x: f64, y: f64, eps: f64) -> bool {
    x == y || ((x - eps) < y && y < (x + eps))
}

/// `Math.min` and `Math.max` of Java propagate NaN, unlike `f64::min`/`max`.
fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// `Math.round`: the closest integer, ties towards positive infinity.
fn java_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 {
        f + 1.0
    } else {
        f
    }
}

/// `DoubleUtil.checkDecimalFraction(val, 1e8)`: with that precision the
/// tolerance is 1, so the value is always rounded to five decimals.
fn check_decimal_fraction(val: f64) -> f64 {
    const INV_MIN_PRECISION: f64 = 1e5;
    let frac = val * INV_MIN_PRECISION;
    let round = java_round(frac);
    if is_equal_eps(frac, round, STANDARD_PRECISION * 1e8) {
        round / INV_MIN_PRECISION
    } else {
        val
    }
}

/// `MyMath.length`.
fn my_length(x: f64, y: f64) -> f64 {
    let (ax, ay) = (x.abs(), y.abs());
    if ax == 0.0 {
        ay
    } else if ay == 0.0 {
        ax
    } else if ax > ay {
        let t = ay / ax;
        ax * (1.0 + t * t).sqrt()
    } else {
        let t = ax / ay;
        ay * (1.0 + t * t).sqrt()
    }
}

/// A point with homogeneous coordinates, as `GeoPoint.setCoords` stores it.
fn hom_point(x: f64, y: f64, z: f64) -> Pt {
    if x.is_nan() || y.is_nan() || z.is_nan() {
        return None;
    }
    // DoubleUtil.isEpsilon(z, x, y): a point at infinity.
    let za = z.abs();
    if !(za > STANDARD_PRECISION
        || za > x.abs() * STANDARD_PRECISION
        || za > y.abs() * STANDARD_PRECISION)
    {
        return None;
    }
    let (x, y, z) = if z < 0.0 { (-x, -y, -z) } else { (x, y, z) };
    if z == 1.0 {
        Some((x, y))
    } else {
        Some((x / z, y / z))
    }
}

fn point(x: f64, y: f64) -> Pt {
    hom_point(x, y, 1.0)
}

// ----------------------------------------------------------------- lines

/// The line's coefficients.
fn raw_line(c: &Construction, line: Id, _line_def: Def) -> Option<[f64; 3]> {
    let (a, b, cc) = c.line(line)?;
    Some([a, b, cc])
}

/// `GeoLine.getNormalizedCoefficients(ret, 2, 0.5)`: scales by powers of 2
/// until no coefficient is above 2 or not all are below 0.5.
fn normalized_coefficients(g: [f64; 3]) -> [f64; 3] {
    let mut r = g;
    if (is_zero(g[0]) && is_zero(g[1]) && is_zero(g[2])) || !g.iter().all(|v| v.is_finite()) {
        return r;
    }
    while r[0].abs() < 0.5 && r[1].abs() < 0.5 && r[2].abs() < 0.5 {
        r = [r[0] * 2.0, r[1] * 2.0, r[2] * 2.0];
    }
    while r[0].abs() > 2.0 && r[1].abs() > 2.0 && r[2].abs() > 2.0 {
        r = [r[0] / 2.0, r[1] / 2.0, r[2] / 2.0];
    }
    r
}

/// `GeoLine.isOnFullLine` for a finite point.
fn on_full_line(g: [f64; 3], (x, y): (f64, f64), eps: f64) -> bool {
    let simple_length = g[0].abs() + g[1].abs();
    (g[0] * x + g[1] * y + g[2]).abs() < eps * simple_length
}

// ---------------------------------------------------------------- conics

/// A circle as a GeoGebra conic: its flat matrix
/// `[A0, A1, A2, A3, A4, A5]` of `A0 x² + A1 y² + A2 + 2 A3 xy + 2 A4 x + 2 A5 y = 0`.
struct Conic {
    m: [f64; 6],
    cx: f64,
    cy: f64,
    r: f64,
}

impl Conic {
    /// `GeoQuadricND.setSphereNDMatrix`.
    fn new(cx: f64, cy: f64, r: f64) -> Self {
        let mut m2 = -r * r;
        m2 += cx * cx;
        m2 += cy * cy;
        Conic {
            m: [1.0, 1.0, m2, 0.0, -cx, -cy],
            cx,
            cy,
            r,
        }
    }

    /// GeoGebra's conic type is a circle from `STANDARD_PRECISION` on and a
    /// single point below it.
    fn is_circle(&self) -> bool {
        self.r >= STANDARD_PRECISION
    }

    /// `GeoConicND.isOnFullConic`.
    fn on_full_conic(&self, (x, y): (f64, f64), eps: f64) -> bool {
        if !self.is_circle() {
            return my_length(x - self.cx, y - self.cy) < eps;
        }
        let px = x - self.cx;
        let py = y - self.cy;
        let radius2 = self.r * self.r;
        is_equal_eps(px * px / radius2 + py * py / radius2, 1.0, eps)
    }

    /// `GeoConicND.distance(x, y, closestPoint)`: distance to the closest point
    /// of the conic found by `pointChangedUnlimited`.
    fn distance(&self, x: f64, y: f64) -> f64 {
        if !self.is_circle() {
            return my_length(x - self.cx, y - self.cy);
        }
        // Eigenvectors of a circle are (1, 0) and (-0, 1).
        let px = x - self.cx;
        let py = y - self.cy;
        let (ex, ey) = (px * 1.0 + py * 0.0, px * -0.0 + py * 1.0);
        let t = (self.r * ey).atan2(self.r * ex);
        let (qx, qy) = (self.r * t.cos(), self.r * t.sin());
        let rx = qx * 1.0 + qy * -0.0 + self.cx;
        let ry = qx * 0.0 + qy * 1.0 + self.cy;
        my_length(x - rx, y - ry)
    }
}

/// `GeoConicND.evaluate(matrix, x, y)`.
fn evaluate(m: &[f64; 6], x: f64, y: f64) -> f64 {
    m[2] + m[4] * x + m[5] * y + y * (m[5] + m[3] * x + m[1] * y) + x * (m[4] + m[0] * x + m[3] * y)
}

/// `AlgoIntersectLineConic.intersectLineConic(double[] g, double[] A, ...)`
/// for a conic that is not a double line. `g` holds normalised coefficients.
fn line_conic_points(g: [f64; 3], m: &[f64; 6], eps: f64) -> [Pt; 2] {
    let [x, y, z] = g;
    // an arbitrary point of the line
    let (px, py) = if x.abs() > y.abs() {
        (-z / x, 0.0)
    } else {
        (0.0, -z / y)
    };
    // u t² + 2 d t + w = 0 for X = p + t (y, -x)
    let sv_x = m[0] * y - m[3] * x;
    let sv_y = m[3] * y - m[1] * x;
    let u = y * sv_x - x * sv_y;
    let d = px * sv_x + py * sv_y + m[4] * y - m[5] * x;
    let w = evaluate(m, px, py);
    let delta = java_min(
        MIN_PRECISION,
        java_max(1.0, (2.0 * d).abs() + u.abs() + w.abs()) * eps,
    );

    if is_zero_eps(u, eps) {
        if is_zero_eps(d, eps) {
            return [None, None]; // the line lies on the conic or is an asymptote
        }
        let t0 = -w / (2.0 * d);
        let p = point(px + t0 * y, py - t0 * x);
        return if d < 0.0 { [p, None] } else { [None, p] };
    }
    let mut dis = d * d - u * w;
    if is_equal_eps(dis, 0.0, delta) {
        // tangent
        let t1 = -d / u;
        let p = point(px + t1 * y, py - t1 * x);
        return [p, p];
    }
    if dis > 0.0 {
        dis = dis.sqrt();
        // one root from (-d ± dis) / u, the other from w / (-d ± dis),
        // with the sign that makes d + dis larger in magnitude
        let swap = d < 0.0;
        if swap {
            dis = -dis;
        }
        let q = -(d + dis);
        let t1 = if swap { w / q } else { q / u };
        let t2 = if swap { q / u } else { w / q };
        return [
            point(px + t1 * y, py - t1 * x),
            point(px + t2 * y, py - t2 * x),
        ];
    }
    [None, None]
}

/// `AlgoIntersectLineConic.intersect`: retries with a growing epsilon until
/// one point lies on both the line and the conic.
fn intersect_line_conic(g: [f64; 3], conic: &Conic) -> [Pt; 2] {
    let normalized = normalized_coefficients(g);
    let mut eps = 1e-15;
    while eps <= MIN_PRECISION {
        let mut sol = line_conic_points(normalized, &conic.m, eps);
        // testPoints
        let mut found = false;
        for p in sol.iter_mut() {
            if let Some(q) = *p {
                if conic.on_full_conic(q, MIN_PRECISION) && on_full_line(g, q, MIN_PRECISION) {
                    found = true;
                } else {
                    *p = None;
                }
            }
        }
        if found {
            return sol;
        }
        eps *= 10.0;
    }
    [None, None]
}

/// `AlgoIntersectConics.testPoints` for two circles.
fn test_conic_points(a: &Conic, b: &Conic, points: &mut [Pt; 2], eps: f64) -> bool {
    let mut found = false;
    for p in points.iter_mut() {
        let Some((x, y)) = *p else { continue };
        let mut q = (x, y);
        // a nearby decimal fraction replaces the point if it is at least as
        // close to both conics
        let x2 = check_decimal_fraction(x);
        let y2 = check_decimal_fraction(y);
        if (x != x2 || y != y2)
            && b.distance(x, y) >= b.distance(x2, y2)
            && a.distance(x, y) >= a.distance(x2, y2)
        {
            q = (x2, y2);
        }
        if a.on_full_conic(q, eps) && b.on_full_conic(q, eps) {
            *p = Some(q);
            found = true;
        } else {
            *p = None;
        }
    }
    found
}

/// `AlgoIntersectConics.intersectConicsWithEqualSubmatrixS`: intersects the
/// radical line with each circle in turn.
fn intersect_equal_submatrix(a: &Conic, b: &Conic, points: &mut [Pt; 2], eps: f64) -> bool {
    let (c1, c2) = (&a.m, &b.m);
    let line = [
        2.0 * (c1[4] * c2[0] - c1[0] * c2[4]),
        2.0 * (c1[5] * c2[0] - c1[0] * c2[5]),
        c1[2] * c2[0] - c1[0] * c2[2],
    ];
    let normalized = normalized_coefficients(line);
    *points = line_conic_points(normalized, c1, eps);
    if test_conic_points(a, b, points, MIN_PRECISION) {
        return true;
    }
    *points = line_conic_points(normalized, c2, eps);
    test_conic_points(a, b, points, MIN_PRECISION)
}

/// `AlgoIntersectConics.intersectConics` for two circles, with the defined
/// points moved to the front as in the non-continuous mode.
fn intersect_conics(a: &Conic, b: &Conic) -> [Pt; 2] {
    let mut points = [None, None];
    let mut ok = false;
    let mut eps = MAX_PRECISION;
    while !ok && eps <= MIN_PRECISION {
        ok = intersect_equal_submatrix(a, b, &mut points, eps);
        eps *= 10.0;
    }
    if !ok {
        return [None, None];
    }
    if points[0].is_none() {
        points = [points[1], None];
    }
    points
}

/// The intersection of two circles through a common point, computed by
/// `AlgoIntersectConics.handleSpecialCase` without that point.
fn shared_point_case(a: &Conic, b: &Conic) -> [Pt; 2] {
    let (r1, x1, y1) = (a.r, a.cx, a.cy);
    let (r2, x2, y2) = (b.r, b.cx, b.cy);
    let dist = (x1 - x2).hypot(y1 - y2);
    if is_zero(r1 + r2 - dist) {
        // touching from outside
        [
            hom_point(r1 * x2 + r2 * x1, r1 * y2 + r2 * y1, r1 + r2),
            None,
        ]
    } else if is_zero((r1 - r2).abs() - dist) {
        // touching from inside
        [
            hom_point(r1 * x2 - r2 * x1, r1 * y2 - r2 * y1, r1 - r2),
            None,
        ]
    } else {
        let mut q = [None, None];
        intersect_equal_submatrix(a, b, &mut q, STANDARD_PRECISION);
        q
    }
}

/// `AlgoIntersectConics.matchExistingIntersections`: a computed point that
/// is close to a point on both circles by construction becomes that point.
fn match_existing_intersections(c: &Construction, c1: Id, c2: Id, p: &mut [Pt; 2]) {
    let existing = circle_points_on_other(c, c1, c2);
    if existing.iter().all(Option::is_none) || p.iter().all(Option::is_none) {
        return;
    }
    let gap = match (p[0], p[1]) {
        (Some((x1, y1)), Some((x2, y2))) => my_length(x1 - x2, y1 - y2),
        _ => f64::INFINITY,
    };
    let mut closest = None;
    for slot in p.iter_mut() {
        let Some((x, y)) = *slot else { continue };
        let mut min_distance = f64::INFINITY;
        for s in existing.iter().flatten() {
            if let Some((sx, sy)) = c.point(*s) {
                let d = ((x - sx) * (x - sx) + (y - sy) * (y - sy)).sqrt();
                if d < min_distance {
                    min_distance = d;
                    closest = Some((sx, sy));
                }
            }
        }
        // DoubleUtil.isGreaterEqual(gap / 2, minDistance)
        if gap / 2.0 + STANDARD_PRECISION > min_distance {
            *slot = closest;
        }
    }
}

// ----------------------------------------------- incidence by construction

/// The points a line was drawn through (`AlgoJoinPoints`,
/// `AlgoOrthoLinePointLine` and `AlgoLinePointLine` add them to the line's
/// incidence).
fn line_points(line_def: Def) -> (Option<Id>, Option<Id>) {
    match line_def {
        Def::Line(p, q) => (Some(p), Some(q)),
        Def::Perpendicular { through, .. } | Def::Parallel { through, .. } => (Some(through), None),
        _ => (None, None),
    }
}

/// Whether `s` is an intersection point of `line` and another object.
fn intersection_on_line(c: &Construction, s: Id, line: Id) -> bool {
    match c.def(s) {
        Def::IntersectLines(l, m) => l == line || m == line,
        Def::IntersectLineCircle { line: l, .. } => l == line,
        _ => false,
    }
}

/// Whether `s` is an intersection point of `circle` and another object.
fn intersection_on_circle(c: &Construction, s: Id, circle: Id) -> bool {
    match c.def(s) {
        Def::IntersectLineCircle { circle: k, .. } => k == circle,
        Def::IntersectCircles { c1, c2, .. } => c1 == circle || c2 == circle,
        _ => false,
    }
}

/// The point a circle was drawn through.
fn circle_through(c: &Construction, circle: Id) -> Option<Id> {
    match c.def(circle) {
        Def::CircleThrough { through, .. } => Some(through),
        _ => None,
    }
}

/// Whether `s` lies on `circle` by construction.
fn on_circle_by_construction(c: &Construction, s: Id, circle: Id) -> bool {
    circle_through(c, circle) == Some(s) || intersection_on_circle(c, s, circle)
}

/// The point that `AlgoIntersectLineConic.handleSpecialCase` makes
/// intersection 1: the first point on the circle by construction (the point
/// it was drawn through, then its intersection points in creation order)
/// that also lies on the line by construction. The intersection points of
/// this line and this circle themselves belong to this intersection and are
/// never chosen.
fn existing_line_circle_point(c: &Construction, line: Id, line_def: Def, circle: Id) -> Option<Id> {
    let (p, q) = line_points(line_def);
    if let Some(t) = circle_through(c, circle) {
        if Some(t) == p || Some(t) == q || intersection_on_line(c, t, line) {
            return Some(t);
        }
    }
    // Any other point on both is an intersection point of the circle that
    // the line was drawn through.
    let on = |s: Option<Id>| s.filter(|&s| intersection_on_circle(c, s, circle));
    match (on(p), on(q)) {
        (Some(p), Some(q)) => Some(p.min(q)),
        (p, q) => p.or(q),
    }
}

/// The points on circle `a` whose incidence list contains circle `b`, in the
/// order of `a`'s list: the point `a` was drawn through if it lies on `b` by
/// construction, then the point `b` was drawn through if it is an
/// intersection point of `a`.
fn circle_points_on_other(c: &Construction, a: Id, b: Id) -> [Option<Id>; 2] {
    let first = circle_through(c, a).filter(|&t| on_circle_by_construction(c, t, b));
    let second = circle_through(c, b)
        .filter(|&t| intersection_on_circle(c, t, a))
        .filter(|&t| Some(t) != first);
    [first, second]
}

/// The point that `AlgoIntersectConics.handleSpecialCase` makes
/// intersection 1, for two circles (not single points) through a common
/// point by construction.
fn shared_circles_point(c: &Construction, c1: Id, c2: Id, a: &Conic, b: &Conic) -> Option<Id> {
    if !(a.is_circle() && b.is_circle()) {
        return None;
    }
    // getPointFrom1on2: only the first candidate is checked numerically.
    let check = |s: Id| {
        let p = c.point(s)?;
        (a.on_full_conic(p, STANDARD_PRECISION) && b.on_full_conic(p, STANDARD_PRECISION))
            .then_some(s)
    };
    let from = |x: Id, y: Id| {
        let [first, second] = circle_points_on_other(c, x, y);
        first.or(second).and_then(check)
    };
    from(c1, c2).or_else(|| from(c2, c1))
}

#[cfg(test)]
mod tests {
    use crate::kernel::{Construction, Value};
    use crate::script::load;

    fn xy(c: &Construction, label: &str) -> Option<(f64, f64)> {
        match c.value(c.id(label).unwrap()) {
            Value::Point { x, y } => Some((x, y)),
            Value::Undefined => None,
            other => panic!("{label} is not a point: {other:?}"),
        }
    }

    fn close(p: Option<(f64, f64)>, x: f64, y: f64) -> bool {
        matches!(p, Some((px, py)) if (px - x).abs() < 1e-9 && (py - y).abs() < 1e-9)
    }

    fn mv(c: &mut Construction, label: &str, x: f64, y: f64) {
        c.move_point(c.id(label).unwrap(), x, y).unwrap();
    }

    const LINE_CIRCLE: &str = "A = (0, 0)\nr = 5\nk = Circle(A, r)\n\
        C = (-10, 3)\nD = (10, 3)\nf = Line(C, D)\n\
        P = Intersect(f, k, 1)\nQ = Intersect(f, k, 2)\n";

    #[test]
    fn line_points_follow_the_line_direction() {
        let mut c = load(LINE_CIRCLE).unwrap();
        assert!(close(xy(&c, "P"), -4.0, 3.0));
        assert!(close(xy(&c, "Q"), 4.0, 3.0));
        // Swapping the two points reverses the line and the order.
        mv(&mut c, "C", 10.0, 3.0);
        mv(&mut c, "D", -10.0, 3.0);
        assert!(close(xy(&c, "P"), 4.0, 3.0));
        assert!(close(xy(&c, "Q"), -4.0, 3.0));
    }

    #[test]
    fn a_tangent_point_is_only_the_first_intersection() {
        let mut c = load(LINE_CIRCLE).unwrap();
        mv(&mut c, "C", -10.0, 5.0);
        mv(&mut c, "D", 10.0, 5.0);
        assert!(close(xy(&c, "P"), 0.0, 5.0));
        assert_eq!(xy(&c, "Q"), None);
        mv(&mut c, "C", -10.0, 5.5);
        mv(&mut c, "D", 10.0, 5.5);
        assert_eq!(xy(&c, "P"), None);
        assert_eq!(xy(&c, "Q"), None);
    }

    #[test]
    fn a_point_on_line_and_circle_by_construction_comes_first() {
        let mut c = load(
            "A = (0, 0)\nB = (3, 4)\nC = (-3, -4)\nk = Circle(A, B)\nf = Line(C, B)\n\
             P = Intersect(f, k, 1)\nQ = Intersect(f, k, 2)\n",
        )
        .unwrap();
        // Along the line C comes before B, but B is on k by construction.
        assert!(close(xy(&c, "P"), 3.0, 4.0));
        assert!(close(xy(&c, "Q"), -3.0, -4.0));
        // Tangent at B: both are B.
        mv(&mut c, "C", 7.0, 1.0);
        assert!(close(xy(&c, "P"), 3.0, 4.0));
        assert!(close(xy(&c, "Q"), 3.0, 4.0));
    }

    #[test]
    fn a_free_point_on_the_circle_is_not_preferred() {
        let c = load(
            "A = (0, 0)\nr = 5\nk = Circle(A, r)\nB = (3, 4)\nC = (-3, -4)\nf = Line(C, B)\n\
             P = Intersect(f, k, 1)\nQ = Intersect(f, k, 2)\n",
        )
        .unwrap();
        assert!(close(xy(&c, "P"), -3.0, -4.0));
        assert!(close(xy(&c, "Q"), 3.0, 4.0));
    }

    const CIRCLES: &str = "A = (0, 0)\nB = (8, 0)\nr = 5\nq = 5\n\
        k = Circle(A, r)\nc = Circle(B, q)\nP = Intersect(k, c, 1)\nQ = Intersect(k, c, 2)\n";

    #[test]
    fn circle_points_are_left_then_right_of_the_centres() {
        let mut c = load(CIRCLES).unwrap();
        assert!(close(xy(&c, "P"), 4.0, 3.0));
        assert!(close(xy(&c, "Q"), 4.0, -3.0));
        mv(&mut c, "B", -8.0, 0.0);
        assert!(close(xy(&c, "P"), -4.0, -3.0));
        assert!(close(xy(&c, "Q"), -4.0, 3.0));
    }

    #[test]
    fn touching_circles_have_one_intersection() {
        let mut c = load(CIRCLES).unwrap();
        mv(&mut c, "B", 10.0, 0.0);
        assert!(close(xy(&c, "P"), 5.0, 0.0));
        assert_eq!(xy(&c, "Q"), None);
        // Same centre: no intersection, whether the radii agree or not.
        mv(&mut c, "B", 0.0, 0.0);
        assert_eq!(xy(&c, "P"), None);
        c.set_number(c.id("q").unwrap(), 3.0).unwrap();
        assert_eq!(xy(&c, "P"), None);
    }

    #[test]
    fn identical_circles_through_a_common_point_meet_there() {
        let mut c = load(
            "A = (0, 0)\nB = (3, 4)\nC = (6, 0)\nk = Circle(A, B)\nc = Circle(C, B)\n\
             P = Intersect(k, c, 1)\nQ = Intersect(k, c, 2)\n",
        )
        .unwrap();
        assert!(close(xy(&c, "P"), 3.0, 4.0));
        assert!(close(xy(&c, "Q"), 3.0, -4.0));
        mv(&mut c, "C", 0.0, 0.0);
        assert!(close(xy(&c, "P"), 3.0, 4.0));
        assert_eq!(xy(&c, "Q"), None);
    }

    #[test]
    fn the_common_point_is_followed_when_the_circles_keep_their_values() {
        // c = Circle(M, S) with M = A is k itself; moving the line moves S
        // along k, so k and c keep their values while P must follow S.
        let mut c = load(
            "A = (0, 0)\nB = (5, 0)\nk = Circle(A, B)\nG = (-9, 3)\nH = (9, 3)\n\
             h = Line(G, H)\nS = Intersect(h, k, 1)\nM = (0, 0)\nc = Circle(M, S)\n\
             P = Intersect(k, c, 1)\nQ = Intersect(k, c, 2)\n",
        )
        .unwrap();
        assert!(close(xy(&c, "P"), -4.0, 3.0));
        mv(&mut c, "G", -9.0, -3.0);
        mv(&mut c, "H", 9.0, -3.0);
        assert!(close(xy(&c, "S"), -4.0, -3.0));
        assert!(close(xy(&c, "P"), -4.0, -3.0));
        assert_eq!(xy(&c, "Q"), None);
    }
}
