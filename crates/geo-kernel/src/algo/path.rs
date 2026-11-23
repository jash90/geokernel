// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/PathParameter.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/PathNormalizer.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/PathMover.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPointOnPath.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDispatcher.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoClosestPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDistancePointObject.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoPathParameter.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoJoinPoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLinePointLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoOrthoLinePointLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLinePointVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoOrthoLinePointVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLineBisector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLineBisectorSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAngularBisectorPoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAngularBisectorLines.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTangentPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTangentLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAxes.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoCurveCartesian.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoRay.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPolyLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec3D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicPartParameters.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Points on paths: `Point(path)`, `Point(path, t)` and `PathParameter(P)`.
//!
//! A point on a path keeps state between updates, as GeoGebra's `GeoPoint`
//! does: its coordinates, its path parameter and the copy of the parameter
//! that `GeoPoint.setCoords` keeps for the case that the point becomes
//! undefined. That state is a [`PathState`], kept by the construction in a
//! side table (see [`Construction::path_state`]); the definition stays a
//! small handle. Moving a point made by `Point(path)` is `setCoords` on the
//! path: the point is projected onto the path and its parameter is stored
//! (`Path.pointChanged`). When the path changes, the point follows its
//! stored parameter (`Path.pathChanged`, called by `AlgoPointOnPath`).

use super::arc::ArcData;
use super::conic_nd::{my_length, ConicData, ConicType};
use super::polygon::Count;
use super::{join, Group, Parents, POINT};
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};
use crate::script::object;
use crate::tol::STANDARD_PRECISION;

/// `PathMover.OPEN_BORDER_OFFSET`.
const OPEN_BORDER_OFFSET: f64 = 1e-5;

/// The kinds of objects a point can be put on. A conic is a path in
/// GeoGebra whatever its type (`GeoConicND` implements `Path`; the behaviour
/// per type is [`super::conic_path`]).
pub(crate) const PATH: &[Kind] = &[
    Kind::Line,
    Kind::Segment,
    Kind::Ray,
    Kind::Circle,
    Kind::Conic,
    Kind::Polygon,
    Kind::PolyLine,
    Kind::Arc,
    Kind::Sector,
    Kind::Curve,
];
const NUMBER: &[Kind] = &[Kind::Number];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathDef {
    /// `PointIn(region)`, with parameters for the interior or boundary.
    InRegion { region: Id },
    /// The second endpoint of `Segment(A, length)` on its internal circle.
    FixedEndpoint { start: Id, length: Count },
    /// A rigid polygon's movable second point on a fixed-radius circle.
    RigidEndpoint {
        anchor: Id,
        radius: f64,
        bearing: f64,
    },
    /// `Point(path)`: a point on `path` that the user can move along it.
    On { path: Id },
    /// `Point(path, t)`: the point at the parameter `t` from 0 to 1 along
    /// `path` (`AlgoPointOnPath` with a parameter); it cannot be moved.
    OnParam { path: Id, param: Id },
    /// `PathParameter(P)`: the parameter of a point on a path, from 0 to 1.
    Parameter { point: Id },
}

