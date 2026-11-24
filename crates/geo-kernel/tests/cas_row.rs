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
