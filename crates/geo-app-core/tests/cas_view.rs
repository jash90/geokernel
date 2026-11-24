use geo_app_core::cas::{MAX_INPUT_BYTES, MAX_OUTPUT_BYTES, MAX_ROWS};
use geo_app_core::workbench::Workbench;

fn outputs(wb: &Workbench) -> Vec<&str> {
    wb.cas_rows()
        .iter()
        .map(|row| row.output.text.as_str())
        .collect()
}

#[test]
fn evaluates_and_prints_cas_rows_against_the_construction() {
    let mut wb = Workbench::from_script("a = 3").unwrap();
    assert!(wb.submit_cas(0, "a / 2"));
    assert!(wb.submit_cas(1, "1 / 0"));
    assert_eq!(outputs(&wb), ["3 / 2", "?"]);
}

#[test]
fn editing_recomputes_following_rows_and_deletion_removes_dependents() {
    let mut wb = Workbench::default();
    assert!(wb.submit_cas(0, "2"));
    assert!(wb.submit_cas(1, "$1 + 3"));
    assert!(wb.submit_cas(2, "$ + 1"));
    assert!(wb.submit_cas(3, "10"));
    assert!(wb.submit_cas(0, "7"));
    assert_eq!(outputs(&wb), ["7", "10", "11", "10"]);
    assert!(wb.delete_cas(0));
    assert_eq!(outputs(&wb), ["10"]);
}

#[test]
fn cas_survives_loading_and_saving() {
    let xml = br#"<geogebra><construction>
      <cascell><cellPair><inputCell><expression value="2 + 3"/></inputCell>
      <outputCell><expression value="5" evalCommand="Evaluate"/></outputCell></cellPair></cascell>
      <cascell><cellPair><inputCell><expression value="$1 * 2"/></inputCell>
      <outputCell><expression value="10" evalCommand="Evaluate"/></outputCell></cellPair></cascell>
    </construction></geogebra>"#;
    let imported = geo_import::import(xml).unwrap();
    assert_eq!(imported.cas_rows.len(), 2);
    let mut wb = Workbench::default();
    wb.load_bytes(xml, "reference.xml").unwrap();
    assert_eq!(outputs(&wb), ["5", "10"]);
    let bytes = wb.export_ggb(None).unwrap();
    let mut back = Workbench::default();
    back.load_for_export(&bytes, "saved.ggb").unwrap();
    assert_eq!(back.cas_rows(), wb.cas_rows());
    assert!(back.submit_cas(0, "7"));
    assert_eq!(outputs(&back), ["7", "14"]);
    back.load_script("").unwrap();
    assert!(back.cas_rows().is_empty());
}

#[test]
fn bounds_rows_input_and_reference_expansion() {
    let mut wb = Workbench::default();
    assert!(!wb.submit_cas(0, &"x".repeat(MAX_INPUT_BYTES + 1)));
    let input = format!("{{{}}}", vec!["123456789"; 300].join(","));
    assert!(wb.submit_cas(0, &input));
    assert_ne!(outputs(&wb)[0], "?");
    assert!(wb.submit_cas(1, &vec!["$1"; 40].join("+")));
    assert_eq!(outputs(&wb)[1], "?");
    for index in 2..MAX_ROWS {
        assert!(wb.submit_cas(index, "1"));
    }
    assert!(!wb.submit_cas(MAX_ROWS, "1"));
    assert_eq!(wb.cas_rows().len(), MAX_ROWS);
    assert!(wb
        .cas_rows()
        .iter()
        .all(|row| row.output.latex.len() <= MAX_OUTPUT_BYTES));
}

#[test]
fn refuses_an_over_budget_cascade_without_changing_any_row() {
    let mut wb = Workbench::default();
    let source = format!("1{}", " ".repeat(MAX_INPUT_BYTES / 2));
    for row in 0..100 {
        assert!(wb.submit_cas(row, &source));
    }
    assert!(!wb.submit_cas(0, "9"));
    assert_eq!(wb.cas_rows()[0].input, source);
    assert_eq!(outputs(&wb), vec!["1"; 100]);
}

#[test]
fn oversized_output_is_refused_before_it_reaches_the_view() {
    let mut wb = Workbench::default();
    let source = format!("{{{}}}", vec!["1 / 2"; 400].join(","));
    let expected = wb.construction().evaluate_cas_row(&source);
    assert!(expected.latex.len() > MAX_OUTPUT_BYTES);
    assert!(wb.submit_cas(0, &source));
    assert_eq!(outputs(&wb), ["?"]);
}

#[test]
fn deleting_an_unrelated_row_keeps_an_undefined_reference() {
    let mut wb = Workbench::default();
    assert!(wb.submit_cas(0, "1"));
    assert!(wb.submit_cas(1, "$100"));
    assert!(wb.submit_cas(2, "3"));
    assert!(wb.delete_cas(0));
    assert_eq!(outputs(&wb), ["?", "3"]);
    assert_eq!(wb.cas_rows()[0].input, "$100");
}

#[test]
fn the_file_reader_checks_the_cas_row_and_attribute_limits() {
    use geo_import::{import, ImportError};

    let xml = |body| format!("<geogebra><construction>{body}</construction></geogebra>");
    let rows = xml("<cascell><cellPair/></cascell>".repeat(MAX_ROWS + 1));
    assert_eq!(import(rows.as_bytes()), Err(ImportError::TooManyCasRows));

    let large = "é".repeat(MAX_INPUT_BYTES / 2 + 1);
    let attrs = xml(format!(
        "<cascell><cellPair><inputCell><expression value=\"{large}\"/></inputCell></cellPair></cascell>"
    ));
    assert_eq!(import(attrs.as_bytes()), Err(ImportError::AttributeTooLong));
}