/// What a point on a path remembers between updates: GeoGebra's
/// homogeneous coordinates of the point, its `PathParameter` (the value `t`
/// and the type of path it was computed for, -1 for none) and the second
/// `PathParameter` that `GeoPoint.setCoords` keeps to restore the first
/// when the point becomes undefined.
///
/// Read it with [`Construction::path_state`] and give it back with
/// [`Construction::restore_path_state`] to rebuild a construction whose
/// points on paths were moved. Two states are equal when they are the same
/// bit for bit (NaN included).
#[derive(Clone, Copy, Debug)]
pub struct PathState {
    x: f64,
    y: f64,
    z: f64,
    t: f64,
    ty: i32,
    temp_t: f64,
    temp_ty: i32,
    region: RegionState,
    function_bounds: Option<[f64; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RegionState {
    Boundary,
    PartEdge,
    Conic(f64, f64),
    Polygon {
        second: usize,
        third: usize,
        u: f64,
        v: f64,
    },
}

impl RegionState {
    fn same(self, other: Self) -> bool {
        let same = |a: f64, b: f64| a.to_bits() == b.to_bits();
        match (self, other) {
            (Self::Boundary, Self::Boundary) | (Self::PartEdge, Self::PartEdge) => true,
            (Self::Conic(u, v), Self::Conic(x, y)) => same(u, x) && same(v, y),
            (
                Self::Polygon {
                    second,
                    third,
                    u,
                    v,
                },
                Self::Polygon {
                    second: s,
                    third: t,
                    u: x,
                    v: y,
                },
            ) => second == s && third == t && same(u, x) && same(v, y),
            _ => false,
        }
    }
}

impl PathState {
    /// A new point on a path before it is placed: `new GeoPoint` with a
    /// path parameter of 0 (`GeoPoint.getPathParameter`) and the copy not
    /// yet set (`new PathParameter()`, NaN).
    fn new() -> Self {
        PathState {
            x: 0.0,
            y: 0.0,
            z: 1.0,
            t: 0.0,
            ty: -1,
            temp_t: f64::NAN,
            temp_ty: -1,
            region: RegionState::Boundary,
            function_bounds: None,
        }
    }

    /// Restore the coordinates before any dependent is constructed.
    pub(crate) fn saved(
        c: &Construction,
        path: Id,
        x: f64,
        y: f64,
        bounds: Option<[f64; 2]>,
    ) -> Self {
        let mut state = Self::new();
        state.function_bounds = bounds;
        state.set_coords(&geom(c, path), x, y);
        state
    }

    /// Restore a curve point from its saved parameter, which identifies its
    /// branch even when multiple parameters share the same coordinates.
    pub(crate) fn saved_curve(c: &Construction, curve: Id, t: f64) -> Self {
        let mut state = Self::new();
        state.t = t;
        state.temp_t = t;
        geom(c, curve).path_changed(&mut state);
        state
    }

    /// The raw path parameter, in the range of the path (not normalised).
    pub fn parameter(&self) -> f64 {
        self.t
    }

    /// Whether the path parameter and the type of path it was computed for
    /// are the same as in `o`, bit for bit.
    pub fn same_parameter(&self, o: &PathState) -> bool {
        self.t.to_bits() == o.t.to_bits() && self.ty == o.ty
    }

    /// Bit equality, so that a change from `0` to `-0` or between two NaNs
    /// with other payloads still counts as a change.
    pub(crate) fn same(&self, o: &PathState) -> bool {
        let b = |a: f64, b: f64| a.to_bits() == b.to_bits();
        b(self.x, o.x)
            && b(self.y, o.y)
            && b(self.z, o.z)
            && b(self.t, o.t)
            && self.ty == o.ty
            && b(self.temp_t, o.temp_t)
            && self.temp_ty == o.temp_ty
            && self.region.same(o.region)
            && self.function_bounds == o.function_bounds
    }

    /// Whether every field is a number or NaN that the state can hold:
    /// what [`Construction::restore_path_state`] accepts.
    pub(crate) fn is_valid(&self) -> bool {
        // any bit pattern is a state GeoGebra could hold, but the type is one
        // of the conic types or -1
        [self.ty, self.temp_ty]
            .iter()
            .all(|&t| t == -1 || (1..=9).contains(&t) || t == 32)
    }

    /// `GeoPoint.isDefined` after `updateCoords`: no coordinate is NaN (a
    /// point at infinity is defined).
    pub(crate) fn is_defined(&self) -> bool {
        let infinite = !self.z.is_nan() && is_epsilon(self.z, self.x, self.y);
        if infinite {
            !(self.x.is_nan() || self.y.is_nan())
        } else {
            !(self.x.is_nan() || self.y.is_nan() || self.z.is_nan())
        }
    }

    /// `GeoPoint.updateCoords`: a finite point gets a positive `z`.
    pub(crate) fn update_coords(&mut self) {
        let infinite = !self.z.is_nan() && is_epsilon(self.z, self.x, self.y);
        if !infinite && self.is_defined() && self.z < 0.0 {
            self.x = -self.x;
            self.y = -self.y;
            self.z = -self.z;
        }
    }

    /// The value of the point: undefined when a coordinate is NaN, when the
    /// point is at infinity and when a coordinate is infinite (the end of a
    /// line at the parameter 0 or 1), which the kernel does not represent.
    pub(crate) fn value(&self) -> Option<Value> {
        let (x, y, z) = (self.x, self.y, self.z);
        if x.is_nan() || y.is_nan() || z.is_nan() || is_epsilon(z, x, y) {
            return None;
        }
        let (x, y) = if z == 1.0 { (x, y) } else { (x / z, y / z) };
        (x.is_finite() && y.is_finite()).then_some(Value::Point { x, y })
    }

    /// The homogeneous coordinates.
    pub(crate) fn coords(&self) -> (f64, f64, f64) {
        (self.x, self.y, self.z)
    }

    /// The type of path the parameter was computed for (`getPathType`).
    pub(crate) fn path_type(&self) -> i32 {
        self.ty
    }

    pub(crate) fn set_type(&mut self, ty: i32) {
        self.ty = ty;
    }

    pub(crate) fn set_coords_2d(&mut self, x: f64, y: f64, z: f64) {
        self.x = x;
        self.y = y;
        self.z = z;
    }

    /// `PathParameter.setT`: NaN is ignored.
    pub(crate) fn set_t(&mut self, t: f64) {
        if !t.is_nan() {
            self.t = t;
        }
    }

    /// A direct write `pp.t = t`, which keeps NaN (unlike [`Self::set_t`]).
    pub(crate) fn put_t(&mut self, t: f64) {
        self.t = t;
    }

    /// `P.setX(x)` alone, as `GeoConicPart.pathChanged` does for an
    /// undefined part.
    pub(crate) fn put_x(&mut self, x: f64) {
        self.x = x;
    }

    /// `GeoPoint.setCoords(x, y, 1)` for a point on a path.
    fn set_coords(&mut self, path: &Geom<'_>, x: f64, y: f64) {
        self.set_coords_hom(path, x, y, 1.0);
    }

    /// `GeoPoint.setCoords(x, y, z)` for a point on a path.
    pub(crate) fn set_coords_hom(&mut self, path: &Geom<'_>, x: f64, y: f64, z: f64) {
        self.set_coords_2d(x, y, z);
        // remember the path parameter for the undefined case
        self.temp_t = self.t;
        self.temp_ty = self.ty;
        path.point_changed(self);
        self.update_coords();
        if self.is_defined() {
            self.temp_t = self.t;
            self.temp_ty = self.ty;
        } else {
            self.t = self.temp_t;
            self.ty = self.temp_ty;
        }
    }

    /// `GeoPoint.setUndefined`: NaN coordinates, and the path parameter of
    /// the last `setCoords`.
    fn set_undefined(&mut self) {
        self.set_coords_2d(f64::NAN, f64::NAN, f64::NAN);
        self.t = self.temp_t;
        self.ty = self.temp_ty;
    }
}

impl PartialEq for PathState {
    fn eq(&self, other: &PathState) -> bool {
        self.same(other)
    }
}

/// `DoubleUtil.isEpsilon(z, x, y)`.
fn is_epsilon(e: f64, x: f64, y: f64) -> bool {
    let ea = e.abs();
    !(ea > STANDARD_PRECISION
        || ea > x.abs() * STANDARD_PRECISION
        || ea > y.abs() * STANDARD_PRECISION)
}

fn is_zero(v: f64) -> bool {
    -STANDARD_PRECISION < v && v < STANDARD_PRECISION
}

/// The parameter that the full conic `d` gives a temporary point at
/// `(x, y)`: `pointChanged` (`pointChangedUnlimited` for a full conic) on a
/// new point. With `on_path` the point lies on `d` first, as a point whose
/// path is `d` does when `setCoords` projects it (`AlgoConicPartConicPoints`
/// calls `setCoords` and then `pointChangedUnlimited`).
pub(crate) fn conic_parameter(d: &ConicData, point: (f64, f64), on_path: bool) -> f64 {
    conic_parameter_from(d, point, on_path, 0.0)
}

/// The same projection with the saved parameter of a reused temporary point.
pub(super) fn conic_parameter_from(
    d: &ConicData,
    (x, y): (f64, f64),
    on_path: bool,
    parameter: f64,
) -> f64 {
    let mut st = PathState::new();
    st.set_t(parameter);
    if on_path {
        st.set_coords(&Geom::Conic(*d), x, y);
    } else {
        st.set_coords_2d(x, y, 1.0);
    }
    super::conic_path::point_changed(d, &mut st);
    st.t
}

/// `PathNormalizer.infFunction`.
pub(crate) fn inf_function(t: f64) -> f64 {
    t / (1.0 - t.abs())
}

/// `PathNormalizer.inverseInfFunction`.
pub(crate) fn inverse_inf_function(z: f64) -> f64 {
    if z == f64::NEG_INFINITY {
        return -1.0 + OPEN_BORDER_OFFSET;
    }
    if z >= 0.0 {
        return z / (1.0 + z);
    }
    z / (1.0 - z)
}

/// `PathNormalizer.toParentPathParameter`.
fn to_parent(parameter: f64, min: f64, max: f64) -> f64 {
    let mut tn = parameter;
    // for points as paths (min = max = 0)
    if min == max {
        return min;
    }
    // NaN stays NaN, as in the upstream if-chain
    tn = tn.clamp(0.0, 1.0);
    if min == f64::NEG_INFINITY {
        if max == f64::INFINITY {
            return inf_function(2.0 * tn - 1.0);
        }
        return max + inf_function(tn - 1.0);
    }
    if max == f64::INFINITY {
        return min + inf_function(tn);
    }
    (1.0 - tn) * min + tn * max
}

/// `PathNormalizer.toNormalizedPathParameter`.
fn to_normalized(p: f64, min: f64, max: f64) -> f64 {
    let mut t = p;
    if min == max {
        return 0.0;
    }
    if t < min {
        t = min;
    } else if t > max {
        t = max;
    }
    if min == f64::NEG_INFINITY {
        if max == f64::INFINITY {
            if t == f64::NEG_INFINITY {
                return 0.0;
            } else if t == f64::INFINITY {
                return 1.0;
            }
            return 0.5 + 0.5 * inverse_inf_function(t);
        }
        if t == f64::NEG_INFINITY {
            return 0.0;
        }
        return 1.0 + inverse_inf_function(t - max);
    }
    if max == f64::INFINITY {
        return inverse_inf_function(t - min);
    }
    (t - min) / (max - min)
}

/// A path as the path code reads it, taken from the construction for one
/// update.
// a conic is read for one update on the stack; boxing it would allocate on
// every recomputation of a point on a path
#[allow(clippy::large_enum_variant)]
pub(crate) enum Geom<'a> {
    /// The path is undefined (`input[0].isDefined()` is false).
    Undefined,
    /// A `GeoLine` with its raw coefficients and its start point, in
    /// homogeneous coordinates, if the algorithm that made it sets one.
    Line {
        g: (f64, f64, f64),
        start: Option<(f64, f64, f64)>,
    },
    Ray {
        g: (f64, f64, f64),
        start: (f64, f64),
    },
    Segment(Seg),
    Conic(ConicData),
    /// A polygon with `n` defined vertices, read from the construction.
    Polygon {
        c: &'a Construction,
        id: Id,
        n: usize,
    },
    PolyLine {
        c: &'a Construction,
        id: Id,
        n: usize,
    },
    /// An arc or sector (`GeoConicPart`) with its record.
    Arc(ArcData),
    Function {
        c: &'a Construction,
        id: Id,
    },
    Curve {
        c: &'a Construction,
        id: Id,
        start: f64,
        end: f64,
    },
}

/// A `GeoSegment`: its line through the start and end point
/// (`GeoVec3D.lineThroughPoints`) and whether its length is zero
/// (`GeoSegment.calcLength`).
#[derive(Clone, Copy)]
pub(crate) struct Seg {
    g: (f64, f64, f64),
    start: (f64, f64),
    end: (f64, f64),
    zero_length: bool,
}

impl Seg {
    fn new(start: (f64, f64), end: (f64, f64)) -> Self {
        let length = my_length(end.0 - start.0, end.1 - start.1);
        Seg {
            g: join(start, end),
            start,
            end,
            zero_length: length == 0.0 || is_zero(length),
        }
    }

