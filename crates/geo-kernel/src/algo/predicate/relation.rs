// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/RelationNumerical.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::{concurrent, concyclic, equal, is_equal, parallel, perpendicular, point, raw_line};
use crate::algo;
use crate::algo::conic_parts::ConicPartDef;
use crate::algo::path::PathDef;
use crate::expr::{Element, List};
use crate::kernel::{Construction, Def, Id, Kind, Value};
use crate::tol::STANDARD_PRECISION;

/// One numerical finding, with the reference's name for the check when it
/// defines one and an English description for the algebra view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    pub check: Option<&'static str>,
    pub outcome: Option<bool>,
    pub message: &'static str,
}

fn finding(check: Option<&'static str>, outcome: bool, message: &'static str) -> Report {
    Report {
        check,
        outcome: Some(outcome),
        message,
    }
}

fn equality(check: Option<&'static str>, equal: bool) -> Report {
    finding(
        check,
        equal,
        if equal {
            "The objects are equal."
        } else {
            "The objects differ."
        },
    )
}

fn intersection(intersect: bool) -> Report {
    finding(
        None,
        intersect,
        if intersect {
            "The paths intersect."
        } else {
            "The paths are disjoint."
        },
    )
}

fn congruence(check: Option<&'static str>, congruent: bool) -> Report {
    finding(
        check,
        congruent,
        if congruent {
            "The segments have the same length."
        } else {
            "The segments have different lengths."
        },
    )
}

pub(crate) fn numerical(c: &Construction, ids: &[Id], stop: impl Fn(u64) -> bool) -> Vec<Report> {
    if ids.iter().any(|&id| !crate::expr::object_defined(c, id)) {
        return vec![Report {
            check: None,
            outcome: None,
            message: "An input is undefined.",
        }];
    }
    let carriers = |ids: &[Id]| ids.iter().all(|&id| super::LINE_LIKE.contains(&c.kind(id)));
    let all = |kind| ids.iter().all(|&id| c.kind(id) == kind);
    match *ids {
        [a, b] => pair(c, a, b, &stop),
        [a, b, d] if carriers(ids) => {
            let report = if [b, d].iter().all(|&id| equal(c, a, id)) {
                equality(None, true)
            } else if all(Kind::Segment)
                && [b, d]
                    .iter()
                    .all(|&id| super::congruent(c, a, id) == Some(true))
            {
                congruence(None, true)
            } else {
                let [a, b, d] = [a, b, d].map(|id| raw_line(c, id));
                if parallel(a, b) && parallel(a, d) {
                    finding(None, true, "The lines are parallel.")
                } else if concurrent(a, b, d) {
                    finding(
                        Some("AreConcurrent"),
                        true,
                        "The lines share an intersection.",
                    )
                } else {
                    equality(None, false)
                }
            };
            vec![report]
        }
        [a, b, d, ..] if all(Kind::Point) => {
            let collinear = |d| {
                let [a, b, d] = [a, b, d].map(|id| point(c, id));
                super::collinear(a, b, d)
            };
            vec![if ids[1..].iter().all(|&id| equal(c, a, id)) {
                equality(None, true)
            } else if ids[2..].iter().all(|&id| collinear(id)) {
                finding(
                    (ids.len() == 3).then_some("AreCollinear"),
                    true,
                    "The points lie on one line.",
                )
            } else if ids.len() == 4 && concyclic(c, [a, b, d, ids[3]]) {
                finding(Some("AreConcyclic"), true, "The points lie on one circle.")
            } else {
                equality(None, false)
            }]
        }
        _ => vec![unavailable()],
    }
}

fn unavailable() -> Report {
    Report {
        check: None,
        outcome: None,
        message: "There is no numerical comparison for these objects.",
    }
}

