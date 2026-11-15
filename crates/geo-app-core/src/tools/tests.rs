use super::*;

fn none(_: &str) -> bool {
    false
}

fn obj(label: &str, kind: Kind) -> Option<(&str, Kind)> {
    Some((label, kind))
}

fn place(line: &str, then: Outcome) -> Outcome {
    Outcome::Place {
        line: line.into(),
        then: Box::new(then),
    }
}

/// The points an outcome places first, and what comes after them.
fn unwrap_places(mut outcome: Outcome) -> (Vec<String>, Outcome) {
    let mut placed = Vec::new();
    while let Outcome::Place { line, then } = outcome {
        placed.push(line);
        outcome = *then;
    }
    (placed, outcome)
}

/// All the lines an outcome runs, placed points first.
fn all_lines(outcome: Outcome) -> Vec<String> {
    let (mut lines, rest) = unwrap_places(outcome);
    if let Outcome::Run(more) = rest {
        lines.extend(more);
    }
    lines
}

#[test]
fn point_tool_creates_a_labelled_free_point() {
    let mut s = ToolState::new(Tool::Point);
    let taken = |l: &str| l == "A";
    assert_eq!(
        s.click(None, (1.234, -0.001), &taken),
        place("B = (1.234, -0.001)", Outcome::Pending)
    );
    // Clicking an existing point selects it and never creates a second one.
    assert_eq!(
        s.click(obj("A", Kind::Point), (0.0, 0.0), &taken),
        Outcome::Select("A".into())
    );
}

#[test]
fn line_tool_over_two_new_points_runs_three_lines() {
    let mut s = ToolState::new(Tool::Line);
    // Each new point is a change of its own, made at once.
    assert_eq!(
        s.click(None, (0.0, 0.0), &none),
        place("A = (0, 0)", Outcome::Pending)
    );
    assert_eq!(s.hint(), "Kliknij drugi punkt prostej");
    let out = s.click(None, (3.0, 4.0), &none);
    assert_eq!(
        out,
        place("B = (3, 4)", Outcome::Run(vec!["f = Line(A, B)".into()]))
    );
    assert!(s.picks().is_empty());
    assert_eq!(s.hint(), "Kliknij pierwszy punkt prostej");
}

#[test]
fn existing_points_are_reused_and_not_picked_twice() {
    let mut s = ToolState::new(Tool::Segment);
    assert_eq!(
        s.click(obj("A", Kind::Point), (0.0, 0.0), &|l| l == "A"),
        Outcome::Pending
    );
    assert!(!s.accepts_object("A", Kind::Point));
    assert!(s.accepts_object("B", Kind::Point));
    // The same point again falls through to a new point? No: the click
    // landed on a point, so it is ignored.
    assert_eq!(
        s.click(obj("A", Kind::Point), (0.0, 0.0), &|l| l == "A"),
        Outcome::Ignored
    );
    let out = s.click(obj("B", Kind::Point), (1.0, 1.0), &|l| l == "A" || l == "B");
    assert_eq!(out, Outcome::Run(vec!["f = Segment(A, B)".into()]));
}

#[test]
fn circle_tool_uses_a_circle_label() {
    let mut s = ToolState::new(Tool::Circle);
    s.click(obj("A", Kind::Point), (0.0, 0.0), &|l| l == "A");
    let out = s.click(None, (2.0, 0.0), &|l| l == "A");
    assert_eq!(all_lines(out), ["B = (2, 0)", "c = Circle(A, B)"]);
}

#[test]
fn perpendicular_takes_point_and_line_in_either_order() {
    let mut s = ToolState::new(Tool::Perpendicular);
    assert!(s.accepts_object("f", Kind::Line));
    assert!(s.accepts_object("A", Kind::Point));
    assert!(!s.accepts_object("c", Kind::Circle));
    assert_eq!(
        s.click(obj("f", Kind::Line), (0.0, 0.0), &|l| l == "f" || l == "A"),
        Outcome::Pending
    );
    assert_eq!(s.hint(), "Kliknij punkt");
    assert!(!s.accepts_object("g", Kind::Line), "needs a point now");
    let out = s.click(obj("A", Kind::Point), (0.0, 0.0), &|l| l == "f" || l == "A");
    assert_eq!(out, Outcome::Run(vec!["g = Perpendicular(A, f)".into()]));
}

