//! The calculus commands of the function group, from GeoGebra's own tests
//! (`CommandsTest`, `CommandsUsingMockedCasTest`, `AlgoNumeratorDenominatorFunTest`)
//! plus degenerate inputs the upstream tests do not reach. Expected values
//! come from recorded t-pairs or an analytic calculation.

mod common;

use common::number_of;
use geo_kernel::Construction;

fn load(src: &str) -> Construction {
    geo_kernel::script::load(src).unwrap()
}

fn set_number(c: &mut Construction, label: &str, v: f64) {
    c.set_number(c.id(label).unwrap(), v).unwrap();
}

fn num(c: &geo_kernel::Construction, label: &str) -> f64 {
    number_of(&c.value(c.id(label).unwrap())).unwrap()
}

fn defined(c: &geo_kernel::Construction, label: &str) -> bool {
    c.value(c.id(label).unwrap()).is_defined()
}

/// `CommandsTest.cmdLeftSum`.
#[test]
fn left_sum_matches_geogebra() {
    let c = load("f(x) = sin(x)\nd = LeftSum(f, 4, 13, 42)");
    assert!(
        (num(&c, "d") - -1.681_216_143_937_807).abs() < 1e-12,
        "{}",
        num(&c, "d")
    );
}

fn exact_sine_extreme_sum(sign: f64) -> f64 {
    let width = 9.0 / 42.0;
    let peak = sign * std::f64::consts::FRAC_PI_2;
    let period = std::f64::consts::TAU;
    let heights: f64 = (0..42)
        .map(|i| {
            let left = 4.0 + f64::from(i) * width;
            let right = left + width;
            let next_peak = peak + period * ((left - peak) / period).ceil();
            if next_peak <= right {
                1.0
            } else {
                (sign * left.sin()).max(sign * right.sin())
            }
        })
        .sum();
    sign * width * heights
}

/// The numerical lower sum lies above the sum of the true sine minima.
#[test]
fn lower_sum_approximates_the_sine_minima() {
    let c = load("f(x) = sin(x)\nd = LowerSum(f, 4, 13, 42)");
    let error = num(&c, "d") - exact_sine_extreme_sum(-1.0);
    assert!((0.0..1e-6).contains(&error), "{error}");
}

/// The numerical upper sum lies below the sum of the true sine maxima.
#[test]
fn upper_sum_approximates_the_sine_maxima() {
    let c = load("f(x) = sin(x)\nd = UpperSum(f, 4, 13, 42)");
    let error = exact_sine_extreme_sum(1.0) - num(&c, "d");
    assert!((0.0..1e-6).contains(&error), "{error}");
}

/// `CommandsTest.cmdTrapezoidalSum`.
#[test]
fn trapezoidal_sum_matches_geogebra() {
    let c = load("f(x) = sin(x)\nd = TrapezoidalSum(f, 4, 13, 42)");
    assert!(
        (num(&c, "d") - -1.555_112_265_494_817_5).abs() < 1e-12,
        "{}",
        num(&c, "d")
    );
}

/// `CommandsTest.cmdRectangleSum`.
#[test]
fn rectangle_sum_matches_geogebra() {
    let c = load("f(x) = sin(x)\nd = RectangleSum(f, 4, 13, 42, 0.05)");
    assert!(
        (num(&c, "d") - -1.670_349_823_767_600_7).abs() < 1e-12,
        "{}",
        num(&c, "d")
    );
}

/// `CommandsTest.cmdLowerSum` and friends: a function undefined on part of
/// the interval makes every sum undefined.
#[test]
fn sums_of_a_piecewise_undefined_function_are_undefined() {
    let c = load(
        "f(x) = If(x^2 > 9, 1)\n\
         a = LowerSum(f, -5, 5, 1)\n\
         b = UpperSum(f, -5, 5, 1)\n\
         c = LeftSum(f, -5, 5, 1)\n\
         d = TrapezoidalSum(f, -5, 5, 1)\n\
         e = RectangleSum(f, -5, 5, 1, 0.1)",
    );
    for label in ["a", "b", "c", "d", "e"] {
        assert!(!defined(&c, label), "{label}");
    }
}

