//! The comparison rules of `golden/SCHEMA.md`.

use crate::model::{ConicParts, Expected, ExpectedData, SAMPLE_XS};
use geo_kernel::algo::stroke::StrokeData;
use geo_kernel::expr::{Element, List};
use geo_kernel::{ArcData, ConicData, Construction, Id, Kind, LocusData, Value};

/// Result of a successful comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    /// Not a line, or an undefined line. Rays must agree in sign, so they
    /// are not counted.
    NotApplicable,
    /// A line whose sign agrees with GeoGebra's.
    Agrees,
    /// A line that agrees only up to a sign flip (counted, not a failure).
    Flipped,
}

pub const TOLERANCE: f64 = 1e-9;

/// Finite values agree within `1e-9 * max(1, |p|, |q|)`.
/// Infinite values agree when their signs match.
pub fn numbers_agree(p: f64, q: f64) -> bool {
    p == q
        || (p.is_finite()
            && q.is_finite()
            && (p - q).abs() <= TOLERANCE * 1.0_f64.max(p.abs()).max(q.abs()))
}

fn all_agree(pairs: &[(f64, f64)]) -> bool {
    pairs.iter().all(|&(p, q)| numbers_agree(p, q))
}

fn samples_agree(want: Option<f64>, got: Option<f64>) -> bool {
    match (want, got) {
        (Some(p), Some(q)) => numbers_agree(p, q),
        (None, got) => got.is_none_or(f64::is_nan),
        _ => false,
    }
}

/// A recorded coordinate against the kernel's: NaN, a recorded `null` from
/// schema 4, agrees with NaN only; every other value by [`numbers_agree`].
fn coordinate_agrees(want: f64, got: f64) -> bool {
    samples_agree((!want.is_nan()).then_some(want), Some(got))
}

pub(crate) fn implicit_matrices_agree(expected: &[Vec<f64>], matrix: &[Vec<f64>]) -> bool {
    expected.len() == matrix.len()
        && expected
            .iter()
            .zip(matrix)
            .all(|(a, b)| a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| numbers_agree(x, y)))
}

/// Scales `(a, b, c)` so that `(a, b)` has length 1. `None` for a zero `(a, b)`.
fn normalise(a: f64, b: f64, c: f64) -> Option<(f64, f64, f64)> {
    let n = a.hypot(b);
    (n > 0.0 && n.is_finite()).then(|| (a / n, b / n, c / n))
}

/// Whether a defined line or ray holds GeoGebra's raw coefficients, scale
/// and sign included: every component agrees within `1e-9` times the largest
/// component magnitude of the two triples. `None` when either side is not a
/// defined line or ray. This is reported next to the comparison and never
/// fails it.
pub fn line_scale_agrees(expected: &Expected, actual: &Value) -> Option<bool> {
    let ((a, b, c), (x, y, z)) = match (&expected.data, *actual) {
        (Some(ExpectedData::Line { a, b, c }), Value::Line { a: x, b: y, c: z })
        | (
            Some(ExpectedData::Ray { a, b, c, .. }),
            Value::Ray {
                a: x, b: y, c: z, ..
            },
        ) => ((a, b, c), (x, y, z)),
        _ => return None,
    };
    let scale = [*a, *b, *c, x, y, z]
        .into_iter()
        .filter(|v| v.is_finite())
        .fold(0.0_f64, |m, v| m.max(v.abs()));
    let close = |p: f64, q: f64| p == q || (p - q).abs() <= TOLERANCE * scale;
    Some(close(*a, x) && close(*b, y) && close(*c, z))
}

pub(crate) fn compare_pie(
    expected: &Expected,
    pie: Option<&geo_kernel::algo::charts::Pie>,
) -> Result<Sign, String> {
    match (&expected.data, pie) {
        (None, None) => Ok(Sign::NotApplicable),
        (
            Some(ExpectedData::Pie {
                center,
                radius,
                fractions,
            }),
            Some(pie),
        ) => {
            if !all_agree(&[
                (center.0, pie.center.0),
                (center.1, pie.center.1),
                (*radius, pie.radius),
            ]) {
                return Err(format!(
                    "pie center or radius differs: {:?}",
                    (pie.center, pie.radius)
                ));
            }
            if fractions.len() != pie.fractions.len()
                || !fractions
                    .iter()
                    .zip(&pie.fractions)
                    .all(|(want, &got)| samples_agree(*want, Some(got)))
            {
                return Err(format!("pie fractions differ: {:?}", pie.fractions));
            }
            Ok(Sign::NotApplicable)
        }
        _ => Err("pie definition differs".into()),
    }
}

/// Compares one recorded object with what the kernel holds, for objects
/// without vertices and other than functions (see [`compare_object`]).
pub fn compare(expected: &Expected, kind: Kind, actual: &Value) -> Result<Sign, String> {
    compare_sampled(expected, kind, actual, &[], None)
}

/// Compares a recorded list with the kernel's elements (`None` when the
/// kernel's list is undefined): the same length, and each element compared
/// like an object, a nested list in the same way.
pub fn compare_list(
    c: &Construction,
    expected: &Expected,
    kind: Kind,
    list: Option<List>,
) -> Result<Sign, String> {
    if expected.kind != kind {
        return Err(format!(
            "type mismatch: expected {}, got {kind}",
            expected.kind
        ));
    }
    let (want, got) = match (&expected.data, list) {
        (None, None) => return Ok(Sign::NotApplicable),
        (None, Some(_)) => return Err("expected undefined, kernel has a value".into()),
        (Some(ExpectedData::List(want)), Some(got)) => (want, got),
        (Some(_), _) => return Err("expected a value, kernel is undefined".into()),
    };
    if want.len() != got.len() {
        return Err(format!(
            "expected {} elements, kernel has {}",
            want.len(),
            got.len()
        ));
    }
    for (k, (w, g)) in want.iter().zip(got.iter()).enumerate() {
        match g {
            Element::List(inner) => compare_list(c, w, Kind::List, Some(inner)),
            Element::Text(s) => match &w.data {
                Some(ExpectedData::Text(text)) if text == s => Ok(Sign::NotApplicable),
                _ => Err(format!("expected {:?}, got text {s:?}", w.data)),
            },
            Element::Value(Kind::Function, value) => {
                let sample = |x| got.function_sample_at(c, k, x);
                compare_sampled(w, Kind::Function, &value, &[], Some(&sample))
            }
            Element::Value(kind, value) => compare(w, kind, &value),
        }
        .map_err(|why| format!("element {}: {why}", k + 1))?;
    }
    Ok(Sign::NotApplicable)
}