fn pair(c: &Construction, a: Id, b: Id, stop: &impl Fn(u64) -> bool) -> Vec<Report> {
    let (first, second) = (c.kind(a), c.kind(b));
    let same = || equal(c, a, b);
    let carrier = |kind| super::LINE_LIKE.contains(&kind);
    let conic = |kind| algo::CONIC_OBJECTS.contains(&kind);
    let area = |v| match v {
        Value::Polygon { area } | Value::Sector { area } => area.abs(),
        _ => f64::NAN,
    };
    let report = match (first, second) {
        (x, y) if x.is_number() && y.is_number() => {
            let magnitude = |id| crate::expr::relation_number(c, id);
            equality(Some("AreEqual"), is_equal(magnitude(a), magnitude(b)))
        }
        (Kind::Point, Kind::Point) => equality(Some("AreEqual"), same()),
        (Kind::Vector, Kind::Vector) => {
            let (Value::Vector { x, y }, Value::Vector { x: u, y: v }) = (c.value(a), c.value(b))
            else {
                return vec![unavailable()];
            };
            if same() {
                equality(Some("AreEqual"), true)
            } else {
                finding(
                    Some("AreEqual"),
                    false,
                    if is_equal(x * v, y * u) {
                        "The vectors are linearly dependent."
                    } else {
                        "The vectors are linearly independent."
                    },
                )
            }
        }
        (Kind::Segment, Kind::Segment) => {
            let length_equal = super::congruent(c, a, b) == Some(true);
            let mut reports = Vec::new();
            if length_equal && same() {
                reports.push(equality(None, true));
            }
            reports.push(congruence(
                length_equal.then_some("AreCongruent"),
                length_equal,
            ));
            let (g, h) = (raw_line(c, a), raw_line(c, b));
            if parallel(g, h) {
                reports.push(finding(
                    Some("AreParallel"),
                    true,
                    "The segments are parallel.",
                ));
            }
            if perpendicular(g, h) {
                reports.push(finding(
                    Some("ArePerpendicular"),
                    true,
                    "The segments are perpendicular.",
                ));
            }
            return reports;
        }
        (x, y) if carrier(x) && carrier(y) => {
            let (g, h) = (raw_line(c, a), raw_line(c, b));
            if same() {
                equality(Some("AreEqual"), true)
            } else if parallel(g, h) {
                finding(Some("AreParallel"), true, "The lines are parallel.")
            } else if perpendicular(g, h) {
                finding(
                    Some("ArePerpendicular"),
                    true,
                    "The lines are perpendicular.",
                )
            } else {
                let det = g.0 * h.1 - g.1 * h.0;
                let p = ((g.1 * h.2 - g.2 * h.1) / det, (g.2 * h.0 - g.0 * h.2) / det);
                intersection(
                    on_path(c, a, p, stop).unwrap_or(false)
                        && on_path(c, b, p, stop).unwrap_or(false),
                )
            }
        }
        (Kind::Polygon, Kind::Polygon) => {
            let same = is_equal(area(c.value(a)), area(c.value(b)));
            finding(
                Some("AreEqual"),
                same,
                if same {
                    "The polygons have the same area."
                } else {
                    "The polygons have different areas."
                },
            )
        }
        (Kind::Point, _) | (_, Kind::Point) => {
            let (p, path) = if first == Kind::Point { (a, b) } else { (b, a) };
            let kind = c.kind(path);
            let attached = matches!(c.def(p), Def::Path(PathDef::On { path: source } | PathDef::OnParam { path: source, .. }) if path == source);
            let on = if attached
                && (carrier(kind) || conic(kind) || matches!(kind, Kind::Polygon | Kind::PolyLine))
            {
                Some(true)
            } else {
                on_path(c, path, point(c, p), stop)
            };
            let Some(on) = on else {
                return vec![unavailable()];
            };
            finding(
                (kind != Kind::Polygon).then_some("IsOnPath"),
                on,
                if on {
                    "The point lies on the path."
                } else {
                    "The point lies outside the path."
                },
            )
        }
        (x, y) if carrier(x) && conic(y) || conic(x) && carrier(y) => {
            let (line, conic) = if carrier(first) { (a, b) } else { (b, a) };
            let Some(shape) = algo::conic_object(c, conic) else {
                return vec![unavailable()];
            };
            let limited = c.kind(line) != Kind::Line || c.arc(conic).is_some();
            if !limited
                && matches!(c.def(line), Def::ConicPart(ConicPartDef::Asymptote { conic: source, .. }) if source == conic)
            {
                return vec![Report {
                    check: None,
                    outcome: None,
                    message: "The line is an asymptote of the conic.",
                }];
            }
            let (a, b, d) = raw_line(c, line);
            let intersections = algo::intersect::helper_line_conic_points([a, b, d], &shape);
            let count = intersections.iter().filter(|point| point.is_some()).count();
            if limited {
                intersection(intersections.into_iter().flatten().any(|p| {
                    on_path(c, line, p, stop).unwrap_or(false)
                        && on_path(c, conic, p, stop).unwrap_or(false)
                }))
            } else if super::tangent(c, line, conic) {
                finding(Some("IsTangent"), true, "The line is tangent to the conic.")
            } else {
                let message = match count {
                    0 => "The line and conic are disjoint.",
                    1 => "The line meets the conic once.",
                    _ => "The line crosses the conic twice.",
                };
                finding(None, false, message)
            }
        }
        (x, y) if conic(x) && conic(y) => {
            return conic_pair(c, a, b);
        }
        (Kind::Function, Kind::Function) => {
            let a_signature = crate::expr::function_signature(c, a);
            let b_signature = crate::expr::function_signature(c, b);
            if a == b {
                equality(None, true)
            } else if a_signature != b_signature {
                equality(None, false)
            } else if a_signature.is_some_and(|signature| signature.boolean) {
                equality(None, c.boolean_functions_equal(a, b))
            } else {
                let work = crate::expr::IntersectionWork::new();
                let differ = [0.31, 10.89].into_iter().any(|x| {
                    let (u, v) = (work.sample(c, a, x), work.sample(c, b, x));
                    !(is_equal(u, v) || u.is_nan() && v.is_nan())
                });
                if differ {
                    equality(None, false)
                } else if let Some(equal) = polynomial_equal(c, a, b, &work) {
                    equality(None, equal)
                } else {
                    unavailable()
                }
            }
        }
        (Kind::List, Kind::List) => equality(
            None,
            match (c.elements(a), c.elements(b)) {
                (Some(a), Some(b)) => equal_lists(a, b, stop),
                _ => false,
            },
        ),
        _ => unavailable(),
    };
    vec![report]
}