/// GeoGebra's `AlgoFunctionAreaSums`: `n < 1` is undefined, a fractional
/// `n` is rounded and `n` above 10000 is capped.
#[test]
fn sums_round_their_subdivisions_and_refuse_less_than_one() {
    let mut c = load("f(x) = x^2\nn = 4\nd = LowerSum(f, 0, 2, n)");
    set_number(&mut c, "n", 0.0);
    assert!(!defined(&c, "d"));
    set_number(&mut c, "n", -3.0);
    assert!(!defined(&c, "d"));
    set_number(&mut c, "n", 2.0);
    assert!((num(&c, "d") - 1.0).abs() < 1e-9, "{}", num(&c, "d"));
    // 1.5 rounds to 2
    set_number(&mut c, "n", 1.5);
    assert!((num(&c, "d") - 1.0).abs() < 1e-9, "{}", num(&c, "d"));
}

/// A reversed interval negates the sum; `RectangleSum` needs a divider in
/// `[0, 1]`.
#[test]
fn sums_read_reversed_intervals_and_rectangle_dividers() {
    let c = load(
        "f(x) = x^2\n\
         a = LeftSum(f, 2, 0, 4)\n\
         b = TrapezoidalSum(f, 2, 0, 4)\n\
         c = RectangleSum(f, 0, 2, 4, 2)",
    );
    assert!((num(&c, "a") - -3.75).abs() < 1e-12, "{}", num(&c, "a"));
    assert!((num(&c, "b") - -2.75).abs() < 1e-12, "{}", num(&c, "b"));
    assert!(!defined(&c, "c"));
}

/// `CommandsTest.cmdIntegralBetween`.
#[test]
fn integral_between_of_equal_functions_is_zero() {
    let c = load("f(x) = sin(x)\ng(x) = sin(x)\nd = IntegralBetween(f, g, 4, 13)");
    assert!((num(&c, "d") - 0.0).abs() < 1e-9, "{}", num(&c, "d"));
    let c = load("f(x) = sin(x)\ng(x) = sin(x)\nd = IntegralBetween(f, g, 4, 13, false)");
    assert!(!defined(&c, "d"));
    // a number counts as the constant function it is
    let c = load("f(x) = x^2\nd = IntegralBetween(f, 3, 0, 4)");
    assert!(
        (num(&c, "d") - 9.333_333_333_333_332).abs() < 1e-12,
        "{}",
        num(&c, "d")
    );
}

/// `CommandsTest.cmdNIntegral`.
#[test]
fn nintegral_definite_matches_geogebra() {
    let c = load("f(x) = x^2\nd = NIntegral(f, -1, 1)");
    assert!(
        (num(&c, "d") - 0.666_666_666_666_666_6).abs() < 1e-12,
        "{}",
        num(&c, "d")
    );
}

/// `CommandsTest.cmdIntegral` (`CommandsUsingMockedCasTest.cmdIntegral`).
#[test]
fn definite_integral_of_a_polynomial_is_exact() {
    let c = load("f(x) = x\nd = Integral(f, 0, 4)");
    assert!((num(&c, "d") - 8.0).abs() < 1e-12, "{}", num(&c, "d"));
    let c = load("f(x) = x\nd = Integral(f, 0, 4, false)");
    assert!(!defined(&c, "d"));
    let c = load("f(x) = x^2\nd = Integral(f, 0, 4)");
    assert!(
        (num(&c, "d") - 21.333_333_333_333_332).abs() < 1e-12,
        "{}",
        num(&c, "d")
    );
}

