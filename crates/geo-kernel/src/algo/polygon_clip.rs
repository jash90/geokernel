// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdIntersectPath.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectPathLinePolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectPolyLineConicRegion.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectSegmentConicRegion.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolygonOperation.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolygonDifference.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::path::{conic_contains, part_contains, part_end, polygon_contains};
use super::{conic_object, polygon, Parents, CARRIERS, CONIC_OBJECTS};
use crate::expr::IntersectionWork;
use crate::script::object;
use crate::tol::STANDARD_PRECISION;
use crate::{Construction, Error, Id, Kind, Value};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};

type XY = (f64, f64);
type Edge = (XY, XY);

const CONIC_CLIP_PATHS: &[Kind] = &[Kind::Polygon, Kind::PolyLine, Kind::Segment];
const MAX_CONIC_PATH_VERTICES: usize = polygon::MAX_REGULAR_VERTICES;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClipMode {
    Carrier,
    Conic,
    Intersection,
    Difference,
    SymmetricDifference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClipOutput {
    Polygon,
    Point,
    Edge,
    Segment,
}

/// An output slot of polygon clipping. Labels attach to the slots at creation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClipDef {
    first: Id,
    second: Id,
    flag: Option<NonZeroU32>,
    index: u32,
    mode: ClipMode,
    output: ClipOutput,
}

#[derive(Default)]
pub(crate) struct ClipCache {
    segments: Mutex<HashMap<[Id; 2], Arc<[Edge]>>>,
    rings: Mutex<HashMap<ClipDef, Arc<[Vec<XY>]>>>,
}