    /// `GeoSegment.pointChanged`.
    fn point_changed(&self, p: &mut PathState) {
        // special case: segment of length 0
        if self.zero_length {
            p.set_coords_2d(self.start.0, self.start.1, 1.0);
            if !(p.t >= 0.0 && p.t <= 1.0) {
                p.t = 0.0;
            }
            return;
        }
        line_point_changed(self.g, Some((self.start.0, self.start.1, 1.0)), p);
        // keep the point on the segment: 0 <= t <= 1
        if p.t < 0.0 {
            p.set_coords_2d(self.start.0, self.start.1, 1.0);
            p.t = 0.0;
        } else if p.t > 1.0 {
            p.set_coords_2d(self.end.0, self.end.1, 1.0);
            p.t = 1.0;
        }
    }

    /// `GeoSegment.pathChanged`.
    fn path_changed(&self, p: &mut PathState) {
        if self.zero_length {
            p.set_coords_2d(self.start.0, self.start.1, 1.0);
            if !(p.t >= 0.0 && p.t <= 1.0) {
                p.t = 0.0;
            }
            return;
        }
        p.t = p.t.clamp(0.0, 1.0);
        let (x, y) = self.point_at(p.t);
        p.set_coords_2d(x, y, 1.0);
    }

    /// `GeoSegment.getPointX` and `getPointY`.
    fn point_at(&self, t: f64) -> (f64, f64) {
        (self.start.0 + t * self.g.1, self.start.1 - t * self.g.0)
    }
}

/// `GeoLine.doPointChanged` (`projectCoordsAndComputePathParam`): projects
/// the point onto the line and sets its parameter from the start point, or
/// from the foot of the perpendicular from the origin when there is none.
pub(crate) fn line_point_changed(
    (x, y, z): (f64, f64, f64),
    start: Option<(f64, f64, f64)>,
    p: &mut PathState,
) {
    let mut px = p.x / p.z;
    let mut py = p.y / p.z;
    // parameter of the projection on the perpendicular line
    let t = -(z + x * px + y * py) / (x * x + y * y);
    if x == 0.0 && px.is_infinite() && py.is_finite() {
        py = -z / y;
    } else if y == 0.0 && py.is_infinite() && px.is_finite() {
        px = -z / x;
    } else {
        px += t * x;
        py += t * y;
    }
    p.set_coords_2d(px, py, 1.0);
    let (spx, spy, spz) = match start {
        Some(s) => s,
        None => {
            let (mut spx, mut spy) = (0.0, 0.0);
            if x != 0.0 && y != 0.0 {
                spx = -z * x / (x * x + y * y);
                spy = -z * y / (x * x + y * y);
            } else if x != 0.0 {
                spx = -z / x;
            } else if y != 0.0 {
                spy = -z / y;
            }
            (spx, spy, 1.0)
        }
    };
    p.t = if x.abs() <= y.abs() {
        (spz * px - spx) / (y * spz)
    } else {
        (spy - spz * py) / (x * spz)
    };
}

/// Project a finite point onto the carrier and read the limited path's parameter.
pub(super) fn line_parameter_at(g: (f64, f64, f64), start: (f64, f64), point: (f64, f64)) -> f64 {
    let mut state = PathState::new();
    state.set_coords_2d(point.0, point.1, 1.0);
    line_point_changed(g, Some((start.0, start.1, 1.0)), &mut state);
    state.parameter()
}

/// `GeoLine.pathChanged(Coords, PathParameter)`.
pub(crate) fn line_path_changed(
    (x, y, z): (f64, f64, f64),
    start: Option<(f64, f64, f64)>,
    p: &mut PathState,
) {
    // if t is infinite and x or y is 0, avoid 0 * inf
    let dx = if y == 0.0 { 0.0 } else { p.t * y };
    let dy = if x == 0.0 { 0.0 } else { -p.t * x };
    if dx.is_infinite() && dy.is_infinite() {
        let sgn = if p.t > 0.0 { 1.0 } else { -1.0 };
        p.set_coords_2d(sgn * y, -sgn * x, 0.0);
    } else if let Some((sx, sy, sz)) = start {
        // `GeoPoint.inhomX`
        let (ix, iy) = if sz == 1.0 {
            (sx, sy)
        } else {
            (sx / sz, sy / sz)
        };
        p.set_coords_2d(ix + dx, iy + dy, 1.0);
    } else {
        let (mut ix, mut iy) = (0.0, 0.0);
        if x != 0.0 && y != 0.0 {
            ix = -z * x / (x * x + y * y);
            iy = -z * y / (x * x + y * y);
        } else if x != 0.0 {
            ix = -z / x;
        } else if y != 0.0 {
            iy = -z / y;
        }
        p.set_coords_2d(ix + dx, iy + dy, 1.0);
    }
}

impl Geom<'_> {
    /// `isDefined` of the path. An arc whose value is not finite (two rays,
    /// a sector of a degenerate conic) is defined, as `GeoConicPart.isDefined`
    /// says.
    pub(crate) fn is_defined(&self) -> bool {
        match self {
            Geom::Undefined => false,
            Geom::Arc(a) => a.defined,
            // a conic that is undefined still carries its record, which a
            // move of a point on it projects onto (`Geom::Conic` below)
            Geom::Conic(d) => d.defined,
            _ => true,
        }
    }

    /// `Path.getMinParameter` and `Path.getMaxParameter`.
    pub(crate) fn range(&self) -> (f64, f64) {
        match self {
            Geom::Undefined => (0.0, 0.0),
            Geom::Line { .. } => (f64::NEG_INFINITY, f64::INFINITY),
            Geom::Ray { .. } => (0.0, f64::INFINITY),
            Geom::Segment(_) => (0.0, 1.0),
            Geom::Conic(d) => super::conic_path::range(d),
            Geom::Polygon { n, .. } => (0.0, *n as f64),
            Geom::PolyLine { n, .. } => (0.0, *n as f64 - 1.0),
            Geom::Arc(a) => a.range(),
            Geom::Curve { start, end, .. } => (*start, *end),
            Geom::Function { c, id } => {
                let [min, max, ..] = c
                    .locus_view()
                    .unwrap_or(crate::kernel::locus::HEADLESS_VIEW);
                c.chart(*id)
                    .and_then(|d| d.function_domain())
                    .map_or((min, max), |(a, b)| (a.max(min), b.min(max)))
            }
        }
    }