/// `CommandsTest.piecewiseIntegration`.
#[test]
fn piecewise_integration_matches_geogebra() {
    let c = load("f(x) = If(x^2 > 1, 1, x > 7, 0, 0)\nd = Integral(f, -1, 1)");
    assert!(num(&c, "d").abs() < 1e-9, "{}", num(&c, "d"));
    let c = load("f(x) = If(x^2 > 1, 1, x > 7, 0, 0)\nd = Integral(f, -2, 2)");
    assert!((num(&c, "d") - 2.0).abs() < 1e-9, "{}", num(&c, "d"));
    let c = load("f(x) = If(x > 2, 1, 2)\nd = Integral(f, 0, 2.01)");
    assert!((num(&c, "d") - 4.01).abs() < 1e-9, "{}", num(&c, "d"));
    let c = load("f(x) = If(x^4 > 1, 1, 0)\nd = Integral(f, -2, 2)");
    assert!((num(&c, "d") - 2.0).abs() < 1e-9, "{}", num(&c, "d"));
    // `computeConditional` splits at the branch intervals: a jump the
    // plain quadrature samples would step over
    let c = load("f(x) = If(x < 1, x^2, If(x < 3, 4 - x, x - 3))\nd = Integral(f, 0, 5)");
    assert!(
        (num(&c, "d") - 6.333_333_333_333_333).abs() < 1e-9,
        "{}",
        num(&c, "d")
    );
    // a chain `1 < x < 3` is the AND of its comparisons
    let c = load("f(x) = If(1 < x < 3, 1, 0)\nd = Integral(f, 0, 4)");
    assert!((num(&c, "d") - 2.0).abs() < 1e-9, "{}", num(&c, "d"));
    // a reversed interval changes the sign of every branch piece
    let c = load("f(x) = If(x < 1, x, 2x)\nd = Integral(f, 4, 0)");
    assert!((num(&c, "d") - -15.5).abs() < 1e-9, "{}", num(&c, "d"));
    // `x == 2` is the interval [2, 2]: it integrates to zero
    let c = load("f(x) = If(x == 2, 1, 0)\nd = Integral(f, 0, 4)");
    assert_eq!(num(&c, "d"), 0.0);
    // `x != 2` is not a comparison the bounds match: the plain quadrature
    let c = load("f(x) = If(x != 2, 1, 0)\nd = Integral(f, 0, 4)");
    assert_eq!(num(&c, "d"), 4.0);
    // a conditional inside an expression is not a conditional function
    let c = load("f(x) = 3 If(x > 1, x, 2)\nd = Integral(f, 0, 2)");
    assert!(
        (num(&c, "d") - 10.499_999_999_999_996).abs() < 1e-9,
        "{}",
        num(&c, "d")
    );
    // the branch bounds move with a number the condition reads
    let mut c = load("c = 1\nf(x) = If(x < c, x, 2x)\nd = Integral(f, 0, 4)");
    assert!((num(&c, "d") - 15.5).abs() < 1e-9, "{}", num(&c, "d"));
    set_number(&mut c, "c", 2.0);
    assert!((num(&c, "d") - 14.0).abs() < 1e-9, "{}", num(&c, "d"));
    set_number(&mut c, "c", 0.0);
    assert!((num(&c, "d") - 16.0).abs() < 1e-9, "{}", num(&c, "d"));
    // a bound at the end of the interval
    set_number(&mut c, "c", 4.0);
    assert!((num(&c, "d") - 8.0).abs() < 1e-9, "{}", num(&c, "d"));
    // a bound outside the interval (the branch is not taken at all)
    set_number(&mut c, "c", -1.0);
    assert!((num(&c, "d") - 16.0).abs() < 1e-9, "{}", num(&c, "d"));
}

/// `AlgoIntegralDefinite.compute`: equal bounds give zero, a reversed
/// interval changes the sign, endpoints outside the domain make the whole
/// integral undefined when it is not computed numerically.
#[test]
fn definite_integral_bounds() {
    let c = load("f(x) = x^2\nd = Integral(f, 2, 2)");
    assert_eq!(num(&c, "d"), 0.0);
    let c = load("f(x) = x^2\nd = Integral(f, 4, 0)");
    assert!(
        (num(&c, "d") - -21.333_333_333_333_332).abs() < 1e-12,
        "{}",
        num(&c, "d")
    );
    // f(a) or f(b) undefined: the symbolic path asks the CAS and the number
    // stays undefined; NIntegral refuses it too.
    let c = load("f(x) = sqrt(x)\nd = Integral(f, -1, 2)");
    assert!(!defined(&c, "d"));
    let c = load("f(x) = sqrt(x)\nd = NIntegral(f, -1, 2)");
    assert!(!defined(&c, "d"));
}

/// The integral of a function that divides by its variable is numeric.
#[test]
fn integral_of_a_division_by_the_variable_is_numeric() {
    let c = load("f(x) = 1/x\nd = Integral(f, 1, 4)");
    assert!(
        (num(&c, "d") - 1.386_294_361_119_245).abs() < 1e-12,
        "{}",
        num(&c, "d")
    );
}

/// `CommandsUsingMockedCasTest.cmdIntegral`: the antiderivative of a
/// polynomial, with and without the variable argument.
#[test]
fn integral_of_a_polynomial_is_exact_and_the_variable_form_agrees() {
    let c = load("f(x) = x\ng = Integral(f)");
    assert_eq!(c.function_value(c.id("g").unwrap(), 4.0), Some(8.0));
    assert_eq!(c.function_value(c.id("g").unwrap(), -2.0), Some(2.0));
    let c = load("f(x) = x^2\ng = Integral(f, x)");
    assert_eq!(c.function_value(c.id("g").unwrap(), 3.0), Some(9.0));
}

