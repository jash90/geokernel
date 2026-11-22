use geo_kernel::{script, svg};

#[test]
fn upstream_formula_vectors_and_piecewise_arrays_are_visible_in_svg() {
    // AlgoLaTeXTest uses align for column vectors. StringTemplateTest
    // uses an array and an invisible right delimiter for piecewise functions.
    for formula in [
        r"\left( \begin{align}-7 \\ 3 \end{align} \right)",
        r"\left\{\begin{array}{ll} x& : x > 0\\ -x& : \text{otherwise} \end{array}\right.",
    ] {
        let source = format!(
            "t = {}\n$textProperties(t, true, -, -, -, -, screen, 31, 42)\n",
            script::text::literal(formula)
        );
        let c = script::load(&source).unwrap();
        let svg = svg::render(&c, svg::View::fit(&c), 400, 300);
        assert!(svg.contains(r#"transform="translate(31.00 42.00)""#));
        assert!(svg.matches("<path ").count() >= 3, "{svg}");
    }
}

#[test]
fn svg_formulas_use_fallback_ink_and_obey_visibility() {
    let source = format!(
        "t = {}\n$textProperties(t, true, -, -, -, -, screen, 70, 90)\n",
        script::text::literal(r"\frac{a}{b} + \unknown{c}")
    );
    let c = script::load(&source).unwrap();
    let view = svg::View::fit(&c);
    let visible = svg::render(&c, view, 400, 300);
    assert!(visible.contains(r##"fill="#b02c71""##));
    let id = c.id("t").unwrap();
    let hidden = svg::render_visible(&c, view, 400, 300, |candidate| candidate != id);
    assert!(!hidden.contains("<path "));
}
