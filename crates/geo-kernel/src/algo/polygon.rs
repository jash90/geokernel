// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdPolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdPolyLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/scripting/CmdRigidPolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/PolygonFactory.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDispatcher.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdArea.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdPerimeter.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdVertex.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdInteriorAngles.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdAngle.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdCentroid.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolygonRegular.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolygonRegularND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolyLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAreaPoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAreaPolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPerimeterPoly.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoCentroidPolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoVertexPolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAnglePolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAnglePolygonND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAnglePoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoElement.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoJoinPointsSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentList.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPolyLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoly.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoElement.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/LabelManager.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/LabelType.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Construction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Kernel.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/parser/function/ParserFunctionsFactory.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/matrix/Coords.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Polygons, polylines and what is measured on them, and other definitions
//! that read any number of points.
//!
//! These definitions keep their points in a list stored by the construction
//! (see [`Construction::add_with_list`]). A list cannot be stored from
//! [`Group::parse`], which only sees the construction, so the script lines of
//! this group are run by [`exec`], which the script layer asks first.
//!
//! A polygon's value holds only its signed area; its vertices are read from
//! its parents ([`Vertices`]). Because the value can stay equal while a
//! vertex moves, every definition that reads the vertices of a polygon also
//! reads the polygon's own parents (see [`hidden_parents`]).
//!
//! `p = Polygon(A, B, C, ...)` also creates the sides of the polygon, and
//! `Polygon(A, B, n)` its new vertices and its sides, labelled as GeoGebra
//! labels them (see [`exec`]). Like every command that creates several
//! objects, the set of objects is fixed when the line runs, and a line
//! creates all of its objects or none.

use super::measure::{angle_of_points, segment_length};
use super::polygon_clip::{ClipDef, ClipOutput};
use super::transform::{parse_transform_for, source_of, Map};
use super::vector::{matrix::Matrix, VectorDef};
use super::{Group, Parents, NUMBER, POINT};
use crate::expr::{Element, List};
use crate::kernel::{Construction, Def, Error, Id, IdList, Kind, Value};
use crate::script::{object, split_call};
use crate::tol::STANDARD_PRECISION;
use std::collections::BTreeSet;
use std::f64::consts::PI;

const POLYGON: &[Kind] = &[Kind::Polygon];
const POLY: &[Kind] = &[Kind::Polygon, Kind::PolyLine];

/// The most vertices of a regular polygon. GeoGebra has no limit; above it
/// the kernel refuses to create the polygon and holds it as undefined.
pub const MAX_REGULAR_VERTICES: usize = 1000;

/// A number argument: a literal or a number object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Count {
    Literal(f64),
    Object(Id),
}

impl Count {
    pub(crate) fn value(self, c: &Construction) -> f64 {
        match self {
            Count::Literal(v) => v,
            Count::Object(id) => c.number(id).unwrap_or(f64::NAN),
        }
    }

    pub(crate) fn id(self) -> Option<Id> {
        match self {
            Count::Object(id) => Some(id),
            Count::Literal(_) => None,
        }
    }

    pub(crate) fn check(self, c: &Construction) -> Result<(), Error> {
        match self {
            Count::Object(id) => c.expect(id, NUMBER),
            Count::Literal(v) if v.is_finite() => Ok(()),
            Count::Literal(_) => Err(Error::NotFinite),
        }
    }
}

/// Polygons, polylines and what is measured on them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PolygonDef {
    /// `Area(P1, ..., Pn)` for five or more points: the area of the polygon
    /// through the points in order (`AlgoAreaPoints`). Three and four points
    /// are `MeasureDef::Area3` and `MeasureDef::Area4`.
    AreaPoints(IdList),
    /// `Polygon(A, B, C, ...)`, also with two points or one (`AlgoPolygon`).
    Polygon(IdList),
    /// A polygon, vertex or segment produced by clipping a polygon.
    Clip(ClipDef),
    /// A vertex in a rigid polygon made from points, in its initial local frame.
    RigidVertex {
        anchor: Id,
        x: f64,
        y: f64,
    },
    /// A point of a rigid polygon, expressed in the frame of its first edge.
    RigidFrameVertex {
        anchor: Id,
        pivot: Id,
        along: f64,
        across: f64,
    },
    /// A vertex in a rigid copy whose shape follows its source polygon.
    RigidCopyVertex {
        poly: Id,
        anchor: Id,
        bearing: f64,
        k: u32,
    },
    /// `Polygon(A, B, n)`: the regular polygon with `round(n)` vertices
    /// counter-clockwise from `A` and `B` (`AlgoPolygonRegular`).
    Regular {
        a: Id,
        b: Id,
        n: Count,
    },
    /// Vertex `k` (2 or more) of `Polygon(A, B, n)`, one of the points the
    /// command creates.
    RegularVertex {
        a: Id,
        b: Id,
        n: Count,
        k: u32,
    },
    /// Side `k` of `Polygon(A, B, n)`, from vertex `k` to the next one.
    RegularSide {
        a: Id,
        b: Id,
        n: Count,
        k: u32,
    },
    /// `PolyLine(A, B, ...)` (`AlgoPolyLine`).
    PolyLine(IdList),
    PolygonFromList(Id),
    PolyLineFromList(Id),
    /// `Area(polygon)` (`AlgoAreaPolygon`).
    Area(Id),
    /// `Perimeter(polygon)` (`AlgoPerimeterPoly`).
    Perimeter(Id),
    /// `Centroid(polygon)` (`AlgoCentroidPolygon`).
    Centroid(Id),
    /// `Vertex(polygon or polyline, n)`: vertex `floor(n)`, counted from 1.
    Vertex {
        poly: Id,
        index: Count,
    },
    /// Output `k` (from 0) of `Vertex(polygon or polyline)`.
    VertexAt {
        poly: Id,
        k: u32,
    },
    /// A named vertex created by the same command as its polygon or polyline image.
    ImageVertex {
        poly: Id,
        source: Id,
    },
    /// Output `k` (from 0) of `InteriorAngles(polygon)` (`internal`) or
    /// `Angle(polygon)` (`AlgoAnglePolygon`).
    Angle {
        poly: Id,
        k: u32,
        internal: bool,
    },
}

/// `DoubleUtil.isEpsilon(e, x, y)`: whether `e` is zero compared with
/// `STANDARD_PRECISION` and with `x` and `y`.
fn is_epsilon(e: f64, x: f64, y: f64) -> bool {
    let ea = e.abs();
    !(ea > STANDARD_PRECISION
        || ea > x.abs() * STANDARD_PRECISION
        || ea > y.abs() * STANDARD_PRECISION)
}

/// `GeoPoint.det(P, Q)` for finite points.
#[inline]
fn det(p: (f64, f64), q: (f64, f64)) -> f64 {
    p.0 * q.1 - q.0 * p.1
}

/// `AlgoPolygon.calcAreaWithSign` for points of the plane: half the sum of
/// `GeoPoint.det` over the closed sequence; positive when the points run
/// counter-clockwise. `None` when a point is undefined or there are fewer
/// than two.
fn signed_area(c: &Construction, ids: &[Id]) -> Option<f64> {
    area_of(ids.iter().map(|&id| c.point(id)))
}

pub(super) fn area_of(mut vertices: impl Iterator<Item = Option<(f64, f64)>>) -> Option<f64> {
    let start = vertices.next()??;
    let mut prev = vertices.next()??;
    let mut sum = det(start, prev);
    for vertex in vertices {
        let p = vertex?;
        sum += det(prev, p);
        prev = p;
    }
    sum += det(prev, start);
    Some(sum / 2.0)
}

/// `(int) Math.round(n)` in Java: rounds half up, saturates to a `long`
/// (NaN gives 0), then keeps the low 32 bits, so 4294967299 gives 3.
fn java_round_to_int(n: f64) -> i32 {
    let f = n.floor();
    // `n - f` is exact, so this is exactly `floor(n + 1/2)`.
    let r = if n - f >= 0.5 { f + 1.0 } else { f };
    // `as i64` saturates and maps NaN to 0, like Java's `(long)`; `as i32`
    // then truncates to the low 32 bits, like Java's `(int)` of a `long`.
    (r as i64) as i32
}

/// The vertex count `AlgoPolygonRegularND.compute` derives from `n`: NaN
/// reads as 2, then `(int) Math.round(n)`, at least 2.
fn vertex_count(n: f64) -> usize {
    let n = if n.is_nan() { 2 } else { java_round_to_int(n) };
    n.max(2) as usize
}

/// `MyMath.cos`: a cosine of magnitude below 1e-16 is 0.
fn my_cos(phi: f64) -> f64 {
    let c = phi.cos();
    if c.abs() < 1e-16 {
        0.0
    } else {
        c
    }
}

/// A defined regular polygon: what `AlgoPolygonRegularND.compute` works out.
#[derive(Clone, Copy, Debug)]
struct Regular {
    a: (f64, f64),
    b: (f64, f64),
    center: (f64, f64),
    alpha: f64,
    n: usize,
}

impl Regular {
    /// `None` when GeoGebra's `checkUnDefined` holds: fewer than 3 vertices
    /// or an undefined point (and, in the kernel, too many vertices).
    fn new(c: &Construction, a: Id, b: Id, n: usize) -> Option<Regular> {
        if !(3..=MAX_REGULAR_VERTICES).contains(&n) {
            return None;
        }
        let (pa, pb) = (c.point(a)?, c.point(b)?);
        // Kernel.PI_2 / n: the centre angle.
        let alpha = 2.0 * PI / n as f64;
        let beta = (PI - alpha) / 2.0;
        // AlgoPolygonRegular.setCenterPoint
        let (mx, my) = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
        let (nx, ny) = (pa.1 - pb.1, pb.0 - pa.0);
        let tan_beta_half = beta.tan() / 2.0;
        Some(Regular {
            a: pa,
            b: pb,
            center: (mx + tan_beta_half * nx, my + tan_beta_half * ny),
            alpha,
            n,
        })
    }

    /// Vertex `k < n`: `A`, `B`, then `A` rotated about the centre by
    /// `k * alpha` (`rotatePoints`, `GeoPoint.rotate`).
    fn vertex(&self, k: usize) -> (f64, f64) {
        match k {
            0 => self.a,
            1 => self.b,
            _ => {
                let phi = k as f64 * self.alpha;
                let (cos, sin) = (my_cos(phi), phi.sin());
                let (x, y) = self.a;
                let (qx, qy) = self.center;
                (
                    (x - qx) * cos + (qy - y) * sin + qx,
                    (x - qx) * sin + (y - qy) * cos + qy,
                )
            }
        }
    }

    /// `AlgoPolygonRegularND.calcArea`, the 2D branch.
    fn signed_area(&self) -> f64 {
        let area = det(self.a, self.b) + det(self.b, self.center) + det(self.center, self.a);
        area * self.n as f64 / 2.0
    }
}

fn image_vertices(c: &Construction, poly: Id) -> &[Id] {
    // The image command adds its vertices as the polygon's first children.
    let children = c.children(poly);
    let count = children.iter().take_while(|&&id| {
        matches!(c.def(id), Def::Polygon(PolygonDef::ImageVertex { poly: owner, .. }) if owner == poly)
    }).count();
    &children[..count]
}

/// Whether the command rebuilds an image from its named vertices.
pub(super) fn has_fixed_vertices(c: &Construction, poly: Id) -> bool {
    match c.def(poly) {
        Def::Polygon(PolygonDef::Polygon(list) | PolygonDef::PolyLine(list)) => {
            c.list(list).len() >= 3
        }
        _ => !image_vertices(c, poly).is_empty(),
    }
}

