//! The comparison rules of `golden/SCHEMA.md`.

use crate::model::{Expected, ExpectedData};
use geo_kernel::{Kind, Value};

/// Result of a successful comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    /// Not a line, or an undefined line.
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

/// Compares one recorded object with what the kernel holds.
/// `kind` is the kernel's kind of the object (valid even when undefined).
pub fn compare(expected: &Expected, kind: Kind, actual: &Value) -> Result<Sign, String> {
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