/// `NIntegral(f)` is the rule integrator: `sin` gives `-cos`, a product it
/// cannot integrate is undefined, and the definite form stays numeric.
#[test]
fn nintegral_uses_the_rule_integrator() {
    let c = load("f(x) = sin(x)\ng = NIntegral(f)");
    let g = c.id("g").unwrap();
    assert_eq!(c.function_value(g, 0.0), Some(-1.0));
    assert!((c.function_value(g, 1.0).unwrap() - -0.540_302_305_868_139_8).abs() < 1e-12);
    let c = load("f(x) = x*sin(x)\ng = NIntegral(f)");
    assert!(!defined(&c, "g"));
    // 1/x integrates to ln(x) by the rules; the definite integral of a
    // division by the variable is numeric on both sides of the pole.
    let c = load("f(x) = 1/x\ng = NIntegral(f)");
    let g = c.id("g").unwrap();
    assert!((c.function_value(g, 4.0).unwrap() - 1.386_294_361_119_890_6).abs() < 1e-9);
    let c = load("f(x) = 1/x\nd = NIntegral(f, -1, 1)");
    assert!(num(&c, "d").abs() < 1e-9, "{}", num(&c, "d"));
}

/// `CommandsUsingMockedCasTest.cmdDerivative`.
#[test]
fn derivative_of_a_polynomial_is_exact() {
    let c = load("f(x) = x^2\ng = Derivative(f)");
    let g = c.id("g").unwrap();
    assert_eq!(c.function_value(g, 3.0), Some(6.0));
    assert_eq!(c.function_value(g, -2.0), Some(-4.0));
    let c = load("f(x) = x^3\ng = Derivative(f, 2)");
    assert_eq!(c.function_value(c.id("g").unwrap(), 1.0), Some(6.0));
    let c = load("f(x) = x^3\ng = Derivative(f, x)");
    assert_eq!(c.function_value(c.id("g").unwrap(), 2.0), Some(12.0));
    let c = load("f(x) = x^3\ng = Derivative(f, x, 2)");
    assert_eq!(c.function_value(c.id("g").unwrap(), 1.0), Some(6.0));
    // higher orders are exact too, past the degree they are zero
    // (`PolyFunction.derivative` shrinks the coefficient array)
    let c = load("f(x) = x^2\nh = Derivative(f, 2)\ni = Derivative(f, 3)");
    assert_eq!(c.function_value(c.id("h").unwrap(), -8.0), Some(2.0));
    assert_eq!(c.function_value(c.id("h").unwrap(), 3.0), Some(2.0));
    assert_eq!(c.function_value(c.id("i").unwrap(), -8.0), Some(0.0));
    let c = load("f(x) = x^3 - 6x^2 + 9x\nv = NDerivative(f, 3)");
    assert_eq!(c.function_value(c.id("v").unwrap(), -8.0), Some(6.0));
    // the zero polynomial is the constant zero, not undefined
    let c = load("a = 0\nf(x) = a x^3\ng = Derivative(f)");
    assert_eq!(c.function_value(c.id("g").unwrap(), 5.0), Some(0.0));
    // the derivative of a constant is zero everywhere
    let c = load("f(x) = 3\ng = Derivative(f)\nh = NDerivative(f)");
    assert_eq!(c.function_value(c.id("g").unwrap(), 5.0), Some(0.0));
    assert_eq!(c.function_value(c.id("h").unwrap(), 5.0), Some(0.0));
    // the slider order rounds like `Math.round` and rebuilds
    let mut c = load("n = 1\nf(x) = x^2\ng = Derivative(f, n)");
    assert_eq!(c.function_value(c.id("g").unwrap(), -8.0), Some(-16.0));
    set_number(&mut c, "n", 0.0);
    assert_eq!(c.function_value(c.id("g").unwrap(), -8.0), Some(64.0));
    set_number(&mut c, "n", 2.0);
    assert_eq!(c.function_value(c.id("g").unwrap(), -8.0), Some(2.0));
    set_number(&mut c, "n", 2.6);
    assert_eq!(c.function_value(c.id("g").unwrap(), -8.0), Some(0.0));
    set_number(&mut c, "n", 0.4);
    assert_eq!(c.function_value(c.id("g").unwrap(), -8.0), Some(64.0));
    set_number(&mut c, "n", -1.0);
    assert!(!defined(&c, "g"));
}

