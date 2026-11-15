//! Every tool, driven by real clicks in the graphics view.

use super::*;

/// A scene in which every kind of object can be clicked away from the others.
const TOOL_SCENE: &str = "A = (-6, -3)\nB = (-2, -3)\nC = (-4, 1)\nf = Line(A, B)\n\
                          D = (4, -2)\nE = (6, -2)\nk = Circle(D, E)\n\
                          G = (-7, 3)\nH = (-3, 4)\nu = Vector(G, H)\n";

#[test]
fn every_tool_works_with_real_clicks() {
    let (a, b, c) = ((-6.0, -3.0), (-2.0, -3.0), (-4.0, 1.0));
    let (f, k, u, empty) = ((0.0, -3.0), (4.0, 0.0), (-5.0, 3.5), (0.0, 2.0));
    let plan = |tool: Tool| -> Vec<(f64, f64)> {
        match tool {
            Tool::Point => vec![empty],
            Tool::Intersect => vec![f, k],
            Tool::Midpoint => vec![a, b],
            Tool::Perpendicular | Tool::Parallel | Tool::ReflectLine => vec![c, f],
            Tool::AngleBisector | Tool::Angle => vec![a, c, b],
            Tool::Tangents | Tool::Polar | Tool::ReflectCircle => vec![c, k],
            Tool::CircleRadius => vec![a],
            Tool::Circle3 => vec![a, b, c],
            Tool::ReflectPoint | Tool::Rotate | Tool::Dilate => vec![c, a],
            Tool::Translate => vec![c, u],
            _ => vec![a, c],
        }
    };
    let mut failures = Vec::new();
    for tool in Tool::all().filter(|t| !t.is_direct()) {
        let mut h = harness(TOOL_SCENE);
        let before = h.state().wb.construction().len();
        h.state_mut().select_tool(tool);
        for (x, y) in plan(tool) {
            click(&mut h, world(x, y));
        }
        if h.state().tool.asking().is_some() {
            // The number dialog: take the value it offers.
            h.key_press(egui::Key::Enter);
            h.run();
        }
        let app = h.state();
        let after = app.wb.construction().len();
        if after <= before || app.tool_error.is_some() {
            failures.push(format!(
                "{tool:?}: {before} -> {after}, {:?}",
                app.tool_error
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_direct_tools_act_on_one_click() {
    let mut h = harness(TOOL_SCENE);
    h.state_mut().select_tool(Tool::Delete);
    click(&mut h, world(-4.0, 1.0));
    assert!(h.state().wb.construction().id("C").is_none());
    h.state_mut().select_tool(Tool::ShowHide);
    click(&mut h, world(-6.0, -3.0));
    let a = h.state().wb.construction().id("A").unwrap();
    assert!(!h.state().wb.is_visible(a));
    let scale = h.state().camera.unwrap().scale;
    h.state_mut().select_tool(Tool::ZoomIn);
    click(&mut h, world(5.0, 5.0));
    let zoomed = h.state().camera.unwrap().scale;
    assert!(zoomed > scale);
    h.state_mut().select_tool(Tool::ZoomOut);
    click(&mut h, world(1.0, 1.0));
    assert!(h.state().camera.unwrap().scale < zoomed);
}
