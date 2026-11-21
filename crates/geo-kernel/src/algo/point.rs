// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdClosestPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoClosestPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/barycentric/AlgoKimberling.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoClosestPointLines.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectLines.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/barycentric/CmdTrilinear.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/barycentric/AlgoTrilinear.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoRay.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/matrix/Coords.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Closest points on paths, trilinear coordinates and triangle centers.
//!
//! `ClosestPoint` calls the `pointChanged` code of the path on a copy of the
//! point, as `AlgoClosestPoint` does.

use super::polygon::Count;
use super::{Group, Parents, CARRIERS, POINT};
use crate::kernel::{Construction, Error, Id, Kind, Value};
use crate::script::object;
use crate::tol::STANDARD_PRECISION;

const NUMBER: &[Kind] = &[Kind::Number];
/// The paths `ClosestPoint` accepts.
pub(crate) const CLOSEST_PATH: &[Kind] = &[
    Kind::Line,
    Kind::Segment,
    Kind::Ray,
    Kind::Circle,
    Kind::Conic,
    Kind::Arc,
    Kind::Sector,
    Kind::Polygon,
    Kind::PolyLine,
    Kind::Function,
    Kind::List,
];

/// Points computed from other objects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointDef {
    /// `ClosestPoint(path, point)` in either order of the arguments.
    Closest { path: Id, point: Id },
    /// `ClosestPoint(g, h)` of two lines: `AlgoClosestPointLines`, which is
    /// `AlgoIntersectLines`.
    LineIntersection { first: Id, second: Id },
    /// `Trilinear(A, B, C, a, b, c)`: the three points, then the three
    /// numbers. The handles are stored as `u32` to keep [`crate::Def`] small.
    Trilinear { ids: [u32; 6] },
    /// `TriangleCenter(A, B, C, n)`.
    TriangleCenter { a: Id, b: Id, c: Id, index: Count },
}

fn id_of(raw: u32) -> Id {
    Id(raw as usize)
}

/// `DoubleUtil.isZero`.
fn is_zero(v: f64) -> bool {
    -STANDARD_PRECISION < v && v < STANDARD_PRECISION
}

/// `DoubleUtil.isEpsilon(e, x, y)`.
fn is_epsilon(e: f64, x: f64, y: f64) -> bool {
    let ea = e.abs();
    !(ea > STANDARD_PRECISION
        || ea > x.abs() * STANDARD_PRECISION
        || ea > y.abs() * STANDARD_PRECISION)
}

/// A point with homogeneous coordinates as `GeoPoint.updateCoords` makes it
/// finite: a point at infinity or with a NaN coordinate has no value here.
fn hom_point(x: f64, y: f64, z: f64) -> Option<Value> {
    if x.is_nan() || y.is_nan() || z.is_nan() || is_epsilon(z, x, y) {
        return None;
    }
    let (x, y, z) = if z < 0.0 { (-x, -y, -z) } else { (x, y, z) };
    Some(if z == 1.0 {
        Value::Point { x, y }
    } else {
        Value::Point { x: x / z, y: y / z }
    })
}

/// `MyMath.length`: no overflow, no underflow.
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

/// `GeoLine.projectCoordsAndComputePathParam` for a finite point: the foot of
/// the perpendicular and the path parameter, counted from `start`.
fn project(line: (f64, f64, f64), start: (f64, f64), p: (f64, f64)) -> ((f64, f64), f64) {
    let (x, y, z) = line;
    let t = -(z + x * p.0 + y * p.1) / (x * x + y * y);
    let px = p.0 + t * x;
    let py = p.1 + t * y;
    let param = if x.abs() <= y.abs() {
        (px - start.0) / y
    } else {
        (start.1 - py) / x
    };
    ((px, py), param)
}

/// `GeoConicND.pointChangedUnlimited` for a circle: the point of the circle
/// in the direction of `p` from the centre.
fn closest_on_circle(cx: f64, cy: f64, r: f64, p: (f64, f64)) -> Value {
    // GeoGebra makes a circle of radius below the tolerance a single point.
    if r < STANDARD_PRECISION {
        return Value::Point { x: cx, y: cy };
    }
    // Into eigenvector coordinates, whose axes are (1, 0) and (-0, 1).
    let px = p.0 - cx;
    let py = p.1 - cy;
    let ex = px * 1.0 + py * 0.0;
    let ey = px * -0.0 + py * 1.0;
    let t = (r * ey).atan2(r * ex);
    let qx = r * t.cos();
    let qy = r * t.sin();
    Value::Point {
        x: qx * 1.0 + qy * -0.0 + cx,
        y: qx * 0.0 + qy * 1.0 + cy,
    }
}

