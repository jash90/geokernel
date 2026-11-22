//! Behaviour added in the second round: unindexed `Intersect`, the cross-check
//! with the saved values, constant expressions, several construction sections,
//! the notes for ignored properties, and detection of a file by its content.

use geo_import::convert::is_constant_expression;
use geo_import::{import, looks_like_construction, Imported};

fn file(body: &str) -> String {
    format!("<geogebra><construction>{body}</construction></geogebra>")
}

fn free(label: &str, x: &str, y: &str) -> String {
    format!(
        "<element type=\"point\" label=\"{label}\"><coords x=\"{x}\" y=\"{y}\" z=\"1\"/></element>"
    )
}

fn ok(body: &str) -> Imported {
    import(file(body).as_bytes()).unwrap()
}

/// A line y = 0 and the unit circle around (2, 0); GeoGebra saves the
/// intersection without an index and lists both points.
fn line_circle(outputs: &str, saved: &str) -> String {
    free("A", "0", "0")
        + &free("B", "4", "0")
        + &free("C", "2", "0")
        + "<element type=\"numeric\" label=\"r\"><value val=\"1\"/></element>"
        + "<command name=\"Line\"><input a0=\"A\" a1=\"B\"/><output a0=\"f\"/></command>"
        + "<command name=\"Circle\"><input a0=\"C\" a1=\"r\"/><output a0=\"k\"/></command>"
        + "<command name=\"Intersect\"><input a0=\"f\" a1=\"k\"/>"
        + outputs
        + "</command>"
        + saved
}

#[test]
fn unindexed_intersect_maps_each_output_to_its_index() {
    let r = ok(&line_circle("<output a0=\"P\" a1=\"Q\"/>", ""));
    assert!(r.skipped.is_empty(), "{:?}", r.skipped);
    assert_eq!(r.script[6], "P = Intersect(f, k, 1)");
    assert_eq!(r.script[7], "Q = Intersect(f, k, 2)");
    // An empty slot creates nothing; the other keeps its own index.
    let r = ok(&line_circle("<output a0=\"\" a1=\"Q\"/>", ""));
    assert_eq!(r.script[6], "Q = Intersect(f, k, 2)");
    assert_eq!(r.script.len(), 7);
    // A single output is the plain form.
    let r = ok(
        &(free("A", "0", "0")
            + &free("B", "4", "0")
            + &free("C", "0", "1")
            + &free("D", "4", "1")
            + "<command name=\"Line\"><input a0=\"A\" a1=\"D\"/><output a0=\"f\"/></command>\
               <command name=\"Line\"><input a0=\"B\" a1=\"C\"/><output a0=\"g\"/></command>\
               <command name=\"Intersect\"><input a0=\"f\" a1=\"g\"/><output a0=\"P\"/></command>"),
    );
    assert_eq!(r.script.last().unwrap(), "P = Intersect(f, g)");
}

#[test]
fn an_unlabelled_optimizer_input_is_live_after_import() {
    use geo_kernel::{script, Construction, Value};

    let imported = ok(
        r#"<element type="numeric" label="a"><value val="0"/><slider min="0" max="6"/></element>
           <element type="point" label="P"><coords x="3" y="0" z="1"/></element>
           <element type="point" label="A"><coords x="0" y="0" z="1"/></element>
           <expression label="f" exp="If(x(P) &gt; 0, a * (2 * x(P) - a))"/>
           <element type="numeric" label="f"><value val="0"/></element>
           <command name="Circle"><input a0="A" a1="Maximize(f, a)"/><output a0="c"/></command>"#,
    );
    assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
    assert_eq!(
        imported.script.last().unwrap(),
        "c = Circle(A, $arg(Maximize(f, a)))"
    );

    let mut c = script::load(&imported.script.join("\n")).unwrap();
    let point = c.id("P").unwrap();
    let objective = c.id("f").unwrap();
    let circle = c.id("c").unwrap();
    let assert_radius = |c: &Construction, expected: f64| {
        let Value::Circle { r, .. } = c.value(circle) else {
            panic!("the imported circle is undefined");
        };
        assert!(
            (r - expected).abs() < 1e-9,
            "radius {r}, expected {expected}"
        );
    };
    assert_radius(&c, 3.0);
    c.move_point(point, 2.0, 0.0).unwrap();
    assert_eq!(c.value(objective), Value::Number(0.0));
    assert_radius(&c, 2.0);
    c.move_point(point, 0.0, 0.0).unwrap();
    assert_eq!(c.value(circle), Value::Undefined);
    c.move_point(point, 4.0, 0.0).unwrap();
    assert_radius(&c, 4.0);
}

#[test]
fn the_saved_value_is_checked_against_the_computed_one() {
    let first = "<element type=\"point\" label=\"P\"><coords x=\"1\" y=\"0\" z=\"1\"/></element>";
    let second = "<element type=\"point\" label=\"Q\"><coords x=\"3\" y=\"0\" z=\"1\"/></element>";
    let r = ok(&line_circle(
        "<output a0=\"P\" a1=\"Q\"/>",
        &(first.to_string() + second),
    ));
    let swapped = ok(&line_circle(
        "<output a0=\"P\" a1=\"Q\"/>",
        &(second.replace('Q', "P") + &first.replace('P', "Q")),
    ));
    assert!(r.mismatched.is_empty());
    assert!(swapped.mismatched.is_empty());
    let wrong = ok(&line_circle(
        "<output a0=\"P\" a1=\"Q\"/>",
        &(first.replace("y=\"0\"", "y=\"9\"") + &second.replace("y=\"0\"", "y=\"9\"")),
    ));
    assert_eq!(wrong.mismatched.len(), 2);
    assert!(wrong.script.iter().any(|l| l == "P = Intersect(f, k, 1)"));
    assert_eq!(wrong.mismatched[0].label, "P");
    assert!(wrong.mismatched[0].saved.contains("coords"));
}

