use super::*;

fn none(_: &str) -> bool {
    false
}

fn obj(label: &str, kind: Kind) -> Option<(&str, Kind)> {
    Some((label, kind))
}

#[test]
fn point_tool_creates_a_labelled_free_point() {
    let mut s = ToolState::new(Tool::Point);
    let taken = |l: &str| l == "A";
    assert_eq!(
        s.click(None, (1.234, -0.001), &taken),
        Outcome::Run(vec!["B = (1.23, 0)".into()])
    );
    // Clicking an existing point does nothing.
    assert_eq!(
        s.click(obj("A", Kind::Point), (0.0, 0.0), &taken),
        Outcome::Ignored
    );
}

#[test]
fn line_tool_over_two_new_points_runs_three_lines() {
    let mut s = ToolState::new(Tool::Line);
    assert_eq!(s.click(None, (0.0, 0.0), &none), Outcome::Pending);
    assert_eq!(s.hint(), "Kliknij drugi punkt prostej");
    let out = s.click(None, (3.0, 4.0), &none);
    assert_eq!(
        out,
        Outcome::Run(vec![
            "A = (0, 0)".into(),
            "B = (3, 4)".into(),
            "f = Line(A, B)".into()
        ])
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
    assert_eq!(
        out,
        Outcome::Run(vec!["B = (2, 0)".into(), "c = Circle(A, B)".into()])
    );
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
    assert_eq!(
        out,
        Outcome::Run(vec!["A = (1, 2)".into(), "g = Perpendicular(A, f)".into()])
    );
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
    assert_eq!(out, Outcome::Pending);
    assert_eq!(s.picks()[0], Pick::New { x: 2.0, y: 1.0 });
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
        out,
        Outcome::Run(vec![
            "B = (0, 0)".into(),
            "C = (1, 0)".into(),
            "f = PerpendicularBisector(B, C)".into()
        ])
    );
}

/// Every pick a tool offers must give lines the kernel accepts.
#[test]
fn every_offered_tool_combination_runs_in_the_kernel() {
    use crate::workbench::Workbench;
    const SCRIPT: &str = "A = (0, 0)\nB = (4, 1)\nC = (1, 3)\nf = Line(A, B)\n\
                          s = Segment(B, C)\nr = Ray(A, C)\nc = Circle(A, B)\nd = Circle(C, B)\n";
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
    let mut ran = 0;
    for tool in Tool::ALL {
        if matches!(tool, Tool::Move | Tool::Delete) {
            continue;
        }
        for first in &candidates {
            for second in &candidates {
                let mut state = ToolState::new(tool);
                let w = Workbench::from_script(SCRIPT).unwrap();
                let is_taken = |l: &str| w.construction().id(l).is_some();
                let mut outcome = Outcome::Ignored;
                let mut offered = 0;
                for pick in [first, second] {
                    let hit = pick.as_ref().map(|(l, k)| (l.as_str(), *k));
                    let allowed = match &hit {
                        Some((l, k)) => state.accepts_object(l, *k),
                        None => state.accepts_new_point(),
                    };
                    if !allowed {
                        break;
                    }
                    offered += 1;
                    outcome = state.click(hit, (1.5, -2.5), &is_taken);
                }
                let needed = if tool == Tool::Point { 1 } else { 2 };
                if offered < needed {
                    continue;
                }
                let Outcome::Run(lines) = outcome else {
                    failures.push(format!("{tool:?}: no lines for {first:?} {second:?}"));
                    continue;
                };
                let mut w = w;
                ran += 1;
                if let Err(e) = w.run_lines(&lines) {
                    failures.push(format!("{tool:?} {first:?} {second:?}: {lines:?}: {e}"));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {ran}:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(ran > 100, "only {ran} combinations ran");
}
