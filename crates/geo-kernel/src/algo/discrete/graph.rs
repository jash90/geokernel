use super::{dist_sq, same_point, sample, Budget, Point};
use crate::expr::Element;
use crate::kernel::locus::LocusPoint;
use crate::script::split_call;
use crate::{Construction, Def, Id, Kind, Value};
use std::collections::HashMap;

/// Prim's algorithm on the complete Euclidean graph. Return edges in
/// input-pair order, oriented as in the command's construction of the graph.
pub(super) fn minimum_spanning_tree(
    points: &[Point],
    work: &mut Budget,
) -> Option<Vec<[Point; 2]>> {
    // Dense Prim needs linear storage even when a long list exceeds the budget.
    let n = points.len();
    work.charge(n.checked_mul(n)?.checked_mul(2)?)?;
    let mut added = vec![false; n];
    let mut nearest = vec![(f64::INFINITY, usize::MAX); n];
    let mut tree = Vec::with_capacity(n.saturating_sub(1));
    for _ in 0..n {
        let p = (0..n).filter(|&i| !added[i]).min_by(|&i, &j| {
            nearest[i]
                .0
                .total_cmp(&nearest[j].0)
                .then_with(|| pair(i, nearest[i].1).cmp(&pair(j, nearest[j].1)))
        })?;
        added[p] = true;
        let (_, from) = nearest[p];
        if from != usize::MAX {
            tree.push(pair(from, p));
        }
        for q in (0..n).filter(|&i| !added[i]) {
            let weight = dist_sq(points[p], points[q]).sqrt();
            if weight.is_finite() && (weight, pair(p, q)) < (nearest[q].0, pair(nearest[q].1, q)) {
                nearest[q] = (weight, p);
            }
        }
    }
    work.charge(sort_cost(tree.len()))?;
    tree.sort_unstable();
    Some(
        tree.into_iter()
            .map(|(a, b)| [points[a], points[b]])
            .collect(),
    )
}

