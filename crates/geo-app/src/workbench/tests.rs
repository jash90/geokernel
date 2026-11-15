use super::*;

fn triangle() -> Workbench {
    Workbench::from_script(include_str!("../../../../examples/triangle.geo")).unwrap()
}

fn labels(w: &Workbench) -> Vec<String> {
    let c = w.construction();
    c.ids().map(|id| c.label(id).to_owned()).collect()
}

#[test]
fn run_line_adds_an_object_and_records_history() {
    let mut w = triangle();
    let n = w.construction().len();
    let v = w.version();
    w.run_line("Z = Midpoint(A, B)").unwrap();
    assert_eq!(w.construction().len(), n + 1);
    assert!(w.version() > v);
    assert_eq!(w.history.len(), 1);
}

#[test]
fn both_label_forms_and_bare_definitions_work() {
    let mut w = Workbench::default();
    w.run_line("A = (0, 0)").unwrap();
    w.run_line("B: (4, 0)").unwrap();
    w.run_line("f: Line(A, B)").unwrap();
    w.run_line("Midpoint(A, B)").unwrap();
    w.run_line("(1, 2)").unwrap();
    w.run_line("Circle(A, B)").unwrap();
    assert_eq!(labels(&w), ["A", "B", "f", "C", "D", "c"]);
    assert_eq!(w.rows()[2].head, "f = Line(A, B)");
}

#[test]
fn failed_line_changes_nothing() {
    let mut w = triangle();
    let n = w.construction().len();
    let v = w.version();
    let err = w.run_line("Z = Midpoint(A, Nope)").unwrap_err();
    assert!(!err.is_empty());
    assert_eq!(w.construction().len(), n);
    assert_eq!(w.version(), v);
    assert_eq!(w.history.len(), 0);
    assert!(!w.can_undo());
    // A duplicate name is also an error.
    assert!(w.run_line("A = (9, 9)").is_err());
}

#[test]
fn a_group_with_a_failing_line_is_rolled_back_whole() {
    let mut w = triangle();
    let n = w.construction().len();
    let lines = vec![
        "Z1 = Midpoint(A, B)".to_owned(),
        "Z2 = Line(A, Nope)".to_owned(),
    ];
    assert!(w.run_lines(&lines).is_err());
    assert_eq!(w.construction().len(), n);
    assert!(w.construction().id("Z1").is_none());
    assert!(!w.can_undo());
    assert_eq!(w.rows().len(), n);
}

#[test]
fn comment_line_is_ok_and_not_a_change() {
    let mut w = triangle();
    let v = w.version();
    w.run_line("# nothing").unwrap();
    assert_eq!(w.version(), v);
    assert!(!w.can_undo());
}

#[test]
fn moving_a_point_updates_version_and_the_logged_position() {
    let mut w = triangle();
    let a = w.construction().id("A").unwrap();
    let v = w.version();
    assert!(w.move_point(a, 0.5, 0.5));
    assert!(w.version() > v);
    let v = w.version();
    assert!(!w.move_point(a, 0.5, 0.5));
    assert_eq!(w.version(), v);
    // Undo of another change and rebuild keep the moved position.
    w.run_line("Z = Midpoint(A, B)").unwrap();
    assert!(w.undo());
    let a = w.construction().id("A").unwrap();
    assert_eq!(w.construction().value(a), Value::Point { x: 0.5, y: 0.5 });
}

#[test]
fn a_whole_drag_is_one_undo_step() {
    let mut w = triangle();
    let a = w.construction().id("A").unwrap();
    w.begin_gesture();
    for i in 1..=10 {
        assert!(w.move_point(a, i as f64, 0.0));
    }
    w.end_gesture();
    assert!(w.undo());
    let a = w.construction().id("A").unwrap();
    assert_eq!(w.construction().value(a), Value::Point { x: 0.0, y: 0.0 });
    assert!(!w.can_undo(), "one step only");
    assert!(w.redo());
    let a = w.construction().id("A").unwrap();
    assert_eq!(w.construction().value(a), Value::Point { x: 10.0, y: 0.0 });
}

#[test]
fn a_drag_that_changes_nothing_records_nothing() {
    let mut w = triangle();
    let a = w.construction().id("A").unwrap();
    w.begin_gesture();
    assert!(!w.move_point(a, 0.0, 0.0));
    w.end_gesture();
    assert!(!w.can_undo());
}

#[test]
fn moving_a_point_without_dependents_still_counts_as_a_change() {
    let mut w = Workbench::default();
    w.run_line("P = (1, 1)").unwrap();
    let p = w.construction().id("P").unwrap();
    let v = w.version();
    assert!(w.move_point(p, 2.0, 2.0));
    assert!(w.version() > v);
}

#[test]
fn moving_a_dependent_object_is_rejected() {
    let mut w = triangle();
    let o = w.construction().id("O").unwrap();
    let v = w.version();
    assert!(!w.move_point(o, 1.0, 1.0));
    assert_eq!(w.version(), v);
    assert!(!w.can_undo());
}

