// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdIntersectPath.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectPathLinePolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectPolyLineConicRegion.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolygonOperation.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolygonDifference.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::path::{conic_contains, part_contains, part_end, polygon_contains};
use super::{conic_object, polygon, Parents, CARRIERS, CONIC_OBJECTS};
use crate::script::object;
use crate::tol::STANDARD_PRECISION;
use crate::{Construction, Error, Id, Kind, Value};
use std::cmp::Ordering;
use std::num::NonZeroU32;

type XY = (f64, f64);
type Edge = (XY, XY);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClipMode {
    Carrier,
    Conic,
    Intersection,
    Difference,
    SymmetricDifference,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClipOutput {
    Polygon,
    Point,
    Edge,
    Segment,
}

/// An output slot of polygon clipping. Labels attach to the slots at creation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipDef {
    first: Id,
    second: Id,
    flag: Option<NonZeroU32>,
    index: u32,
    mode: ClipMode,
    output: ClipOutput,
}

impl ClipDef {
    pub(crate) fn parse(c: &Construction, func: &str, args: &[&str]) -> Result<Self, String> {
        let (a, b, xor) = match (func, args) {
            ("IntersectPath" | "IntersectionPaths" | "IntersectRegion" | "Difference", [a, b]) => {
                (*a, *b, None)
            }

            ("Difference", [a, b, xor]) => (*a, *b, Some(*xor)),
            _ => return Err(format!("`{func}` does not take {} argument(s)", args.len())),
        };
        let (mut first, mut second) = (object(c, a)?, object(c, b)?);
        let mut flag = None;
        let mode = if func == "Difference" {
            c.expect(first, &[Kind::Polygon])
                .map_err(|e| e.to_string())?;
            c.expect(second, &[Kind::Polygon])
                .map_err(|e| e.to_string())?;
            match xor {
                None | Some("false") => ClipMode::Difference,
                Some("true") => ClipMode::SymmetricDifference,
                Some(label) => {
                    let id = object(c, label)?;
                    c.expect(id, &[Kind::Boolean]).map_err(|e| e.to_string())?;
                    flag = u32::try_from(id.index())
                        .ok()
                        .and_then(|n| n.checked_add(1))
                        .and_then(NonZeroU32::new);
                    if flag.is_none() {
                        return Err("the Boolean handle is out of range".into());
                    }
                    ClipMode::Difference
                }
            }
        } else {
            if c.kind(first) != Kind::Polygon {
                (first, second) = (second, first);
            }
            c.expect(first, &[Kind::Polygon])
                .map_err(|e| e.to_string())?;
            match c.kind(second) {
                Kind::Polygon => ClipMode::Intersection,
                kind if CARRIERS.contains(&kind) => ClipMode::Carrier,
                kind if CONIC_OBJECTS.contains(&kind) => ClipMode::Conic,
                _ => return Err(
                    "`IntersectPath` needs a polygon and a polygon, line, ray, segment or conic"
                        .into(),
                ),
            }
        };
        let output = if matches!(mode, ClipMode::Carrier | ClipMode::Conic) {
            ClipOutput::Segment
        } else {
            ClipOutput::Polygon
        };
        Ok(Self {
            first,
            second,
            flag,
            index: 0,
            mode,
            output,
        })
    }

    pub(crate) fn flag(self) -> Option<Id> {
        self.flag.map(|id| Id(id.get() as usize - 1))
    }

    pub(crate) fn kind(self) -> Kind {
        match self.output {
            ClipOutput::Polygon => Kind::Polygon,
            ClipOutput::Point => Kind::Point,
            ClipOutput::Edge | ClipOutput::Segment => Kind::Segment,
        }
    }

    pub(crate) fn parents(self) -> Parents {
        if let Some(id) = self.flag() {
            Parents::of(&[self.first, self.second, id])
        } else {
            Parents::of(&[self.first, self.second])
        }
    }

    pub(crate) fn hidden_parents(self, c: &Construction) -> Vec<Id> {
        let mut ids = polygon::inputs(c, self.first);
        if c.kind(self.second) == Kind::Polygon {
            ids.extend(polygon::inputs(c, self.second));
        }
        ids
    }

