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
fn the_saved_value_is_checked_against_the_computed_one() {
    let first = "<element type=\"point\" label=\"P\"><coords x=\"1\" y=\"0\" z=\"1\"/></element>";
    let second = "<element type=\"point\" label=\"Q\"><coords x=\"3\" y=\"0\" z=\"1\"/></element>";
    let r = ok(&line_circle("<output a0=\"P\" a1=\"Q\"/>", &(first.to_string() + second)));
    let swapped = ok(&line_circle(
        "<output a0=\"P\" a1=\"Q\"/>",
        &(second.replace('Q', "P") + &first.replace('P', "Q")),
    ));
    // Exactly one of the two orders agrees with the kernel's index order,
    // and the other is reported, not dropped.
    assert_eq!(
        r.mismatched.len() + swapped.mismatched.len(),
        2,
        "{:?} {:?}",
        r.mismatched,
        swapped.mismatched
    );
    let wrong = if r.mismatched.is_empty() { &swapped } else { &r };
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
    for yes in ["(1/2, 3)", "2^10", "1.5E-3", "(1, 2) ", "-(3 + 4) * 2", "1e5"] {
        assert!(is_constant_expression(yes), "{yes}");
    }
    for no in ["a + 1", "pi", "sqrt(2)", "e", "E5", "(A, 1)", "", "()", "x(A)", "1 + t_{1}"] {
        assert!(!is_constant_expression(no), "{no}");
    }
    let r = ok("<expression label=\"A\" exp=\"(1/2, 3)\" type=\"point\"/>\
                <element type=\"point\" label=\"A\"><coords x=\"0.5\" y=\"3\" z=\"1\"/></element>\
                <expression label=\"n\" exp=\"2^10\" type=\"numeric\"/>\
                <element type=\"numeric\" label=\"n\"><value val=\"1024\"/></element>\
                <expression label=\"m\" exp=\"n + 1\" type=\"numeric\"/>\
                <element type=\"numeric\" label=\"m\"><value val=\"1025\"/></element>");
    assert_eq!(r.script, ["A = (0.5, 3)", "n = 1024"]);
    assert_eq!(r.skipped.len(), 1);
    assert_eq!(r.skipped[0].label, "m");
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
    assert!(r.notes.iter().any(|n| n.contains("2 construction sections")));
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
    assert!(notes.contains("1 objects with a condition"), "{notes}");
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
    assert!(looks_like_construction(b"<geogebra><construction/></geogebra>"));
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
