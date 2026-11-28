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

/// Texts are drawn with the colour the canvas gives them, and a plain
/// text sits at its anchor with the width the formula renderer measures.
#[test]
fn svg_texts_use_the_object_colour_and_measured_width() {
    let source = format!(
        "t = {}\n$textProperties(t, true, -, -, -, -, screen, 40, 50)\n\
         p = {}\n$textProperties(p, false, -, -, -, -, screen, 40, 150)\n",
        script::text::literal(r"\frac{a}{b}"),
        script::text::literal("plain text")
    );
    let c = script::load(&source).unwrap();
    let svg = svg::render(&c, svg::View::fit(&c), 400, 300);
    assert!(svg.contains(r##"fill="#1c1c1f""##), "{svg}");
    assert!(
        svg.contains(r#"<text x="40.00" y="150.00""#),
        "plain text at its anchor: {svg}"
    );
}

#[test]
fn svg_plain_text_keeps_the_saved_serif_family() {
    let source = format!(
        "t = {}\n$textProperties(t, false, 1, 7, -, -, screen, 40, 50)\n\
         p = {}\n$textProperties(p, false, 1, 0, -, -, screen, 40, 150)\n",
        script::text::literal("serif first\nserif second"),
        script::text::literal("plain sans")
    );
    let c = script::load(&source).unwrap();
    let svg = svg::render(&c, svg::View::fit(&c), 400, 300);
    for label in ["serif first", "serif second"] {
        let row = svg.lines().find(|line| line.contains(label)).unwrap();
        assert!(row.contains(r#"font-family="serif""#), "{row}");
        assert!(
            row.contains(r#"font-style="italic" font-weight="bold""#),
            "{row}"
        );
    }
    let row = svg
        .lines()
        .find(|line| line.contains("plain sans"))
        .unwrap();
    assert!(!row.contains("font-family"), "{row}");
}

/// A text parked at a huge coordinate does not swallow the fitted view;
/// only a construction of texts alone is fitted to them.
#[test]
fn a_text_far_away_does_not_swallow_the_fitted_view() {
    let source = "A = (1, 1)\nB = (3, 1)\ns = Segment(A, B)\n".to_owned()
        + &format!(
            "t = {}\n$textProperties(t, false, -, -, -, -, world, 1000000, -1000000)\n",
            script::text::literal("far away")
        );
    let c = script::load(&source).unwrap();
    let view = svg::View::fit(&c);
    assert!(view.xmin > -10.0 && view.xmax < 10.0, "{view:?}");
    let svg = svg::render(&c, view, 400, 300);
    assert!(svg.contains("far away"), "the text is still drawn");

    let only = format!(
        "t = {}\n$textProperties(t, false, -, -, -, -, world, 10, -20)\n",
        script::text::literal("lonely")
    );
    let c = script::load(&only).unwrap();
    let view = svg::View::fit(&c);
    assert!(
        (view.xmin - 10.0).abs() < 0.6 && (view.ymax + 20.0).abs() < 0.6,
        "{view:?}"
    );
}