    pub(crate) fn check(self, c: &Construction) -> Result<(), Error> {
        c.expect(self.first, &[Kind::Polygon])?;
        c.expect(
            self.second,
            match self.mode {
                ClipMode::Carrier => CARRIERS,
                ClipMode::Conic => CONIC_OBJECTS,
                _ => &[Kind::Polygon],
            },
        )?;
        if let Some(flag) = self.flag() {
            c.expect(flag, &[Kind::Boolean])?;
        }
        Ok(())
    }

    pub(crate) fn is_carrier(self) -> bool {
        self.mode == ClipMode::Carrier
    }

    pub(crate) fn slot(self, output: ClipOutput, index: usize) -> Self {
        Self {
            output,
            index: index as u32,
            ..self
        }
    }

    pub(crate) fn rings(self, c: &Construction) -> Vec<Vec<XY>> {
        let vertices = |id| c.vertices(id).collect::<Option<Vec<_>>>();
        let Some((a, b)) = vertices(self.first).zip(vertices(self.second)) else {
            return Vec::new();
        };
        let mode = if self.flag().is_some_and(|i| c.value(i) == Value::Bool(true)) {
            ClipMode::SymmetricDifference
        } else {
            self.mode
        };
        overlay(&a, &b, mode)
    }

    pub(crate) fn vertices(self, c: &Construction) -> Option<Vec<XY>> {
        self.rings(c).into_iter().nth(self.index as usize)
    }

    pub(crate) fn segments(self, c: &Construction) -> Vec<Edge> {
        let Some(vertices) = c.vertices(self.first).collect::<Option<Vec<_>>>() else {
            return Vec::new();
        };
        if self.mode == ClipMode::Carrier {
            carrier(c.value(self.second), &vertices)
        } else {
            conic_segments(c, self.second, &vertices)
        }
    }

    pub(crate) fn compute(self, c: &Construction) -> Option<Value> {
        let index = self.index as usize;
        match self.output {
            ClipOutput::Polygon => Some(Value::Polygon {
                area: area(&self.vertices(c)?),
            }),
            ClipOutput::Point => {
                let (x, y) = self.rings(c).into_iter().flatten().nth(index)?;
                Some(Value::Point { x, y })
            }
            ClipOutput::Edge => self
                .rings(c)
                .iter()
                .flat_map(|ring| edges(ring))
                .nth(index)
                .map(segment),
            ClipOutput::Segment => self.segments(c).get(index).copied().map(segment),
        }
    }
}

fn segment(((x1, y1), (x2, y2)): Edge) -> Value {
    Value::Segment { x1, y1, x2, y2 }
}

fn edges(vertices: &[XY]) -> impl Iterator<Item = Edge> + '_ {
    vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
}

fn add(a: XY, b: XY) -> XY {
    (a.0 + b.0, a.1 + b.1)
}

fn sub(a: XY, b: XY) -> XY {
    (a.0 - b.0, a.1 - b.1)
}

fn scale(a: XY, t: f64) -> XY {
    (t * a.0, t * a.1)
}

fn dot(a: XY, b: XY) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

fn cross(a: XY, b: XY) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

fn near(a: XY, b: XY) -> bool {
    (a.0 - b.0).abs().max((a.1 - b.1).abs()) < STANDARD_PRECISION
}

fn cuts_on_line(origin: XY, dir: XY, (a, b): Edge, cuts: &mut Vec<f64>) {
    let offset = sub(a, origin);
    let edge = sub(b, a);
    let denom = cross(dir, edge);
    if denom.abs() < STANDARD_PRECISION {
        if cross(offset, dir).abs() < STANDARD_PRECISION {
            let norm = dot(dir, dir);
            cuts.extend([dot(offset, dir) / norm, dot(sub(b, origin), dir) / norm]);
        }
    } else {
        let other = cross(offset, dir) / denom;
        if (-STANDARD_PRECISION..=1.0 + STANDARD_PRECISION).contains(&other) {
            cuts.push(cross(offset, edge) / denom);
        }
    }
}

