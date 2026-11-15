//! Compact text for values, kinds and durations.

use geo_kernel::kernel::Value;
use std::time::Duration;

/// A number with at most 4 decimals and no trailing zeros; very large or very
/// small magnitudes use scientific notation.
pub fn number(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf" } else { "-inf" }.into();
    }
    let m = v.abs();
    if m != 0.0 && !(1e-4..1e9).contains(&m) {
        return format!("{v:.3e}");
    }
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    // Rounds to zero from either side; avoid "-0".
    if s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}

fn signed_term(coef: f64, var: &str, first: bool) -> String {
    let body = if var.is_empty() {
        number(coef.abs())
    } else if number(coef.abs()) == "1" {
        var.to_owned()
    } else {
        format!("{}{var}", number(coef.abs()))
    };
    match (first, coef.is_sign_negative() && coef != 0.0) {
        (true, false) => body,
        (true, true) => format!("-{body}"),
        (false, false) => format!(" + {body}"),
        (false, true) => format!(" - {body}"),
    }
}

/// A line `a x + b y + c = 0`, scaled to a unit normal when the coefficients
/// allow it (the kernel does not promise normalised coefficients).
fn line(a: f64, b: f64, c: f64) -> String {
    let n = a.hypot(b);
    let (a, b, c) = if n.is_finite() && n > 0.0 {
        (a / n, b / n, c / n)
    } else {
        (a, b, c)
    };
    format!(
        "{}{}{} = 0",
        signed_term(a, "x", true),
        signed_term(b, "y", false),
        signed_term(c, "", false)
    )
}

/// The current value of an object in one short line.
pub fn value(v: &Value) -> String {
    match *v {
        Value::Undefined => "undefined".into(),
        Value::Point { x, y } => format!("({}, {})", number(x), number(y)),
        Value::Number(n) => number(n),
        Value::Line { a, b, c } => line(a, b, c),
        Value::Segment { x1, y1, x2, y2 } => format!(
            "({}, {}) to ({}, {}), length {}",
            number(x1),
            number(y1),
            number(x2),
            number(y2),
            number((x2 - x1).hypot(y2 - y1))
        ),
        Value::Circle { cx, cy, r } => {
            format!("centre ({}, {}), r = {}", number(cx), number(cy), number(r))
        }
        Value::Vector { x, y } => format!("({}, {})", number(x), number(y)),
        Value::Ray { x, y, a, b, .. } => {
            // A ray points along (b, -a); show the unit direction.
            let len = a.hypot(b);
            let (dx, dy) = if len > 0.0 {
                (b / len, -a / len)
            } else {
                (0.0, 0.0)
            };
            format!(
                "from ({}, {}), direction ({}, {})",
                number(x),
                number(y),
                number(dx),
                number(dy)
            )
        }
        // Kinds added to the kernel later still get a readable fallback.
        #[allow(unreachable_patterns)]
        other => format!("{other:?}"),
    }
}

/// A duration with a unit chosen for readability.
pub fn duration(d: Duration) -> String {
    let us = d.as_secs_f64() * 1e6;
    if us < 1000.0 {
        format!("{us:.0} us")
    } else if us < 1e6 {
        format!("{:.2} ms", us / 1e3)
    } else {
        format!("{:.2} s", us / 1e6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_trimmed() {
        assert_eq!(number(3.0), "3");
        assert_eq!(number(2.5), "2.5");
        assert_eq!(number(1.0 / 3.0), "0.3333");
        assert_eq!(number(-0.00001), "-1.000e-5");
        assert_eq!(number(0.0), "0");
        assert_eq!(number(-0.0), "0");
        assert_eq!(number(-0.00004999), "-4.999e-5");
        assert_eq!(number(123456.789), "123456.789");
    }

    #[test]
    fn extreme_numbers_use_scientific_notation() {
        assert_eq!(number(1.5e12), "1.500e12");
        assert_eq!(number(2e-7), "2.000e-7");
        assert_eq!(number(f64::NAN), "NaN");
        assert_eq!(number(f64::INFINITY), "inf");
        assert_eq!(number(f64::NEG_INFINITY), "-inf");
    }

    #[test]
    fn rounding_to_zero_has_no_sign() {
        assert_eq!(number(-0.00004), "-4.000e-5");
        assert_eq!(number(-0.0001), "-0.0001");
    }

    #[test]
    fn points_segments_circles() {
        assert_eq!(value(&Value::Point { x: 1.5, y: -2.0 }), "(1.5, -2)");
        assert_eq!(
            value(&Value::Segment {
                x1: 0.0,
                y1: 0.0,
                x2: 3.0,
                y2: 4.0
            }),
            "(0, 0) to (3, 4), length 5"
        );
        assert_eq!(
            value(&Value::Circle {
                cx: 1.0,
                cy: 2.0,
                r: 0.5
            }),
            "centre (1, 2), r = 0.5"
        );
        assert_eq!(value(&Value::Number(7.25)), "7.25");
        assert_eq!(value(&Value::Undefined), "undefined");
    }

    #[test]
    fn vectors_and_rays() {
        assert_eq!(value(&Value::Vector { x: 4.0, y: -1.5 }), "(4, -1.5)");
        // Raw coefficients of any scale: the ray from (1, 2) along +x.
        assert_eq!(
            value(&Value::Ray {
                x: 1.0,
                y: 2.0,
                a: 0.0,
                b: 8.0,
                c: -16.0
            }),
            "from (1, 2), direction (1, 0)"
        );
    }

    #[test]
    fn lines_are_normalised_for_display() {
        // 2x + 0y - 4 = 0 is x = 2.
        assert_eq!(
            value(&Value::Line {
                a: 2.0,
                b: 0.0,
                c: -4.0
            }),
            "x + 0y - 2 = 0"
        );
        assert_eq!(
            value(&Value::Line {
                a: 0.0,
                b: 1.0,
                c: 3.0
            }),
            "0x + y + 3 = 0"
        );
        // Already-unit and scaled coefficients show the same text.
        let u = value(&Value::Line {
            a: 0.6,
            b: -0.8,
            c: 1.0,
        });
        let s = value(&Value::Line {
            a: 6.0,
            b: -8.0,
            c: 10.0,
        });
        assert_eq!(u, s);
        assert_eq!(u, "0.6x - 0.8y + 1 = 0");
    }

    #[test]
    fn degenerate_line_does_not_divide_by_zero() {
        let s = value(&Value::Line {
            a: 0.0,
            b: 0.0,
            c: 1.0,
        });
        assert_eq!(s, "0x + 0y + 1 = 0");
    }

    #[test]
    fn durations_pick_a_unit() {
        assert_eq!(duration(Duration::from_micros(12)), "12 us");
        assert_eq!(duration(Duration::from_micros(2500)), "2.50 ms");
        assert_eq!(duration(Duration::from_millis(1500)), "1.50 s");
    }
}