#[test]
fn values_within_tolerance_and_scaled_lines_agree() {
    let body = free("A", "0", "0")
        + &free("B", "4", "2")
        // homogeneous point (2, 1, 0.5) x 2 = (4, 2, 1)
        + "<command name=\"Midpoint\"><input a0=\"A\" a1=\"B\"/><output a0=\"M\"/></command>\
           <element type=\"point\" label=\"M\"><coords x=\"4.000000000001\" y=\"2\" z=\"2\"/></element>\
           <command name=\"Line\"><input a0=\"A\" a1=\"B\"/><output a0=\"f\"/></command>\
           <element type=\"line\" label=\"f\"><coords x=\"-20\" y=\"40\" z=\"0\"/></element>\
           <command name=\"Segment\"><input a0=\"A\" a1=\"B\"/><output a0=\"s\"/></command>\
           <element type=\"segment\" label=\"s\"><coords x=\"-2\" y=\"4\" z=\"0\"/></element>";
    let r = ok(&body);
    assert!(r.mismatched.is_empty(), "{:?}", r.mismatched);
    // a different value is reported, and the object stays
    let r = ok(&body.replace("x=\"4.000000000001\"", "x=\"4.01\""));
    assert_eq!(r.mismatched.len(), 1);
    assert_eq!(r.mismatched[0].label, "M");
    assert!(r.script.contains(&"M = Midpoint(A, B)".to_string()));
}

#[test]
fn constant_expressions_are_free_objects_with_the_saved_value() {
    for yes in [
        "(1/2, 3)",
        "2^10",
        "1.5E-3",
        "(1, 2) ",
        "-(3 + 4) * 2",
        "1e5",
    ] {
        assert!(is_constant_expression(yes), "{yes}");
    }
    for no in [
        "a + 1",
        "pi",
        "sqrt(2)",
        "e",
        "E5",
        "(A, 1)",
        "",
        "()",
        "x(A)",
        "1 + t_{1}",
    ] {
        assert!(!is_constant_expression(no), "{no}");
    }
    let r = ok("<expression label=\"A\" exp=\"(1/2, 3)\" type=\"point\"/>\
                <element type=\"point\" label=\"A\"><coords x=\"0.5\" y=\"3\" z=\"1\"/></element>\
                <expression label=\"n\" exp=\"2^10\" type=\"numeric\"/>\
                <element type=\"numeric\" label=\"n\"><value val=\"1024\"/></element>\
                <expression label=\"m\" exp=\"n + 1\" type=\"numeric\"/>\
                <element type=\"numeric\" label=\"m\"><value val=\"1025\"/></element>");
    assert_eq!(
        r.script,
        [
            "A = $saved($point(0.5, 3), (1/2, 3))",
            "n = $saved(1024, 2^10)",
            "m = n + 1"
        ]
    );
    assert!(r.skipped.is_empty());
}

#[test]
fn older_expression_spellings_and_invalid_expressions() {
    let r = ok(
        "<element type=\"numeric\" label=\"n\"><value val=\"2\"/></element>\
                <expression label=\"f\" exp=\"f(x) = If[x ≥ 0, n · x² + π]\"/>\
                <element type=\"function\" label=\"f\"/>\
                <expression label=\"bad\" exp=\"If[n ≥ 0, n)\"/>\
                <element type=\"numeric\" label=\"bad\"/>\
                <expression label=\"g\" exp=\"g(x) = If[x ≠ 0, n, bad]\"/>\
                <element type=\"function\" label=\"g\"/>\
                <expression label=\"text\" exp=\"&quot;[text]&quot;\"/>\
                <element type=\"text\" label=\"text\"/>\
                <expression label=\"injection\" exp=\"n + 1 # comment&#10; extra = 2\"/>\
                <element type=\"numeric\" label=\"injection\"/>\
                <expression label=\"after\" exp=\"f(n)\"/>\
                <element type=\"numeric\" label=\"after\"/>",
    );
    assert_eq!(
        r.script,
        [
            "n = 2",
            "f(x) = If(x ≥ 0, n * x² + π)",
            "text = \"[text]\"",
            "after = f(n)"
        ]
    );
    let skips: Vec<_> = r
        .skipped
        .iter()
        .map(|s| (s.label.as_str(), s.reason.as_str()))
        .collect();
    assert_eq!(skips.len(), 3);
    assert_eq!(skips[0].0, "bad");
    assert!(skips[0].1.contains("square bracket"));
    assert_eq!(skips[1], ("g", "depends on the skipped object `bad`"));
    assert_eq!(skips[2].0, "injection");
    assert!(skips[2].1.contains("comment marker"));
}