pub(crate) fn closest_coords(c: &Construction, path: Id, p: (f64, f64)) -> Option<(f64, f64)> {
    if c.kind(path) == Kind::Function {
        return super::curve::closest(c, path, p);
    }
    if c.kind(path) == Kind::List {
        return closest_list(c.elements(path)?, p);
    }
    let Value::Point { x, y } = closest_point_value(c, path, p)? else {
        return None;
    };
    Some((x, y))
}

fn closest_list(list: crate::expr::List<'_>, p: (f64, f64)) -> Option<(f64, f64)> {
    use crate::expr::Element;
    let mut best = None;
    let mut distance = f64::INFINITY;
    for element in list.iter() {
        let q = match element {
            Element::Value(Kind::Point, Value::Point { x, y }) => Some((x, y)),
            Element::List(nested) => closest_list(nested, p),
            _ => None,
        };
        if let Some((x, y)) = q {
            let d = (x - p.0).hypot(y - p.1);
            if d < distance {
                distance = d;
                best = q;
            }
        }
    }
    best
}

fn closest_point_value(c: &Construction, path: Id, p: (f64, f64)) -> Option<Value> {
    match c.value(path) {
        Value::Line { a, b, c: z } => {
            let (foot, _) = project((a, b, z), (0.0, 0.0), p);
            Some(Value::Point {
                x: foot.0,
                y: foot.1,
            })
        }
        Value::Segment { x1, y1, x2, y2 } => {
            // A segment of length 0 is its start point.
            let (dx, dy) = (x2 - x1, y2 - y1);
            let length = my_length(dx, dy);
            if length == 0.0 || is_zero(length) {
                return Some(Value::Point { x: x1, y: y1 });
            }
            let line = super::join((x1, y1), (x2, y2));
            let (foot, t) = project(line, (x1, y1), p);
            Some(if t < 0.0 {
                Value::Point { x: x1, y: y1 }
            } else if t > 1.0 {
                Value::Point { x: x2, y: y2 }
            } else {
                Value::Point {
                    x: foot.0,
                    y: foot.1,
                }
            })
        }
        Value::Ray { x, y, a, b, c: z } => {
            let (foot, t) = project((a, b, z), (x, y), p);
            Some(if t < 0.0 {
                Value::Point { x, y }
            } else {
                Value::Point {
                    x: foot.0,
                    y: foot.1,
                }
            })
        }
        Value::Circle { cx, cy, r } => Some(closest_on_circle(cx, cy, r, p)),
        _ => super::path::project(c, path, p),
    }
}

/// `AlgoTrilinear.compute` with `GeoPoint.setBarycentric`.
fn trilinear(c: &Construction, ids: [u32; 6]) -> Option<Value> {
    let pa = c.point(id_of(ids[0]))?;
    let pb = c.point(id_of(ids[1]))?;
    let pc = c.point(id_of(ids[2]))?;
    let dist = |p: (f64, f64), q: (f64, f64)| {
        let (dx, dy) = (p.0 - q.0, p.1 - q.1);
        (dx * dx + dy * dy).sqrt()
    };
    let side_a = dist(pb, pc);
    let side_b = dist(pa, pc);
    let side_c = dist(pa, pb);
    let wa = c.number(id_of(ids[3]))? * side_a;
    let wb = c.number(id_of(ids[4]))? * side_b;
    let wc = c.number(id_of(ids[5]))? * side_c;
    barycentric(pa, pb, pc, [wa, wb, wc])
}

fn triangle_center(c: &Construction, a: Id, b: Id, d: Id, index: Count) -> Option<Value> {
    let (pa, pb, pc) = (c.point(a)?, c.point(b)?, c.point(d)?);
    let dist = |a: (f64, f64), b: (f64, f64)| super::length(a.0 - b.0, a.1 - b.1);
    let (a, b, d) = (dist(pb, pc), dist(pc, pa), dist(pa, pb));
    let min = a.min(b).min(d);
    let (a, b, d) = (a / min, b / min, d / min);
    let k = index.value(c) as i32;
    let weights = [
        super::kimberling::weight(k, a, b, d),
        super::kimberling::weight(k, b, d, a),
        super::kimberling::weight(k, d, a, b),
    ];
    let sum = weights[0] + weights[1] + weights[2];
    if sum.is_nan() || is_zero(sum) {
        None
    } else {
        barycentric(pa, pb, pc, weights)
    }
}

fn barycentric(
    pa: (f64, f64),
    pb: (f64, f64),
    pc: (f64, f64),
    [wa, wb, wc]: [f64; 3],
) -> Option<Value> {
    let w = wa + wb + wc;
    // The barycentric combination of the homogeneous vectors (x, y, 0, 1).
    let (ka, kb, kc) = (wa / w, wb / w, wc / w);
    let mut x = pa.0 * ka;
    let mut y = pa.1 * ka;
    let mut z = ka;
    x += kb * pb.0;
    y += kb * pb.1;
    z += kb;
    x += kc * pc.0;
    y += kc * pc.1;
    z += kc;
    hom_point(x, y, z)
}

