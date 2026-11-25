use super::*;

fn open_point_properties(h: &mut Harness<'_, App>) {
    click(h, row_menu(0.0, false));
    h.get_by_label("Właściwości obiektu").click();
    h.run();
}

#[test]
fn visibility_edit_updates_canvas_and_algebra_in_one_undo_step() {
    let mut h = harness("A = (2, 1)");
    let shown = |app: &App| app.wb.is_visible(app.wb.construction().id("A").unwrap());
    open_point_properties(&mut h);
    assert!(shown(h.state()));
    h.get_by_label("Pokaż obiekt").click();
    h.run();
    assert!(!shown(h.state()));
    let a = h.state().wb.construction().id("A").unwrap();
    assert!(!h
        .state()
        .label_rects
        .borrow()
        .iter()
        .any(|&(id, _)| id == a));
    assert!(h
        .state()
        .wb
        .current_rows()
        .iter()
        .any(|row| row.label == "A"));
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    assert!(h.state_mut().undo());
    h.run();
    assert!(shown(h.state()));
    assert!(h
        .state()
        .wb
        .current_rows()
        .iter()
        .any(|row| row.label == "A"));
}

fn pick(h: &mut Harness<'_, App>, label: &str) {
    h.get_by_label(label).click();
    h.run();
}

fn enter(h: &mut Harness<'_, App>, label: &str, value: &str) {
    h.get_by_label(label).focus();
    h.run();
    assert!(h.get_by_label(label).is_focused(), "{label}");
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run();
    h.input_mut().events.push(Event::Text(value.into()));
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
}

#[test]
fn point_style_and_position_fields_are_labelled_and_undoable() {
    let mut h = harness("A = (2, 1)\nB = (4, 1)\nM = Midpoint(A, B)");
    open_point_properties(&mut h);
    pick(&mut h, "Styl");
    enter(&mut h, "Rozmiar punktu", "8");
    pick(&mut h, "Położenie");
    enter(&mut h, "Przesunięcie etykiety X (px)", "20");
    assert_eq!(h.state().wb.undo_depth(), (2, 0));
    enter(&mut h, "Przesunięcie etykiety Y (px)", "15");
    assert_eq!(h.state().wb.undo_depth(), (3, 0));
    assert!(h.state_mut().undo());
    assert!(h.state_mut().undo());
    assert!(h.state_mut().undo());
    assert_eq!(h.state().wb.undo_depth(), (0, 3));
}

#[test]
fn condition_tracks_boolean_and_does_not_change_the_algebra_value() {
    let mut h = harness("A = (2, 1)\nb = false");
    open_point_properties(&mut h);
    pick(&mut h, "Zaawansowane");
    enter(&mut h, "Warunek wyświetlania", "b");
    let c = h.state().wb.construction();
    assert!(!h.state().wb.is_visible(c.id("A").unwrap()));
    assert_eq!(c.value(c.id("A").unwrap()), Value::Point { x: 2.0, y: 1.0 });
    assert!(h.state_mut().undo());
    let c = h.state().wb.construction();
    assert!(h.state().wb.is_visible(c.id("A").unwrap()));
}

#[test]
fn slider_bound_clamps_the_number_and_updates_its_dependent() {
    let mut h = harness("a = Slider(0, 10)\nb = a * 2");
    let id = h.state().wb.construction().id("a").unwrap();
    h.state_mut().wb.set_number(id, 7.0);
    open_point_properties(&mut h);
    pick(&mut h, "Zaawansowane");
    enter(&mut h, "Maksimum suwaka", "4");
    let c = h.state().wb.construction();
    assert_eq!(
        c.slider_interval(id),
        Some((0.0, 4.0)),
        "{:?}",
        h.state()
            .properties
            .as_ref()
            .and_then(|dialog| dialog.error)
    );
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(4.0));
    assert_eq!(c.value(c.id("b").unwrap()), Value::Number(8.0));
    assert!(h.state_mut().undo());
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(7.0));
    assert_eq!(c.value(c.id("b").unwrap()), Value::Number(14.0));
}

