// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoConicPart.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicPartParameters.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoConicPart.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoConicPartCircle.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoConicPartCircumcircleND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoConicPartConicPoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoConicPartConicPointsND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoConicPartConicParameters.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSemicircle.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCircleTwoPoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoArcLength.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAreaConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCircumferenceConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdArcSector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdCircleArcSector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdSemicircle.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/Commands.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Arcs and sectors of circles and conics (GeoGebra's `GeoConicPart`):
//! `CircularArc`, `CircularSector`, `CircumcircularArc`,
//! `CircumcircularSector`, `Semicircle`, `Arc` and `Sector`, and their
//! `Length`, `Area` and `Perimeter`.
//!
//! An arc's value is its length, a sector's its area. The rest of the part
//! (its conic, its start and end parameter, its orientation) is kept in an
//! [`ArcData`] record in a side table of the construction, read with
//! [`Construction::arc`]. A point on an arc reads that record, so the
//! construction recomputes the arc's children whenever the record changes,
//! bit for bit, even when its value stays the same.

use std::f64::consts::PI;

use super::conic::{circumcircle, Circumcircle};
use super::conic_nd::{my_length, ConicData, ConicType};
use super::conic_path::{self, type_number};
use super::path::{conic_parameter, line_path_changed, line_point_changed, PathState};
use super::{join, Group, Parents, POINT};
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};
use crate::script::object;
use crate::tol::STANDARD_PRECISION;

/// Homogeneous coordinates of a point or the coefficients of a line.
type Hom = (f64, f64, f64);

/// `Kernel.PI_2`.
const PI_2: f64 = 2.0 * PI;

const NUMBER: &[Kind] = &[Kind::Number];
/// What `Arc` and `Sector` between two parameters accept as the conic: a
/// circle or a general conic.
const CONIC: &[Kind] = &[Kind::Circle, Kind::Conic];
/// What `Arc` and `Sector` between two points accept: a circle only. On a
/// general conic GeoGebra projects the points with the closest-point
/// solvers of `GeoConicND.pointChanged`, which are not ported (see
/// `DIFFERENCES.md`).
const CIRCLE: &[Kind] = &[Kind::Circle];
/// Checks that points can be put on the part `path`: not on a part of a
/// general conic, whose points GeoGebra projects with the closest-point
/// solvers of `GeoConicND.pointChangedUnlimited` (not ported, see
/// `DIFFERENCES.md`). Reports the conic as not being a circle.
pub(crate) fn expect_point_path(c: &Construction, path: Id) -> Result<(), Error> {
    match c.def(path) {
        Def::Arc(ArcDef::ConicParameters { conic, .. }) => c.expect(conic, CIRCLE),
        _ => Ok(()),
    }
}
/// Arcs and sectors.
pub(crate) const PART: &[Kind] = &[Kind::Arc, Kind::Sector];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArcDef {
    /// `CircularArc(M, A, B)` and `CircularSector(M, A, B)` (also named
    /// `CircleArc` and `CircleSector`): the part of the circle around `M`
    /// through `A` from `A` counter-clockwise to the ray `MB`.
    Circle {
        center: Id,
        start: Id,
        end: Id,
        sector: bool,
    },
    /// `CircumcircularArc(A, B, C)` and `CircumcircularSector(A, B, C)` (also
    /// `CircumcircleArc`, `CircumcircleSector`): the part of the circle
    /// through `A`, `B` and `C` from `A` through `B` to `C`.
    Circum { a: Id, b: Id, c: Id, sector: bool },
    /// `Semicircle(A, B)`: the half circle with diameter `AB`, running
    /// counter-clockwise from `B` to `A` (to the left of the direction from
    /// `A` to `B`).
    Semicircle { a: Id, b: Id },
    /// `Arc(k, P, Q)` and `Sector(k, P, Q)`: the part of the conic `k` from
    /// the parameter of `P` to that of `Q`.
    ConicPoints {
        conic: Id,
        p: Id,
        q: Id,
        sector: bool,
    },
    /// `Arc(k, s, t)` and `Sector(k, s, t)`: the part of the conic `k` from
    /// the parameter `s` to `t`.
    ConicParameters {
        conic: Id,
        s: Id,
        t: Id,
        sector: bool,
    },
    /// `Length(a)` of an arc or sector: its arc length (`AlgoArcLength`).
    Length(Id),
    /// `Area(a)` of an arc or sector (`AlgoAreaConic`): the area of a
    /// sector, and for an arc of a circle the area between the arc and its
    /// chord.
    Area(Id),
    /// `Perimeter(a)` and `Circumference(a)` of a part of a circle
    /// (`AlgoCircumferenceConic`): the arc and its chord, or the arc and two
    /// radii.
    Perimeter(Id),
}

