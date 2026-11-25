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
            Tool::Point | Tool::PointOnObject => vec![empty],
            Tool::SegmentFixed => vec![a],
            Tool::AngleFixed => vec![a, c],
            Tool::Extremum => vec![(1.0, 1.0)],
            Tool::Roots => vec![(0.0, -1.0)],
            Tool::Intersect => vec![f, k],
            Tool::Midpoint => vec![a, b],
            Tool::Perpendicular | Tool::Parallel | Tool::ReflectLine => vec![c, f],
            Tool::AngleBisector | Tool::Angle => vec![a, c, b],
            Tool::Tangents | Tool::Polar | Tool::ReflectCircle => vec![c, k],
            Tool::CircleRadius => vec![a],
            Tool::Circle3
            | Tool::CircularArc
            | Tool::CircumcircularArc
            | Tool::CircularSector
            | Tool::CircumcircularSector
            | Tool::Compasses
            | Tool::Ellipse
            | Tool::Hyperbola => vec![a, b, c],
            Tool::Semicircle => vec![a, b],
            Tool::Conic5 => vec![a, b, c, (4.0, -2.0), (6.0, -2.0)],
            Tool::Parabola => vec![f, c],
            Tool::Area => vec![k],
            Tool::Slope => vec![f],
            Tool::VectorFromPoint => vec![u, c],
            Tool::ReflectPoint | Tool::Rotate | Tool::Dilate => vec![c, a],
            Tool::Translate => vec![c, u],
            Tool::Polygon | Tool::Polyline | Tool::VectorPolygon | Tool::RigidPolygon => {
                vec![a, b, c, a]
            }
            Tool::RegularPolygon => vec![a, b],
            Tool::Locus => vec![(1.0, 0.0), (2.0, 0.0)],
            _ => vec![a, c],
        }
    };
    // The first output of each tool: label, kind and definition.
    let expected = |tool: Tool| -> (&str, Kind, &str) {
        match tool {
            Tool::Point | Tool::PointOnObject => ("F", Kind::Point, "(0, 2)"),
            Tool::RigidPolygon => ("t1", Kind::Polygon, "RigidPolygon(A, B, C)"),
            Tool::SegmentFixed => ("g", Kind::Segment, "Segment(A, 2)"),
            Tool::AngleFixed => ("α", Kind::Angle, "Angle(A, C, 0.7853981633974483)"),
            Tool::Extremum => ("A", Kind::Point, "Extremum(f)"),
            Tool::Roots => ("A", Kind::Point, "Root(f)"),
            Tool::Intersect => ("F_{1}", Kind::Point, "Intersect(f, k)"),
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
            Tool::Semicircle => ("c", Kind::Arc, "Semicircle(A, B)"),
            Tool::CircularArc => ("c", Kind::Arc, "CircularArc(A, B, C)"),
            Tool::CircumcircularArc => ("c", Kind::Arc, "CircumcircularArc(A, B, C)"),
            Tool::CircularSector => ("c", Kind::Sector, "CircularSector(A, B, C)"),
            Tool::CircumcircularSector => ("c", Kind::Sector, "CircumcircularSector(A, B, C)"),
            Tool::Compasses => ("c", Kind::Circle, "Circle(C, Segment(A, B))"),
            Tool::Ellipse => ("c", Kind::Conic, "Ellipse(A, B, C)"),
            Tool::Hyperbola => ("c", Kind::Conic, "Hyperbola(A, B, C)"),
            Tool::Parabola => ("c", Kind::Conic, "Parabola(C, f)"),
            Tool::Conic5 => ("c", Kind::Conic, "Conic(A, B, C, D, E)"),
            Tool::Polyline => ("g", Kind::PolyLine, "PolyLine(A, B, C)"),
            Tool::Area => ("areak", Kind::Number, "Area(k)"),
            Tool::Slope => ("m", Kind::Number, "Slope(f)"),
            Tool::VectorFromPoint => ("v", Kind::Vector, "Vector(C, C')"),
            Tool::VectorPolygon => ("t1", Kind::Polygon, "Polygon(A, B, C)"),
            Tool::Locus => ("locus1", Kind::Locus, "Locus(Q, P)"),
            Tool::Angle => ("α", Kind::Angle, "Angle(A, C, B)"),
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
        let scene = match tool {
            Tool::Extremum => "f(x) = (x - 1)^2 + 1",
            Tool::Roots => "f(x) = x^2 - 1",
            Tool::VectorPolygon | Tool::RigidPolygon => "A = (-6, -3)\nB = (-2, -3)\nC = (-4, 1)\n",
            Tool::Locus => "A=(0,0)\nB=(2,0)\nk=Circle(A,B)\nP=Point(k)\nQ=Midpoint(P,A)",
            _ => TOOL_SCENE,
        };
        let mut h = harness(scene);
        h.state_mut().select_tool(tool);
        let clicks = plan(tool);
        let (last, first) = clicks.split_last().unwrap();
        for &(x, y) in first {
            click(&mut h, world(x, y));
        }
        let before = labels(&h);
        assert_eq!(h.state().wb.undo_depth(), (0, 0), "{tool:?}");
        click(&mut h, world(last.0, last.1));
        if h.state().tool.asking().is_some() {
            // The number dialog: take the value it offers.
            h.key_press(egui::Key::Enter);
            h.run();
        }
        let after = labels(&h);
        let (label, kind, rhs) = expected(tool);
        let app = h.state();
        assert_eq!(app.wb.undo_depth(), (1, 0), "{tool:?}");
        let c = app.wb.construction();
        let intersection_labels = [label, "F_{2}"];
        let expected_labels = if tool == Tool::Intersect {
            let added: Vec<_> = after
                .iter()
                .filter(|name| !before.contains(name))
                .map(String::as_str)
                .collect();
            if added != intersection_labels {
                failures.push(format!("{tool:?}: expected both points, got {added:?}"));
            }
            &intersection_labels[..]
        } else {
            std::slice::from_ref(&label)
        };
        for &label in expected_labels {
            let made = c.id(label).map(|id| (c.kind(id), app.wb.definition(label)));
            if made != Some((kind, Some(rhs))) {
                failures.push(format!(
                    "{tool:?}: expected {label} as {kind:?} = {rhs}, got {made:?}"
                ));
            }
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
fn cancelling_the_number_dialog_takes_the_new_point_away() {
    // Circle with radius on empty space: the centre is made at once, as a
    // pending point of the tool use; cancelling the dialog ends the use
    // unfinished and takes it away (spec E5; what the reference does on a
    // cancelled dialog is unconfirmed, this follows its Escape rule).
    let mut h = with_clicks(Tool::CircleRadius, &[(0.0, 2.0)]);
    assert!(h.state().tool.asking().is_some());
    assert!(h.state().wb.construction().id("F").is_some());
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(labels(&h), scene_labels());
    let app = h.state();
    assert!(app.tool.asking().is_none() && app.tool.picks().is_empty());
    assert_eq!(app.wb.undo_depth(), (0, 0));
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
fn escape_in_the_middle_of_a_tool_removes_its_points_and_drops_the_picks() {
    // Spec E5 (`initNewMode` -> `Kernel.restoreStateForInitNewMode`).
    let mut h = with_clicks(Tool::Circle3, &[(0.0, 2.0), (1.0, 2.0)]);
    assert_eq!(h.state().tool.picks().len(), 2);
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(labels(&h), scene_labels());
    assert!(h.state().tool.picks().is_empty());
    assert_eq!(
        h.state().tool.tool(),
        Tool::Move,
        "Escape goes to the move tool"
    );
    assert_eq!(h.state().wb.undo_depth(), (0, 0));
    // A tool switch does the same.
    let mut h = with_clicks(Tool::Segment, &[(0.0, 2.0)]);
    assert!(h.state().wb.construction().id("F").is_some());
    h.state_mut().select_tool(Tool::Line);
    h.run();
    assert!(h.state().tool.picks().is_empty());
    assert!(h.state().wb.construction().id("F").is_none());
    assert_eq!(h.state().wb.undo_depth(), (0, 0));
}

#[test]
fn undo_in_the_middle_of_a_tool_removes_all_its_points_and_resets_the_picks() {
    // Spec F4 (`Kernel.undo` -> `restoreStateForModeStarting`): no real
    // undo, the earlier steps stay.
    let mut h = with_clicks(Tool::Point, &[(5.0, 5.0)]);
    h.state_mut().select_tool(Tool::Circle3);
    for &(x, y) in &[(0.0, 2.0), (1.0, 2.0)] {
        click(&mut h, world(x, y));
    }
    let mut expected = scene_labels();
    expected.push("F".into());
    undo(&mut h);
    assert_eq!(labels(&h), expected);
    assert!(h.state().tool.picks().is_empty());
    assert_eq!(
        h.state().tool.hint(),
        crate::tools::ToolState::new(Tool::Circle3).hint(),
        "the hint is back at the first pick"
    );
    assert_eq!(
        h.state().wb.undo_depth(),
        (1, 0),
        "the point tool's step stays"
    );
    undo(&mut h);
    assert_eq!(labels(&h), scene_labels());
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
    // The whole use is one step, its two new points included (spec F1).
    undo(&mut h);
    assert_eq!(
        labels(&h),
        scene_labels(),
        "one undo takes the whole use back"
    );
    redo(&mut h);
    assert_eq!(labels(&h), all);
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
    let scale = h.state().camera.unwrap().xscale;
    h.state_mut().select_tool(Tool::ZoomIn);
    click(&mut h, world(5.0, 5.0));
    let zoomed = h.state().camera.unwrap().xscale;
    assert!(zoomed > scale);
    h.state_mut().select_tool(Tool::ZoomOut);
    click(&mut h, world(1.0, 1.0));
    assert!(h.state().camera.unwrap().xscale < zoomed);
}

#[test]
fn new_curve_tools_group_free_points_and_result_in_one_step() {
    let a = (-4.0, 0.0);
    let b = (2.0, 0.0);
    let c = (1.0, 3.0);
    let d = (-2.0, -2.0);
    let e = (4.0, 3.0);
    for (tool, points, label, definition) in [
        (Tool::Semicircle, vec![a, b], "c", "Semicircle(A, B)"),
        (
            Tool::CircularArc,
            vec![a, b, c],
            "c",
            "CircularArc(A, B, C)",
        ),
        (
            Tool::CircumcircularArc,
            vec![a, b, c],
            "c",
            "CircumcircularArc(A, B, C)",
        ),
        (
            Tool::CircularSector,
            vec![a, b, c],
            "c",
            "CircularSector(A, B, C)",
        ),
        (
            Tool::CircumcircularSector,
            vec![a, b, c],
            "c",
            "CircumcircularSector(A, B, C)",
        ),
        (
            Tool::Compasses,
            vec![a, b, c],
            "c",
            "Circle(C, Segment(A, B))",
        ),
        (Tool::Ellipse, vec![a, b, c], "c", "Ellipse(A, B, C)"),
        (Tool::Hyperbola, vec![a, b, c], "c", "Hyperbola(A, B, C)"),
        (
            Tool::Conic5,
            vec![a, b, c, d, e],
            "c",
            "Conic(A, B, C, D, E)",
        ),
        (Tool::Polyline, vec![a, b, c, a], "f", "PolyLine(A, B, C)"),
        (
            Tool::VectorPolygon,
            vec![a, b, c, a],
            "t1",
            "Polygon(A, B, C)",
        ),
    ] {
        let mut h = harness("");
        h.state_mut().select_tool(tool);
        let (last, earlier) = points.split_last().unwrap();
        for &(x, y) in earlier {
            click(&mut h, world(x, y));
            assert_eq!(h.state().wb.undo_depth(), (0, 0), "{tool:?}");
        }
        click(&mut h, world(last.0, last.1));
        assert_eq!(h.state().wb.definition(label), Some(definition), "{tool:?}");
        assert_eq!(h.state().wb.undo_depth(), (1, 0), "{tool:?}");
        let after = labels(&h);
        undo(&mut h);
        assert!(labels(&h).is_empty(), "{tool:?}");
        redo(&mut h);
        assert_eq!(labels(&h), after, "{tool:?}");
    }
}

#[test]
fn compasses_copies_radii_and_can_reuse_a_radius_point_as_center() {
    let scene = "A=(-4,0)\nB=(2,0)\nC=(4,3)\nf=Segment(A,B)\nk=Circle(A,B)";
    for (clicks, definition) in [
        (vec![(-1.0, 0.0), (4.0, 3.0)], "Circle(C, f)"),
        (vec![(-4.0, 6.0), (4.0, 3.0)], "Circle(C, Radius(k))"),
        (
            vec![(-4.0, 0.0), (2.0, 0.0), (2.0, 0.0)],
            "Circle(B, Segment(A, B))",
        ),
    ] {
        let mut h = harness(scene);
        h.state_mut().select_tool(Tool::Compasses);
        for (x, y) in clicks {
            click(&mut h, world(x, y));
        }
        assert_eq!(h.state().wb.definition("c"), Some(definition));
        assert_eq!(h.state().wb.definition("a"), None);
        let c = h.state().wb.construction();
        assert!(matches!(
            c.value(c.id("c").unwrap()),
            Value::Circle { r: 6.0, .. }
        ));
        assert_eq!(h.state().wb.undo_depth(), (1, 0));
        undo(&mut h);
        assert!(h.state().wb.construction().id("c").is_none());
    }
}

#[test]
fn parabola_creates_a_free_focus_in_either_order() {
    for clicks in [[(0.0, 2.0), (0.0, -2.0)], [(0.0, -2.0), (0.0, 2.0)]] {
        let mut h = harness("A=(-4,-2)\nB=(4,-2)\nf=Line(A,B)");
        h.state_mut().select_tool(Tool::Parabola);
        click(&mut h, world(clicks[0].0, clicks[0].1));
        assert_eq!(h.state().wb.undo_depth(), (0, 0));
        click(&mut h, world(clicks[1].0, clicks[1].1));
        assert_eq!(h.state().wb.definition("c"), Some("Parabola(C, f)"));
        assert_eq!(h.state().wb.definition("C"), Some("(0, 2)"));
        assert_eq!(h.state().wb.undo_depth(), (1, 0));
        undo(&mut h);
        assert_eq!(labels(&h), ["A", "B", "f"]);
    }
}

#[test]
fn vector_from_point_creates_a_translated_endpoint_in_one_step() {
    let mut h = harness(TOOL_SCENE);
    let before = labels(&h);
    h.state_mut().select_tool(Tool::VectorFromPoint);
    click(&mut h, world(0.0, 2.0));
    assert_eq!(labels(&h), before);
    click(&mut h, world(-4.0, 1.0));
    click(&mut h, world(-5.0, 3.5));
    assert_eq!(h.state().wb.definition("C'"), Some("Translate(C, u)"));
    assert_eq!(h.state().wb.definition("v"), Some("Vector(C, C')"));
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    undo(&mut h);
    assert_eq!(labels(&h), before);
}

#[test]
fn vector_polygon_redefines_free_vertices_and_undo_restores_them() {
    let mut h = harness("A=(-3,0)\nB=(0,3)\nC=(3,0)\nD=(0,-2)\nn=x(D)");
    h.state_mut().select_tool(Tool::VectorPolygon);
    for (x, y) in [
        (-3.0, 0.0),
        (0.0, -2.0),
        (0.0, 3.0),
        (3.0, 0.0),
        (-3.0, 0.0),
    ] {
        click(&mut h, world(x, y));
    }
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    assert_eq!(h.state().wb.definition("B"), Some("(a + x(A), b + y(A))"));
    assert_eq!(h.state().wb.definition("t1"), Some("Polygon(A, B, C)"));
    let wb = &mut h.state_mut().wb;
    assert!(wb.move_point(wb.construction().id("A").unwrap(), -1.0, 1.0));
    let c = wb.construction();
    assert_eq!(c.value(c.id("B").unwrap()), Value::Point { x: 2.0, y: 4.0 });
    assert_eq!(c.value(c.id("C").unwrap()), Value::Point { x: 5.0, y: 1.0 });
    undo(&mut h);
    undo(&mut h);
    let c = h.state().wb.construction();
    assert!(c.def(c.id("B").unwrap()).is_free());
    assert!(c.def(c.id("C").unwrap()).is_free());
    assert_eq!(labels(&h), ["A", "B", "C", "D", "n"]);
}

#[test]
fn vector_polygon_copies_an_existing_polygon_with_its_own_anchor() {
    let mut h = harness("A=(-3,0)\nB=(0,3)\nC=(3,0)\nt1=Polygon(A,B,C)");
    let before = labels(&h);
    h.state_mut().select_tool(Tool::VectorPolygon);
    click(&mut h, world(0.0, 1.0));
    assert_eq!(h.state().wb.definition("t2"), Some("Polygon(D, E, F)"));
    let c = h.state().wb.construction();
    let a = c.value(c.id("A").unwrap());
    let d = c.value(c.id("D").unwrap());
    assert_ne!(a, d);
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    undo(&mut h);
    assert_eq!(labels(&h), before);
}

#[test]
fn move_picks_nearby_outlines_and_the_filled_sector() {
    let cases = [
        (
            "A=(-2,0)\nB=(2,0)\nC=(0,3)\nc=Ellipse(A,B,C)",
            "c",
            (-13.0_f64.sqrt(), 0.0),
            (0.0, 1.0),
            false,
        ),
        (
            "A=(-3,0)\nB=(3,0)\nc=Semicircle(A,B)",
            "c",
            (0.0, 3.0),
            (0.0, -3.0),
            false,
        ),
        (
            "A=(0,0)\nB=(3,0)\nC=(0,3)\nc=CircularSector(A,B,C)",
            "c",
            (1.5, 0.0),
            (1.5, 1.0),
            true,
        ),
        ("f(x)=x^2", "f", (1.0, 1.0), (1.0, 2.0), false),
    ];
    for (script, label, near, far, fills) in cases {
        let mut h = harness(script);
        click(&mut h, world(far.0, far.1));
        let expected = fills.then(|| h.state().wb.construction().id(label).unwrap());
        assert_eq!(h.state().wb.selected, expected, "{script}");
        let mut pos = world(near.0, near.1);
        pos.y -= 3.0;
        click(&mut h, pos);
        let wb = &h.state().wb;
        assert_eq!(wb.selected, wb.construction().id(label), "{script}");
        assert_eq!(wb.undo_depth(), (0, 0));
    }
}

#[test]
fn a_point_on_a_circular_part_uses_the_path_and_is_undoable() {
    for command in ["CircularArc", "CircularSector"] {
        let mut h = harness(&format!("A=(0,0)\nB=(3,0)\nC=(0,3)\nc={command}(A,B,C)"));
        h.state_mut().select_tool(Tool::Point);
        let coordinate = 3.0 / 2.0_f64.sqrt();
        click(&mut h, world(coordinate, coordinate));
        let wb = &h.state().wb;
        assert_eq!(wb.definition("D"), Some("Point(c)"));
        let c = wb.construction();
        let Value::Point { x, y } = c.value(c.id("D").unwrap()) else {
            panic!("missing point")
        };
        assert!((x.hypot(y) - 3.0).abs() < 1e-9);
        assert!(x > 0.0 && y > 0.0);
        assert_eq!(wb.undo_depth(), (1, 0));
        undo(&mut h);
        assert_eq!(labels(&h), ["A", "B", "C", "c"]);
    }
}

#[test]
fn intersect_picks_a_conic_and_line_and_makes_both_intersections_atomically() {
    let mut h =
        harness("A=(-2,0)\nB=(2,0)\nC=(0,3)\nc=Ellipse(A,B,C)\nD=(-6,1)\nE=(6,1)\nf=Line(D,E)");
    h.state_mut().select_tool(Tool::Intersect);
    click(&mut h, world(13.0_f64.sqrt(), 0.0));
    assert_eq!(h.state().wb.undo_depth(), (0, 0));
    click(&mut h, world(0.0, 1.0));
    assert_eq!(h.state().wb.definition("F"), Some("Intersect(f, c, 1)"));
    assert_eq!(h.state().wb.definition("G"), Some("Intersect(f, c, 2)"));
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    undo(&mut h);
    assert_eq!(labels(&h), ["A", "B", "C", "c", "D", "E", "f"]);
}

#[test]
fn slope_picks_a_function_graph_and_measures_the_derivative_at_zero() {
    let mut h = harness("f(x)=x^2+3x");
    h.state_mut().select_tool(Tool::Slope);
    click(&mut h, world(1.0, 4.0));
    let wb = &h.state().wb;
    assert_eq!(wb.definition("m"), Some("Slope(f)"));
    let c = wb.construction();
    let m = c.id("m").unwrap();
    assert!(wb.is_visible(m));
    let Value::Number(n) = c.value(m) else {
        panic!("missing slope")
    };
    assert!((n - 3.0).abs() < 1e-9, "{n}");
    assert_eq!(wb.undo_depth(), (1, 0));
    undo(&mut h);
    assert_eq!(labels(&h), ["f"]);
}

#[test]
fn area_measures_a_sector_but_ignores_an_arc() {
    for (command, expected) in [("CircularSector", Some("Area(c)")), ("CircularArc", None)] {
        let mut h = harness(&format!("A=(0,0)\nB=(3,0)\nC=(0,3)\nc={command}(A,B,C)"));
        h.state_mut().select_tool(Tool::Area);
        let coordinate = 3.0 / 2.0_f64.sqrt();
        click(&mut h, world(coordinate, coordinate));
        let wb = &h.state().wb;
        assert_eq!(wb.definition("areac"), expected);
        assert_eq!(wb.undo_depth(), (usize::from(expected.is_some()), 0));
    }
}

#[test]
fn compasses_matches_the_reference_construction_lists() {
    for (scene, clicks, expected, circle) in [
        (
            "A=(2,-2)\nB=(0,0)",
            vec![(2.0, -2.0), (0.0, 0.0), (3.0, 0.0)],
            vec!["A", "B", "C", "c"],
            "c",
        ),
        (
            "A=(2,-2)\nB=(0,0)\nc=Circle(A,B)",
            vec![(4.0, -4.0), (3.0, 0.0)],
            vec!["A", "B", "c", "C", "d"],
            "d",
        ),
    ] {
        let mut h = harness(scene);
        h.state_mut().select_tool(Tool::Compasses);
        for (x, y) in clicks {
            click(&mut h, world(x, y));
        }
        let wb = &mut h.state_mut().wb;
        assert_eq!(
            wb.rows()
                .iter()
                .map(|r| r.label.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(wb.definition("a"), None);
        let c = wb.construction();
        let Value::Circle { cx, cy, r } = c.value(c.id(circle).unwrap()) else {
            panic!("missing circle")
        };
        assert_eq!((cx, cy), (3.0, 0.0));
        assert!((r - 8.0_f64.sqrt()).abs() < 1e-9);
        assert_eq!(wb.undo_depth(), (1, 0));
    }
}

#[test]
fn polyline_deselects_non_closing_vertices() {
    let a = (-2.0, 0.0);
    let b = (0.0, 0.0);
    let c = (0.0, -3.0);
    for (clicks, selected) in [
        (vec![a, b, a], vec!["B"]),
        (vec![a, b, c, b], vec!["A", "C"]),
    ] {
        let mut h = harness("A=(-2,0)\nB=(0,0)\nC=(0,-3)");
        h.state_mut().select_tool(Tool::Polyline);
        for (x, y) in clicks {
            click(&mut h, world(x, y));
        }
        assert_eq!(labels(&h), ["A", "B", "C"]);
        assert_eq!(
            h.state()
                .tool
                .picks()
                .iter()
                .map(|p| p.label())
                .collect::<Vec<_>>(),
            selected
        );
        assert_eq!(h.state().wb.undo_depth(), (0, 0));
    }
}

#[test]
fn vector_from_point_deselects_the_first_point_when_clicked_again() {
    let mut h = harness("A=(-2,-2)\nB=(-3,-3)\nP=(0,0)\nQ=(4,2)\nu=Vector(P,Q)");
    h.state_mut().select_tool(Tool::VectorFromPoint);
    for (x, y) in [(-2.0, -2.0), (-2.0, -2.0), (2.0, 1.0), (-3.0, -3.0)] {
        click(&mut h, world(x, y));
    }
    assert_eq!(h.state().wb.definition("v"), Some("Vector(B, B')"));
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
}

#[test]
fn compasses_rejects_an_ellipse_and_clears_a_partially_picked_radius() {
    let mut h = harness("F=(4,-3.5)\nG=(4,-0.5)\nP=(6,-2)\ne=Ellipse(F,G,P)\nA=(1,-1)\nB=(3,-1)\ns=Segment(A,B)\nC=(8,-2)");
    let before = labels(&h);
    h.state_mut().select_tool(Tool::Compasses);
    click(&mut h, world(1.0, -1.0));
    click(&mut h, world(4.0, 0.5));
    assert_eq!(labels(&h), before);
    assert!(h.state().tool.picks().is_empty());
    click(&mut h, world(2.0, -1.0));
    click(&mut h, world(8.0, -2.0));
    assert_eq!(h.state().wb.definition("c"), Some("Circle(C, s)"));
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
}

#[test]
fn compasses_uses_circular_parts_for_their_radius() {
    for command in ["CircularArc", "CircularSector"] {
        let mut h = harness(&format!("A=(2,-2)\nB=(4,-2)\nC=(2,0)\ns={command}(A,B,C)"));
        h.state_mut().select_tool(Tool::Compasses);
        click(&mut h, world(2.0 + 2.0_f64.sqrt(), -2.0 + 2.0_f64.sqrt()));
        click(&mut h, world(5.0, 0.0));
        let wb = &mut h.state_mut().wb;
        assert_eq!(
            wb.rows()
                .iter()
                .map(|r| r.label.as_str())
                .collect::<Vec<_>>(),
            ["A", "B", "C", "s", "D", "c"]
        );
        assert_eq!(wb.definition("c"), Some("Circle(D, Radius(s))"));
        let c = wb.construction();
        assert!(matches!(
            c.value(c.id("c").unwrap()),
            Value::Circle {
                cx: 5.0,
                cy: 0.0,
                r: 2.0
            }
        ));
        assert_eq!(wb.undo_depth(), (1, 0));
    }
}

#[test]
fn area_uses_the_conic_label_and_prefers_a_conic_over_polygon_fill() {
    let mut h = harness("O=(3,-2)\nN=(4,-2)\nc=Circle(O,N)\nA=(1,-0.5)\nB=(5,-0.5)\nC=(5,-3.5)\nD=(1,-3.5)\np=Polygon(A,B,C,D)");
    h.state_mut().select_tool(Tool::Area);
    click(&mut h, world(3.0, -1.0));
    assert_eq!(h.state().wb.definition("areac"), Some("Area(c)"));
    click(&mut h, world(2.0, -2.0));
    assert_eq!(h.state().wb.definition("areac_1"), Some("Area(c)"));
    click(&mut h, world(1.5, -2.0));
    assert!(h.state().wb.construction().id("areap").is_none());
    assert_eq!(h.state().wb.undo_depth(), (2, 0));
}

#[test]
fn vector_polygon_can_copy_a_polygon_during_vertex_selection() {
    let mut h = harness("A=(-3,0)\nB=(0,3)\nC=(3,0)\nt1=Polygon(A,B,C)\nD=(-3,-3)");
    let before = labels(&h);
    h.state_mut().select_tool(Tool::VectorPolygon);
    click(&mut h, world(-3.0, -3.0));
    click(&mut h, world(0.0, 1.0));
    assert_eq!(h.state().wb.definition("t2"), Some("Polygon(E, F, G)"));
    assert!(h.state().tool.picks().is_empty());
    assert_eq!(h.state().wb.undo_depth(), (1, 0));
    undo(&mut h);
    assert_eq!(labels(&h), before);
}

#[test]
fn compasses_radius_updates_and_survives_undo_and_snapshot_restore() {
    let mut h = harness("A=(1,-1)\nB=(4,-1)\nC=(8,-2)\ns=Segment(A,B)");
    h.state_mut().select_tool(Tool::Compasses);
    for (x, y) in [(1.0, -1.0), (4.0, -1.0), (8.0, -2.0)] {
        click(&mut h, world(x, y));
    }
    let wb = &mut h.state_mut().wb;
    let check_radius = |wb: &Workbench, expected| {
        let c = wb.construction();
        let Value::Circle { r, .. } = c.value(c.id("c").unwrap()) else {
            panic!("missing circle")
        };
        assert_eq!(r, expected);
    };
    check_radius(wb, 3.0);
    let b = wb.construction().id("B").unwrap();
    assert!(wb.move_point(b, 5.0, -1.0));
    check_radius(wb, 4.0);
    assert!(wb.undo());
    check_radius(wb, 3.0);
    assert!(wb.redo());
    check_radius(wb, 4.0);
    let before = wb.snapshot_text();
    assert!(!before.contains('\0'));
    assert!(wb.restore_text(&before).unwrap().is_empty());
    check_radius(wb, 4.0);
    assert_eq!(
        wb.rows()
            .iter()
            .map(|r| r.label.as_str())
            .collect::<Vec<_>>(),
        ["A", "B", "C", "s", "c"]
    );
    let b = wb.construction().id("B").unwrap();
    assert!(wb.delete(b));
    assert!(wb.construction().id("c").is_none());
    assert!(wb.undo());
    check_radius(wb, 4.0);
}
