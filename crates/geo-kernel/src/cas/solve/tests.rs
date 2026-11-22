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