/// GeoGebra's `GeoConicPart` with its `GeoConicPartParameters`: the conic
/// the part lies on, the range of its parameter and its measures.
#[derive(Clone, Copy, Debug)]
pub struct ArcData {
    /// The conic of the part (`GeoConicPart` extends `GeoConic`).
    pub conic: ConicData,
    /// A sector rather than an arc (`conicPartType`).
    pub sector: bool,
    /// `posOrientation`: whether the part runs from its first to its second
    /// parameter; when false they are swapped, so that `start <= end`
    /// always, and points on the part run from `end` back to `start`.
    pub positive: bool,
    /// `paramStart` and `paramEnd`, each in `[0, 2 pi]`, and `paramExtent`,
    /// the angle from start to end in `[0, 2 pi]`.
    pub start: f64,
    pub end: f64,
    pub extent: f64,
    /// The length of an arc or the area of a sector (`value`).
    pub value: f64,
    arc_length: f64,
    area: f64,
    /// `valueDefined`, which is `GeoConicPart.isDefined`.
    pub defined: bool,
    /// The start and end point of the first line of a degenerate part
    /// (`CircumcircularArc` of collinear points): `A` and `C`.
    line_points: Option<[(f64, f64); 2]>,
}

impl ArcData {
    /// A new `GeoConicPart`: every number 0, positive orientation.
    fn new(sector: bool) -> Self {
        ArcData {
            conic: ConicData::new(),
            sector,
            positive: true,
            start: 0.0,
            end: 0.0,
            extent: 0.0,
            value: 0.0,
            arc_length: 0.0,
            area: 0.0,
            defined: false,
            line_points: None,
        }
    }

    /// Bit equality, so that a recomputation that changes any bit counts.
    pub(crate) fn same(&self, o: &ArcData) -> bool {
        let b = |x: f64, y: f64| x.to_bits() == y.to_bits();
        let points = |p: Option<[(f64, f64); 2]>| p.map(|[a, c]| [a.0, a.1, c.0, c.1]);
        let pts = match (points(self.line_points), points(o.line_points)) {
            (None, None) => true,
            (Some(x), Some(y)) => x.iter().zip(&y).all(|(&x, &y)| b(x, y)),
            _ => false,
        };
        self.conic.same(&o.conic)
            && self.sector == o.sector
            && self.positive == o.positive
            && b(self.start, o.start)
            && b(self.end, o.end)
            && b(self.extent, o.extent)
            && b(self.value, o.value)
            && b(self.arc_length, o.arc_length)
            && b(self.area, o.area)
            && self.defined == o.defined
            && pts
    }

    /// Whether the part lies on a circle or an ellipse, where its parameter
    /// range and measures are computed afresh on every update. For the
    /// other types GeoGebra keeps them from an earlier update.
    pub fn is_curved(&self) -> bool {
        matches!(self.conic.ty, ConicType::Circle | ConicType::Ellipse)
    }

    /// `GeoConicPart.getArcLength`: NaN when undefined. Only parts of
    /// circles and ellipses have one here (see [`Self::is_curved`]).
    pub fn arc_length(&self) -> f64 {
        if self.defined && self.is_curved() {
            self.arc_length
        } else {
            f64::NAN
        }
    }

    /// `GeoConicPart.getArea`: NaN when undefined; like
    /// [`Self::arc_length`] only for parts of circles and ellipses.
    pub fn area(&self) -> f64 {
        if self.defined && self.is_curved() {
            self.area
        } else {
            f64::NAN
        }
    }