#[test]
fn expressions_keep_saved_point_and_vector_types_after_renaming() {
    let r = ok(
        "<element type=\"point\" label=\"A\"><coords x=\"1\" y=\"2\" z=\"1\"/></element>\
                <expression label=\"p\" exp=\"(x(A), y(A) + 1)\" type=\"point\"/>\
                <element type=\"point\" label=\"p\"/>\
                <expression label=\"V\" exp=\"(y(p), x(p))\" type=\"vector\"/>\
                <element type=\"vector\" label=\"V\"/>",
    );
    assert!(r.skipped.is_empty(), "{:?}", r.skipped);
    let mut c = geo_kernel::script::load(&r.script.join("\n")).unwrap();
    c.move_point(c.id("A").unwrap(), 3.0, 5.0).unwrap();
    assert_eq!(c.kind(c.id("p").unwrap()), geo_kernel::Kind::Point);
    assert_eq!(c.kind(c.id("V").unwrap()), geo_kernel::Kind::Vector);
    assert_eq!(
        c.value(c.id("V").unwrap()),
        geo_kernel::Value::Vector { x: 6.0, y: 3.0 }
    );
}

#[test]
fn rejecting_a_multi_output_expression_releases_its_labels() {
    let r = ok(
        "<element type=\"point\" label=\"A\"><coords x=\"0\" y=\"0\" z=\"1\"/></element>\
                <element type=\"point\" label=\"B\"><coords x=\"1\" y=\"0\" z=\"1\"/></element>\
                <element type=\"point\" label=\"C\"><coords x=\"0\" y=\"1\" z=\"1\"/></element>\
                <expression label=\"bad\" exp=\"Polygon[A, B, C]\"/>\
                <element type=\"numeric\" label=\"bad\"/>\
                <command name=\"Polygon\"><input a0=\"A\" a1=\"B\" a2=\"C\"/>\
                <output a0=\"p\" a1=\"a\" a2=\"b\" a3=\"c\"/></command>",
    );
    assert_eq!(r.skipped.len(), 1, "{:?}", r.skipped);
    assert_eq!(r.skipped[0].label, "bad");
    assert!(r.skipped[0].reason.contains("exactly one object"));
    assert!(r.hidden.is_empty());
    let c = geo_kernel::script::load(&r.script.join("\n")).unwrap();
    assert!(c.id("bad").is_none());
    for label in ["p", "a", "b", "c"] {
        assert!(c.id(label).is_some(), "{label}");
    }
}

#[test]
fn a_second_construction_section_is_appended() {
    let xml = "<geogebra><construction>\
               <element type=\"point\" label=\"A\"><coords x=\"1\" y=\"2\" z=\"1\"/></element>\
               </construction><construction>\
               <element type=\"point\" label=\"B\"><coords x=\"3\" y=\"4\" z=\"1\"/></element>\
               <command name=\"Midpoint\"><input a0=\"A\" a1=\"B\"/><output a0=\"M\"/></command>\
               </construction></geogebra>";
    let r = import(xml.as_bytes()).unwrap();
    assert_eq!(r.script, ["A = (1, 2)", "B = (3, 4)", "M = Midpoint(A, B)"]);
    assert!(r
        .notes
        .iter()
        .any(|n| n.contains("2 construction sections")));
}

#[test]
fn ignored_properties_are_listed_in_the_notes() {
    let r = ok("<element type=\"point\" label=\"A\"><coords x=\"1\" y=\"2\" z=\"1\"/>\
                <fixed val=\"true\"/><auxiliary val=\"true\"/><condition showObject=\"A\"/></element>\
                <element type=\"vector\" label=\"v\"><coords x=\"1\" y=\"2\" z=\"0\"/>\
                <startPoint x=\"0\" y=\"0\" z=\"1\"/></element>\
                <element type=\"point\" label=\"B\"><coords x=\"1\" y=\"2\" z=\"1\"/>\
                <fixed val=\"false\"/></element>");
    let notes = r.notes.join("\n");
    assert!(notes.contains("1 fixed"), "{notes}");
    assert!(notes.contains("1 auxiliary"), "{notes}");
    assert!(
        notes.contains("visibility condition for `A`: `A` is not a boolean"),
        "{notes}"
    );
    assert!(notes.contains("1 vectors with a start point"), "{notes}");
    assert_eq!(r.script.len(), 3);
}

#[test]
fn indexed_labels_with_letters_inside_the_braces() {
    let r = ok(&(free("v_{max}", "1", "2") + &free("P_{a'}", "3", "4")));
    assert_eq!(r.script, ["v_{max} = $point(1, 2)", "P_{a'} = (3, 4)"]);
    assert!(r.skipped.is_empty());
}

#[test]
fn the_angle_unit_does_not_change_what_is_computed() {
    // An angle command takes objects, not numbers, so the unit of the file
    // (`degree`, `radiant`) only affects how GeoGebra prints; the import and
    // the cross-check against the saved radians are the same for both.
    let body = |unit: &str| {
        format!(
            "<geogebra><kernel><angleUnit val=\"{unit}\"/></kernel><construction>{}\
             <command name=\"Angle\"><input a0=\"B\" a1=\"A\" a2=\"C\"/><output a0=\"a\"/></command>\
             <element type=\"angle\" label=\"a\"><value val=\"1.5707963267948966\"/></element>\
             </construction></geogebra>",
            free("A", "0", "0") + &free("B", "1", "0") + &free("C", "0", "1")
        )
    };
    let degree = import(body("degree").as_bytes()).unwrap();
    let radiant = import(body("radiant").as_bytes()).unwrap();
    assert_eq!(degree.script, radiant.script);
    assert!(degree.mismatched.is_empty(), "{:?}", degree.mismatched);
    assert!(radiant.mismatched.is_empty(), "{:?}", radiant.mismatched);
}