/// Compares a snapshot of one object with the live kernel, including data
/// stored outside the fixed-size value.
pub fn compare_construction_object(
    expected: &Expected,
    c: &Construction,
    id: Id,
) -> Result<Sign, String> {
    let kind = c.kind(id);
    let value = c.value(id);
    match kind {
        Kind::Arc | Kind::Sector => compare_arc(expected, kind, &value, c.arc(id).as_ref()),
        Kind::Locus => compare_locus(expected, kind, &value, c.locus(id)),
        Kind::PenStroke => compare_stroke(expected, kind, &value, c.stroke(id)),
        Kind::Conic => compare_conic(expected, kind, &value, c.conic(id).as_ref()).0,
        Kind::List => compare_list(c, expected, kind, c.elements(id)),
        Kind::Text | Kind::FunctionNVar | Kind::Curve | Kind::ImplicitCurve => {
            compare_extra(c, id, expected)
        }
        Kind::PieChart if expected.kind == kind => {
            compare_pie(expected, c.chart(id).and_then(|data| data.pie.as_ref()))
        }
        _ => compare_sampled(
            expected,
            kind,
            &value,
            &c.vertices(id).collect::<Vec<_>>(),
            Some(&|x| c.function_value(id, x)),
        ),
    }
}

/// Compares results whose data are held outside the fixed-size value.
pub(crate) fn compare_extra(c: &Construction, id: Id, expected: &Expected) -> Result<Sign, String> {
    let kind = c.kind(id);
    if expected.kind != kind || expected.data.is_none() || !c.value(id).is_defined() {
        return compare(expected, kind, &c.value(id));
    }
    match &expected.data {
        Some(ExpectedData::Text(text)) if c.text(id) == Some(text.as_str()) => (),
        Some(ExpectedData::Text(text)) => {
            return Err(format!("expected text {text:?}, got {:?}", c.text(id)));
        }
        Some(ExpectedData::Relation(expected)) => {
            let classify = expected.iter().any(|report| report.contains('/'));
            let mut actual = c
                .relation_reports(id)
                .ok_or("missing relation reports")?
                .into_iter()
                .map(|r| {
                    let outcome = match r.outcome {
                        Some(true) => "true",
                        Some(false) => "false",
                        None => "null",
                    };
                    let kind = if classify {
                        match r.message {
                            "The conics touch." => "/touch",
                            "The conics intersect." => "/intersect",
                            "The conics are disjoint." => "/disjoint",
                            _ => "",
                        }
                    } else {
                        ""
                    };
                    format!("{}={outcome}{kind}", r.check.unwrap_or("null"))
                })
                .collect::<Vec<_>>();
            actual.sort_unstable();
            if *expected != actual {
                return Err(format!("expected {expected:?}, got {actual:?}"));
            }
        }
        Some(ExpectedData::Implicit {
            coefficients,
            equation,
            input_form,
        }) => {
            let matrix = c
                .implicit_coefficients(id)
                .ok_or("missing implicit coefficients")?;
            if !implicit_matrices_agree(coefficients, matrix) {
                return Err(format!(
                    "expected coefficients {coefficients:?}, got {matrix:?}"
                ));
            }
            let user = c.equation_form(id) == Some(&geo_kernel::kernel::EquationForm::User);
            if user != *input_form {
                return Err(format!("expected input form {input_form}, got {user}"));
            }
            let actual = c
                .formatted_equation(id)
                .ok_or("missing implicit equation text")?;
            let compact = |s: &str| {
                let mut normalized = String::new();
                let mut previous_super = false;
                for ch in s.chars() {
                    if let Some(i) = "⁰¹²³⁴⁵⁶⁷⁸⁹".chars().position(|digit| digit == ch)
                    {
                        if !previous_super {
                            normalized.push('^');
                        }
                        normalized.push(char::from(b'0' + i as u8));
                        previous_super = true;
                    } else {
                        previous_super = false;
                        if !ch.is_whitespace() && !matches!(ch, '(' | ')' | '*' | '·') {
                            normalized.push(ch);
                        }
                    }
                }
                normalized
            };
            if compact(&actual) != compact(equation) {
                return Err(format!("expected equation {equation:?}, got {actual:?}"));
            }
        }
        Some(ExpectedData::Curve { samples, .. }) => {
            let result = compare(expected, kind, &c.value(id))?;
            for &(t, want) in samples {
                let got = c.curve_sample(id, t);
                let agrees = match (want, got) {
                    (Some((x, y)), Some((a, b))) => {
                        coordinate_agrees(x, a) && coordinate_agrees(y, b)
                    }
                    (Some((x, y)), None) => x.is_nan() && y.is_nan(),
                    (None, got) => got.is_none_or(|(x, y)| {
                        !x.is_infinite() && !y.is_infinite() && (x.is_nan() || y.is_nan())
                    }),
                };
                if !agrees {
                    return Err(format!("curve at {t}: expected {want:?}, got {got:?}"));
                }
            }
            return Ok(result);
        }
        Some(ExpectedData::FunctionNVar(samples)) => {
            for &(x, y, want) in samples {
                let got = c.function_value_xy(id, x, y);
                if !samples_agree(want, got) {
                    return Err(format!("at ({x}, {y}): expected {want:?}, got {got:?}"));
                }
            }
        }
        _ => return Err(format!("unexpected fixture for {kind}")),
    }
    Ok(Sign::NotApplicable)
}

/// Whether two lists of vertices agree: same length, and each vertex
/// undefined on both sides or agreeing in both coordinates.
fn vertices_agree(expected: &[Option<(f64, f64)>], actual: &[Option<(f64, f64)>]) -> bool {
    expected.len() == actual.len()
        && expected.iter().zip(actual).all(|(e, a)| match (e, a) {
            (None, None) => true,
            (Some((x, y)), Some((ax, ay))) => all_agree(&[(*x, *ax), (*y, *ay)]),
            _ => false,
        })
}

/// The kernel's value of a function object at `x` (`Construction::function_value`).
pub type Sampler<'a> = &'a dyn Fn(f64) -> Option<f64>;

/// Compares one recorded object with what the kernel holds.
/// `kind` is the kernel's kind of the object (valid even when undefined);
/// `vertices` are the kernel's vertices of a polygon or polyline
/// (`Construction::vertices`), empty for other objects. A defined function
/// is never a match here; see [`compare_sampled`].
pub fn compare_object(
    expected: &Expected,
    kind: Kind,
    actual: &Value,
    vertices: &[Option<(f64, f64)>],
) -> Result<Sign, String> {
    compare_sampled(expected, kind, actual, vertices, None)
}