    /// The end points of a part that degenerated to a segment or two rays
    /// (`CircumcircularArc` of collinear points): `A` and `C`.
    pub fn segment(&self) -> Option<[(f64, f64); 2]> {
        self.line_points
    }

    /// `setUndefined`.
    fn set_undefined(&mut self) {
        self.defined = false;
    }

    /// `GeoConicPartParameters.setParameters`.
    fn set_parameters(&mut self, defined: bool, start: f64, end: f64, positive: bool) {
        self.positive = positive;
        if positive {
            self.set_parameters_keep_orientation(defined, start, end);
        } else {
            // internally the orientation is always positive, start <= end
            self.set_parameters_keep_orientation(defined, end, start);
        }
    }

    /// `GeoConicPartParameters.setParametersKeepOrientation`.
    fn set_parameters_keep_orientation(&mut self, defined: bool, start: f64, end: f64) {
        self.defined = defined;
        if !self.defined {
            self.value = f64::NAN;
            return;
        }
        match self.conic.ty {
            ConicType::Circle => {
                self.set_extent_ellipse(start, end);
                let r = self.conic.half_axes[0];
                self.arc_length = r * self.extent;
                if self.sector {
                    self.value = r * r * self.extent / 2.0;
                    self.area = self.value;
                } else {
                    self.value = self.arc_length;
                    // area of the arc: area of the sector minus the triangle
                    self.area = r * r * (self.extent - self.extent.sin()) / 2.0;
                }
                self.defined = !self.value.is_nan() && !self.value.is_infinite();
            }
            ConicType::Ellipse => {
                self.set_extent_ellipse(start, end);
                self.arc_length = super::elliptic::arc_length(&self.conic, self.start, self.end);
                self.area = self.conic.half_axes[0] * self.conic.half_axes[1] * self.extent / 2.0;
                self.value = if self.sector {
                    self.area
                } else {
                    self.arc_length
                };
                self.defined = !self.value.is_nan() && !self.value.is_infinite();
            }
            // a circular arc through three points may degenerate to a
            // segment or two rays
            ConicType::ParallelLines => {
                if !self.sector && self.positive {
                    match self.line_points {
                        Some([a, c]) => self.value = my_length(c.0 - a.0, c.1 - a.1),
                        None => {
                            self.value = f64::INFINITY;
                            self.defined = false;
                            return;
                        }
                    }
                } else {
                    // area of a sector, or length of two rays
                    self.value = f64::INFINITY;
                }
                self.defined = true;
            }
            _ => self.defined = false,
        }
    }

    /// `GeoConicPartParameters.setExtentEllipse`.
    fn set_extent_ellipse(&mut self, start: f64, end: f64) {
        self.start = angle_value(start);
        self.end = angle_value(end);
        self.extent = self.end - self.start;
        if self.extent < 0.0 {
            self.extent += PI_2;
        }
    }

    /// The line a degenerate part lies on, with its start point.
    fn first_line(&self) -> (Hom, Option<Hom>) {
        let [x, y, z] = self.conic.lines[0];
        let start = self.line_points.map(|[a, _]| (a.0, a.1, 1.0));
        ((x, y, z), start)
    }

    /// `GeoConicPart.getMinParameter` and `getMaxParameter`.
    pub(crate) fn range(&self) -> (f64, f64) {
        match self.conic.ty {
            ConicType::Circle | ConicType::Ellipse => (0.0, 1.0),
            ConicType::ParallelLines if self.positive => (0.0, 1.0),
            ConicType::ParallelLines => (f64::NEG_INFINITY, f64::INFINITY),
            _ => (f64::NAN, f64::NAN),
        }
    }