/// Where the vertices of a polygon or polyline come from.
#[derive(Clone, Debug)]
enum Shape<'a> {
    Points(&'a [Id]),
    List(List<'a>),
    Regular(Regular),
    Clipped(Vec<(f64, f64)>),
    /// The image of another shape under a map: a polygon or polyline under
    /// `Mirror`, `Rotate`, `Dilate`, `Shear` or `Stretch`
    /// (`Transform.transformPoly` moves the points). Without a map (an
    /// undefined input) the vertices are holes, not gone.
    Moved(Box<Shape<'a>>, Option<Map>),
    /// Another shape translated by a vector (`GeoPoly.translate`).
    Shifted(Box<Shape<'a>>, Option<(f64, f64)>),
    Applied(Box<Shape<'a>>, Option<Matrix>),
    Empty,
}

impl<'a> Shape<'a> {
    fn of(c: &'a Construction, id: Id) -> Shape<'a> {
        let image = image_vertices(c, id);
        if !image.is_empty() {
            return Shape::Points(image);
        }
        match c.def(id) {
            Def::Polygon(PolygonDef::Polygon(list) | PolygonDef::PolyLine(list)) => {
                Shape::Points(c.list(list))
            }
            Def::Polygon(PolygonDef::PolygonFromList(id) | PolygonDef::PolyLineFromList(id)) => {
                point_list(c, id).map_or(Shape::Empty, Shape::List)
            }
            Def::Polygon(PolygonDef::Regular { a, b, n }) => {
                match Regular::new(c, a, b, vertex_count(n.value(c))) {
                    Some(r) => Shape::Regular(r),
                    None => Shape::Empty,
                }
            }
            Def::Polygon(PolygonDef::Clip(d)) if d.kind() == Kind::Polygon => {
                d.vertices(c).map_or(Shape::Empty, Shape::Clipped)
            }
            Def::Transform(def) if matches!(def.kind(), Kind::Polygon | Kind::PolyLine) => {
                Shape::Moved(Box::new(Shape::of(c, source_of(&def))), def.map(c))
            }
            Def::Vector(VectorDef::ApplyMatrix {
                kind: Kind::Polygon | Kind::PolyLine,
                src,
                matrix,
            }) => Shape::Applied(
                Box::new(Shape::of(c, src)),
                c.elements(matrix).and_then(Matrix::read),
            ),
            Def::Vector(VectorDef::Translate {
                kind: Kind::Polygon | Kind::PolyLine,
                src,
                by,
            }) => {
                let by = match c.value(by) {
                    Value::Vector { x, y } => Some((x, y)),
                    _ => None,
                };
                Shape::Shifted(Box::new(Shape::of(c, src)), by)
            }
            _ => Shape::Empty,
        }
    }

    fn len(&self) -> usize {
        match self {
            Shape::Points(ids) => ids.len(),
            Shape::List(list) => list.len(),
            Shape::Regular(r) => r.n,
            Shape::Clipped(vertices) => vertices.len(),
            Shape::Moved(inner, ..) | Shape::Shifted(inner, ..) | Shape::Applied(inner, ..) => {
                inner.len()
            }
            Shape::Empty => 0,
        }
    }

    fn raw_vertex(&self, c: &Construction, k: usize) -> Option<Value> {
        match self {
            Shape::Points(ids) => Some(c.stored_value(*ids.get(k)?)),
            Shape::List(list) => list.raw_point(k),
            Shape::Applied(inner, matrix) => (*matrix)?.apply(inner.raw_vertex(c, k)?),
            _ => {
                let (x, y) = self.vertex(c, k)?;
                Some(Value::Point { x, y })
            }
        }
    }

    fn vertex(&self, c: &Construction, k: usize) -> Option<(f64, f64)> {
        match self {
            Shape::Points(_) | Shape::List(_) | Shape::Applied(..) => {
                match self.raw_vertex(c, k)?.cartesian() {
                    Value::Point { x, y } => Some((x, y)),
                    _ => None,
                }
            }
            Shape::Regular(r) => (k < r.n).then(|| r.vertex(k)),
            Shape::Clipped(vertices) => vertices.get(k).copied(),
            Shape::Moved(inner, map) => {
                let (x, y) = inner.vertex(c, k)?;
                map.map(|m| m.point(x, y))
            }
            Shape::Shifted(inner, by) => {
                let (x, y) = inner.vertex(c, k)?;
                by.map(|(dx, dy)| (x + dx, y + dy))
            }
            Shape::Empty => None,
        }
    }
}

fn point_list(c: &Construction, id: Id) -> Option<List<'_>> {
    let list = c.elements(id)?;
    list.iter()
        .all(|element| matches!(element, Element::Value(Kind::Point, _)))
        .then_some(list)
}

fn list_point(element: Element<'_>) -> Option<(f64, f64)> {
    match element {
        Element::Value(Kind::Point, Value::Point { x, y }) => Some((x, y)),
        _ => None,
    }
}

fn polyline_length(vertices: impl Iterator<Item = Option<(f64, f64)>>) -> Value {
    let mut length = 0.0;
    let mut previous = None;
    for current in vertices {
        if let (Some((x, y)), Some((u, v))) = (current, previous) {
            let (dx, dy): (f64, f64) = (x - u, y - v);
            length += (dx * dx + dy * dy).sqrt();
        }
        previous = current;
    }
    Value::PolyLine { length }
}

/// The vertices of a polygon or polyline, in order (see
/// [`Construction::vertices`]).
#[derive(Clone)]
pub struct Vertices<'a> {
    c: &'a Construction,
    shape: Shape<'a>,
    next: usize,
}

impl<'a> Vertices<'a> {
    pub(crate) fn of(c: &'a Construction, id: Id) -> Vertices<'a> {
        Vertices {
            c,
            shape: Shape::of(c, id),
            next: 0,
        }
    }
}

impl Iterator for Vertices<'_> {
    type Item = Option<(f64, f64)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.shape.len() {
            return None;
        }
        let v = self.shape.vertex(self.c, self.next);
        self.next += 1;
        Some(v)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.shape.len().saturating_sub(self.next);
        (left, Some(left))
    }
}

/// Whether `poly` counts as defined for the commands that read its
/// vertices: a polygon when its area is defined, a polyline always
/// (`GeoPolyLine.isDefined` holds whenever it has points).
fn poly_defined(c: &Construction, poly: Id) -> bool {
    c.kind(poly) == Kind::PolyLine || c.value(poly).is_defined()
}

/// The objects that decide the vertices of a polygon or polyline: the points
/// of its list, or `A`, `B` and `n` of a regular polygon.
pub(crate) fn inputs(c: &Construction, poly: Id) -> Vec<Id> {
    let image = image_vertices(c, poly);
    if !image.is_empty() {
        return image.to_vec();
    }
    match c.def(poly) {
        Def::Polygon(PolygonDef::Polygon(list) | PolygonDef::PolyLine(list)) => {
            c.list(list).to_vec()
        }
        Def::Polygon(PolygonDef::PolygonFromList(list) | PolygonDef::PolyLineFromList(list)) => {
            vec![list]
        }
        Def::Polygon(PolygonDef::Regular { a, b, n }) => {
            let mut ids = vec![a, b];
            ids.extend(n.id());
            ids
        }
        Def::Polygon(PolygonDef::Clip(d)) => {
            let mut ids = d.parents().as_slice().to_vec();
            ids.extend(d.hidden_parents(c));
            ids
        }
        // the image reads the vertices of the object it transforms, and so
        // the objects they come from
        Def::Transform(def) if matches!(def.kind(), Kind::Polygon | Kind::PolyLine) => {
            inputs(c, source_of(&def))
        }
        Def::Vector(VectorDef::ApplyMatrix {
            kind: Kind::Polygon | Kind::PolyLine,
            src,
            matrix,
        }) => {
            let mut ids = inputs(c, src);
            ids.push(matrix);
            ids
        }
        Def::Vector(VectorDef::Translate {
            kind: Kind::Polygon | Kind::PolyLine,
            src,
            ..
        }) => inputs(c, src),
        _ => Vec::new(),
    }
}

/// Objects a definition of this group reads besides its parents and its
/// list: whatever reads the vertices of a polygon also reads the polygon's
/// own parents, because the polygon's value (its area) can stay equal while
/// the vertices move. A shear with a vector axis also reads its start point.
pub(crate) fn hidden_parents(c: &Construction, def: &Def) -> Vec<Id> {
    let mut parents = match *def {
        Def::Polygon(PolygonDef::Clip(d)) => d.hidden_parents(c),
        Def::Polygon(PolygonDef::ImageVertex { poly, .. }) => {
            let transform = c.def(poly);
            let mut ids = transform.parents().as_slice().to_vec();
            ids.extend(super::transform::shear_start_parent(c, &transform));
            ids
        }
        Def::Polygon(
            PolygonDef::Perimeter(poly)
            | PolygonDef::Centroid(poly)
            | PolygonDef::Vertex { poly, .. }
            | PolygonDef::VertexAt { poly, .. }
            | PolygonDef::Angle { poly, .. }
            | PolygonDef::RigidCopyVertex { poly, .. },
        ) => inputs(c, poly),
        // a transformed or translated polygon reads the vertices of the
        // object it moves
        Def::Transform(def) if matches!(def.kind(), Kind::Polygon | Kind::PolyLine) => {
            inputs(c, source_of(&def))
        }
        Def::Vector(
            VectorDef::Translate {
                kind: Kind::Polygon | Kind::PolyLine,
                src,
                ..
            }
            | VectorDef::ApplyMatrix {
                kind: Kind::Polygon | Kind::PolyLine,
                src,
                ..
            },
        ) => inputs(c, src),
        _ => Vec::new(),
    };
    parents.extend(super::transform::shear_start_parent(c, def));
    parents
}

fn point(p: (f64, f64)) -> Option<Value> {
    Some(Value::Point { x: p.0, y: p.1 })
}

impl Group for PolygonDef {
    fn kind(&self) -> Kind {
        match self {
            PolygonDef::Clip(d) => d.kind(),
            PolygonDef::AreaPoints(_) | PolygonDef::Area(_) | PolygonDef::Perimeter(_) => {
                Kind::Number
            }
            PolygonDef::Angle { .. } => Kind::Number,
            PolygonDef::RigidVertex { .. }
            | PolygonDef::RigidCopyVertex { .. }
            | PolygonDef::RigidFrameVertex { .. } => Kind::Point,
            PolygonDef::Polygon(_)
            | PolygonDef::PolygonFromList(_)
            | PolygonDef::Regular { .. } => Kind::Polygon,
            PolygonDef::PolyLine(_) | PolygonDef::PolyLineFromList(_) => Kind::PolyLine,
            PolygonDef::RegularSide { .. } => Kind::Segment,
            PolygonDef::RegularVertex { .. }
            | PolygonDef::Centroid(_)
            | PolygonDef::Vertex { .. }
            | PolygonDef::VertexAt { .. }
            | PolygonDef::ImageVertex { .. } => Kind::Point,
        }
    }

    fn parents(&self) -> Parents {
        match *self {
            PolygonDef::Clip(d) => d.parents(),
            PolygonDef::RigidVertex { anchor, .. } => Parents::of(&[anchor]),
            PolygonDef::RigidFrameVertex { anchor, pivot, .. } => Parents::of(&[anchor, pivot]),
            PolygonDef::ImageVertex { poly, source } => Parents::of(&[poly, source]),
            PolygonDef::RigidCopyVertex { poly, anchor, .. } => Parents::of(&[poly, anchor]),
            PolygonDef::AreaPoints(_) | PolygonDef::Polygon(_) | PolygonDef::PolyLine(_) => {
                Parents::none()
            }
            PolygonDef::Regular { a, b, n }
            | PolygonDef::RegularVertex { a, b, n, .. }
            | PolygonDef::RegularSide { a, b, n, .. } => match n.id() {
                Some(n) => Parents::of(&[a, b, n]),
                None => Parents::of(&[a, b]),
            },
            PolygonDef::PolygonFromList(p)
            | PolygonDef::PolyLineFromList(p)
            | PolygonDef::Area(p)
            | PolygonDef::Perimeter(p)
            | PolygonDef::Centroid(p)
            | PolygonDef::VertexAt { poly: p, .. }
            | PolygonDef::Angle { poly: p, .. } => Parents::of(&[p]),
            PolygonDef::Vertex { poly, index } => match index.id() {
                Some(i) => Parents::of(&[poly, i]),
                None => Parents::of(&[poly]),
            },
        }
    }

    fn list(&self) -> Option<IdList> {
        match *self {
            PolygonDef::AreaPoints(list)
            | PolygonDef::Polygon(list)
            | PolygonDef::PolyLine(list) => Some(list),
            _ => None,
        }
    }

    fn with_list(self, list: IdList) -> Self {
        match self {
            PolygonDef::AreaPoints(_) => PolygonDef::AreaPoints(list),
            PolygonDef::Polygon(_) => PolygonDef::Polygon(list),
            PolygonDef::PolyLine(_) => PolygonDef::PolyLine(list),
            other => other,
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            PolygonDef::Clip(d) => d.check(c),
            PolygonDef::PolygonFromList(id) | PolygonDef::PolyLineFromList(id) => {
                c.expect(id, &[Kind::List])
            }
            PolygonDef::RigidFrameVertex {
                anchor,
                pivot,
                along,
                across,
            } => {
                c.expect(anchor, POINT)?;
                c.expect(pivot, POINT)?;
                if along.is_finite() && across.is_finite() {
                    Ok(())
                } else {
                    Err(Error::NotFinite)
                }
            }
            PolygonDef::RigidVertex { anchor, x, y } => {
                c.expect(anchor, POINT)?;
                if x.is_finite() && y.is_finite() {
                    Ok(())
                } else {
                    Err(Error::NotFinite)
                }
            }
            PolygonDef::RigidCopyVertex { poly, anchor, .. } => {
                c.expect(poly, &[Kind::Polygon])?;
                c.expect(anchor, POINT)
            }
            PolygonDef::AreaPoints(list)
            | PolygonDef::Polygon(list)
            | PolygonDef::PolyLine(list) => {
                c.expect_list(list, POINT)?;
                if list.is_empty() {
                    Err(Error::UnknownObject)
                } else {
                    Ok(())
                }
            }
            PolygonDef::Regular { a, b, n }
            | PolygonDef::RegularVertex { a, b, n, .. }
            | PolygonDef::RegularSide { a, b, n, .. } => {
                c.expect(a, POINT)?;
                c.expect(b, POINT)?;
                n.check(c)
            }
            PolygonDef::Area(p) | PolygonDef::Perimeter(p) | PolygonDef::Centroid(p) => {
                c.expect(p, POLYGON)
            }
            PolygonDef::Angle { poly, .. } => c.expect(poly, POLYGON),
            PolygonDef::VertexAt { poly, .. } => c.expect(poly, POLY),
            PolygonDef::ImageVertex { poly, source } => {
                c.expect(poly, POLY)?;
                c.expect(source, POINT)
            }
            PolygonDef::Vertex { poly, index } => {
                c.expect(poly, POLY)?;
                index.check(c)
            }
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        match *self {
            PolygonDef::Clip(d) => d.compute(c),
            PolygonDef::ImageVertex { poly, source } => match c.def(poly) {
                Def::Transform(def) => def.of(Kind::Point, source).compute(c),
                Def::Vector(VectorDef::ApplyMatrix { matrix, .. }) => VectorDef::ApplyMatrix {
                    matrix,
                    src: source,
                    kind: Kind::Point,
                }
                .compute(c),
                Def::Vector(VectorDef::Translate { by, .. }) => VectorDef::Translate {
                    kind: Kind::Point,
                    src: source,
                    by,
                }
                .compute(c),
                _ => None,
            },
            PolygonDef::RigidFrameVertex {
                anchor,
                pivot,
                along,
                across,
            } => {
                let (x, y) = c.point(anchor)?;
                let (px, py) = c.point(pivot)?;
                let (dx, dy) = (px - x, py - y);
                let length = dx.hypot(dy);
                let (cos, sin) = (dx / length, dy / length);
                point((
                    x + along * cos - across * sin,
                    y + along * sin + across * cos,
                ))
            }
            PolygonDef::RigidVertex { anchor, x, y } => {
                let (ax, ay) = c.point(anchor)?;
                point((ax + x, ay + y))
            }
            PolygonDef::RigidCopyVertex {
                poly,
                anchor,
                bearing,
                k,
            } => {
                let shape = Shape::of(c, poly);
                let p = shape.vertex(c, 0)?;
                let q = shape.vertex(c, 1)?;
                let vertex = shape.vertex(c, k as usize)?;
                let (x, y) = (vertex.0 - p.0, vertex.1 - p.1);
                let (sin, cos) = (bearing - (q.1 - p.1).atan2(q.0 - p.0)).sin_cos();
                let (ax, ay) = c.point(anchor)?;
                point((ax + x * cos - y * sin, ay + x * sin + y * cos))
            }
            PolygonDef::AreaPoints(list) => {
                Some(Value::Number(signed_area(c, c.list(list))?.abs()))
            }
            PolygonDef::Polygon(list) => Some(Value::Polygon {
                area: signed_area(c, c.list(list))?,
            }),
            PolygonDef::Regular { a, b, n } => {
                let r = Regular::new(c, a, b, vertex_count(n.value(c)))?;
                Some(Value::Polygon {
                    area: r.signed_area(),
                })
            }
            PolygonDef::RegularVertex { a, b, n, k } => {
                let r = Regular::new(c, a, b, vertex_count(n.value(c)))?;
                let k = k as usize;
                (k < r.n).then(|| r.vertex(k)).and_then(point)
            }
            PolygonDef::RegularSide { a, b, n, k } => regular_side(c, a, b, n, k as usize),
            PolygonDef::PolygonFromList(id) => Some(Value::Polygon {
                area: area_of(point_list(c, id)?.iter().map(list_point))?,
            }),
            PolygonDef::PolyLineFromList(id) => {
                Some(polyline_length(point_list(c, id)?.iter().map(list_point)))
            }
            PolygonDef::PolyLine(list) => {
                Some(polyline_length(c.list(list).iter().map(|&id| c.point(id))))
            }
            PolygonDef::Area(p) => match c.value(p) {
                Value::Polygon { area } => Some(Value::Number(area.abs())),
                _ => None,
            },
            PolygonDef::Perimeter(p) => {
                if !c.value(p).is_defined() {
                    return None;
                }
                let shape = Shape::of(c, p);
                let n = shape.len();
                let mut length = 0.0;
                for i in 0..n {
                    let (s, e) = (shape.vertex(c, i)?, shape.vertex(c, (i + 1) % n)?);
                    length += segment_length(s.0, s.1, e.0, e.1);
                }
                Some(Value::Number(length))
            }
            PolygonDef::Centroid(p) => centroid(c, p),
            PolygonDef::Vertex { poly, index } => {
                if !poly_defined(c, poly) {
                    return None;
                }
                // `(int) Math.floor(index) - 1`: `as i32` saturates and maps
                // NaN to 0 like Java's `(int)`, and the subtraction wraps
                // like Java's `int` arithmetic.
                let i = (index.value(c).floor() as i32).wrapping_sub(1);
                let shape = Shape::of(c, poly);
                let i = usize::try_from(i).ok().filter(|&i| i < shape.len())?;
                point(shape.vertex(c, i)?)
            }
            PolygonDef::VertexAt { poly, k } => {
                if !poly_defined(c, poly) {
                    return None;
                }
                point(Shape::of(c, poly).vertex(c, k as usize)?)
            }
            PolygonDef::Angle { poly, k, internal } => {
                let shape = Shape::of(c, poly);
                let (len, k) = (shape.len(), k as usize);
                if k >= len {
                    return None;
                }
                let ccw = matches!(c.value(poly), Value::Polygon { area } if area > 0.0);
                let dir = if !internal || ccw { 1 } else { len - 1 };
                let a = shape.vertex(c, (k + dir) % len)?;
                let b = shape.vertex(c, k)?;
                let d = shape.vertex(c, (k + len - dir) % len)?;
                Some(Value::Number(angle_of_points(a, b, d)?))
            }
        }
    }

    /// The commands of this group need to store a list; see [`exec`].
    fn parse(_c: &Construction, _func: &str, _args: &[&str]) -> Option<Result<Self, String>> {
        None
    }
}

/// Side `k` of a regular polygon. With fewer than 3 vertices GeoGebra keeps
/// the first two sides as `A`-`B` and `B`-`A` (`updateOutput`).
fn regular_side(c: &Construction, a: Id, b: Id, n: Count, k: usize) -> Option<Value> {
    let count = vertex_count(n.value(c));
    let (s, e) = if count < 3 {
        let (pa, pb) = (c.point(a)?, c.point(b)?);
        match k {
            0 => (pa, pb),
            1 => (pb, pa),
            _ => return None,
        }
    } else {
        let r = Regular::new(c, a, b, count)?;
        if k >= r.n {
            return None;
        }
        (r.vertex(k), r.vertex((k + 1) % r.n))
    };
    Some(Value::Segment {
        x1: s.0,
        y1: s.1,
        x2: e.0,
        y2: e.1,
    })
}

/// `AlgoPolygon.calcCentroid` and `AlgoPolygonRegular.calcCentroid`, read as
/// GeoGebra's point would be: a weight that `DoubleUtil.isEpsilon` finds
/// zero makes a point at infinity, which is undefined here.
fn centroid(c: &Construction, p: Id) -> Option<Value> {
    let Value::Polygon { area } = c.value(p) else {
        return None;
    };
    let ids = match (c.def(p), Shape::of(c, p)) {
        (_, Shape::Regular(r)) => return point(r.center),
        (_, Shape::Points(ids)) => ids,
        _ => return None,
    };
    let n = ids.len();
    let (mut xsum, mut ysum) = (0.0, 0.0);
    for i in 0..n {
        let (p, q) = (c.point(ids[i])?, c.point(ids[(i + 1) % n])?);
        let factor = p.0 * q.1 - q.0 * p.1;
        xsum += (p.0 + q.0) * factor;
        ysum += (p.1 + q.1) * factor;
    }
    let z = 6.0 * area;
    if is_epsilon(z, xsum, ysum) {
        return None;
    }
    point((xsum / z, ysum / z))
}

// ------------------------------------------------------------------ labels

/// `LabelType.pointLabels`.
const POINT_LABELS: &[char] = &[
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'Z',
];

/// `LabelType.lineLabels`, also used for segments.
const LINE_LABELS: &[char] = &[
    'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'p', 'q', 'r', 's', 't', 'a', 'b', 'c', 'd', 'e',
];

/// The names GeoGebra reserves for built-in functions
/// (`ParserFunctionsFactory`); no object may take them.
const RESERVED: &[&str] = &[
    "Dirac",
    "Heaviside",
    "LambertW",
    "NRoot",
    "abs",
    "acos",
    "acosd",
    "acosh",
    "alt",
    "arbcomplex",
    "arbconst",
    "arbint",
    "arccos",
    "arccosd",
    "arccosh",
    "arcosh",
    "arcsin",
    "arcsind",
    "arcsinh",
    "arctan",
    "arctan2",
    "arctan2d",
    "arctand",
    "arctanh",
    "arg",
    "arsinh",
    "artanh",
    "asin",
    "asind",
    "asinh",
    "atan",
    "atan2",
    "atan2d",
    "atand",
    "atanh",
    "beta",
    "betaRegularized",
    "cbrt",
    "ceil",
    "conjugate",
    "cos",
    "cosIntegral",
    "cosec",
    "cosech",
    "cosh",
    "cot",
    "cotan",
    "cotanh",
    "coth",
    "csc",
    "csch",
    "deg",
    "erf",
    "exp",
    "expIntegral",
    "floor",
    "fractionalPart",
    "fractionalpart",
    "freehand",
    "gGbIfElSe",
    "gGbInTeGrAl",
    "gGbPrOdUcT",
    "gGbSuBsTiTuTiOn",
    "gGbSuM",
    "gamma",
    "gammaRegularized",
    "ggbdiff",
    "ibeta",
    "imaginary",
    "ld",
    "lg",
    "ln",
    "log",
    "log10",
    "log2",
    "nCr",
    "nPr",
    "nroot",
    "polygamma",
    "psi",
    "random",
    "real",
    "round",
    "sec",
    "sech",
    "sgn",
    "sign",
    "sin",
    "sinIntegral",
    "sinh",
    "sqrt",
    "tan",
    "tanh",
    "vectorize",
    "x",
    "y",
    "z",
    "zeta",
    "\u{3af}",
    "\u{3c0}",
    "\u{212f}",
    "\u{212f}_\u{3b3}",
];

/// Labels already given to objects of the line being planned.
type Taken = BTreeSet<String>;

/// `Construction.isFreeLabel`: not taken (by the construction or by an
/// earlier object of the same line) and not reserved.
fn is_free(c: &Construction, taken: &Taken, label: &str) -> bool {
    c.id(label).is_none() && !taken.contains(label) && !RESERVED.contains(&label)
}

/// `LabelManager.getNextIndexedLabel`: the first free label of `c0, c1,
/// ..., c0_1, c1_1, ...` (the short form `c_1` below index 10, which needs
/// `c_{1}` to be free too, and `c_{10}` from 10 on).
fn next_indexed_label(c: &Construction, taken: &Taken, chars: &[char]) -> String {
    let base = |ch: char| {
        // this Arabic letter is two Unicode chars
        if ch == '\u{647}' {
            "\u{647}\u{640}".to_string()
        } else {
            ch.to_string()
        }
    };
    // Every counter below the hint is taken by the construction (see
    // `Construction::label_hint`); `taken` holds only this line's labels, so
    // it does not move the hint.
    let family: String = format!("L:{}", chars.iter().collect::<String>());
    let none = Taken::new();
    let mut counter = c.label_hint(&family);
    let mut first_open = None;
    loop {
        let (q, r) = (counter / chars.len(), counter % chars.len());
        let b = base(chars[r]);
        let at = counter;
        counter += 1;
        if q == 0 {
            if is_free(c, &none, &b) {
                first_open.get_or_insert(at);
                if is_free(c, taken, &b) {
                    c.set_label_hint(&family, first_open.unwrap_or(at));
                    return b;
                }
            }
            continue;
        }
        let short = format!("{b}_{q}");
        let long = format!("{b}_{{{q}}}");
        if is_free(c, &none, &short) && is_free(c, &none, &long) {
            first_open.get_or_insert(at);
            if is_free(c, taken, &short) && is_free(c, taken, &long) {
                c.set_label_hint(&family, first_open.unwrap_or(at));
                return if q < 10 { short } else { long };
            }
        }
    }
}

/// `GeoPolygon.setLabel(segment, point)`: the label of a side named after
/// the vertex `p`: the vertex label in lower case, with an index when that
/// is a single character, or the next default label for segments when a
/// longer lower-case label is taken.
fn side_label(c: &Construction, taken: &Taken, vertex: &str) -> String {
    let lower = vertex.to_lowercase();
    let mut units = lower.encode_utf16();
    if let (Some(unit), None) = (units.next(), units.next()) {
        if let Some(ch) = char::from_u32(u32::from(unit)) {
            return next_indexed_label(c, taken, &[ch]);
        }
    }
    if is_free(c, taken, &lower) {
        lower
    } else {
        next_indexed_label(c, taken, LINE_LABELS)
    }
}

/// `Construction.buildIndexedLabel` (as `script::index_label`), also skipping
/// labels given earlier in the same line.
fn planned_index_label(c: &Construction, taken: &Taken, prefix: &str) -> String {
    let free = |label: &str| c.id(label).is_none() && !taken.contains(label);
    let in_construction = |label: &str| c.id(label).is_none();
    let family = format!("I:{prefix}");
    let mut n = c.label_hint(&family).max(1);
    let mut first_open = None;
    loop {
        let long = format!("{prefix}_{{{n}}}");
        let short = format!("{prefix}_{n}");
        if in_construction(&long) && (n >= 10 || in_construction(&short)) {
            first_open.get_or_insert(n);
            if free(&long) && (n >= 10 || free(&short)) {
                c.set_label_hint(&family, first_open.unwrap_or(n));
                return long;
            }
        }
        n += 1;
    }
}

// ----------------------------------------------------------------- scripts

/// One object a line will create.
enum Planned {
    Def(Def),
    /// A definition over a list of points, stored when it is added.
    List(Vec<Id>, fn(IdList) -> PolygonDef),
}

/// The objects one line creates, with their labels, checked before the
/// first one is added.
#[derive(Default)]
struct Plan {
    items: Vec<(String, Planned)>,
    taken: Taken,
}

impl Plan {
    /// A plan whose first object is labelled `name`, which must be free.
    fn new(c: &Construction, name: &str) -> Result<Plan, String> {
        if name.is_empty() {
            return Err(Error::EmptyLabel.to_string());
        }
        if c.id(name).is_some() {
            return Err(Error::DuplicateLabel(name.to_string()).to_string());
        }
        let mut plan = Plan::default();
        plan.taken.insert(name.to_string());
        Ok(plan)
    }

    /// Plans `item` under `label`, a label chosen as free for this plan.
    fn push(&mut self, label: String, item: Planned) {
        self.taken.insert(label.clone());
        self.items.push((label, item));
    }

    /// Plans the first object, under the name given to [`Plan::new`].
    fn first(&mut self, name: &str, item: Planned) {
        self.items.push((name.to_string(), item));
    }

    /// Adds every planned object, or none: each definition is checked first,
    /// and if the construction still refuses one, everything added by this
    /// call is removed again. Returns the first object.
    fn commit(self, c: &mut Construction) -> Result<Id, String> {
        for (_, item) in &self.items {
            match item {
                Planned::Def(def) => {
                    c.evaluate(*def).map_err(|e| e.to_string())?;
                }
                Planned::List(ids, _) => {
                    for &id in ids {
                        c.expect(id, POINT).map_err(|e| e.to_string())?;
                    }
                }
            }
        }
        let start = c.checkpoint();
        let mut first = None;
        for (label, item) in self.items {
            let added = match item {
                Planned::Def(def) => c.add(&label, def),
                Planned::List(ids, make) => {
                    c.add_with_list(&label, &ids, |list| Def::Polygon(make(list)))
                }
            };
            match added {
                Ok(id) => {
                    first.get_or_insert(id);
                }
                Err(e) => {
                    c.discard_since(start);
                    return Err(e.to_string());
                }
            }
        }
        first.ok_or_else(|| "the command creates no object".to_string())
    }
}

/// Runs `name = rhs` when it is a command of this group. Returns `None` for
/// every other line, so that the other groups and the built-in commands can
/// try; otherwise what [`crate::script::exec`] returns: the first object
/// created. A line creates all its objects or, when it fails, none.
///
/// Objects created by one line, in this order:
///
/// - `p = Polygon(A, B, C, ...)`: the polygon `p`, then those of its sides
///   from each vertex to the next that are defined when the line runs (as
///   GeoGebra labels only those). A side is named after a vertex
///   (`GeoPolygon`'s `defaultSegmentLabels`): for a triangle the side
///   opposite the vertex, otherwise the side starting at it; the vertex
///   label in lower case, `a`, then `a_1`, `a_2`, ... when taken (`a_{10}`
///   from 10 on, and `x`, `y`, `z` are reserved); a longer label in lower
///   case when it is free, else the next default segment label `f`, `g`,
///   `h`, ... `Polygon(A)` creates no side.
/// - `p = Polygon(A, B, n)`: the polygon and, when `n` gives three or more
///   vertices as the line runs, the new vertices from the third on with
///   default point labels (the next free of `A`, `B`, ..., `Z`), then, when
///   `A` and `B` are defined then, the sides with default segment labels.
///   Their number is fixed by `n` when the line runs.
/// - `V = Vertex(p)`, `w = InteriorAngles(p)`, `w = Angle(p)`: one object per
///   vertex at creation (one when the polygon is undefined then for
///   `Vertex`), labelled `V_{1}`, `V_{2}`, ... even when there is one.
pub(crate) fn exec(
    c: &mut Construction,
    name: &str,
    rhs: &str,
) -> Option<Result<Option<Id>, String>> {
    let (func, args) = split_call(rhs)?;
    let n = args.len();
    let kind_of = |c: &Construction, arg: &str| c.id(arg).map(|id| c.kind(id));
    let result = match func {
        "Polygon" => polygon_line(c, name, &args),
        "RigidPolygon" => rigid_polygon_line(c, name, &args),
        "IntersectPath" | "Difference" => clip_line(c, name, func, &args),
        "Segment" if let [a, b, polygon] = args.as_slice() => {
            let checked = (|| {
                points(c, &[*a, *b])?;
                let polygon = object(c, polygon)?;
                c.expect(polygon, &[Kind::Polygon])
                    .map_err(|e| e.to_string())?;
                Ok(None)
            })();
            return Some(checked);
        }
        "Segment"
            if n == 2
                && match kind_of(c, args[1]) {
                    Some(k) => k == Kind::Number,
                    None => decimal(args[1]).is_some(),
                } =>
        {
            fixed_segment_line(c, name, args[0], args[1])
        }
        "PolyLine" if kind_of(c, args[0]) == Some(Kind::List) && n <= 2 => {
            if n == 1
                || matches!(args[1], "true" | "false")
                || kind_of(c, args[1]) == Some(Kind::Boolean)
            {
                single(c, name, args[0], PolygonDef::PolyLineFromList)
            } else {
                Err("`PolyLine` needs a boolean second argument with a list".into())
            }
        }
        "PolyLine" => {
            // `CmdPolyLine`: a single argument must be a list.
            if n < 2 {
                Err(format!(
                    "`PolyLine` does not take {} argument(s)",
                    arg_count(&args)
                ))
            } else {
                points_line(c, name, &args, PolygonDef::PolyLine)
            }
        }
        // `CmdArea`: any number of points above two; three and four are
        // handled by the measuring group.
        "Area" if n >= 5 => points_line(c, name, &args, PolygonDef::AreaPoints),
        "Area" if n == 1 && kind_of(c, args[0]) == Some(Kind::Polygon) => {
            single(c, name, args[0], PolygonDef::Area)
        }
        "Perimeter" if n == 1 && kind_of(c, args[0]) == Some(Kind::Polygon) => {
            single(c, name, args[0], PolygonDef::Perimeter)
        }
        "Centroid" if n == 1 => single(c, name, args[0], PolygonDef::Centroid),
        "Centroid" => Err(format!("`Centroid` does not take {n} argument(s)")),
        // `Vertex(k)` of a general conic belongs to the conic readers
        "Vertex" | "Corner" if !(n == 1 && kind_of(c, args[0]) == Some(Kind::Conic)) => {
            vertex_line(c, name, &args)
        }
        "InteriorAngles" if n == 1 => outputs_line(c, name, args[0], Outputs::Angles(true)),
        "InteriorAngles" => Err(format!("`InteriorAngles` does not take {n} argument(s)")),
        "Angle" if n == 1 && kind_of(c, args[0]) == Some(Kind::Polygon) => {
            outputs_line(c, name, args[0], Outputs::Angles(false))
        }
        // Transform.transformPoly rebuilds a fixed-vertex polygon or
        // polyline from the images of its vertices.
        "Mirror" | "Reflect" | "Rotate" | "Dilate" | "Shear" | "Stretch" | "Translate"
            if n >= 2 =>
        {
            let src = c.id(args[0])?;
            if !POLY.contains(&c.kind(src)) || !has_fixed_vertices(c, src) {
                return None;
            }
            transform_line(c, name, func, &args, src)
        }
        _ => return None,
    };
    Some(result.map(Some))
}

/// Transform.transformPoly creates named images of the vertices and builds
/// polygon sides from them. The polygon or polyline is the first output.
fn transform_line(
    c: &mut Construction,
    name: &str,
    func: &str,
    args: &[&str],
    src: Id,
) -> Result<Id, String> {
    let kind = c.kind(src);
    let def = if func == "Translate" {
        if args.len() != 2 {
            return Err(format!(
                "`Translate` does not take {} argument(s)",
                args.len()
            ));
        }
        Def::Vector(VectorDef::Translate {
            kind,
            src,
            by: object(c, args[1])?,
        })
    } else {
        Def::Transform(parse_transform_for(c, func, args, POLY)?)
    };
    add_transform(c, name, src, def)
}

pub(super) fn add_transform(
    c: &mut Construction,
    name: &str,
    src: Id,
    def: Def,
) -> Result<Id, String> {
    let mut plan = Plan::new(c, name)?;
    let kind = c.kind(src);
    let source = match c.def(src) {
        Def::Polygon(PolygonDef::Polygon(list) | PolygonDef::PolyLine(list)) => {
            c.list(list).to_vec()
        }
        _ => image_vertices(c, src).to_vec(),
    };
    let checkpoint = c.checkpoint();
    let result = (|| {
        let poly = c.add(name, def).map_err(|e| e.to_string())?;
        let mut image = Vec::with_capacity(source.len());
        for source in source {
            let label = c.label(source);
            let label = if label.contains('_') || label.ends_with("'''") {
                next_indexed_label(c, &plan.taken, POINT_LABELS)
            } else {
                let prime = format!("{label}'");
                if is_free(c, &plan.taken, &prime) {
                    prime
                } else {
                    planned_index_label(c, &plan.taken, &prime)
                }
            };
            let vertex = Def::Polygon(PolygonDef::ImageVertex { poly, source });
            image.push(c.add(&label, vertex).map_err(|e| e.to_string())?);
        }
        if kind == Kind::Polygon {
            plan_sides(c, &mut plan, &image, &image)?;
            if !plan.items.is_empty() {
                plan.commit(c)?;
            }
        }
        Ok(poly)
    })();
    if result.is_err() {
        c.discard_since(checkpoint);
    }
    result
}

/// ApplyMatrix rebuilds fixed polygons from transformed vertices. For a
/// variable-size polygon GeoPolygon retains the area scale of the 2D block.
pub(super) fn apply_matrix(
    c: &Construction,
    src: Id,
    kind: Kind,
    matrix: Option<Matrix>,
) -> Option<Value> {
    let source = Shape::of(c, src);
    let vertices = (0..source.len()).map(|k| matrix?.apply(source.raw_vertex(c, k)?));
    if kind == Kind::Polygon {
        let area = area_of(vertices.map(|v| match v?.cartesian() {
            Value::Point { x, y } => Some((x, y)),
            _ => None,
        }))?;
        let area = if has_fixed_vertices(c, src) {
            area
        } else if let Value::Polygon { area } = c.value(src) {
            area * matrix?.area_scale()
        } else {
            return None;
        };
        return Some(Value::Polygon { area });
    }
    if source.len() == 0 || matrix.is_none() && !has_fixed_vertices(c, src) {
        return None;
    }
    let mut length = 0.0;
    let mut prev = None;
    for value in vertices {
        let point = value.and_then(Value::point_coords);
        if let (Some((x, y, w)), Some((u, v, z))) = (prev, point) {
            length += segment_length(x / w, y / w, u / z, v / z);
        }
        prev = point;
    }
    length.is_finite().then_some(Value::PolyLine { length })
}

/// The number of arguments as GeoGebra counts them: `F()` has none.
fn arg_count(args: &[&str]) -> usize {
    if args == [""] {
        0
    } else {
        args.len()
    }
}

/// A plain decimal literal such as `5`, `-2` or `3.5`. Words such as `inf`
/// or `nan` and exponents are not literals here.
pub(crate) fn decimal(arg: &str) -> Option<f64> {
    let digits = arg.strip_prefix('-').unwrap_or(arg);
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let all_digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    let valid = !(whole.is_empty() && fraction.is_empty())
        && all_digits(whole)
        && all_digits(fraction)
        && !(digits.ends_with('.'));
    if valid {
        arg.parse().ok()
    } else {
        None
    }
}

/// A number argument: the label of a number object (an existing label
/// wins) or a decimal literal.
pub(crate) fn count_arg(c: &Construction, arg: &str) -> Result<Count, String> {
    if let Some(id) = c.id(arg) {
        c.expect(id, NUMBER).map_err(|e| e.to_string())?;
        return Ok(Count::Object(id));
    }
    match decimal(arg) {
        Some(v) if v.is_finite() => Ok(Count::Literal(v)),
        Some(_) => Err(Error::NotFinite.to_string()),
        None => Err(format!("unknown object `{arg}`")),
    }
}

fn points(c: &Construction, args: &[&str]) -> Result<Vec<Id>, String> {
    let ids = args
        .iter()
        .map(|a| object(c, a))
        .collect::<Result<Vec<_>, _>>()?;
    for &id in &ids {
        c.expect(id, POINT).map_err(|e| e.to_string())?;
    }
    Ok(ids)
}

/// Adds a definition over the points named by `args`.
fn points_line(
    c: &mut Construction,
    name: &str,
    args: &[&str],
    make: fn(IdList) -> PolygonDef,
) -> Result<Id, String> {
    let ids = points(c, args)?;
    let mut plan = Plan::new(c, name)?;
    plan.first(name, Planned::List(ids, make));
    plan.commit(c)
}

/// Adds a definition over one object named by `arg`.
fn single(
    c: &mut Construction,
    name: &str,
    arg: &str,
    make: fn(Id) -> PolygonDef,
) -> Result<Id, String> {
    let id = object(c, arg)?;
    let mut plan = Plan::new(c, name)?;
    plan.first(name, Planned::Def(Def::Polygon(make(id))));
    plan.commit(c)
}

/// Whether `def` would be defined if it were added now.
fn defined(c: &Construction, def: Def) -> Result<bool, String> {
    c.evaluate(def)
        .map(|v| v.is_defined())
        .map_err(|e| e.to_string())
}

/// `AlgoDispatcher.segmentFixed` constructs the endpoint on an internal circle.
fn fixed_segment_line(c: &mut Construction, name: &str, a: &str, n: &str) -> Result<Id, String> {
    use super::path::PathDef;
    let plan = Plan::new(c, name)?;
    let start = object(c, a)?;
    let length = count_arg(c, n)?;
    let label = next_indexed_label(c, &plan.taken, POINT_LABELS);
    let checkpoint = c.checkpoint();
    let end = c
        .add(&label, Def::Path(PathDef::FixedEndpoint { start, length }))
        .map_err(|e| e.to_string())?;
    c.add(name, Def::Segment(start, end))
        .map(|_| end)
        .map_err(|e| {
            c.discard_since(checkpoint);
            e.to_string()
        })
}

fn clip_line(c: &mut Construction, name: &str, func: &str, args: &[&str]) -> Result<Id, String> {
    let mut plan = Plan::new(c, name)?;
    let def = ClipDef::parse(c, func, args)?;
    let prefix = name
        .split('_')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("a");
    if def.kind() == Kind::Segment {
        let count = def.segments(c).len();
        if count == 0 {
            plan.first(name, Planned::Def(Def::Polygon(PolygonDef::Clip(def))));
            return plan.commit(c);
        }
        for i in 0..count {
            let label = planned_index_label(c, &plan.taken, prefix);
            let index = if def.is_carrier() { count - 1 - i } else { i };
            plan.push(
                label,
                Planned::Def(Def::Polygon(PolygonDef::Clip(
                    def.slot(ClipOutput::Segment, index),
                ))),
            );
        }
    } else {
        let rings = def.rings(c);
        let vertices = rings.iter().map(Vec::len).sum();
        for i in 0..rings.len().max(1) {
            let label = planned_index_label(c, &plan.taken, prefix);
            plan.push(
                label,
                Planned::Def(Def::Polygon(PolygonDef::Clip(
                    def.slot(ClipOutput::Polygon, i),
                ))),
            );
        }
        plan.taken.remove(name);
        for (labels, output) in [
            (POINT_LABELS, ClipOutput::Point),
            (LINE_LABELS, ClipOutput::Edge),
        ] {
            for i in 0..vertices {
                let label = next_indexed_label(c, &plan.taken, labels);
                plan.push(
                    label,
                    Planned::Def(Def::Polygon(PolygonDef::Clip(def.slot(output, i)))),
                );
            }
        }
    }
    plan.commit(c)
}

/// Rigid copies keep an independent first vertex and the bearing of the
/// copied first side. A polygon made from points keeps its creation offsets.
fn rigid_polygon_line(c: &mut Construction, name: &str, args: &[&str]) -> Result<Id, String> {
    let mut plan = Plan::new(c, name)?;
    let checkpoint = c.checkpoint();
    let first = Id(c.len());
    let result = (|| {
        let source = args
            .first()
            .and_then(|label| c.id(label))
            .filter(|&id| c.kind(id) == Kind::Polygon);
        let mut side_names = Vec::new();
        let mut vertices = Vec::new();
        if let Some(poly) = source {
            let (dx, dy) = match args {
                // The headless default view has a right edge of 11.7.
                [_] => (11.7 / 15.0, -11.7 / 15.0),
                [_, x, y] => (count_arg(c, x)?.value(c), count_arg(c, y)?.value(c)),
                _ => return Err("`RigidPolygon` of a polygon takes zero or two offsets".into()),
            };
            let shape = Shape::of(c, poly);
            let n = shape.len();
            let p = shape
                .vertex(c, 0)
                .ok_or("the source polygon has no first vertex")?;
            let q = shape
                .vertex(c, 1)
                .ok_or("the source polygon has no second vertex")?;
            let bearing = (q.1 - p.1).atan2(q.0 - p.0);
            let label = next_indexed_label(c, &plan.taken, POINT_LABELS);
            let anchor = c
                .add(
                    &label,
                    Def::FreePoint {
                        x: p.0 + dx,
                        y: p.1 + dy,
                    },
                )
                .map_err(|e| e.to_string())?;
            vertices.push(anchor);
            for k in 1..n {
                let label = next_indexed_label(c, &plan.taken, POINT_LABELS);
                let d = PolygonDef::RigidCopyVertex {
                    poly,
                    anchor,
                    bearing,
                    k: k as u32,
                };
                vertices.push(c.add(&label, Def::Polygon(d)).map_err(|e| e.to_string())?);
            }
            side_names.clone_from(&vertices);
        } else {
            if args.len() < 3 {
                return Err("`RigidPolygon` needs a polygon or at least three points".into());
            }
            side_names = points(c, args)?;
            let anchor = side_names[0];
            let a = c.point(anchor).ok_or("the first point is undefined")?;
            if let Some(defs) = rigid_point_definitions(c, &side_names, a) {
                plan_polygon_sides(c, &mut plan, name, &side_names, &side_names)?;
                let polygon = plan.commit(c)?;
                c.rigidify_points(polygon, defs);
                return Ok(polygon);
            }
            vertices.push(anchor);
            for &id in &side_names[1..] {
                let p = c.point(id).ok_or("a vertex is undefined")?;
                let label = next_indexed_label(c, &plan.taken, POINT_LABELS);
                let d = PolygonDef::RigidVertex {
                    anchor,
                    x: p.0 - a.0,
                    y: p.1 - a.1,
                };
                vertices.push(c.add(&label, Def::Polygon(d)).map_err(|e| e.to_string())?);
            }
        }
        plan_polygon_sides(c, &mut plan, name, &vertices, &side_names)?;
        plan.commit(c).map(|_| first)
    })();
    if result.is_err() {
        c.discard_since(checkpoint);
    }
    result
}

/// Keep point identities and their existing dependants when the replacement
/// fits the construction's creation-order dependency graph.
fn rigid_point_definitions(
    c: &Construction,
    points: &[Id],
    (x, y): (f64, f64),
) -> Option<Vec<(Id, Def)>> {
    let &[anchor, pivot, ref rest @ ..] = points else {
        return None;
    };
    if anchor >= pivot
        || rest.iter().any(|&id| id <= pivot)
        || points[1..]
            .iter()
            .any(|&id| !matches!(c.def(id), Def::FreePoint { .. }))
    {
        return None;
    }
    let (px, py) = c.point(pivot)?;
    let (dx, dy) = (px - x, py - y);
    let radius = dx.hypot(dy);
    if !radius.is_finite() || radius == 0.0 {
        return None;
    }
    let (cos, sin) = (dx / radius, dy / radius);
    let mut defs = Vec::with_capacity(points.len() - 1);
    defs.push((
        pivot,
        Def::Path(super::path::PathDef::RigidEndpoint {
            anchor,
            radius,
            bearing: dy.atan2(dx),
        }),
    ));
    for &id in rest {
        let (u, v) = c.point(id)?;
        let (u, v) = (u - x, v - y);
        let (along, across) = (u * cos + v * sin, v * cos - u * sin);
        if !(along.is_finite() && across.is_finite()) {
            return None;
        }
        defs.push((
            id,
            Def::Polygon(PolygonDef::RigidFrameVertex {
                anchor,
                pivot,
                along,
                across,
            }),
        ));
    }
    defs.sort_unstable_by_key(|&(id, _)| id);
    defs.dedup_by_key(|item| item.0);
    Some(defs)
}

/// `CmdPolygon`.
fn polygon_line(c: &mut Construction, name: &str, args: &[&str]) -> Result<Id, String> {
    let n = arg_count(args);
    if n == 0 {
        return Err("`Polygon` does not take 0 argument(s)".into());
    }
    if n == 1 && c.id(args[0]).is_some_and(|id| c.kind(id) == Kind::List) {
        return single(c, name, args[0], PolygonDef::PolygonFromList);
    }
    let mut plan = Plan::new(c, name)?;
    if n == 3 {
        // An existing label wins over a literal.
        let kind = |arg: &str| c.id(arg).map(|id| c.kind(id));
        let is_point = |arg: &str| kind(arg) == Some(Kind::Point);
        let is_number = |arg: &str| match kind(arg) {
            Some(k) => k == Kind::Number,
            None => decimal(arg).is_some(),
        };
        if is_point(args[0]) && is_point(args[1]) && is_number(args[2]) {
            plan_regular(c, &mut plan, name, args)?;
            return plan.commit(c);
        }
    }
    let ids = points(c, args)?;
    plan_polygon_sides(c, &mut plan, name, &ids, &ids)?;
    plan.commit(c)
}

fn plan_polygon_sides(
    c: &Construction,
    plan: &mut Plan,
    name: &str,
    ids: &[Id],
    side_names: &[Id],
) -> Result<(), String> {
    plan.first(name, Planned::List(ids.to_vec(), PolygonDef::Polygon));
    plan_sides(c, plan, ids, side_names)
}

fn plan_sides(
    c: &Construction,
    plan: &mut Plan,
    ids: &[Id],
    side_names: &[Id],
) -> Result<(), String> {
    let n = ids.len();
    // `Polygon(A)` makes a polygon from a list, which has no sides.
    if n >= 2 {
        for i in 0..n {
            let side = Def::Segment(ids[i], ids[(i + 1) % n]);
            if !defined(c, side)? {
                continue;
            }
            // `GeoPolygon.defaultSegmentLabels`
            let named_after = side_names[if n == 3 { (i + 2) % 3 } else { i }];
            let label = side_label(c, &plan.taken, c.label(named_after));
            plan.push(label, Planned::Def(side));
        }
    }
    Ok(())
}

/// `Polygon(A, B, n)` (`AlgoPolygonRegularND`): the polygon, its new
/// vertices and its sides, as many as `n` gives now. GeoGebra labels them
/// in `updateOutput`, before it checks that the polygon is defined: nothing
/// when `round(n) <= 2`; else every new vertex, and the sides only when
/// they are defined then, which is when the side from `A` to `B` is (each
/// new side starts as a copy of it).
fn plan_regular(
    c: &Construction,
    plan: &mut Plan,
    name: &str,
    args: &[&str],
) -> Result<(), String> {
    let (a, b) = (object(c, args[0])?, object(c, args[1])?);
    let n = count_arg(c, args[2])?;
    let count = vertex_count(n.value(c));
    if count > MAX_REGULAR_VERTICES {
        return Err(format!(
            "a regular polygon may have at most {MAX_REGULAR_VERTICES} vertices"
        ));
    }
    plan.first(
        name,
        Planned::Def(Def::Polygon(PolygonDef::Regular { a, b, n })),
    );
    if count < 3 {
        return Ok(());
    }
    for k in 2..count as u32 {
        let label = next_indexed_label(c, &plan.taken, POINT_LABELS);
        let def = PolygonDef::RegularVertex { a, b, n, k };
        plan.push(label, Planned::Def(Def::Polygon(def)));
    }
    if !defined(c, Def::Segment(a, b))? {
        return Ok(());
    }
    for k in 0..count as u32 {
        let label = next_indexed_label(c, &plan.taken, LINE_LABELS);
        let def = PolygonDef::RegularSide { a, b, n, k };
        plan.push(label, Planned::Def(Def::Polygon(def)));
    }
    Ok(())
}

/// `CmdVertex` for polygons and polylines.
fn vertex_line(c: &mut Construction, name: &str, args: &[&str]) -> Result<Id, String> {
    match arg_count(args) {
        1 => outputs_line(c, name, args[0], Outputs::Vertices),
        2 => {
            let poly = object(c, args[0])?;
            c.expect(poly, POLY).map_err(|e| e.to_string())?;
            let index = count_arg(c, args[1])?;
            let mut plan = Plan::new(c, name)?;
            let def = PolygonDef::Vertex { poly, index };
            plan.first(name, Planned::Def(Def::Polygon(def)));
            plan.commit(c)
        }
        n => Err(format!("`Vertex` does not take {n} argument(s)")),
    }
}

/// Commands that create one object per vertex.
#[derive(Clone, Copy)]
enum Outputs {
    Vertices,
    /// `true` for `InteriorAngles`, `false` for `Angle`.
    Angles(bool),
}

/// `Vertex(p)`, `InteriorAngles(p)` and `Angle(p)`: one output per vertex
/// at creation, labelled through `OutputHandler.setLabelsMulti` with a
/// single label, so always with an index. As for other commands with
/// several results, `name` itself must be free.
fn outputs_line(c: &mut Construction, name: &str, arg: &str, what: Outputs) -> Result<Id, String> {
    let poly = object(c, arg)?;
    let expected = match what {
        Outputs::Vertices => POLY,
        Outputs::Angles(_) => POLYGON,
    };
    c.expect(poly, expected).map_err(|e| e.to_string())?;
    // `name` is not used itself, only its indexed forms are; but as for
    // other commands with several results it must be free.
    if c.id(name).is_some() {
        return Err(Error::DuplicateLabel(name.to_string()).to_string());
    }
    let mut plan = Plan::default();
    let len = Shape::of(c, poly).len();
    let count = match what {
        // `AlgoVertexPolygon.compute`: grows from one output only when the
        // polygon is defined.
        Outputs::Vertices if poly_defined(c, poly) => len.max(1),
        Outputs::Vertices => 1,
        Outputs::Angles(_) => len.max(1),
    };
    // Construction.getIndexLabel: the prefix is the name up to the first `_`.
    let prefix = match name.split('_').next() {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => "a".to_string(),
    };
    for k in 0..count as u32 {
        let def = match what {
            Outputs::Vertices => PolygonDef::VertexAt { poly, k },
            Outputs::Angles(internal) => PolygonDef::Angle { poly, k, internal },
        };
        let label = planned_index_label(c, &plan.taken, &prefix);
        plan.push(label, Planned::Def(Def::Polygon(def)));
    }
    plan.commit(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::{exec as run, load};

    fn num(c: &Construction, label: &str) -> Option<f64> {
        match c.value(c.id(label).unwrap()) {
            Value::Number(v) => Some(v),
            Value::Undefined => None,
            other => panic!("{label} is {other:?}"),
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn labels(c: &Construction, from: usize) -> Vec<String> {
        c.ids()
            .skip(from)
            .map(|id| c.label(id).to_string())
            .collect()
    }

    #[test]
    fn rolling_back_a_rigid_polygon_releases_its_existing_points() {
        let mut c = load("A = (0, 0)\nB = (2, 0)\nC = (0, 1)\nP = (B + C) / 2\n").unwrap();
        let checkpoint = c.checkpoint();
        crate::script::exec(&mut c, "p = RigidPolygon(A, B, C)").unwrap();
        let a = c.id("A").unwrap();
        let b = c.id("B").unwrap();
        let vertex = c.id("C").unwrap();
        let p = c.id("P").unwrap();
        assert!(c.is_movable(b));
        assert!(!c.is_movable(vertex));
        c.rollback(checkpoint).unwrap();
        assert!(c.is_movable(vertex));
        c.move_point(a, 3.0, 4.0).unwrap();
        assert_eq!(c.point(b), Some((2.0, 0.0)));
        c.move_point(vertex, 4.0, 2.0).unwrap();
        assert_eq!(c.point(p), Some((3.0, 1.0)));
    }

    #[test]
    fn area_of_many_points() {
        let mut c = load(
            "A = (0, 0)\nB = (4, 0)\nC = (4, 3)\nD = (2, 5)\nE = (0, 3)\n\
             a = Area(A, B, C, D, E)\nb = Area(A, B, C, D, E, A)\n",
        )
        .unwrap();
        let a = c.id("a").unwrap();
        assert_eq!(c.value(a), Value::Number(16.0));
        assert_eq!(num(&c, "b"), Some(16.0));
        // clockwise order: the area is still positive
        run(&mut c, "r = Area(E, D, C, B, A)").unwrap();
        assert_eq!(num(&c, "r"), Some(16.0));
        // every point is a parent
        let d = c.id("D").unwrap();
        assert_eq!(c.move_point(d, 2.0, 7.0).unwrap(), 3);
        assert_eq!(c.value(a), Value::Number(20.0));
    }

    #[test]
    fn a_rejected_line_creates_nothing() {
        let mut c = load("A = (0, 0)\nB = (4, 0)\nk = Circle(A, B)\nr = 3\n").unwrap();
        let before = c.len();
        for (line, text) in [
            (
                "a = Area(A, B, A, B, k)",
                "`k` is a circle, expected a point",
            ),
            ("a = Area(A, B, A, B, Z)", "unknown object `Z`"),
            ("p = Polygon(A, B, k)", "`k` is a circle, expected a point"),
            ("p = Polygon()", "does not take 0"),
            ("A = Polygon(A, B, r)", "already exists"),
            ("p = PolyLine(A)", "does not take 1"),
            ("p = Centroid(k)", "`k` is a circle, expected a polygon"),
            (
                "p = Vertex(k)",
                "`k` is a circle, expected a polygon or polyline",
            ),
            ("p = Vertex(A, B, r)", "does not take 3"),
            (
                "p = InteriorAngles(A)",
                "`A` is a point, expected a polygon",
            ),
            ("p = Polygon(A, B, 5000)", "at most 1000"),
            ("p = Polygon(A, B, inf)", "unknown object `inf`"),
            ("p = Polygon(A, B, 1e3)", "unknown object `1e3`"),
        ] {
            let e = run(&mut c, line).unwrap_err();
            assert!(e.contains(text), "{line}: {e}");
            assert_eq!(c.len(), before, "{line}");
        }
    }

    #[test]
    fn a_huge_literal_is_not_finite() {
        let mut c = load("A = (0, 0)\nB = (4, 0)\n").unwrap();
        let line = format!("p = Polygon(A, B, {})", "9".repeat(400));
        let e = run(&mut c, &line).unwrap_err();
        assert!(e.contains("finite"), "{e}");
        assert_eq!(c.len(), 2);
    }

    /// The labels, edges and lists of a construction.
    fn structure(c: &Construction) -> (Vec<String>, Vec<Vec<Id>>, usize) {
        (
            labels(c, 0),
            c.ids().map(|id| c.children(id).to_vec()).collect(),
            c.stored_ids(),
        )
    }

    #[test]
    fn a_failure_in_the_middle_of_a_line_removes_what_it_added() {
        let mut c = load(
            "A = (0, 0)\nB = (4, 0)\nC = (4, 3)\nD = (0, 3)\nE = (-1, 1)\n\
             a = Area(A, B, C, D, E)\n",
        )
        .unwrap();
        let before = structure(&c);
        let ids: Vec<Id> = ["A", "B", "C", "D"].map(|l| c.id(l).unwrap()).to_vec();
        // the second object reuses the first label: refused only when added
        let mut plan = Plan::new(&c, "q").unwrap();
        plan.first("q", Planned::List(ids.clone(), PolygonDef::Polygon));
        plan.push("s".into(), Planned::Def(Def::Segment(ids[0], ids[1])));
        plan.push("q".into(), Planned::Def(Def::Segment(ids[1], ids[2])));
        let e = plan.commit(&mut c).unwrap_err();
        assert!(e.contains("already exists"), "{e}");
        assert_eq!(structure(&c), before);
        assert_eq!(c.id("q"), None);
        assert_eq!(c.id("s"), None);
        // and the next line works as if nothing had happened
        run(&mut c, "q = Polygon(A, B, C, D)").unwrap();
        assert_eq!(labels(&c, 6), ["q", "a_1", "b", "c", "d"]);
    }

    #[test]
    fn only_sides_defined_at_creation_are_created() {
        let mut c = load(
            "A = (0, 0)\nC = (4, 3)\nO = (10, 0)\nr = 1\nk = Circle(O, r)\n\
             l = Line(A, C)\nX = Intersect(l, k)\n",
        )
        .unwrap();
        let n = c.len();
        // `x` is reserved: the side from C to A, named after X, is `x_1`
        run(&mut c, "p = Polygon(A, X, C)").unwrap();
        assert_eq!(labels(&c, n), ["p", "x_1"]);
        assert_eq!(c.value(c.id("p").unwrap()), Value::Undefined);
        run(&mut c, "q = Polygon(A, X)").unwrap();
        assert_eq!(labels(&c, n + 2), ["q"]);
        run(&mut c, "s = Polygon(A)").unwrap();
        assert_eq!(labels(&c, n + 3), ["s"]);
    }

    #[test]
    fn regular_polygons_label_what_geogebra_labels_at_creation() {
        let src = "A = (0, 0)\nB = (4, 0)\nO = (10, 0)\nr = 1\nk = Circle(O, r)\n\
                   P = (0, 5)\nQ = (1, 5)\nl = Line(P, Q)\nX = Intersect(l, k)\nm = 2\n";
        for (rhs, created, vertices) in [
            // at most two vertices: nothing besides the polygon
            ("Polygon(A, B, 0)", &["p"][..], 0),
            ("Polygon(A, B, 1)", &["p"], 0),
            ("Polygon(A, B, 2)", &["p"], 0),
            ("Polygon(A, B, -3)", &["p"], 0),
            ("Polygon(A, B, 2.4)", &["p"], 0),
            ("Polygon(A, B, m)", &["p"], 5),
            // an undefined point: the vertices, but no sides
            ("Polygon(X, B, 4)", &["p", "C", "D"], 0),
            ("Polygon(A, X, 3)", &["p", "C"], 0),
        ] {
            let mut c = load(src).unwrap();
            let n = c.len();
            run(&mut c, &format!("p = {rhs}")).unwrap();
            assert_eq!(labels(&c, n), created, "{rhs}");
            let p = c.id("p").unwrap();
            assert_eq!(c.value(p), Value::Undefined, "{rhs}");
            for id in c.ids().skip(n + 1) {
                assert_eq!(c.value(id), Value::Undefined, "{rhs}");
            }
            // the polygon follows later changes, without new objects
            c.set_number(c.id("m").unwrap(), 5.0).unwrap();
            assert_eq!(c.vertices(p).count(), vertices, "{rhs}");
            assert_eq!(c.len(), n + created.len());
        }
    }

    #[test]
    fn vertex_counts_wrap_like_java_ints() {
        let mut c = load("A = (0, 0)\nB = (4, 0)\n").unwrap();
        run(&mut c, "p = Polygon(A, B, 4294967299)").unwrap();
        assert_eq!(labels(&c, 2), ["p", "C", "f", "g", "h"]);
        run(&mut c, "q = Polygon(A, B, -4294967293)").unwrap();
        assert_eq!(c.vertices(c.id("q").unwrap()).count(), 3);
        run(&mut c, "s = Polygon(A, B, 4294967296)").unwrap();
        assert_eq!(c.value(c.id("s").unwrap()), Value::Undefined);
        assert_eq!(vertex_count(2.5), 3);
        assert_eq!(vertex_count(-2.5), 2);
        assert_eq!(vertex_count(f64::NAN), 2);
        assert_eq!(vertex_count(1e20), 2);
    }

    #[test]
    fn vertex_indices_wrap_like_java_ints() {
        for (index, defined) in [
            ("5", true),
            ("1", true),
            ("7", false),
            ("0.99", false),
            ("-0.5", false),
            ("4294967298", false),
            ("4294967297", false),
            ("100000000000000000000", false),
            ("-100000000000000000000", false),
        ] {
            let mut c = load("A = (0, 0)\nB = (4, 0)\np = Polygon(A, B, 5)\n").unwrap();
            let id = run(&mut c, &format!("V = Vertex(p, {index})"))
                .unwrap()
                .unwrap();
            assert_eq!(c.value(id).is_defined(), defined, "{index}");
        }
    }

    #[test]
    fn decimal_literals() {
        for (arg, v) in [
            ("5", Some(5.0)),
            ("-2", Some(-2.0)),
            ("3.5", Some(3.5)),
            (".5", Some(0.5)),
        ] {
            assert_eq!(decimal(arg), v, "{arg}");
        }
        for arg in [
            "", "-", ".", "5.", "1e3", "inf", "-inf", "nan", "NaN", "+1", "1.2.3", "0x10",
        ] {
            assert_eq!(decimal(arg), None, "{arg}");
        }
    }

    #[test]
    fn existing_labels_win_over_number_words() {
        let mut c =
            load("A = (0, 0)\nB = (4, 0)\nC = (4, 6)\ninf = Midpoint(A, C)\nnan = 4\n").unwrap();
        let n = c.len();
        run(&mut c, "p = Polygon(A, B, inf)").unwrap();
        assert_eq!(c.kind(c.id("p").unwrap()), Kind::Polygon);
        assert_eq!(c.vertices(c.id("p").unwrap()).count(), 3);
        // `inf` is taken, so the side named after it is the next default
        assert_eq!(labels(&c, n), ["p", "f", "a", "b"]);
        run(&mut c, "q = Polygon(A, B, nan)").unwrap();
        assert_eq!(c.vertices(c.id("q").unwrap()).count(), 4);
    }

    #[test]
    fn outputs_need_a_free_name() {
        let mut c = load("A = (0, 0)\nB = (4, 0)\nC = (4, 3)\np = Polygon(A, B, C)\n").unwrap();
        let n = c.len();
        for line in ["A = Vertex(p)", "B = InteriorAngles(p)", "p = Angle(p)"] {
            let e = run(&mut c, line).unwrap_err();
            assert!(e.contains("already exists"), "{line}: {e}");
            assert_eq!(c.len(), n, "{line}");
        }
    }

    #[test]
    fn polygon_sides_are_named_like_geogebra() {
        let mut c = load("A = (0, 0)\nB = (4, 0)\nC = (1, 3)\nD = (0, 3)\n").unwrap();
        let n = c.len();
        run(&mut c, "p = Polygon(A, B, C)").unwrap();
        assert_eq!(labels(&c, n), ["p", "c", "a", "b"]);
        assert_eq!(
            c.value(c.id("c").unwrap()),
            Value::Segment {
                x1: 0.0,
                y1: 0.0,
                x2: 4.0,
                y2: 0.0
            }
        );
        let n = c.len();
        run(&mut c, "q = Polygon(A, B, C, D)").unwrap();
        assert_eq!(labels(&c, n), ["q", "a_1", "b_1", "c_1", "d"]);
        let n = c.len();
        run(&mut c, "r = Polygon(A, B)").unwrap();
        assert_eq!(labels(&c, n), ["r", "a_2", "b_2"]);
        let n = c.len();
        run(&mut c, "s = Polygon(A)").unwrap();
        assert_eq!(labels(&c, n), ["s"]);

        let mut c = load("PA = (0, 0)\nPB = (4, 0)\nX = (1, 3)\npb = 2\n").unwrap();
        let n = c.len();
        run(&mut c, "p = Polygon(PA, PB, X, PA)").unwrap();
        // `pb` is taken, `x` is reserved
        assert_eq!(labels(&c, n), ["p", "pa", "f", "x_1", "g"]);
    }

    #[test]
    fn regular_polygons_create_points_and_sides() {
        let mut c = load("A = (0, 0)\nB = (4, 0)\nn = 4\n").unwrap();
        let start = c.len();
        run(&mut c, "p = Polygon(A, B, n)").unwrap();
        assert_eq!(labels(&c, start), ["p", "C", "D", "f", "g", "h", "i"]);
        let p = c.id("p").unwrap();
        assert!(close(
            match c.value(p) {
                Value::Polygon { area } => area,
                v => panic!("{v:?}"),
            },
            16.0
        ));
        let verts: Vec<_> = c.vertices(p).map(Option::unwrap).collect();
        assert_eq!(verts.len(), 4);
        assert!(close(verts[2].0, 4.0) && close(verts[2].1, 4.0));
        assert!(close(verts[3].0, 0.0) && close(verts[3].1, 4.0));
        // three vertices: D and the side from D are undefined, h closes
        let n = c.id("n").unwrap();
        c.set_number(n, 3.0).unwrap();
        assert_eq!(c.value(c.id("D").unwrap()), Value::Undefined);
        assert_eq!(c.value(c.id("i").unwrap()), Value::Undefined);
        match c.value(c.id("h").unwrap()) {
            Value::Segment { x2, y2, .. } => assert!(x2 == 0.0 && y2 == 0.0),
            v => panic!("{v:?}"),
        }
        assert_eq!(c.vertices(p).count(), 3);
        // two: undefined polygon, sides A-B and B-A
        c.set_number(n, 2.0).unwrap();
        assert_eq!(c.value(p), Value::Undefined);
        assert_eq!(c.vertices(p).count(), 0);
        assert_eq!(
            c.value(c.id("g").unwrap()),
            Value::Segment {
                x1: 4.0,
                y1: 0.0,
                x2: 0.0,
                y2: 0.0
            }
        );
        // six: the polygon has six vertices, only C and D exist as points
        c.set_number(n, 6.0).unwrap();
        assert_eq!(c.vertices(p).count(), 6);
        run(&mut c, "P = Polygon(A, B, 5)").unwrap();
        assert!(c.id("E").is_some() && c.id("G").is_some() && c.id("j").is_some());
    }

    #[test]
    fn measures_of_a_polygon() {
        let mut c = load(
            "A = (0, 0)\nB = (4, 0)\nC = (4, 3)\np = Polygon(A, B, C)\n\
             ar = Area(p)\npe = Perimeter(p)\nG = Centroid(p)\nW = Vertex(p, 2)\n\
             V = Vertex(p)\nw = InteriorAngles(p)\nu = Angle(p)\n",
        )
        .unwrap();
        assert_eq!(num(&c, "ar"), Some(6.0));
        assert_eq!(num(&c, "pe"), Some(12.0));
        match c.value(c.id("G").unwrap()) {
            Value::Point { x, y } => assert!(close(x, 8.0 / 3.0) && close(y, 1.0)),
            v => panic!("{v:?}"),
        }
        assert_eq!(c.value(c.id("W").unwrap()), Value::Point { x: 4.0, y: 0.0 });
        assert_eq!(
            c.value(c.id("V_{3}").unwrap()),
            Value::Point { x: 4.0, y: 3.0 }
        );
        assert!(close(num(&c, "w_{2}").unwrap(), PI / 2.0));
        let sum: f64 = (1..=3)
            .map(|i| num(&c, &format!("w_{{{i}}}")).unwrap())
            .sum();
        assert!(close(sum, PI));
        // clockwise: interior angles stay interior, `Angle` turns
        let cc = c.id("C").unwrap();
        c.move_point(cc, 4.0, -3.0).unwrap();
        assert!(close(num(&c, "w_{2}").unwrap(), PI / 2.0));
        assert!(close(num(&c, "u_{2}").unwrap(), 1.5 * PI));
        // a move that keeps the area: the perimeter and the centroid follow
        c.move_point(cc, 4.0, 3.0).unwrap();
        c.move_point(cc, 0.0, 3.0).unwrap();
        assert_eq!(num(&c, "ar"), Some(6.0));
        assert_eq!(num(&c, "pe"), Some(12.0));
        match c.value(c.id("G").unwrap()) {
            Value::Point { x, y } => assert!(close(x, 4.0 / 3.0) && close(y, 1.0)),
            v => panic!("{v:?}"),
        }
        assert_eq!(
            c.value(c.id("V_{3}").unwrap()),
            Value::Point { x: 0.0, y: 3.0 }
        );
        // collinear: area 0, no centroid
        c.move_point(cc, 8.0, 0.0).unwrap();
        assert_eq!(num(&c, "ar"), Some(0.0));
        assert_eq!(c.value(c.id("G").unwrap()), Value::Undefined);
    }

    #[test]
    fn a_transformed_polygon_comes_with_its_sides() {
        let src = "A = (0, 0)\nB = (4, 0)\nC = (4, 3)\nQ = (1, 1)\nw = 0.5\n\
                   l = Line(A, C)\nv = Vector(A, B)\n";
        let mut c = load(src).unwrap();
        run(&mut c, "p = Polygon(A, B, C)").unwrap();
        let n = c.len();
        // a triangle's sides are named after the opposite vertex's image
        let q = run(&mut c, "q = Mirror(p, Q)").unwrap().unwrap();
        assert_eq!(q.index(), n);
        assert_eq!(labels(&c, n), ["q", "A'", "B'", "C'", "c'", "a'", "b'"]);
        // reflection in a point turns nothing: the image runs as the source
        assert!(close(
            match c.value(c.id("q").unwrap()) {
                Value::Polygon { area } => area,
                v => panic!("{v:?}"),
            },
            6.0
        ));
        // `a'` is the side from B' = (-2, 2) to C' = (-2, -1)
        assert_eq!(
            c.value(c.id("a'").unwrap()),
            Value::Segment {
                x1: -2.0,
                y1: 2.0,
                x2: -2.0,
                y2: -1.0
            }
        );
        // the image follows a moved vertex and the moved mirror
        c.move_point(c.id("Q").unwrap(), 0.0, 0.0).unwrap();
        assert_eq!(
            c.value(c.id("c'").unwrap()),
            Value::Segment {
                x1: 0.0,
                y1: 0.0,
                x2: -4.0,
                y2: 0.0
            }
        );
        // the measures and vertices of the image read the moved points
        run(&mut c, "ar = Area(q)").unwrap();
        assert!(close(num(&c, "ar").unwrap(), 6.0));
        run(&mut c, "V = Vertex(q, 2)").unwrap();
        assert_eq!(
            c.value(c.id("V").unwrap()),
            Value::Point { x: -4.0, y: 0.0 }
        );
    }

    #[test]
    fn transformations_of_other_polygons_and_polylines() {
        let src = "A = (0, 0)\nB = (4, 0)\nD = (1, 3)\nQ = (1, 1)\nw = 0.5\n\
                   v = Vector(A, D)\nt = PolyLine(A, B, D)\nu = PolyLine(A, B)\n";
        // a polyline from three points is transformed through its points
        // (no sides); one from two goes the plain way
        let mut c = load(src).unwrap();
        let n = c.len();
        run(&mut c, "m = Rotate(t, w)").unwrap();
        run(&mut c, "o = Rotate(u, w)").unwrap();
        assert_eq!(labels(&c, n), ["m", "A'", "B'", "D'", "o"]);
        // a rotation moves the points and keeps every length
        assert!(close(
            match c.value(c.id("m").unwrap()) {
                Value::PolyLine { length } => length,
                v => panic!("{v:?}"),
            },
            4.0 + 18.0_f64.sqrt()
        ));
        // a polygon of two points and a regular one give no sides either
        let mut c = load(src).unwrap();
        run(&mut c, "p = Polygon(A, B)").unwrap();
        run(&mut c, "r = Polygon(A, B, 4)").unwrap();
        let n = c.len();
        run(&mut c, "q = Mirror(p, Q)").unwrap();
        run(&mut c, "s = Mirror(r, Q)").unwrap();
        assert_eq!(labels(&c, n), ["q", "s"]);
        // a translated polyline keeps its shape
        let mut c = load(src).unwrap();
        run(&mut c, "d = Translate(t, v)").unwrap();
        run(&mut c, "e = Polygon(A, B, D)").unwrap();
        let n = c.len();
        // a translation creates the sides as well, named after the moved
        // vertices like the polygon command
        run(&mut c, "f = Translate(e, v)").unwrap();
        assert_eq!(
            labels(&c, n),
            ["f", "A'_{1}", "B'_{1}", "D'_{1}", "d'_{1}", "a'_{1}", "b'_{1}"]
        );
        let verts: Vec<_> = c.vertices(c.id("f").unwrap()).map(Option::unwrap).collect();
        assert_eq!(verts.len(), 3);
        assert_eq!(verts[0], (1.0, 3.0));
        // the image of a polygon under inversion is refused, like a curve
        // GeoGebra draws instead of it
        let e = run(&mut c, "g = Mirror(f, k)");
        assert!(e.is_err());
    }

    #[test]
    fn polylines_skip_undefined_points() {
        let mut c = load(
            "A = (0, 0)\nB = (3, 4)\nO = (10, 0)\nr = 1\nk = Circle(O, r)\nl = Line(A, B)\n\
             X = Intersect(l, k)\nC = (6, 0)\npl = PolyLine(A, B, X, C)\nV = Vertex(pl)\n",
        )
        .unwrap();
        let pl = c.id("pl").unwrap();
        assert_eq!(c.value(pl), Value::PolyLine { length: 5.0 });
        assert_eq!(c.vertices(pl).count(), 4);
        assert_eq!(c.vertices(pl).nth(2), Some(None));
        assert_eq!(c.value(c.id("V_{3}").unwrap()), Value::Undefined);
        assert_eq!(
            c.value(c.id("V_{4}").unwrap()),
            Value::Point { x: 6.0, y: 0.0 }
        );
        let e = run(&mut c, "a = Area(pl)").unwrap_err();
        assert!(e.contains("expected a circle"), "{e}");
    }

    #[test]
    fn matrix_can_restore_a_projective_vertex_of_a_list_backed_polyline() {
        let c = load(
            "A = (0, 1)\nB = (1, 1)\nC = (2, 2)\n\
             I = ApplyMatrix({{1,0,0},{0,1,0},{1,0,0}}, A)\n\
             l = {I, B, C}\np = PolyLine(l)\n\
             q = ApplyMatrix({{1,0,0},{0,0,1},{0,1,0}}, p)\nR = Vertex(q, 1)",
        )
        .unwrap();
        assert_eq!(c.value(c.id("I").unwrap()), Value::Undefined);
        assert_eq!(c.value(c.id("R").unwrap()), Value::Point { x: 0.0, y: 0.0 });
    }

    #[test]
    fn a_fourth_image_names_its_sides_after_default_point_labels() {
        // past three transformations GeoGebra stops deriving the side labels
        // from the vertex labels: the vertex images get default point labels
        // and the sides are named after those
        let mut c = load(
            "A = (0, 5)\nB = (4, 0)\nE = (4, 3)\nQ = (1, 1)\nw = 0.5\nX = (-1, 2)\n\
             Y = (2, -3)\nn = Line(X, Y)\np = Polygon(A, B, E)\nq = Mirror(p, Q)\n\
             r = Rotate(q, w)\ns = Dilate(r, w)\n",
        )
        .unwrap();
        let n = c.len();
        run(&mut c, "t = Shear(s, n, w)").unwrap();
        // the vertex images are labelled C, D, F (A, B and E are taken)
        assert_eq!(labels(&c, n), ["t", "C", "D", "F", "f", "c", "d"]);
    }

    #[test]
    fn a_polyline_translated_by_an_undefined_vector_keeps_length_zero() {
        let mut c = load(
            "A = (0, 0)\nB = (3, 4)\nC = (6, 0)\nD = (2, 2)\nt = PolyLine(A, B, C)\n\
             q = Vector(D, C)\nu = UnitVector(q)\nm = Translate(t, u)\n",
        )
        .unwrap();
        assert_eq!(
            c.value(c.id("m").unwrap()),
            Value::PolyLine { length: 10.0 }
        );
        // a zero vector has no unit vector: the image stays a polyline whose
        // vertices are undefined, with no length
        c.move_point(c.id("C").unwrap(), 2.0, 2.0).unwrap();
        assert_eq!(c.value(c.id("m").unwrap()), Value::PolyLine { length: 0.0 });
        c.move_point(c.id("C").unwrap(), 6.0, 0.0).unwrap();
        assert_eq!(
            c.value(c.id("m").unwrap()),
            Value::PolyLine { length: 10.0 }
        );
    }
}
