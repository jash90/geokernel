use super::*;
use crate::geom::ToEgui;

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
    pick(&mut h, "Pokaż obiekt");
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
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert_eq!(h.state().wb.undo_depth(), (0, 1));
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

fn rendered_views(h: &mut Harness<'_, App>) -> [Vec<egui::epaint::ClippedShape>; 2] {
    h.input_mut().events.push(Event::PointerGone);
    h.run();
    let graphics = h.state().view.unwrap().0.egui();
    let algebra = h.state().regions.algebra.egui();
    let shapes = &h.output().shapes;
    [
        shapes
            .iter()
            .filter(|shape| shape.clip_rect == graphics)
            .cloned()
            .collect(),
        shapes
            .iter()
            .filter(|shape| algebra.contains_rect(shape.clip_rect))
            .cloned()
            .collect(),
    ]
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
        walk_tab_order(&mut h, id, &fields);
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
            let rendered_before = rendered_views(&mut h);
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
            let rendered_after = rendered_views(&mut h);
            assert!(
                rendered_after.iter().all(|view| !view.is_empty()),
                "{field:?}"
            );
            // Drawing-only edits leave the ordinary algebra representation stable.
            // Interaction settings (such as fixed and tooltip) can leave both views
            // unchanged before the next drag or hover.
            let canvas_changes = matches!(
                field,
                Field::Visible
                    | Field::ShowLabel
                    | Field::LabelMode
                    | Field::Color
                    | Field::PointSize
                    | Field::PointStyle
                    | Field::LabelX
                    | Field::LabelY
                    | Field::Condition
                    | Field::DynamicRed
                    | Field::DynamicGreen
                    | Field::DynamicBlue
                    | Field::Layer
                    | Field::Thickness
                    | Field::LineType
                    | Field::LineOpacity
                    | Field::SegmentStart
                    | Field::SegmentEnd
                    | Field::SegmentDecoration
                    | Field::VectorArrow
                    | Field::FillOpacity
                    | Field::DynamicAlpha
                    | Field::RightAngle
                    | Field::AngleRadius
                    | Field::Animating
                    | Field::SliderTrackColor
                    | Field::SliderKnob
                    | Field::PositionX
                    | Field::PositionY
                    | Field::SliderMin
                    | Field::SliderWidth
                    | Field::SliderHorizontal
                    | Field::TextBackground
                    | Field::TextBold
                    | Field::TextItalic
                    | Field::TextLatex
                    | Field::InputLength
                    | Field::ImageBackground
            );
            let algebra_changes = matches!(
                field,
                Field::Visible
                    | Field::Auxiliary
                    | Field::Color
                    | Field::Condition
                    | Field::DynamicRed
                    | Field::DynamicGreen
                    | Field::DynamicBlue
            );
            assert_eq!(
                rendered_before[0] != rendered_after[0],
                canvas_changes,
                "{field:?} graphics"
            );
            assert_eq!(
                rendered_before[1] != rendered_after[1],
                algebra_changes,
                "{field:?} algebra"
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

#[test]
fn dash_dot_segment_paints_after_the_menu_edit() {
    let mut h = harness("A = (2, 1)\nB = (4, 1)\ns = Segment(A, B)");
    let s = h.state().wb.construction().id("s").unwrap();
    h.state_mut().open_properties(s);
    h.run();
    pick(&mut h, "Styl");
    pick(&mut h, "Styl linii");
    pick(&mut h, "Kreski i kropki");
    assert_eq!(
        h.state().wb.construction().appearance(s).line_type,
        Some(30)
    );
}

#[test]
fn edited_slider_increment_overrides_command_when_dragging_and_saving() {
    use geo_app_core::properties::Field;
    let mut wb = Workbench::from_script("a = Slider(0,10,1)").unwrap();
    let a = wb.construction().id("a").unwrap();
    wb.edit_property(&["a".into()], Field::SliderStep, "2", None)
        .unwrap();
    let dragged = wb
        .construction()
        .slider_pointer_value(a, (30.0, 100.0), false);
    let mut reopened = Workbench::default();
    reopened
        .load_bytes(&wb.export_ggb(None).unwrap(), "archive")
        .unwrap();
    let mut reference = Workbench::default();
    reference
        .load_bytes(
            include_bytes!("../../../../golden/ggb/saved/saved-property-slider-step-001.xml"),
            "reference",
        )
        .unwrap();
    let step =
        |wb: &Workbench| wb.property_value(wb.construction().id("a").unwrap(), Field::SliderStep);
    assert_eq!((dragged, step(&reopened)), (Some(4.0), step(&reference)));
    let mut native = Workbench::default();
    assert!(native.restore_text(&wb.snapshot_text()).unwrap().is_empty());
    assert_eq!(step(&native), step(&reference));
    assert!(wb.undo());
    assert_eq!(
        wb.construction()
            .slider_pointer_value(a, (30.0, 100.0), false),
        Some(3.0)
    );
}

fn type_uncommitted(h: &mut Harness<'_, App>, label: &str, text: &str) {
    h.get_by_label(label).focus();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::A);
    h.run();
    h.input_mut().events.push(Event::Text(text.into()));
    h.run();
}

#[test]
fn caption_blur_and_visibility_click_both_commit() {
    let mut h = harness("A = (2, 1)");
    open_point_properties(&mut h);
    type_uncommitted(&mut h, "Podpis", "Vertex");
    pick(&mut h, "Pokaż obiekt");
    let c = h.state().wb.construction();
    let a = c.id("A").unwrap();
    assert_eq!(c.appearance(a).caption.as_deref(), Some("Vertex"));
    assert!(!h.state().wb.is_visible(a));
    assert_eq!(h.state().wb.undo_depth(), (2, 0));
}

#[test]
fn moving_focus_to_an_earlier_field_commits_the_later_draft() {
    let mut h = harness("A = (2, 1)");
    open_point_properties(&mut h);
    pick(&mut h, "Położenie");
    type_uncommitted(&mut h, "Przesunięcie etykiety Y (px)", "15");
    h.get_by_label("Przesunięcie etykiety X (px)").focus();
    h.run();
    let a = h.state().wb.construction().id("A").unwrap();
    assert_eq!(h.state().wb.label_offset(a), Some((0.0, 15.0)));
}

#[test]
fn choosing_the_first_points_shape_updates_the_mixed_selection() {
    use geo_app_core::properties::Field;
    let mut h = harness("A = (2, 1)\nB = (4, 1)");
    let (a, b) = {
        let wb = &mut h.state_mut().wb;
        wb.edit_property(&["B".into()], Field::PointStyle, "2", None)
            .unwrap();
        (
            wb.construction().id("A").unwrap(),
            wb.construction().id("B").unwrap(),
        )
    };
    h.state_mut().properties = geo_app_core::properties::Dialog::open(&h.state().wb, [a, b]);
    h.run();
    pick(&mut h, "Styl");
    pick(&mut h, "Kształt punktu");
    pick(&mut h, "Kropka");
    assert_eq!(h.state().wb.property_value(b, Field::PointStyle), "0");
    assert_eq!(h.state().wb.undo_depth(), (2, 0));
    assert!(h.state_mut().undo());
    assert_eq!(h.state().wb.property_value(b, Field::PointStyle), "2");
}

#[test]
fn dragging_a_label_replaces_the_dialog_offset() {
    let mut h = harness("A = (2, 1)");
    open_point_properties(&mut h);
    pick(&mut h, "Położenie");
    enter(&mut h, "Przesunięcie etykiety X (px)", "20");
    enter(&mut h, "Przesunięcie etykiety Y (px)", "15");
    pick(&mut h, "Zamknij");
    let a = h.state().wb.construction().id("A").unwrap();
    let at = h.state().view.unwrap().0.egui().min
        + h.state()
            .label_rects
            .borrow()
            .iter()
            .find(|(id, _)| *id == a)
            .unwrap()
            .1
            .egui()
            .center()
            .to_vec2();
    drag(&mut h, at, at + egui::vec2(40.0, 30.0));
    assert_eq!(h.state().wb.label_offset(a), Some((60.0, 45.0)));
    assert_eq!(h.state().wb.undo_depth(), (2, 0));
    let mut reopened = Workbench::default();
    reopened
        .load_bytes(&h.state().wb.export_ggb(None).unwrap(), "archive")
        .unwrap();
    let mut reference = Workbench::default();
    reference
        .load_bytes(
            include_bytes!("../../../../golden/ggb/saved/saved-property-equation-001.xml"),
            "reference",
        )
        .unwrap();
    let label_offset = |wb: &Workbench| wb.label_offset(wb.construction().id("A").unwrap());
    assert_eq!(label_offset(&reopened), label_offset(&reference));
}

#[test]
fn point_size_batch_remains_available_with_unrelated_protocol_objects() {
    use geo_app_core::properties::{Field, MAX_EDIT_TARGETS};
    let points: Vec<_> = (0..MAX_EDIT_TARGETS)
        .map(|i| format!("P_{{{i}}}"))
        .collect();
    let script: String = points
        .iter()
        .map(|label| format!("{label} = (2, 1)\n"))
        .chain((0..1000).map(|i| format!("c_{{{i}}} = ConstructionStep()\n")))
        .collect();
    let mut wb = Workbench::from_script(&script).unwrap();
    wb.edit_property(&points, Field::PointSize, "8", None)
        .unwrap();
    assert_eq!(wb.undo_depth(), (1, 0));
    for label in points {
        assert_eq!(
            wb.construction()
                .appearance(wb.construction().id(&label).unwrap())
                .point_size,
            Some(8)
        );
    }
}

#[test]
fn discarded_color_formulas_do_not_grow_the_construction() {
    use geo_app_core::properties::Field;
    let mut wb = Workbench::from_script("A = (2, 1)\na = 0.2").unwrap();
    let labels = ["A".into()];
    let len = wb.construction().len();
    let a = wb.construction().id("a").unwrap();
    let point = wb.construction().id("A").unwrap();
    for source in ["2 a", "3 a", "2 a", "3 a", ""] {
        wb.edit_property(&labels, Field::DynamicRed, source, None)
            .unwrap();
    }
    assert!(wb.set_number(a, 0.5));
    assert!(wb.construction().dynamic_color(point, [0; 3]).is_none());
    assert_eq!(wb.construction().len(), len);
}

#[test]
fn oversized_propagating_batch_is_refused_below_the_selection_limit() {
    use geo_app_core::properties::{EditError, Field};
    let points: Vec<_> = (0..32).map(|i| format!("P_{{{i}}}")).collect();
    let script: String = points
        .iter()
        .map(|label| format!("{label} = (2, 1)\n"))
        .chain((0..1000).map(|i| format!("c_{{{i}}} = ConstructionStep()\n")))
        .collect();
    let mut wb = Workbench::from_script(&script).unwrap();
    assert_eq!(
        wb.edit_property(&points, Field::DynamicRed, "2 c_{0}", None),
        Err(EditError::SelectionTooLarge)
    );
    assert_eq!(wb.undo_depth(), (0, 0));
}

fn walk_tab_order(
    h: &mut Harness<'_, App>,
    id: geo_kernel::Id,
    fields: &[geo_app_core::properties::Field],
) {
    use geo_app_core::properties::Tab;
    for (tab, label) in [
        (Tab::Basic, "Podstawowe"),
        (Tab::Color, "Kolor"),
        (Tab::Style, "Styl"),
        (Tab::Position, "Położenie"),
        (Tab::Advanced, "Zaawansowane"),
    ] {
        let labels: Vec<_> = fields
            .iter()
            .filter(|f| f.tab() == tab)
            .map(|f| f.label_for(h.state().wb.construction(), id))
            .collect();
        let Some(first) = labels.first() else {
            continue;
        };
        pick(h, label);
        h.get_by_label(first).focus();
        h.run();
        assert!(h.get_by_label(first).is_focused(), "{tab:?} {first}");
        for label in &labels[1..] {
            h.key_press(egui::Key::Tab);
            h.run();
            assert!(h.get_by_label(label).is_focused(), "{tab:?} {label}");
        }
        for label in labels[..labels.len() - 1].iter().rev() {
            h.key_press_modifiers(Modifiers::SHIFT, egui::Key::Tab);
            h.run();
            assert!(h.get_by_label(label).is_focused(), "{tab:?} {label}");
        }
    }
}

#[test]
fn slider_thickness_uses_pixels_and_text_size_accepts_reference_extremes() {
    use geo_app_core::properties::Field;
    let mut h = harness("a = Slider(0,10,1)\nT = Text(\"Text\", (2, 1))");
    let a = h.state().wb.construction().id("a").unwrap();
    h.state_mut().open_properties(a);
    h.run();
    pick(&mut h, "Styl");
    enter(&mut h, "Grubość linii (px)", "2");
    let witness = include_bytes!("../../../../golden/ggb/saved/saved-property-slider-step-001.xml");
    let mut reference = Workbench::default();
    reference.load_bytes(witness, "reference").unwrap();
    let expected = reference
        .construction()
        .drawing_style(reference.construction().id("a").unwrap())
        .line_thickness;
    assert_eq!(
        h.state().wb.construction().drawing_style(a).line_thickness,
        expected
    );
    pick(&mut h, "Zamknij");

    let t = h.state().wb.construction().id("T").unwrap();
    h.state_mut().open_properties(t);
    h.run();
    pick(&mut h, "Styl");
    for size in ["1", "10000"] {
        enter(&mut h, "Rozmiar tekstu (%)", size);
        assert_eq!(h.state().wb.property_value(t, Field::TextSize), size);
    }
}

#[test]
fn circle_parametric_form_is_available_and_matches_the_saved_reference() {
    use geo_app_core::properties::Field;
    let script = "A = (2, 1)\nB = (4, 1)\nk = Circle(A, B)\nv = Vector(A, B)";
    let app = App::new(Workbench::from_script(script).unwrap(), None);
    let mut h = Harness::builder()
        .with_size(egui::vec2(SIZE.0 + 2.0 * MARGIN, SIZE.1 + 2.0 * MARGIN))
        .with_max_steps(20)
        .build_ui_state(|ui, app| crate::app::show(app, ui), app);
    h.run();
    let k = h.state().wb.construction().id("k").unwrap();
    h.state_mut().open_properties(k);
    h.run();
    pick(&mut h, "Zaawansowane");
    pick(&mut h, Field::ConicEquation.label());
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Parametryczna")
        .click_accesskit();
    h.run_steps(8);
    let mut reference = Workbench::default();
    reference
        .load_bytes(
            include_bytes!("../../../../golden/ggb/saved/saved-property-equation-001.xml"),
            "reference",
        )
        .unwrap();
    let reference_k = reference.construction().id("k").unwrap();
    let c = h.state().wb.construction();
    assert_eq!(
        c.equation_form(k),
        reference.construction().equation_form(reference_k)
    );
    assert_eq!(
        c.formatted_equation(k),
        reference.construction().formatted_equation(reference_k)
    );
    assert!(h.state_mut().undo());
}

#[test]
fn a_degree_angle_label_uses_the_algebra_units() {
    use geo_app_core::properties::Field;
    let mut h = harness("A = (0, 0)\nB = (2, 0)\nC = (0, 2)\na = Angle(B, A, C)");
    let a = h.state().wb.construction().id("a").unwrap();
    h.state_mut().open_properties(a);
    h.run();
    pick(&mut h, Field::LabelMode.label());
    pick(&mut h, "Wartość");
    pick(&mut h, "Zamknij");
    let [graphics, algebra] = rendered_views(&mut h);
    for view in [graphics, algebra] {
        assert!(view.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("90°"))));
    }
}