#[test]
fn perpendicular_with_a_new_point_after_the_line() {
    let mut s = ToolState::new(Tool::Perpendicular);
    s.click(obj("f", Kind::Line), (0.0, 0.0), &|l| l == "f");
    let out = s.click(None, (1.0, 2.0), &|l| l == "f");
    assert_eq!(all_lines(out), ["A = (1, 2)", "g = Perpendicular(A, f)"]);
}

#[test]
fn perpendicular_and_parallel_reject_rays_and_intersect_takes_whole_lines_and_circles() {
    let s = ToolState::new(Tool::Perpendicular);
    assert!(!s.accepts_object("r", Kind::Ray));
    let i = ToolState::new(Tool::Intersect);
    assert!(i.accepts_object("c", Kind::Circle) && i.accepts_object("f", Kind::Line));
    assert!(!i.accepts_object("s", Kind::Segment) && !i.accepts_object("r", Kind::Ray));
    let s = ToolState::new(Tool::Parallel);
    assert!(s.accepts_object("f", Kind::Segment));
    assert!(!s.accepts_object("r", Kind::Ray));
}

#[test]
fn intersect_needs_two_distinct_objects_and_makes_a_point() {
    let mut s = ToolState::new(Tool::Intersect);
    assert!(!s.accepts_new_point());
    assert_eq!(s.click(None, (0.0, 0.0), &none), Outcome::Ignored);
    assert_eq!(
        s.click(obj("h", Kind::Line), (0.0, 0.0), &none),
        Outcome::Pending
    );
    assert_eq!(
        s.click(obj("h", Kind::Line), (0.0, 0.0), &none),
        Outcome::Ignored
    );
    assert!(!s.accepts_object("A", Kind::Point));
    let out = s.click(obj("i", Kind::Line), (0.0, 0.0), &|l| l == "h" || l == "i");
    assert_eq!(out, Outcome::Run(vec!["A = Intersect(h, i)".into()]));
}

#[test]
fn clicking_a_line_with_the_line_tool_places_a_new_point() {
    let mut s = ToolState::new(Tool::Line);
    let out = s.click(obj("f", Kind::Line), (2.0, 1.0), &none);
    assert_eq!(out, place("A = (2, 1)", Outcome::Pending));
    assert_eq!(s.picks()[0].label(), "A");
}

#[test]
fn delete_and_move_tools() {
    let mut s = ToolState::new(Tool::Delete);
    assert_eq!(
        s.click(obj("A", Kind::Point), (0.0, 0.0), &none),
        Outcome::Delete("A".into())
    );
    assert_eq!(s.click(None, (0.0, 0.0), &none), Outcome::Ignored);
    let mut m = ToolState::new(Tool::Move);
    assert_eq!(m.click(None, (0.0, 0.0), &none), Outcome::Ignored);
}

#[test]
fn switching_tool_drops_picks() {
    let mut s = ToolState::new(Tool::Line);
    s.click(None, (0.0, 0.0), &none);
    assert_eq!(s.picks().len(), 1);
    s.set_tool(Tool::Circle);
    assert!(s.picks().is_empty());
}

#[test]
fn labels_allocated_inside_one_run_do_not_collide() {
    let mut s = ToolState::new(Tool::Bisector);
    s.click(None, (0.0, 0.0), &|l| l == "A");
    let out = s.click(None, (1.0, 0.0), &|l| l == "A");
    assert_eq!(
        all_lines(out),
        ["C = (1, 0)", "f = PerpendicularBisector(B, C)"]
    );
}