/// [`compare_object`], with `sample` giving the kernel's values of a
/// function. A defined function needs `sample`: each sample is compared like
/// a number, and a `null` sample must meet a value that is not finite, and
/// the other way round. Without `sample` a defined function is an error,
/// never a match.
pub fn compare_sampled(
    expected: &Expected,
    kind: Kind,
    actual: &Value,
    vertices: &[Option<(f64, f64)>],
    sample: Option<Sampler>,
) -> Result<Sign, String> {
    if expected.kind != kind && !(expected.legacy_angle_number && kind == Kind::Angle) {
        return Err(format!(
            "type mismatch: expected {}, got {kind}",
            expected.kind
        ));
    }
    let data = match (&expected.data, actual) {
        (None, Value::Undefined) => return Ok(Sign::NotApplicable),
        (None, _) => return Err("expected undefined, kernel has a value".into()),
        (Some(_), Value::Undefined) => return Err("expected a value, kernel is undefined".into()),
        (Some(d), _) => d,
    };
    let ok = |b: bool| {
        if b {
            Ok(Sign::NotApplicable)
        } else {
            Err("values differ".to_string())
        }
    };
    match (data, *actual) {
        (ExpectedData::Point { x, y }, Value::Point { x: ax, y: ay }) => {
            ok(all_agree(&[(*x, ax), (*y, ay)]))
        }
        (ExpectedData::Number(v), Value::Number(av)) => ok(numbers_agree(*v, av)),
        (ExpectedData::Curve { start, end, .. }, Value::Curve { start: a, end: b }) => {
            ok(numbers_agree(*start, a) && numbers_agree(*end, b))
        }
        (ExpectedData::Bool(b), Value::Bool(ab)) => ok(*b == ab),
        (ExpectedData::Image, Value::Image) => ok(true),
        (
            ExpectedData::Circle { cx, cy, r },
            Value::Circle {
                cx: ax,
                cy: ay,
                r: ar,
            },
        ) => ok(all_agree(&[(*cx, ax), (*cy, ay), (*r, ar)])),
        (
            ExpectedData::Segment { x1, y1, x2, y2 },
            Value::Segment {
                x1: a1,
                y1: b1,
                x2: a2,
                y2: b2,
            },
        ) => ok(all_agree(&[(*x1, a1), (*y1, b1), (*x2, a2), (*y2, b2)])),
        (
            ExpectedData::Line { a, b, c },
            Value::Line {
                a: aa,
                b: ab,
                c: ac,
            },
        ) => {
            let e = normalise(*a, *b, *c).ok_or("expected line has a zero direction")?;
            let k = normalise(aa, ab, ac).ok_or("kernel line has a zero direction")?;
            let sign = if e.0 * k.0 + e.1 * k.1 >= 0.0 {
                1.0
            } else {
                -1.0
            };
            if all_agree(&[(e.0, sign * k.0), (e.1, sign * k.1), (e.2, sign * k.2)]) {
                Ok(if sign > 0.0 {
                    Sign::Agrees
                } else {
                    Sign::Flipped
                })
            } else {
                Err("line differs even up to sign".into())
            }
        }
        (ExpectedData::Vector { x, y }, Value::Vector { x: ax, y: ay }) => {
            ok(all_agree(&[(*x, ax), (*y, ay)]))
        }
        (
            ExpectedData::Ray { x, y, a, b, c },
            Value::Ray {
                x: ax,
                y: ay,
                a: aa,
                b: ab,
                c: ac,
            },
        ) => {
            if !all_agree(&[(*x, ax), (*y, ay)]) {
                return Err("ray start points differ".into());
            }
            let e = normalise(*a, *b, *c).ok_or("expected ray has a zero direction")?;
            let k = normalise(aa, ab, ac).ok_or("kernel ray has a zero direction")?;
            // A ray has a direction: no sign flip is accepted.
            if all_agree(&[(e.0, k.0), (e.1, k.1), (e.2, k.2)]) {
                Ok(Sign::NotApplicable)
            } else if all_agree(&[(e.0, -k.0), (e.1, -k.1), (e.2, -k.2)]) {
                Err("ray points the opposite way".into())
            } else {
                Err("ray lies on a different line".into())
            }
        }
        (ExpectedData::Function { samples }, Value::Function) => {
            let Some(sample) = sample else {
                return Err("a function is compared only through its samples".into());
            };
            for (&x, &want) in SAMPLE_XS.iter().zip(samples) {
                let got = sample(x);
                if !samples_agree(want, got) {
                    return Err(format!("at x = {x}: expected {want:?}, got {got:?}"));
                }
            }
            Ok(Sign::NotApplicable)
        }
        (
            ExpectedData::Polygon {
                vertices: ev,
                area,
                signed_area,
            },
            Value::Polygon { area: signed },
        ) => {
            let ev: Vec<_> = ev.iter().copied().map(Some).collect();
            if !all_agree(&[(*area, signed.abs()), (*signed_area, signed)]) {
                Err("areas differ".into())
            } else if !vertices_agree(&ev, vertices) {
                Err(format!("vertices differ: kernel has {vertices:?}"))
            } else {
                Ok(Sign::NotApplicable)
            }
        }
        (
            ExpectedData::PolyLine {
                vertices: ev,
                length,
            },
            Value::PolyLine { length: al },
        ) => {
            if !numbers_agree(*length, al) {
                Err("lengths differ".into())
            } else if !vertices_agree(ev, vertices) {
                Err(format!("vertices differ: kernel has {vertices:?}"))
            } else {
                Ok(Sign::NotApplicable)
            }
        }
        _ => Err("kernel value has a different shape than its kind".into()),
    }
}

/// How a conic's matrix compares with GeoGebra's: raw, as GeoGebra holds it
/// (this decides the comparison), and up to a common factor (counted only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MatrixAgreement {
    pub raw: bool,
    pub up_to_scale: bool,
}

/// Whether two matrices agree up to a common non-zero factor: each is
/// divided by its entry of largest magnitude (signed), then compared like
/// numbers.
fn matrices_agree_up_to_scale(e: &[f64; 6], k: &[f64; 6]) -> bool {
    let unit = |m: &[f64; 6]| {
        let big = m
            .iter()
            .copied()
            .fold(0.0_f64, |b, v| if v.abs() > b.abs() { v } else { b });
        (big != 0.0).then(|| m.map(|v| v / big))
    };
    match (unit(e), unit(k)) {
        (Some(a), Some(b)) => a.iter().zip(&b).all(|(&p, &q)| numbers_agree(p, q)),
        _ => false,
    }
}

