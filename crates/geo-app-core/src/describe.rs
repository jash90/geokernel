//! Algebra-view text for objects: numbers rounded to two decimals, line,
//! circle and conic equations. No UI types.

use crate::text::{self, Label};
use geo_kernel::kernel::{Construction, Id, Value};
use geo_kernel::ConicData;

/// Two decimals with trailing zeros removed and never a negative zero.
pub fn n2(v: f64) -> String {
    if v.is_nan() {
        return "?".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf" } else { "-inf" }.into();
    }
    let s = format!("{v:.2}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        s
    };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// `v` with at most `decimals` decimals, trailing zeros removed, never `-0`.
pub fn fixed(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        s
    };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// How many decimals a point placed by a click needs so that rounding moves
/// it by at most a quarter of a pixel at `scale` pixels per unit. At the
/// standard zoom (50 px per unit) that is two decimals.
pub fn click_decimals(scale: f64) -> usize {
    if !(scale.is_finite() && scale > 0.0) {
        return 2;
    }
    (scale / 0.5).log10().ceil().clamp(0.0, 12.0) as usize
}

/// `value` as a term appended after `y = ` or `x`: `+ 3.75` / `- 3.75`.
fn signed_tail(v: f64) -> Option<String> {
    let s = n2(v);
    if s == "0" {
        None
    } else if let Some(rest) = s.strip_prefix('-') {
        Some(format!(" - {rest}"))
    } else {
        Some(format!(" + {s}"))
    }
}

/// The equation of `a*x + b*y + c = 0`: `y = -0.12x + 3.75`, or `x = -5.64`
/// when the line is vertical.
pub fn line_equation(a: f64, b: f64, c: f64) -> String {
    let len = a.hypot(b);
    if !(len.is_finite() && len > 0.0) {
        return "?".into();
    }
    // Vertical when the slope would be beyond what the display can show.
    if b.abs() <= 1e-9 * len {
        return format!("x = {}", n2(-c / a));
    }
    let slope = -a / b;
    let intercept = -c / b;
    let slope_text = match n2(slope).as_str() {
        "0" => None,
        "1" => Some("x".to_owned()),
        "-1" => Some("-x".to_owned()),
        s => Some(format!("{s}x")),
    };
    match (slope_text, signed_tail(intercept)) {
        (None, _) => format!("y = {}", n2(intercept)),
        (Some(s), None) => format!("y = {s}"),
        (Some(s), Some(tail)) => format!("y = {s}{tail}"),
    }
}

/// `(x - 1)² + (y + 2)² = 9`; a zero centre coordinate gives a bare `x²`.
pub fn circle_equation(cx: f64, cy: f64, r: f64) -> String {
    fn square(var: &str, centre: f64) -> String {
        match n2(centre).as_str() {
            "0" => format!("{var}²"),
            s => match s.strip_prefix('-') {
                Some(rest) => format!("({var} + {rest})²"),
                None => format!("({var} - {s})²"),
            },
        }
    }
    format!("{} + {} = {}", square("x", cx), square("y", cy), n2(r * r))
}

/// The implicit equation of a conic, `x² + 2y² = 4`, in the form the
/// reference shows by default (see `geo_kernel::equation`).
pub fn conic_equation(d: &ConicData) -> String {
    geo_kernel::equation::conic_equation(d, &n2)
}

/// The text after `=` for the object `id`: [`value_text`] of its value, or
/// the equation of a conic, whose value alone does not hold it. A function
/// has no text of its own: its row shows its definition.
pub fn object_text(c: &Construction, id: Id) -> String {
    match c.value(id) {
        Value::Conic { .. } => match c.conic(id) {
            Some(d) => conic_equation(&d),
            None => text::label(Label::Undefined).into(),
        },
        Value::Function => String::new(),
        v => value_text(&v),
    }
}

/// The row value of an object of a kind the window does not know yet: a
/// kernel that gains a kind must not make its objects vanish. Such an object
/// keeps its algebra row (label, definition and this text); the drawing and
/// picking skip it.
pub const NOT_DRAWN_YET: &str = text::label(Label::NotDrawnYet);

/// Whether the window knows how to show a value: every variant the kernel
/// has today. `Value` is `#[non_exhaustive]`; a variant added later is not
/// known and its row says so (see [`NOT_DRAWN_YET`]). One match, no
/// allocation.
pub fn is_known(v: &Value) -> bool {
    matches!(
        v,
        Value::Undefined
            | Value::Point { .. }
            | Value::Vector { .. }
            | Value::Number(_)
            | Value::Bool(_)
            | Value::Line { .. }
            | Value::Ray { .. }
            | Value::Segment { .. }
            | Value::Circle { .. }
            | Value::Polygon { .. }
            | Value::PolyLine { .. }
            | Value::Conic { .. }
            | Value::Function
    )
}

/// The text after `=` in the second line of a dependent object's row.
pub fn value_text(v: &Value) -> String {
    match *v {
        Value::Undefined => text::label(Label::Undefined).into(),
        Value::Point { x, y } | Value::Vector { x, y } => format!("({}, {})", n2(x), n2(y)),
        Value::Number(n) => n2(n),
        Value::Line { a, b, c } | Value::Ray { a, b, c, .. } => line_equation(a, b, c),
        Value::Segment { x1, y1, x2, y2 } => n2((x2 - x1).hypot(y2 - y1)),
        Value::Circle { cx, cy, r } => circle_equation(cx, cy, r),
        // A polygon shows its area, a polyline its length.
        Value::Polygon { area } => n2(area.abs()),
        Value::PolyLine { length } => n2(length),
        // As before the fallback below existed (the debug form, `Bool(true)`).
        Value::Bool(_) => format!("{v:?}"),
        // A kind added to the kernel later keeps its row with this text.
        #[allow(unreachable_patterns)]
        _ => NOT_DRAWN_YET.into(),
    }
}

/// An angle given in radians, shown in degrees: `45°`, `33.69°`.
pub fn degrees(radians: f64) -> String {
    format!("{}°", n2(radians.to_degrees()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_are_shown_in_degrees() {
        assert_eq!(degrees(std::f64::consts::FRAC_PI_4), "45°");
        assert_eq!(degrees(std::f64::consts::PI), "180°");
    }

    #[test]
    fn numbers_have_two_decimals_and_no_negative_zero() {
        assert_eq!(n2(-5.644), "-5.64");
        assert_eq!(n2(3.3), "3.3");
        assert_eq!(n2(4.0), "4");
        assert_eq!(n2(-0.004), "0");
        assert_eq!(n2(-0.0), "0");
        assert_eq!(n2(0.005), "0.01");
        assert_eq!(n2(1234.5678), "1234.57");
        assert_eq!(n2(f64::NAN), "?");
    }

    #[test]
    fn clicked_points_round_trip_to_a_quarter_pixel() {
        assert_eq!(click_decimals(50.0), 2);
        assert_eq!(click_decimals(5.0), 1);
        assert_eq!(click_decimals(5000.0), 4);
        assert_eq!(click_decimals(0.001), 0);
        for scale in [0.5, 3.0, 50.0, 700.0, 2.5e5] {
            let d = click_decimals(scale);
            let x = 1.234567891;
            let back: f64 = fixed(x, d).parse().unwrap();
            assert!((back - x).abs() * scale <= 0.26, "scale {scale}");
        }
        assert_eq!(fixed(-0.001, 2), "0");
    }

    #[test]
    fn lines_show_slope_intercept_form() {
        // Through (-5.64, 4.44) and (3.3, 3.34): the reference line f.
        let (x1, y1, x2, y2) = (-5.64, 4.44, 3.3, 3.34);
        let (a, b) = (y1 - y2, x2 - x1);
        let c = -(a * x1 + b * y1);
        // The intercept is 3.746..., which rounds to 3.75.
        assert_eq!(line_equation(a, b, c), "y = -0.12x + 3.75");
    }

    #[test]
    fn vertical_horizontal_and_special_slopes() {
        assert_eq!(line_equation(1.0, 0.0, 5.64), "x = -5.64");
        assert_eq!(line_equation(-2.0, 0.0, -2.0), "x = -1");
        assert_eq!(line_equation(0.0, 1.0, -3.0), "y = 3");
        assert_eq!(line_equation(0.0, 2.0, 0.0), "y = 0");
        assert_eq!(line_equation(-1.0, 1.0, 0.0), "y = x");
        assert_eq!(line_equation(1.0, 1.0, 0.0), "y = -x");
        assert_eq!(line_equation(-2.0, 1.0, 3.0), "y = 2x - 3");
        assert_eq!(line_equation(0.0, 0.0, 1.0), "?");
    }

    #[test]
    fn nearly_vertical_lines_are_vertical() {
        assert_eq!(line_equation(1.0, 1e-12, -2.0), "x = 2");
    }

    #[test]
    fn circles_show_the_centre_form() {
        assert_eq!(circle_equation(1.0, -2.0, 3.0), "(x - 1)² + (y + 2)² = 9");
        assert_eq!(circle_equation(0.0, 0.0, 1.5), "x² + y² = 2.25");
        assert_eq!(circle_equation(-0.001, 2.0, 1.0), "x² + (y - 2)² = 1");
    }

    #[test]
    fn values_per_kind() {
        assert_eq!(
            value_text(&Value::Point { x: 4.92, y: -2.14 }),
            "(4.92, -2.14)"
        );
        assert_eq!(value_text(&Value::Number(2.0)), "2");
        assert_eq!(
            value_text(&Value::Segment {
                x1: 0.0,
                y1: 0.0,
                x2: 3.0,
                y2: 4.0
            }),
            "5"
        );
        assert_eq!(value_text(&Value::Vector { x: 1.0, y: -0.0 }), "(1, 0)");
        assert_eq!(value_text(&Value::Undefined), "niezdefiniowany");
    }
}