#[test]
fn files_are_told_from_scripts_by_content() {
    assert!(looks_like_construction(b"PK\x03\x04rest"));
    assert!(looks_like_construction(
        b"<geogebra><construction/></geogebra>"
    ));
    assert!(looks_like_construction(
        b"\xEF\xBB\xBF<?xml version=\"1.0\"?>\n<!-- c -->\n  <geogebra format=\"5\">"
    ));
    assert!(looks_like_construction(b"<geogebra>"));
    assert!(!looks_like_construction(b"A = (0, 0)\nB = (1, 1)\n"));
    assert!(!looks_like_construction(b"# <geogebra>\nA = (0, 0)\n"));
    assert!(!looks_like_construction(b"<geogebrafoo/>"));
    assert!(!looks_like_construction(b"<html><geogebra/></html>"));
    assert!(!looks_like_construction(b""));
    assert!(!looks_like_construction(b"<?xml"));
}

#[test]
fn polygon_outputs_are_labelled_by_the_kernel() {
    let points = free("A", "0", "0") + &free("B", "4", "0") + &free("C", "0", "3");
    let polygon = |outputs: &str| {
        ok(&(points.clone()
            + "<command name=\"Polygon\"><input a0=\"A\" a1=\"B\" a2=\"C\"/>"
            + outputs
            + "</command>\
               <command name=\"Area\"><input a0=\"p\"/><output a0=\"ar\"/></command>"))
    };
    // the labels GeoGebra gives to the sides of a triangle
    let r = polygon("<output a0=\"p\" a1=\"c\" a2=\"a\" a3=\"b\"/>");
    assert!(r.skipped.is_empty(), "{:?}", r.skipped);
    assert_eq!(r.script[3], "p = Polygon(A, B, C)");
    assert_eq!(r.script[4], "ar = Area(p)");
    // a side the user renamed has no counterpart: it is skipped, the rest stays
    let r = polygon("<output a0=\"p\" a1=\"mine\" a2=\"a\" a3=\"b\"/>");
    assert_eq!(r.skipped.len(), 1);
    assert_eq!(r.skipped[0].label, "mine");
    assert!(r.skipped[0].reason.contains("does not make"));
    assert_eq!(r.script[3], "p = Polygon(A, B, C)");
    assert_eq!(r.script[4], "ar = Area(p)");
}

fn circle_file(cx: &str, r: &str, matrix: &str) -> String {
    free("A", cx, "0")
        + &format!("<element type=\"numeric\" label=\"r\"><value val=\"{r}\"/></element>")
        + "<command name=\"Circle\"><input a0=\"A\" a1=\"r\"/><output a0=\"k\"/></command>"
        + &format!("<element type=\"conic\" label=\"k\"><matrix {matrix}/></element>")
}

#[test]
fn a_circle_far_from_the_origin_is_compared_with_the_precision_of_its_matrix() {
    // centre (2000, 0): A2 = 2000^2 - r^2
    let m = |r2: f64| {
        format!(
            "A0=\"1\" A1=\"1\" A2=\"{}\" A3=\"0\" A4=\"-2000\" A5=\"0\"",
            4_000_000.0 - r2
        )
    };
    let same = ok(&circle_file("2000", "1", &m(1.0)));
    assert!(same.mismatched.is_empty(), "{:?}", same.mismatched);
    // r = 1.001 saved, r = 1 computed: reported, although the squared radius
    // differs by 0.002 only, far below 1e-9 of the 4e6 in the matrix
    let other = ok(&circle_file("2000", "1", &m(1.001 * 1.001)));
    assert_eq!(other.mismatched.len(), 1);
    assert_eq!(other.mismatched[0].label, "k");
    // a wrong centre is reported
    let moved = ok(&circle_file(
        "2000",
        "1",
        "A0=\"1\" A1=\"1\" A2=\"3996001\" A3=\"0\" A4=\"-1999.999\" A5=\"0\"",
    ));
    assert_eq!(moved.mismatched.len(), 1);
}

#[test]
fn a_tiny_saved_radius_stands_for_a_single_point_only_in_a_transform_image() {
    // A circle of radius 4e-5 around (1, 1): the matrix classifies afresh as
    // a single point (beta = -1.6e-9 is below the zero threshold).
    let tiny = "A0=\"1\" A1=\"1\" A2=\"1.9999999984\" A3=\"0\" A4=\"-1\" A5=\"-1\"";
    // A constructor circle computed with radius 0 against that saved matrix
    // is a mismatch: only a transformed matrix is classified again.
    let made = ok(&(free("A", "1", "1")
        + "<element type=\"numeric\" label=\"r\"><value val=\"0\"/></element>"
        + "<command name=\"Circle\"><input a0=\"A\" a1=\"r\"/><output a0=\"k\"/></command>"
        + &format!("<element type=\"conic\" label=\"k\"><matrix {tiny}/></element>")));
    assert_eq!(made.mismatched.len(), 1, "{:?}", made.mismatched);
    assert_eq!(made.mismatched[0].label, "k");
    // The image of that circle under a dilation by 2 is a single point of
    // radius 0 for GeoGebra and the kernel, while its saved matrix keeps the
    // radius 8e-5: it agrees.
    let image = ok(&(free("A", "1", "1")
        + "<element type=\"numeric\" label=\"r\"><value val=\"0.00004\"/></element>"
        + "<command name=\"Circle\"><input a0=\"A\" a1=\"r\"/><output a0=\"k\"/></command>"
        + &format!("<element type=\"conic\" label=\"k\"><matrix {tiny}/></element>")
        + "<element type=\"numeric\" label=\"f\"><value val=\"2\"/></element>"
        + "<command name=\"Dilate\"><input a0=\"k\" a1=\"f\"/><output a0=\"m\"/></command>"
        + "<element type=\"conic\" label=\"m\"><matrix A0=\"1\" A1=\"1\" \
           A2=\"7.9999999936\" A3=\"0\" A4=\"-2\" A5=\"-2\"/></element>"));
    assert!(image.mismatched.is_empty(), "{:?}", image.mismatched);
}

