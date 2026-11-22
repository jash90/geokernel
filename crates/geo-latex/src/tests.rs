use super::*;

fn math(source: &str, size: f32, style: Style) -> Layout {
    let formula = layout(source, size, style, Face::default());
    assert!(!formula.has_fallback, "{source:?}");
    assert!(formula.size().into_iter().all(f32::is_finite));
    formula
}

#[test]
fn upstream_serializations_are_typeset() {
    // LaTeXSerializationTest and StringTemplateTest exercise fraction,
    // nth-root, Unicode label and conditional-array layout.
    for source in [
        r"\frac{x+y}{x-y}",
        r"1-\sqrt[3]{x}",
        r"\sqrt[4]{{3}\frac{4}{5}}",
        r"X=\left(x_0+2x_x,y_0+2x_y\right)",
        r"1.\overline{34}",
        r"\operatorname{sin} ^{2}\left( x \right)",
        r"\left\{\begin{array}{ll} x& : x > 1\\ -x& : x < 0\\ 7& : \text{otherwise} \end{array}\right.",
        // AuralTextTest's two color macros and its scaling command.
        r"\fgcolor{red}{\text{red text}}",
        r"\bgcolor{red}{\text{not red text}}",
        r"\scalebox{0.5}{hello}",
        r"\displaylines{x\\y}",
        r"\textsf{textsf} \mathsf{mathsf} \sf{sf}",
        r"\textit{textit} \mathit{mathit} \it{it}",
        r"\texttt{texttt} \mathtt{mathtt} \tt{tt}",
        r"\textbf{textbf} \mathbf{mathbf} \bf{bf}",
        r"\textsc{textsc} \sc{sc}",
        r"\xleftrightarrow{p}j",
        r"\underrightarrow{p}j",
    ] {
        let result = math(source, 20.0, Style::Display);
        assert!(result.width > 0.0 && result.height > 0.0);
    }
}

#[test]
fn fractions_scale_and_place_rule_between_children() {
    let fraction = |size, style| math(r"\frac{1}{2}", size, style);
    let small = fraction(20.0, Style::Display);
    let big = fraction(40.0, Style::Display);
    assert!(small.height > 0.9 * 20.0);
    assert!(small.depth > 0.3 * 20.0);
    assert!(small.width > 0.5 * 20.0 && small.width < 0.95 * 20.0);
    for (a, b) in small.size().into_iter().zip(big.size()) {
        assert!((b - 2.0 * a).abs() < 0.05, "{a} {b}");
    }
    let text = fraction(20.0, Style::Text);
    assert!(text.height + text.depth < small.height + small.depth);
    let [num, den, rule] = small.ink.as_slice() else {
        panic!("one rule")
    };
    let Shape::Fill { points } = &rule.shape else {
        panic!("fraction rule")
    };
    assert!(num.transform.origin[1] < points[0][1]);
    assert!(den.transform.origin[1] > points[2][1]);
}

#[test]
fn delimiters_grow_to_the_matrix_and_scripts_shrink() {
    let small = math(r"\left(x\right)", 20.0, Style::Display);
    let large = math(
        r"\begin{pmatrix}x_1&x^2\\0&\frac{1}{x}\\1&2\end{pmatrix}",
        20.0,
        Style::Display,
    );
    assert!(large.height > small.height * 2.0 && large.depth > small.depth);
    assert!(large.ink[0].transform.y[1] > 2.0 * small.ink[0].transform.y[1]);
    let scripts = math(r"x_{i_1}^{k^2}", 20.0, Style::Text);
    let sizes: Vec<_> = scripts
        .ink
        .iter()
        .filter_map(|ink| match &ink.shape {
            Shape::Glyphs { size, .. } => Some(*size),
            _ => None,
        })
        .collect();
    assert_eq!(sizes, [20.0, 14.0, 10.0, 14.0, 10.0]);
}

#[test]
fn limits_and_accents_contribute_to_the_box() {
    let display = math(r"\sum_{i=1}^n", 24.0, Style::Display);
    let text = math(r"\sum_{i=1}^n", 24.0, Style::Text);
    assert!(display.height > text.height && display.depth > text.depth);
    let integral = math(
        r"\int\limits_0^{\pi}\! \overrightarrow{AB}\cdot\mathbf{x}",
        24.0,
        Style::Display,
    );
    assert!(integral
        .ink
        .iter()
        .any(|ink| matches!(ink.shape, Shape::Stroke { .. })));
    assert!(integral.ink.iter().any(|ink| matches!(
        ink.shape,
        Shape::Glyphs {
            face: Face { bold: true, .. },
            ..
        }
    )));
}

#[test]
fn unknown_commands_keep_their_source_and_neighboring_math() {
    let result = layout(
        r"1 + \unsupported[7]{x} + \pi",
        20.0,
        Style::Display,
        Face::default(),
    );
    assert!(result.has_fallback);
    assert!(result.ink.iter().any(|ink| matches!(&ink.shape,
        Shape::Glyphs { text, .. } if text == r"\unsupported[7]{x}" && ink.color == Some(FALLBACK_COLOR)
    )));
    assert!(result.ink.iter().any(|ink| matches!(&ink.shape,
        Shape::Glyphs { text, .. } if text == "π" && ink.color.is_none()
    )));
}

#[test]
fn nesting_limit_handles_incomplete_and_pathological_input() {
    let nested = "{".repeat(30_000);
    for source in [
        r"\begin{array}{cc} x & 7",
        r"\left(\frac{1}{2}",
        nested.as_str(),
    ] {
        assert!(layout(source, 20.0, Style::Display, Face::default()).has_fallback);
    }
}

#[test]
fn matrix_aliases_and_aligned_columns_share_the_grid_layout() {
    let table = r"x_0&=1\\y_0&=\frac{1}{2}";
    for env in [
        "matrix",
        "pmatrix",
        "bmatrix",
        "Bmatrix",
        "vmatrix",
        "Vmatrix",
        "smallmatrix",
        "cases",
        "rcases",
        "align",
        "align*",
        "aligned",
        "split",
        "gathered",
    ] {
        let source = format!(r"\begin{{{env}}}{table}\end{{{env}}}");
        let result = math(&source, 20.0, Style::Display);
        assert!(result.height + result.depth > 20.0, "{env}");
    }
    let ruled = math(
        r"\begin{array}{|l|C{40}|}\hline 1&2\\[10]3&\cellcolor{pink}4\\\hline\end{array}",
        20.0,
        Style::Display,
    );
    assert!(ruled.width >= 40.0);
    assert_eq!(
        ruled
            .ink
            .iter()
            .filter(|ink| matches!(ink.shape, Shape::Stroke { .. }))
            .count(),
        5
    );
    assert!(ruled
        .ink
        .iter()
        .any(|ink| ink.color == Some([255, 192, 203])));
}

#[test]
fn nested_middle_delimiters_are_laid_out_once_per_group() {
    let nested = (0..24).fold(String::from(r"\frac{1}{2}"), |child, _| {
        format!(r"\left\{{ {child} \middle| x \right\}}")
    });
    let formula = math(&nested, 20.0, Style::Display);
    assert_eq!(formula.ink.len(), 24 * 4 + 3);
}