fn restrict(cuts: &mut Vec<f64>, min: f64, max: f64) {
    cuts.retain(|t| t.is_finite() && min <= *t && *t <= max);
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < STANDARD_PRECISION);
}

/// Intersect intervals along the carrier and combine consecutive inside intervals.
fn carrier(line: Value, polygon: &[XY]) -> Vec<Edge> {
    let (origin, dir, min, max) = match line {
        Value::Line { a, b, c } => {
            let origin = if a.abs() > b.abs() {
                (-c / a, 0.0)
            } else {
                (0.0, -c / b)
            };
            (origin, (b, -a), f64::NEG_INFINITY, f64::INFINITY)
        }
        Value::Ray { x, y, a, b, .. } => ((x, y), (b, -a), 0.0, f64::INFINITY),
        Value::Segment { x1, y1, x2, y2 } => ((x1, y1), (x2 - x1, y2 - y1), 0.0, 1.0),
        _ => return Vec::new(),
    };
    if dir == (0.0, 0.0) {
        return Vec::new();
    }
    let mut cuts = vec![min, max];
    for edge in edges(polygon) {
        cuts_on_line(origin, dir, edge, &mut cuts);
    }
    restrict(&mut cuts, min, max);
    let mut segments: Vec<Edge> = Vec::new();
    let along = |t| add(origin, scale(dir, t));
    for interval in cuts.windows(2) {
        let (a, b) = (interval[0], interval[1]);
        if polygon_contains(polygon, along(a.midpoint(b))) {
            let start = along(a);
            let end = along(b);
            if let Some(last) = segments.last_mut() {
                if near(last.1, start) {
                    last.1 = end;
                    continue;
                }
            }
            segments.push((start, end));
        }
    }
    segments
}

fn conic_segments(c: &Construction, id: Id, polygon: &[XY]) -> Vec<Edge> {
    let Some(d) = conic_object(c, id).filter(|d| d.defined) else {
        return Vec::new();
    };
    let arc = c.arc(id);
    let inside = |p| conic_contains(&d, p) && arc.as_ref().is_none_or(|a| part_contains(a, p));
    let [xx, yy, z, xy, xz, yz] = d.matrix;
    let mut result = Vec::new();
    for (p, end) in edges(polygon) {
        let dir = sub(end, p);
        if dir == (0.0, 0.0) {
            continue;
        }
        let (x, y) = p;
        let (u, v) = dir;
        let qa = xx * u * u + yy * v * v + 2.0 * xy * u * v;
        let qb = u * (xx * x + xy * y + xz) + v * (xy * x + yy * y + yz);
        let qc = xx * x * x + yy * y * y + 2.0 * (xy * x * y + xz * x + yz * y) + z;
        let mut cuts = vec![0.0, 1.0];
        let discriminant = qb * qb - qa * qc;
        if qa.abs() < STANDARD_PRECISION {
            if qb.abs() >= STANDARD_PRECISION {
                cuts.push(-qc / (2.0 * qb));
            }
        } else if discriminant >= 0.0 {
            let root = discriminant.sqrt();
            cuts.extend([(-qb - root) / qa, (-qb + root) / qa]);
        }
        if let Some(arc) = &arc {
            let a = part_end(arc, arc.start);
            let b = part_end(arc, arc.end);
            if arc.sector {
                let [cx, cy] = d.midpoint;
                cuts_on_line(p, dir, ((cx, cy), a), &mut cuts);
                cuts_on_line(p, dir, ((cx, cy), b), &mut cuts);
            } else {
                cuts_on_line(p, dir, (a, b), &mut cuts);
            }
        }
        restrict(&mut cuts, 0.0, 1.0);
        let along = |t| add(p, scale(dir, t));
        for interval in cuts.windows(2) {
            let (a, b) = (interval[0], interval[1]);
            if inside(along(a.midpoint(b))) {
                result.push((along(a), along(b)));
            }
        }
    }
    result
}