#[test]
fn defined_against_undefined_is_a_mismatch_both_ways() {
    // the kernel gives an undefined circle for a negative radius; the file
    // saved a defined one
    let r = ok(&circle_file(
        "0",
        "-1",
        "A0=\"1\" A1=\"1\" A2=\"-1\" A3=\"0\" A4=\"0\" A5=\"0\"",
    ));
    assert_eq!(r.mismatched.len(), 1, "{:?}", r.mismatched);
    // the kernel gives a defined circle; the file saved an undefined one
    let r = ok(&circle_file(
        "0",
        "1",
        "A0=\"NaN\" A1=\"NaN\" A2=\"NaN\" A3=\"0\" A4=\"0\" A5=\"0\"",
    ));
    assert_eq!(r.mismatched.len(), 1, "{:?}", r.mismatched);
    // a point at infinity saved, a finite midpoint computed
    let r = ok(&(free("A", "0", "0")
        + &free("B", "4", "2")
        + "<command name=\"Midpoint\"><input a0=\"A\" a1=\"B\"/><output a0=\"M\"/></command>\
           <element type=\"point\" label=\"M\"><coords x=\"1\" y=\"1\" z=\"0\"/></element>"));
    assert_eq!(r.mismatched.len(), 1);
    // nothing saved to judge by: no mismatch
    let r = ok(&(free("A", "0", "0")
        + &free("B", "4", "2")
        + "<command name=\"Midpoint\"><input a0=\"A\" a1=\"B\"/><output a0=\"M\"/></command>\
           <element type=\"point\" label=\"M\"/>"));
    assert!(r.mismatched.is_empty());
}

#[test]
fn presentation_tags_are_kept_as_the_saved_style() {
    use geo_import::export::{
        DynamicColor, Fill, LineStyle, ObjectStyle, Rounding, SliderAnimation, SliderFlags,
        SliderLayout,
    };
    let r = ok(
        "<element type=\"numeric\" label=\"r\"><value val=\"0.5\"/></element>\
         <element type=\"point\" label=\"A\"><coords x=\"1\" y=\"2\" z=\"1\"/>\
         <show object=\"true\" label=\"false\" ev=\"3\"/>\
         <objColor r=\"10\" g=\"20\" b=\"30\" alpha=\"0.25\" dynamicr=\"r\" dynamicg=\"\" \
         dynamicb=\"1 - r\" dynamica=\"r\" colorSpace=\"2\" fillType=\"8\" hatchAngle=\"60\" \
         hatchDistance=\"12\" fillSymbol=\"*\" inverseFill=\"true\"/>\
         <bgColor r=\"1\" g=\"2\" b=\"3\" alpha=\"128\"/>\
         <layer val=\"4\"/><labelMode val=\"3\"/><tooltipMode val=\"2\"/>\
         <trace val=\"true\"/><decoration type=\"2\"/><ordering val=\"2.5\"/>\
         <lineStyle thickness=\"7\" type=\"15\" typeHidden=\"2\" opacity=\"99\"/>\
         <fixed val=\"true\"/><selectionAllowed val=\"false\"/><auxiliary val=\"true\"/>\
         <caption val=\"Start\"/><pointSize val=\"7\"/><pointStyle val=\"3\"/>\
         <animation step=\"r\" speed=\"2\" type=\"3\" playing=\"false\"/></element>\
         <element type=\"numeric\" label=\"a\"><value val=\"1\"/>\
         <slider min=\"0\" max=\"5\" width=\"150\" x=\"20\" y=\"40\" fixed=\"true\" \
         horizontal=\"false\" showAlgebra=\"true\" absoluteScreenLocation=\"true\"/></element>\
         <expression label=\"t\" exp=\"&quot;note&quot;\"/>\
         <element type=\"text\" label=\"t\"><decimals val=\"4\"/></element>\
         <element type=\"point\" label=\"B\"><coords x=\"0\" y=\"0\" z=\"1\"/></element>",
    );
    assert_eq!(
        r.object_styles["A"],
        ObjectStyle {
            caption: "Start".into(),
            show_label: Some(false),
            views: Some(3),
            ordering: Some(2.5),
            layer: Some(4),
            label_mode: Some(3),
            tooltip_mode: Some(2),
            color: Some([10, 20, 30]),
            alpha: Some(0.25),
            fill: Some(Box::new(Fill {
                pattern: 8,
                hatch: Some([60, 12]),
                symbol: Some("*".into()),
                inverse: Some(true),
            })),
            // An empty channel is 0, as the reference reader fills it in.
            dynamic_color: Some(Box::new(DynamicColor {
                red: "r".into(),
                green: "0".into(),
                blue: "1 - r".into(),
                alpha: Some("r".into()),
                space: 2,
            })),
            background: Some([1, 2, 3, 128]),
            line: Some(LineStyle {
                kind: 15,
                thickness: Some(7),
                hidden_kind: Some(2),
                opacity: Some(99),
                draw_arrow: None,
            }),
            point_size: Some(7),
            point_style: Some(3),
            fixed: Some(true),
            auxiliary: Some(true),
            selection_allowed: Some(false),
            trace: Some(true),
            decoration: Some(2),
            animation: Some(Box::new(SliderAnimation {
                step: Some("r".into()),
                speed: Some("2".into()),
                mode: 3,
                playing: false,
            })),
            ..ObjectStyle::default()
        }
    );
    assert_eq!(
        r.object_styles["a"].slider,
        Some(Box::new(SliderLayout {
            absolute: true,
            position: Some([20.0, 40.0]),
            width: Some(150.0),
            flags: Some(SliderFlags {
                fixed: true,
                show_algebra: true,
                horizontal: false,
            }),
        }))
    );
    assert_eq!(r.object_styles["t"].rounding, Some(Rounding::Decimals(4)));
    // An object without any tag still has an entry: its tags stay left out.
    assert_eq!(
        r.object_styles["B"],
        ObjectStyle {
            untagged: true,
            ..ObjectStyle::default()
        }
    );
    // Without a `<show>` tag there are no view bits.
    assert_eq!(r.object_styles["a"].views, None);
    assert!(!r.object_styles["A"].untagged);
    assert!(r.hidden.is_empty());
}