#[test]
fn set_number_recomputes_dependents_and_undoes() {
    let mut w = triangle();
    let r = w.construction().id("r").unwrap();
    let m = w.construction().id("m").unwrap();
    let before = w.construction().value(m);
    assert!(w.set_number(r, 4.0));
    assert_ne!(w.construction().value(m), before);
    assert!(!w.set_number(r, 4.0));
    let d = w.construction().id("d").unwrap();
    assert!(!w.set_number(d, 1.0));
    assert!(w.undo());
    let m = w.construction().id("m").unwrap();
    assert_eq!(w.construction().value(m), before);
}

#[test]
fn delete_removes_dependents_and_can_be_undone() {
    let mut w = Workbench::from_script(
        "A = (0, 0)\nB = (2, 0)\nC = (0, 3)\nf = Line(A, B)\ng = Line(A, C)\nE = Intersect(f, g)\n",
    )
    .unwrap();
    let b = w.construction().id("B").unwrap();
    assert!(w.delete(b));
    // f depends on B, E on f: both go. g and C stay.
    assert_eq!(labels(&w), ["A", "C", "g"]);
    assert!(w.undo());
    assert_eq!(labels(&w), ["A", "B", "C", "f", "g", "E"]);
    assert!(w.redo());
    assert_eq!(labels(&w), ["A", "C", "g"]);
}

#[test]
fn undo_after_a_new_object_removes_it_and_keeps_the_selection() {
    let mut w = triangle();
    w.selected = w.construction().id("B");
    w.run_line("Z = Midpoint(A, B)").unwrap();
    assert!(w.undo());
    assert!(w.construction().id("Z").is_none());
    assert_eq!(w.selected, w.construction().id("B"));
    assert!(!w.undo());
    assert!(w.redo());
    assert!(w.construction().id("Z").is_some());
}

#[test]
fn visibility_is_per_label_and_survives_a_rebuild() {
    let mut w = triangle();
    let a = w.construction().id("A").unwrap();
    assert!(w.is_visible(a));
    w.toggle_visible(a);
    assert!(!w.is_visible(a));
    w.run_line("Z = Midpoint(A, B)").unwrap();
    w.undo();
    let a = w.construction().id("A").unwrap();
    assert!(!w.is_visible(a));
    w.toggle_visible(a);
    assert!(w.is_visible(a));
}

#[test]
fn rows_describe_free_and_dependent_objects() {
    let mut w = Workbench::from_script(
        "A = (-5.644, 4.44)\nB = (3.3, 3.34)\nf = Line(A, B)\nr = 3\nE = Midpoint(A, B)\n",
    )
    .unwrap();
    let rows = w.rows().to_vec();
    assert_eq!(rows[0].head, "A = (-5.64, 4.44)");
    assert!(rows[0].free && rows[0].value.is_none());
    assert_eq!(rows[2].head, "f = Line(A, B)");
    assert_eq!(rows[2].value.as_deref(), Some("y = -0.12x + 3.75"));
    assert_eq!(rows[3].free_number, Some(3.0));
    assert_eq!(rows[3].head, "r =");
    assert_eq!(rows[4].head, "E = Midpoint(A, B)");
    assert_eq!(rows[4].value.as_deref(), Some("(-1.17, 3.89)"));
}

#[test]
fn rows_are_cached_until_something_changes() {
    let mut w = triangle();
    w.rows();
    w.rows();
    assert_eq!(w.rows_built(), 1);
    let a = w.construction().id("A").unwrap();
    w.move_point(a, 1.0, 1.0);
    w.rows();
    w.rows();
    assert_eq!(w.rows_built(), 2);
    w.move_point(a, 1.0, 1.0);
    w.rows();
    assert_eq!(w.rows_built(), 2);
    w.run_line("Z = Midpoint(A, B)").unwrap();
    assert_eq!(w.rows().len(), w.construction().len());
    assert_eq!(w.rows_built(), 3);
}

#[test]
fn undefined_objects_are_listed_as_undefined() {
    let mut w = triangle();
    let r = w.construction().id("r").unwrap();
    assert!(w.set_number(r, 100.0));
    let rows = w.rows().to_vec();
    let p = rows.iter().find(|r| r.label == "P").unwrap();
    assert_eq!(p.value.as_deref(), Some("niezdefiniowany"));
}

#[test]
fn load_script_resets_state_and_a_bad_script_changes_nothing() {
    let mut w = triangle();
    w.selected = w.construction().id("A");
    w.run_line("Z = Midpoint(A, B)").unwrap();
    let n = w.construction().len();
    assert!(w.load_script("A = (0, 0)\nB = Line(A, Q)").is_err());
    assert_eq!(w.construction().len(), n);
    w.load_script("A = (1, 1)").unwrap();
    assert_eq!(w.selected, None);
    assert!(!w.can_undo());
    assert_eq!(w.rows().len(), 1);
}

