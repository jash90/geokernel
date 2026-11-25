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
