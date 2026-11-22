use super::{system::Equation, *};

fn rational(num: i128, den: i128) -> Rat {
    Rat::new(num, den).unwrap()
}

fn ints(values: &[i128]) -> Vec<Rat> {
    values.iter().map(|&value| rational(value, 1)).collect()
}

fn poly(terms: &[([u8; 3], i128, i128)]) -> Equation {
    terms
        .iter()
        .map(|&(degree, num, den)| (degree, rational(num, den)))
        .collect()
}

fn assert_roots(coefficients: &[i128], expected: &[f64]) {
    let mut work = Work::new(&|_| true);
    let Some(Roots::Finite(roots)) = real_roots(&ints(coefficients), &mut work) else {
        panic!("expected a finite real root set for {coefficients:?}");
    };
    assert_eq!(roots.len(), expected.len(), "{roots:?}");
    for (root, &expected) in roots.iter().zip(expected) {
        assert!(
            (root.value() - expected).abs() < 1e-9,
            "{root:?} != {expected}"
        );
    }
}

#[test]
fn upstream_solve_and_nsolve() {
    // CommandsUsingMockedCasTest.java:154–172, shared/common-jre.
    assert_roots(&[0, -1, 0, 1], &[-1.0, 0.0, 1.0]);
    // CommandsUsingCASTest.java:60–65, desktop/desktop.
    assert_roots(&[-3, 0, 1], &[-3.0_f64.sqrt(), 3.0_f64.sqrt()]);
    assert_roots(&[1, 0, 1], &[]);
    let mut work = Work::new(&|_| true);
    assert!(matches!(
        real_roots(&ints(&[0]), &mut work),
        Some(Roots::All)
    ));
    // CommandFilterCASTest.java:43–44, desktop/desktop.
    assert_roots(&[1, -2, 0, 0, 0, 0, 1], &[0.508_660_391_642, 1.0]);
    assert_roots(
        &[-34, 0, 0, 34, 1],
        &[-34.000_864_985_88, 0.990_473_888_557_4],
    );
}

#[test]
fn repeated_and_close_roots_are_distinct_solutions() {
    assert_roots(&[8, -36, 66, -63, 33, -9, 1], &[1.0, 2.0]);
    let mut work = Work::new(&|_| true);
    let scale = 1_000_000_000_i128;
    let coefficients = ints(&[scale + 1, -2 * scale - 1, scale]);
    let expected = [
        Root::Rational(Rat::ONE),
        Root::Rational(rational(scale + 1, scale)),
    ];
    assert_eq!(
        real_roots(&coefficients, &mut work),
        Some(Roots::Finite(expected.to_vec()))
    );
}

#[test]
fn exact_nth_root_is_kept() {
    let mut work = Work::new(&|_| true);
    let Some(Roots::Finite(roots)) = real_roots(&ints(&[2, 0, 0, 1]), &mut work) else {
        panic!("expected a finite real root set");
    };
    assert!(matches!(
        roots.as_slice(),
        [Root::Radical { degree: 3, .. }]
    ));
    assert!((roots[0].value() + 2.0_f64.cbrt()).abs() < 1e-9);
}

#[test]
fn upstream_polynomial_systems() {
    // CommandsUsingCASTest.java:62 and :396, desktop/desktop.
    let linear = [
        poly(&[([0, 0, 0], -1, 1), ([1, 0, 0], 1, 1), ([0, 1, 0], 1, 1)]),
        poly(&[([1, 0, 0], 1, 1), ([0, 1, 0], -1, 1)]),
    ];
    let powers = [
        poly(&[([0, 0, 0], -8436, 100), ([1, 3, 0], 1, 1)]),
        poly(&[([0, 0, 0], -12656, 100), ([1, 4, 0], 1, 1)]),
    ];
    for (equations, expected) in [
        (linear, [rational(1, 2), rational(1, 2)]),
        (
            powers,
            [rational(19783645390161, 791861873600), rational(3164, 2109)],
        ),
    ] {
        let mut work = Work::new(&|_| true);
        assert_eq!(
            systems(&equations, 2, &mut work),
            Some(SystemRoots::Finite(vec![expected
                .map(Root::Rational)
                .to_vec()]))
        );
    }
}