/// Without the CAS only polynomials differentiate; everything else is
/// undefined. `NDerivative` walks the rules instead.
#[test]
fn derivative_without_the_cas_and_nderivative() {
    let c = load("f(x) = sin(x)\ng = Derivative(f)");
    assert!(!defined(&c, "g"));
    let c = load("f(x) = sin(x)\ng = NDerivative(f)");
    let g = c.id("g").unwrap();
    assert_eq!(c.function_value(g, 0.0), Some(1.0));
    assert!((c.function_value(g, 1.0).unwrap() - 0.540_302_305_868_139_8).abs() < 1e-12);
    assert!((c.function_value(g, 8.0).unwrap() - -0.145_500_033_809_161_13).abs() < 1e-12);
    // a polynomial takes the exact path in NDerivative too
    let c = load("f(x) = x^2\ng = NDerivative(f)");
    assert_eq!(c.function_value(c.id("g").unwrap(), 3.0), Some(6.0));
    // the order rounds like GeoGebra; below zero is undefined, zero copies
    let mut c = load("f(x) = x^3\nn = 2\ng = NDerivative(f, n)");
    set_number(&mut c, "n", 0.0);
    assert_eq!(c.function_value(c.id("g").unwrap(), 5.0), Some(125.0));
    set_number(&mut c, "n", -1.0);
    assert!(!defined(&c, "g"));
    set_number(&mut c, "n", 1.0);
    assert_eq!(c.function_value(c.id("g").unwrap(), 5.0), Some(75.0));
}

/// `CommandsTest.productDegree`.
#[test]
fn degree_of_a_function() {
    let c = load("f(x) = x\nd = Degree(f)");
    assert_eq!(num(&c, "d"), 1.0);
    let c = load("f(x) = sin(x)\nd = Degree(f)");
    assert!(!defined(&c, "d"));
}

/// `CommandsTest.cmdCoefficients` (the function forms this kernel has).
#[test]
fn coefficients_of_a_function() {
    let coefficients = |script: &str| -> Vec<f64> {
        let c = load(script);
        c.elements(c.id("c").unwrap())
            .unwrap()
            .iter()
            .map(|item| match item {
                geo_kernel::expr::Element::Value(_, geo_kernel::Value::Number(v)) => v,
                _ => f64::NAN,
            })
            .collect()
    };
    assert_eq!(
        coefficients("f(x) = x^2\nc = Coefficients(f)"),
        vec![1.0, 0.0, 0.0]
    );
    // not a polynomial: the CAS-less fallback answers the number
    // literals of the tree (`extractConstants`)
    assert_eq!(
        coefficients("f(x) = 7x^3 + sin(x)\nc = Coefficients(f)"),
        vec![7.0, 3.0]
    );
    assert_eq!(
        coefficients("f(x) = x^3/6 + x^2/4 + 3\nc = Coefficients(f)"),
        vec![0.166_666_666_666_666_66, 0.25, 0.0, 3.0]
    );
    assert_eq!(
        coefficients("f(x) = sin(x)\nc = Coefficients(f)"),
        Vec::<f64>::new()
    );
}

/// `CommandsTest.cmdNumerator` and `cmdDenominator`: the exact-fraction
/// path reduces integers, the decimal path finds the closest fraction.
#[test]
fn numerator_and_denominator_of_numbers() {
    let script = |expr: &str| format!("a = {expr}\nn = Numerator(a)\nd = Denominator(a)");
    // the exact tree of an unreduced fraction with huge integers is not
    // kept on a free number (a known gap); the closest fraction of the
    // value answers instead
    let cases = [
        ("3/7", 3.0, 7.0),
        ("5/(-8)", -5.0, 8.0),
        ("2/0", 1.0, 0.0),
        ("1234/5678", 617.0, 2839.0),
        ("1/(-3)", -1.0, 3.0),
        ("0", 0.0, 1.0),
    ];
    for (expr, n, d) in cases {
        let c = load(&script(expr));
        assert_eq!(num(&c, "n"), n, "{expr}");
        assert_eq!(num(&c, "d"), d, "{expr}");
    }
    let c = load("a = 10/6\nn = Numerator(a)\nd = Denominator(a)");
    assert_eq!((num(&c, "n"), num(&c, "d")), (5.0, 3.0));
    let c = load("a = -10/6\nn = Numerator(a)\nd = Denominator(a)");
    assert_eq!((num(&c, "n"), num(&c, "d")), (-5.0, 3.0));
    let c = load("a = -10/-6\nn = Numerator(a)\nd = Denominator(a)");
    assert_eq!((num(&c, "n"), num(&c, "d")), (5.0, 3.0));
    let c = load("a = 0.125/3\nn = Numerator(a)\nd = Denominator(a)");
    assert_eq!((num(&c, "n"), num(&c, "d")), (1.0, 24.0));
    let c = load("a = 3/0.166666666666666666\nn = Numerator(a)\nd = Denominator(a)");
    assert_eq!((num(&c, "n"), num(&c, "d")), (18.0, 1.0));
    // infinity: AlgoFractionText gives (1, 0) and (-1, 0)
    let c = load("a = infinity\nn = Numerator(a)\nd = Denominator(a)");
    assert_eq!((num(&c, "n"), num(&c, "d")), (1.0, 0.0));
}