fn pair(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

fn sort_cost(n: usize) -> usize {
    n.saturating_mul(usize::BITS as usize - n.leading_zeros() as usize + 1)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum NodeKey {
    Point(Id),
    Coordinates(u64, u64),
    DetachedEndpoint(usize, usize),
}

impl NodeKey {
    fn new(id: Option<Id>, (x, y): Point) -> Self {
        let bits = |v: f64| if v == 0.0 { 0 } else { v.to_bits() };
        id.map_or_else(|| Self::Coordinates(bits(x), bits(y)), Self::Point)
    }
}

#[derive(Default)]
struct Matcher {
    node: Option<usize>,
    exact: bool,
}

impl Matcher {
    fn check(&mut self, target: (Id, Point), id: Option<Id>, point: Point, node: usize) {
        if id == Some(target.0) {
            self.node = Some(node);
            self.exact = true;
        } else if !self.exact && same_point(target.1, point) {
            self.node = Some(node);
        }
    }
}

/// Dijkstra on the segment endpoints. Separate named points at the same
/// coordinates are distinct vertices. For the query points, prefer object
/// identity and fall back to the last coordinate match in input order.
pub(super) fn shortest_path(
    c: &Construction,
    list: Id,
    start: Id,
    end: Id,
    weighted: bool,
    work: &mut Budget,
) -> Option<Vec<LocusPoint>> {
    let list = c.elements(list)?;
    if list.is_empty() {
        return None;
    }
    let (start, end) = ((start, c.point(start)?), (end, c.point(end)?));
    let (mut from, mut to) = (Matcher::default(), Matcher::default());
    let mut keys = HashMap::new();
    let mut points = Vec::new();
    let mut edges = Vec::new();
    let mut neighbors = Vec::<Vec<(usize, usize)>>::new();
    for i in 0..list.len() {
        work.charge(1)?;
        let Some(Element::Value(Kind::Segment, Value::Segment { x1, y1, x2, y2 })) = list.get(i)
        else {
            continue;
        };
        let segment = list.object(i);
        let ids = match segment.map(|id| c.def(id)) {
            Some(Def::Segment(a, b)) => [Some(a), Some(b)],
            _ => [None, None],
        };
        let xy = [(x1, y1), (x2, y2)];
        let nodes = std::array::from_fn(|j| {
            let key = if segment.is_some() {
                NodeKey::new(ids[j], xy[j])
            } else {
                NodeKey::DetachedEndpoint(i, j)
            };
            let node = *keys.entry(key).or_insert_with(|| {
                let node = points.len();
                points.push(xy[j]);
                neighbors.push(Vec::new());
                node
            });
            from.check(start, ids[j], xy[j], node);
            to.check(end, ids[j], xy[j], node);
            node
        });
        let [a, b] = nodes;
        let weight = if weighted {
            // GeoSegment.calcLength uses MyMath.length and snaps tiny
            // segments to length zero.
            let (x, y) = ((x2 - x1).abs(), (y2 - y1).abs());
            let (hi, lo) = (x.max(y), x.min(y));
            let length = if lo == 0.0 {
                hi
            } else {
                let ratio = lo / hi;
                hi * (1.0 + ratio * ratio).sqrt()
            };
            if length < crate::tol::STANDARD_PRECISION {
                0.0
            } else {
                length
            }
        } else {
            1.0
        };
        let edge = edges.len();
        edges.push((nodes, weight));
        neighbors[a].push((b, edge));
        neighbors[b].push((a, edge));
    }
    let (a, b) = (from.node?, to.node?);
    if a == b {
        return Some(vec![sample(start.1, false)]);
    }

    let n = points.len();
    work.charge(n)?;
    let mut distance = vec![f64::INFINITY; n];
    let mut settled = vec![false; n];
    let mut prev = vec![None; n];
    distance[a] = 0.0;
    loop {
        work.charge(n)?;
        let Some(u) = (0..n)
            .filter(|&i| !settled[i] && distance[i].is_finite())
            .min_by(|&i, &j| distance[i].total_cmp(&distance[j]))
        else {
            return Some(Vec::new()); // The endpoints exist, but are disconnected.
        };
        if u == b {
            break;
        }
        settled[u] = true;
        for &(v, edge) in &neighbors[u] {
            work.charge(1)?;
            let next = distance[u] + edges[edge].1;
            if !settled[v] && next < distance[v] {
                distance[v] = next;
                prev[v] = Some((u, edge));
            }
        }
    }
    let mut path = Vec::new();
    let mut current = b;
    while current != a {
        work.charge(1)?;
        let (u, edge) = prev[current]?;
        path.push(edges[edge].0);
        current = u;
    }
    let [first, second] = *path.last()?;
    let mut last = points[if first == a || first == b {
        first
    } else {
        second
    }];
    let mut output = vec![sample(last, false)];
    for [p, q] in path.into_iter().rev() {
        work.charge(1)?;
        // The reference orients each edge by comparing the last coordinates.
        last = if points[p] == last {
            points[q]
        } else {
            points[p]
        };
        output.push(sample(last, true));
    }
    Some(output)
}

/// Resolve inline segments into ordinary construction objects so the list
/// retains the same endpoint identities as a list of named segments.
pub(super) fn inline_segments(
    c: &mut Construction,
    label: &str,
    source: &str,
) -> Option<Result<Id, String>> {
    let inner = source.strip_prefix('{')?.strip_suffix('}')?;
    if !inner.contains("Segment") {
        return None;
    }
    let wrapper = format!("List({inner})");
    let (_, args) = split_call(&wrapper)?;
    if !args
        .iter()
        .any(|arg| split_call(arg).is_some_and(|(name, _)| name == "Segment"))
    {
        return None;
    }
    let start = c.checkpoint();
    let result = (|| {
        let mut ids = Vec::new();
        for arg in args {
            let id = if let Some(("Segment", args)) = split_call(arg) {
                let [a, b] = args.as_slice() else {
                    return Err("Segment needs two arguments".into());
                };
                let a = super::argument(c, a, Kind::Point)?;
                let b = super::argument(c, b, Kind::Point)?;
                c.add(&format!("\0graph{}", c.len()), Def::Segment(a, b))
                    .map_err(|e| e.to_string())?
            } else if let Some(id) = c.id(arg) {
                id
            } else {
                let label = format!("\0graph{}", c.len());
                crate::expr::define(c, &label, arg, false)?
            };
            ids.push(id);
        }
        crate::expr::list_of_objects(c, label, ids)
    })();
    if result.is_err() {
        c.discard_since(start);
    }
    Some(result)
}