#[test]
fn properties_merge_imports_five_presentation_fields() {
    use geo_app_core::properties::Field;
    let mut h = harness("");
    let witness = include_bytes!("../../../../golden/ggb/saved/saved-propertydialog-001.xml");
    h.state_mut()
        .wb
        .load_bytes(witness, "style witness")
        .unwrap();
    let a = h.state().wb.construction().id("A").unwrap();
    let s = h.state().wb.construction().id("s").unwrap();
    h.state_mut().open_properties(a);
    h.run();
    h.get_by_label(Field::LabelMode.label());
    assert_eq!(h.state().wb.property_value(a, Field::LabelMode), "3");
    h.get_by_label("Kolor").click_accesskit();
    h.run();
    h.get_by_label(Field::Color.label());
    assert_eq!(h.state().wb.property_value(a, Field::Color), "#DB3456");
    h.get_by_label("Zamknij").click_accesskit();
    h.run();
    h.state_mut().open_properties(s);
    h.run();
    h.get_by_label("Styl").click_accesskit();
    h.run();
    for (field, expected) in [(Field::Thickness, "7"), (Field::LineType, "2")] {
        h.get_by_label(field.label());
        assert_eq!(h.state().wb.property_value(s, field), expected);
    }
    h.get_by_label("Zamknij").click_accesskit();
    h.run();
    let [graphics, _] = rendered_views(&mut h);
    let red = egui::Color32::from_rgb(219, 52, 86);
    assert!(graphics.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text() == "Vertex"
            && text.fallback_color == red)));
    assert!(
        graphics
            .iter()
            .filter(|shape| matches!(&shape.shape,
                egui::Shape::LineSegment { points, stroke }
                    if stroke.width == 3.5 && points[0].distance(points[1]) <= 11.01
            ))
            .count()
            > 1,
        "the thick segment must paint as multiple dashes"
    );

    let witness = include_bytes!("../../../../golden/ggb/saved/saved-styles-006.xml");
    h.state_mut()
        .wb
        .load_bytes(witness, "font witness")
        .unwrap();
    let text = h.state().wb.construction().id("value").unwrap();
    h.state_mut().open_properties(text);
    h.run();
    h.get_by_label("Styl").click_accesskit();
    h.run();
    h.get_by_label(Field::TextSize.label());
    assert_eq!(h.state().wb.property_value(text, Field::TextSize), "70");
    assert_eq!(h.state().wb.property_value(text, Field::TextItalic), "true");
    h.get_by_label("Zamknij").click_accesskit();
    h.run();
    let [graphics, _] = rendered_views(&mut h);
    assert!(graphics.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text().starts_with("n = ")
            && text.galley.job.sections.iter().all(|part|
                part.format.italics && part.format.font_id.size == 11.0)
    )));
}

