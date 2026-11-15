//! The window chrome: toolbar, drop-downs, menus, style bar and keyboard.

use super::*;

#[test]
fn a_toolbar_button_selects_the_tool_its_group_shows() {
    let mut h = harness("A = (0, 0)\n");
    // The first button is "move", the second "point".
    click(&mut h, group_button(1));
    assert_eq!(h.state().tool.tool(), Tool::Point);
    // The drop-down opened by the click is closed by the click in the view.
    click(&mut h, world(3.0, 3.0));
    assert_eq!(labels(&h), ["A", "B"]);
    click(&mut h, group_button(0));
    assert_eq!(h.state().tool.tool(), Tool::Move);
}

#[test]
fn a_group_drop_down_picks_a_tool_and_the_button_shows_it_after() {
    let mut h = harness("A = (0, 0)\n");
    click(&mut h, group_button(2));
    assert_eq!(h.state().tool.tool(), Tool::Line);
    h.get_by_label("Półprosta").click();
    h.run();
    assert_eq!(h.state().tool.tool(), Tool::Ray);
    assert_eq!(h.state().group_tools[2], Tool::Ray);
    assert!(h.query_by_label("Odcinek").is_none(), "the list closed");
    // Every group's list names every tool of the group.
    for (i, group) in GROUPS.iter().enumerate().filter(|(_, g)| g.len() > 1) {
        h.state_mut().select_tool(Tool::Move);
        click(&mut h, group_button(i));
        for tool in group.iter().skip(1) {
            assert!(
                h.query_all_by_label(tool.caption()).next().is_some(),
                "{tool:?} is missing in group {i}"
            );
        }
        h.key_press(egui::Key::Escape);
        h.run();
    }
}

#[test]
fn the_input_row_brings_up_the_keyboard_and_its_keys_type() {
    let mut h = harness("A = (0, 0)\n");
    assert!(!h.state().keyboard.open);
    click(&mut h, row_body(ROW));
    assert!(h.state().keyboard.open, "focus on the input row opens it");
    h.get_by_label("ABC").click();
    h.run();
    assert_eq!(h.state().keyboard.tab, crate::keyboard::Tab::Letters);
    h.get_by_label("123").click();
    h.run();
    for key in ["(", "1", ",", "2", ")"] {
        h.get_by_label(key).click();
        h.run();
    }
    assert_eq!(h.state().input, "(1,2)");
    h.get_by_label("Zatwierdź").click();
    h.run();
    assert_eq!(labels(&h), ["A", "B"]);
    assert!(h.state().input.is_empty());
    // Escape leaves the field and puts the keyboard away.
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!h.state().keyboard.open);
}

#[test]
fn the_style_bar_switches_grid_and_axes() {
    let mut h = harness("A = (0, 0)\n");
    h.get_by_label("Pasek stylu").click();
    h.run();
    assert!(h.state().style_bar);
    h.get_by_label("Siatka").click();
    h.run();
    assert!(!h.state().decor.grid && h.state().decor.axes);
    h.get_by_label("Osie").click();
    h.run();
    assert!(!h.state().decor.grid && !h.state().decor.axes);
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!h.state().style_bar);
    assert_eq!(labels(&h), ["A"]);
}

#[test]
fn the_main_menu_switches_grid_and_axes() {
    let mut h = harness("A = (0, 0)\n");
    h.get_by_label("Menu").click();
    h.run();
    h.get_by_label("Siatka").click();
    h.run();
    assert!(!h.state().decor.grid && h.state().decor.axes);
    // The menu closes after a choice; open it again.
    h.get_by_label("Menu").click();
    h.run();
    h.get_by_label("Osie").click();
    h.run();
    assert!(!h.state().decor.grid && !h.state().decor.axes);
    assert_eq!(labels(&h), ["A"]);
}

#[test]
fn the_toolbar_undo_and_redo_buttons_work() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().tool.set_tool(Tool::Point);
    click(&mut h, world(2.0, 2.0));
    assert_eq!(labels(&h), ["A", "B"]);
    h.get_by_label("Cofnij").click();
    h.run();
    assert_eq!(labels(&h), ["A"]);
    h.get_by_label("Ponów").click();
    h.run();
    assert_eq!(labels(&h), ["A", "B"]);
}

#[test]
fn a_click_on_a_round_view_button_does_not_place_a_point() {
    let mut h = harness("A = (0, 0)\n");
    h.state_mut().tool.set_tool(Tool::Point);
    let scale = h.state().camera.unwrap().scale;
    // The zoom-in button sits in the lower right corner of the view.
    click(&mut h, at(SIZE.0 - 35.0, SIZE.1 - 33.0 - 88.0));
    assert_eq!(labels(&h), ["A"]);
    assert!(h.state().camera.unwrap().scale > scale);
}
