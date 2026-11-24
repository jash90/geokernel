//! Damaged and unusual input: every case is an error or a skipped object with
//! a reason, never a panic.

use geo_import::{import, ImportError};
use std::io::Write;

fn file(body: &str) -> String {
    format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><geogebra><construction>{body}</construction></geogebra>")
}

fn point(label: &str, x: &str, y: &str, z: &str) -> String {
    format!(
        "<expression label=\"{label}\" exp=\"(0, 0)\" type=\"point\"/>\
         <element type=\"point\" label=\"{label}\"><show object=\"true\" label=\"true\"/>\
         <coords x=\"{x}\" y=\"{y}\" z=\"{z}\"/></element>"
    )
}

fn line(label: &str, a: &str, b: &str) -> String {
    format!(
        "<command name=\"Line\"><input a0=\"{a}\" a1=\"{b}\"/><output a0=\"{label}\"/></command>\
         <element type=\"line\" label=\"{label}\"><coords x=\"0\" y=\"1\" z=\"0\"/></element>"
    )
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            w.start_file(*name, options).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }
    out
}

fn ok(body: &str) -> geo_import::Imported {
    import(file(body).as_bytes()).unwrap()
}

#[test]
fn not_a_construction() {
    assert_eq!(import(b""), Err(ImportError::NoConstruction));
    assert_eq!(import(b"hello"), Err(ImportError::NoConstruction));
    assert_eq!(import(b"<geogebra/>"), Err(ImportError::NoConstruction));
    assert!(matches!(import(b"\xff\xfe\x00"), Err(ImportError::Xml(_))));
    assert!(matches!(import(b"<a><b></a>"), Err(ImportError::Xml(_))));
    // an empty construction is a construction
    let empty = import(b"<geogebra><construction/></geogebra>").unwrap();
    assert!(empty.script.is_empty() && empty.skipped.is_empty());
}

#[test]
fn truncated_xml_is_an_error() {
    let full =
        file(&(point("A", "1", "2", "1") + &point("B", "3", "4", "1") + &line("f", "A", "B")));
    let mut errors = 0;
    for len in 0..full.len() {
        if !full.is_char_boundary(len) {
            continue;
        }
        if import(&full.as_bytes()[..len]).is_err() {
            errors += 1;
        }
    }
    // everything cut off before the closing tags is an error
    assert!(errors >= full.find("</construction>").unwrap());
}

#[test]
fn zip_errors() {
    let xml = file(&point("A", "1", "2", "1"));
    let good = zip_of(&[("geogebra.xml", xml.as_bytes())]);
    assert_eq!(import(&good).unwrap().script, ["A = (1, 2)"]);
    // other entries around it do not matter
    let more = zip_of(&[
        ("geogebra_defaults2d.xml", b"<x/>"),
        ("geogebra.xml", xml.as_bytes()),
        ("geogebra_javascript.js", b"function ggbOnInit() {}"),
    ]);
    assert_eq!(import(&more).unwrap().script, ["A = (1, 2)"]);
    // no geogebra.xml
    let none = zip_of(&[("other.xml", xml.as_bytes())]);
    assert_eq!(import(&none), Err(ImportError::MissingXml));
    // not UTF-8 inside
    let bad = zip_of(&[("geogebra.xml", b"<geogebra>\xff\xfe</geogebra>")]);
    assert!(matches!(import(&bad), Err(ImportError::Xml(_))));
    // every truncation of an archive is an error (the directory is at the end)
    for len in 0..good.len() {
        assert!(import(&good[..len]).is_err(), "prefix of {len} bytes");
    }
    // only the signature
    assert!(matches!(import(b"PK\x03\x04"), Err(ImportError::Zip(_))));
    // a byte flipped in the stored data fails the checksum or the XML, or is
    // harmless; it never panics
    for i in 0..good.len() {
        let mut b = good.clone();
        b[i] ^= 0x55;
        let _ = import(&b);
    }
}

#[test]
fn a_damaged_optional_directory_entry_does_not_prevent_opening_xml() {
    let xml = file(&point("A", "1", "2", "1"));
    let mut archive = zip_of(&[("geogebra.xml", xml.as_bytes()), ("thumbnail.png", b"")]);
    let damaged = archive
        .windows(4)
        .rposition(|b| b == b"PK\x01\x02")
        .unwrap();
    archive[damaged] = 0;
    let imported = import(&archive).unwrap();
    assert_eq!(imported.script, ["A = (1, 2)"]);
    assert!(!imported.notes.is_empty());
}

