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
fn a_failed_rollback_keeps_the_change_without_pretending_it_can_be_undone() {
    let mut w = Workbench::from_script("A = (0, 0)\n").unwrap();
    w.run_line("C = (5, 5)").unwrap();
    w.run_line("D = (6, 6)").unwrap();
    assert!(w.undo() && w.can_redo());
    // Every rebuild fails from now on, as a real one would on each replay.
    w.fail_rebuilds = Some("test".into());
    let lines = ["B = (1, 1)".to_owned(), "f = Line(B, Nope)".to_owned()];
    assert!(w.run_lines(&lines).is_err());
    assert_eq!(
        w.notice.as_deref(),
        Some("Nie można cofnąć ostatniej zmiany: test")
    );
    assert_eq!(labels(&w), ["A", "C", "B"]);
    assert!(!w.can_redo(), "the redo steps no longer follow");
    // The log holds only the line that succeeded and matches the
    // construction.
    w.fail_rebuilds = None;
    let replayed = w.log.rebuild().unwrap().0;
    let replayed: Vec<&str> = replayed.ids().map(|id| replayed.label(id)).collect();
    assert_eq!(replayed, ["A", "C", "B"]);
    // The kept change is not an undo step; the step before it still is.
    assert!(w.can_undo());
    assert!(w.undo());
    assert_eq!(labels(&w), ["A"], "undo went to the state before C");
    assert!(!w.can_undo());

    // Normally the rollback succeeds and nothing stays.
    let mut w = Workbench::from_script("A = (0, 0)\n").unwrap();
    assert!(w.run_lines(&lines).is_err());
    assert_eq!(labels(&w), ["A"]);
    assert!(w.notice.is_none() && !w.undo());
}

#[test]
fn a_cancelled_drag_that_cannot_be_rolled_back_stays_but_is_not_an_undo_step() {
    let mut w = Workbench::from_script("A = (0, 0)\n").unwrap();
    let a = w.construction().id("A").unwrap();
    w.begin_gesture();
    w.move_point(a, 2.0, 1.0);
    w.fail_rebuilds = Some("test".into());
    w.cancel_gesture();
    assert_eq!(
        w.notice.as_deref(),
        Some("Nie można cofnąć ostatniej zmiany: test")
    );
    assert!(!w.can_undo());
    let a = w.construction().id("A").unwrap();
    assert_eq!(
        w.construction().value(a),
        geo_kernel::kernel::Value::Point { x: 2.0, y: 1.0 }
    );
    w.fail_rebuilds = None;
    let (replayed, _) = w.log.rebuild().unwrap();
    assert_eq!(
        replayed.value(replayed.id("A").unwrap()),
        geo_kernel::kernel::Value::Point { x: 2.0, y: 1.0 },
        "the log has the move"
    );
}

#[test]
fn every_kind_of_the_kernel_is_known_listed_and_described() {
    // One object of each kind the kernel has. `Kind` is `#[non_exhaustive]`,
    // so this list is kept by hand: a kind added to the kernel goes here once
    // the window shows it, and until then its objects keep their rows with
    // `describe::NOT_DRAWN_YET` (the drawing and picking skip them).
    let mut wb = Workbench::from_script(
        "A = (0, 0)\nB = (4, 0)\nC = (1, 3)\nf = Line(A, B)\ns = Segment(A, C)\n\
         c = Circle(A, B)\nn = 2\nv = (1, 2)\nr = Ray(A, C)\nt = AreCollinear(A, B, C)\n\
         q = Polygon(A, B, C)\np = PolyLine(A, B, C)\nm = 3\ne = Ellipse(A, B, m)\n\
         g(x) = x^2\n",
    )
    .unwrap();
    let c = wb.construction();
    let mut kinds: Vec<Kind> = c.ids().map(|id| c.kind(id)).collect();
    kinds.dedup();
    for kind in [
        Kind::Point,
        Kind::Line,
        Kind::Segment,
        Kind::Circle,
        Kind::Number,
        Kind::Vector,
        Kind::Ray,
        Kind::Boolean,
        Kind::Polygon,
        Kind::PolyLine,
        Kind::Conic,
        Kind::Function,
    ] {
        assert!(kinds.contains(&kind), "no {kind:?} in the sample");
    }
    for id in c.ids() {
        assert!(describe::is_known(&c.value(id)), "{}", c.label(id));
    }
    let n = c.len();
    let rows = wb.rows();
    assert_eq!(rows.len(), n, "every object has a row");
    assert!(rows
        .iter()
        .all(|r| r.value.as_deref() != Some(describe::NOT_DRAWN_YET)));
}

