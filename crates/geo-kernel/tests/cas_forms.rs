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
fn rewriting_an_implicit_curve_is_refused() {
    // The reference answers an implicit curve with an equation text of its
    // CAS; the forms stay open.
    let mut c = build(&["k: x^3+y^3=1"]);
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
    for argument in ["A", "p", "l", "{1, 2}", "t", "\"\"", "x^3+y^3=1"] {
        rejected(&mut c, &format!("r = Simplify({argument})"));
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