fn contains(vertices: &[XY], p: XY) -> bool {
    edges(vertices)
        .filter(|&(a, b)| {
            (a.1 > p.1) != (b.1 > p.1) && (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 > p.0
        })
        .count()
        % 2
        == 1
}

fn area(ring: &[XY]) -> f64 {
    edges(ring).map(|(p, q)| cross(p, q)).sum::<f64>() / 2.0
}

fn top_right(a: XY, b: XY) -> Ordering {
    a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0))
}

/// Split all boundary edges at crossings, orient the pieces with the result
/// interior on the left, and join them into closed rings.
fn overlay(a: &[XY], b: &[XY], mode: ClipMode) -> Vec<Vec<XY>> {
    let all: Vec<_> = edges(a)
        .chain(edges(b))
        .filter(|&(p, q)| !near(p, q))
        .collect();
    let inside = |p| {
        let (in_a, in_b) = (contains(a, p), contains(b, p));
        match mode {
            ClipMode::Intersection => in_a && in_b,
            ClipMode::SymmetricDifference => in_a != in_b,
            _ => in_a && !in_b,
        }
    };
    let mut boundary: Vec<Edge> = Vec::new();
    for &(origin, end) in &all {
        let dir = sub(end, origin);
        let mut cuts = vec![0.0, 1.0];
        for &edge in &all {
            cuts_on_line(origin, dir, edge, &mut cuts);
        }
        restrict(&mut cuts, 0.0, 1.0);
        for cut in cuts.windows(2) {
            let (u, v) = (cut[0], cut[1]);
            let p = add(origin, scale(dir, u));
            let q = add(origin, scale(dir, v));
            let mid = add(origin, scale(dir, u.midpoint(v)));
            let length = dir.0.hypot(dir.1);
            let offset = (length * (v - u) * 0.000001)
                .max(32.0 * f64::EPSILON * mid.0.abs().max(mid.1.abs()).max(1.0));
            let normal = scale((-dir.1, dir.0), offset / length);
            let piece = match (inside(add(mid, normal)), inside(sub(mid, normal))) {
                (true, false) => (p, q),
                (false, true) => (q, p),
                _ => continue,
            };
            if !boundary
                .iter()
                .any(|&(a, b)| near(a, piece.0) && near(b, piece.1))
            {
                boundary.push(piece);
            }
        }
    }
    let mut rings = Vec::new();
    while let Some((start, mut end)) = boundary.pop() {
        let mut ring = vec![start];
        while !near(end, start) {
            let incoming = sub(end, *ring.last().unwrap());
            let turn = |q| {
                let out = sub(q, end);
                cross(incoming, out).atan2(dot(incoming, out))
            };
            let next = boundary
                .iter()
                .enumerate()
                .filter(|&(_, &(p, _))| near(p, end))
                .max_by(|a, b| turn(a.1 .1).total_cmp(&turn(b.1 .1)))
                .map(|(i, _)| i);
            let Some(next) = next else { break };
            ring.push(end);
            end = boundary.swap_remove(next).1;
        }
        if !near(end, start) || ring.len() < 3 {
            continue;
        }
        remove_collinear(&mut ring);
        let signed = area(&ring);
        if signed.abs() < STANDARD_PRECISION || ring.len() < 3 {
            continue;
        }
        let first = if signed > 0.0 {
            ring.iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| top_right(**a, **b))
        } else {
            ring.iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| top_right(**a, **b))
        };
        if let Some((i, _)) = first {
            ring.rotate_left(i);
        }
        rings.push(ring);
    }
    rings.sort_by(|a, b| b[0].1.total_cmp(&a[0].1).then(a[0].0.total_cmp(&b[0].0)));
    rings
}

fn remove_collinear(ring: &mut Vec<XY>) {
    let mut i = ring.len();
    while i > 0 && ring.len() > 2 {
        i -= 1;
        let n = ring.len();
        let a = sub(ring[i], ring[(i + n - 1) % n]);
        let b = sub(ring[(i + 1) % n], ring[i]);
        if cross(a, b).abs() < STANDARD_PRECISION && dot(a, b) >= 0.0 {
            ring.remove(i);
        }
    }
}
