use super::*;

#[test]
fn typed_text_preserves_punctuation_and_does_not_require_a_label() {
    let mut wb = Workbench::from_script("P = (3, 4)").unwrap();
    wb.run_line(r#""P = " + P"#).unwrap();
    wb.run_line(r#""Heading: value #1""#).unwrap();
    wb.run_line(r#"alias: "x = y""#).unwrap();
    let c = wb.construction();
    let texts: Vec<_> = c.ids().filter_map(|id| c.text(id)).collect();
    assert_eq!(texts, ["P = (3, 4)", "Heading: value #1", "x = y"]);
    assert_eq!(c.text(c.id("alias").unwrap()), Some("x = y"));
}

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
fn bare_label_reads_leave_the_journal_version_and_undo_unchanged() {
    let mut w = Workbench::default();
    w.run_line("A = (1, 2)").unwrap();
    let text = w.snapshot_text();
    let version = w.version();
    w.run_line("A").unwrap();
    w.run_lines(&[" A # read".into()]).unwrap();
    assert_eq!(w.version(), version);
    assert_eq!(w.undo_depth(), (1, 0));
    assert_eq!(w.snapshot_text(), text);
    assert_eq!(w.log.rhs_of("A"), Some("(1, 2)"));

    let clean = Workbench::from_script("A = (1, 2)").unwrap();
    let clean_bytes = clean.export_ggb(None).unwrap();
    let expected_xml = geo_import::archive::extract_xml(&clean_bytes).unwrap();
    let mut restored = Workbench::default();
    assert!(restored
        .restore_text(&format!("{text}\nA\nA # labels: A\n"))
        .unwrap()
        .is_empty());
    for document in [&w, &restored] {
        let bytes = document.export_ggb(None).unwrap();
        let xml = geo_import::archive::extract_xml(&bytes).unwrap();
        assert_eq!(xml, expected_xml);
        assert!(xml.contains(r#"<coords x="1" y="2" z="1"/>"#));
    }

    assert!(w.undo());
    assert!(w.construction().id("A").is_none());
    assert!(w.redo());
    assert_eq!(w.snapshot_text(), text);
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
    assert!(w.run_line("A = A + (9, 9)").is_err());
}

#[test]
fn redefinition_keeps_the_object_and_survives_undo_and_replay() {
    let mut w = triangle();
    let a = w.construction().id("A").unwrap();
    let before = w.construction().value(a);
    w.run_line("A = (9, 9)").unwrap();
    assert_eq!(w.construction().id("A"), Some(a));
    assert_eq!(w.construction().value(a), Value::Point { x: 9.0, y: 9.0 });
    let text = w.snapshot_text();
    assert!(w.undo());
    assert_eq!(w.construction().value(a), before);
    assert!(w.redo());
    assert_eq!(w.construction().value(a), Value::Point { x: 9.0, y: 9.0 });
    let (restored, _) = crate::scriptlog::Log::load(&text).unwrap();
    assert_eq!(restored.value(a), Value::Point { x: 9.0, y: 9.0 });
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
fn a_view_change_refreshes_the_rows_of_view_dependent_objects() {
    let mut w = Workbench::from_script(
        "p = Corner(3)\ng = AxisStepX()\nh = AxisStepY()\ns = x(p) + g + h\n",
    )
    .unwrap();
    assert_eq!(row_value(&mut w, "g"), "1");
    assert_eq!(row_value(&mut w, "h"), "1");
    let v = w.version();
    let view = geo_kernel::svg::View {
        xmin: 100.0,
        ymin: 200.0,
        xmax: 116.0,
        ymax: 212.0,
    };
    let pan = crate::interact::ViewTransform::new(view, 800.0, 600.0);
    w.set_view(pan);
    assert!(w.version() > v);
    assert_eq!(row_value(&mut w, "p"), "(116.02, 212.02)");
    assert_eq!(row_value(&mut w, "s"), "118.02");

    let zoom = crate::interact::ViewTransform::new(view, 300.0, 240.0);
    w.set_view(zoom);
    assert_eq!(row_value(&mut w, "g"), "5");
    assert_eq!(row_value(&mut w, "h"), "2");
    assert_eq!(row_value(&mut w, "s"), "123.05");
    let v = w.version();
    w.set_view(zoom);
    assert_eq!(w.version(), v);

    w.run_line("extra = 1").unwrap();
    assert!(w.undo());
    // The rows can be read before the window publishes its view again.
    w.rows();
    w.set_view(zoom);
    assert_eq!(row_value(&mut w, "g"), "5");
    assert_eq!(row_value(&mut w, "h"), "2");
    assert_eq!(row_value(&mut w, "s"), "123.05");
}

fn row_value<'a>(w: &'a mut Workbench, label: &str) -> &'a str {
    w.rows()
        .iter()
        .find(|r| r.label == label)
        .and_then(|r| r.value.as_deref())
        .unwrap()
}

#[test]
fn a_construction_step_refreshes_rows_and_survives_replay() {
    let mut w = Workbench::from_script("s = ConstructionStep()\nA = (1, 2)\n").unwrap();
    assert_eq!(row_value(&mut w, "s"), "2");
    w.run_line("SetConstructionStep(1)").unwrap();
    assert_eq!(row_value(&mut w, "s"), "1");
    assert!(w.undo());
    assert_eq!(row_value(&mut w, "s"), "2");
    assert!(w.redo());
    assert_eq!(row_value(&mut w, "s"), "1");

    let mut restored = Workbench::default();
    assert!(restored
        .restore_text(&w.snapshot_text())
        .unwrap()
        .is_empty());
    assert_eq!(row_value(&mut restored, "s"), "1");

    let bad = ["SetConstructionStep(1/0)".into(), "q = Line(A, Z)".into()];
    assert!(w.run_lines(&bad).is_err());
    assert_eq!(row_value(&mut w, "s"), "1");
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
fn a_slow_plot_slider_assignment_survives_undo_and_redo() {
    let mut w = Workbench::from_script("f(x) = x\ng = SlowPlot(f)\np = g(2)\n").unwrap();
    let value = |w: &Workbench| {
        let c = w.construction();
        c.value(c.id("p").unwrap())
    };
    assert_eq!(value(&w), Value::Undefined);
    w.run_line("a = 2").unwrap();
    assert_eq!(value(&w), Value::Number(2.0));
    assert!(w.undo());
    assert_eq!(value(&w), Value::Undefined);
    assert!(w.redo());
    assert_eq!(value(&w), Value::Number(2.0));
}

#[test]
fn slow_plot_hides_its_source_and_plays_its_slider() {
    let mut w = Workbench::default();
    w.run_line("f(x) = sin(x)").unwrap();
    w.run_line("g = SlowPlot(f, false)").unwrap();
    let c = w.construction();
    let (f, a) = (c.id("f").unwrap(), c.id("a").unwrap());
    assert!(!w.is_visible(f));
    assert!(w.is_visible(a));
    assert!(w.is_animating());
    for _ in 0..400 {
        w.animate(30.0);
    }
    // played once to the upper end, without journal entries of its own
    assert!(!w.is_animating());
    assert_eq!(w.construction().value(a), Value::Number(1.0));
    assert!(w.undo());
    assert!(w.construction().id("g").is_none());
    assert!(w.is_visible(w.construction().id("f").unwrap()));
}

#[test]
fn a_rebuild_keeps_where_the_slow_plot_slider_was() {
    let mut w = Workbench::default();
    w.run_line("f(x) = x").unwrap();
    w.run_line("g = SlowPlot(f, false)").unwrap();
    for _ in 0..400 {
        w.animate(30.0);
    }
    assert!(!w.is_animating());
    w.run_line("B = (1, 1)").unwrap();
    let id = w.construction().id("B").unwrap();
    assert!(w.delete(id));
    let c = w.construction();
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(1.0));
    assert!(!w.is_animating());
    // a slider still playing goes on from where it was
    w.run_line("h = SlowPlot(f)").unwrap();
    for _ in 0..30 {
        w.animate(30.0);
    }
    let b = w.construction().id("b").unwrap();
    let before = w.construction().value(b);
    w.run_line("C = (2, 2)").unwrap();
    let id = w.construction().id("C").unwrap();
    assert!(w.delete(id));
    assert_eq!(w.construction().value(b), before);
    assert!(w.is_animating());
    w.set_animation_running(false);
    assert!(!w.is_animating() && w.has_animation());
}

#[test]
fn a_redone_slow_plot_plays_while_older_sliders_stay_paused() {
    let mut w = Workbench::default();
    w.run_line("f(x) = x").unwrap();
    w.run_line("g = SlowPlot(f)").unwrap();
    w.set_animation_running(false);
    w.run_line("h = SlowPlot(f)").unwrap();
    assert!(w.is_animating());
    w.set_animation_running(false);
    assert!(w.undo());
    assert!(w.construction().id("h").is_none());
    assert!(!w.is_animating());
    // the redone command starts its slider again, as running it did
    assert!(w.redo());
    assert!(w.construction().id("h").is_some());
    assert!(w.is_animating());
}

#[test]
fn a_typed_stroke_keeps_its_points_through_a_rebuild_in_another_view() {
    let mut w = Workbench::default();
    // 10 pixels per unit: the stroke is subdivided less than in the 50
    // pixels per unit of a construction a rebuild starts from
    w.set_view(crate::interact::ViewTransform::new(
        geo_kernel::svg::View {
            xmin: -40.0,
            xmax: 40.0,
            ymin: -40.0,
            ymax: 40.0,
        },
        800.0,
        800.0,
    ));
    w.run_line("s = PenStroke((-3, 3), (4, 3), (2, 5))")
        .unwrap();
    w.run_line("A = (1, 1)").unwrap();
    let bits = |w: &Workbench| {
        let c = w.construction();
        let stroke = c.stroke(c.id("s").unwrap()).unwrap();
        stroke
            .triplets()
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>()
    };
    let before = bits(&w);
    assert_eq!(before.len(), 3 * 16, "smoothed at 10 pixels per unit");
    assert_eq!(
        w.definition("s"),
        Some("PenStroke((-3, 3), (4, 3), (2, 5))")
    );
    let id = w.construction().id("A").unwrap();
    assert!(w.delete(id));
    assert_eq!(bits(&w), before);
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
fn equation_rows_keep_the_typed_form_after_a_coefficient_edit() {
    let mut w =
        Workbench::from_script("a = 2\nl: y = a x + 1\nm: 2x - y = -1\np: y = a x^2 - x + 1\n")
            .unwrap();
    let expected = [
        ("l", "y = 2 x + 1"),
        ("m", "2x - y = -1"),
        ("p", "y = 2 x² - x + 1"),
    ];
    for (label, value) in expected {
        let row = w.rows().iter().find(|row| row.label == label).unwrap();
        assert_eq!(row.value.as_deref(), Some(value), "{label}");
    }
    let a = w.construction().id("a").unwrap();
    assert!(w.set_number(a, -3.0));
    let row = w.rows().iter().find(|row| row.label == "p").unwrap();
    assert_eq!(row.value.as_deref(), Some("y = -3 x² - x + 1"));
}

#[test]
fn imported_equation_rows_keep_the_saved_form_through_replay() {
    let saved = include_bytes!("../../../../golden/ggb/saved/saved-text-equations-001.xml");
    let mut w = Workbench::default();
    w.load_bytes(saved, "saved equation forms").unwrap();
    let expected = [
        ("l", "y = 2 x + 1"),
        ("p", "y = 2 x² - x + 1"),
        ("e", "y = 2x + 1"),
        ("g", "-2x + y - 1 = 0"),
        ("r", "X = (0, 1) + u (1, 2)"),
    ];
    let assert_rows = |w: &mut Workbench| {
        for (label, value) in expected {
            let row = w.rows().iter().find(|row| row.label == label).unwrap();
            assert_eq!(row.value.as_deref(), Some(value), "{label}");
        }
    };
    assert_rows(&mut w);

    let mut restored = Workbench::default();
    assert!(restored
        .restore_text(&w.snapshot_text())
        .unwrap()
        .is_empty());
    assert_rows(&mut restored);
    restored.run_line("B' = (2, 4)").unwrap();
    assert!(restored.undo());
    assert_rows(&mut restored);
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

/// A locus freezes the start point of a line without one (the foot from the
/// origin, `PathMoverLine.init` installing `setStandardStartPoint`): the
/// parameter of a point on the line is measured from it. GeoGebra keeps that
/// start when the locus is deleted (it was installed on the line itself), so
/// the point stays where it was; the rebuild of a deletion keeps it too.
#[test]
fn deleting_a_locus_keeps_the_frozen_start_of_its_line() {
    let src = "M = (0, 0)\nR = (2, 0)\nk = Circle(M, R)\nQ = (5, 1)\nh = Polar(Q, k)\n\
               P = Point(h)\nt = PathParameter(P)\nB = (1, 3)\nC = Midpoint(P, B)\nL = Locus(C, P)\n";
    let mut w = Workbench::from_script(src).unwrap();
    let p = w.construction().id("P").unwrap();
    let t = w.construction().id("t").unwrap();
    let q = w.construction().id("Q").unwrap();
    // the line moves after the locus froze its start: the parameter of P is
    // measured from the start it had then, which stays with the line
    assert!(w.move_point(q, 3.0, 2.0));
    assert!(w.move_point(p, 1.0, 1.0));
    let (at, param) = (w.construction().value(p), w.construction().value(t));
    let l = w.construction().id("L").unwrap();
    assert!(w.delete(l));
    assert_eq!(
        w.construction().value(p),
        at,
        "the point stays where it was"
    );
    assert_eq!(
        w.construction().value(t),
        param,
        "its parameter keeps the frozen start"
    );
}

/// Moves recorded before the locus were made with no frozen start on the
/// line: the live foot of the line was the origin (`PathMoverLine` before
/// `init` installed the start point). A rebuild of a deletion must not
/// install the start before the entry that froze it, or those moves
/// replay from a different origin and the point ends up elsewhere. Moves
/// after the locus are measured from the frozen start, as in GeoGebra, which
/// does not rebuild on a deletion at all.
#[test]
fn a_deletion_keeps_a_point_moved_before_the_locus() {
    let src = "M = (0, 0)\nR = (2, 0)\nk = Circle(M, R)\nQ = (5, 1)\nh = Polar(Q, k)\n\
               P = Point(h)\nB = (1, 3)\nC = Midpoint(P, B)\nZ = (7, 7)\n";
    let mut w = Workbench::from_script(src).unwrap();
    let p = w.construction().id("P").unwrap();
    let q = w.construction().id("Q").unwrap();
    // before the locus: P moves along h, then h itself moves and P follows
    assert!(w.move_point(p, 1.0, 1.0));
    assert!(w.move_point(q, 3.0, 2.0));
    // the locus freezes the foot of h as it is then
    w.run_line("L = Locus(C, P)").unwrap();
    // after the locus: h moves again and P follows from the frozen start
    assert!(w.move_point(q, 4.0, 1.0));
    let at = w.construction().value(p);
    let z = w.construction().id("Z").unwrap();
    assert!(w.delete(z));
    assert_eq!(
        w.construction().value(p),
        at,
        "P stays exactly where it was before the deletion"
    );
}

/// A start whose locus entry is gone has to live in the journal, not only
/// in the construction: GeoGebra keeps the start for the rest of the
/// session (it installed it on the line itself), so deleting the locus
/// writes the start where the locus entry was, and every later replay —
/// further deletions, and the undo and redo of them — installs it at the
/// same place again, reproducing the points bit for bit.
#[test]
fn later_deletions_and_their_undo_keep_the_frozen_start() {
    let src = "M = (0, 0)\nR = (2, 0)\nk = Circle(M, R)\nQ = (5, 1)\nh = Polar(Q, k)\n\
               P = Point(h)\nB = (1, 3)\nC = Midpoint(P, B)\nZ = (7, 7)\nY = (8, 8)\n";
    let mut w = Workbench::from_script(src).unwrap();
    let p = w.construction().id("P").unwrap();
    let q = w.construction().id("Q").unwrap();
    // before the locus: P moves along h, then h itself moves and P follows
    assert!(w.move_point(p, 1.0, 1.0));
    assert!(w.move_point(q, 3.0, 2.0));
    // the locus freezes the foot of h as it is then
    w.run_line("L = Locus(C, P)").unwrap();
    // after the locus: h moves again and P follows from the frozen start
    assert!(w.move_point(q, 4.0, 1.0));
    let at = w.construction().value(p);
    // Every deletion rebuilds the construction, which renumbers the
    // objects, so each id is read again just before it goes.
    assert!(w.delete(w.construction().id("L").unwrap()));
    assert_eq!(w.construction().value(p), at, "deleting the locus keeps P");
    assert!(w.delete(w.construction().id("Z").unwrap()));
    assert_eq!(
        w.construction().value(p),
        at,
        "the first later deletion keeps P"
    );
    assert!(w.delete(w.construction().id("Y").unwrap()));
    assert_eq!(
        w.construction().value(p),
        at,
        "the second later deletion keeps P"
    );
    // the deletions undone replay the journal again, and the locus comes
    // back with the state before its own deletion
    assert!(w.undo());
    assert_eq!(
        w.construction().value(p),
        at,
        "undo of the second deletion keeps P"
    );
    assert!(w.undo());
    assert_eq!(
        w.construction().value(p),
        at,
        "undo of the first deletion keeps P"
    );
    assert!(w.undo());
    assert_eq!(
        w.construction().value(p),
        at,
        "undo of the locus deletion keeps P"
    );
    assert!(w.redo());
    assert_eq!(
        w.construction().value(p),
        at,
        "redo of the locus deletion keeps P"
    );
    assert!(w.redo());
    assert_eq!(
        w.construction().value(p),
        at,
        "redo of the first deletion keeps P"
    );
}

/// A saved script does not keep the frozen start (GeoGebra's files do not
/// save the point either): the journal entry that carries the start over
/// rebuilds is internal and never written, so a reload starts without one —
/// the moves replay from the live foot of the line — and the next locus
/// computation installs a fresh one, on the line as it is then.
#[test]
fn a_saved_script_does_not_keep_the_frozen_start() {
    let src = "M = (0, 0)\nR = (2, 0)\nk = Circle(M, R)\nQ = (5, 1)\nh = Polar(Q, k)\n\
               P = Point(h)\nB = (1, 3)\nC = Midpoint(P, B)\n";
    let mut w = Workbench::from_script(src).unwrap();
    let p = w.construction().id("P").unwrap();
    let q = w.construction().id("Q").unwrap();
    assert!(w.move_point(p, 1.0, 1.0));
    assert!(w.move_point(q, 3.0, 2.0));
    w.run_line("L = Locus(C, P)").unwrap();
    assert!(w.move_point(q, 4.0, 1.0));
    let at = w.construction().value(p);
    assert!(w.delete(w.construction().id("L").unwrap()));
    assert_eq!(w.construction().value(p), at, "deleting the locus keeps P");

    let mut re = Workbench::default();
    assert!(re.restore_text(&w.snapshot_text()).unwrap().is_empty());
    let value = |w: &Workbench| w.construction().value(w.construction().id("P").unwrap());
    assert_ne!(
        value(&re),
        at,
        "a reload does not keep the start the locus froze"
    );
    // the next locus computation installs a fresh one, and the entry its
    // own deletion writes keeps that one too
    re.run_line("L = Locus(C, P)").unwrap();
    let fresh = value(&re);
    assert!(re.delete(re.construction().id("L").unwrap()));
    assert_eq!(
        value(&re),
        fresh,
        "the fresh start stays when its locus goes"
    );
}

/// A start entry whose line is deleted goes with the line: the rebuild
/// drops it like the entries that used the line, instead of failing on a
/// label that is no longer there.
#[test]
fn deleting_the_line_drops_the_start_entry_it_kept() {
    let src = "M = (0, 0)\nR = (2, 0)\nk = Circle(M, R)\nQ = (5, 1)\nh = Polar(Q, k)\n\
               P = Point(h)\nB = (1, 3)\nC = Midpoint(P, B)\n";
    let mut w = Workbench::from_script(src).unwrap();
    let p = w.construction().id("P").unwrap();
    let q = w.construction().id("Q").unwrap();
    assert!(w.move_point(p, 1.0, 1.0));
    w.run_line("L = Locus(C, P)").unwrap();
    assert!(w.move_point(q, 4.0, 1.0));
    // the deletion of the locus writes the start entry into the journal
    assert!(w.delete(w.construction().id("L").unwrap()));
    // the circle goes, and with it the line, the point on it and the entry
    // that kept the line's start
    assert!(w.delete(w.construction().id("k").unwrap()));
    assert!(w.construction().id("P").is_none());
    assert!(w.construction().id("h").is_none());
    assert_eq!(labels(&w), ["M", "R", "Q", "B"]);
}

/// The same with the locus itself deleted: the journal entry its deletion
/// writes belongs after the moves the locus found in place (they replayed
/// with no start, the live foot of the line) and before the moves made
/// after it (measured from the start, as the frozen start stays with the
/// line in GeoGebra).
#[test]
fn deleting_the_locus_keeps_a_point_moved_before_it() {
    let src = "M = (0, 0)\nR = (2, 0)\nk = Circle(M, R)\nQ = (5, 1)\nh = Polar(Q, k)\n\
               P = Point(h)\nB = (1, 3)\nC = Midpoint(P, B)\n";
    let mut w = Workbench::from_script(src).unwrap();
    let p = w.construction().id("P").unwrap();
    let q = w.construction().id("Q").unwrap();
    assert!(w.move_point(p, 1.0, 1.0));
    assert!(w.move_point(q, 3.0, 2.0));
    w.run_line("L = Locus(C, P)").unwrap();
    assert!(w.move_point(q, 4.0, 1.0));
    let at = w.construction().value(p);
    let l = w.construction().id("L").unwrap();
    assert!(w.delete(l));
    assert_eq!(
        w.construction().value(p),
        at,
        "P stays exactly where it was before the deletion"
    );
}

const BOUNDARY_GROWTH: &str = "A = (0, 0)\nB = (2, 0)\nC = (2, 2)\nD = (0, 2)\n\
    p = Polygon(A, B, C, D)\nG = (-1, 3)\nH = (3, 3)\ng = Line(G, H)\nR = Intersect(g, p)";

#[test]
fn boundary_outputs_grow_only_after_the_whole_translation() {
    let mut w = Workbench::from_script(BOUNDARY_GROWTH).unwrap();
    let points = ["G", "H"].map(|label| w.construction().id(label).unwrap());
    let labels_before = labels(&w);
    w.begin_gesture();
    assert!(w.translate(&points, (0.0, -6.0)));
    w.end_gesture();
    // The intermediate tilted line crosses the square twice. The line as
    // a whole has moved from above the square to below it.
    assert_eq!(labels(&w), labels_before);
    assert!(w.undo());
    assert!(w.redo());
    assert_eq!(labels(&w), labels_before);

    let mut saved = Workbench::default();
    saved
        .load_bytes(w.snapshot_text().as_bytes(), "construction")
        .unwrap();
    assert_eq!(labels(&saved), labels_before);

    let points = ["G", "H"].map(|label| saved.construction().id(label).unwrap());
    assert!(saved.translate(&points, (0.0, 4.0)));
    let second = saved.construction().id("R_{2}").unwrap();
    assert!(matches!(
        saved.construction().value(second),
        Value::Point { y: 1.0, .. }
    ));

    assert!(saved.delete(points[0]));
    let c = saved.construction();
    assert_eq!(c.value(c.id("H").unwrap()), Value::Point { x: 3.0, y: 1.0 });
    assert!(c.id("R_{2}").is_none());
}

#[test]
fn interleaved_boundary_groups_replay_after_unrelated_deletion() {
    let src = "Z = (9, 9)\nA = (0, 0)\nB = (2, 0)\nC = (2, 2)\nD = (0, 2)\n\
        p = Polygon(A, B, C, D)\nn = 3\nE = (n, 0.5)\nF = (n + 2, 0.5)\n\
        G = (n + 2, 1.5)\nH = (n, 1.5)\nq = Polygon(E, F, G, H)\nR = Intersect(p, q)\n\
        u = 3\nsLine: y = u\nS = Intersect(sLine, p)\nv = 3\ntLine: y = v\nT = Intersect(tLine, q)";
    let mut w = Workbench::from_script(src).unwrap();
    let [n, u, v] = ["n", "u", "v"].map(|label| w.construction().id(label).unwrap());
    for label in ["R_{2}", "S_{2}", "T_{2}"] {
        assert!(w.construction().id(label).is_none());
    }

    assert!(w.set_number(u, 0.5));
    assert!(w.set_number(n, 1.0));
    let pair_points = ["R_{1}", "R_{2}"].map(|label| {
        let c = w.construction();
        c.value(c.id(label).unwrap())
    });
    assert!(pair_points.contains(&Value::Point { x: 2.0, y: 0.5 }));
    assert!(pair_points.contains(&Value::Point { x: 2.0, y: 1.5 }));
    assert!(w.set_number(u, 3.0));
    assert!(w.set_number(v, 1.0));
    assert!(w.set_number(n, 3.0));
    w.run_line("W = Midpoint(T_{1}, T_{2})").unwrap();

    let before = labels(&w);
    assert!(w.delete(w.construction().id("Z").unwrap()));
    let after = &before[1..];
    assert_eq!(labels(&w), after);
    assert!(w.undo());
    assert_eq!(labels(&w), before);
    assert!(w.redo());
    assert_eq!(labels(&w), after);

    let mut saved = Workbench::default();
    assert!(saved.restore_text(&w.snapshot_text()).unwrap().is_empty());
    assert_eq!(labels(&saved), after);
    for (label, rhs) in [
        ("R_{2}", "Intersect(p, q)"),
        ("S_{2}", "Intersect(sLine, p)"),
    ] {
        let id = saved.construction().id(label).unwrap();
        assert_eq!(saved.construction().value(id), Value::Undefined);
        assert_eq!(saved.definition(label), Some(rhs));
    }
    let c = saved.construction();
    assert_eq!(c.value(c.id("W").unwrap()), Value::Point { x: 4.0, y: 1.0 });
    assert!(saved.set_number(c.id("n").unwrap(), 1.0));
    assert_eq!(labels(&saved), after);
}

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

#[test]
fn inequality_vertices_keep_grown_labels_through_replay() {
    let src = "P_{1} = (8, 9)\nh = -1\nP = Vertex(x > 0 && y > 0 && x + y < h)";
    let mut w = Workbench::from_script(src).unwrap();
    let h = w.construction().id("h").unwrap();
    assert!(w.construction().id("P_{2}").is_some());
    assert!(w.construction().id("P_{3}").is_none());
    assert!(w.set_number(h, 2.0));
    assert!(w.construction().id("P_{4}").is_some());
    assert!(w.set_number(h, -1.0));
    assert!(w.delete(w.construction().id("P_{1}").unwrap()));

    let mut restored = Workbench::default();
    assert!(restored
        .restore_text(&w.snapshot_text())
        .unwrap()
        .is_empty());
    assert!(restored.construction().id("P_{1}").is_none());
    for label in ["P_{2}", "P_{3}", "P_{4}"] {
        let c = restored.construction();
        assert_eq!(c.value(c.id(label).unwrap()), Value::Undefined);
    }
    assert!(restored.set_number(restored.construction().id("h").unwrap(), 3.0));
    let c = restored.construction();
    let last = c.id("P_{4}").unwrap();
    assert_eq!(c.value(last), Value::Point { x: 3.0, y: 0.0 });
    assert!(restored.delete(last));
    for label in ["P_{2}", "P_{3}", "P_{4}"] {
        assert!(restored.construction().id(label).is_none());
    }
}

#[test]
fn implicit_tangents_keep_grown_labels_through_replay() {
    let src = "h = 1\nA = (h, 0)\nc: x^4 + y^4 = 1\nt_{1} = Line(A, xAxis)\nt = Tangent(A, c)";
    let mut w = Workbench::from_script(src).unwrap();
    let h = w.construction().id("h").unwrap();
    assert!(w.construction().id("t_{2}").is_some());
    assert!(w.construction().id("t_{3}").is_none());
    assert!(w.set_number(h, 2.0));
    assert!(w.construction().id("t_{3}").is_some());
    assert!(w.set_number(h, 0.0));
    assert!(w.delete(w.construction().id("t_{1}").unwrap()));

    let mut restored = Workbench::default();
    assert!(restored
        .restore_text(&w.snapshot_text())
        .unwrap()
        .is_empty());
    let c = restored.construction();
    assert!(c.id("t_{1}").is_none());
    let tangents = ["t_{2}", "t_{3}"].map(|label| c.id(label).unwrap());
    assert!(tangents.iter().all(|&id| c.value(id) == Value::Undefined));
    assert!(restored.set_number(c.id("h").unwrap(), 2.0));
    assert!(tangents
        .iter()
        .all(|&id| matches!(restored.construction().value(id), Value::Line { .. })));
}

#[test]
fn automatic_intersection_follows_its_root_through_journal_replay() {
    let mut w = Workbench::default();
    for line in [
        "a = 0",
        "f(x) = sin(x - a)",
        "R = Intersect(f, xAxis)",
        "U = (2, 3)",
    ] {
        w.run_line(line).unwrap();
    }
    let a = w.construction().id("a").unwrap();
    for x in 1..=4 {
        assert!(w.set_number(a, f64::from(x)));
    }
    let check = |w: &Workbench, expected: f64| {
        let c = w.construction();
        let Value::Point { x, y } = c.value(c.id("R").unwrap()) else {
            panic!("undefined intersection");
        };
        assert!(
            (x - expected).abs() < 1e-9 && y.abs() < 1e-9,
            "R = ({x}, {y})"
        );
    };
    check(&w, 4.0);
    assert!(w.undo());
    check(&w, 3.0);
    assert!(w.redo());
    check(&w, 4.0);
    assert!(w.delete(w.construction().id("U").unwrap()));
    check(&w, 4.0);
    let mut reloaded = Workbench::default();
    let problems = reloaded.restore_text(&w.snapshot_text()).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    check(&reloaded, 4.0);
}

#[test]
fn base_and_relation_text_rows_update_and_survive_undo() {
    let mut w = Workbench::default();
    for input in [
        "a = 15",
        "b = 15",
        "s = ToBase(a, 16)",
        "r = Relation(a, b)",
    ] {
        w.run_line(input).unwrap();
    }
    let text = |w: &mut Workbench, name: &str| {
        w.rows()
            .iter()
            .find(|row| row.label == name)
            .unwrap()
            .value
            .clone()
            .unwrap()
    };
    assert_eq!(text(&mut w, "s"), "F");
    let equal = text(&mut w, "r");
    assert!(equal.contains("equal"));
    let id = w.construction().id("a").unwrap();
    assert!(w.set_number(id, 16.0));
    assert_eq!(text(&mut w, "s"), "10");
    assert_ne!(text(&mut w, "r"), equal);
    assert!(w.undo());
    assert_eq!(text(&mut w, "s"), "F");
    assert_eq!(text(&mut w, "r"), equal);
}

#[test]
fn a_literal_circle_radius_uses_no_object_id() {
    let mut w = Workbench::from_script("A = (1, 2)\n").unwrap();
    w.run_line("Circle(A, 2)").unwrap();
    w.run_line("P = Center(c)").unwrap();
    w.run_line("n = Radius(c)").unwrap();

    let check = |w: &mut Workbench, cx, cy| {
        let c = w.construction();
        assert_eq!(c.len(), 4);
        for (index, label) in ["A", "c", "P", "n"].into_iter().enumerate() {
            assert_eq!(c.id(label).unwrap().index(), index);
        }
        assert_eq!(
            c.value(c.id("c").unwrap()),
            Value::Circle { cx, cy, r: 2.0 }
        );
        let rows: Vec<_> = w.rows().iter().map(|r| r.label.as_str()).collect();
        assert_eq!(rows, ["A", "c", "P", "n"]);
    };
    check(&mut w, 1.0, 2.0);
    assert!(w.move_point(w.construction().id("A").unwrap(), -3.0, 4.0));
    check(&mut w, -3.0, 4.0);
    assert!(w.undo());
    check(&mut w, 1.0, 2.0);
    assert!(w.redo());
    check(&mut w, -3.0, 4.0);

    let mut loaded = Workbench::default();
    let issues = loaded.restore_text(&w.snapshot_text()).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    check(&mut loaded, -3.0, 4.0);
}

#[test]
fn a_hash_inside_text_survives_a_script_and_saved_snapshot() {
    let mut w = Workbench::default();
    w.run_line(r#"n = Length("a# labels: b") # trailing comment"#)
        .unwrap();
    let mut loaded = Workbench::default();
    let issues = loaded.restore_text(&w.snapshot_text()).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    let c = loaded.construction();
    assert_eq!(c.value(c.id("n").unwrap()), Value::Number(12.0));
}

#[test]
fn equation_sides_keep_their_inputs_through_rebuild_and_undo() {
    let mut w = Workbench::from_script(
        "unused(x) = x+1\n\
         a = 2\n\
         j: a*x+y = a*x+5*x+1\n\
         f = LeftSide(j)\n\
         g = RightSide(j)\n\
         n = f(3,4)\n\
         m = g(3,4)\n",
    )
    .unwrap();
    let check = |w: &Workbench, left, right| {
        let c = w.construction();
        for (side, number, expected) in [("f", "n", left), ("g", "m", right)] {
            assert_eq!(
                c.function_value_xy(c.id(side).unwrap(), 3.0, 4.0),
                Some(expected)
            );
            assert_eq!(c.value(c.id(number).unwrap()), Value::Number(expected));
        }
    };
    check(&w, 10.0, 22.0);
    // Both sides depend on a, although it cancels in the line's coefficients.
    assert!(w.set_number(w.construction().id("a").unwrap(), -1.0));
    check(&w, 1.0, 13.0);
    assert!(w.undo());
    check(&w, 10.0, 22.0);
    assert!(w.redo());
    check(&w, 1.0, 13.0);
    assert!(w.delete(w.construction().id("unused").unwrap()));
    check(&w, 1.0, 13.0);
    assert!(w.undo());
    check(&w, 1.0, 13.0);
}

#[test]
fn deleting_a_replacement_parent_removes_the_active_dependents() {
    let mut w = Workbench::from_script("X = (1, 1)\nA = (0, 0)\nB = 2 A\nC = (4, 1)\n").unwrap();
    w.run_line("A = X + (1, 0)").unwrap();
    let check = |w: &Workbench, exists| {
        let c = w.construction();
        for label in ["X", "A", "B"] {
            assert_eq!(c.id(label).is_some(), exists, "{label}");
        }
        assert!(c.id("C").is_some());
        if exists {
            assert_eq!(c.value(c.id("A").unwrap()), Value::Point { x: 2.0, y: 1.0 });
        }
    };
    assert!(w.delete(w.construction().id("X").unwrap()));
    check(&w, false);
    assert!(w.undo());
    check(&w, true);
    assert!(w.redo());
    check(&w, false);
    let mut restored = Workbench::default();
    assert!(restored
        .restore_text(&w.snapshot_text())
        .unwrap()
        .is_empty());
    check(&restored, false);
}

#[test]
fn deleting_a_superseded_parent_keeps_the_redefined_object_and_its_dependents() {
    for (replacement, tail, expected) in [
        ("(0, 0)", "C = (4, 1)", Value::Point { x: 0.0, y: 0.0 }),
        ("(n, 1 / n)", "n = 2", Value::Point { x: 4.0, y: 1.0 }),
    ] {
        let mut w = Workbench::from_script(&format!("X = (1, 1)\nA = X + (1, 0)\nB = 2 A\n{tail}"))
            .unwrap();
        w.run_line(&format!("A = {replacement}")).unwrap();
        let before = w.snapshot_text();
        let check = |w: &Workbench| {
            let c = w.construction();
            assert!(c.id("X").is_none());
            let tail = tail.split_once(' ').unwrap().0;
            assert_eq!(labels(w), ["A", "B", tail]);
            assert_eq!(c.value(c.id("B").unwrap()), expected);
            assert_eq!(w.log.rhs_of("A"), Some(replacement));
        };
        assert!(w.delete(w.construction().id("X").unwrap()));
        check(&w);
        assert!(w.undo());
        assert_eq!(w.snapshot_text(), before);
        assert!(w.redo());
        check(&w);
        let mut restored = Workbench::default();
        assert!(restored
            .restore_text(&w.snapshot_text())
            .unwrap()
            .is_empty());
        check(&restored);
    }
}

/// A failed undo keeps its step: the stacks go back to exactly what they
/// were, so the undo can be taken again and still reaches the state the
/// step holds (the recovery used to push a fresh state of the current
/// journal, which replaced the step with a no-op).
#[test]
fn a_failed_undo_keeps_its_step() {
    let mut w = Workbench::from_script("A = (0, 0)\n").unwrap();
    w.run_line("C = (5, 5)").unwrap();
    w.run_line("D = (6, 6)").unwrap();
    assert_eq!(w.undo_depth(), (2, 0));
    // Every rebuild fails, as a replay of an undo step would.
    w.fail_rebuilds = Some("test".into());
    assert!(!w.undo());
    w.fail_rebuilds = None;
    assert_eq!(w.undo_depth(), (2, 0), "the stacks are as they were");
    assert!(w.undo());
    assert_eq!(labels(&w), ["A", "C"], "the step still holds its state");
    assert!(w.undo());
    assert_eq!(labels(&w), ["A"]);
    assert!(w.redo());
    assert_eq!(labels(&w), ["A", "C"]);
}

/// The same for a failed redo: the redo step stays, so trying again still
/// goes to the state it holds.
#[test]
fn a_failed_redo_keeps_its_step() {
    let mut w = Workbench::from_script("A = (0, 0)\n").unwrap();
    w.run_line("C = (5, 5)").unwrap();
    w.run_line("D = (6, 6)").unwrap();
    assert!(w.undo() && w.undo());
    assert_eq!(labels(&w), ["A"]);
    assert_eq!(w.undo_depth(), (0, 2));
    w.fail_rebuilds = Some("test".into());
    assert!(!w.redo());
    w.fail_rebuilds = None;
    assert_eq!(w.undo_depth(), (0, 2), "the stacks are as they were");
    assert!(w.redo());
    assert_eq!(labels(&w), ["A", "C"], "the step still holds its state");
    assert!(w.redo());
    assert_eq!(labels(&w), ["A", "C", "D"]);
}

/// A view-only step changed no object, so neither its undo nor its redo
/// replays the journal: the construction is never rebuilt on the way
/// (`step_back`), in either direction — the state the stacks keep for the
/// way back is view-only too.
#[test]
fn the_undo_and_redo_of_a_view_only_step_do_not_replay() {
    let mut w = Workbench::from_script("A = (0, 0)\n").unwrap();
    w.run_line("C = (5, 5)").unwrap();
    w.set_camera(crate::view::Camera::uniform(0.0, 0.0, 50.0));
    w.record_view_step(crate::view::Camera::uniform(3.0, 1.0, 50.0));
    assert_eq!(w.undo_depth(), (2, 0));
    let (rebuilds, version) = (w.rebuilds(), w.version());
    assert!(w.undo());
    assert_eq!(
        (w.rebuilds(), w.version()),
        (rebuilds, version),
        "the undo replayed the journal"
    );
    assert!(w.redo());
    assert_eq!(
        (w.rebuilds(), w.version()),
        (rebuilds, version),
        "the redo replayed the journal"
    );
    assert_eq!(labels(&w), ["A", "C"]);
}

/// A long run of view changes cannot evict construction steps from the
/// undo history: the oldest view-only step goes first, and none being left
/// to take, the history just grows past its bound until a construction
/// step comes (`Undo::record_view`). The reference app, which has no
/// bound, keeps them all; this keeps as many as the bound allows.
#[test]
fn sustained_view_changes_do_not_evict_construction_history() {
    let mut w = Workbench::from_script("A = (0, 0)\n").unwrap();
    w.run_line("C = (5, 5)").unwrap();
    w.run_line("D = (6, 6)").unwrap();
    for k in 0..600 {
        let before = crate::view::Camera::uniform(k as f64, 0.0, 50.0);
        w.set_camera(crate::view::Camera::uniform(k as f64 + 1.0, 0.0, 50.0));
        w.record_view_step(before);
    }
    // Undo to the beginning: the two construction steps survived all 600
    // view steps (a view-only undo replays nothing, so this is quick).
    let mut undos = 0;
    while w.undo() {
        undos += 1;
    }
    assert_eq!(labels(&w), ["A"], "a construction step was evicted");
    assert!(undos >= 2, "only {undos} steps left");
}

#[test]
fn typed_slider_assignments_reveal_interval_roots() {
    let src = "b = Slider(1, 13)\nb = 4\nf(x) = sin(x)\nh(x) = 0\nR = Intersect(f, h, 1, b)";
    let check = |w: &Workbench| {
        let c = w.construction();
        for (label, x) in [("A", 2.0), ("B", 3.0)] {
            let id = c.id(label).unwrap_or_else(|| panic!("no {label}"));
            let Value::Point { x: px, .. } = c.value(id) else {
                panic!("{label} is undefined");
            };
            assert!(
                (px - x * std::f64::consts::PI).abs() < 1e-6,
                "{label}: {px}"
            );
        }
    };
    let mut w = Workbench::from_script(src).unwrap();
    assert!(w.construction().id("A").is_none());
    w.run_line("b = 10").unwrap();
    check(&w);
    // A replay of the journal keeps the grown labels.
    let mut restored = Workbench::default();
    assert!(restored
        .restore_text(&w.snapshot_text())
        .unwrap()
        .is_empty());
    check(&restored);
    // A plain script with the assignment grows the same outputs.
    check(&Workbench::from_script(&format!("{src}\nb = 10")).unwrap());
}

#[test]
fn points_from_a_growing_list_keep_their_labels_after_reload() {
    let src = "a = 1\nl = If(a > 0, {{a, 2}}, {{a, 2}, {3, 4}})\nP = Point(l)";
    let mut w = Workbench::from_script(src).unwrap();
    assert!(w.construction().id("A").is_none());
    w.run_line("a = 0").unwrap();
    let id = w.construction().id("A").unwrap();
    assert_eq!(w.construction().value(id), Value::Point { x: 3.0, y: 4.0 });
    w.run_line("a = 1").unwrap();
    assert_eq!(w.construction().value(id), Value::Undefined);
    let bytes = w.export_ggb(None).unwrap();
    let mut restored = Workbench::default();
    restored.load_for_export(&bytes, "coordinate list").unwrap();
    let id = restored.construction().id("A").unwrap();
    assert_eq!(restored.construction().value(id), Value::Undefined);
    restored.run_line("a = 0").unwrap();
    assert_eq!(
        restored.construction().value(id),
        Value::Point { x: 3.0, y: 4.0 }
    );
    assert!(restored.construction().id("B").is_none());
}

#[test]
fn recorded_slots_do_not_stop_later_growth() {
    let src = concat!(
        "b = Slider(1, 13)\nb = 4\nf(x) = sin(x)\nh(x) = 0\nR = Intersect(f, h, 1, b)\n",
        "b = 7\nA = $boundarySlot(R, 2)\nb = 10"
    );
    let w = Workbench::from_script(src).unwrap();
    let c = w.construction();
    for (label, x) in [("A", 2.0), ("B", 3.0)] {
        let id = c.id(label).unwrap_or_else(|| panic!("no {label}"));
        let Value::Point { x: px, .. } = c.value(id) else {
            panic!("{label} is undefined");
        };
        assert!(
            (px - x * std::f64::consts::PI).abs() < 1e-6,
            "{label}: {px}"
        );
    }
    assert!(c.id("C").is_none());
}

/// The labelled objects with their values, by label.
fn labelled_values(w: &Workbench) -> Vec<String> {
    let c = w.construction();
    let mut values: Vec<_> = c
        .ids()
        .filter(|&id| !c.label(id).starts_with('\0'))
        .map(|id| format!("{} = {:?}", c.label(id), c.value(id)))
        .collect();
    values.sort();
    values
}

/// A script, the slider it edits, the values it takes, and the labels
/// present and absent at the end.
type LabelCase<'a> = (&'a str, &'a str, &'a [f64], &'a [&'a str], &'a [&'a str]);

#[test]
fn interval_roots_free_labels_and_take_new_ones_through_every_replay() {
    let cases: [LabelCase; 3] = [
        (
            "f(x) = sin(x)\nh(x) = 0\nb = 10\nR = Intersect(f, h, 1, b)\nd = x(R_{3})",
            "b",
            &[2.0, 10.0, 13.0, 2.0, 13.0],
            &["R", "R_{3}", "A", "B", "d"],
            &["R_{1}", "R_{2}"],
        ),
        (
            "f(x) = x^2 - 1\nh(x) = 0\na = 3\nb = -3\nR = Intersect(f, h, a, b)",
            "a",
            &[-3.0, 1.0, 0.5, -0.5],
            &["R_{1}"],
            &["R_{2}", "A"],
        ),
        (
            concat!(
                "b = 1.5\nf(x) = (x - 1) (x - 2)\ng(x) = (x - 3) (x - 4) (x - 5)\n",
                "h(x) = 0\nR = Intersect(f, h, b, 3)\nS = Intersect(g, h, 0, b)"
            ),
            "b",
            &[0.5, 5.5, 0.5, 1.5, 5.5],
            &["R", "S", "A", "B"],
            &["C"],
        ),
    ];
    for (src, slider, values, present, absent) in cases {
        let mut w = Workbench::from_script(src).unwrap();
        let mut states = vec![labelled_values(&w)];
        for &value in values {
            let id = w.construction().id(slider).unwrap();
            assert!(w.set_number(id, value));
            states.push(labelled_values(&w));
        }
        let last = states.last().unwrap();
        let c = w.construction();
        for label in present {
            assert!(c.id(label).is_some(), "{label} missing after {src:?}");
        }
        for label in absent {
            assert!(c.id(label).is_none(), "{label} kept after {src:?}");
        }
        // The snapshot, its script without the header and the bare lines
        // reproduce the labels.
        let snapshot = w.snapshot_text();
        let mut restored = Workbench::default();
        assert!(restored.restore_text(&snapshot).unwrap().is_empty());
        assert_eq!(labelled_values(&restored), *last, "{snapshot}");
        let script: String = snapshot.lines().skip(1).map(|l| format!("{l}\n")).collect();
        restored.load_script(&script).unwrap();
        assert_eq!(labelled_values(&restored), *last, "{script}");
        let bare: String = script
            .lines()
            .map(|l| format!("{}\n", geo_kernel::script::split_comment(l).0))
            .collect();
        assert!(restored.restore_text(&bare).unwrap().is_empty());
        assert_eq!(labelled_values(&restored), *last, "{bare}");
        // Undo and redo rebuild every state from the journal.
        for back in 1..=values.len() {
            assert!(w.undo());
            assert_eq!(labelled_values(&w), states[values.len() - back]);
        }
        for state in &states[1..=values.len()] {
            assert!(w.redo());
            assert_eq!(labelled_values(&w), *state);
        }
    }
}

#[test]
fn export_refuses_interval_roots_with_a_freed_label() {
    let src = "f(x) = x^2 - 1\nh(x) = 0\na = 3\nb = -3\nR = Intersect(f, h, a, b)";
    let mut w = Workbench::from_script(src).unwrap();
    assert!(w.export_ggb(None).is_ok());
    let a = w.construction().id("a").unwrap();
    assert!(w.set_number(a, -3.0));
    let error = w.export_ggb(None).unwrap_err().to_string();
    assert!(error.contains("freed"), "{error}");
}

#[test]
fn deleting_a_reader_keeps_an_interval_root_that_a_later_line_reads() {
    let src = "f(x) = sin(x)\nh(x) = 0\nb = 10\nR = Intersect(f, h, 1, b)\nd = x(R_{2})";
    let mut w = Workbench::from_script(src).unwrap();
    let b = w.construction().id("b").unwrap();
    // d keeps R_{2} labelled while it is undefined; e reads it then
    assert!(w.set_number(b, 4.0));
    w.run_line("e = x(R_{2}) + 1").unwrap();
    assert!(w.set_number(b, 10.0));
    let d = w.construction().id("d").unwrap();
    assert!(w.delete(d));
    let c = w.construction();
    assert!(c.id("d").is_none());
    let root = c.id("R_{2}").expect("R_{2} stays labelled");
    let Value::Point { x, .. } = c.value(root) else {
        panic!("R_{{2}} is undefined");
    };
    assert!((x - 2.0 * std::f64::consts::PI).abs() < 1e-6, "{x}");
    let e = c.id("e").expect("e survives");
    assert!(matches!(c.value(e), Value::Number(v) if (v - x - 1.0).abs() < 1e-9));
    // Every replay of the journal the deletion left gives the same labels.
    let after = labelled_values(&w);
    assert!(w.undo());
    assert!(w.construction().id("d").is_some());
    assert!(w.redo());
    assert_eq!(labelled_values(&w), after);
    let b = w.construction().id("b").unwrap();
    assert!(w.set_number(b, 7.0));
    assert!(w.undo());
    assert_eq!(labelled_values(&w), after);
    let snapshot = w.snapshot_text();
    let mut restored = Workbench::default();
    assert!(restored.restore_text(&snapshot).unwrap().is_empty());
    assert_eq!(labelled_values(&restored), after, "{snapshot}");
    let script: String = snapshot.lines().skip(1).map(|l| format!("{l}\n")).collect();
    restored.load_script(&script).unwrap();
    assert_eq!(labelled_values(&restored), after, "{script}");
    let bare: String = script
        .lines()
        .map(|l| format!("{}\n", geo_kernel::script::split_comment(l).0))
        .collect();
    assert!(restored.restore_text(&bare).unwrap().is_empty());
    assert_eq!(labelled_values(&restored), after, "{bare}");
    // Later edits free outputs again: with e gone, R_{2} loses its label
    // when b leaves it without a root.
    let e = w.construction().id("e").unwrap();
    assert!(w.delete(e));
    let b = w.construction().id("b").unwrap();
    assert!(w.set_number(b, 4.0));
    assert!(w.construction().id("R_{2}").is_none());
}

#[test]
fn deleting_an_object_replays_labels_another_command_took() {
    let src = concat!(
        "P = (1, 1)\nb = 1.5\nf(x) = (x - 1) (x - 2)\ng(x) = (x - 3) (x - 4) (x - 5)\n",
        "h(x) = 0\nR = Intersect(f, h, b, 3)\nS = Intersect(g, h, 0, b)"
    );
    let mut w = Workbench::from_script(src).unwrap();
    for value in [0.5, 5.5] {
        let b = w.construction().id("b").unwrap();
        assert!(w.set_number(b, value));
    }
    // R freed A, which S took
    let before: Vec<_> = labelled_values(&w)
        .into_iter()
        .filter(|row| !row.starts_with("P "))
        .collect();
    let p = w.construction().id("P").unwrap();
    assert!(w.delete(p));
    assert_eq!(labelled_values(&w), before);
}