fn polynomial_equal(
    c: &Construction,
    a: Id,
    b: Id,
    work: &crate::expr::IntersectionWork,
) -> Option<bool> {
    let (a, d) = work.fraction(c, a)?;
    let (b, e) = work.fraction(c, b)?;
    let (&[d], &[e]) = (d.as_slice(), e.as_slice()) else {
        return None;
    };
    if !d.is_finite() || !e.is_finite() || d == 0.0 || e == 0.0 {
        return None;
    }
    let len = a.len().max(b.len());
    work.charge(len as u64).then(|| {
        (0..len).all(|i| {
            is_equal(
                a.get(i).copied().unwrap_or(0.0) / d,
                b.get(i).copied().unwrap_or(0.0) / e,
            )
        })
    })
}

fn conic_pair(c: &Construction, a: Id, b: Id) -> Vec<Report> {
    use algo::intersect::conics::{on_part, Intersection};
    let (Some(first), Some(second)) = (algo::conic_object(c, a), algo::conic_object(c, b)) else {
        return vec![unavailable()];
    };
    let equal = super::equal_matrix(&first.matrix, &second.matrix);
    let (part_a, part_b) = (c.arc(a), c.arc(b));
    if let (Some(a), Some(b)) = (&part_a, &part_b) {
        let same_kind = a.sector == b.sector;
        let mut reports = vec![equality(
            None,
            equal && same_kind && is_equal(a.start, b.start) && is_equal(a.extent, b.extent),
        )];
        if same_kind {
            let same = if a.sector {
                is_equal(a.area(), b.area())
            } else {
                is_equal(a.arc_length(), b.arc_length())
            };
            let message = match (a.sector, same) {
                (true, true) => "The sectors have the same area.",
                (true, false) => "The sectors have different areas.",
                (false, true) => "The arcs have the same length.",
                (false, false) => "The arcs have different lengths.",
            };
            reports.push(finding(None, same, message));
        }
        return reports;
    }
    if equal {
        return vec![equality(None, true)];
    }
    let points = Intersection::default().update(&first, &second, false, |point| {
        part_a.as_ref().is_none_or(|p| on_part(p, point))
            && part_b.as_ref().is_none_or(|p| on_part(p, point))
    });
    let message = if algo::intersect::points_equal(points[0], points[1]) {
        "The conics touch."
    } else if points.iter().any(Option::is_some) {
        "The conics intersect."
    } else {
        "The conics are disjoint."
    };
    // The numerical report is affirmative for this classification,
    // including the disjoint case.
    vec![finding(None, true, message)]
}

