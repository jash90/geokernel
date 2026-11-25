use geo_kernel::spreadsheet::Address;
use geo_kernel::{script, Construction, Value};

fn assert_number(c: &Construction, label: &str, expected: f64) {
    let id = c.id(label).unwrap();
    assert!(
        c.value(id) == Value::Number(expected)
            || expected.is_nan() && c.value(id) == Value::Undefined,
        "{label}: expected {expected}, got {:?}",
        c.value(id)
    );
}

#[test]
fn cell_range_and_absolute_references() {
    let c = script::load("empty = CellRange(A1, A1)\nA1 = 2\nA2 = 3\nB1 = 5\nr = A1:B2\ns = CellRange(B2, A1)\nt = CellRange(C1, C2, \"numeric\")\nv = Sum($A$1:B2)").unwrap();
    assert_eq!(c.elements(c.id("empty").unwrap()).unwrap().len(), 1);
    assert_eq!(c.elements(c.id("r").unwrap()).unwrap().len(), 3);
    assert_eq!(c.elements(c.id("s").unwrap()).unwrap().len(), 3);
    assert_eq!(c.elements(c.id("t").unwrap()).unwrap().len(), 0);
    assert_number(&c, "v", 10.0);
}

#[test]
fn row_and_column() {
    let c = script::load("NTO9999 = 1\nNTP9999 = 1\nNTO10000 = 1\nr = Row(NTO9999)\ns = Row(NTP9999)\nt = Row(NTO10000)\nu = Row((1,1))\nv = Column(AA1)").unwrap();
    for label in ["s", "t", "u"] {
        assert_number(&c, label, f64::NAN);
    }
    assert_number(&c, "r", 9999.0);
    assert_number(&c, "v", 27.0);
}

#[test]
fn column_name() {
    let c = script::load("v = ColumnName(A1)\nw = ColumnName((1,1))").unwrap();
    assert_eq!(c.text(c.id("v").unwrap()), Some("A"));
    assert_eq!(c.text(c.id("w").unwrap()), None);
}

#[test]
fn cell_lookup() {
    let mut c = script::load("A1 = 3\nB1 = 4\nn = 1\nv = Cell(n, 1)\nw = Cell(42, 4)").unwrap();
    assert_number(&c, "v", 3.0);
    assert_number(&c, "w", f64::NAN);
    script::exec(&mut c, "n = 2").unwrap();
    assert_number(&c, "v", 4.0);
    script::exec(&mut c, "B1 = 7").unwrap();
    assert_number(&c, "v", 7.0);
}

#[test]
fn fill_row() {
    let c = script::load("FillRow(4, {1, 2, 3, 4, 5})").unwrap();
    assert_number(&c, "A4", 1.0);
    assert_number(&c, "E4", 5.0);
}

#[test]
fn fill_column() {
    let c = script::load("FillColumn(13, {1, 2, 3, 4, 5})").unwrap();
    assert_number(&c, "M1", 1.0);
    assert_number(&c, "M5", 5.0);
}

#[test]
fn fill_cells() {
    let c = script::load("FillCells(A1, {1,2,3,4,5})\nFillCells(A1, {{1,2},{3,4}})\nFillCells(A3:C4, {5,6})\nFillCells(F1:F2, A1+1)").unwrap();
    assert_number(&c, "E1", 5.0);
    assert_number(&c, "B2", 4.0);
    assert_number(&c, "A4", 6.0);
    assert_number(&c, "F1", 2.0);
    assert_number(&c, "F2", 4.0);
}

#[test]
fn rejects_oversized_ranges_and_fills_without_creating_cells() {
    let mut c = Construction::new();
    for line in [
        "v = A1:NTO9999",
        "v = CellRange(A1, NTO9999)",
        "FillCells(A1:NTO9999, 1)",
        "FillCells(A1:NTO9999, {1,2})",
        "$spreadsheetDrag(A1:A1025, 1)",
        "FillCells(NTO9999, {1,2})",
        "FillRow(1, Sequence(n,n,1,5000))",
        "FillColumn(1, Sequence(n,n,1,5000))",
        "FillCells(A1, Sequence(n,n,1,5000))",
    ] {
        let error = script::exec(&mut c, line).unwrap_err();
        assert!(error.contains("spreadsheet limit"), "{line}: {error}");
        assert!(c.is_empty(), "{line}");
    }
}

#[test]
fn a_fill_checks_a_cycle_across_all_destinations_before_replacing_them() {
    let mut c = script::load("B1 = 1\nB2 = 2\nB3 = 3\nB4 = 4").unwrap();
    let error = script::exec(&mut c, "FillCells(B2:B3, B1+B3)").unwrap_err();
    assert!(error.contains("dependency cycle"), "{error}");
    assert_number(&c, "B2", 2.0);
    assert_number(&c, "B3", 3.0);
    assert_eq!(c.len(), 4);
}

#[test]
fn repeated_ranges_and_editor_inputs_have_a_shared_bound() {
    use geo_kernel::spreadsheet::{Address, MAX_INPUT_BYTES};
    let mut c = Construction::new();
    let error = script::exec(&mut c, "h = Sum(A1:A600) + Sum(A1:A600)").unwrap_err();
    assert!(error.contains("spreadsheet limit"), "{error}");
    assert!(c.is_empty());
    let input = "x".repeat(MAX_INPUT_BYTES + 1);
    assert!(c
        .edit_spreadsheet_cell(Address::parse("A1").unwrap(), &input)
        .is_err());
    assert!(c.is_empty());
}