#[test]
fn bad_numbers_are_errors_not_panics() {
    let e = import(file(&point("A", "abc", "2", "1")).as_bytes()).unwrap_err();
    assert_eq!(
        e,
        ImportError::BadNumber {
            label: "A".into(),
            text: "abc".into()
        }
    );
    assert!(matches!(
        import(file(&point("A", "1", "", "1")).as_bytes()),
        Err(ImportError::BadNumber { .. })
    ));
    let free_number = |v: &str| {
        file(&format!(
            "<element type=\"numeric\" label=\"r\"><value val=\"{v}\"/></element>"
        ))
    };
    assert!(matches!(
        import(free_number("three").as_bytes()),
        Err(ImportError::BadNumber { .. })
    ));
    // not finite: a skipped object with a reason
    for v in ["NaN", "Infinity", "-Infinity"] {
        let r = import(free_number(v).as_bytes()).unwrap();
        assert!(r.script.is_empty(), "{v}");
        assert_eq!(r.skipped.len(), 1, "{v}");
    }
    // no number at all
    let r = ok("<element type=\"numeric\" label=\"r\"/>");
    assert_eq!(r.skipped.len(), 1);
    let r = ok("<element type=\"point\" label=\"A\"/>");
    assert_eq!(r.skipped.len(), 1);
}

#[test]
fn duplicate_labels_are_errors() {
    let two = point("A", "1", "2", "1") + &point("A", "3", "4", "1");
    assert_eq!(
        import(file(&two).as_bytes()),
        Err(ImportError::DuplicateLabel("A".into()))
    );
    // a command output that repeats a label
    let body = point("A", "1", "2", "1") + &point("B", "3", "4", "1") + &line("A", "A", "B");
    assert_eq!(
        import(file(&body).as_bytes()),
        Err(ImportError::DuplicateLabel("A".into()))
    );
    let body =
        "<command name=\"Tangent\"><input a0=\"P\" a1=\"k\"/><output a0=\"t\" a1=\"t\"/></command>";
    assert_eq!(
        import(file(body).as_bytes()),
        Err(ImportError::DuplicateLabel("t".into()))
    );
}

#[test]
fn outputs_that_do_not_fit() {
    // numbered with a gap
    let body =
        "<command name=\"Tangent\"><input a0=\"P\" a1=\"k\"/><output a0=\"t\" a2=\"u\"/></command>";
    assert!(matches!(
        import(file(body).as_bytes()),
        Err(ImportError::BadOutputs { .. })
    ));
    let circle = point("A", "0", "0", "1")
        + "<expression label=\"r\" exp=\"2\"/><element type=\"numeric\" label=\"r\"><value val=\"2\"/></element>"
        + "<command name=\"Circle\"><input a0=\"A\" a1=\"r\"/><output a0=\"k\"/></command>"
        + &point("P", "5", "1", "1");
    // a third output with a label: the kernel's command makes two
    let body = format!(
        "{circle}<command name=\"Tangent\"><input a0=\"P\" a1=\"k\"/>\
         <output a0=\"t_{{1}}\" a1=\"t_{{2}}\" a2=\"t_{{3}}\"/></command>"
    );
    let r = import(file(&body).as_bytes()).unwrap();
    assert!(r.script.len() >= 3);
    assert!(r.skipped.iter().any(|s| s.label == "t_{1}"));
    // a third output without a label is not created and does no harm
    let body = format!(
        "{circle}<command name=\"Tangent\"><input a0=\"P\" a1=\"k\"/>\
         <output a0=\"t_{{1}}\" a1=\"t_{{2}}\" a2=\"\"/></command>"
    );
    let r = import(file(&body).as_bytes()).unwrap();
    assert!(r.skipped.is_empty());
    // the right number is accepted, with an empty slot allowed
    let body = format!(
        "{circle}<command name=\"Tangent\"><input a0=\"P\" a1=\"k\"/><output a0=\"t_{{1}}\" a1=\"t_{{2}}\"/></command>"
    );
    let r = import(file(&body).as_bytes()).unwrap();
    assert!(r.skipped.is_empty());
    assert_eq!(r.script.last().unwrap(), "{t_{1}, t_{2}} = Tangent(P, k)");
    // one output for a command that makes two: the outputs are named by the file
    let body = format!(
        "{circle}<command name=\"Circle\"><input a0=\"A\" a1=\"r\"/><output a0=\"a\" a1=\"b\"/></command>"
    );
    let r = import(file(&body).as_bytes()).unwrap();
    assert_eq!(r.skipped.len(), 2);
    // outputs that have no label at all
    let r = ok("<command name=\"Line\"><input a0=\"A\" a1=\"B\"/><output a0=\"\"/></command>");
    assert_eq!(r.skipped.len(), 1);
    assert_eq!(r.skipped[0].label, "");
    let r = ok("<command name=\"Line\"><input a0=\"A\" a1=\"B\"/></command>");
    assert_eq!(r.skipped.len(), 1);
}