#[test]
fn degenerate_systems_are_not_fabricated_points() {
    let x = poly(&[([1, 0, 0], 1, 1)]);
    let x_minus_one = poly(&[([1, 0, 0], 1, 1), ([0, 0, 0], -1, 1)]);
    let mut work = Work::new(&|_| true);
    assert_eq!(
        systems(&[x.clone(), x_minus_one], 2, &mut work),
        Some(SystemRoots::Finite(vec![]))
    );
    assert_eq!(systems(&[x], 2, &mut work), Some(SystemRoots::Infinite));
}

#[test]
fn nonlinear_systems_and_quadratic_extensions() {
    let sum = poly(&[([0, 0, 0], -5, 1), ([2, 0, 0], 1, 1), ([0, 2, 0], 1, 1)]);
    let product = poly(&[([0, 0, 0], -2, 1), ([1, 1, 0], 1, 1)]);
    let mut work = Work::new(&|_| true);
    let Some(SystemRoots::Finite(rows)) = systems(&[sum, product], 2, &mut work) else {
        panic!("expected four solutions");
    };
    assert_eq!(rows.len(), 4);
    for (row, pair) in rows
        .iter()
        .zip([[-2.0, -1.0], [-1.0, -2.0], [1.0, 2.0], [2.0, 1.0]])
    {
        assert!(row
            .iter()
            .zip(pair)
            .all(|(root, coordinate)| (root.value() - coordinate).abs() < 1e-9));
    }
    let diagonal = poly(&[([1, 0, 0], 1, 1), ([0, 1, 0], -1, 1)]);
    let square = poly(&[([0, 0, 0], -2, 1), ([0, 2, 0], 1, 1)]);
    let Some(SystemRoots::Finite(rows)) = systems(&[diagonal, square], 2, &mut work) else {
        panic!("expected two exact quadratic solutions");
    };
    assert_eq!(rows.len(), 2);
    for (row, expected) in rows.iter().zip([-2.0_f64.sqrt(), 2.0_f64.sqrt()]) {
        assert!(row
            .iter()
            .all(|root| matches!(root, Root::Radical { degree: 2, .. })
                && (root.value() - expected).abs() < 1e-9));
    }
}

#[test]
fn shifted_pure_powers_keep_exact_radicals() {
    let mut work = Work::new(&|_| true);
    let Some(Roots::Finite(roots)) = real_roots(&ints(&[-3, 3, -3, 1]), &mut work) else {
        panic!("expected one translated cubic root");
    };
    assert!(matches!(
        roots.as_slice(),
        [Root::Radical {
            offset: Rat::ONE,
            degree: 3,
            ..
        }]
    ));
    assert!((roots[0].value() - (1.0 + 2.0_f64.cbrt())).abs() < 1e-9);
    let Some(Roots::Finite(roots)) = real_roots(&ints(&[-1, -4, 6, -4, 1]), &mut work) else {
        panic!("expected two translated quartic roots");
    };
    assert!(matches!(
        roots.as_slice(),
        [
            Root::Radical {
                offset: Rat::ONE,
                degree: 4,
                ..
            },
            Root::Radical {
                offset: Rat::ONE,
                degree: 4,
                ..
            }
        ]
    ));
}

#[test]
fn dependent_affine_equations_keep_their_free_parameter() {
    let first = poly(&[([1, 0, 0], 1, 1), ([0, 1, 0], 1, 1), ([0, 0, 0], -1, 1)]);
    let second = poly(&[([1, 0, 0], 2, 1), ([0, 1, 0], 2, 1), ([0, 0, 0], -2, 1)]);
    let mut work = Work::new(&|_| true);
    assert_eq!(
        affine_system(&[first, second], 2, &mut work),
        Some(vec![ints(&[1, -1]), ints(&[0, 1])])
    );
    let fixed_y = poly(&[([0, 1, 0], 1, 1), ([0, 0, 0], -2, 1)]);
    assert_eq!(
        affine_system(std::slice::from_ref(&fixed_y), 2, &mut work),
        Some(vec![ints(&[0, 1]), ints(&[2])])
    );
    assert_eq!(affine_system(&[fixed_y], 3, &mut work), None);
}