#[test]
fn expressions_the_kernel_does_not_evaluate_are_not_kept() {
    let r = ok(
        "<element type=\"point\" label=\"B\"><coords x=\"0\" y=\"0\" z=\"1\"/></element>\
         <element type=\"point\" label=\"A\"><coords x=\"1\" y=\"2\" z=\"1\"/>\
         <objColor r=\"10\" g=\"20\" b=\"30\" alpha=\"0\" \
         dynamicr=\"Execute[{&quot;Delete[B]&quot;}]\" dynamicg=\"1\" dynamicb=\"missing\"/>\
         <animation step=\"RunClickScript[A]\" speed=\"x(B) + 1\" type=\"1\"/></element>",
    );
    let style = &r.object_styles["A"];
    assert_eq!(style.color, Some([10, 20, 30]));
    assert_eq!(style.dynamic_color, None);
    let animation = style.animation.as_deref().unwrap();
    assert_eq!(animation.step, None);
    assert_eq!(animation.speed.as_deref(), Some("x(B) + 1"));
    let notes = r.notes.join("\n");
    assert!(notes.contains("dynamic colour of `A`"), "{notes}");
    assert!(notes.contains("animation step of `A`"), "{notes}");
}

#[test]
fn style_arithmetic_over_undefined_inputs_is_kept() {
    // `a` is negative when the file is read, so `Q` is undefined: the
    // expressions have no value right now, but the kernel evaluates them.
    let r = ok(
        "<element type=\"numeric\" label=\"a\"><value val=\"-1\"/></element>\
         <expression label=\"Q\" exp=\"(sqrt(a), 0)\" type=\"point\"/>\
         <element type=\"point\" label=\"Q\"><coords x=\"NaN\" y=\"NaN\" z=\"NaN\"/></element>\
         <element type=\"point\" label=\"A\"><coords x=\"1\" y=\"2\" z=\"1\"/>\
         <objColor r=\"10\" g=\"20\" b=\"30\" alpha=\"0\" \
         dynamicr=\"x(Q) / 10\" dynamicg=\"sqrt(a)\" dynamicb=\"1\"/>\
         <animation step=\"sqrt(a)\" speed=\"x(Q) + 1\" type=\"1\"/></element>\
         <element type=\"point\" label=\"B\"><coords x=\"1\" y=\"2\" z=\"1\"/>\
         <animation step=\"Sum(Sequence(k, k, 1, 10))\" speed=\"a := 3\" type=\"1\"/></element>",
    );
    let style = &r.object_styles["A"];
    let dynamic = style.dynamic_color.as_deref().unwrap();
    assert_eq!(
        [&dynamic.red, &dynamic.green, &dynamic.blue].map(String::as_str),
        ["x(Q) / 10", "sqrt(a)", "1"]
    );
    let animation = style.animation.as_deref().unwrap();
    assert_eq!(animation.step.as_deref(), Some("sqrt(a)"));
    assert_eq!(animation.speed.as_deref(), Some("x(Q) + 1"));
    // A list command the kernel parses without running and an assignment
    // are still refused.
    let animation = r.object_styles["B"].animation.as_deref().unwrap();
    assert_eq!((&animation.step, &animation.speed), (&None, &None));
    let notes = r.notes.join("\n");
    assert!(!notes.contains("of `A`"), "{notes}");
    assert!(notes.contains("animation step of `B`"), "{notes}");
    assert!(notes.contains("animation speed of `B`"), "{notes}");
}