#[test]
fn properties_merge_caption_and_value_round_trips_with_the_reference_code() {
    use geo_app_core::properties::Field;
    let witness =
        include_bytes!("../../../../golden/ggb/saved/saved-property-caption-value-001.xml");
    let mut reference = Workbench::default();
    reference.load_bytes(witness, "caption witness").unwrap();
    let styles = reference.export_presentation(None).object_styles;
    let mut h = harness("A = (2, 1)\na = Slider(0, 10)");
    for (label, caption) in [("A", "Vertex"), ("a", "Amount")] {
        let id = h.state().wb.construction().id(label).unwrap();
        h.state_mut().open_properties(id);
        h.run();
        enter(&mut h, Field::Caption.label(), caption);
        pick(&mut h, Field::LabelMode.label());
        pick(&mut h, "Podpis i wartość");
        pick(&mut h, "Zamknij");
    }
    let archive = h.state().wb.export_ggb(None).unwrap();
    let mut reopened = Workbench::default();
    reopened.load_bytes(&archive, "edited archive").unwrap();
    let saved = reopened.export_presentation(None);
    for label in ["A", "a"] {
        assert_eq!(
            saved.object_styles[label].label_mode,
            styles[label].label_mode
        );
    }

    let mut h = harness("");
    h.state_mut()
        .wb
        .load_bytes(witness, "caption witness")
        .unwrap();
    let [graphics, _] = rendered_views(&mut h);
    for prefix in ["Vertex = ", "Amount = 4"] {
        assert!(
            graphics.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.text().starts_with(prefix)
            )),
            "{prefix}"
        );
    }

    for (code, choice) in [(4, "0"), (5, "0"), (6, "1"), (7, "2"), (8, "3")] {
        let xml = format!(
            r#"<geogebra format="5.0"><construction>
            <element type="point" label="A"><coords x="2" y="1" z="1"/>
            <labelMode val="{code}"/><caption val="Vertex"/></element>
            </construction></geogebra>"#
        );
        reopened.load_bytes(xml.as_bytes(), "label mode").unwrap();
        let id = reopened.construction().id("A").unwrap();
        assert_eq!(
            reopened.property_value(id, Field::LabelMode),
            choice,
            "{code}"
        );
    }
}