/// `GeoPoint.movePoint` without an end position: free points translated
/// together, each coordinate cleaned at the fifth decimal, all in one undo
/// step of the drag.
#[test]
fn translating_points_in_a_drag_is_one_undo_step() {
    let mut w = Workbench::from_script("A = (0.1, 1)\nB = (3, 2)\nf = Line(A, B)\n").unwrap();
    let (a, b) = (
        w.construction().id("A").unwrap(),
        w.construction().id("B").unwrap(),
    );
    w.begin_gesture();
    assert!(w.translate(&[a, b], (0.2, 0.0)));
    assert!(w.translate(&[a, b], (0.0, -0.5)));
    w.end_gesture();
    // 0.1 + 0.2 is 0.30000000000000004 before the cleaning.
    assert_eq!(w.construction().value(a), Value::Point { x: 0.3, y: 0.5 });
    assert_eq!(w.construction().value(b), Value::Point { x: 3.2, y: 1.5 });
    assert!(w.undo());
    let a = w.construction().id("A").unwrap();
    assert_eq!(w.construction().value(a), Value::Point { x: 0.1, y: 1.0 });
    assert!(!w.can_undo(), "one step for the whole drag");
}

/// A label offset is no undo step and survives undo; it goes with its
/// object, so a later object of the same label starts in place.
#[test]
fn label_offsets_stay_out_of_undo_and_go_with_their_object() {
    let mut w = Workbench::from_script("A = (0, 0)\nB = (1, 1)\n").unwrap();
    let b = w.construction().id("B").unwrap();
    w.set_label_offset(b, (40.0, 30.0));
    assert!(!w.can_undo());
    assert!(w.move_point(w.construction().id("A").unwrap(), 2.0, 0.0));
    assert!(w.undo());
    let b = w.construction().id("B").unwrap();
    assert_eq!(w.label_offset(b), Some((40.0, 30.0)));
    assert!(w.delete(b));
    w.run_line("B = (5, 5)").unwrap();
    let b = w.construction().id("B").unwrap();
    assert_eq!(w.label_offset(b), None);
}

fn lines(src: &[&str]) -> Vec<String> {
    src.iter().map(|l| l.to_string()).collect()
}

#[test]
fn a_tool_use_with_its_new_points_is_one_undo_step() {
    // Spec F1: the points a tool places are no steps of their own; the
    // completed use is one step (`storeUndoInfoAndStateForModeStarting`).
    let mut w = Workbench::from_script("Z = (9, 9)\n").unwrap();
    w.run_tool_lines(&lines(&["A = (1, 1)"])).unwrap();
    assert!(w.tool_use_pending());
    assert_eq!(w.undo_depth(), (0, 0), "the first click is no step");
    w.run_tool_lines(&lines(&["B = (4, 3)"])).unwrap();
    w.run_tool_lines(&lines(&["f = Line(A, B)"])).unwrap();
    w.end_tool_use();
    assert!(!w.tool_use_pending());
    assert_eq!(w.undo_depth(), (1, 0));
    assert!(w.undo());
    assert_eq!(labels(&w), ["Z"]);
    assert!(w.redo());
    assert_eq!(labels(&w), ["Z", "A", "B", "f"]);
}

#[test]
fn a_cancelled_tool_use_takes_its_points_away_and_is_no_step() {
    // Spec E5: Escape or a tool switch removes the pending points.
    let mut w = Workbench::from_script("Z = (9, 9)\n").unwrap();
    w.run_line("Y = (8, 8)").unwrap();
    assert!(w.undo());
    assert_eq!(w.undo_depth(), (0, 1));
    w.run_tool_lines(&lines(&["A = (1, 1)"])).unwrap();
    assert!(w.cancel_tool_use());
    assert_eq!(labels(&w), ["Z"]);
    assert_eq!(w.undo_depth(), (0, 1), "no step, and the redo step stays");
    assert!(!w.cancel_tool_use(), "nothing more to cancel");
}

#[test]
fn undo_in_the_middle_of_a_tool_use_only_takes_the_use_back() {
    // Spec F4: `Kernel.undo` restores the state from before the tool use
    // and undoes no step.
    let mut w = Workbench::from_script("Z = (9, 9)\n").unwrap();
    w.run_line("Y = (8, 8)").unwrap();
    w.run_tool_lines(&lines(&["A = (1, 1)"])).unwrap();
    w.run_tool_lines(&lines(&["B = (2, 1)"])).unwrap();
    assert!(w.can_undo());
    assert!(w.undo());
    assert_eq!(labels(&w), ["Z", "Y"]);
    assert_eq!(w.undo_depth(), (1, 0));
    assert!(w.undo());
    assert_eq!(labels(&w), ["Z"]);
}