/// Compares a recorded conic with the kernel's record `data` of the object
/// (`Construction::conic`). The type name, the raw matrix and the parts of
/// the type must agree; the lines of a degenerate conic in GeoGebra's order.
/// Also tells how the matrices compare when both are defined.
pub fn compare_conic(
    expected: &Expected,
    kind: Kind,
    actual: &Value,
    data: Option<&ConicData>,
) -> (Result<Sign, String>, Option<MatrixAgreement>) {
    if expected.kind != kind {
        return (
            Err(format!(
                "type mismatch: expected {}, got {kind}",
                expected.kind
            )),
            None,
        );
    }
    let (conic_type, matrix, parts) = match (&expected.data, actual, data) {
        (None, Value::Undefined, _) => return (Ok(Sign::NotApplicable), None),
        (None, _, _) => return (Err("expected undefined, kernel has a value".into()), None),
        (Some(_), Value::Undefined, _) | (Some(_), _, None) => {
            return (Err("expected a value, kernel is undefined".into()), None)
        }
        (Some(_), _, Some(d)) if !d.defined => {
            return (
                Err("expected a value, kernel's record is undefined".into()),
                None,
            )
        }
        (
            Some(ExpectedData::Conic {
                conic_type,
                matrix,
                parts,
            }),
            Value::Conic { .. },
            Some(_),
        ) => (conic_type, matrix, parts),
        _ => {
            return (
                Err("kernel value has a different shape than its kind".into()),
                None,
            )
        }
    };
    let d = data.expect("matched above");
    let agreement = MatrixAgreement {
        raw: matrix
            .iter()
            .zip(&d.matrix)
            .all(|(&p, &q)| numbers_agree(p, q)),
        up_to_scale: matrices_agree_up_to_scale(matrix, &d.matrix),
    };
    let result = if conic_type != d.ty.name() {
        Err(format!("conic type differs: kernel has {}", d.ty.name()))
    } else if !agreement.raw {
        Err(format!("matrix differs: kernel has {:?}", d.matrix))
    } else {
        let pt = |e: (f64, f64), k: [f64; 2]| all_agree(&[(e.0, k[0]), (e.1, k[1])]);
        let ev = |e: &[(f64, f64); 2]| pt(e[0], d.eigenvec[0]) && pt(e[1], d.eigenvec[1]);
        let ok = match parts {
            ConicParts::Central {
                midpoint,
                half_axes,
                eigenvectors,
            } => {
                pt(*midpoint, d.midpoint)
                    && all_agree(&[(half_axes.0, d.half_axes[0]), (half_axes.1, d.half_axes[1])])
                    && ev(eigenvectors)
            }
            ConicParts::Parabola {
                vertex,
                p,
                eigenvectors,
            } => pt(*vertex, d.midpoint) && numbers_agree(*p, d.p) && ev(eigenvectors),
            ConicParts::SinglePoint { point } => pt(*point, d.midpoint),
            ConicParts::Lines { lines } => lines
                .iter()
                .zip(&d.lines)
                .all(|(e, k)| all_agree(&[(e[0], k[0]), (e[1], k[1]), (e[2], k[2])])),
            ConicParts::Empty => true,
        };
        if ok {
            Ok(Sign::NotApplicable)
        } else {
            Err(format!("parts differ: kernel has {d:?}"))
        }
    };
    (result, Some(agreement))
}

/// Compares an arc or sector with the kernel's value and its record
/// (`Construction::arc`): the value, the conic type, the orientation where
/// GeoGebra records it, and for a part of a circle or ellipse its start and
/// end parameter, arc length, area and conic parts, raw.
/// Compares a locus point by point: the same number of points, each within
/// the tolerance with the same `lineTo` flag; an undefined point (NaN on the
/// kernel's side) only agrees with an undefined one.
pub fn compare_locus(
    expected: &Expected,
    kind: Kind,
    actual: &Value,
    data: Option<&LocusData>,
) -> Result<Sign, String> {
    if expected.kind != kind {
        return Err(format!(
            "type mismatch: expected {}, got {kind}",
            expected.kind
        ));
    }
    let (want, got) = match (&expected.data, actual, data) {
        (None, Value::Undefined, _) => return Ok(Sign::NotApplicable),
        (None, _, _) => return Err("expected undefined, kernel has a value".into()),
        (Some(_), Value::Undefined, _) | (Some(_), _, None) => {
            return Err("expected a value, kernel is undefined".into())
        }
        (Some(ExpectedData::Locus(want)), Value::Locus { .. }, Some(d)) => (want, d.points.iter()),
        _ => return Err("kernel value has a different shape than its kind".into()),
    };
    if want.len() != got.len() {
        return Err(format!(
            "{} points expected, kernel has {}",
            want.len(),
            got.len()
        ));
    }
    for (i, ((at, line_to), p)) in want.iter().zip(got).enumerate() {
        let same = match *at {
            // from schema 4 a NaN coordinate stands for a recorded `null`
            Some((x, y)) => coordinate_agrees(x, p.x) && coordinate_agrees(y, p.y),
            None => p.x.is_nan() && p.y.is_nan(),
        };
        if !same || *line_to != p.line_to {
            return Err(format!(
                "point {i} differs: expected {at:?} lineTo {line_to}, kernel has ({}, {}) lineTo {}",
                p.x, p.y, p.line_to
            ));
        }
    }
    Ok(Sign::NotApplicable)
}

/// Compares a pen stroke point by point: the same number of points, each
/// with the same segment code and coordinates within the tolerance; a
/// missing coordinate only agrees with a non-finite one.
pub fn compare_stroke(
    expected: &Expected,
    kind: Kind,
    actual: &Value,
    data: Option<&StrokeData>,
) -> Result<Sign, String> {
    if expected.kind != kind {
        return Err(format!(
            "type mismatch: expected {}, got {kind}",
            expected.kind
        ));
    }
    let (want, got) = match (&expected.data, actual, data) {
        (None, Value::Undefined, _) => return Ok(Sign::NotApplicable),
        (None, _, _) => return Err("expected undefined, kernel has a value".into()),
        (Some(_), Value::Undefined, _) | (Some(_), _, None) => {
            return Err("expected a value, kernel is undefined".into())
        }
        (Some(ExpectedData::Stroke(want)), Value::PenStroke { .. }, Some(d)) => {
            (want, d.points.iter())
        }
        _ => return Err("kernel value has a different shape than its kind".into()),
    };
    if want.len() != got.len() {
        return Err(format!(
            "{} points expected, kernel has {}",
            want.len(),
            got.len()
        ));
    }
    let agree = |want: Option<f64>, got: f64| match want {
        Some(w) => numbers_agree(w, got),
        None => !got.is_finite(),
    };
    for (i, (&(x, y, code), p)) in want.iter().zip(got).enumerate() {
        let got_code = if p.is_defined() { p.segment.code() } else { 0 };
        let same_y = x.is_none() || agree(y, p.y);
        if !agree(x, p.x) || !same_y || code != got_code {
            return Err(format!(
                "point {i} differs: expected ({x:?}, {y:?}) code {code}, kernel has ({}, {}) code {got_code}",
                p.x, p.y
            ));
        }
    }
    Ok(Sign::NotApplicable)
}