impl Group for PointDef {
    fn kind(&self) -> Kind {
        Kind::Point
    }

    fn parents(&self) -> Parents {
        match *self {
            PointDef::Closest { path, point } => Parents::of(&[path, point]),
            PointDef::LineIntersection { first, second } => Parents::of(&[first, second]),
            PointDef::Trilinear { ids } => Parents::of(&ids.map(id_of)),
            PointDef::TriangleCenter { a, b, c, index } => {
                if let Some(id) = index.id() {
                    Parents::of(&[a, b, c, id])
                } else {
                    Parents::of(&[a, b, c])
                }
            }
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            PointDef::Closest { path, point } => {
                c.expect(path, CLOSEST_PATH)?;
                c.expect(point, POINT)
            }
            PointDef::LineIntersection { first, second } => {
                c.expect(first, CARRIERS)?;
                c.expect(second, CARRIERS)
            }
            PointDef::TriangleCenter { a, b, c: d, index } => {
                c.expect(a, POINT)?;
                c.expect(b, POINT)?;
                c.expect(d, POINT)?;
                index.check(c)
            }
            PointDef::Trilinear { ids } => {
                for &raw in &ids[..3] {
                    c.expect(id_of(raw), POINT)?;
                }
                for &raw in &ids[3..] {
                    c.expect(id_of(raw), NUMBER)?;
                }
                Ok(())
            }
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        match *self {
            PointDef::Closest { path, point } => {
                let (x, y) = closest_coords(c, path, c.point(point)?)?;
                Some(Value::Point { x, y })
            }
            PointDef::LineIntersection { first, second } => {
                super::intersect::closest_lines(c, first, second)
            }
            PointDef::Trilinear { ids } => trilinear(c, ids),
            PointDef::TriangleCenter { a, b, c: d, index } => triangle_center(c, a, b, d, index),
        }
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        let n = args.len();
        match func {
            "ClosestPoint" => {
                if n != 2 {
                    return Some(Err(format!("`{func}` does not take {n} argument(s)")));
                }
                let (first, second) = match (object(c, args[0]), object(c, args[1])) {
                    (Ok(a), Ok(b)) => (a, b),
                    (Err(e), _) | (_, Err(e)) => return Some(Err(e)),
                };
                let is_path = |id: Id| CLOSEST_PATH.contains(&c.kind(id));
                let is_line = |id: Id| CARRIERS.contains(&c.kind(id));
                Some(match (c.kind(first), c.kind(second)) {
                    (_, Kind::Point) if is_path(first) => Ok(PointDef::Closest {
                        path: first,
                        point: second,
                    }),
                    (Kind::Point, _) if is_path(second) => Ok(PointDef::Closest {
                        path: second,
                        point: first,
                    }),
                    // `AlgoClosestPointLines(g, h)` is made with the arguments swapped.
                    (Kind::Line, Kind::Segment) | (Kind::Segment, Kind::Line) => Err(format!(
                        "`{func}` of a line and a segment is unsupported"
                    )),
                    _ if is_line(first) && is_line(second) => Ok(PointDef::LineIntersection {
                        first: second,
                        second: first,
                    }),
                    (_, _) if is_path(first) && is_path(second) => Err(format!(
                        "`{func}` of two paths is only supported for two lines, give a path and a point"
                    )),
                    (_, Kind::Point) => Err(format!(
                        "`{}` is a {}, expected a path",
                        c.label(first),
                        c.kind(first)
                    )),
                    _ => Err(format!(
                        "`{func}` needs a path and a point"
                    )),
                })
            }
            "TriangleCenter" => Some((|| {
                let &[a, b, d, index] = args else {
                    return Err(format!("`{func}` does not take {n} argument(s)"));
                };
                Ok(PointDef::TriangleCenter {
                    a: object(c, a)?,
                    b: object(c, b)?,
                    c: object(c, d)?,
                    index: super::polygon::count_arg(c, index)?,
                })
            })()),
            "Trilinear" => Some((|| {
                if n != 6 {
                    return Err(format!("`{func}` does not take {n} argument(s)"));
                }
                let mut ids = [0u32; 6];
                for (i, (slot, arg)) in ids.iter_mut().zip(args).enumerate() {
                    let id = object(c, arg)?;
                    let expected = if i < 3 { POINT } else { NUMBER };
                    c.expect(id, expected).map_err(|e| e.to_string())?;
                    *slot =
                        u32::try_from(id.index()).map_err(|_| Error::UnknownObject.to_string())?;
                }
                Ok(PointDef::Trilinear { ids })
            })()),
            _ => None,
        }
    }
}