#[test]
fn a_failed_tool_step_keeps_the_points_of_the_earlier_picks_pending() {
    let mut w = Workbench::from_script("Z = (9, 9)\n").unwrap();
    w.run_tool_lines(&lines(&["A = (1, 1)"])).unwrap();
    assert!(w
        .run_tool_lines(&lines(&["B = (2, 1)", "f = Line(A, Nope)"]))
        .is_err());
    assert_eq!(labels(&w), ["Z", "A"]);
    assert!(w.tool_use_pending());
    assert!(w.cancel_tool_use());
    assert_eq!(labels(&w), ["Z"]);
}

#[test]
fn other_changes_take_an_unfinished_tool_use_back() {
    // A typed line, a deletion, a visibility toggle or a drag in the middle
    // of a tool use: the use does not finish, so its points go (the rule for
    // what the reference leaves open), and the change is the only step.
    for change in 0..4 {
        let mut w = Workbench::from_script("Z = (9, 9)\n").unwrap();
        w.run_tool_lines(&lines(&["A = (1, 1)"])).unwrap();
        let z = w.construction().id("Z").unwrap();
        match change {
            0 => w.run_line("Y = (8, 8)").unwrap(),
            1 => assert!(w.delete(z)),
            2 => w.toggle_visible(z),
            _ => {
                w.begin_gesture();
                assert!(w.move_point(z, 7.0, 7.0));
                w.end_gesture();
            }
        }
        assert!(!w.tool_use_pending(), "change {change}");
        assert!(w.construction().id("A").is_none(), "change {change}");
        assert_eq!(w.undo_depth(), (1, 0), "change {change}");
        assert!(w.undo(), "change {change}");
        assert!(!w.undo(), "change {change}");
        assert_eq!(labels(&w), ["Z"], "change {change}");
        assert_eq!(
            w.construction().value(z),
            Value::Point { x: 9.0, y: 9.0 },
            "change {change}"
        );
    }
}

#[test]
fn a_line_that_fails_mid_use_leaves_the_use_open() {
    let mut w = Workbench::from_script("Z = (9, 9)\n").unwrap();
    w.run_tool_lines(&lines(&["A = (1, 1)"])).unwrap();
    assert!(w.run_line("Nonsense((").is_err());
    assert!(w.tool_use_pending());
    assert_eq!(labels(&w), ["Z", "A"]);
    assert_eq!(w.undo_depth(), (0, 0));
    // The use goes on and is still one step when it finishes.
    w.run_tool_lines(&lines(&["B = (2, 2)", "f = Segment(A, B)"]))
        .unwrap();
    w.end_tool_use();
    assert_eq!(w.undo_depth(), (1, 0));
    assert!(w.undo());
    assert_eq!(labels(&w), ["Z"]);
}

#[test]
fn a_line_typed_mid_use_takes_the_labels_of_the_points_it_removed() {
    let mut w = Workbench::from_script("Z = (9, 9)\n").unwrap();
    w.run_tool_lines(&lines(&["A = (1, 1)"])).unwrap();
    w.run_line("(3, 3)").unwrap();
    assert_eq!(labels(&w), ["Z", "A"]);
    let a = w.construction().id("A").unwrap();
    assert_eq!(w.construction().value(a), Value::Point { x: 3.0, y: 3.0 });
    assert_eq!(w.undo_depth(), (1, 0));
}

#[test]
fn deleting_a_point_of_the_open_use_takes_the_use_back() {
    let mut w = Workbench::from_script("Z = (9, 9)\n").unwrap();
    w.run_tool_lines(&lines(&["A = (1, 1)"])).unwrap();
    let a = w.construction().id("A").unwrap();
    w.delete(a);
    assert!(!w.tool_use_pending());
    assert_eq!(labels(&w), ["Z"]);
    assert_eq!(w.undo_depth(), (0, 0));
}

#[test]
fn a_point_put_where_the_pointer_was_belongs_to_the_tool_use() {
    let mut w = Workbench::from_script("Z = (9, 9)\n").unwrap();
    w.run_tool_lines(&lines(&["A = (1, 1)"])).unwrap();
    let a = w.construction().id("A").unwrap();
    assert!(w.move_pending_point(a, 2.0, 2.0));
    assert_eq!(w.undo_depth(), (0, 0));
    w.end_tool_use();
    assert_eq!(w.undo_depth(), (1, 0));
    assert!(w.undo());
    assert_eq!(labels(&w), ["Z"]);
}