#[test]
fn quadratic_compositions_keep_nested_radicals() {
    // The native reference gives radicals for both powers of x and powers of x - 1.
    for (polynomial, expected, outer_degree) in [
        (
            ints(&[-1, 0, -1, 0, 1]),
            [-1.272_019_649_514_069, 1.272_019_649_514_069],
            2,
        ),
        (
            ints(&[-1, -2, 5, -4, 1]),
            [-0.272_019_649_514_069, 2.272_019_649_514_069],
            2,
        ),
        (
            ints(&[-1, 0, 0, -1, 0, 0, 1]),
            [-0.851_799_642_079_242_9, 1.173_984_996_705_328_4],
            3,
        ),
    ] {
        let mut work = Work::new(&|_| true);
        let Some(Roots::Finite(roots)) = real_roots(&polynomial, &mut work) else {
            panic!("expected two real nested radicals");
        };
        assert_eq!(roots.len(), expected.len());
        for (root, coordinate) in roots.into_iter().zip(expected) {
            assert!(matches!(root, Root::NestedRadical { degree, .. } if degree == outer_degree));
            assert!((root.value() - coordinate).abs() < 1e-9);
        }
    }
}

#[test]
fn positive_polynomial_proves_system_has_no_real_solution() {
    let impossible = poly(&[([0, 0, 0], 1, 1), ([2, 0, 0], 1, 1), ([0, 2, 0], 1, 1)]);
    let mut work = Work::new(&|_| true);
    assert_eq!(
        systems(&[impossible], 2, &mut work),
        Some(SystemRoots::Finite(vec![]))
    );
}

#[test]
fn large_coefficients_keep_tiny_roots_and_large_exact_radicands() {
    let huge = 10_i128.pow(38);
    let mut work = Work::new(&|_| true);
    assert_eq!(
        real_roots(&ints(&[-huge, 0, 1]), &mut work),
        Some(Roots::Finite(vec![
            Root::Rational(rational(-10_i128.pow(19), 1)),
            Root::Rational(rational(10_i128.pow(19), 1))
        ]))
    );
    let Some(Roots::Finite(roots)) = real_roots(&ints(&[1, huge, 1]), &mut work) else {
        panic!("expected a large root and a small root");
    };
    assert_eq!(roots.len(), 2);
    assert!((roots[0].value() / 1e38 + 1.0).abs() < 1e-9);
    assert!((roots[1].value() * 1e38 + 1.0).abs() < 1e-9);
}

#[test]
#[allow(clippy::excessive_precision)] // exact f64 bit patterns of the roots
fn wilkinson_products_stay_correct_to_the_last_double_bit() {
    // product(x - i, i = 1..n) + 1: the reference harness answers every root
    // as the 12-digit rounding of these values, so anything but full double
    // precision (the roots sit up to 4.1e-13 away from the integers) shows.
    for (degree, expected) in [
        (
            18_usize,
            &[
                1.000_000_000_000_002_9,
                1.999_999_999_999_952_3,
                3.000_000_000_000_382_4,
                3.999_999_999_998_088_2,
                5.000_000_000_006_691_5,
                5.999_999_999_982_602_4,
                7.000_000_000_034_794_4,
                7.999_999_999_945_322_8,
                9.000_000_000_068_347_1,
                9.999_999_999_931_652_9,
                11.000_000_000_054_678,
                11.999_999_999_965_205,
                13.000_000_000_017_398,
                13.999_999_999_993_308,
                15.000_000_000_001_911,
                15.999_999_999_999_618,
                17.000_000_000_000_046,
                17.999_999_999_999_996,
            ][..],
        ),
        (
            20,
            &[
                1.0,
                1.999_999_999_999_999_8,
                3.000_000_000_000_001_3,
                3.999_999_999_999_992,
                5.000_000_000_000_032,
                5.999_999_999_999_904_1,
                7.000_000_000_000_222_9,
                7.999_999_999_999_586_1,
                9.000_000_000_000_621_7,
                9.999_999_999_999_239_7,
                11.000_000_000_000_76,
                11.999_999_999_999_378,
                13.000_000_000_000_414,
                13.999_999_999_999_776,
                15.000_000_000_000_096,
                15.999_999_999_999_968,
                17.000_000_000_000_007,
                18.0,
                19.0,
                20.0,
            ][..],
        ),
    ] {
        let scale = |value: i128| rational(value, 1);
        let mut polynomial = vec![Rat::ONE];
        for root in 1..=degree {
            let mut next = vec![Rat::ZERO; polynomial.len() + 1];
            for (i, &coefficient) in polynomial.iter().enumerate() {
                next[i] = next[i]
                    .add(coefficient.mul(scale(-(root as i128))).unwrap())
                    .unwrap();
                next[i + 1] = next[i + 1].add(coefficient).unwrap();
            }
            polynomial = next;
        }
        polynomial[0] = polynomial[0].add(Rat::ONE).unwrap();
        let mut work = Work::new(&|_| true);
        let Some(Roots::Finite(roots)) = real_roots(&polynomial, &mut work) else {
            panic!("expected {degree} numeric roots");
        };
        assert_eq!(roots.len(), expected.len());
        for (root, exact) in roots.iter().zip(expected) {
            assert!(
                (root.value() - exact).abs() <= 4.0 * f64::EPSILON * exact.abs().max(1.0),
                "root {} is not {exact} to double precision",
                root.value()
            );
        }
    }
}