#[test]
fn lookup_limits_the_number_of_dynamic_dependencies() {
    let mut c = script::load("FillCells(A1:A1024, 1)\nB1 = 2\nn = 1").unwrap();
    let error = script::exec(&mut c, "v = Cell(n, 1)").unwrap_err();
    assert!(error.contains("spreadsheet limit"), "{error}");
    assert!(c.id("v").is_none());
}

#[test]
fn a_fill_bounds_dependency_walks_and_copied_text() {
    use std::fmt::Write as _;

    let mut source = String::from("a0 = 1\n");
    for index in 1..=4_200 {
        writeln!(source, "a{index} = a{} + 1", index - 1).unwrap();
    }
    let mut c = script::load(&source).unwrap();
    let error = script::exec(&mut c, "FillCells(A1:B1, a4200 + 1)").unwrap_err();
    assert!(error.contains("spreadsheet limit"), "{error}");
    assert!(c.spreadsheet_cells().next().is_none());

    let text = "abc".repeat(50);
    let error = script::exec(&mut c, &format!("FillCells(A1:Y40, \"{text}\")")).unwrap_err();
    assert!(error.contains("spreadsheet limit"), "{error}");
    assert!(c.spreadsheet_cells().next().is_none());
}

#[test]
fn a_lookup_observes_the_initial_address_even_when_created_later() {
    let mut c = script::load("n = 4\nv = Cell(n + 1, 1)\nw = v + 1").unwrap();
    assert_number(&c, "v", f64::NAN);
    script::exec(&mut c, "E1 = 9").unwrap();
    assert_number(&c, "v", 9.0);
    assert_number(&c, "w", 10.0);
    script::exec(&mut c, "E1 = 12").unwrap();
    assert_number(&c, "w", 13.0);
}

#[test]
fn fill_refusing_a_slider_replacement_leaves_the_sheet_unchanged() {
    let mut c = script::load("B1 = Slider(0,10)").unwrap();
    assert!(script::exec(&mut c, "FillRow(1,{7,8})").is_err());
    assert!(c.id("A1").is_none());
    assert_number(&c, "B1", 0.0);
}

#[test]
fn range_and_dynamic_lookup_share_the_cell_budget() {
    let mut c = script::load("FillCells(A1:A1000, 1)\nn = 1").unwrap();
    let error = script::exec(&mut c, "v = Cell(n,1) + Sum(A1:A25)").unwrap_err();
    assert!(error.contains("spreadsheet limit"), "{error}");
    assert!(c.id("v").is_none());
}

#[test]
fn reserving_lookup_coordinates_charges_expanded_function_work() {
    use std::fmt::Write as _;

    let mut source = String::from("f0(x) = x\n");
    for i in 1..=13 {
        writeln!(source, "f{i}(x) = f{}(x) + f{}(x)", i - 1, i - 1).unwrap();
    }
    let mut c = script::load(&source).unwrap();
    let error = script::exec(&mut c, "v = Cell(f13(1) / 8192, 1)").unwrap_err();
    assert!(error.contains("spreadsheet limit"), "{error}");
    assert!(c.id("v").is_none());
    assert!(c.id("A1").is_none());
}

#[test]
fn a_range_keeps_its_budget_when_command_parsing_backtracks() {
    let c = script::load("FillCells(A1:A600, {1})\nv = Sum(A1:A600)\nn = Length(A1:A600)").unwrap();
    assert_number(&c, "v", 600.0);
    assert_number(&c, "n", 600.0);
}

#[test]
fn a_labelled_fill_that_cannot_keep_its_result_changes_no_cells() {
    let mut c = script::load("A1 = 3").unwrap();
    assert!(script::exec(&mut c, r#"r = FillRow(1,{"text",A1^2})"#).is_err());
    assert_number(&c, "A1", 3.0);
    assert!(c.id("B1").is_none());
    assert!(c.id("r").is_none());
}

#[test]
fn spreadsheet_watch_classification_spends_the_deletion_budget() {
    let c = script::load("A1 = 1\nB1 = 2\nn = 1\nv = Cell(n,1)\nw = Cell(n,1)+B1").unwrap();
    let b1 = c.id("B1").unwrap();
    let v = c.id("v").unwrap();
    let w = c.id("w").unwrap();
    let mut remaining = 100;
    assert!(c.spreadsheet_watch_only(v, b1, &mut remaining).unwrap());
    assert!(remaining < 100);
    assert!(!c.spreadsheet_watch_only(w, b1, &mut remaining).unwrap());
    assert!(c.spreadsheet_watch_only(v, b1, &mut 0).is_err());
}

#[test]
fn fill_counts_distinct_source_cells_across_shifted_formulas() {
    let mut c = Construction::new();
    let before = c.len();
    let error = script::exec(&mut c, "FillCells(A1:B1,Sum(C1:C600))").unwrap_err();
    assert_eq!(error, "spreadsheet limit exceeded");
    assert_eq!(c.len(), before);
    assert_eq!(c.spreadsheet_cells().count(), 0);

    script::exec(&mut c, "FillCells(B1:B2,Sum(C1:C600))").unwrap();
    assert_eq!(c.spreadsheet_display(Address::parse("B1").unwrap()), "0");
    assert_eq!(c.spreadsheet_display(Address::parse("B2").unwrap()), "0");
}
