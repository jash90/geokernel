//! The comparison rules of `golden/SCHEMA.md`.

use crate::model::{Expected, ExpectedData, SAMPLE_XS};
use geo_kernel::{Kind, Value};

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

/// `|p - q| <= 1e-9 * max(1, |p|, |q|)`.
pub fn numbers_agree(p: f64, q: f64) -> bool {
    (p - q).abs() <= TOLERANCE * 1.0_f64.max(p.abs()).max(q.abs())
}

fn all_agree(pairs: &[(f64, f64)]) -> bool {
    pairs.iter().all(|&(p, q)| numbers_agree(p, q))
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
        .iter()
        .fold(0.0_f64, |m, v| m.max(v.abs()));
    let close = |p: f64, q: f64| (p - q).abs() <= TOLERANCE * scale;
    Some(close(*a, x) && close(*b, y) && close(*c, z))
}

/// Compares one recorded object with what the kernel holds.
/// `kind` is the kernel's kind of the object (valid even when undefined).
/// A function is compared by [`compare_sampled`].
pub fn compare(expected: &Expected, kind: Kind, actual: &Value) -> Result<Sign, String> {
    compare_sampled(expected, kind, actual, &|_| None)
}

/// [`compare`], with `sample` giving the kernel's value of the object at `x`
/// when it is a function. Each sample is compared like a number; a `null`
/// sample must meet a value that is not finite, and the other way round.
pub fn compare_sampled(
    expected: &Expected,
    kind: Kind,
    actual: &Value,
    sample: &dyn Fn(f64) -> Option<f64>,
) -> Result<Sign, String> {
    if expected.kind != kind {
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
            for (&x, want) in SAMPLE_XS.iter().zip(samples) {
                let got = sample(x).filter(|v| v.is_finite());
                let agree = match (*want, got) {
                    (None, None) => true,
                    (Some(p), Some(q)) => numbers_agree(p, q),
                    _ => false,
                };
                if !agree {
                    return Err(format!("value at x = {x} differs: kernel has {got:?}"));
                }
            }
            Ok(Sign::NotApplicable)
        }
        _ => Err("kernel value has a different shape than its kind".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(v: f64) -> Expected {
        Expected {
            kind: Kind::Number,
            data: Some(ExpectedData::Number(v)),
        }
    }

    fn line(a: f64, b: f64, c: f64) -> Expected {
        Expected {
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
    fn undefined_must_match_in_both_directions() {
        let undefined = Expected {
            kind: Kind::Point,
            data: None,
        };
        let defined = Expected {
            kind: Kind::Point,
            data: Some(ExpectedData::Point { x: 1.0, y: 2.0 }),
        };
        assert!(compare(&undefined, Kind::Point, &Value::Undefined).is_ok());
        assert!(compare(&undefined, Kind::Point, &Value::Point { x: 1.0, y: 2.0 }).is_err());
        assert!(compare(&defined, Kind::Point, &Value::Undefined).is_err());
        assert!(compare(&defined, Kind::Point, &Value::Point { x: 1.0, y: 2.0 }).is_ok());
    }

    #[test]
    fn type_mismatch_fails_even_when_undefined() {
        let e = Expected {
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
    fn functions_are_compared_by_samples() {
        let mut samples = vec![Some(1.0); 11];
        samples[0] = None;
        let e = Expected {
            kind: Kind::Function,
            data: Some(ExpectedData::Function { samples }),
        };
        let at = |f: fn(f64) -> f64| move |x: f64| Some(f(x));
        let ok = at(|x| if x == -8.0 { f64::NAN } else { 1.0 + 1e-12 });
        assert!(compare_sampled(&e, Kind::Function, &Value::Function, &ok).is_ok());
        let infinite = at(|x| if x == -8.0 { f64::INFINITY } else { 1.0 });
        assert!(compare_sampled(&e, Kind::Function, &Value::Function, &infinite).is_ok());
        // a value where GeoGebra has none, and the other way round
        let value = at(|_| 1.0);
        assert!(compare_sampled(&e, Kind::Function, &Value::Function, &value).is_err());
        let missing = at(|x| if x >= 0.0 { f64::NAN } else { 1.0 });
        assert!(compare_sampled(&e, Kind::Function, &Value::Function, &missing).is_err());
        let off = at(|x| if x == -8.0 { f64::NAN } else { 1.001 });
        assert!(compare_sampled(&e, Kind::Function, &Value::Function, &off).is_err());
        assert!(compare_sampled(&e, Kind::Function, &Value::Undefined, &ok).is_err());
        assert!(compare(&e, Kind::Number, &Value::Number(1.0)).is_err());
    }

    #[test]
    fn vectors_are_compared_by_coordinates() {
        let e = Expected {
            kind: Kind::Vector,
            data: Some(ExpectedData::Vector { x: 3.0, y: -0.0 }),
        };
        assert!(compare(&e, Kind::Vector, &Value::Vector { x: 3.0, y: 0.0 }).is_ok());
        assert!(compare(&e, Kind::Vector, &Value::Vector { x: -3.0, y: 0.0 }).is_err());
        assert!(compare(&e, Kind::Point, &Value::Point { x: 3.0, y: 0.0 }).is_err());
        let undefined = Expected {
            kind: Kind::Vector,
            data: None,
        };
        assert!(compare(&undefined, Kind::Vector, &Value::Undefined).is_ok());
    }

    #[test]
    fn rays_need_the_start_point_and_the_same_direction() {
        // From (1, 2) towards (4, 6): raw coefficients (-4, 3, -2).
        let e = Expected {
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
}