    /// `GeoConicPart.pointChanged`: the closest point of the part.
    pub(crate) fn point_changed(&self, p: &mut PathState) {
        let ty = self.conic.ty;
        p.set_type(type_number(ty));
        match ty {
            ConicType::Circle | ConicType::Ellipse => {
                self.set_ellipse_parameter(p);
                self.clip_ellipse_parameter(p);
            }
            // degenerate: one segment or two rays
            ConicType::ParallelLines => {
                let (g, start) = self.first_line();
                if self.positive {
                    line_point_changed(g, start, p);
                    // keep the point on the segment: 0 <= t <= 1
                    if p.parameter() < 0.0 {
                        p.put_t(0.0);
                        self.path_changed(p);
                    } else if p.parameter() > 1.0 {
                        p.put_t(1.0);
                        self.path_changed(p);
                    }
                } else {
                    // two rays: the point at infinity
                    p.set_coords_2d(-g.1, g.0, 0.0);
                }
            }
            _ => p.put_t(f64::NAN),
        }
    }

    /// `GeoConicPart.pathChanged`: the point at its parameter.
    pub(crate) fn path_changed(&self, p: &mut PathState) {
        if !self.defined {
            p.put_x(f64::NAN);
            return;
        }
        let ty = self.conic.ty;
        if p.path_type() != type_number(ty) || p.parameter().is_nan() {
            self.point_changed(p);
            return;
        }
        if p.parameter() < 0.0 {
            p.put_t(0.0);
        } else if p.parameter() > 1.0 {
            p.put_t(1.0);
        }
        match ty {
            ConicType::Circle | ConicType::Ellipse => {
                // the point on the conic at this parameter, in eigenvector
                // coordinates
                let t = if self.positive {
                    p.parameter()
                } else {
                    1.0 - p.parameter()
                };
                let angle = self.start + t * self.extent;
                let [ha, hb] = self.conic.half_axes;
                p.set_coords_2d(ha * angle.cos(), hb * angle.sin(), 1.0);
                conic_path::ev_to_rw(&self.conic, p);
            }
            ConicType::ParallelLines => {
                let (g, start) = self.first_line();
                if self.positive {
                    line_path_changed(g, start, p);
                } else {
                    p.set_coords_2d(-g.1, g.0, 0.0);
                }
            }
            // an unsupported part keeps the point where it is
            _ => {}
        }
    }

    /// `GeoConicPartParameters.setEllipseParameter`: the parameter on the
    /// full conic, mapped from `[start, end]` to `[0, 1]`.
    fn set_ellipse_parameter(&self, p: &mut PathState) {
        conic_path::point_changed(&self.conic, p);
        if p.parameter() < 0.0 {
            p.put_t(p.parameter() + PI_2);
        }
        let mut t = p.parameter() - self.start;
        if t < 0.0 {
            t += PI_2;
        }
        p.put_t(t / self.extent);
    }

    /// `GeoConicPartParameters.clipEllipseParameter`: a parameter between 1
    /// and `2 pi / extent` goes to the nearer end.
    fn clip_ellipse_parameter(&self, p: &mut PathState) {
        let t = p.parameter();
        if t > 0.5 + PI / self.extent {
            p.put_t(if self.positive { 0.0 } else { 1.0 });
            self.path_changed(p);
        } else if t > 1.0 {
            p.put_t(if self.positive { 1.0 } else { 0.0 });
            self.path_changed(p);
        } else if !self.positive {
            p.put_t(1.0 - t);
        }
    }
}

/// Whether the conic `id` with record `d` is defined, as `conicPart.set`
/// copies it: a conic whose value is undefined (the image under an
/// undefined transformation) may still have a record.
fn conic_defined(c: &Construction, id: Id, d: &ConicData) -> bool {
    d.defined && c.value(id).is_defined()
}

/// `DoubleUtil.convertToAngleValue`: an angle in `[0, 2 pi]`.
fn angle_value(val: f64) -> f64 {
    if val > STANDARD_PRECISION && val < PI_2 {
        return val;
    }
    let mut value = val % PI_2;
    if -STANDARD_PRECISION < value && value < STANDARD_PRECISION {
        value = if val < 1.0 { 0.0 } else { PI_2 };
    } else if value < 0.0 {
        value += PI_2;
    }
    value
}

/// `GeoPoint.affineRatio(start, pt1, pt2)` for 2D points: `lambda` with
/// `pt2 = start + lambda * (pt1 - start)`.
fn affine_ratio(start: (f64, f64), pt1: (f64, f64), pt2: (f64, f64)) -> f64 {
    let abx = pt1.0 - start.0;
    let aby = pt1.1 - start.1;
    if abx.abs() > aby.abs() {
        (pt2.0 - start.0) / abx
    } else if aby.abs() > 0.0 {
        (pt2.1 - start.1) / aby
    } else {
        // `(cC.z - cA.z) / ABz` with all z 0
        f64::NAN
    }
}