#[test]
fn a_reused_label_is_not_born_hidden() {
    let mut w = Workbench::from_script("A = (0, 0)\nB = (2, 0)\n").unwrap();
    let b = w.construction().id("B").unwrap();
    w.toggle_visible(b);
    assert!(w.delete(b));
    w.run_line("B = (5, 5)").unwrap();
    let b = w.construction().id("B").unwrap();
    assert!(w.is_visible(b));
    // Same when the object disappears through undo.
    w.toggle_visible(b);
    assert!(w.undo());
    assert!(w.undo());
    w.run_line("B = (7, 7)").unwrap();
    let b = w.construction().id("B").unwrap();
    assert!(w.is_visible(b));
}

#[test]
fn stale_ids_are_ignored_instead_of_panicking() {
    let mut w = Workbench::from_script("A = (0, 0)\nB = (1, 1)\n").unwrap();
    let b = w.construction().id("B").unwrap();
    assert!(w.delete(b));
    assert!(!w.move_point(b, 3.0, 3.0));
    assert!(!w.set_number(b, 3.0));
    assert!(!w.delete(b));
}

#[test]
fn cancelling_a_gesture_restores_the_state_and_leaves_no_undo_step() {
    let mut w = Workbench::from_script("A = (0, 0)\n").unwrap();
    let a = w.construction().id("A").unwrap();
    w.begin_gesture();
    assert!(w.move_point(a, 4.0, 4.0));
    w.cancel_gesture();
    let a = w.construction().id("A").unwrap();
    assert_eq!(
        w.construction().value(a),
        geo_kernel::kernel::Value::Point { x: 0.0, y: 0.0 }
    );
    assert!(!w.can_undo());
}

#[test]
fn a_number_set_to_any_value_keeps_the_tangent_labels_through_undo_and_redo() {
    let script =
        "A = (0, 0)\nr = 2\nk = Circle(A, r)\nP = (5, 0)\nt = Tangent(P, k)\nQ = (-4, 3)\n";
    let mut w = Workbench::from_script(script).unwrap();
    let r = w.construction().id("r").unwrap();
    assert!(w.set_number(r, 6.0));
    let state = labels(&w);
    assert_eq!(state, ["A", "r", "k", "P", "t_{1}", "t_{2}", "Q"]);
    let q = w.construction().id("Q").unwrap();
    assert!(w.delete(q));
    assert!(w.undo());
    assert!(w.undo());
    assert!(w.redo());
    assert!(w.redo());
    assert_eq!(labels(&w), ["A", "r", "k", "P", "t_{1}", "t_{2}"]);
    assert!(w.notice.is_none());
}

#[test]
fn the_sides_of_a_polygon_name_the_polygon() {
    let mut w =
        Workbench::from_script("A = (0, 0)\nB = (4, 0)\nC = (1, 3)\nt1 = Polygon(A, B, C)\n")
            .unwrap();
    let rows = w.rows().to_vec();
    let heads: Vec<&str> = rows.iter().map(|r| r.head.as_str()).collect();
    assert!(heads.contains(&"t1 = Polygon(A, B, C)"), "{heads:?}");
    assert!(heads.contains(&"a = Segment(B, C, t1)"), "{heads:?}");
    let t1 = rows.iter().find(|r| r.label == "t1").unwrap();
    assert_eq!(t1.value.as_deref(), Some("6"));
}

#[test]
fn a_failed_rollback_is_told_and_kept_as_one_undo_step() {
    let mut w = Workbench::from_script("A = (0, 0)\n").unwrap();
    w.fail_next_rebuild = Some("test".into());
    let lines = ["B = (1, 1)".to_owned(), "f = Line(B, Nope)".to_owned()];
    assert!(w.run_lines(&lines).is_err());
    assert_eq!(
        w.notice.as_deref(),
        Some("Nie można odtworzyć konstrukcji: test")
    );
    // The objects made before the error stay, consistent with the log.
    assert_eq!(labels(&w), ["A", "B"]);
    let replayed = w.log.rebuild().unwrap().0;
    assert_eq!(replayed.len(), w.construction().len());
    // One undo takes the partial change back.
    assert!(w.undo());
    assert_eq!(labels(&w), ["A"]);
    assert!(w.notice.is_none());

    // Normally the rollback succeeds and nothing stays.
    let mut w = Workbench::from_script("A = (0, 0)\n").unwrap();
    assert!(w.run_lines(&lines).is_err());
    assert_eq!(labels(&w), ["A"]);
    assert!(w.notice.is_none() && !w.undo());
}

#[test]
fn a_cancelled_drag_that_cannot_be_rolled_back_stays_undoable() {
    let mut w = Workbench::from_script("A = (0, 0)\n").unwrap();
    let a = w.construction().id("A").unwrap();
    w.begin_gesture();
    w.move_point(a, 2.0, 1.0);
    w.fail_next_rebuild = Some("test".into());
    w.cancel_gesture();
    assert!(w.notice.is_some());
    assert!(w.can_undo(), "the drag is still an undo step");
    assert!(w.undo());
    let a = w.construction().id("A").unwrap();
    assert_eq!(
        w.construction().value(a),
        geo_kernel::kernel::Value::Point { x: 0.0, y: 0.0 }
    );
}