    /// `Path.pointChanged`: projects the point onto the path and sets its
    /// parameter.
    fn point_changed(&self, p: &mut PathState) {
        match self {
            // an undefined path gives NaN, and `setCoords` keeps the
            // parameter of before
            Geom::Undefined => p.set_coords_2d(f64::NAN, f64::NAN, f64::NAN),
            Geom::Line { g, start } => line_point_changed(*g, *start, p),
            Geom::Ray { g, start } => {
                line_point_changed(*g, Some((start.0, start.1, 1.0)), p);
                // keep the point on the ray: 0 <= t
                if p.t < 0.0 {
                    p.set_coords_2d(start.0, start.1, 1.0);
                    p.t = 0.0;
                }
            }
            Geom::Segment(s) => s.point_changed(p),
            Geom::Conic(d) => super::conic_path::point_changed(d, p),
            &Geom::Polygon { c, id, n } => polygonal_point_changed(c, id, n, true, p),
            &Geom::PolyLine { c, id, n } => polygonal_point_changed(c, id, n, false, p),
            Geom::Arc(a) => a.point_changed(p),
            Geom::Curve { c, id, .. } => {
                let (x, y) = (p.x / p.z, p.y / p.z);
                if let Some(curve) = super::curve::Cartesian::new(c, *id) {
                    // Keep the current branch when its parameter already fits the point.
                    let (px, py) = curve.value(p.t);
                    let distance = (px - x).hypot(py - y);
                    if !distance.is_finite() || distance >= crate::tol::MIN_PRECISION {
                        p.t = curve.closest_from((x, y), p.t);
                    }
                }
                self.path_changed(p);
            }
            Geom::Function { c, id } => {
                let (x, y) = (p.x / p.z, p.y / p.z);
                if let Some((a, b)) = c.chart(*id).and_then(|d| d.function_domain()) {
                    p.t = x.clamp(a, b);
                    self.path_changed(p);
                    return;
                }
                let at_x = c.function_value(*id, x).unwrap_or(f64::NAN);
                // A saved point is already on the graph up to rounding.
                // A drag off a polynomial graph uses the nearest point.
                p.t = if (at_x - y).abs() <= 4.0 * f64::EPSILON * at_x.abs().max(1.0) {
                    x
                } else {
                    closest_function_x(c, *id, x, y).unwrap_or(x)
                };
                self.path_changed(p);
            }
        }
    }

    /// `Path.pathChanged`: puts the point at its parameter.
    pub(crate) fn path_changed(&self, p: &mut PathState) {
        match self {
            Geom::Curve { c, id, start, end } => {
                p.t = p.t.clamp(*start, *end);
                let (x, y) = c.curve_point(*id, p.t).unwrap_or((f64::NAN, f64::NAN));
                p.set_coords_2d(x, y, 1.0);
            }
            Geom::Function { c, id } => {
                if let Some((a, b)) = c.chart(*id).and_then(|d| d.function_domain()) {
                    p.t = p.t.clamp(a, b);
                }
                p.set_coords_2d(p.t, c.function_value(*id, p.t).unwrap_or(f64::NAN), 1.0);
            }
            Geom::Undefined => p.set_coords_2d(f64::NAN, f64::NAN, f64::NAN),
            Geom::Line { g, start } => line_path_changed(*g, *start, p),
            Geom::Ray { g, start } => {
                if p.t < 0.0 {
                    p.t = 0.0;
                }
                p.set_coords_2d(start.0 + p.t * g.1, start.1 - p.t * g.0, 1.0);
            }
            Geom::Segment(s) => s.path_changed(p),
            Geom::Conic(d) => super::conic_path::path_changed(d, p),
            &Geom::Polygon { c, id, n } => {
                // `GeoPolygon.pathChanged`: the parameter runs from 0 to the
                // number of sides; its whole part is the side
                let length = n as f64;
                p.t %= length;
                if p.t < 0.0 {
                    p.t += length;
                }
                let index = p.t.floor() as usize;
                let seg = Seg::new(vertex(c, id, index), vertex(c, id, (index + 1) % n));
                let (x, y) = seg.point_at(p.t - index as f64);
                p.set_coords_2d(x, y, 1.0);
            }
            &Geom::PolyLine { c, id, n } => {
                // `GeoPolyLine.pathChanged`
                if n == 1 {
                    let (x, y) = vertex(c, id, 0);
                    p.set_coords_2d(x, y, 1.0);
                    return;
                }
                let last = (n - 1) as f64;
                let index = if p.t == last {
                    n - 2
                } else {
                    p.t %= last;
                    if p.t < 0.0 {
                        p.t += last;
                    }
                    (p.t.floor() as usize).min(n - 2)
                };
                let seg = Seg::new(vertex(c, id, index), vertex(c, id, index + 1));
                let (x, y) = seg.point_at(p.t - index as f64);
                p.set_coords_2d(x, y, 1.0);
            }
            Geom::Arc(a) => a.path_changed(p),
        }
    }
}

/// `AlgoDistancePointObject.closestValPoly`: minimize the squared distance
/// by solving `(t - x) + (f(t) - y) f'(t) = 0`.
fn closest_function_x(c: &Construction, id: Id, x: f64, y: f64) -> Option<f64> {
    let work = crate::expr::IntersectionWork::new();
    let (mut coefficients, denominator) = work.fraction(c, id)?;
    let &[scale] = denominator.as_slice() else {
        return None;
    };
    if scale == 0.0 || !work.charge((coefficients.len() * coefficients.len()) as u64) {
        return None;
    }
    for coefficient in &mut coefficients {
        *coefficient /= scale;
    }
    let mut stationary = vec![0.0; 2 * coefficients.len().saturating_sub(1).max(1)];
    stationary[0] = -x;
    stationary[1] = 1.0;
    for (degree, &coefficient) in coefficients.iter().enumerate().skip(1) {
        let derivative = degree as f64 * coefficient;
        stationary[degree - 1] -= y * derivative;
        for (i, &value) in coefficients.iter().enumerate() {
            stationary[degree - 1 + i] += derivative * value;
        }
    }
    let roots = super::intersect::conics::graph_roots(&stationary, &|units| work.charge(units));
    let mut closest = None;
    let mut best = f64::INFINITY;
    for t in roots {
        let value = coefficients.iter().rev().fold(0.0, |v, &a| v * t + a);
        let distance = (x - t).powi(2) + (y - value).powi(2);
        if distance + STANDARD_PRECISION < best {
            best = distance;
            closest = Some(t);
        }
    }
    closest
}

/// Vertex `i` of a polygon or polyline whose vertices are all defined.
fn vertex(c: &Construction, id: Id, i: usize) -> (f64, f64) {
    c.vertices(id)
        .nth(i)
        .flatten()
        .unwrap_or((f64::NAN, f64::NAN))
}

/// `GeoPoint.pointChanged(GeoPolygon)` and `GeoPolyLine.pointChanged`: the
/// closest point on any side, where side `i` runs from vertex `i` to vertex
/// `i + 1` (and the last side of a polygon back to vertex 0). One path
/// parameter is threaded through the sides, as there: a side of length 0
/// keeps the parameter the previous side left when it lies in `[0, 1]`.
fn polygonal_point_changed(c: &Construction, id: Id, n: usize, closed: bool, p: &mut PathState) {
    let qx = p.x / p.z;
    let qy = p.y / p.z;
    let mut best = Best {
        min_dist: f64::INFINITY,
        res: (0.0, 0.0, 0.0, 0.0),
    };
    let mut first = (f64::NAN, f64::NAN);
    let mut prev = (f64::NAN, f64::NAN);
    for (k, v) in c.vertices(id).enumerate().take(n) {
        let v = v.unwrap_or((f64::NAN, f64::NAN));
        if k == 0 {
            first = v;
        } else {
            best.side(k - 1, prev, v, (qx, qy), p);
        }
        prev = v;
    }
    if closed && n > 0 {
        best.side(n - 1, prev, first, (qx, qy), p);
    }
    let (x, y, z, t) = best.res;
    p.set_coords_2d(x, y, z);
    p.t = t;
}