/// `CommandsTest.cmdNumerator` and `cmdDenominator`: the function form
/// splits the fraction tree.
#[test]
fn numerator_and_denominator_of_functions() {
    let c = load("f(x) = (x + 2)/(x + 1)\nn = Numerator(f)\nd = Denominator(f)");
    let (n, d) = (c.id("n").unwrap(), c.id("d").unwrap());
    assert_eq!(c.function_value(n, 3.0), Some(5.0));
    assert_eq!(c.function_value(d, 3.0), Some(4.0));
    let c = load("f(x) = x^2 + 1\nn = Numerator(f)\nd = Denominator(f)");
    assert_eq!(c.function_value(c.id("n").unwrap(), 2.0), Some(5.0));
    assert_eq!(c.function_value(c.id("d").unwrap(), 2.0), Some(1.0));
}

/// `CommandsTest.cmdPolynomial`.
#[test]
fn polynomial_of_a_function() {
    let c = load("g = Polynomial(sin(x))");
    assert!(!defined(&c, "g"));
    let value = |script: &str, x: f64| {
        let c = load(script);
        c.function_value(c.id("g").unwrap(), x)
    };
    assert_eq!(value("g = Polynomial(1x^2 - 1x + 1)", 3.0), Some(7.0));
    assert_eq!(value("g = Polynomial(-x*(x + 1)*(x - 1))", 2.0), Some(-6.0));
    assert_eq!(value("g = Polynomial((2x + 3)^3)", 1.0), Some(125.0));
    assert_eq!(value("g = Polynomial((2x + 3)^3)", 0.0), Some(27.0));
}

/// `CommandsTest.cmdPolynomial` through points and lists.
#[test]
fn polynomial_through_points() {
    let c = load("g = Polynomial({(1, 1), (-1, 1), (0, 0)})");
    assert_eq!(c.function_value(c.id("g").unwrap(), 2.0), Some(4.0));
    let c = load("g = Polynomial({(1, 0), (-1, 2), (0, 0)})");
    let g = c.id("g").unwrap();
    assert_eq!(c.function_value(g, 2.0), Some(2.0));
    assert_eq!(c.function_value(g, 3.0), Some(6.0));
    let c = load("P = (1, 1)\nQ = (-1, 1)\nR = (0, 0)\ng = Polynomial(P, Q, R)");
    assert_eq!(c.function_value(c.id("g").unwrap(), 3.0), Some(9.0));
    // fewer than two points, or an x repeated with two different y, is
    // undefined
    let c = load("g = Polynomial({(1, 1)})");
    assert!(!defined(&c, "g"));
    let c = load("g = Polynomial({(1, 1), (1, 2)})");
    assert!(!defined(&c, "g"));
    // two equal points collapse to one
    let c = load("g = Polynomial({(1, 1), (1, 1), (3, 3)})");
    let g = c.id("g").unwrap();
    assert_eq!(c.function_value(g, 1.0), Some(1.0));
    assert_eq!(c.function_value(g, 3.0), Some(3.0));
}

/// `Function.getDerivative` checks its factor only inside the polynomial
/// branch, so the derivative of a non-polynomial with a high power needs
/// the CAS and stays undefined, while `NDerivative` keeps the rule tree
/// (`calculus-derivative-022` to `-025`).
#[test]
fn derivative_of_a_high_power_of_a_non_polynomial() {
    for script in [
        "f(x) = sin(x)^11",
        "f(x) = (x + sin(x))^12",
        "f(x) = (1/x + 1)^11",
    ] {
        let c = load(&format!("{script}\ng = Derivative(f)"));
        assert!(!defined(&c, "g"), "{script}");
    }
    let c = load("f(x) = sin(x)^11\nh = NDerivative(f)");
    let h = c.id("h").unwrap();
    // 11 cos(x) sin(x)^10
    assert!((c.function_value(h, 1.0).unwrap() - 1.057_842_376_392_636_1).abs() < 1e-9);
    // a polynomial base with a high power routes around the expansion:
    // the rule tree 11 (2x + 3)^10 * 2
    let c = load("f(x) = (2x + 3)^11\ng = Derivative(f)");
    let g = c.id("g").unwrap();
    assert_eq!(c.function_value(g, -2.0), Some(22.0));
    assert_eq!(c.function_value(g, -1.0), Some(22.0));
}