/// Every pick a tool offers must give lines the kernel accepts.
#[test]
fn every_offered_tool_combination_runs_in_the_kernel() {
    use crate::workbench::Workbench;
    const SCRIPT: &str = "A = (0, 0)\nB = (4, 1)\nC = (1, 3)\nf = Line(A, B)\n\
                          s = Segment(B, C)\nr = Ray(A, C)\nc = Circle(A, B)\nd = Circle(C, B)\n\
                          u = Vector(A, C)\n";
    let objects: Vec<(String, Kind)> = {
        let w = Workbench::from_script(SCRIPT).unwrap();
        let c = w.construction();
        c.ids()
            .map(|id| (c.label(id).to_owned(), c.kind(id)))
            .collect()
    };
    let mut candidates: Vec<Option<(String, Kind)>> = vec![None];
    candidates.extend(objects.iter().cloned().map(Some));
    let mut failures = Vec::new();
    let mut ran = std::collections::HashMap::new();
    let n = candidates.len();
    for tool in Tool::all().filter(|t| !t.is_direct()) {
        for code in 0..n * n * n {
            let picks = [
                &candidates[code % n],
                &candidates[code / n % n],
                &candidates[code / n / n],
            ];
            let mut state = ToolState::new(tool);
            let w = Workbench::from_script(SCRIPT).unwrap();
            let is_taken = |l: &str| w.construction().id(l).is_some();
            let mut outcome = Outcome::Ignored;
            let mut placed = Vec::new();
            let mut used = 0;
            for pick in picks {
                let point_done = tool == Tool::Point && !placed.is_empty();
                if point_done || !matches!(outcome, Outcome::Ignored | Outcome::Pending) {
                    break;
                }
                let hit = pick.as_ref().map(|(l, k)| (l.as_str(), *k));
                let allowed = match &hit {
                    Some((l, k)) => state.accepts_object(l, *k),
                    None => state.accepts_new_point(),
                };
                if !allowed {
                    break;
                }
                used += 1;
                let (more, rest) = unwrap_places(state.click(hit, (1.5, -2.5), &is_taken));
                placed.extend(more);
                outcome = rest;
            }
            // Count each distinct sequence once: the unused tail varies.
            if code % n.pow(used) != code {
                continue;
            }
            if let Outcome::AskNumber(request) = outcome {
                outcome = state.finish_number(request.default, &is_taken);
            }
            let more = match outcome {
                Outcome::Run(more) => more,
                // The point tool is done once its point is placed.
                Outcome::Pending if tool == Tool::Point && !placed.is_empty() => Vec::new(),
                _ => continue,
            };
            let mut lines = placed;
            lines.extend(more);
            let mut w = w;
            *ran.entry(tool).or_insert(0) += 1;
            if let Err(e) = w.run_lines(&lines) {
                failures.push(format!("{tool:?} {picks:?}: {lines:?}: {e}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
    // A polygon needs a fourth click to close; it has its own test.
    for tool in Tool::all().filter(|t| !t.is_direct() && *t != Tool::Polygon) {
        assert!(
            ran.get(&tool).copied().unwrap_or(0) > 0,
            "{tool:?} never finished"
        );
    }
}

/// Was `new_points_keep_the_precision_the_zoom_asks_for` (rounded by zoom):
/// a new point keeps the position it is given at full precision, as the
/// reference app stores it (`EuclidianControllerCompanion.createNewPoint`;
/// the algebra view prints two decimals).
#[test]
fn new_points_keep_full_precision() {
    let mut s = ToolState::new(Tool::Point);
    let out = s.click(None, (1.23456789, -0.000049), &none);
    assert_eq!(all_lines(out), ["A = (1.23456789, -0.000049)"]);
    s.set_tool(Tool::Line);
    let out = s.click(None, (0.1 + 0.2, -0.0), &none);
    assert_eq!(all_lines(out), ["A = (0.30000000000000004, 0)"]);
}

#[test]
fn a_click_that_hits_nothing_always_places_a_point() {
    // No distance guard: what a click lands on is decided by the canvas hit
    // test alone. A click on a point placed a moment ago hits that point
    // (see `ui_tests::pointer`); a click that hits nothing is a new point.
    let mut s = ToolState::new(Tool::Circle);
    assert_eq!(
        s.click(None, (1.0, 1.0), &none),
        place("A = (1, 1)", Outcome::Pending)
    );
    assert_eq!(
        all_lines(s.click(None, (1.004, 1.0), &none)),
        ["B = (1.004, 1)", "c = Circle(A, B)"]
    );
}

#[test]
fn a_polygon_closes_on_its_first_vertex_and_runs_in_the_kernel() {
    use crate::workbench::Workbench;
    let mut w = Workbench::from_script("A = (0, 0)\nB = (4, 0)\n").unwrap();
    let names = ["A", "B"];
    let t = |l: &str| names.contains(&l);
    let mut s = ToolState::new(Tool::Polygon);
    assert!(matches!(
        s.click(obj("A", Kind::Point), (0.0, 0.0), &t),
        Outcome::Pending
    ));
    assert!(matches!(
        s.click(obj("B", Kind::Point), (4.0, 0.0), &t),
        Outcome::Pending
    ));
    // The first vertex cannot close a polygon of two vertices.
    assert!(!s.accepts_object("A", Kind::Point));
    assert_eq!(
        s.click(None, (4.0, 3.0), &t),
        place("C = (4, 3)", Outcome::Pending)
    );
    assert_eq!(
        s.click(None, (1.0, 3.0), &t),
        place("D = (1, 3)", Outcome::Pending)
    );
    let t = |l: &str| ["A", "B", "C", "D"].contains(&l);
    assert!(s.accepts_object("A", Kind::Point));
    assert!(
        !s.accepts_object("B", Kind::Point),
        "only the first vertex closes"
    );
    let Outcome::Run(lines) = s.click(obj("A", Kind::Point), (0.0, 0.0), &t) else {
        panic!("clicking A again closes the polygon");
    };
    assert_eq!(lines, ["q1 = Polygon(A, B, C, D)"]);
    w.run_lines(&["C = (4, 3)".into(), "D = (1, 3)".into()])
        .unwrap();
    w.run_lines(&lines).unwrap();
    let c = w.construction();
    assert!(matches!(
        c.value(c.id("q1").unwrap()),
        geo_kernel::kernel::Value::Polygon { .. }
    ));
    // Its four sides came with it.
    assert_eq!(c.len(), 4 + 1 + 4);

    // Placed points are objects: the first of them closes the polygon.
    let mut s = ToolState::new(Tool::Polygon);
    for p in [(0.0, 0.0), (2.0, 0.0), (1.0, 2.0)] {
        assert!(matches!(s.click(None, p, &none), Outcome::Place { .. }));
    }
    let Outcome::Run(lines) = s.click(obj("A", Kind::Point), (0.0, 0.0), &none) else {
        panic!("a click on the first placed point closes the polygon");
    };
    assert_eq!(lines, ["t1 = Polygon(A, B, C)"]);
}

#[test]
fn a_regular_polygon_asks_for_the_number_of_vertices() {
    use crate::workbench::Workbench;
    let mut w = Workbench::from_script("A = (0, 0)\nB = (2, 0)\n").unwrap();
    let names = ["A", "B"];
    let t = |l: &str| names.contains(&l);
    let mut s = ToolState::new(Tool::RegularPolygon);
    s.click(obj("A", Kind::Point), (0.0, 0.0), &t);
    let Outcome::AskNumber(request) = s.click(obj("B", Kind::Point), (2.0, 0.0), &t) else {
        panic!("two vertices ask for the count");
    };
    assert_eq!(request.default, 4.0);
    let Outcome::Run(lines) = s.finish_number(6.0, &t) else {
        panic!("the count finishes it");
    };
    assert_eq!(lines, ["a = 6", "poly1 = Polygon(A, B, a)"]);
    w.run_lines(&lines).unwrap();
    assert!(w.construction().id("poly1").is_some());
}

#[test]
fn cancelling_keeps_the_placed_points_out_of_the_tool_only() {
    let mut s = ToolState::new(Tool::Segment);
    assert!(matches!(
        s.click(None, (0.0, 0.0), &none),
        Outcome::Place { .. }
    ));
    // The point is already in the construction; the tool just forgets it.
    s.reset();
    assert!(s.picks().is_empty());
    assert_eq!(
        s.hint(),
        crate::text::hint(recipe::hint(Tool::Segment, &[]))
    );
}
