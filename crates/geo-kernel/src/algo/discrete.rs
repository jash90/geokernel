// Command behavior and tour heuristic translated from GeoGebra
// (https://github.com/geogebra/geogebra), commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/discrete/
//   AlgoConvexHull.java, AlgoDelauneyTriangulation.java, AlgoVoronoi.java,
//   AlgoMinimumSpanningTree.java, AlgoShortestDistance.java,
//   AlgoTravelingSalesman.java, tsp/impl/FLS.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

mod graph;
mod ordered;
mod triangulation;

use crate::algo::measure::MeasureDef;
use crate::algo::{Parents, POINT};
use crate::expr::{math::is_equal, Element, MAX_EVALUATION_WORK};
use crate::kernel::locus::{locus_from_points, LocusData, LocusPoint};
use crate::script::split_call;
use crate::{Construction, Def, Error, Id, Kind, Value};
use std::cmp::Ordering;
use std::collections::HashSet;

pub(super) type Point = (f64, f64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointGraph {
    Hull,
    Delaunay,
    Voronoi,
    SpanningTree,
    Tour,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DiscreteDef {
    Points {
        graph: PointGraph,
        list: Id,
    },
    ShortestPath {
        list: Id,
        start: Id,
        end: Id,
        weighted: Id,
    },
}

impl DiscreteDef {
    pub(crate) fn parents(self) -> Parents {
        match self {
            Self::Points { list, .. } => Parents::of(&[list]),
            Self::ShortestPath {
                list,
                start,
                end,
                weighted,
            } => Parents::of(&[list, start, end, weighted]),
        }
    }

    pub(crate) fn check(self, c: &Construction) -> Result<(), Error> {
        match self {
            Self::Points { list, .. } => c.expect(list, &[Kind::List]),
            Self::ShortestPath {
                list,
                start,
                end,
                weighted,
            } => {
                c.expect(list, &[Kind::List])?;
                c.expect(start, POINT)?;
                c.expect(end, POINT)?;
                c.expect(weighted, &[Kind::Boolean])
            }
        }
    }

    pub(crate) fn record(self, c: &Construction, previous: Option<&LocusData>) -> LocusData {
        // With one entry the reference never enters the graph-building loop,
        // so the previous locus survives this size change.
        if let Self::Points {
            graph: PointGraph::SpanningTree,
            list,
        } = self
        {
            if c.elements(list).is_some_and(|points| points.len() == 1) {
                return previous.cloned().unwrap_or_default();
            }
        }
        let mut work = Budget(MAX_EVALUATION_WORK);
        let Some(points) = self.compute(c, &mut work) else {
            return LocusData::default();
        };
        if work.charge(points.len()).is_none() {
            return LocusData::default();
        }
        locus_from_points(points)
    }

    fn compute(self, c: &Construction, work: &mut Budget) -> Option<Vec<LocusPoint>> {
        let (graph, list) = match self {
            Self::Points { graph, list } => (graph, list),
            Self::ShortestPath {
                list,
                start,
                end,
                weighted,
            } => {
                let Value::Bool(weighted) = c.value(weighted) else {
                    return None;
                };
                return graph::shortest_path(c, list, start, end, weighted, work);
            }
        };
        let list = c.elements(list)?;
        if graph == PointGraph::Tour && list.len() < 3
            || graph == PointGraph::SpanningTree && list.len() < 2
        {
            return None;
        }

        let mut points = Vec::new();
        let mut seen = HashSet::new();
        for (i, item) in list.iter().enumerate() {
            work.charge(1)?;
            match item {
                Element::Value(Kind::Point, Value::Point { x, y }) => {
                    // The first pair looks up both endpoints before adding
                    // them, even when both names refer to the same point.
                    if graph != PointGraph::SpanningTree
                        || list.object(i).is_none_or(|id| seen.insert(id) || i == 1)
                    {
                        points.push((x, y));
                    }
                }
                _ if graph == PointGraph::Delaunay => break, // terminator in the reference array
                _ if graph == PointGraph::Tour => return None,
                Element::Value(Kind::Point, _) if graph == PointGraph::SpanningTree => {
                    // Undefined points have no finite edges.
                }
                _ if graph == PointGraph::SpanningTree => return None,
                _ => {}
            }
        }

        match graph {
            PointGraph::Hull => hull(points, work),
            PointGraph::Tour => tour(points, work),
            PointGraph::SpanningTree => {
                segments(graph::minimum_spanning_tree(&points, work)?, work)
            }
            PointGraph::Delaunay => segments(
                triangulation::Triangulation::new(&points, work)?.edges(false, work)?,
                work,
            ),
            PointGraph::Voronoi => {
                let points = voronoi_sites(points, work)?;
                segments(
                    triangulation::Triangulation::new(&points, work)?.edges(true, work)?,
                    work,
                )
            }
        }
    }
}

struct Budget(u64);

impl Budget {
    fn charge(&mut self, n: usize) -> Option<()> {
        self.0 = self.0.checked_sub(n as u64)?;
        Some(())
    }
}

fn dist_sq(a: Point, b: Point) -> f64 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    dx * dx + dy * dy
}

fn sample((x, y): Point, line_to: bool) -> LocusPoint {
    LocusPoint { x, y, line_to }
}

fn segments(edges: Vec<[Point; 2]>, work: &mut Budget) -> Option<Vec<LocusPoint>> {
    work.charge(edges.len() * 2)?;
    Some(
        edges
            .into_iter()
            .flat_map(|[a, b]| [sample(a, false), sample(b, true)])
            .collect(),
    )
}

fn close(points: Vec<Point>, work: &mut Budget) -> Option<Vec<LocusPoint>> {
    let first = *points.first()?;
    work.charge(points.len() + 1)?;
    Some(
        points
            .into_iter()
            .chain([first])
            .enumerate()
            .map(|(i, p)| sample(p, i != 0))
            .collect(),
    )
}

fn same_point(a: Point, b: Point) -> bool {
    is_equal(a.0, b.0) && is_equal(a.1, b.1)
}

/// Build the clockwise boundary with a monotone chain in each scan direction.
fn hull(points: Vec<Point>, work: &mut Budget) -> Option<Vec<LocusPoint>> {
    let mut sites = Vec::new();
    for p in points {
        work.charge(sites.len() + 1)?;
        if !sites.iter().any(|&q| same_point(p, q)) {
            sites.push(p);
        }
    }
    if sites.len() <= 1 {
        return close(sites, work);
    }
    let n = sites.len();
    work.charge(n * (1 + n.ilog2() as usize))?;
    sites.sort_unstable_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .expect("finite site coordinates")
            .then_with(|| a.1.partial_cmp(&b.1).expect("finite site coordinates"))
    });

    let mut boundary = Vec::with_capacity(n);
    append_hull_side(&mut boundary, sites.iter().copied(), work)?;
    append_hull_side(&mut boundary, sites.into_iter().rev(), work)?;
    close(boundary, work)
}