pub fn compare_arc(
    expected: &Expected,
    kind: Kind,
    actual: &Value,
    data: Option<&ArcData>,
) -> Result<Sign, String> {
    if expected.kind != kind {
        return Err(format!(
            "type mismatch: expected {}, got {kind}",
            expected.kind
        ));
    }
    let (value, conic_type, orientation, curve) = match (&expected.data, actual, data) {
        (None, Value::Undefined, _) => return Ok(Sign::NotApplicable),
        (None, _, _) => return Err("expected undefined, kernel has a value".into()),
        (Some(_), Value::Undefined, _) | (Some(_), _, None) => {
            return Err("expected a value, kernel is undefined".into())
        }
        (
            Some(ExpectedData::Arc {
                value,
                conic_type,
                orientation,
                curve,
            }),
            Value::Arc { .. } | Value::Sector { .. },
            Some(_),
        ) => (value, conic_type, orientation, curve),
        _ => return Err("kernel value has a different shape than its kind".into()),
    };
    let d = data.expect("matched above");
    let got = match *actual {
        Value::Arc { length } => length,
        Value::Sector { area } => area,
        _ => unreachable!("matched above"),
    };
    if !numbers_agree(*value, got) {
        return Err(format!("value differs: kernel has {got}"));
    }
    if conic_type != d.conic.ty.name() {
        return Err(format!(
            "conic type differs: kernel has {}",
            d.conic.ty.name()
        ));
    }
    if orientation.is_some_and(|o| o != d.positive) {
        return Err(format!(
            "orientation differs: kernel has positive = {}",
            d.positive
        ));
    }
    if let Some(e) = curve {
        let k = &d.conic;
        let ok = all_agree(&[
            (e.start, d.start),
            (e.end, d.end),
            (e.arc_length, d.arc_length()),
            (e.area, d.area()),
            (e.midpoint.0, k.midpoint[0]),
            (e.midpoint.1, k.midpoint[1]),
            (e.half_axes.0, k.half_axes[0]),
            (e.half_axes.1, k.half_axes[1]),
            (e.eigenvectors[0].0, k.eigenvec[0][0]),
            (e.eigenvectors[0].1, k.eigenvec[0][1]),
            (e.eigenvectors[1].0, k.eigenvec[1][0]),
            (e.eigenvectors[1].1, k.eigenvec[1][1]),
        ]);
        if !ok {
            return Err(format!("parts differ: kernel has {d:?}"));
        }
    }
    Ok(Sign::NotApplicable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_kernel::algo::conic_nd::ConicType;

    fn num(v: f64) -> Expected {
        Expected {
            legacy_angle_number: false,
            kind: Kind::Number,
            data: Some(ExpectedData::Number(v)),
        }
    }

    fn line(a: f64, b: f64, c: f64) -> Expected {
        Expected {
            legacy_angle_number: false,
            kind: Kind::Line,
            data: Some(ExpectedData::Line { a, b, c }),
        }
    }

    #[test]
    fn tolerance_boundary_is_relative_above_one() {
        let n = |q| compare(&num(1.0), Kind::Number, &Value::Number(q));
        assert!(n(1.0 + 0.9e-9).is_ok());
        assert!(n(1.0 + 1.5e-9).is_err());
        // Absolute below one.
        let small = |q| compare(&num(0.0), Kind::Number, &Value::Number(q));
        assert!(small(0.9e-9).is_ok());
        assert!(small(1.5e-9).is_err());
        // Relative for large magnitudes: 1e6 may differ by 1e-3.
        let big = |q| compare(&num(1e6), Kind::Number, &Value::Number(q));
        assert!(big(1e6 + 0.9e-3).is_ok());
        assert!(big(1e6 + 1.5e-3).is_err());
    }

    #[test]
    fn an_infinite_number_agrees_only_with_the_same_infinity() {
        let inf = f64::INFINITY;
        let n = |want: f64, got: f64| compare(&num(want), Kind::Number, &Value::Number(got));
        assert!(n(inf, inf).is_ok());
        assert!(n(-inf, -inf).is_ok());
        assert!(n(inf, -inf).is_err());
        assert!(n(inf, 1e308).is_err());
        assert!(n(1e308, inf).is_err());
        assert!(n(0.0, -inf).is_err());
        assert!(compare(&num(inf), Kind::Number, &Value::Undefined).is_err());
        let undefined = Expected {
            legacy_angle_number: false,
            kind: Kind::Number,
            data: None,
        };
        assert!(compare(&undefined, Kind::Number, &Value::Number(inf)).is_err());
        assert!(compare(&undefined, Kind::Number, &Value::Number(-inf)).is_err());
        assert!(compare(&undefined, Kind::Number, &Value::Undefined).is_ok());
    }

    #[test]
    fn undefined_must_match_in_both_directions() {
        let undefined = Expected {
            legacy_angle_number: false,
            kind: Kind::Point,
            data: None,
        };
        let defined = Expected {
            legacy_angle_number: false,
            kind: Kind::Point,
            data: Some(ExpectedData::Point { x: 1.0, y: 2.0 }),
        };
        assert!(compare(&undefined, Kind::Point, &Value::Undefined).is_ok());
        assert!(compare(&undefined, Kind::Point, &Value::Point { x: 1.0, y: 2.0 }).is_err());
        assert!(compare(&defined, Kind::Point, &Value::Undefined).is_err());
        assert!(compare(&defined, Kind::Point, &Value::Point { x: 1.0, y: 2.0 }).is_ok());
    }

    fn boolean(b: bool) -> Expected {
        Expected {
            legacy_angle_number: false,
            kind: Kind::Boolean,
            data: Some(ExpectedData::Bool(b)),
        }
    }

    #[test]
    fn booleans_are_compared_exactly() {
        let cmp = |e: &Expected, b: bool| compare(e, Kind::Boolean, &Value::Bool(b));
        assert!(cmp(&boolean(true), true).is_ok());
        assert!(cmp(&boolean(false), false).is_ok());
        assert!(cmp(&boolean(true), false).is_err());
        assert!(cmp(&boolean(false), true).is_err());
    }

    #[test]
    fn a_boolean_defined_on_one_side_only_fails() {
        let undefined = Expected {
            legacy_angle_number: false,
            kind: Kind::Boolean,
            data: None,
        };
        assert!(compare(&undefined, Kind::Boolean, &Value::Undefined).is_ok());
        assert!(compare(&undefined, Kind::Boolean, &Value::Bool(false)).is_err());
        assert!(compare(&boolean(false), Kind::Boolean, &Value::Undefined).is_err());
        assert!(compare(&boolean(true), Kind::Boolean, &Value::Undefined).is_err());
    }

    #[test]
    fn a_boolean_is_not_another_kind() {
        assert!(compare(&boolean(true), Kind::Number, &Value::Number(1.0)).is_err());
        assert!(compare(&num(1.0), Kind::Boolean, &Value::Bool(true)).is_err());
        assert!(compare(
            &boolean(true),
            Kind::Point,
            &Value::Point { x: 1.0, y: 0.0 }
        )
        .is_err());
        // The kinds agree but the value is of another type.
        assert!(compare(&boolean(true), Kind::Boolean, &Value::Number(1.0)).is_err());
    }

    #[test]
    fn type_mismatch_fails_even_when_undefined() {
        let e = Expected {
            legacy_angle_number: false,
            kind: Kind::Point,
            data: None,
        };
        assert!(compare(&e, Kind::Number, &Value::Undefined).is_err());
        assert!(compare(&num(1.0), Kind::Point, &Value::Point { x: 1.0, y: 0.0 }).is_err());
    }

    #[test]
    fn lines_agree_up_to_sign_and_scale() {
        let k = Value::Line {
            a: 0.0,
            b: 1.0,
            c: -2.0,
        };
        // Same line, scaled: same sign.
        assert_eq!(
            compare(&line(0.0, 6.0, -12.0), Kind::Line, &k),
            Ok(Sign::Agrees)
        );
        // Flipped sign is counted, not failed.
        assert_eq!(
            compare(&line(0.0, -3.0, 6.0), Kind::Line, &k),
            Ok(Sign::Flipped)
        );
        // A different line fails.
        assert!(compare(&line(0.0, 1.0, -3.0), Kind::Line, &k).is_err());
        assert!(compare(&line(1.0, 0.0, -2.0), Kind::Line, &k).is_err());
    }

    #[test]
    fn line_scale_needs_the_same_raw_triple() {
        let k = Value::Line {
            a: 0.0,
            b: 6.0,
            c: -12.0,
        };
        assert_eq!(line_scale_agrees(&line(0.0, 6.0, -12.0), &k), Some(true));
        // Same line, other scale or other sign: the comparison passes, the
        // scale count does not.
        assert_eq!(line_scale_agrees(&line(0.0, 1.0, -2.0), &k), Some(false));
        assert_eq!(line_scale_agrees(&line(0.0, -6.0, 12.0), &k), Some(false));
        // Relative to the largest component: noise in a tiny component passes.
        let big = Value::Line {
            a: 1e6,
            b: 1e-12,
            c: 3.0,
        };
        assert_eq!(line_scale_agrees(&line(1e6, 0.0, 3.0), &big), Some(true));
        assert_eq!(line_scale_agrees(&line(1e6, 0.0, 3.01), &big), Some(false));
        // Rays count too.
        let e = Expected {
            legacy_angle_number: false,
            kind: Kind::Ray,
            data: Some(ExpectedData::Ray {
                x: 1.0,
                y: 2.0,
                a: -4.0,
                b: 3.0,
                c: -2.0,
            }),
        };
        let ray = |s: f64| Value::Ray {
            x: 1.0,
            y: 2.0,
            a: -4.0 * s,
            b: 3.0 * s,
            c: -2.0 * s,
        };
        assert_eq!(line_scale_agrees(&e, &ray(1.0)), Some(true));
        assert_eq!(line_scale_agrees(&e, &ray(0.2)), Some(false));
        // Not applicable to other objects or undefined lines.
        assert_eq!(line_scale_agrees(&num(1.0), &Value::Number(1.0)), None);
        assert_eq!(
            line_scale_agrees(&line(0.0, 1.0, 0.0), &Value::Undefined),
            None
        );
    }

    #[test]
    fn signed_infinity_only_agrees_with_itself() {
        for infinite in [f64::INFINITY, f64::NEG_INFINITY] {
            assert!(numbers_agree(infinite, infinite));
            for different in [-infinite, 1.0, f64::NAN] {
                assert!(!numbers_agree(infinite, different));
                assert!(!numbers_agree(different, infinite));
            }
        }
    }

    #[test]
    fn curve_and_two_variable_samples_keep_infinite_values() {
        let c = geo_kernel::script::load("c = Curve(t, 1/t, t, -1, 1)\nf(x, y) = y/x").unwrap();
        for (label, kind) in [("c", Kind::Curve), ("f", Kind::FunctionNVar)] {
            let id = c.id(label).unwrap();
            for (sample, agrees) in [
                (None, false),
                (Some(f64::NEG_INFINITY), false),
                (Some(f64::INFINITY), true),
            ] {
                let data = if kind == Kind::Curve {
                    ExpectedData::Curve {
                        start: -1.0,
                        end: 1.0,
                        samples: vec![(0.0, sample.map(|y| (0.0, y)))],
                    }
                } else {
                    ExpectedData::FunctionNVar(vec![(0.0, 1.0, sample)])
                };
                let expected = Expected {
                    kind,
                    legacy_angle_number: false,
                    data: Some(data),
                };
                assert_eq!(compare_extra(&c, id, &expected).is_ok(), agrees, "{label}");
            }
        }
    }

    #[test]
    fn a_recorded_null_coordinate_meets_nan_only() {
        assert!(coordinate_agrees(f64::NAN, f64::NAN));
        assert!(!coordinate_agrees(f64::NAN, 1.0));
        assert!(!coordinate_agrees(f64::NAN, f64::INFINITY));
        assert!(coordinate_agrees(f64::INFINITY, f64::INFINITY));
        assert!(!coordinate_agrees(1.0, f64::NAN));
    }

    #[test]
    fn signed_curve_coordinates_meet_nan_or_the_same_infinity() {
        let c = geo_kernel::script::load("c = Curve(t, 1/t, t, -1, 1)").unwrap();
        let id = c.id("c").unwrap();
        for (want, agrees) in [
            ((0.0, f64::INFINITY), true),
            ((0.0, f64::NEG_INFINITY), false),
            ((0.0, f64::NAN), false),
            ((f64::NAN, f64::INFINITY), false),
        ] {
            let expected = Expected {
                kind: Kind::Curve,
                legacy_angle_number: false,
                data: Some(ExpectedData::Curve {
                    start: -1.0,
                    end: 1.0,
                    samples: vec![(0.0, Some(want))],
                }),
            };
            assert_eq!(compare_extra(&c, id, &expected).is_ok(), agrees, "{want:?}");
        }
    }

    #[test]
    fn functions_are_compared_by_samples() {
        let mut samples = vec![Some(1.0); 11];
        samples[0] = None;
        let e = Expected {
            legacy_angle_number: false,
            kind: Kind::Function,
            data: Some(ExpectedData::Function { samples }),
        };
        let at = |f: fn(f64) -> f64| move |x: f64| Some(f(x));
        let ok = at(|x| if x == -8.0 { f64::NAN } else { 1.0 + 1e-12 });
        assert!(compare_sampled(&e, Kind::Function, &Value::Function, &[], Some(&ok)).is_ok());
        let infinite = at(|x| if x == -8.0 { f64::INFINITY } else { 1.0 });
        assert!(
            compare_sampled(&e, Kind::Function, &Value::Function, &[], Some(&infinite)).is_err()
        );
        // a value where GeoGebra has none, and the other way round
        let value = at(|_| 1.0);
        assert!(compare_sampled(&e, Kind::Function, &Value::Function, &[], Some(&value)).is_err());
        let missing = at(|x| if x >= 0.0 { f64::NAN } else { 1.0 });
        assert!(
            compare_sampled(&e, Kind::Function, &Value::Function, &[], Some(&missing)).is_err()
        );
        let off = at(|x| if x == -8.0 { f64::NAN } else { 1.001 });
        assert!(compare_sampled(&e, Kind::Function, &Value::Function, &[], Some(&off)).is_err());
        assert!(compare_sampled(&e, Kind::Function, &Value::Undefined, &[], Some(&ok)).is_err());
        assert!(compare(&e, Kind::Number, &Value::Number(1.0)).is_err());
        // without its sampler a function never matches
        assert!(compare(&e, Kind::Function, &Value::Function).is_err());
    }

    #[test]
    fn vectors_are_compared_by_coordinates() {
        let e = Expected {
            legacy_angle_number: false,
            kind: Kind::Vector,
            data: Some(ExpectedData::Vector { x: 3.0, y: -0.0 }),
        };
        assert!(compare(&e, Kind::Vector, &Value::Vector { x: 3.0, y: 0.0 }).is_ok());
        assert!(compare(&e, Kind::Vector, &Value::Vector { x: -3.0, y: 0.0 }).is_err());
        assert!(compare(&e, Kind::Point, &Value::Point { x: 3.0, y: 0.0 }).is_err());
        let undefined = Expected {
            legacy_angle_number: false,
            kind: Kind::Vector,
            data: None,
        };
        assert!(compare(&undefined, Kind::Vector, &Value::Undefined).is_ok());
    }

    #[test]
    fn rays_need_the_start_point_and_the_same_direction() {
        // From (1, 2) towards (4, 6): raw coefficients (-4, 3, -2).
        let e = Expected {
            legacy_angle_number: false,
            kind: Kind::Ray,
            data: Some(ExpectedData::Ray {
                x: 1.0,
                y: 2.0,
                a: -4.0,
                b: 3.0,
                c: -2.0,
            }),
        };
        let ray = |x: f64, s: f64| Value::Ray {
            x,
            y: 2.0,
            a: s * -0.8,
            b: s * 0.6,
            c: s * -0.4,
        };
        assert_eq!(
            compare(&e, Kind::Ray, &ray(1.0, 1.0)),
            Ok(Sign::NotApplicable)
        );
        assert_eq!(
            compare(&e, Kind::Ray, &ray(1.0, -1.0)),
            Err("ray points the opposite way".into())
        );
        assert!(compare(&e, Kind::Ray, &ray(1.5, 1.0)).is_err());
        let other = Value::Ray {
            x: 1.0,
            y: 2.0,
            a: -0.8,
            b: 0.6,
            c: 0.0,
        };
        assert!(compare(&e, Kind::Ray, &other).is_err());
        assert!(compare(&e, Kind::Line, &ray(1.0, 1.0)).is_err());
    }

    #[test]
    fn segments_are_compared_in_order() {
        let e = Expected {
            legacy_angle_number: false,
            kind: Kind::Segment,
            data: Some(ExpectedData::Segment {
                x1: 0.0,
                y1: 0.0,
                x2: 1.0,
                y2: 0.0,
            }),
        };
        let same = Value::Segment {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 0.0,
        };
        let swapped = Value::Segment {
            x1: 1.0,
            y1: 0.0,
            x2: 0.0,
            y2: 0.0,
        };
        assert!(compare(&e, Kind::Segment, &same).is_ok());
        assert!(compare(&e, Kind::Segment, &swapped).is_err());
    }

    #[test]
    fn polygons_compare_areas_and_vertices_in_order() {
        let e = Expected {
            legacy_angle_number: false,
            kind: Kind::Polygon,
            data: Some(ExpectedData::Polygon {
                vertices: vec![(0.0, 0.0), (4.0, 0.0), (0.0, 3.0)],
                area: 6.0,
                signed_area: 6.0,
            }),
        };
        let v = |pts: &[(f64, f64)]| pts.iter().copied().map(Some).collect::<Vec<_>>();
        let ccw = v(&[(0.0, 0.0), (4.0, 0.0), (0.0, 3.0)]);
        let poly = |area| Value::Polygon { area };
        assert!(compare_object(&e, Kind::Polygon, &poly(6.0), &ccw).is_ok());
        // the sign of the area counts
        assert!(compare_object(&e, Kind::Polygon, &poly(-6.0), &ccw).is_err());
        // order and number of the vertices count
        let turned = v(&[(4.0, 0.0), (0.0, 3.0), (0.0, 0.0)]);
        assert!(compare_object(&e, Kind::Polygon, &poly(6.0), &turned).is_err());
        assert!(compare_object(&e, Kind::Polygon, &poly(6.0), &ccw[..2]).is_err());
        // without vertices, a polygon never agrees
        assert!(compare(&e, Kind::Polygon, &poly(6.0)).is_err());
    }

    #[test]
    fn polylines_compare_holes() {
        let e = Expected {
            legacy_angle_number: false,
            kind: Kind::PolyLine,
            data: Some(ExpectedData::PolyLine {
                vertices: vec![Some((0.0, 0.0)), None, Some((3.0, 4.0))],
                length: 0.0,
            }),
        };
        let line = Value::PolyLine { length: 0.0 };
        let holed = [Some((0.0, 0.0)), None, Some((3.0, 4.0))];
        let filled = [Some((0.0, 0.0)), Some((1.0, 1.0)), Some((3.0, 4.0))];
        assert!(compare_object(&e, Kind::PolyLine, &line, &holed).is_ok());
        assert!(compare_object(&e, Kind::PolyLine, &line, &filled).is_err());
        let longer = Value::PolyLine { length: 5.0 };
        assert!(compare_object(&e, Kind::PolyLine, &longer, &holed).is_err());
    }

    /// A record written by hand, independent of the kernel's computations.
    fn record(ty: ConicType, matrix: [f64; 6]) -> ConicData {
        let mut d = ConicData::new();
        d.ty = ty;
        d.defined = true;
        d.matrix = matrix;
        d
    }

    fn conic(conic_type: &str, matrix: [f64; 6], parts: ConicParts) -> Expected {
        Expected {
            legacy_angle_number: false,
            kind: Kind::Conic,
            data: Some(ExpectedData::Conic {
                conic_type: conic_type.into(),
                matrix,
                parts,
            }),
        }
    }

    fn conic_result(e: &Expected, d: &ConicData) -> Result<Sign, String> {
        let value = if d.defined {
            Value::Conic { ty: d.ty }
        } else {
            Value::Undefined
        };
        compare_conic(e, Kind::Conic, &value, Some(d)).0
    }

    /// Every way a conic can differ from its expectation is reported, one
    /// part at a time, for every kind of parts.
    #[test]
    fn conics_report_each_wrong_part() {
        // x^2/25 + y^2/16 = 1 around (1, 2)
        let ellipse_matrix = [16.0, 25.0, -284.0, 0.0, -16.0, -50.0];
        let mut ellipse = record(ConicType::Ellipse, ellipse_matrix);
        ellipse.midpoint = [1.0, 2.0];
        ellipse.half_axes = [5.0, 4.0];
        ellipse.eigenvec = [[1.0, 0.0], [0.0, 1.0]];
        let e_ellipse = conic(
            "ellipse",
            ellipse_matrix,
            ConicParts::Central {
                midpoint: (1.0, 2.0),
                half_axes: (5.0, 4.0),
                eigenvectors: [(1.0, 0.0), (0.0, 1.0)],
            },
        );
        assert_eq!(conic_result(&e_ellipse, &ellipse), Ok(Sign::NotApplicable));
        let central: [fn(&mut ConicData); 5] = [
            |d| d.midpoint[1] += 0.5,
            |d| d.half_axes[0] = 5.5,
            |d| d.half_axes[1] = 3.0,
            |d| d.eigenvec[0] = [0.0, 1.0],
            |d| d.eigenvec[1] = [0.0, -1.0],
        ];
        for (i, wrong) in central.iter().enumerate() {
            let mut d = ellipse;
            wrong(&mut d);
            let r = conic_result(&e_ellipse, &d);
            assert!(
                matches!(&r, Err(m) if m.starts_with("parts differ")),
                "central part {i}: {r:?}"
            );
        }

        // y = x^2 / 4: focus (0, 1), vertex (0, 0), p = 2
        let parabola_matrix = [1.0, 0.0, 0.0, 0.0, 0.0, -2.0];
        let mut parabola = record(ConicType::Parabola, parabola_matrix);
        parabola.midpoint = [0.0, 0.0];
        parabola.p = 2.0;
        parabola.eigenvec = [[0.0, 1.0], [-1.0, 0.0]];
        let e_parabola = conic(
            "parabola",
            parabola_matrix,
            ConicParts::Parabola {
                vertex: (0.0, 0.0),
                p: 2.0,
                eigenvectors: [(0.0, 1.0), (-1.0, 0.0)],
            },
        );
        assert_eq!(
            conic_result(&e_parabola, &parabola),
            Ok(Sign::NotApplicable)
        );
        let parabolic: [fn(&mut ConicData); 4] = [
            |d| d.midpoint[0] = 0.25,
            |d| d.p = 1.0,
            |d| d.eigenvec[0] = [0.0, -1.0],
            |d| d.eigenvec[1] = [1.0, 0.0],
        ];
        for (i, wrong) in parabolic.iter().enumerate() {
            let mut d = parabola;
            wrong(&mut d);
            let r = conic_result(&e_parabola, &d);
            assert!(
                matches!(&r, Err(m) if m.starts_with("parts differ")),
                "parabola part {i}: {r:?}"
            );
        }

        // the single point (3, -1): (x - 3)^2 + (y + 1)^2 = 0
        let point_matrix = [1.0, 1.0, 10.0, 0.0, -3.0, 1.0];
        let mut point = record(ConicType::SinglePoint, point_matrix);
        point.midpoint = [3.0, -1.0];
        let e_point = conic(
            "single_point",
            point_matrix,
            ConicParts::SinglePoint { point: (3.0, -1.0) },
        );
        assert_eq!(conic_result(&e_point, &point), Ok(Sign::NotApplicable));
        let mut moved = point;
        moved.midpoint = [3.0, -1.5];
        assert!(matches!(conic_result(&e_point, &moved), Err(m) if m.starts_with("parts differ")));

        // the lines x = 0 and y = 0: x y = 0
        let pair_matrix = [0.0, 0.0, 0.0, 0.5, 0.0, 0.0];
        let mut pair = record(ConicType::IntersectingLines, pair_matrix);
        pair.lines = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let e_pair = conic(
            "intersecting_lines",
            pair_matrix,
            ConicParts::Lines {
                lines: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            },
        );
        assert_eq!(conic_result(&e_pair, &pair), Ok(Sign::NotApplicable));
        for wrong in [[0.0, 1.0, 1.0], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0]] {
            let mut d = pair;
            d.lines[1] = wrong;
            let r = conic_result(&e_pair, &d);
            assert!(
                matches!(&r, Err(m) if m.starts_with("parts differ")),
                "second line {wrong:?}: {r:?}"
            );
        }
        // the lines in the other order
        let mut swapped = pair;
        swapped.lines = [pair.lines[1], pair.lines[0]];
        assert!(conic_result(&e_pair, &swapped).is_err());

        // a wrong type with the same matrix and parts
        let mut circle = ellipse;
        circle.ty = ConicType::Circle;
        let r = conic_result(&e_ellipse, &circle);
        assert!(
            matches!(&r, Err(m) if m.starts_with("conic type differs")),
            "{r:?}"
        );
        let mut hyperbola = ellipse;
        hyperbola.ty = ConicType::Hyperbola;
        assert!(conic_result(&e_ellipse, &hyperbola).is_err());

        // a zero matrix on either side, and a NaN entry
        let mut zero = ellipse;
        zero.matrix = [0.0; 6];
        assert!(
            matches!(conic_result(&e_ellipse, &zero), Err(m) if m.starts_with("matrix differs"))
        );
        let e_zero = conic(
            "ellipse",
            [0.0; 6],
            ConicParts::Central {
                midpoint: (1.0, 2.0),
                half_axes: (5.0, 4.0),
                eigenvectors: [(1.0, 0.0), (0.0, 1.0)],
            },
        );
        assert!(
            matches!(conic_result(&e_zero, &ellipse), Err(m) if m.starts_with("matrix differs"))
        );
        let mut nan = ellipse;
        nan.matrix[2] = f64::NAN;
        assert!(
            matches!(conic_result(&e_ellipse, &nan), Err(m) if m.starts_with("matrix differs"))
        );
        let mut nan_part = ellipse;
        nan_part.half_axes[1] = f64::NAN;
        assert!(conic_result(&e_ellipse, &nan_part).is_err());

        // defined flipped either way
        let mut undefined = ellipse;
        undefined.defined = false;
        assert!(conic_result(&e_ellipse, &undefined).is_err());
        // the record says undefined although the value claims a conic
        let r = compare_conic(
            &e_ellipse,
            Kind::Conic,
            &Value::Conic {
                ty: ConicType::Ellipse,
            },
            Some(&undefined),
        )
        .0;
        assert!(r.is_err(), "{r:?}");
        let none = Expected {
            legacy_angle_number: false,
            kind: Kind::Conic,
            data: None,
        };
        assert!(conic_result(&none, &ellipse).is_err());
        assert_eq!(conic_result(&none, &undefined), Ok(Sign::NotApplicable));
    }

    #[test]
    fn conics_compare_the_raw_matrix_and_count_the_scale() {
        use geo_kernel::script::load;
        let c = load("F = (0, 1)\nA = (0, -1)\nB = (1, -1)\ng = Line(A, B)\np = Parabola(F, g)\n")
            .unwrap();
        let id = c.id("p").unwrap();
        let d = c.conic(id).unwrap();
        let expected = |matrix: [f64; 6]| Expected {
            legacy_angle_number: false,
            kind: Kind::Conic,
            data: Some(ExpectedData::Conic {
                conic_type: "parabola".into(),
                matrix,
                parts: ConicParts::Parabola {
                    vertex: (d.midpoint[0], d.midpoint[1]),
                    p: d.p,
                    eigenvectors: [
                        (d.eigenvec[0][0], d.eigenvec[0][1]),
                        (d.eigenvec[1][0], d.eigenvec[1][1]),
                    ],
                },
            }),
        };
        let (r, a) = compare_conic(&expected(d.matrix), Kind::Conic, &c.value(id), Some(&d));
        assert_eq!(r, Ok(Sign::NotApplicable));
        assert_eq!(
            a,
            Some(MatrixAgreement {
                raw: true,
                up_to_scale: true
            })
        );
        // the same conic with another scale fails, and is counted as agreeing up to scale
        let (r, a) = compare_conic(
            &expected(d.matrix.map(|v| 2.0 * v)),
            Kind::Conic,
            &c.value(id),
            Some(&d),
        );
        assert!(r.is_err());
        assert_eq!(
            a,
            Some(MatrixAgreement {
                raw: false,
                up_to_scale: true
            })
        );
        // an undefined expectation against a defined conic
        let undefined = Expected {
            legacy_angle_number: false,
            kind: Kind::Conic,
            data: None,
        };
        assert!(
            compare_conic(&undefined, Kind::Conic, &c.value(id), Some(&d))
                .0
                .is_err()
        );
    }
}