/// An object exponent expands with its truncated `(int)` power while its
/// value is a whole number not below zero, and the variable degree makes
/// the result no polynomial otherwise (`calculus-degree-008`,
/// `calculus-derivative-026`, `calculus-integral-019`,
/// `calculus-coefficients-010`, `calculus-polynomial-016`).
#[test]
fn an_object_exponent_expands_with_its_value() {
    let mut c = load("a = 3\nf(x) = x^a\nd = Degree(f)\ng = Derivative(f)\nc = Coefficients(f)");
    assert_eq!(num(&c, "d"), 3.0);
    assert_eq!(c.function_value(c.id("g").unwrap(), 2.0), Some(12.0));
    // a value within the integer tolerance of one below truncates
    set_number(&mut c, "a", 2.999_999_999_9);
    assert_eq!(num(&c, "d"), 2.0);
    set_number(&mut c, "a", 4.0);
    assert_eq!(num(&c, "d"), 4.0);
    // a fractional value is no polynomial: Degree, Derivative (without
    // the CAS) and the expansion are undefined, the coefficients fall
    // back to the (empty) literal list, NDerivative walks the rules and
    // the definite integrals are numeric
    set_number(&mut c, "a", 2.5);
    assert!(!defined(&c, "d"));
    assert!(!defined(&c, "g"));
    assert_eq!(c.elements(c.id("c").unwrap()).unwrap().len(), 0);
    let c = load("a = 2.5\nf(x) = x^a\nd = Degree(f)\ng = Polynomial(f)");
    assert!(!defined(&c, "d"));
    assert!(!defined(&c, "g"));
    let c = load("a = 2.5\nf(x) = x^a\nd = Integral(f, 0, 2)\ne = NIntegral(f, 0, 2)");
    assert!((num(&c, "d") - 3.232_488_142_777_426_4).abs() < 1e-9);
    assert!((num(&c, "e") - 3.232_488_142_777_426_4).abs() < 1e-9);
    let c = load("a = 3\nf(x) = x^a\nd = Integral(f, 0, 2)\ne = NIntegral(f, 0, 2)");
    assert_eq!(num(&c, "d"), 4.0);
    assert_eq!(num(&c, "e"), 4.0);
    let c = load("a = 3\nf(x) = x^a\ng = Polynomial(f)");
    assert_eq!(c.function_value(c.id("g").unwrap(), 2.0), Some(8.0));
}

/// A number object counts as a number in the no-CAS rules
/// (`isNumberValue`), so a constant factor or divisor comes out instead
/// of falling into the product or quotient rule
/// (`calculus-derivative-027`, `calculus-derivative-028`,
/// `calculus-integral-020`).
#[test]
fn object_constants_are_numbers_in_the_rules() {
    let c = load("a = 2\nf(x) = a*ln(x)\ng = NDerivative(f)");
    let g = c.id("g").unwrap();
    // (1/x) a: defined wherever the rule tree is, also at negative x
    assert_eq!(c.function_value(g, -8.0), Some(-0.25));
    assert_eq!(c.function_value(g, 0.5), Some(4.0));
    let c = load("a = 2\nf(x) = ln(x)/a\ng = NDerivative(f)");
    let g = c.id("g").unwrap();
    assert_eq!(c.function_value(g, -8.0), Some(-0.062_5));
    assert_eq!(c.function_value(g, 0.5), Some(1.0));
    // an object exponent: x^(a-1) * a, so at -1 with a = 3 it is 3
    let c = load("a = 3\nf(x) = x^a\ng = NDerivative(f)");
    assert_eq!(c.function_value(c.id("g").unwrap(), -1.0), Some(3.0));
    // the rule integral moves a constant factor out as well
    let c = load("a = 2\nf(x) = a*ln(x)\ng = NIntegral(f)");
    let g = c.id("g").unwrap();
    assert_eq!(c.function_value(g, 1.0), Some(-2.0));
    assert!((c.function_value(g, 0.5).unwrap() - -1.693_147_180_559_945_4).abs() < 1e-12);
}

/// `AlgoCoefficients.extractConstants` walks the tree, not the parser's
/// arena: the literals of what the function wrote (`calculus-coefficients-009`).
#[test]
fn coefficients_of_a_non_polynomial_walk_the_tree() {
    let c = load("f(x) = sin(x) + (-2)\nc = Coefficients(f)");
    let elements: Vec<f64> = c
        .elements(c.id("c").unwrap())
        .unwrap()
        .iter()
        .map(|item| match item {
            geo_kernel::expr::Element::Value(_, geo_kernel::Value::Number(v)) => v,
            _ => f64::NAN,
        })
        .collect();
    assert_eq!(elements, vec![-2.0]);
}

