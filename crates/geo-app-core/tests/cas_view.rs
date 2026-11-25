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
fn an_unrelated_edit_skips_independent_long_rows() {
    let mut wb = Workbench::default();
    let source = format!("1{}", " ".repeat(MAX_INPUT_BYTES / 2));
    for row in 0..100 {
        assert!(wb.submit_cas(row, &source));
    }
    assert!(wb.submit_cas(0, "9"));
    assert_eq!(outputs(&wb)[0], "9");
    assert_eq!(outputs(&wb)[1..], vec!["1"; 99]);
}

#[test]
fn charges_failed_expansions_and_restarts_the_budget_after_a_refusal() {
    let mut wb = Workbench::default();
    let source = format!("1{}", " ".repeat(4095));
    assert!(wb.submit_cas(0, &source));
    for row in 1..=30 {
        assert!(wb.submit_cas(row, "$1$+$1$+$1$+$1$"));
    }
    let undo = wb.undo_depth();
    assert!(!wb.submit_cas(0, &format!("2{}", " ".repeat(4095))));
    assert_eq!(wb.undo_depth(), undo);
    assert_eq!(wb.cas_rows()[0].input, source);
    assert!(wb.submit_cas(0, "2"));
    assert_eq!(outputs(&wb)[1..], vec!["8"; 30]);
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
fn oversized_saved_cells_still_open_and_round_trip() {
    let large = "1".to_owned() + &" ".repeat(1 << 20);
    let small = r#"<cascell><cellPair><inputCell><expression value="7"/></inputCell>
      <outputCell><expression value="7" native="true"/></outputCell></cellPair></cascell>"#;
    let xml = format!(
        r#"<geogebra><construction><cascell><cellPair>
        <inputCell><expression value="{large}"/></inputCell>
        <outputCell><expression value="1" native="true"/></outputCell></cellPair></cascell>
        {}</construction></geogebra>"#,
        small.repeat(MAX_ROWS)
    );
    let mut wb = Workbench::default();
    wb.load_bytes(xml.as_bytes(), "large.xml").unwrap();
    assert_eq!(wb.cas_rows().len(), MAX_ROWS + 1);
    assert_eq!(wb.cas_rows()[0].input, large);
    assert_eq!(outputs(&wb)[0], "1");
    assert_eq!(outputs(&wb)[MAX_ROWS], "7");
    assert!(wb.cas_rows()[0].unevaluated);
    assert!(wb.cas_rows()[MAX_ROWS].unevaluated);
    assert!(wb.cas_rows()[MAX_ROWS].output.exact.is_none());
    assert!(!wb.submit_cas(MAX_ROWS, "2"));
    assert!(wb.submit_cas(1, "3"));
    let bytes = wb.export_ggb(None).unwrap();
    wb.load_bytes(&bytes, "large.ggb").unwrap();
    assert_eq!(wb.cas_rows()[0].input, large);
    assert_eq!(outputs(&wb)[MAX_ROWS], "7");
    assert!(wb.cas_rows()[MAX_ROWS].unevaluated);
}

#[test]
fn editing_another_row_never_promotes_a_comment_to_a_value() {
    let xml = br#"<geogebra><construction>
      <cascell><cellPair><inputCell><expression value="1"/></inputCell>
      <outputCell><expression value="1"/></outputCell></cellPair></cascell>
      <cascell><cellPair><useAsText/><inputCell><expression value="2"/></inputCell></cellPair></cascell>
    </construction></geogebra>"#;
    let mut wb = Workbench::default();
    wb.load_bytes(xml, "comment.xml").unwrap();
    assert!(wb.submit_cas(2, "$2 + 1"));
    assert!(wb.submit_cas(3, "$2$ + 1"));
    assert!(wb.submit_cas(0, "4"));
    assert_eq!(outputs(&wb), ["4", "", "?", "?"]);
}

#[test]
fn native_output_is_restored_without_recomputing_its_input() {
    let xml = br#"<geogebra><construction>
      <cascell><cellPair><inputCell><expression value="5!"/></inputCell>
      <outputCell><expression value="120" native="true"/></outputCell></cellPair></cascell>
      <cascell><cellPair><inputCell><expression value="2"/></inputCell>
      <outputCell><expression value="7" native="true"/></outputCell></cellPair></cascell>
    </construction></geogebra>"#;
    let mut wb = Workbench::default();
    wb.load_bytes(xml, "native.xml").unwrap();
    assert_eq!(outputs(&wb), ["120", "7"]);
    assert!(wb.submit_cas(2, "$1 + $2"));
    assert_eq!(outputs(&wb), ["120", "7", "127"]);
}

#[test]
fn cas_visibility_survives_saving_with_a_populated_or_empty_panel() {
    for (visible, input) in [(false, Some("7")), (true, None)] {
        let mut wb = Workbench::default();
        if let Some(input) = input {
            assert!(wb.submit_cas(0, input));
        }
        let bytes = wb.export_ggb_with_cas_visibility(None, visible).unwrap();
        let mut reopened = Workbench::default();
        reopened.load_bytes(&bytes, "perspective.ggb").unwrap();
        assert_eq!(reopened.cas_visible_on_open(), visible);
        assert_eq!(reopened.cas_rows().len(), wb.cas_rows().len());
    }
}

#[test]
fn cas_and_construction_changes_share_undo_order() {
    let mut wb = Workbench::default();
    assert!(wb.submit_cas(0, "2"));
    wb.run_line("A = (0, 0)").unwrap();
    assert!(wb.submit_cas(0, "7"));
    assert!(wb.delete_cas(0));
    assert!(wb.undo());
    assert_eq!(outputs(&wb), ["7"]);
    assert!(wb.undo());
    assert_eq!(outputs(&wb), ["2"]);
    assert!(wb.construction().id("A").is_some());
    assert!(wb.undo());
    assert!(wb.construction().id("A").is_none());
    assert_eq!(outputs(&wb), ["2"]);
    assert!(wb.redo());
    assert!(wb.redo());
    assert_eq!(outputs(&wb), ["7"]);
}