/// One styled conic `k` and one slider `n`, with the given tags.
fn styled(conic: &str, slider: &str) -> Imported {
    ok(&format!(
        "{}{}<command name=\"Circle\"><input a0=\"A\" a1=\"B\"/><output a0=\"k\"/></command>\
         <element type=\"conic\" label=\"k\">{conic}</element>\
         <element type=\"numeric\" label=\"n\"><value val=\"2\"/>{slider}</element>",
        free("A", "1", "2"),
        free("B", "3", "1"),
    ))
}

#[test]
fn a_fill_is_read_up_to_the_number_the_reference_reader_stops_at() {
    use geo_import::export::Fill;
    let fill = |tag: &str| {
        let r = styled(tag, "");
        let style = r.object_styles["k"].clone();
        let stops = r
            .notes
            .iter()
            .any(|note| note.contains("`<objColor>` of `k`"));
        (
            style.color,
            style.fill.map(|fill| *fill),
            style.alpha,
            stops,
        )
    };
    // The angle is no number: the colour is set, nothing else.
    assert_eq!(
        fill("<objColor r=\"10\" g=\"20\" b=\"30\" alpha=\"0.5\" hatchAngle=\"abc\" hatchDistance=\"7\" fillType=\"2\" inverseFill=\"true\"/>"),
        (Some([10, 20, 30]), None, None, true)
    );
    // The distance is missing: the angle and the inversion are set, the
    // distance and the pattern keep their defaults, the opacity is not read.
    assert_eq!(
        fill("<objColor r=\"10\" g=\"20\" b=\"30\" alpha=\"0.5\" hatchAngle=\"60\" fillType=\"2\" inverseFill=\"true\"/>"),
        (
            Some([10, 20, 30]),
            Some(Fill {
                pattern: 0,
                hatch: Some([60, Fill::DEFAULT_DISTANCE]),
                symbol: None,
                inverse: Some(true),
            }),
            None,
            true
        )
    );
    // No pattern has the index -1: the hatch is set, the symbol and the
    // opacity are not read.
    assert_eq!(
        fill("<objColor r=\"10\" g=\"20\" b=\"30\" alpha=\"0.5\" hatchAngle=\"60\" hatchDistance=\"7\" fillType=\"-1\" fillSymbol=\"x\"/>"),
        (
            Some([10, 20, 30]),
            Some(Fill {
                pattern: 1,
                hatch: Some([60, 7]),
                symbol: None,
                inverse: None,
            }),
            None,
            true
        )
    );
    // The opacity is no number: the pattern before it is set.
    assert_eq!(
        fill("<objColor r=\"10\" g=\"20\" b=\"30\" alpha=\"abc\" fillType=\"3\"/>"),
        (
            Some([10, 20, 30]),
            Some(Fill {
                pattern: 3,
                ..Fill::default()
            }),
            None,
            true
        )
    );
    // A complete tag reads everything and stops nothing.
    assert_eq!(
        fill("<objColor r=\"10\" g=\"20\" b=\"30\" alpha=\"0.5\" hatchAngle=\"60\" hatchDistance=\"7\" fillType=\"2\"/>"),
        (
            Some([10, 20, 30]),
            Some(Fill {
                pattern: 2,
                hatch: Some([60, 7]),
                symbol: None,
                inverse: None,
            }),
            Some(0.5),
            false
        )
    );
}

#[test]
fn colour_channels_keep_their_low_byte() {
    let r = styled(
        "<objColor r=\"300\" g=\"-1\" b=\"256\" alpha=\"0.5\"/>\
         <bgColor r=\"300\" g=\"20\" b=\"30\" alpha=\"300\"/>",
        "",
    );
    let style = &r.object_styles["k"];
    assert_eq!(style.color, Some([44, 255, 0]));
    assert_eq!(style.background, Some([44, 20, 30, 44]));
    // A channel that is no 32 bit integer leaves the whole tag unread.
    let r = styled("<objColor r=\"1e2\" g=\"0\" b=\"0\" alpha=\"0.5\"/>", "");
    assert_eq!(r.object_styles["k"].color, None);
    assert_eq!(r.object_styles["k"].alpha, None);
}

#[test]
fn a_line_style_is_read_pattern_first_and_keeps_any_thickness() {
    use geo_import::export::LineStyle;
    let line = |tag: &str| styled(tag, "").object_styles["k"].line;
    // Without a dash pattern nothing is set, not even the thickness.
    assert_eq!(line("<lineStyle thickness=\"7\"/>"), None);
    // The pattern alone is set; the reader stops at the missing thickness.
    assert_eq!(
        line("<lineStyle type=\"20\" typeHidden=\"2\" opacity=\"90\"/>"),
        Some(LineStyle {
            kind: 20,
            ..LineStyle::default()
        })
    );
    // The thickness has no upper bound; a negative one is 0.
    assert_eq!(
        line("<lineStyle thickness=\"300\" type=\"10\"/>")
            .unwrap()
            .thickness,
        Some(300)
    );
    assert_eq!(
        line("<lineStyle thickness=\"-4\" type=\"10\"/>")
            .unwrap()
            .thickness,
        Some(0)
    );
    // A hidden pattern that is no number stops the reader before the opacity.
    assert_eq!(
        line("<lineStyle thickness=\"3\" type=\"10\" typeHidden=\"x\" opacity=\"90\" drawArrow=\"true\"/>"),
        Some(LineStyle {
            kind: 10,
            thickness: Some(3),
            ..LineStyle::default()
        })
    );
    assert_eq!(
        line("<lineStyle thickness=\"3\" type=\"10\" opacity=\"90\" drawArrow=\"true\"/>"),
        Some(LineStyle {
            kind: 10,
            thickness: Some(3),
            hidden_kind: None,
            opacity: Some(90),
            draw_arrow: Some(true),
        })
    );
}