/// `AlgoCircleTwoPoints`: the circle around `m` through `p`.
fn circle_through(m: (f64, f64), p: (f64, f64)) -> ConicData {
    ConicData::circle(m.0, m.1, my_length(p.0 - m.0, p.1 - m.1))
}

/// The value of an arc or a sector from its record: undefined when the
/// part is, or when its value is not finite (two rays, a degenerate
/// sector), which GeoGebra counts as defined.
pub(crate) fn value(sector: bool, a: &ArcData) -> Value {
    if !a.defined || !a.value.is_finite() {
        return Value::Undefined;
    }
    if sector {
        Value::Sector { area: a.value }
    } else {
        Value::Arc { length: a.value }
    }
}

impl ArcDef {
    /// Whether this makes an arc or sector, which has an [`ArcData`]
    /// record, rather than a number measured on one.
    pub(crate) fn has_record(self) -> bool {
        !matches!(
            self,
            ArcDef::Length(_) | ArcDef::Area(_) | ArcDef::Perimeter(_)
        )
    }

    fn is_sector(self) -> bool {
        match self {
            ArcDef::Circle { sector, .. }
            | ArcDef::Circum { sector, .. }
            | ArcDef::ConicPoints { sector, .. }
            | ArcDef::ConicParameters { sector, .. } => sector,
            _ => false,
        }
    }

    /// The record of the part for the current positions. It depends only on
    /// the parents (unlike GeoGebra's, which keeps the parameters and the
    /// measures of a degenerate part from earlier updates; those are not
    /// read here, see [`ArcData::is_curved`]).
    pub(crate) fn record(self, c: &Construction) -> ArcData {
        let mut a = ArcData::new(self.is_sector());
        match self {
            ArcDef::Circle {
                center, start, end, ..
            } => {
                // `AlgoConicPartCircle`
                let (Some(m), Some(p), Some(q)) = (c.point(center), c.point(start), c.point(end))
                else {
                    a.set_undefined();
                    return a;
                };
                a.conic = circle_through(m, p);
                // the temporary points P and Q put on the circle
                let s = conic_parameter(&a.conic, p, false);
                let t = conic_parameter(&a.conic, q, false);
                a.set_parameters(a.conic.defined, s, t, true);
            }
            ArcDef::Semicircle { a: pa, b: pb } => {
                // `AlgoSemicircle`: the circle around the midpoint through B
                let (Some(pa), Some(pb)) = (c.point(pa), c.point(pb)) else {
                    a.set_undefined();
                    return a;
                };
                let m = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
                a.conic = circle_through(m, pb);
                if !a.conic.defined {
                    a.set_undefined();
                    return a;
                }
                let alpha = (pb.1 - pa.1).atan2(pb.0 - pa.0);
                a.set_parameters(true, alpha, alpha + PI, true);
            }
            ArcDef::Circum {
                a: ia,
                b: ib,
                c: ic,
                ..
            } => {
                // `AlgoConicPartCircumcircleND` on `AlgoCircleThreePoints`
                let (Some(pa), Some(pb), Some(pc)) = (c.point(ia), c.point(ib), c.point(ic)) else {
                    a.set_undefined();
                    return a;
                };
                match circumcircle(pa, pb, pc) {
                    Circumcircle::Circle { cx, cy, r } => {
                        a.conic = ConicData::circle(cx, cy, r);
                        circum_circle(&mut a, pa, pb, pc);
                    }
                    Circumcircle::Lines => circum_degenerate(&mut a, pa, pb, pc),
                    Circumcircle::Undefined => a.set_undefined(),
                }
            }
            ArcDef::ConicPoints { conic, p, q, .. } => {
                // `AlgoConicPartConicPoints`
                let Some(d) = c.conic(conic) else {
                    a.set_undefined();
                    return a;
                };
                a.conic = d;
                let nan = (f64::NAN, f64::NAN);
                let s = conic_parameter(&d, c.point(p).unwrap_or(nan), true);
                let t = conic_parameter(&d, c.point(q).unwrap_or(nan), true);
                a.set_parameters(conic_defined(c, conic, &d), s, t, true);
            }
            ArcDef::ConicParameters { conic, s, t, .. } => {
                // `AlgoConicPartConicParameters` (`AlgoConicPart.compute`)
                let Some(d) = c.conic(conic) else {
                    a.set_undefined();
                    return a;
                };
                a.conic = d;
                let s = c.number(s).unwrap_or(f64::NAN);
                let t = c.number(t).unwrap_or(f64::NAN);
                a.set_parameters(conic_defined(c, conic, &d), s, t, true);
            }
            ArcDef::Length(_) | ArcDef::Area(_) | ArcDef::Perimeter(_) => a.set_undefined(),
        }
        a
    }
}

