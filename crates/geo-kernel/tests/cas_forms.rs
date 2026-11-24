//! `Simplify` of a text and the inputs the reference refuses, for what the
//! golden cases cannot record: display strings and refused inputs.

use geo_kernel::{script, Construction, Id};

fn build(lines: &[&str]) -> Construction {
    let mut c = Construction::new();
    for line in lines {
        script::exec(&mut c, line).unwrap_or_else(|e| panic!("`{line}`: {e}"));
    }
    c
}

fn id(c: &Construction, label: &str) -> Id {
    c.id(label).unwrap_or_else(|| panic!("no object `{label}`"))
}

fn rejected(c: &mut Construction, line: &str) -> String {
    let before = c.len();
    let error = script::exec(c, line).expect_err(line);
    assert_eq!(c.len(), before, "`{line}` left objects behind");
    error
}

#[test]
fn rewriting_an_implicit_curve_requires_the_giac_backend() {
    let mut c = build(&["k: x^3+y^3=1"]);
    if geo_kernel::giac_backend_active() {
        for command in ["Expand", "Factor", "IFactor", "Simplify"] {
            let result = script::exec(&mut c, &format!("r{command} = {command}(k)"))
                .unwrap()
                .unwrap();
            assert_eq!(c.kind(result), geo_kernel::Kind::ImplicitCurve);
            assert_eq!(c.value(result).is_defined(), command != "IFactor");
        }
        return;
    }
    for command in ["Expand", "Factor", "IFactor", "Simplify"] {
        rejected(&mut c, &format!("r = {command}(k)"));
    }
    for line in ["r = Simplify(x^3+y^3=1)", "r = Simplify(x^3-x*y^2=0)"] {
        rejected(&mut c, line);
    }
}

#[test]
fn a_text_with_an_equals_sign_is_a_text() {
    let c = build(&[
        "r1 = Simplify(\"\u{3b1}=\u{3b2}\")",
        "r2 = Simplify(\"x+-1x=2\")",
        "r3 = Simplify(\"a,b=c\")",
        "r4 = Simplify(\"(a=b\")",
    ]);
    assert_eq!(c.text(id(&c, "r1")), Some("\u{3b1} = \u{3b2}"));
    assert_eq!(c.text(id(&c, "r2")), Some("x \u{2212} x = 2"));
    assert_eq!(c.text(id(&c, "r3")), Some("a,b = c"));
    assert_eq!(c.text(id(&c, "r4")), Some("(a = b"));
}

#[test]
fn hints_that_are_no_variable_or_value_are_refused() {
    let mut c = build(&["f(x) = x^2-3", "g: x+y=1"]);
    if geo_kernel::giac_backend_active() {
        // cas-name-010.
        assert_eq!(
            rejected(&mut c, "r = Solutions(f, a)"),
            "undefined variable `a`"
        );
    }
    for line in [
        "r = Solve(f, x>0)",
        "r = NSolve(f, {x=1})",
        "r = NSolutions(f, 1)",
        "r = Solve(f, x, y)",
        "r = NSolve(f, x=g)",
        "r = Solutions(g, x=)",
    ] {
        rejected(&mut c, line);
    }
}

#[test]
fn curve_coordinates_require_numbers_with_giac() {
    // cas-edge-018: CmdCurveCartesian requires a GeoNumberValue, while
    // CmdSimplify supplies a function for this coordinate expression.
    if geo_kernel::giac_backend_active() {
        let mut c = Construction::new();
        assert_eq!(
            rejected(&mut c, "k = Curve(1+Simplify((t-10)^2), t, t, 0, 2)"),
            "`Curve` needs numeric coordinates"
        );
    }
}

#[test]
fn nested_simplify_samples_graphs_after_input_changes() {
    let mut c = build(&[
        "a = 6",
        "b = 1",
        "g: 2x + 3y = a",
        "k: y = b*x^2",
        "f(x) = 1 + Simplify(g)",
        "h(x) = 1 + Simplify(k)",
    ]);
    let value_at = |c: &Construction, label| c.function_value(id(c, label), 3.0);
    assert!((value_at(&c, "f").unwrap() - 1.0).abs() < 1e-12);
    assert_eq!(value_at(&c, "h"), Some(10.0));

    c.set_number(id(&c, "a"), 12.0).unwrap();
    c.set_number(id(&c, "b"), 2.0).unwrap();
    assert!((value_at(&c, "f").unwrap() - 3.0).abs() < 1e-12);
    assert_eq!(value_at(&c, "h"), Some(19.0));
}

#[test]
fn nested_simplify_limits_graph_rewrites() {
    let mut c = build(&["g: 2x + 3y = 6"]);
    let expression = ["Simplify(g)"; 9].join(" + ");
    assert_eq!(
        rejected(&mut c, &format!("f(x) = {expression}")),
        "too many nested graph rewrites"
    );
}

#[test]
fn objects_that_are_no_expression_are_refused() {
    let mut c = build(&[
        "g: x+y=1",
        "k: x^2+y^2=4",
        "l = {1, 2}",
        "A = (0, 0)",
        "B = (1, 2)",
        "C = (3, 1)",
        "s = Spline({A, B, C})",
    ]);
    for command in ["Expand", "Factor", "IFactor"] {
        for argument in ["g", "k", "l", "A", "{1, 2}", "x+y=1"] {
            rejected(&mut c, &format!("r = {command}({argument})"));
        }
        rejected(&mut c, &format!("r = {command}(s)"));
        rejected(&mut c, &format!("r = {command}(s, 2)"));
    }
}

#[test]
fn simplify_refuses_what_is_no_function_text_or_curve() {
    let mut c = build(&[
        "A = (0, 1)",
        "B = (1, 0)",
        "C = (2, 3)",
        "p = Polygon(A, B, C)",
        "l = {1, 2}",
        "t = \"\"",
    ]);
    for argument in ["A", "p", "l", "{1, 2}", "t", "\"\""] {
        rejected(&mut c, &format!("r = Simplify({argument})"));
    }
    if !geo_kernel::giac_backend_active() {
        rejected(&mut c, "r = Simplify(x^3+y^3=1)");
    }
    rejected(&mut c, "r = Simplify(l, 2)");
}

#[test]
fn a_simplified_text_follows_its_source() {
    let mut c = build(&["a = 2", "t = \"1x+\" + a", "s = Simplify(t)"]);
    assert_eq!(c.text(id(&c, "s")), Some("x + 2"));
    c.set_number(id(&c, "a"), -3.0).unwrap();
    assert_eq!(c.text(id(&c, "s")), Some("x \u{2212} 3"));
    // A text of 4 MB of signs stays within the text bound or is undefined.
    let long = "+-".repeat(1 << 21);
    let mut c = Construction::new();
    script::exec(&mut c, &format!("t = \"{long}\"")).ok();
    if c.id("t").is_some() {
        script::exec(&mut c, "s = Simplify(t)").ok();
    }
    // The kernel's text bound (`MAX_TEXT_BYTES` in the text commands).
    const TEXT_BOUND: usize = 1 << 20;
    if let Some(s) = c.id("s") {
        assert!(c.text(s).is_none_or(|text| text.len() <= TEXT_BOUND));
    }
}