#[test]
fn point_caption_color_and_shape_change_painted_objects_and_undo() {
    let mut h = harness("A = (2, 1)\nB = (4, 1)");
    open_point_properties(&mut h);
    enter(&mut h, "Podpis", "Wierzchołek");
    pick(&mut h, "Treść etykiety");
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Podpis")
        .click();
    h.run();
    pick(&mut h, "Kolor");
    enter(&mut h, "Kolor obiektu", "#DB3456");
    pick(&mut h, "Styl");
    pick(&mut h, "Kształt punktu");
    pick(&mut h, "Okrąg");
    let c = h.state().wb.construction();
    let id = c.id("A").unwrap();
    assert_eq!(c.appearance(id).point_style, Some(2));
    assert_eq!(h.state().wb.palette().color(id).r, 0xDB);
    assert_eq!(h.state().wb.undo_depth(), (4, 0));
    pick(&mut h, "Zamknij");
    for _ in 0..4 {
        assert!(h.state_mut().undo());
    }
    h.run();
    let c = h.state().wb.construction();
    assert_eq!(c.appearance(c.id("A").unwrap()).point_style, None);
}

#[test]
fn large_batch_is_refused_before_it_partially_changes_the_selection() {
    use geo_app_core::properties::{EditError, Field, MAX_EDIT_TARGETS};
    let script: String = (0..MAX_EDIT_TARGETS + 1)
        .map(|i| format!("P_{{{i}}} = (2, 1)\n"))
        .collect();
    let mut wb = Workbench::from_script(&script).unwrap();
    let labels: Vec<_> = wb
        .object_ids()
        .map(|id| wb.construction().label(id).to_owned())
        .collect();
    assert_eq!(
        wb.edit_property(&labels, Field::Visible, "false", None),
        Err(EditError::SelectionTooLarge)
    );
    assert!(wb.object_ids().all(|id| wb.is_visible(id)));
    assert_eq!(wb.undo_depth(), (0, 0));
    assert!(wb
        .edit_property(&labels[..MAX_EDIT_TARGETS], Field::Visible, "false", None)
        .is_ok());
    assert_eq!(wb.object_ids().filter(|&id| wb.is_visible(id)).count(), 1);
    assert_eq!(wb.undo_depth(), (1, 0));
    assert!(wb.undo());
    assert!(wb.object_ids().all(|id| wb.is_visible(id)));
}

#[test]
fn tab_order_follows_the_form_and_escape_returns_focus_to_graphics() {
    let mut h = harness("A = (2, 1)");
    open_point_properties(&mut h);
    h.get_by_label("Podpis").focus();
    h.run();
    for label in ["Pokaż obiekt", "Pokaż etykietę", "Treść etykiety"] {
        h.key_press(egui::Key::Tab);
        h.run();
        assert!(h.get_by_label(label).is_focused(), "{label}");
    }
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.state().properties.is_none());
    assert!(h.get_by_label("Widok graficzny").is_focused());
}

#[test]
fn a_shared_field_commits_once_for_two_selected_rows() {
    let mut h = harness("A = (2, 1)\nB = (4, 1)\nC = (6, 1)");
    click(&mut h, row_body(0.0));
    let pos = row_body(ROW);
    moved(&mut h, pos);
    h.input_mut()
        .events
        .push(Event::ModifiersChanged(Modifiers::COMMAND));
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::COMMAND,
        });
        h.step();
    }
    h.input_mut()
        .events
        .push(Event::ModifiersChanged(Modifiers::NONE));
    open_point_properties(&mut h);
    assert_eq!(h.state().properties.as_ref().unwrap().targets(), ["A", "B"]);
    pick(&mut h, "Pokaż obiekt");
    for (label, visible) in [("A", false), ("B", false), ("C", true)] {
        let id = h.state().wb.construction().id(label).unwrap();
        assert_eq!(h.state().wb.is_visible(id), visible);
    }
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    assert!(h.state_mut().undo());
    assert!(h
        .state()
        .wb
        .object_ids()
        .all(|id| h.state().wb.is_visible(id)));
}