/// `AlgoConicPartCircumcircleND.compute` for a circle or a single point.
fn circum_circle(a: &mut ArcData, pa: (f64, f64), pb: (f64, f64), pc: (f64, f64)) {
    match a.conic.ty {
        ConicType::Circle => {
            // `computeCircle`: start angle from MA, end angle from MC
            let [mx, my] = a.conic.midpoint;
            let alpha = (pa.1 - my).atan2(pa.0 - mx);
            let beta = (pc.1 - my).atan2(pc.0 - mx);
            // the orientation of the triangle decides the direction
            let det = (pb.0 - pa.0) * (pc.1 - pa.1) - (pb.1 - pa.1) * (pc.0 - pa.0);
            let defined = a.conic.defined;
            a.set_parameters(defined, alpha, beta, det > 0.0);
        }
        // `computeSinglePoint` (`setParametersToSinglePoint`)
        ConicType::SinglePoint => {
            a.value = 0.0;
            a.defined = true;
        }
        _ => a.set_undefined(),
    }
}

/// `AlgoConicPartCircumcircleND.computeDegenerate`: the arc became the
/// segment `AC` (when `B` lies between them) or two rays from `A` and `C`
/// away from each other.
fn circum_degenerate(a: &mut ArcData, pa: (f64, f64), pb: (f64, f64), pc: (f64, f64)) {
    a.conic.ty = ConicType::ParallelLines;
    a.conic.defined = true;
    a.line_points = Some([pa, pc]);
    let (x, y, z) = join(pa, pc);
    let lambda = affine_ratio(pa, pc, pb);
    // NaN (A on C) is the segment case, as in GeoGebra
    #[allow(clippy::manual_range_contains)]
    if lambda < 0.0 || lambda > 1.0 {
        // two rays: the second from C along AC, the first from A the
        // other way
        a.conic.lines[1] = [x, y, z];
        a.conic.lines[0] = [-x, -y, -z];
        a.set_parameters(true, 0.0, 1.0, false);
    } else {
        a.conic.lines[0] = [x, y, z];
        a.set_parameters(true, 0.0, 1.0, true);
    }
}

/// `AlgoCircumferenceConic` for a part.
fn perimeter(a: &ArcData) -> f64 {
    let value = if a.defined { a.value } else { f64::NAN };
    let r = a.conic.half_axes[0];
    match (a.conic.ty, a.sector) {
        // value of a sector is its area: arc length = area * 2 / r
        (ConicType::Circle, true) => value * 2.0 / r + 2.0 * r,
        // value of an arc is its length; add the chord
        (ConicType::Circle, false) => value + 2.0 * r * (a.extent / 2.0).sin(),
        _ => f64::NAN,
    }
}

fn number(v: f64) -> Option<Value> {
    v.is_finite().then_some(Value::Number(v))
}

impl Group for ArcDef {
    fn kind(&self) -> Kind {
        match *self {
            ArcDef::Length(_) | ArcDef::Area(_) | ArcDef::Perimeter(_) => Kind::Number,
            d if d.is_sector() => Kind::Sector,
            _ => Kind::Arc,
        }
    }