impl ClipCache {
    pub(crate) fn clear(&mut self) {
        self.segments.get_mut().expect("path clip cache").clear();
        self.rings.get_mut().expect("polygon overlay cache").clear();
    }
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
        } else if CONIC_OBJECTS.contains(&c.kind(first))
            && CONIC_CLIP_PATHS.contains(&c.kind(second))
        {
            (first, second) = (second, first);
            ClipMode::Conic
        } else if CONIC_CLIP_PATHS.contains(&c.kind(first))
            && CONIC_OBJECTS.contains(&c.kind(second))
        {
            ClipMode::Conic
        } else {
            if c.kind(first) != Kind::Polygon {
                (first, second) = (second, first);
            }
            c.expect(first, &[Kind::Polygon])
                .map_err(|e| e.to_string())?;
            match c.kind(second) {
                Kind::Polygon => ClipMode::Intersection,
                kind if CARRIERS.contains(&kind) => ClipMode::Carrier,
                _ => return Err("`IntersectPath` needs two polygons, a polygon and a carrier, or a segment, polyline or polygon and a conic".into()),
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
        c.expect(
            self.first,
            if self.mode == ClipMode::Conic {
                CONIC_CLIP_PATHS
            } else {
                &[Kind::Polygon]
            },
        )?;
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

    pub(crate) fn is_conic(self) -> bool {
        self.mode == ClipMode::Conic
    }

    /// A line can have at most three inside intervals after cutting at two
    /// conic intersections and two closure edges. Saved labels can outnumber
    /// the currently defined intervals.
    pub(crate) fn output_capacity(self, c: &Construction) -> usize {
        if c.kind(self.first) == Kind::Segment {
            3
        } else {
            MAX_CONIC_PATH_VERTICES * 3
        }
    }

    pub(crate) fn slot(self, output: ClipOutput, index: usize) -> Self {
        Self {
            output,
            index: index as u32,
            ..self
        }
    }

    pub(crate) fn rings(self, c: &Construction) -> Arc<[Vec<XY>]> {
        let key = self.slot(ClipOutput::Polygon, 0);
        let cache = &c.clips.rings;
        if let Some(rings) = cache.lock().expect("polygon overlay cache").get(&key) {
            return Arc::clone(rings);
        }
        // An input can itself be a clipped polygon. Release the lock before
        // fetching its vertices so its own overlay can use this cache.
        let rings = self.overlay(c).unwrap_or_default().into();
        cache
            .lock()
            .expect("polygon overlay cache")
            .insert(key, Arc::clone(&rings));
        rings
    }

    fn overlay(self, c: &Construction) -> Option<Vec<Vec<XY>>> {
        let mode = if self.flag().is_some_and(|i| c.value(i) == Value::Bool(true)) {
            ClipMode::SymmetricDifference
        } else {
            self.mode
        };
        let a = c.vertices(self.first);
        let b = c.vertices(self.second);
        let work = IntersectionWork::new();
        if !charge_overlay_pairs(&work, a.size_hint().0, b.size_hint().0) {
            return None;
        }
        let a = a.collect::<Option<Vec<_>>>()?;
        let b = b.collect::<Option<Vec<_>>>()?;
        overlay(&a, &b, mode, &work)
    }

    pub(crate) fn vertices(self, c: &Construction) -> Option<Vec<XY>> {
        self.rings(c).get(self.index as usize).cloned()
    }

    pub(crate) fn segments(self, c: &Construction) -> Arc<[Edge]> {
        let key = [self.first, self.second];
        let cache = &c.clips.segments;
        if let Some(segments) = cache.lock().expect("path clip cache").get(&key) {
            return Arc::clone(segments);
        }
        let segments = if self.mode == ClipMode::Conic {
            conic_segments(c, self.first, self.second)
        } else {
            carrier_segments(c, self.first, self.second).unwrap_or_default()
        };
        let segments = segments.into();
        cache
            .lock()
            .expect("path clip cache")
            .insert(key, Arc::clone(&segments));
        segments
    }

    pub(crate) fn compute(self, c: &Construction) -> Option<Value> {
        let index = self.index as usize;
        match self.output {
            ClipOutput::Polygon => Some(Value::Polygon {
                area: area(&self.vertices(c)?),
            }),
            ClipOutput::Point => {
                let &(x, y) = self.rings(c).iter().flatten().nth(index)?;
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
fn carrier_segments(c: &Construction, polygon: Id, line: Id) -> Option<Vec<Edge>> {
    let vertices = c.vertices(polygon);
    let count = vertices.size_hint().0 as u64;
    // An edge supplies at most two cuts and each resulting interval can
    // require a full containment scan. Include the endpoint cuts as well.
    let cuts = count.saturating_mul(2).saturating_add(2);
    let work = IntersectionWork::new();
    if !work.charge(count.saturating_add(1).saturating_mul(cuts)) {
        return None;
    }
    let vertices = vertices.collect::<Option<Vec<_>>>()?;
    Some(carrier(c.value(line), &vertices))
}

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

fn conic_segments(c: &Construction, path: Id, id: Id) -> Vec<Edge> {
    let Some(d) = conic_object(c, id).filter(|d| d.defined) else {
        return Vec::new();
    };
    let arc = c.arc(id);
    let inside = |p| conic_contains(&d, p) && arc.as_ref().is_none_or(|a| part_contains(a, p));
    let [xx, yy, z, xy, xz, yz] = d.matrix;
    let mut result = Vec::new();
    let kind = c.kind(path);
    let work = IntersectionWork::new();
    let vertices;
    let path_edges: Box<dyn Iterator<Item = Edge>> = match kind {
        Kind::Segment => {
            let Value::Segment { x1, y1, x2, y2 } = c.value(path) else {
                return result;
            };
            Box::new(std::iter::once(((x1, y1), (x2, y2))))
        }
        Kind::PolyLine | Kind::Polygon => {
            let points = c.vertices(path);
            let count = points.size_hint().0;
            // Each edge has at most two conic roots and two closure cuts.
            // Reserve a fixed allowance for collecting, cutting and testing it.
            if count > MAX_CONIC_PATH_VERTICES || !work.charge(32 * count as u64) {
                return result;
            }
            vertices = points.collect::<Vec<_>>();
            if kind == Kind::Polygon && vertices.iter().any(Option::is_none) {
                return result;
            }
            let points = vertices.iter().copied();
            let count = if kind == Kind::Polygon {
                count
            } else {
                count.saturating_sub(1)
            };
            Box::new(
                points
                    .clone()
                    .zip(points.cycle().skip(1))
                    .take(count)
                    .filter_map(|(p, q)| p.zip(q)),
            )
        }
        _ => return result,
    };
    let mut cuts = Vec::with_capacity(6);
    for (p, end) in path_edges {
        let dir = sub(end, p);
        if dir == (0.0, 0.0) {
            if inside(p) {
                result.push((p, end));
            }
            continue;
        }
        let (x, y) = p;
        let (u, v) = dir;
        let qa = xx * u * u + yy * v * v + 2.0 * xy * u * v;
        let qb = u * (xx * x + xy * y + xz) + v * (xy * x + yy * y + yz);
        let qc = xx * x * x + yy * y * y + 2.0 * (xy * x * y + xz * x + yz * y) + z;
        cuts.clear();
        cuts.extend([0.0, 1.0]);
        let discriminant = qb * qb - qa * qc;
        let rounding = 8.0 * f64::EPSILON * (qb * qb + (qa * qc).abs());
        if qa.abs() < STANDARD_PRECISION {
            if qb.abs() >= STANDARD_PRECISION {
                cuts.push(-qc / (2.0 * qb));
            }
        } else if discriminant.abs() <= rounding {
            cuts.push(-qb / qa);
        } else if discriminant > 0.0 {
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

fn charge_overlay_pairs(work: &IntersectionWork, a: usize, b: usize) -> bool {
    let vertices = (a as u64).saturating_add(b as u64);
    work.charge(vertices.saturating_mul(vertices.saturating_add(1)))
}

/// Split all boundary edges at crossings, orient the pieces with the result
/// interior on the left, and join them into closed rings.
fn overlay(a: &[XY], b: &[XY], mode: ClipMode, work: &IntersectionWork) -> Option<Vec<Vec<XY>>> {
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
        // Charge sorting, both containment tests per piece, and boundary scans.
        let cuts_len = cuts.len() as u64;
        if !work.charge(cuts_len * u64::from(cuts_len.ilog2() + 1)) {
            return None;
        }
        restrict(&mut cuts, 0.0, 1.0);
        for cut in cuts.windows(2) {
            if !work.charge(2 * (a.len() + b.len()) as u64 + 1) {
                return None;
            }
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
            if !work.charge(boundary.len() as u64 + 1) {
                return None;
            }
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
            if !work.charge(boundary.len() as u64 + 1) {
                return None;
            }
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
        if !work.charge(4 * ring.len() as u64) {
            return None;
        }
        remove_collinear(&mut ring, work)?;
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
    if !work.charge(rings.len() as u64 * u64::from(rings.len().max(1).ilog2() + 1)) {
        return None;
    }
    rings.sort_by(|a, b| b[0].1.total_cmp(&a[0].1).then(a[0].0.total_cmp(&b[0].0)));
    Some(rings)
}

fn remove_collinear(ring: &mut Vec<XY>, work: &IntersectionWork) -> Option<()> {
    let mut i = ring.len();
    while i > 0 && ring.len() > 2 {
        i -= 1;
        let n = ring.len();
        let a = sub(ring[i], ring[(i + n - 1) % n]);
        let b = sub(ring[(i + 1) % n], ring[i]);
        if cross(a, b).abs() < STANDARD_PRECISION && dot(a, b) >= 0.0 {
            if !work.charge((n - i) as u64) {
                return None;
            }
            ring.remove(i);
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{expr, script};

    #[test]
    fn an_empty_polygon_clip_keeps_the_labelled_point_slot() {
        // The initial snapshot of misc-alias-003 records an undefined
        // polygon and an undefined point, with no labelled segment.
        let mut c = script::load(
            "A = (0, 0)\nB = (2, 0)\nC = (1, 2)\nD = (5, 5)\nE = (7, 5)\nF = (6, 7)\np = Polygon(A, B, C)\nq = Polygon(D, E, F)",
        )
        .unwrap();
        let before = c.len();
        let clip = script::exec(&mut c, "r = IntersectRegion(p, q)")
            .unwrap()
            .unwrap();
        let point = c.id("G").expect("the labelled point of the empty clip");
        assert_eq!(c.kind(clip), Kind::Polygon);
        assert_eq!(c.kind(point), Kind::Point);
        assert_eq!(
            (c.value(clip), c.value(point)),
            (Value::Undefined, Value::Undefined)
        );
        assert_eq!(c.len() - before, 2);
    }

    #[test]
    fn polygon_conic_outputs_share_the_clip_until_the_next_update() {
        let mut c = script::load("A = (0, 0)\nB = (4, 0)\nD = (0, 4)\np = Polygon(A, B, D)\nO = (1, 1)\nk = Circle(O, 3)").unwrap();
        let clip = ClipDef::parse(&c, "IntersectPath", &["p", "k"]).unwrap();
        let before = expr::work_done();
        let segments = clip.segments(&c);
        assert!(expr::work_done() > before);
        assert!(segments.len() >= 2);
        let after = expr::work_done();
        for (i, edge) in segments.iter().enumerate() {
            assert_eq!(
                clip.slot(ClipOutput::Segment, i).compute(&c),
                Some(segment(*edge))
            );
        }
        assert_eq!(expr::work_done(), after);

        c.move_point(c.id("O").unwrap(), 50.0, 50.0).unwrap();
        assert!(clip.segments(&c).is_empty());
    }

    #[test]
    fn overlay_refuses_more_vertices_than_the_work_bound_allows() {
        // Even the initial edge-pair pass exceeds the work allowance.
        let work = IntersectionWork::new();
        let before = expr::work_done();
        assert!(!charge_overlay_pairs(&work, 10_000, 10_000));
        assert!(work.exhausted());
        assert_eq!(expr::work_done(), before);
    }

    #[test]
    fn polygon_overlay_charges_one_solve_for_its_outputs() {
        let c = script::load(
            "A = (0, 0)\nB = (4, 0)\nC = (0, 4)\np = Polygon(A, B, C)\nq = Polygon(A, B, C)",
        )
        .unwrap();
        let clip = ClipDef::parse(&c, "IntersectPath", &["p", "q"]).unwrap();
        let before = expr::work_done();
        let rings = clip.rings(&c);
        assert_eq!(rings.len(), 1);
        let after = expr::work_done();
        assert!(after > before);
        assert!(clip.slot(ClipOutput::Edge, 0).compute(&c).is_some());
        assert_eq!(expr::work_done(), after);
        assert!(expr::work_done() > before);
    }
}