#[test]
fn edited_properties_survive_native_and_archive_round_trips() {
    use geo_app_core::properties::Field::*;
    let mut wb = Workbench::from_script(
        "A = (2, 1)\nB = (4, 1)\nC = (2, 3)\ns = Segment(A, B)\na = Slider(0, 10)\nb = 2a",
    )
    .unwrap();
    let witness = include_bytes!("../../../../golden/ggb/saved/saved-propertydialog-001.xml");
    let mut classic = Workbench::default();
    classic.load_bytes(witness, "property witness").unwrap();
    let edits = [
        ("A", Color, "#DB3456"),
        ("A", PointSize, "8"),
        ("A", PointStyle, "2"),
        ("A", LabelMode, "3"),
        ("A", Caption, "Vertex"),
        ("A", LabelX, "20"),
        ("A", LabelY, "15"),
        ("A", Selectable, "false"),
        ("A", Tooltip, "3"),
        ("A", Layer, "2"),
        ("s", LineType, "2"),
        ("s", Thickness, "7"),
        ("s", LineOpacity, "50"),
        ("s", SegmentStart, "2"),
        ("s", SegmentEnd, "7"),
        ("s", SegmentDecoration, "2"),
        ("a", SliderMax, "4"),
    ];
    let a = wb.construction().id("a").unwrap();
    assert!(wb.set_number(a, 7.0));
    for (label, field, value) in edits {
        assert!(
            wb.edit_property(&[label.to_owned()], field, value, None)
                .is_ok(),
            "{field:?}"
        );
        assert_eq!(
            classic.property_value(classic.construction().id(label).unwrap(), field),
            value,
            "{field:?}"
        );
    }

    let mut native = Workbench::default();
    assert!(native.restore_text(&wb.snapshot_text()).unwrap().is_empty());
    let mut archive = Workbench::default();
    archive
        .load_bytes(&wb.export_ggb(None).unwrap(), "edited properties")
        .unwrap();
    for (label, field, value) in edits {
        for restored in [&native, &archive] {
            let id = restored.construction().id(label).unwrap();
            assert_eq!(
                restored.property_value(id, field),
                value,
                "{label} {field:?}"
            );
        }
    }
    for restored in [wb, native, archive, classic] {
        let c = restored.construction();
        assert_eq!(c.value(c.id("b").unwrap()), Value::Number(8.0));
    }
}

#[test]
fn a_dynamic_color_expression_updates_both_views_when_its_input_changes() {
    let mut h = harness("A = (2, 1)\na = 0.25");
    open_point_properties(&mut h);
    pick(&mut h, "Zaawansowane");
    enter(&mut h, "Składowa koloru 1", "2 a");
    let channel = |app: &App| {
        let id = app.wb.construction().id("A").unwrap();
        app.wb.palette().color(id).r
    };
    assert_eq!(channel(h.state()), 128);
    let mut archive = Workbench::default();
    archive
        .load_bytes(&h.state().wb.export_ggb(None).unwrap(), "dynamic color")
        .unwrap();
    let point = archive.construction().id("A").unwrap();
    assert!(!archive
        .property_value(point, geo_app_core::properties::Field::DynamicRed)
        .is_empty());

    let a = h.state().wb.construction().id("a").unwrap();
    assert!(h.state_mut().wb.set_number(a, 0.5));
    h.run();
    assert_eq!(channel(h.state()), 255);
    assert!(h.state_mut().undo());
    assert_eq!(channel(h.state()), 128);
    assert!(h.state_mut().undo());
    assert_ne!(channel(h.state()), 128);
}