    fn parents(&self) -> Parents {
        match *self {
            ArcDef::Circle {
                center, start, end, ..
            } => Parents::of(&[center, start, end]),
            ArcDef::Circum { a, b, c, .. } => Parents::of(&[a, b, c]),
            ArcDef::Semicircle { a, b } => Parents::of(&[a, b]),
            ArcDef::ConicPoints { conic, p, q, .. } => Parents::of(&[conic, p, q]),
            ArcDef::ConicParameters { conic, s, t, .. } => Parents::of(&[conic, s, t]),
            ArcDef::Length(a) | ArcDef::Area(a) | ArcDef::Perimeter(a) => Parents::of(&[a]),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            ArcDef::Circle {
                center, start, end, ..
            } => {
                c.expect(center, POINT)?;
                c.expect(start, POINT)?;
                c.expect(end, POINT)
            }
            ArcDef::Circum { a, b, c: cc, .. } => {
                c.expect(a, POINT)?;
                c.expect(b, POINT)?;
                c.expect(cc, POINT)
            }
            ArcDef::Semicircle { a, b } => {
                c.expect(a, POINT)?;
                c.expect(b, POINT)
            }
            ArcDef::ConicPoints { conic, p, q, .. } => {
                c.expect(conic, CIRCLE)?;
                c.expect(p, POINT)?;
                c.expect(q, POINT)
            }
            ArcDef::ConicParameters { conic, s, t, .. } => {
                c.expect(conic, CONIC)?;
                c.expect(s, NUMBER)?;
                c.expect(t, NUMBER)
            }
            ArcDef::Length(a) | ArcDef::Area(a) | ArcDef::Perimeter(a) => c.expect(a, PART),
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        match *self {
            ArcDef::Length(a) => number(c.arc(a)?.arc_length()),
            ArcDef::Area(a) => number(c.arc(a)?.area()),
            ArcDef::Perimeter(a) => number(perimeter(&c.arc(a)?)),
            d => Some(value(d.is_sector(), &d.record(c))),
        }
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        let n = args.len();
        // the English names and GeoGebra's internal ones
        let (sector, form) = match func {
            "CircularArc" | "CircleArc" => (false, "circle"),
            "CircularSector" | "CircleSector" => (true, "circle"),
            "CircumcircularArc" | "CircumcircleArc" => (false, "circum"),
            "CircumcircularSector" | "CircumcircleSector" => (true, "circum"),
            "Semicircle" => (false, "semicircle"),
            "Arc" => (false, "conic"),
            "Sector" => (true, "conic"),
            "Length" | "Area" | "Perimeter" | "Circumference" if n == 1 => {
                // only for an arc or sector; other objects are measured
                // elsewhere
                let a = object(c, args[0]).ok()?;
                if !PART.contains(&c.kind(a)) {
                    return None;
                }
                return Some(Ok(match func {
                    "Length" => ArcDef::Length(a),
                    "Area" => ArcDef::Area(a),
                    _ => ArcDef::Perimeter(a),
                }));
            }
            _ => return None,
        };
        let wanted = if form == "semicircle" { 2 } else { 3 };
        if n != wanted {
            return Some(Err(format!("`{func}` does not take {n} argument(s)")));
        }
        let mut ids = [Id(0); 3];
        for (slot, arg) in ids.iter_mut().zip(args) {
            match object(c, arg) {
                Ok(id) => *slot = id,
                Err(e) => return Some(Err(e)),
            }
        }
        let [x, y, z] = ids;
        Some(Ok(match form {
            "circle" => ArcDef::Circle {
                center: x,
                start: y,
                end: z,
                sector,
            },
            "circum" => ArcDef::Circum {
                a: x,
                b: y,
                c: z,
                sector,
            },
            "semicircle" => ArcDef::Semicircle { a: x, b: y },
            // `CmdArcSector`: two numbers or two points after the conic
            _ if c.kind(y) == Kind::Number && c.kind(z) == Kind::Number => {
                ArcDef::ConicParameters {
                    conic: x,
                    s: y,
                    t: z,
                    sector,
                }
            }
            _ => ArcDef::ConicPoints {
                conic: x,
                p: y,
                q: z,
                sector,
            },
        }))
    }
}