/// The closest point found so far by [`polygonal_point_changed`].
struct Best {
    min_dist: f64,
    res: (f64, f64, f64, f64),
}

impl Best {
    fn side(&mut self, i: usize, a: (f64, f64), b: (f64, f64), q: (f64, f64), p: &mut PathState) {
        p.set_coords_2d(q.0, q.1, 1.0);
        Seg::new(a, b).point_changed(p);
        let x1 = p.x / p.z - q.0;
        let y1 = p.y / p.z - q.1;
        let dist = x1 * x1 + y1 * y1;
        if dist < self.min_dist {
            self.min_dist = dist;
            self.res = (p.x, p.y, p.z, i as f64 + p.t);
        }
    }
}

/// The start point of a line, in homogeneous coordinates, as the algorithm
/// that made it sets it with `GeoLine.setStartPoint`; `None` for a line
/// without one. A start point that is undefined now has NaN coordinates.
fn defined_line_start(c: &Construction, line: Id) -> Option<(f64, f64, f64)> {
    use super::conic_parts::ConicPartDef;
    use super::lines::LineDef;
    use super::vector::VectorDef;
    const NAN: (f64, f64, f64) = (f64::NAN, f64::NAN, f64::NAN);
    let hom = |p: Option<(f64, f64)>| p.map_or(NAN, |(x, y)| (x, y, 1.0));
    // `AlgoLineBisector` and `AlgoLineBisectorSegment`: the midpoint
    // `(A + B, 2)`
    let mid = |a: Option<(f64, f64)>, b: Option<(f64, f64)>| match (a, b) {
        (Some(a), Some(b)) => (a.0 + b.0, a.1 + b.1, 2.0),
        _ => NAN,
    };
    Some(match c.def(line) {
        Def::Line(p, _)
        | Def::Perpendicular { through: p, .. }
        | Def::Parallel { through: p, .. }
        | Def::Vector(VectorDef::LineVector(p, _) | VectorDef::OrthoLineVector(p, _))
        | Def::Lines(
            LineDef::AngleBisectorPoints { vertex: p, .. }
            | LineDef::TangentPoint { point: p, .. }
            | LineDef::ThroughRay { point: p, .. }
            | LineDef::NormalToConic { point: p, .. },
        )
        | Def::ConicPart(ConicPartDef::TangentPoint { point: p, .. }) => hom(c.point(p)),
        Def::Lines(LineDef::BisectorPoints(a, b)) => mid(c.point(a), c.point(b)),
        Def::Lines(LineDef::BisectorSegment(s)) => match c.value(s) {
            Value::Segment { x1, y1, x2, y2 } => mid(Some((x1, y1)), Some((x2, y2))),
            _ => NAN,
        },
        // `AlgoAngularBisectorLines`: the intersection `g x h`
        Def::Lines(LineDef::AngleBisectorLines { g, h, .. }) => {
            match (c.raw_carrier(g), c.raw_carrier(h)) {
                (Some(g), Some(h)) => (
                    g.1 * h.2 - g.2 * h.1,
                    g.2 * h.0 - g.0 * h.2,
                    g.0 * h.1 - g.1 * h.0,
                ),
                _ => NAN,
            }
        }
        // `AlgoTangentLine`: the point where the tangent touches
        Def::Lines(d @ LineDef::TangentLine { circle, .. }) => {
            hom(super::lines::tangent_point_on(c, d, circle).flatten())
        }
        Def::Lines(LineDef::TangentCurvePoint { point, curve }) => {
            hom(super::lines::curve_tangent_contact(c, point, curve))
        }
        Def::ConicPart(d @ ConicPartDef::TangentLine { conic, .. }) => {
            hom(super::conic_parts::tangent_point_on(c, d, conic).flatten())
        }
        // AlgoAxes.java and AlgoAxis.java set the start from the kept midpoint.
        Def::ConicPart(ConicPartDef::Axis { conic, .. }) => hom(c
            .conic_fields(conic)
            .map(|d| (d.midpoint[0], d.midpoint[1]))),
        Def::ConicPart(ConicPartDef::Asymptote { conic, .. }) => {
            hom(c.conic(conic).map(|d| (d.midpoint[0], d.midpoint[1])))
        }
        _ => return None,
    })
}

fn line_start(c: &Construction, line: Id) -> Option<(f64, f64, f64)> {
    defined_line_start(c, line).or_else(|| {
        let &(x, y, _, _) = c.frozen_line_starts.get(&line)?;
        Some((x, y, 1.0))
    })
}

/// `GeoLine.PathMoverLine.init` installs the foot from the origin once,
/// on the actual line. Later moves of the line leave this point in place.
/// `owner` is the locus whose computation installed it.
pub(crate) fn freeze_line_start(c: &mut Construction, path: Id, owner: Id) {
    if let Geom::Line {
        g: (a, b, z),
        start: None,
    } = geom(c, path)
    {
        let norm = a * a + b * b;
        let order = c.frozen_line_starts.len();
        c.frozen_line_starts
            .insert(path, (-z * a / norm, -z * b / norm, order, Some(owner)));
    }
}

impl Construction {
    /// The start points a locus froze on lines without one, as
    /// `(label, x, y, owner)` in the order they were frozen: the state of the
    /// lines that a rebuild can carry over (GeoGebra keeps the installed
    /// start when the locus is deleted). `owner` is the label of the locus
    /// that installed it, so a rebuild that drops that locus entry knows
    /// which starts it has to place itself.
    pub fn frozen_line_starts(&self) -> Vec<(String, f64, f64, String)> {
        let mut starts: Vec<(usize, String, f64, f64, String)> = self
            .frozen_line_starts
            .iter()
            .map(|(&id, &(x, y, order, owner))| {
                let owner = owner
                    .filter(|&o| o.index() < self.len())
                    .map(|o| self.label(o).to_owned())
                    .unwrap_or_default();
                (order, self.label(id).to_owned(), x, y, owner)
            })
            .collect();
        starts.sort_by_key(|&(order, ..)| order);
        starts
            .into_iter()
            .map(|(_, label, x, y, owner)| (label, x, y, owner))
            .collect()
    }

    /// Freezes `(x, y)` as the start point of the line `label`, when it is a
    /// line that no algorithm gave a start point: [`Self::frozen_line_starts`]
    /// read it back bit for bit (without an owner: the entry that froze it
    /// is gone when a rebuild installs one). Returns whether it was
    /// installed.
    pub fn set_frozen_line_start(&mut self, label: &str, x: f64, y: f64) -> bool {
        let Some(id) = self.id(label) else {
            return false;
        };
        if self.kind(id) != Kind::Line || defined_line_start(self, id).is_some() {
            return false;
        }
        let order = self.frozen_line_starts.len();
        self.frozen_line_starts.insert(id, (x, y, order, None));
        true
    }
}