#[test]
fn each_property_is_a_labelled_control_and_commits_one_step() {
    use geo_app_core::properties::{Field, Input, Tab};
    use std::collections::HashSet;

    const SCENE: &str = "A = (2, 1)\nB = (4, 1)\nC = (2, 3)\nflag = false\nbox1 = $checkBox(false, _)\n        s = Segment(A, B)\nv = Vector(A, B)\np = Polygon(A, B, C)\n        α = Angle(B, A, C)\nl = Line(A, B)\nk = Circle(A, B)\n        n = Slider(0, 10)\nT = Text(\"x^2\", (2, 1), true, true)\n        field = $inputBox(n, _)";
    let app = App::new(Workbench::from_script(SCENE).unwrap(), None);
    let mut h = Harness::builder()
        .with_size(egui::vec2(SIZE.0 + 2.0 * MARGIN, SIZE.1 + 2.0 * MARGIN))
        .with_max_steps(20)
        .build_ui_state(|ui, app| crate::app::show(app, ui), app);
    h.run();
    h.state_mut().select_tool(Tool::Image);
    click(&mut h, world(1.0, 1.0));
    h.state_mut()
        .finish_image(
            include_bytes!("../../../../tools/ggb-harness/cases/editor_tools/fixture.png").to_vec(),
        )
        .unwrap();
    h.run();
    let image = {
        let c = h.state().wb.construction();
        c.label(c.images().next().unwrap().0).to_owned()
    };
    let mut covered = HashSet::new();
    for label in [
        "A", "s", "v", "p", "α", "l", "k", "n", "T", "field", "flag", "box1", &image,
    ] {
        let id = h.state().wb.construction().id(label).unwrap();
        h.state_mut().open_properties(id);
        h.run();
        let fields = h.state().properties.as_ref().unwrap().fields.clone();
        let kind = h.state().wb.construction().kind(id);
        println!("{kind:?} {label}: {} fields", fields.len());
        for field in fields {
            if !covered.insert(field) {
                continue;
            }
            let tab = match field.tab() {
                Tab::Basic => "Podstawowe",
                Tab::Color => "Kolor",
                Tab::Style => "Styl",
                Tab::Position => "Położenie",
                Tab::Advanced => "Zaawansowane",
            };
            pick(&mut h, tab);
            let before = h.state().wb.property_value(id, field);
            let depth = h.state().wb.undo_depth().0;
            match field.input() {
                Input::Check if field == Field::Animating => {
                    h.get_by_label(field.label()).click_accesskit();
                    h.step();
                    h.state_mut().wb.set_animation_running(false);
                }
                Input::Check => pick(&mut h, field.label()),
                Input::Choice(choices) => {
                    pick(&mut h, field.label());
                    let current: usize = before.parse().unwrap();
                    let choice = choices[(current + 1) % choices.len()];
                    h.get_by_role_and_label(egui::accesskit::Role::Button, choice)
                        .click_accesskit();
                    h.run();
                }
                Input::Integer(min, max) => {
                    let current = before.parse().unwrap_or(min - 1);
                    let value = if current < max { current + 1 } else { min };
                    enter(&mut h, field.label(), &value.to_string());
                }
                Input::Number(min, max) => {
                    let current = before.parse::<f64>().unwrap();
                    let value = if current + 1.0 <= max {
                        current + 1.0
                    } else {
                        min
                    };
                    enter(&mut h, field.label(), &value.to_string());
                }
                Input::Color => enter(&mut h, field.label(), "#cc9955"),
                Input::Text => {
                    let value = match field {
                        Field::Condition => "flag",
                        Field::SliderMin => "-2",
                        Field::SliderMax => "3",
                        Field::Anchor => "A",
                        Field::DynamicRed
                        | Field::DynamicGreen
                        | Field::DynamicBlue
                        | Field::DynamicAlpha => "0.7",
                        _ => "Vertex",
                    };
                    enter(&mut h, field.label(), value);
                }
            }
            assert_eq!(h.state().wb.undo_depth().0, depth + 1, "{kind:?} {field:?}");
            assert_ne!(
                h.state().wb.property_value(id, field),
                before,
                "{kind:?} {field:?}"
            );
            assert!(h.state_mut().undo(), "{field:?}");
            h.run();
            assert_eq!(
                h.state().wb.property_value(id, field),
                before,
                "{kind:?} {field:?}"
            );
        }
        pick(&mut h, "Zamknij");
    }
    let missing: Vec<_> = Field::ALL
        .iter()
        .filter(|&&field| !covered.contains(&field))
        .collect();
    assert!(missing.is_empty(), "fields without a UI edit: {missing:?}");
    println!("{} editable properties", covered.len());
}

#[test]
fn linked_number_rounding_changes_the_input_box_preview_and_undo_restores_it() {
    let mut h = harness("a = 1.234567\nfield = $inputBox(a, _)");
    let find = |app: &App| app.wb.construction().id("field").unwrap();
    let id = find(h.state());
    h.state_mut().open_properties(id);
    h.run();
    pick(&mut h, "Zaawansowane");
    enter(&mut h, "Miejsca po przecinku", "3");
    let preview = |app: &App| app.wb.construction().input_box_text(find(app)).into_owned();
    assert_eq!(preview(h.state()), "1.235");
    enter(&mut h, "Cyfry znaczące", "2");
    assert_eq!(preview(h.state()), "1.2");
    assert!(h.state_mut().undo());
    assert_eq!(preview(h.state()), "1.235");
    assert!(h.state_mut().undo());
    assert_eq!(preview(h.state()), "1.234567");
}
