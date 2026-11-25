use geo_kernel::Construction;

#[test]
fn evaluates_exact_rows_without_mutating_the_construction() {
    let c = Construction::new();
    let result = c.evaluate_cas_row(" 2 / 3 * 5 / 7 = ");
    assert_eq!(result.exact.as_deref(), Some("10/21"));
    assert_eq!(result.text, "10 / 21");
    assert_eq!(result.latex, r"\mathbf{\frac{10}{21}}");
    assert_eq!(c.len(), 0);
}

#[test]
fn refuses_a_row_above_the_input_limit_before_trimming() {
    let c = Construction::new();
    let mut input = " ".repeat(64 * 1024 - 1);
    input.push('1');
    assert_eq!(c.evaluate_cas_row(&input).text, "1");
    input.push(' ');
    assert_eq!(c.evaluate_cas_row(&input).text, "?");
}

#[test]
fn feature_commands_explain_the_refusal_when_giac_is_unavailable() {
    if geo_kernel::giac_backend_active() {
        return;
    }
    let c = Construction::new();
    let result = c.evaluate_cas_row("1 + Numeric(2/3)");
    assert_eq!(result.text, "Numeric requires a CAS build");
    assert!(result.latex.is_empty());
    assert_eq!(result.exact, None);
}

#[cfg(not(feature = "giac"))]
#[test]
fn evaluates_recorded_exact_row_regressions() {
    let mut failures = Vec::new();
    for (script, input, expected, latex) in [
        ("a = 1/3", "3*a", "1", r"\mathbf{1}"),
        ("", "Solve(x^2/x = 0)", "{}", r"\mathbf{\left\{ \right\}}"),
        (
            "",
            "Solve(y = 1)",
            "{y = 1}",
            r"\mathbf{ \left\{ y = 1 \right\} }",
        ),
        ("", "x² - 1", "x² - 1", r"\mathbf{x^{2} - 1}"),
        ("", "5!", "120", r"\mathbf{120}"),
        ("", "abs(-3)", "3", r"\mathbf{3}"),
        ("", "abs(x)", "abs(x)", r"\mathbf{\left|x\right|}"),
        (
            "",
            "Factor(x^3-x)",
            "x (x + 1) (x - 1)",
            r"\mathbf{x \; \left(x + 1 \right) \; \left(x - 1 \right)}",
        ),
        ("g: x + y = 0", "g", "x + y = 0", r"\mathbf{x + y = 0}"),
        (
            "g: x + y = 0.5",
            "g",
            "x + y = 1 / 2",
            r"\mathbf{x + y = \frac{1}{2}}",
        ),
    ] {
        let c = geo_kernel::script::load(script).unwrap();
        let result = c.evaluate_cas_row(input);
        if (result.text.as_str(), result.latex.as_str()) != (expected, latex) {
            failures.push(format!(
                "{script}: {input}: got {result:?}; expected {expected}, {latex}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