#[test]
fn properties_merge_clears_multi_selection_when_primary_selection_changes() {
    use geo_app_core::tools::Outcome;

    let select_pair = |app: &mut App| {
        let c = app.wb.construction();
        let (a, b) = (c.id("A").unwrap(), c.id("B").unwrap());
        app.select_algebra_object(a, false);
        app.select_algebra_object(b, true);
        assert_eq!(app.property_selection.len(), 2);
        a
    };
    for (name, change) in [
        (
            "leave tool",
            (|app: &mut App| app.leave_tool()) as fn(&mut App),
        ),
        ("submit input", |app: &mut App| {
            app.input = "C = (1, 1)".into();
            app.submit_input();
        }),
        ("tool selection", |app: &mut App| {
            app.apply_outcome(Outcome::SelectionChanged);
        }),
        ("pick existing point", |app: &mut App| {
            app.apply_outcome(Outcome::Select("A".into()));
        }),
        ("finish construction", |app: &mut App| {
            app.apply_outcome(Outcome::Run(vec!["D = (1, 1)".into()]));
        }),
        ("place point", |app: &mut App| {
            app.apply_outcome(Outcome::Place {
                line: "E = (1, 1)".into(),
                at: None,
                then: Box::new(Outcome::Pending),
            });
        }),
    ] {
        let mut h = harness("A = (2, 1)\nB = (4, 1)");
        let app = h.state_mut();
        let a = select_pair(app);
        change(app);
        assert!(app.property_selection.is_empty(), "{name}");
        app.open_properties(a);
        assert_eq!(app.properties.take().unwrap().targets(), ["A"], "{name}");
    }
    let mut h = harness("A = (2, 1)\nB = (4, 1)");
    let app = h.state_mut();
    select_pair(app);
    app.input = "@".into();
    app.submit_input();
    assert_eq!(app.property_selection.len(), 2);
}