/// The path `path` as the path code reads it now.
pub(crate) fn geom(c: &Construction, path: Id) -> Geom<'_> {
    // an arc is read as it is even when undefined: `setCoords` still asks
    // it to place the point (see `Geom::is_defined`)
    if let Some(a) = c.arc(path) {
        return Geom::Arc(a);
    }
    // a general conic keeps the parts of its last defined state when its
    // classification fails (`classifyConic` returns before it rewrites
    // them), so a point moved on an undefined one is still projected onto
    // that record (`GeoPoint.setCoords` asks `pointChanged` whatever the
    // path's state); only the update of a point on it sees it undefined
    // (`AlgoPointOnPath` checks `isDefined`, [`Geom::is_defined`])
    if c.kind(path) == Kind::Conic {
        if let Some(d) = c.conic(path) {
            return Geom::Conic(d);
        }
        return Geom::Undefined;
    }
    let value = c.value(path);
    if !value.is_defined() {
        return Geom::Undefined;
    }
    let polygonal = || {
        let n = c.vertices(path).count();
        (n > 0 && c.vertices(path).all(|v| v.is_some())).then_some(n)
    };
    match (c.kind(path), value) {
        (_, Value::Line { a, b, c: z }) => Geom::Line {
            g: (a, b, z),
            start: line_start(c, path),
        },
        (_, Value::Ray { x, y, a, b, c: z }) => Geom::Ray {
            g: (a, b, z),
            start: (x, y),
        },
        (_, Value::Segment { x1, y1, x2, y2 }) => Geom::Segment(Seg::new((x1, y1), (x2, y2))),
        (Kind::Circle, _) => match c.conic(path) {
            Some(d) if d.defined => Geom::Conic(d),
            _ => Geom::Undefined,
        },
        (Kind::Polygon, _) => match polygonal() {
            Some(n) => Geom::Polygon { c, id: path, n },
            None => Geom::Undefined,
        },
        (Kind::PolyLine, _) => match polygonal() {
            Some(n) => Geom::PolyLine { c, id: path, n },
            None => Geom::Undefined,
        },
        (Kind::Function, Value::Function)
            if crate::expr::function_signature(c, path).is_some_and(|sig| !sig.boolean) =>
        {
            Geom::Function { c, id: path }
        }
        (Kind::Curve, Value::Curve { start, end }) => Geom::Curve {
            c,
            id: path,
            start,
            end,
        },
        _ => Geom::Undefined,
    }
}

/// Projection used when a command copies a point onto a path.
pub(crate) fn project(c: &Construction, id: Id, (x, y): (f64, f64)) -> Option<Value> {
    let path = geom(c, id);
    if !path.is_defined() {
        return None;
    }
    let mut state = PathState::new();
    state.set_coords(&path, x, y);
    path.point_changed(&mut state);
    state.update_coords();
    state.value()
}

/// `AlgoPointOnPath.compute`: a parameter given as a number is put on the
/// path first; then the point follows the path, or becomes undefined with
/// it.
fn follow(c: &Construction, path: &Geom<'_>, param: Option<Id>, st: &mut PathState) {
    if let Some(param) = param {
        let (min, max) = path.range();
        st.set_t(to_parent(c.number(param).unwrap_or(f64::NAN), min, max));
    }
    if !path.is_defined() {
        st.set_undefined();
    } else {
        path.path_changed(st);
        st.update_coords();
    }
}

impl PathDef {
    /// The path and the parameter object of a point on a path.
    fn path_and_param(self) -> Option<(Id, Option<Id>)> {
        match self {
            PathDef::On { path } => Some((path, None)),
            PathDef::OnParam { path, param } => Some((path, Some(param))),
            PathDef::Parameter { .. }
            | PathDef::InRegion { .. }
            | PathDef::FixedEndpoint { .. }
            | PathDef::RigidEndpoint { .. } => None,
        }
    }

    /// Whether this defines a point on a path, which has a [`PathState`].
    pub(crate) fn has_state(self) -> bool {
        !matches!(self, PathDef::Parameter { .. })
    }

    /// The state of a new point (`AlgoDispatcher.point` with a path): placed
    /// at `(0, 0)`, then `(1, 0)` if that is undefined. GeoGebra then tries
    /// a random point; this kernel does not (see `DIFFERENCES.md`).
    pub(crate) fn create(self, c: &Construction) -> PathState {
        let mut st = PathState::new();
        match self {
            PathDef::InRegion { region } => {
                st.place_in_region(c, region, (0.0, 0.0));
                if st.region == RegionState::PartEdge {
                    let p = (st.x, st.y);
                    st.place_in_region(c, region, p);
                }
                return st;
            }
            PathDef::RigidEndpoint {
                anchor,
                radius,
                bearing,
            } => {
                st.set_t(bearing);
                follow(
                    c,
                    &fixed_circle(c, anchor, Count::Literal(radius)),
                    None,
                    &mut st,
                );
                st.temp_t = st.t;
                st.temp_ty = st.ty;
                return st;
            }
            PathDef::FixedEndpoint { start, length } => {
                let (x, y) = c.point(start).unwrap_or((f64::NAN, f64::NAN));
                st.set_coords(&fixed_circle(c, start, length), x + length.value(c), y);
                return st;
            }
            _ => {}
        }
        let Some((path, param)) = self.path_and_param() else {
            return st;
        };
        let path = geom(c, path);
        st.set_coords(&path, 0.0, 0.0);
        // `AlgoPointOnPath` with a parameter computes once when created
        if param.is_some() {
            follow(c, &path, param, &mut st);
        }
        if !st.is_defined() {
            st.set_coords(&path, 1.0, 0.0);
        }
        st
    }

    /// The state after the path or the parameter changed.
    pub(crate) fn update(self, c: &Construction, old: &PathState) -> PathState {
        let mut st = *old;
        match self {
            PathDef::InRegion { region } => st.follow_region(c, region),
            PathDef::RigidEndpoint { anchor, radius, .. } => {
                follow(
                    c,
                    &fixed_circle(c, anchor, Count::Literal(radius)),
                    None,
                    &mut st,
                );
            }
            PathDef::FixedEndpoint { start, length } => {
                follow(c, &fixed_circle(c, start, length), None, &mut st);
            }
            _ => {
                if let Some((path, param)) = self.path_and_param() {
                    follow(c, &geom(c, path), param, &mut st);
                }
            }
        }
        st
    }

    /// The state after the user moved a point made by `Point(path)` to
    /// `(x, y)`; `None` for any other definition.
    pub(crate) fn moved(
        self,
        c: &Construction,
        old: &PathState,
        x: f64,
        y: f64,
    ) -> Option<PathState> {
        let mut st = *old;
        match self {
            PathDef::On { path } => st.set_coords(&geom(c, path), x, y),
            PathDef::InRegion { region } => st.place_in_region(c, region, (x, y)),
            PathDef::RigidEndpoint { anchor, radius, .. } => {
                st.set_coords(&fixed_circle(c, anchor, Count::Literal(radius)), x, y)
            }
            PathDef::FixedEndpoint { start, length } => {
                st.set_coords(&fixed_circle(c, start, length), x, y)
            }
            _ => return None,
        }
        Some(st)
    }

    /// The value of a point with state `st`.
    pub(crate) fn value(st: &PathState) -> Value {
        st.value().unwrap_or(Value::Undefined)
    }
}

/// `AlgoPathParameter.compute`: the parameter of a point on a path,
/// normalised to `[0, 1]`. A point of `ClosestPoint` lies on its path too:
/// its parameter is that of `AlgoClosestPoint.compute`, `setCoords` with the
/// point followed by `pointChanged`.
fn path_parameter(c: &Construction, point: Id) -> Option<Value> {
    use super::point::PointDef;
    let (st, path) = match c.def(point) {
        Def::Path(PathDef::On { path } | PathDef::OnParam { path, .. }) => {
            (c.path_state(point)?, path)
        }
        Def::Point(PointDef::Closest { path, point: q }) => {
            let (qx, qy) = c.point(q)?;
            let g = geom(c, path);
            if !g.is_defined() {
                return None;
            }
            let mut st = PathState::new();
            st.set_coords(&g, qx, qy);
            g.point_changed(&mut st);
            st.update_coords();
            (st, path)
        }
        _ => return None,
    };
    // A point with an infinite coordinate is undefined in this kernel, and
    // so is its parameter (GeoGebra counts such a point as defined).
    if !st.is_defined() || st.value().is_none() {
        return None;
    }
    let (min, max) = st
        .function_bounds
        .map_or_else(|| geom(c, path).range(), |[a, b]| (a, b));
    Some(Value::Number(to_normalized(st.t, min, max)))
}

