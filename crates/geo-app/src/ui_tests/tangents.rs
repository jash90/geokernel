//! Tangents and other multi-output commands while their point is dragged.

use super::*;

const TANGENTS: &str =
    "A = (0, 0)\nr = 2\nk = Circle(A, r)\nP = (5, 0)\nt = Tangent(P, k)\nQ = (-4, 3)\n";

/// Drags P across the circle with the pointer, places a point with the tool,
/// then walks the undo stack all the way back and forward. Every rebuild must
/// bring the objects back under their own labels.
fn labels_survive_rebuilds(script: &str, from: Pos2, to: Pos2, expect: &[&str]) {
    let mut h = harness(script);
    let start_x = point_x(&h, "P");
    drag(&mut h, from, to);
    assert!((point_x(&h, "P") - start_x).abs() > 0.2, "the drag moved P");
    assert!(h.state().wb.construction().id("Q").is_some());
    h.state_mut().tool.set_tool(Tool::Point);
    click(&mut h, world(3.0, 3.0));
    let moved_x = point_x(&h, "P");
    let full = labels(&h);
    assert_eq!(&full[..expect.len()], expect);
    undo(&mut h);
    undo(&mut h);
    assert_eq!(labels(&h), expect);
    assert!(h.state().wb.notice.is_none());
    redo(&mut h);
    redo(&mut h);
    assert_eq!(labels(&h), full);
    assert!(h.state().wb.notice.is_none());
    assert!((point_x(&h, "P") - moved_x).abs() < 1e-9);
}

#[test]
fn two_tangents_keep_their_labels_when_the_point_is_dragged_around() {
    labels_survive_rebuilds(
        TANGENTS,
        world(5.0, 0.0),
        world(6.0, 1.0),
        &["A", "r", "k", "P", "t_{1}", "t_{2}", "Q"],
    );
}

#[test]
fn one_tangent_keeps_its_label_when_the_point_is_dragged_around_inside() {
    labels_survive_rebuilds(
        &TANGENTS.replace("(5, 0)", "(1, 0)"),
        world(1.0, 0.0),
        world(0.5, 0.5),
        &["A", "r", "k", "P", "t", "Q"],
    );
}

fn is_undefined(h: &Harness<'_, App>, label: &str) -> bool {
    let c = h.state().wb.construction();
    matches!(c.value(c.id(label).unwrap()), Value::Undefined)
}

fn p_at(h: &Harness<'_, App>) -> (f64, f64) {
    let c = h.state().wb.construction();
    let Value::Point { x, y } = c.value(c.id("P").unwrap()) else {
        panic!("P is not a point");
    };
    (x, y)
}

/// Everything the user can see of the construction.
fn snapshot(h: &Harness<'_, App>) -> Vec<(String, String)> {
    let c = h.state().wb.construction();
    c.ids()
        .map(|id| (c.label(id).to_owned(), format!("{:?}", c.value(id))))
        .collect()
}

#[test]
fn dragging_into_the_circle_undefines_both_tangents_and_they_come_back() {
    let all = ["A", "r", "k", "P", "t_{1}", "t_{2}", "Q"];
    let mut h = harness(TANGENTS);
    // A fast drag: the first pointer move is already far from the point.
    drag(&mut h, world(5.0, 0.0), world(1.0, 0.0));
    assert_eq!(labels(&h), all);
    let (x, y) = p_at(&h);
    assert!((x - 1.0).abs() < 1e-6 && y.abs() < 1e-6, "{x} {y}");
    assert!(is_undefined(&h, "t_{1}") && is_undefined(&h, "t_{2}"));
    let state = snapshot(&h);
    // An unrelated delete, then undo and redo, reproduce exactly that state.
    let q = h.state().wb.construction().id("Q").unwrap();
    assert!(h.state_mut().wb.delete(q));
    assert!(is_undefined(&h, "t_{1}"));
    undo(&mut h);
    assert_eq!(snapshot(&h), state);
    undo(&mut h);
    assert_eq!(p_at(&h), (5.0, 0.0));
    assert!(!is_undefined(&h, "t_{1}"));
    redo(&mut h);
    assert_eq!(snapshot(&h), state);
    assert!(h.state().wb.notice.is_none());
    // Dragging back out brings both tangents back under the same labels.
    drag(&mut h, world(1.0, 0.0), world(6.0, 1.0));
    assert_eq!(labels(&h), all);
    assert!(!is_undefined(&h, "t_{1}") && !is_undefined(&h, "t_{2}"));
}

#[test]
fn a_single_tangent_stays_one_object_when_the_point_moves_outside() {
    let script = TANGENTS.replace("(5, 0)", "(1, 0)");
    let all = ["A", "r", "k", "P", "t", "Q"];
    let mut h = harness(&script);
    assert!(is_undefined(&h, "t"));
    drag(&mut h, world(1.0, 0.0), world(5.0, 0.0));
    assert_eq!(labels(&h), all, "no new object appears");
    assert!(!is_undefined(&h, "t"));
    let state = snapshot(&h);
    let q = h.state().wb.construction().id("Q").unwrap();
    assert!(h.state_mut().wb.delete(q));
    undo(&mut h);
    assert_eq!(snapshot(&h), state);
    undo(&mut h);
    assert!(is_undefined(&h, "t"));
    redo(&mut h);
    assert_eq!(snapshot(&h), state);
    drag(&mut h, world(5.0, 0.0), world(1.0, 0.0));
    assert_eq!(labels(&h), all);
    assert!(is_undefined(&h, "t"));
    assert!(h.state().wb.notice.is_none());
}

#[test]
fn deleting_one_tangent_says_the_sibling_goes_too() {
    let mut h = harness(TANGENTS);
    // Row 4 is t_{1}, under A, r, k and P.
    click(&mut h, row_menu(3.0 * ROW + ROW_DEPENDENT, true));
    h.get_by_label_contains("razem z t_{2}").click();
    h.run();
    assert_eq!(labels(&h), ["A", "r", "k", "P", "Q"]);
}