#[test]
fn every_deletion_of_one_gesture_is_one_undo_step() {
    // The eraser's stroke (spec E11) deletes several objects as one step
    // (`ModeDeleteLocus.storeUndoAfterDrag`).
    let mut w = Workbench::from_script("A = (1, 1)\nB = (2, 2)\nC = (3, 3)\n").unwrap();
    w.begin_gesture();
    let a = w.construction().id("A").unwrap();
    assert!(w.delete(a));
    let b = w.construction().id("B").unwrap();
    assert!(w.delete(b));
    w.end_gesture();
    assert_eq!(labels(&w), ["C"]);
    assert_eq!(w.undo_depth(), (1, 0));
    assert!(w.undo());
    assert_eq!(labels(&w), ["A", "B", "C"]);
}
const BOUNDARY_GROWTH: &str = "A = (0, 0)\nB = (2, 0)\nC = (2, 2)\nD = (0, 2)\n\
    p = Polygon(A, B, C, D)\nG = (-1, 3)\nH = (3, 3)\ng = Line(G, H)\nR = Intersect(g, p)";

#[test]
fn a_boundary_output_born_during_a_drag_survives_replay() {
    let mut w = Workbench::from_script(BOUNDARY_GROWTH).unwrap();
    let point = w.construction().id("G").unwrap();
    assert!(w.construction().id("R_{2}").is_none());
    w.begin_gesture();
    assert!(w.move_point(point, -1.0, 1.0));
    let born = w.construction().id("R_{2}").unwrap();
    assert!(matches!(w.construction().value(born), Value::Point { .. }));
    assert!(w.move_point(point, -1.0, 3.0));
    w.end_gesture();
    assert_eq!(w.construction().value(born), Value::Undefined);

    assert!(w.undo());
    assert!(w.construction().id("R_{2}").is_none());
    assert!(w.redo());
    assert_eq!(
        w.construction()
            .value(w.construction().id("R_{2}").unwrap()),
        Value::Undefined
    );

    let mut saved = Workbench::default();
    saved
        .load_bytes(w.snapshot_text().as_bytes(), "construction")
        .unwrap();
    let born = saved.construction().id("R_{2}").unwrap();
    assert_eq!(saved.construction().value(born), Value::Undefined);
    assert_eq!(saved.definition("R_{2}"), Some("Intersect(g, p)"));
    assert_eq!(
        saved
            .rows()
            .iter()
            .find(|row| row.id == born)
            .unwrap()
            .siblings,
        ["R_{1}"]
    );
    saved.run_line("S = Midpoint(R_{1}, R_{2})").unwrap();
    assert!(saved.delete(born));
    for label in ["R_{1}", "R_{2}", "S"] {
        assert!(saved.construction().id(label).is_none());
    }
    assert!(saved.undo());
    assert!(saved.construction().id("R_{2}").is_some());
}

#[test]
fn a_boundary_output_grown_by_a_slider_keeps_its_label_after_deletion() {
    let src = "A = (0, 0)\nB = (2, 0)\nC = (2, 2)\nD = (0, 2)\np = Polygon(A, B, C, D)\n\
        n = 3\ng: y = n\nR_{1} = (8, 9)\nR = Intersect(g, p)";
    let mut w = Workbench::from_script(src).unwrap();
    let number = w.construction().id("n").unwrap();
    assert!(w.construction().id("R_{2}").is_some());
    assert!(w.set_number(number, 1.0));
    assert!(w.set_number(number, 3.0));
    assert!(w.construction().id("R_{3}").is_some());

    assert!(w.delete(w.construction().id("R_{1}").unwrap()));
    assert!(w.construction().id("R_{1}").is_none());
    for label in ["R_{2}", "R_{3}"] {
        let id = w.construction().id(label).unwrap();
        assert_eq!(w.construction().value(id), Value::Undefined);
    }
    let text = w.snapshot_text();
    let mut restored = Workbench::default();
    assert!(restored.restore_text(&text).unwrap().is_empty());
    assert_eq!(restored.snapshot_text(), text);
    let mut w = Workbench::from_script(&text).unwrap();
    let number = w.construction().id("n").unwrap();
    assert!(w.set_number(number, 1.0));
    assert!(w.construction().id("R_{1}").is_none());
    let id = w.construction().id("R_{3}").unwrap();
    assert!(matches!(w.construction().value(id), Value::Point { .. }));
}

#[test]
fn script_moves_can_reveal_boundary_outputs_used_by_later_lines() {
    let script = format!("{BOUNDARY_GROWTH}\nSetCoords(G, -1, 1)\nS = Midpoint(R_{{1}}, R_{{2}})");
    let w = Workbench::from_script(&script).unwrap();
    let id = w.construction().id("S").unwrap();
    assert!(matches!(w.construction().value(id), Value::Point { .. }));
}
