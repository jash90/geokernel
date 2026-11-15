//! Every tool, driven by real clicks in the graphics view.

use super::*;
use geo_kernel::kernel::Kind;

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
            Tool::Polygon => vec![a, b, c, a],
            Tool::RegularPolygon => vec![a, b],
            _ => vec![a, c],
        }
    };
    // The object each tool makes: label, kind and definition (its
    // parents by label).
    let expected = |tool: Tool| -> (&str, Kind, &str) {
        match tool {
            Tool::Point => ("F", Kind::Point, "(0, 2)"),
            Tool::Intersect => ("F", Kind::Point, "Intersect(f, k)"),
            Tool::Midpoint => ("F", Kind::Point, "Midpoint(A, B)"),
            Tool::Line => ("g", Kind::Line, "Line(A, C)"),
            Tool::Segment => ("g", Kind::Segment, "Segment(A, C)"),
            Tool::Ray => ("g", Kind::Ray, "Ray(A, C)"),
            Tool::Vector => ("v", Kind::Vector, "Vector(A, C)"),
            Tool::Perpendicular => ("g", Kind::Line, "Perpendicular(C, f)"),
            Tool::Parallel => ("g", Kind::Line, "Parallel(C, f)"),
            Tool::Bisector => ("g", Kind::Line, "PerpendicularBisector(A, C)"),
            Tool::AngleBisector => ("g", Kind::Line, "AngleBisector(A, C, B)"),
            Tool::Tangents => ("g_{1}", Kind::Line, "Tangent(C, k)"),
            Tool::Polar => ("g", Kind::Line, "Polar(C, k)"),
            Tool::Polygon => ("t1", Kind::Polygon, "Polygon(A, B, C)"),
            Tool::RegularPolygon => ("q1", Kind::Polygon, "Polygon(A, B, a)"),
            Tool::Circle => ("c", Kind::Circle, "Circle(A, C)"),
            Tool::CircleRadius => ("c", Kind::Circle, "Circle(A, a)"),
            Tool::Circle3 => ("c", Kind::Circle, "Circle(A, B, C)"),
            Tool::Angle => ("α", Kind::Number, "Angle(A, C, B)"),
            Tool::Distance => ("a", Kind::Number, "Distance(A, C)"),
            Tool::ReflectLine => ("C'", Kind::Point, "Reflect(C, f)"),
            Tool::ReflectPoint => ("C'", Kind::Point, "Reflect(C, A)"),
            Tool::ReflectCircle => ("C'", Kind::Point, "Reflect(C, k)"),
            Tool::Rotate => ("C'", Kind::Point, "Rotate(C, α, A)"),
            Tool::Translate => ("C'", Kind::Point, "Translate(C, u)"),
            Tool::Dilate => ("C'", Kind::Point, "Dilate(C, a, A)"),
            other => panic!("{other:?} has no expectation"),
        }
    };
    let mut failures = Vec::new();
    for tool in Tool::all().filter(|t| !t.is_direct()) {
        let mut h = harness(TOOL_SCENE);
        h.state_mut().select_tool(tool);
        let clicks = plan(tool);
        let (last, first) = clicks.split_last().unwrap();
        for &(x, y) in first {
            click(&mut h, world(x, y));
        }
        let before = labels(&h);
        click(&mut h, world(last.0, last.1));
        if h.state().tool.asking().is_some() {
            // The number dialog: take the value it offers.
            h.key_press(egui::Key::Enter);
            h.run();
        }
        let after = labels(&h);
        let (label, kind, rhs) = expected(tool);
        let app = h.state();
        let c = app.wb.construction();
        let made = c.id(label).map(|id| (c.kind(id), app.wb.definition(label)));
        if made != Some((kind, Some(rhs))) {
            failures.push(format!(
                "{tool:?}: {label} is {made:?}, not {kind:?} = {rhs}"
            ));
        }
        if !app.tool.picks().is_empty() || app.tool_error.is_some() {
            failures.push(format!(
                "{tool:?}: picks {:?}, error {:?}",
                app.tool.picks(),
                app.tool_error
            ));
        }
        // One undo takes back exactly the final object, redo brings the
        // same labels back.
        undo(&mut h);
        if labels(&h) != before {
            failures.push(format!(
                "{tool:?}: undo left {:?}, not {before:?}",
                labels(&h)
            ));
        }
        redo(&mut h);
        if labels(&h) != after {
            failures.push(format!(
                "{tool:?}: redo gave {:?}, not {after:?}",
                labels(&h)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The scene, then `clicks` with `tool`.
fn with_clicks(tool: Tool, clicks: &[(f64, f64)]) -> Harness<'static, App> {
    let mut h = harness(TOOL_SCENE);
    h.state_mut().select_tool(tool);
    for &(x, y) in clicks {
        click(&mut h, world(x, y));
    }
    h
}

fn scene_labels() -> Vec<String> {
    labels(&harness(TOOL_SCENE))
}

#[test]
fn cancelling_the_number_dialog_keeps_the_new_point_and_makes_nothing_else() {
    // Circle with radius on empty space: the centre is made at once.
    let mut h = with_clicks(Tool::CircleRadius, &[(0.0, 2.0)]);
    assert!(h.state().tool.asking().is_some());
    h.key_press(egui::Key::Escape);
    h.run();
    let mut expected = scene_labels();
    expected.push("F".into());
    assert_eq!(labels(&h), expected);
    let app = h.state();
    assert!(app.tool.asking().is_none() && app.tool.picks().is_empty());
    assert_eq!(app.wb.definition("F"), Some("(0, 2)"));
}

#[test]
fn undo_and_redo_after_a_regular_polygon_and_a_circle_with_radius() {
    for (tool, clicks, made) in [
        (
            Tool::RegularPolygon,
            vec![(-6.0, -3.0), (-2.0, -3.0)],
            vec!["a", "q1", "F", "I", "g", "h", "i", "j"],
        ),
        (Tool::CircleRadius, vec![(-6.0, -3.0)], vec!["a", "c"]),
    ] {
        let mut h = with_clicks(tool, &clicks);
        h.key_press(egui::Key::Enter);
        h.run();
        let mut full = scene_labels();
        full.extend(made.iter().map(|l| l.to_string()));
        assert_eq!(labels(&h), full, "{tool:?}");
        undo(&mut h);
        assert_eq!(labels(&h), scene_labels(), "{tool:?}");
        assert!(h.state().tool.picks().is_empty());
        redo(&mut h);
        assert_eq!(labels(&h), full, "{tool:?}");
    }
}

#[test]
fn escape_in_the_middle_of_a_tool_keeps_its_points_and_drops_the_picks() {
    let mut h = with_clicks(Tool::Circle3, &[(0.0, 2.0), (1.0, 2.0)]);
    assert_eq!(h.state().tool.picks().len(), 2);
    h.key_press(egui::Key::Escape);
    h.run();
    let mut expected = scene_labels();
    expected.extend(["F".to_owned(), "I".to_owned()]);
    assert_eq!(labels(&h), expected);
    assert!(h.state().tool.picks().is_empty());
    // A tool switch does the same.
    let mut h = with_clicks(Tool::Segment, &[(0.0, 2.0)]);
    h.state_mut().select_tool(Tool::Line);
    h.run();
    assert!(h.state().tool.picks().is_empty());
    assert!(h.state().wb.construction().id("F").is_some());
}

#[test]
fn undo_in_the_middle_of_a_tool_removes_the_last_point_and_resets_the_picks() {
    let mut h = with_clicks(Tool::Circle3, &[(0.0, 2.0), (1.0, 2.0)]);
    undo(&mut h);
    let mut expected = scene_labels();
    expected.push("F".into());
    assert_eq!(labels(&h), expected);
    assert!(h.state().tool.picks().is_empty());
    assert_eq!(
        h.state().tool.hint(),
        crate::tools::ToolState::new(Tool::Circle3).hint(),
        "the hint is back at the first pick"
    );
}

#[test]
fn a_regular_polygon_with_fewer_than_three_vertices_is_undefined() {
    // As in the reference (the dialog does not check the count, and
    // `AlgoPolygonRegularND.checkUnDefined` makes the polygon undefined
    // below three vertices), the number and an undefined polygon are made.
    let mut h = with_clicks(Tool::RegularPolygon, &[(0.0, 2.0), (1.0, 2.0)]);
    assert!(h.state().tool.asking().is_some());
    h.state_mut().number_text = "2".into();
    h.key_press(egui::Key::Enter);
    h.run();
    let mut points = scene_labels();
    points.extend(["F".to_owned(), "I".to_owned()]);
    let mut all = points.clone();
    all.extend(["a".to_owned(), "poly1".to_owned()]);
    assert_eq!(labels(&h), all, "no vertices or sides");
    let app = h.state();
    assert!(app.tool_error.is_none() && app.tool.picks().is_empty());
    assert_eq!(app.wb.definition("poly1"), Some("Polygon(F, I, a)"));
    let c = app.wb.construction();
    assert!(!c.value(c.id("poly1").unwrap()).is_defined());
    undo(&mut h);
    assert_eq!(labels(&h), points, "one undo takes the polygon back");
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