fn append_hull_side(
    boundary: &mut Vec<Point>,
    sites: impl Iterator<Item = Point>,
    work: &mut Budget,
) -> Option<()> {
    let start = boundary.len();
    for p in sites {
        work.charge(1)?;
        while boundary.len() - start >= 2 {
            work.charge(1)?;
            let len = boundary.len();
            let (a, b) = (boundary[len - 2], boundary[len - 1]);
            let turn = (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
            if turn >= 0.0 {
                boundary.pop();
            } else {
                break;
            }
        }
        boundary.push(p);
    }
    // The next side starts at this endpoint. Each side can retain at most n - 1 sites.
    boundary.pop();
    Some(())
}

/// Preserve the reference's per-axis perturbation of repeated coordinates.
fn voronoi_sites(points: Vec<Point>, work: &mut Budget) -> Option<Vec<Point>> {
    let mut sites = ordered::Tree::default();
    for p in points {
        sites.insert(
            p,
            |a, b| {
                if same_point(a, b) {
                    Ordering::Equal
                } else if a.0 > b.0 {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            },
            work,
        )?;
    }
    let mut x_used = HashSet::new();
    let mut y_used = HashSet::new();
    let mut perturbed = Vec::new();
    for (mut x, mut y) in sites.into_values(work)? {
        work.charge(1)?;
        for (v, used) in [(&mut x, &mut x_used), (&mut y, &mut y_used)] {
            while !used.insert(v.to_bits()) {
                work.charge(1)?;
                let next = *v + 1e-7;
                if next == *v {
                    return None;
                }
                *v = next;
            }
        }
        perturbed.push((x, y));
    }
    Some(perturbed)
}

/// First improving 2-opt move, trying the incoming edge before the outgoing
/// edge. The active bits belong to the points and move with the tour.
fn tour(points: Vec<Point>, work: &mut Budget) -> Option<Vec<LocusPoint>> {
    let n = points.len();
    work.charge(n)?;
    let mut nodes: Vec<_> = points.into_iter().map(|p| (p, true)).collect();
    let mut skipped = 0;
    let mut i = 0;
    while skipped < n {
        work.charge(1)?;
        let prev = (i + n - 1) % n;
        let next = (i + 1) % n;
        if nodes[i].1 {
            let mut improvement = None;
            for offset in 2..n - 1 {
                let c = (i + offset) % n;
                let d = (c + 1) % n;
                for (a, b) in [(prev, i), (i, next)] {
                    work.charge(1)?;
                    let [a_point, b_point, c_point, d_point] = [a, b, c, d].map(|k| nodes[k].0);
                    let (ab, cd) = (dist_sq(a_point, b_point), dist_sq(c_point, d_point));
                    let (ac, bd) = (dist_sq(a_point, c_point), dist_sq(b_point, d_point));
                    if ab < ac && cd < bd {
                        continue;
                    }
                    let old = ab.sqrt() + cd.sqrt();
                    let new = ac.sqrt() + bd.sqrt();
                    if new < old {
                        improvement = Some((a, b, c, d));
                        break;
                    }
                }
                if improvement.is_some() {
                    break;
                }
            }
            if let Some((a, b, c, d)) = improvement {
                for k in [a, b, c, d] {
                    nodes[k].1 = true;
                }
                work.charge(a.abs_diff(c))?;
                nodes[a.min(c) + 1..=a.max(c)].reverse();
                i = prev;
                skipped = 0;
                continue;
            }
            nodes[i].1 = false;
        }
        i = next;
        skipped += 1;
    }
    work.charge(n)?;
    close(nodes.into_iter().map(|(p, _)| p).collect(), work)
}

fn argument(c: &mut Construction, text: &str, kind: Kind) -> Result<Id, String> {
    let id = if let Some(id) = c.id(text) {
        id
    } else {
        let label = if kind == Kind::List {
            format!("\0graphlist{}", c.len())
        } else {
            format!("\0graph{}", c.len())
        };
        match kind {
            Kind::List => {
                if let Some(result) = graph::inline_segments(c, &label, text) {
                    result?
                } else if crate::expr::argument_object(c, text, &label, kind) {
                    c.id(&label).expect("list argument was added")
                } else {
                    return Err(format!("expected a list, got `{text}`"));
                }
            }
            Kind::Point | Kind::Number | Kind::Boolean => {
                crate::expr::define(c, &label, text, false)?
            }
            _ => unreachable!(),
        }
    };
    if c.kind(id) != kind {
        return Err(format!("`{text}` is a {}, expected a {kind}", c.kind(id)));
    }
    Ok(id)
}

pub(crate) fn exec(c: &mut Construction, label: &str, rhs: &str) -> Option<Result<Id, String>> {
    if let Some(result) = graph::inline_segments(c, label, rhs) {
        return Some(result);
    }
    let (name, args) = split_call(rhs)?;
    if name == "ShortestDistance" {
        return Some(add(c, label, name, &args, None));
    }
    let graph = match name {
        "ConvexHull" | "Hull" => PointGraph::Hull,
        "DelauneyTriangulation" | "DelaunayTriangulation" => PointGraph::Delaunay,
        "Voronoi" => PointGraph::Voronoi,
        "MinimumSpanningTree" => PointGraph::SpanningTree,
        "TravelingSalesman" => PointGraph::Tour,
        "Perimeter" | "Circumference" => {
            let [arg] = args.as_slice() else {
                return None;
            };
            if split_call(arg).is_none_or(|(call, _)| call != "ShortestDistance") {
                return None;
            }
            let start = c.checkpoint();
            let helper = format!("\0perimeter{}", c.len());
            let result = exec(c, &helper, arg)
                .expect("checked command")
                .and_then(|locus| {
                    c.add(label, Def::Measure(MeasureDef::LocusPerimeter(locus)))
                        .map_err(|e| e.to_string())
                });
            if result.is_err() {
                c.discard_since(start);
            }
            return Some(result);
        }
        _ => return None,
    };
    Some(add(c, label, name, &args, Some(graph)))
}

fn add(
    c: &mut Construction,
    label: &str,
    name: &str,
    args: &[&str],
    graph: Option<PointGraph>,
) -> Result<Id, String> {
    let start = c.checkpoint();
    let result = (|| {
        let arity = || format!("`{name}` does not take {} argument(s)", args.len());
        let def = match (graph, args) {
            (None, [list, start, end, weighted]) => DiscreteDef::ShortestPath {
                list: argument(c, list, Kind::List)?,
                start: argument(c, start, Kind::Point)?,
                end: argument(c, end, Kind::Point)?,
                weighted: argument(c, weighted, Kind::Boolean)?,
            },
            (None, _) => return Err(arity()),
            (Some(graph), [list, precision]) if name == "Hull" => {
                argument(c, precision, Kind::Number)?;
                DiscreteDef::Points {
                    graph,
                    list: argument(c, list, Kind::List)?,
                }
            }
            (Some(_), _) if name == "Hull" => return Err(arity()),
            (Some(graph), [list]) => DiscreteDef::Points {
                graph,
                list: argument(c, list, Kind::List)?,
            },
            (Some(graph), [first, ..]) => {
                let first_is_number = c.id(first).map_or_else(
                    || crate::expr::constant_number(c, first).is_some(),
                    |id| c.kind(id) == Kind::Number,
                );
                let kind = if first_is_number
                    && matches!(
                        graph,
                        PointGraph::Voronoi | PointGraph::SpanningTree | PointGraph::Tour
                    ) {
                    Kind::Number
                } else {
                    Kind::Point
                };
                let ids = args
                    .iter()
                    .map(|arg| argument(c, arg, kind))
                    .collect::<Result<Vec<_>, _>>()?;
                let list = crate::expr::list_of_objects(c, &format!("\0graph{}", c.len()), ids)?;
                DiscreteDef::Points { graph, list }
            }
            (Some(_), []) => return Err(arity()),
        };
        if let DiscreteDef::Points { graph, list } = def {
            if let Some(items) = c.elements(list) {
                let points_only = match graph {
                    PointGraph::SpanningTree => items.len() > 1,
                    PointGraph::Tour => items.len() >= 3,
                    _ => false,
                };
                if points_only
                    && items.iter().any(|item| {
                        !matches!(item, Element::Value(Kind::Point, value)
                        if graph != PointGraph::Tour || matches!(value, Value::Point { .. }))
                    })
                {
                    return Err(format!("`{name}` needs points"));
                }
            }
        }
        c.add(label, Def::Discrete(def)).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        c.discard_since(start);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearly_collinear_hull_finishes_within_a_small_work_budget() {
        let points = vec![
            (0.01999999999999999, 0.21),
            (-0.01999999999999999, 0.13999999999999999),
            (0.30000000000000004, 0.7),
        ];
        // Roundoff can retain a site in both passes. Allow both sides and closure.
        let max_vertices = 2 * points.len() - 1;
        let locus = hull(points, &mut Budget(40)).expect("three sites must fit the budget");
        assert!((2..=max_vertices).contains(&locus.len()));
    }

    #[test]
    fn sampling_a_locus_restores_the_spanning_tree_at_one_site() {
        let mut c = crate::script::load(
            "n = Slider(1, 3)\n\
             A = (0, 0)\n\
             B = (2, 0)\n\
             C = (4, 0)\n\
             p = First({A, B, C}, n)\n\
             g = MinimumSpanningTree(p)\n\
             s = Perimeter(g)\n\
             Q = (n, s)",
        )
        .unwrap();
        let n = c.id("n").unwrap();
        let g = c.id("g").unwrap();
        c.set_number(n, 2.0).unwrap();
        c.set_number(n, 1.0).unwrap();
        let before = c.locus(g).unwrap().points.clone();

        let h = crate::script::exec(&mut c, "h = Locus(Q, n)")
            .unwrap()
            .unwrap();
        assert!(c.locus(h).unwrap().defined);
        assert_eq!(c.locus(g).unwrap().points, before);
    }

    #[test]
    fn discrete_algorithms_charge_their_searches_after_reading_inputs() {
        let c = crate::script::load(
            "p = Sequence((i, i^2), i, 1, 20)\n\
             g = ConvexHull(p)\n\
             d = DelauneyTriangulation(p)\n\
             v = Voronoi(p)\n\
             s = MinimumSpanningTree(p)\n\
             t = TravelingSalesman(p)",
        )
        .unwrap();
        for label in ["g", "d", "v", "s", "t"] {
            let id = c.id(label).unwrap();
            let Def::Discrete(def) = c.def(id) else {
                panic!("expected a discrete locus for {label}");
            };
            assert!(c.locus(id).unwrap().defined, "{label}");
            assert!(def.compute(&c, &mut Budget(30)).is_none(), "{label}");
        }
    }
}