#[test]
fn a_slider_tag_is_read_up_to_the_number_the_reference_reader_stops_at() {
    use geo_import::export::{SliderFlags, SliderLayout};
    let layout = |tag: &str| *styled("", tag).object_styles["n"].slider.clone().unwrap();
    // Without a width the flags after it are not read.
    assert_eq!(
        layout("<slider min=\"0\" max=\"5\" x=\"10\" y=\"10\" fixed=\"true\" horizontal=\"false\" showAlgebra=\"true\"/>"),
        SliderLayout {
            absolute: false,
            position: Some([10.0, 10.0]),
            width: None,
            flags: None,
        }
    );
    // The reference reads `NaN` as a width.
    let nan = layout("<slider min=\"0\" max=\"5\" x=\"10\" y=\"10\" width=\"NaN\" fixed=\"true\" horizontal=\"false\" showAlgebra=\"true\"/>");
    assert!(nan.width.is_some_and(f64::is_nan));
    assert_eq!(
        nan.flags,
        Some(SliderFlags {
            fixed: true,
            show_algebra: true,
            horizontal: false,
        })
    );
    // A position without `y` stops the reader before the width.
    assert_eq!(
        layout(
            "<slider min=\"0\" max=\"5\" absoluteScreenLocation=\"true\" x=\"10\" width=\"100\"/>"
        ),
        SliderLayout {
            absolute: true,
            ..SliderLayout::default()
        }
    );
    // The bounds are read before any of this.
    let r = styled("", "<slider min=\"0\" max=\"5\" x=\"10\"/>");
    let c = geo_kernel::script::load(&r.script.join("\n")).unwrap();
    assert_eq!(c.slider_bounds(c.id("n").unwrap()), [Some(0.0), Some(5.0)]);
}

#[test]
fn a_show_tag_shows_only_what_it_says() {
    // Without `object="true"` the reference reader hides the object and the
    // label follows `label`; earlier versions of the importer showed an
    // object unless the tag said `false`.
    let r = ok("<element type=\"point\" label=\"A\"><show label=\"true\"/>\
         <coords x=\"1\" y=\"2\" z=\"1\"/></element>\
         <element type=\"point\" label=\"B\"><show object=\"yes\"/>\
         <coords x=\"1\" y=\"2\" z=\"1\"/></element>\
         <element type=\"point\" label=\"C\"><show object=\"true\"/>\
         <coords x=\"1\" y=\"2\" z=\"1\"/></element>\
         <element type=\"point\" label=\"D\"><coords x=\"1\" y=\"2\" z=\"1\"/></element>");
    assert_eq!(r.hidden, ["A", "B"]);
    assert_eq!(r.object_styles["A"].show_label, Some(true));
    assert_eq!(r.object_styles["C"].show_label, Some(false));
    // No tag at all leaves the object visible in the editor.
    assert!(r.object_styles["D"].untagged);
}

#[test]
fn a_style_expression_the_kernel_cannot_evaluate_is_reported() {
    let r = ok(
        "<element type=\"numeric\" label=\"n\"><value val=\"2\"/></element>\
         <element type=\"point\" label=\"A\"><coords x=\"1\" y=\"2\" z=\"1\"/>\
         <animation step=\"n / 4\" speed=\"Limit[x^2, 2]\" type=\"1\" playing=\"false\"/></element>",
    );
    let animation = r.object_styles["A"].animation.clone().unwrap();
    assert_eq!(animation.step.as_deref(), Some("n / 4"));
    assert_eq!(animation.speed, None);
    assert!(
        r.notes
            .iter()
            .any(|note| note.starts_with("animation speed of `A`:")),
        "{:?}",
        r.notes
    );
}

/// The reference's reader sets a slider's expression endpoints while it
/// reads the slider, so an optimizer saved after it searches at once; a
/// recorded reload gives `m = 3`, and `m = 2` after `w = 1`.
#[test]
fn an_optimizer_over_a_slider_with_expression_endpoints_searches_after_import() {
    use geo_kernel::{script, Value};

    let imported = ok(
        r#"<element type="numeric" label="b"><value val="6"/></element>
           <element type="numeric" label="a"><value val="2"/><slider min="0" max="b"/></element>
           <element type="numeric" label="w"><value val="2"/></element>
           <expression label="f" exp="(a - w - 1)^2"/>
           <element type="numeric" label="f"><value val="1"/></element>
           <command name="Minimize"><input a0="f" a1="a"/><output a0="m"/></command>
           <element type="numeric" label="m"><value val="3"/></element>"#,
    );
    assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
    let mut c = script::load(&imported.script.join("\n")).unwrap();
    let m = |c: &geo_kernel::Construction| match c.value(c.id("m").unwrap()) {
        Value::Number(v) => v,
        other => panic!("the optimizer is {other:?}"),
    };
    assert!((m(&c) - 3.0).abs() < 1e-6, "{}", m(&c));
    let w = c.id("w").unwrap();
    c.set_number(w, 1.0).unwrap();
    assert!((m(&c) - 2.0).abs() < 1e-6, "{}", m(&c));
}