#[test]
fn free_objects() {
    // Keep the raw coordinates of both a weighted point and a direction.
    let r = ok(&(point("A", "4", "6", "2") + &point("B", "1", "1", "0")));
    assert_eq!(
        r.script,
        [
            "A = $homogeneousPoint(4, 6, 2)",
            "B = $homogeneousPoint(1, 1, 0)"
        ]
    );
    assert!(r.skipped.is_empty());
    let c = geo_kernel::script::load(&r.script.join("\n")).unwrap();
    assert_eq!(
        c.value(c.id("A").unwrap()),
        geo_kernel::Value::Point { x: 2.0, y: 3.0 }
    );
    assert_eq!(c.value(c.id("B").unwrap()), geo_kernel::Value::Undefined);
    // a lower-case point and an upper-case vector need the forced forms
    let vector = |label: &str| {
        format!(
            "<element type=\"vector\" label=\"{label}\"><coords x=\"1\" y=\"2\" z=\"0\"/></element>"
        )
    };
    let r =
        ok(&(point("p", "1", "2", "1") + &point("P", "1", "2", "1") + &vector("v") + &vector("V")));
    assert_eq!(
        r.script,
        [
            "p = $point(1, 2)",
            "P = (1, 2)",
            "v = (1, 2)",
            "V = $vector(1, 2)"
        ]
    );
    // the exact value of the element, not the rounded text of the expression
    let body = "<expression label=\"r\" exp=\"0.3\"/>\
                <element type=\"numeric\" label=\"r\"><value val=\"0.30000000000000004\"/></element>\
                <element type=\"numeric\" label=\"s\"><value val=\"1.0E-6\"/></element>\
                <element type=\"numeric\" label=\"t\"><value val=\"-1.2345678901234567E21\"/></element>";
    let r = ok(body);
    let c = geo_kernel::script::load(&r.script.join("\n")).unwrap();
    let value = |l: &str| match c.value(c.id(l).unwrap()) {
        geo_kernel::Value::Number(v) => v,
        other => panic!("{other:?}"),
    };
    assert_eq!(value("r"), 0.1 + 0.2);
    assert_eq!(value("s"), 1.0e-6);
    assert_eq!(value("t"), -1.2345678901234567e21);
    // hidden objects are listed, skipped ones are not
    let r = ok(
        "<element type=\"point\" label=\"A\"><show object=\"false\" label=\"true\"/>\
                <coords x=\"1\" y=\"2\" z=\"1\"/></element>\
                <element type=\"text\" label=\"T\"><show object=\"false\"/></element>",
    );
    assert_eq!(r.hidden, ["A"]);
    assert_eq!(r.skipped.len(), 1);
}

#[test]
fn skipped_objects_carry_their_dependents() {
    let body = point("A", "0", "0", "1")
        + &point("B", "1", "0", "1")
        + "<command name=\"NoSuchCommand\"><input a0=\"A\" a1=\"B\"/><output a0=\"f\"/></command>"
        + &line("g", "A", "f")
        + &line("h", "g", "B")
        + "<command name=\"Circle\"><input a0=\"A\" a1=\"(MISSING, 2)\"/><output a0=\"k\"/></command>"
        + "<command name=\"Circle\"><input a0=\"A\" a1=\"B\"/><output a0=\"c\"/></command>"
        + "<expression label=\"e\" exp=\"A + B\"/><element type=\"numeric\" label=\"e\"><value val=\"1\"/></element>"
        + "<command name=\"Segment\"><input a0=\"A\" a1=\"e\"/><output a0=\"s\"/></command>";
    let r = ok(&body);
    assert_eq!(r.script, ["A = (0, 0)", "B = (1, 0)", "c = Circle(A, B)"]);
    let labels: Vec<&str> = r.skipped.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels, ["f", "g", "h", "k", "e", "s"]);
    assert!(
        r.skipped[0].reason.contains("NoSuchCommand"),
        "{:?}",
        r.skipped[0]
    );
    assert!(r.skipped[1]
        .reason
        .contains("depends on the skipped object `f`"));
    assert!(r.skipped[2]
        .reason
        .contains("depends on the skipped object `g`"));
    assert!(r.skipped[3]
        .reason
        .contains("cannot evaluate command input"));
    assert!(r.skipped[4].reason.contains("expression"));
    assert_eq!(r.skipped[4].kind, "numeric");
    assert!(r.skipped[5]
        .reason
        .contains("depends on the skipped object `e`"));
}