/// `AlgoIntegralDefinite.compute`: the numerical section integrates a
/// polynomial exactly (`getNumericPolynomialIntegral`) for `NIntegral`
/// too (`calculus-integral-018`). The adaptive quadrature the numeric
/// command used to run instead drifts past the tolerance from about the
/// 12th power and gives up past the 16th.
#[test]
fn nintegral_of_a_polynomial_is_exact() {
    let c = load("f(x) = x^12\nd = NIntegral(f, 0, 5)\ne = Integral(f, 0, 5)");
    let exact = 5.0_f64.powi(13) / 13.0;
    assert!((num(&c, "d") - exact).abs() < 1e-9, "{}", num(&c, "d"));
    assert!(
        (num(&c, "e") - 93_900_240.384_615_39).abs() < 1e-6,
        "{}",
        num(&c, "e")
    );
    // past the 16th power the quadrature used to answer nothing at all
    let c = load("f(x) = x^16\nd = NIntegral(f, 0, 5)");
    let exact = 5.0_f64.powi(17) / 17.0;
    assert!(
        (num(&c, "d") - exact).abs() < 1e-9 * exact.abs(),
        "{}",
        num(&c, "d")
    );
}

/// `AlgoFunctionAreaSums`: a subdivision count over `MAX_RECTANGLES`
/// clamps to it — an infinite `n` is a defined `GeoNumeric` (`isDefined`
/// is `!isNaN`) — and an infinite bound makes the total infinite, which
/// the kernel has no object value for (`calculus-sums-012`,
/// `calculus-sums-013`).
#[test]
fn sums_read_an_infinite_subdivision_count_and_bound() {
    let c = load("f(x) = 1\nb = 1/0\nd = LeftSum(f, 0, b, 4)\nu = UpperSum(f, 0, b, 4)");
    assert!(!defined(&c, "d"));
    assert!(!defined(&c, "u"));
    let c = load("f(x) = 1\nn = 1/0\nd = LeftSum(f, 0, 2, n)\ne = LeftSum(f, 2, 0, n)");
    assert_eq!(num(&c, "d"), 2.0);
    assert_eq!(num(&c, "e"), -2.0);
}

/// The loops that finish one subtree per exponent or coefficient stay
/// inside the work budget and bounded memory: a sum of thousands of
/// powers, a degree-300 polynomial of slider coefficients, and a high
/// derivative order all answer (the last as undefined, past the budget).
#[test]
fn polynomial_work_stays_inside_the_budget() {
    // a sum of 3000 (x + 1)^2 terms, parenthesised into a balanced tree
    // so the line stays inside the parser's depth bound: the degree check
    // finishes each of the 3000 exponents without cloning the whole arena
    // per one
    let balanced = |terms: &[&str]| -> String {
        fn join(terms: &[&str]) -> String {
            match terms {
                [one] => (*one).into(),
                _ => {
                    let half = terms.len() / 2;
                    format!("({} + {})", join(&terms[..half]), join(&terms[half..]))
                }
            }
        }
        join(terms)
    };
    let terms: Vec<&str> = vec!["(x + 1)^2"; 3000];
    let script = format!(
        "f(x) = {}\nd = Degree(f)\ng = NDerivative(f)",
        balanced(&terms)
    );
    let c = load(&script);
    assert_eq!(num(&c, "d"), 2.0);
    assert_eq!(c.function_value(c.id("g").unwrap(), 3.0), Some(24000.0));
    // (a x + b)^300: `Polynomial.apply` allows the exponent (the degree
    // bound is 300), and the univariate expansion has only 301 terms, so
    // the term cap of the builder (`MAX_TERMS`, a bound GeoGebra does not
    // have) is checked on the merged terms and the power answers, as in
    // GeoGebra, with the coefficient values read once each (a memo over
    // the builder's shared coefficient trees) in bounded time and memory
    let t = std::time::Instant::now();
    let c = load("a = 1\nb = 1\nf(x) = (a x + b)^300\nd = Degree(f)\ng = Derivative(f)");
    assert_eq!(num(&c, "d"), 300.0);
    assert!(defined(&c, "g"));
    assert!(t.elapsed().as_secs() < 20, "{}s", t.elapsed().as_secs());
    // the 25th rule derivative of a product triples its tree per pass;
    // the work budget stops it instead of the memory growing without end
    let c = load("f(x) = x*sin(x)*cos(x)\nh = NDerivative(f, 25)");
    assert!(!defined(&c, "h"));
}