/// The objects a definition of this module reads besides its parents: for
/// a point on a line with a start point, or on a polygon or polyline, the
/// inputs of the path (a start point or a vertex can move while the line
/// or the polygon's value stays the same); for `PathParameter`, also the
/// path of the point, and the point a `ClosestPoint` is closest to.
pub(crate) fn hidden_parents(c: &Construction, def: &Def) -> Vec<Id> {
    use super::point::PointDef;
    let inputs = |path: Id| match c.kind(path) {
        Kind::Line if defined_line_start(c, path).is_some() => c.read_ids(&c.def(path)),
        Kind::Polygon | Kind::PolyLine => c.read_ids(&c.def(path)),
        _ => Vec::new(),
    };
    match *def {
        Def::Path(
            PathDef::On { path }
            | PathDef::OnParam { path, .. }
            | PathDef::InRegion { region: path },
        )
        | Def::Point(PointDef::Closest { path, .. })
        | Def::Measure(super::measure::MeasureDef::DistancePointPath { path, .. }) => inputs(path),
        Def::Path(PathDef::Parameter { point }) => match c.def(point) {
            Def::Path(PathDef::On { path } | PathDef::OnParam { path, .. }) => {
                let mut ids = inputs(path);
                ids.push(path);
                ids
            }
            // the parameter is computed from the point `q` itself, which can
            // move while the closest point stays the same to the last bit
            Def::Point(PointDef::Closest { path, point: q }) => {
                let mut ids = inputs(path);
                ids.extend([path, q]);
                ids
            }
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

impl Group for PathDef {
    fn kind(&self) -> Kind {
        match self {
            PathDef::On { .. }
            | PathDef::OnParam { .. }
            | PathDef::InRegion { .. }
            | PathDef::FixedEndpoint { .. }
            | PathDef::RigidEndpoint { .. } => Kind::Point,
            PathDef::Parameter { .. } => Kind::Number,
        }
    }

    fn parents(&self) -> Parents {
        match *self {
            PathDef::On { path }
            | PathDef::InRegion { region: path }
            | PathDef::RigidEndpoint { anchor: path, .. } => Parents::of(&[path]),
            PathDef::FixedEndpoint { start, length } => {
                if let Some(length) = length.id() {
                    Parents::of(&[start, length])
                } else {
                    Parents::of(&[start])
                }
            }
            PathDef::OnParam { path, param } => Parents::of(&[path, param]),
            PathDef::Parameter { point } => Parents::of(&[point]),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            PathDef::InRegion { region } => c.expect(region, REGIONS),
            PathDef::RigidEndpoint {
                anchor,
                radius,
                bearing,
            } => {
                c.expect(anchor, POINT)?;
                if radius.is_finite() && radius >= 0.0 && bearing.is_finite() {
                    Ok(())
                } else {
                    Err(Error::NotFinite)
                }
            }
            PathDef::FixedEndpoint { start, length } => {
                c.expect(start, POINT)?;
                length.check(c)
            }
            PathDef::On { path } => c
                .expect(path, &[Kind::Function])
                .or_else(|_| c.expect(path, PATH)),
            PathDef::OnParam { path, param } => {
                c.expect(path, PATH)?;
                c.expect(param, NUMBER)
            }
            PathDef::Parameter { point } => c.expect(point, POINT),
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        match *self {
            PathDef::Parameter { point } => path_parameter(c, point),
            d => d.create(c).value(),
        }
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        let path = |label: &str| -> Result<Id, String> {
            let id = object(c, label)?;
            if PATH.contains(&c.kind(id))
                || (c.kind(id) == Kind::Function && matches!(c.def(id), Def::Chart(_)))
            {
                Ok(id)
            } else {
                Err(format!(
                    "`Point` needs a line, segment, ray, circle, conic, polygon, polyline, arc, \
                     sector or curve, and `{label}` is a {}",
                    c.kind(id)
                ))
            }
        };
        Some(match (func, args) {
            ("PointIn", [region]) => object(c, region).map(|region| PathDef::InRegion { region }),
            ("PointIn", _) => Err("`PointIn` takes one region".into()),
            ("Point", [p]) => path(p).map(|path| PathDef::On { path }),
            ("Point", [p, t]) => (|| {
                let path = path(p)?;
                let param = object(c, t)?;
                if c.kind(param) != Kind::Number {
                    return Err(format!("`Point({p}, {t})` needs a number as the parameter"));
                }
                Ok(PathDef::OnParam { path, param })
            })(),
            ("Point", _) => Err("`Point` takes a path, or a path and a number".into()),
            ("PathParameter", [p]) => (|| {
                let point = object(c, p)?;
                if c.kind(point) != Kind::Point {
                    return Err(format!(
                        "`PathParameter` needs a point, and `{p}` is a {}",
                        c.kind(point)
                    ));
                }
                Ok(PathDef::Parameter { point })
            })(),
            ("PathParameter", _) => Err("`PathParameter` takes one point".into()),
            _ => return None,
        })
    }
}

pub(super) const REGIONS: &[Kind] = &[
    Kind::Polygon,
    Kind::Circle,
    Kind::Conic,
    Kind::Arc,
    Kind::Sector,
];

fn fixed_circle(c: &Construction, start: Id, length: Count) -> Geom<'_> {
    let r = length.value(c);
    match c.point(start) {
        Some((x, y)) if r.is_finite() && r >= 0.0 => Geom::Conic(ConicData::circle(x, y, r)),
        _ => Geom::Undefined,
    }
}

impl PathState {
    fn place_in_region(&mut self, c: &Construction, region: Id, p: (f64, f64)) {
        self.set_coords_2d(p.0, p.1, 1.0);
        if c.kind(region) == Kind::Polygon {
            let vertices: Vec<_> = c.vertices(region).collect();
            if let Some(vertices) = vertices.into_iter().collect::<Option<Vec<_>>>() {
                if polygon_contains(&vertices, p) {
                    if let Some((second, third)) = polygon_basis(&vertices) {
                        let a = vertices[0];
                        let (u, v) = (sub(vertices[second], a), sub(vertices[third], a));
                        let q = sub(p, a);
                        let determinant = cross(u, v);
                        self.region = RegionState::Polygon {
                            second,
                            third,
                            u: cross(q, v) / determinant,
                            v: cross(u, q) / determinant,
                        };
                        return;
                    }
                }
            }
        } else if let Some(d) = super::conic_object(c, region) {
            if conic_contains(&d, p) && c.arc(region).is_none_or(|a| part_contains(&a, p)) {
                super::conic_path::rw_to_ev(&d, self);
                let (x, y, _) = self.coords();
                let [a, b] = conic_region_axes(&d);
                if a != 0.0 && b != 0.0 {
                    self.region = RegionState::Conic(x / a, y / b);
                    self.set_coords_2d(p.0, p.1, 1.0);
                    return;
                }
            } else if let Some(arc) = c.arc(region) {
                self.region = if conic_contains(&d, p) {
                    RegionState::PartEdge
                } else {
                    RegionState::Boundary
                };
                project_part(&arc, self, p);
                return;
            }
        }
        self.region = RegionState::Boundary;
        self.set_coords(&geom(c, region), p.0, p.1);
    }

    fn follow_region(&mut self, c: &Construction, region: Id) {
        let path = geom(c, region);
        if !path.is_defined() {
            self.set_undefined();
            return;
        }
        match self.region {
            RegionState::Boundary => {
                path.path_changed(self);
            }
            RegionState::PartEdge => self.place_in_region(c, region, (self.x, self.y)),
            RegionState::Conic(u, v) => {
                let Some(d) = super::conic_object(c, region) else {
                    self.set_undefined();
                    return;
                };
                let [a, b] = conic_region_axes(&d);
                self.set_coords_2d(u * a, v * b, 1.0);
                super::conic_path::ev_to_rw(&d, self);
                let q = (self.x, self.y);
                if !conic_contains(&d, q) || c.arc(region).is_some_and(|a| !part_contains(&a, q)) {
                    self.place_in_region(c, region, q);
                    if self.region == RegionState::PartEdge {
                        self.region = RegionState::Conic(u, v);
                        if let Some(arc) = c.arc(region) {
                            if !part_contains(&arc, (self.x, self.y)) {
                                arc.point_changed(self);
                            }
                        }
                    }
                }
            }
            RegionState::Polygon {
                second,
                third,
                u,
                v,
            } => {
                let vertices: Option<Vec<_>> = c.vertices(region).collect();
                let Some(vertices) = vertices else {
                    self.set_undefined();
                    return;
                };
                let basis = vertices.get(second).zip(vertices.get(third));
                let Some((&b, &d)) = basis else {
                    self.place_in_region(c, region, (self.x, self.y));
                    return;
                };
                let a = vertices[0];
                let (ab, ad) = (sub(b, a), sub(d, a));
                let p = (a.0 + u * ab.0 + v * ad.0, a.1 + u * ab.1 + v * ad.1);
                if !polygon_contains(&vertices, p) {
                    self.place_in_region(c, region, p);
                } else {
                    self.set_coords_2d(p.0, p.1, 1.0);
                }
            }
        }
        self.update_coords();
    }
}

fn conic_region_axes(d: &ConicData) -> [f64; 2] {
    if d.ty == ConicType::Parabola {
        [1.0, d.p.sqrt()]
    } else {
        d.half_axes
    }
}

pub(crate) fn conic_contains(d: &ConicData, (x, y): (f64, f64)) -> bool {
    let [a, b, z, h, g, f] = d.matrix;
    let evaluate =
        |x: f64, y: f64| x * (a * x + h * y + 2.0 * g) + y * (h * x + b * y + 2.0 * f) + z;
    let [mx, my] = d.midpoint;
    let sign = match d.ty {
        ConicType::IntersectingLines => evaluate(
            mx + d.lines[0][0] + d.lines[1][0],
            my + d.lines[0][1] + d.lines[1][1],
        ),
        ConicType::Hyperbola => -evaluate(mx, my),
        ConicType::Parabola => evaluate(mx + d.p * d.eigenvec[0][0], my + d.p * d.eigenvec[0][1]),
        ConicType::SinglePoint => evaluate(mx + 1.0, my),
        _ => evaluate(mx, my),
    };
    evaluate(x, y) / sign >= -STANDARD_PRECISION
}

pub(crate) fn part_end(a: &ArcData, t: f64) -> (f64, f64) {
    let mut p = PathState::new();
    let [x, y] = a.conic.half_axes;
    p.set_coords_2d(x * t.cos(), y * t.sin(), 1.0);
    super::conic_path::ev_to_rw(&a.conic, &mut p);
    (p.x, p.y)
}

pub(crate) fn part_contains(a: &ArcData, p: (f64, f64)) -> bool {
    if a.sector {
        let mut state = PathState::new();
        state.set_coords_2d(p.0, p.1, 1.0);
        super::conic_path::rw_to_ev(&a.conic, &mut state);
        let [ha, hb] = a.conic.half_axes;
        let positive = |t: f64| {
            if t < 0.0 {
                t + std::f64::consts::TAU
            } else {
                t
            }
        };
        let t = positive((ha * state.y).atan2(hb * state.x));
        positive(t - a.start) <= a.extent + STANDARD_PRECISION
    } else {
        let [u, v] = a.conic.eigenvec;
        let unit = |t: f64| {
            let (s, c) = t.sin_cos();
            (u[0] * c + v[0] * s, u[1] * c + v[1] * s)
        };
        let start = unit(a.start);
        let end = unit(a.end);
        let [mx, my] = a.conic.midpoint;
        let r = a.conic.half_axes[1];
        let p = ((p.0 - mx) / r, (p.1 - my) / r);
        cross(sub(end, start), sub(p, start)) <= STANDARD_PRECISION
    }
}

fn project_part(a: &ArcData, state: &mut PathState, p: (f64, f64)) {
    let start = part_end(a, a.start);
    let end = part_end(a, a.end);
    let [x, y] = a.conic.midpoint;
    let center = (x, y);
    let mut boundary = *state;
    a.point_changed(&mut boundary);
    let mut best = if a.sector { center } else { start };
    let mut distance = my_length(best.0 - p.0, best.1 - p.1);
    for q in [start, end] {
        let d = my_length(q.0 - p.0, q.1 - p.1);
        if d < distance {
            best = q;
            distance = d;
        }
    }
    let segments = if a.sector {
        [(center, start), (center, end)]
    } else {
        [(start, end), (end, end)]
    };
    for (start, end) in segments {
        let mut foot = *state;
        Seg::new(start, end).point_changed(&mut foot);
        let d = my_length(foot.x - p.0, foot.y - p.1);
        if d < distance {
            best = (foot.x, foot.y);
            distance = d;
        }
    }
    if !conic_contains(&a.conic, p) {
        let d = my_length(boundary.x - p.0, boundary.y - p.1);
        if d < distance {
            best = (boundary.x, boundary.y);
        }
    }
    state.set_coords_2d(best.0, best.1, 1.0);
    let mut parameter = *state;
    a.point_changed(&mut parameter);
    state.t = parameter.t;
    state.ty = parameter.ty;
    state.temp_t = state.t;
    state.temp_ty = state.ty;
}

fn sub(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 - b.0, a.1 - b.1)
}

fn cross(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

fn polygon_basis(vertices: &[(f64, f64)]) -> Option<(usize, usize)> {
    let &origin = vertices.first()?;
    let second = vertices.iter().position(|&p| {
        let (x, y) = sub(p, origin);
        x.abs() >= STANDARD_PRECISION || y.abs() >= STANDARD_PRECISION
    })?;
    let u = sub(vertices[second], origin);
    let third = (second + 1..vertices.len())
        .rev()
        .find(|&i| cross(u, sub(vertices[i], origin)).abs() >= STANDARD_PRECISION)?;
    Some((second, third))
}

pub(crate) fn polygon_contains(vertices: &[(f64, f64)], p: (f64, f64)) -> bool {
    let Some(&last) = vertices.last() else {
        return false;
    };
    let mut prev = sub(last, p);
    let mut inside = false;
    for &vertex in vertices {
        let next = sub(vertex, p);
        let det = cross(prev, next);
        if det.abs() < STANDARD_PRECISION && prev.0 * next.0 + prev.1 * next.1 <= STANDARD_PRECISION
        {
            return true;
        }
        if (prev.1 > 0.0) != (next.1 > 0.0) && det / (next.1 - prev.1) > 0.0 {
            inside = !inside;
        }
        prev = next;
    }
    inside
}
