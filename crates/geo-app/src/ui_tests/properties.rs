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
    assert!(h.state().wb.current_rows().iter().any(|row| row.label == "A"));
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    assert!(h.state_mut().undo());
    h.run();
    assert!(shown(h.state()));
    assert!(h.state().wb.current_rows().iter().any(|row| row.label == "A"));
}

fn pick(h: &mut Harness<'_, App>, label: &str) {
    h.get_by_label(label).click();
    h.run();
}

fn enter(h: &mut Harness<'_, App>, label: &str, value: &str) {
    pick(h, label);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
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
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(4.0));
    assert_eq!(c.value(c.id("b").unwrap()), Value::Number(8.0));
    assert!(h.state_mut().undo());
    let c = h.state().wb.construction();
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(7.0));
    assert_eq!(c.value(c.id("b").unwrap()), Value::Number(14.0));
}