#[test]
fn hostile_labels_and_names_cannot_inject_lines() {
    let body = point("A", "0", "0", "1")
        + &point("A\nB", "1", "1", "1")
        + &point("C # x", "1", "1", "1")
        + "<command name=\"Line(A, A) #\"><input a0=\"A\" a1=\"A\"/><output a0=\"f\"/></command>"
        + "<command name=\"Line\"><input a0=\"A\" a1=\"A\"/><output a0=\"g = \"/></command>"
        + "<command name=\"Line\"><input a0=\"A, A\" a1=\"A\"/><output a0=\"h\"/></command>"
        + "<command name=\"Line\"><input a0=\"A\" a1=\"A\"\n a2=\"x\"/><output a0=\"i\"/></command>";
    let r = ok(&body);
    assert_eq!(r.script[0], "A = (0, 0)");
    for line in &r.script {
        assert!(!line.contains('\n') && !line.contains('#'), "{line:?}");
    }
    assert_eq!(r.script.len(), 1, "{:?}", r.script);
    assert_eq!(r.skipped.len(), 6);
}

#[test]
fn settings_and_notes() {
    let xml = "<geogebra><kernel><continuous val=\"true\"/><decimals val=\"5\"/>\
               <angleUnit val=\"radiant\"/></kernel><construction/></geogebra>";
    let r = import(xml.as_bytes()).unwrap();
    assert!(r.settings.continuous);
    assert_eq!(r.settings.decimals, Some(5));
    assert_eq!(r.settings.angle_unit.as_deref(), Some("radiant"));
    assert_eq!(r.notes.len(), 1);
}

#[test]
fn entities_in_labels() {
    let body = "<element type=\"point\" label=\"A&apos;\"><coords x=\"1\" y=\"2\" z=\"1\"/></element>\
                <command name=\"Midpoint\"><input a0=\"A&apos;\" a1=\"A&apos;\"/><output a0=\"M&apos;&apos;\"/></command>";
    let r = ok(body);
    assert_eq!(r.script, ["A' = (1, 2)", "M'' = Midpoint(A', A')"]);
    // a bad entity is a damaged file
    let r = import(file("<element type=\"point\" label=\"A&bogus;\"/>").as_bytes());
    assert!(matches!(r, Err(ImportError::Xml(_))));
}

#[test]
fn a_line_that_creates_other_objects_than_the_file_has_is_skipped() {
    // The file names one output of a command that makes two; the kernel
    // refuses the line before it creates anything, so the scratch
    // construction is as before and the next line still works.
    let body = point("A", "0", "0", "1")
        + &point("B", "4", "0", "1")
        + &point("C", "0", "1", "1")
        + &line("f", "A", "B")
        + &line("g", "A", "C")
        + "<command name=\"AngularBisector\"><input a0=\"f\" a1=\"g\"/><output a0=\"b_{1}\"/></command>"
        + &line("h", "B", "C");
    let r = ok(&body);
    assert_eq!(r.skipped.len(), 1);
    assert!(
        r.skipped[0].reason.contains("outputs"),
        "{:?}",
        r.skipped[0]
    );
    assert_eq!(r.script.last().unwrap(), "h = Line(B, C)");
    let c = geo_kernel::script::load(&r.script.join("\n")).unwrap();
    assert!(c.id("b_{2}").is_none());
}

#[test]
fn central_and_local_zip_names_must_match() {
    let xml = file(&point("A", "1", "2", "1"));
    let mut archive = zip_of(&[("geogebra.xml", xml.as_bytes())]);
    // Change only the local name. The directory still identifies the XML.
    archive[30] = b'G';
    let err = import(&archive).unwrap_err();
    assert!(matches!(err, ImportError::Zip(_)));
    assert!(err.to_string().contains("name"), "{err}");
}