fn equal_lists(a: List<'_>, b: List<'_>, stop: &impl Fn(u64) -> bool) -> bool {
    if a.len() != b.len() || stop(a.len() as u64) {
        return false;
    }
    a.iter().zip(b.iter()).all(|(a, b)| match (a, b) {
        (Element::List(a), Element::List(b)) => equal_lists(a, b, stop),
        (Element::Value(a, v), Element::Value(b, w)) if a == b => match (v, w) {
            (Value::Number(a), Value::Number(b)) => is_equal(a, b),
            (
                Value::Point { x, y } | Value::Vector { x, y },
                Value::Point { x: u, y: v } | Value::Vector { x: u, y: v },
            ) => is_equal(x, u) && is_equal(y, v),
            _ => v == w,
        },
        _ => false,
    })
}

fn on_path(c: &Construction, id: Id, p: (f64, f64), stop: &impl Fn(u64) -> bool) -> Option<bool> {
    let on_line = |(a, b, d)| algo::intersect::on_full_line([a, b, d], p, STANDARD_PRECISION);
    let on_segment = |start, end| {
        let g = algo::join(start, end);
        on_line(g)
            && (-STANDARD_PRECISION..=1.0 + STANDARD_PRECISION)
                .contains(&algo::path::line_parameter_at(g, start, p))
    };
    Some(match c.value(id) {
        Value::Line { a, b, c } => on_line((a, b, c)),
        Value::Segment { x1, y1, x2, y2 } => on_segment((x1, y1), (x2, y2)),
        Value::Ray {
            x: s,
            y: t,
            a,
            b,
            c,
        } => {
            let g = (a, b, c);
            on_line(g) && algo::path::line_parameter_at(g, (s, t), p) >= -STANDARD_PRECISION
        }
        Value::Circle { .. } | Value::Conic { .. } => {
            c.conic(id)?.on_full_conic(p, STANDARD_PRECISION)
        }
        Value::Arc { .. } | Value::Sector { .. } => {
            algo::intersect::conics::on_part_with_precision(&c.arc(id)?, p, STANDARD_PRECISION)
        }
        Value::Vector { x: vx, y: vy } => {
            let start = algo::vector::start(c, id)?;
            on_segment(start, (start.0 + vx, start.1 + vy))
        }
        Value::Function => is_equal(c.function_value(id, p.0)?, p.1),
        Value::Polygon { .. } | Value::PolyLine { .. } => {
            let mut vertices = c.vertices(id);
            let first = vertices.next()??;
            let mut from = first;
            for to in vertices {
                if stop(1) {
                    return None;
                }
                let to = to?;
                if on_segment(from, to) {
                    return Some(true);
                }
                from = to;
            }
            c.kind(id) == Kind::Polygon && on_segment(from, first)
        }
        _ => return None,
    })
}