#[test]
fn a_guided_root_follows_the_newton_iteration() {
    let rats = |values: &[i128]| -> Vec<Rat> { values.iter().map(|&v| rational(v, 1)).collect() };
    let guided = |coefficients: &[i128], start: f64| {
        super::guided_root(&rats(coefficients), start, &mut Work::new(&|_| true))
    };
    let near = |got: Option<f64>, want: f64| {
        let got = got.unwrap_or_else(|| panic!("no root near {want}"));
        assert!((got - want).abs() < 1e-9, "{got} is not {want}");
    };
    // x^3 - x, start values recorded in golden/cas/hint.json.
    let cubic = [0, -1, 0, 1];
    for (start, root) in [
        (0.4, 0.0),
        (0.5, -1.0),
        (2.0, 1.0),
        (-0.45, 0.0),
        (100.0, 1.0),
        (0.0, 0.0),
        (1.0, 1.0),
    ] {
        near(guided(&cubic, start), root);
    }
    // x^5 - 3x + 1
    let quintic = [1, -3, 0, 0, 0, 1];
    near(guided(&quintic, 0.0), 0.334_734_141_943_4);
    near(guided(&quintic, 1.5), 1.214_648_042_698);
    assert_eq!(guided(&quintic, 0.9), None);
    // No real root, a vanishing derivative, a constant, a start that is no
    // number: undefined.
    assert_eq!(guided(&[1, 0, 1], 1.0), None);
    assert_eq!(guided(&[-3, 0, 1], 0.0), None);
    assert_eq!(guided(&[5], 1.0), None);
    assert_eq!(guided(&[-3, 0, 1], f64::NAN), None);
    assert_eq!(guided(&[-3, 0, 1], f64::INFINITY), None);
}

#[test]
fn a_guided_root_gives_up_where_the_iteration_is_not_plain() {
    let rats = |values: &[i128]| -> Vec<Rat> { values.iter().map(|&v| rational(v, 1)).collect() };
    let guided = |coefficients: &[i128], start: f64| {
        super::guided_root(&rats(coefficients), start, &mut Work::new(&|_| true))
    };
    // x^2 - 3 from -0.001: the first step lands at 1500 and needs ten
    // halvings to lower the residual.
    assert_eq!(guided(&[-3, 0, 1], -0.001), None);
    assert!(guided(&[-3, 0, 1], -1.0).is_some());
    // A double root is approached at a linear rate, where the reference
    // stops at its own tolerance.
    assert_eq!(guided(&[0, 0, 1], 1.0), None);
    assert_eq!(guided(&[-4, -7, -2, 1], -2.752), None);
    // The simple root of the same polynomial is still answered.
    assert_eq!(guided(&[-4, -7, -2, 1], 5.0), Some(4.0));
    // Too far away for the step budget, and a start whose square overflows.
    assert_eq!(guided(&[-2, 0, 1], 1e30), None);
    assert_eq!(guided(&[-2, 0, 1], 1e300), None);
    // x^3 - x from 0.57: the full step overshoots to -14.6 and only the
    // fourth halving lowers the residual. From 0.6 it is the fourth too.
    assert_eq!(guided(&[0, -1, 0, 1], 0.57), None);
    assert_eq!(guided(&[0, -1, 0, 1], 0.6), None);
    // x^3 - 2x + 2 cycles between 0 and 1.
    assert_eq!(guided(&[2, -2, 0, 1], 0.0), None);
}

#[test]
fn a_guided_root_stops_when_the_budget_is_spent() {
    let coefficients = [rational(-3, 1), Rat::ZERO, Rat::ONE];
    let spent = std::cell::Cell::new(0usize);
    let charge = |n: usize| {
        spent.set(spent.get() + n);
        spent.get() <= 4
    };
    assert_eq!(
        super::guided_root(&coefficients, 5.0, &mut Work::new(&charge)),
        None
    );
}
