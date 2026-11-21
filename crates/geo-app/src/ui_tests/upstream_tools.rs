//! Click scenarios taken from GeoGebra's own controller tests
//! (`EuclidianControllerTest`, with the helpers of
//! `BaseEuclidianControllerTest`), ported for the tools the window has.
//!
//! Upstream clicks at pixel `(px, py)` of a view with 50 pixels per unit
//! and the origin at its top left corner; the world point is
//! `(px / 50, -py / 50)`. The upstream tests write their setup lines as
//! equations (`c: x^2 + y^2 = 25`); the port builds the same objects with
//! the commands the kernel has (`c = Circle((0, 0), 5)`) and expects the
//! definitions in the window's own form. What is asserted is what the
//! upstream test asserts: which objects exist after the clicks, in which
//! order, and where the points are.
//!
//! Scenarios the window cannot pass yet are listed in
//! `upstream-tool-gaps.toml` (with the upstream methods that have no tool
//! here at all); the test fails when a listed scenario starts passing and
//! when an unlisted one fails, like the t-pairs runner does.

use super::*;
use geo_kernel::kernel::Kind;

/// One click of an upstream test, in its pixels.
fn px(x: i32, y: i32) -> (f64, f64) {
    (x as f64 / 50.0, -(y as f64) / 50.0)
}

/// A drag of an upstream test, in its pixels: from and to.
type PixelDrag = ((i32, i32), (i32, i32));

/// A ported scenario: run its clicks with the tool and compare what the
/// construction holds afterwards.
struct Scenario {
    /// The upstream test method this comes from.
    name: &'static str,
    tool: Tool,
    /// Setup lines, the ported form of the upstream `t(...)` calls.
    script: &'static str,
    /// Drags (the upstream box selections) before the clicks, in pixels.
    drags: &'static [PixelDrag],
    clicks: &'static [(i32, i32)],
    /// What the number dialog is given after the last click (the upstream
    /// `prepareInput`), then confirmed with Enter.
    number: Option<&'static str>,
    /// The objects the construction must hold: label, kind and definition.
    expect: &'static [(&'static str, Kind, &'static str)],
    /// The points among them, with their coordinates.
    points: &'static [(&'static str, f64, f64)],
}

const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "deleteTool",
        tool: Tool::Delete,
        script: "A = (1,0)\nB = (1,2)\na = Line(A, B)\nC = (0,-1)\nD = (2,-1)\nb = Line(C, D)\n",
        drags: &[],
        clicks: &[(50, 50), (50, 50)],
        number: None,
        expect: &[("a", Kind::Line, "Line(A, B)")],
        points: &[],
    },
    Scenario {
        name: "mirrorAtPointTool",
        tool: Tool::ReflectPoint,
        script: "",
        drags: &[],
        clicks: &[(0, 0), (100, 100)],
        number: None,
        expect: &[
            ("A", Kind::Point, "(0,0)"),
            ("B", Kind::Point, "(2,-2)"),
            ("A'", Kind::Point, "Reflect(A, B)"),
        ],
        points: &[("A'", 4.0, -4.0)],
    },
    Scenario {
        name: "mirrorAtLineTool",
        tool: Tool::ReflectLine,
        script: "P = (4,0)\nQ = (0,-4)\nf = Line(P, Q)\n",
        drags: &[],
        clicks: &[(0, 0), (100, 100)],
        number: None,
        expect: &[
            ("f", Kind::Line, "Line(P, Q)"),
            ("A", Kind::Point, "(0,0)"),
            ("A'", Kind::Point, "Reflect(A, f)"),
        ],
        points: &[("A'", 4.0, -4.0)],
    },
    Scenario {
        name: "rotateByAngleToolPointToExistingPoint",
        tool: Tool::Rotate,
        script: "A = (1, -1)\nB = (2, -2)\n",
        drags: &[],
        clicks: &[(50, 50), (100, 100)],
        number: Some("180°"),
        expect: &[
            ("A", Kind::Point, "(1, -1)"),
            ("B", Kind::Point, "(2, -2)"),
            ("α", Kind::Number, "3.141592653589793"),
            ("A'", Kind::Point, "Rotate(A, α, B)"),
        ],
        points: &[("A'", 3.0, -3.0)],
    },
    Scenario {
        name: "regularPolygonTool",
        tool: Tool::RegularPolygon,
        script: "",
        drags: &[],
        clicks: &[(100, 100), (0, 0)],
        number: Some("4"),
        expect: &[
            ("A", Kind::Point, "(2, -2)"),
            ("B", Kind::Point, "(0, 0)"),
            ("a", Kind::Number, "4"),
            ("q1", Kind::Polygon, "Polygon(A, B, a)"),
            ("C", Kind::Point, "Polygon(A, B, a)"),
            ("D", Kind::Point, "Polygon(A, B, a)"),
            ("f", Kind::Segment, "Polygon(A, B, a)"),
            ("g", Kind::Segment, "Polygon(A, B, a)"),
            ("h", Kind::Segment, "Polygon(A, B, a)"),
            ("i", Kind::Segment, "Polygon(A, B, a)"),
        ],
        points: &[("C", -2.0, -2.0), ("D", 0.0, -4.0)],
    },
    Scenario {
        name: "mirrorAtCircleTool",
        tool: Tool::ReflectCircle,
        script: "O = (0,0)\nr = 2.8284271247461903\nc = Circle(O, r)\nA = (1,-1)\n",
        drags: &[],
        clicks: &[(50, 50), (100, 100)],
        number: None,
        expect: &[
            ("O", Kind::Point, "(0,0)"),
            ("r", Kind::Number, "2.8284271247461903"),
            ("c", Kind::Circle, "Circle(O, r)"),
            ("A", Kind::Point, "(1,-1)"),
            ("A'", Kind::Point, "Reflect(A, c)"),
        ],
        points: &[("A'", 4.0, -4.0)],
    },
    Scenario {
        name: "rotateByAngleToolMultiplePointsToExistingPoint",
        tool: Tool::Rotate,
        script: "A = (1, -1)\nB = (2, -1)\nC = (2, -2)\n",
        drags: &[((25, 25), (125, 75))],
        clicks: &[(100, 100)],
        number: Some("180°"),
        expect: &[
            ("A", Kind::Point, "(1, -1)"),
            ("B", Kind::Point, "(2, -1)"),
            ("C", Kind::Point, "(2, -2)"),
            ("α", Kind::Angle, "180°"),
            ("A'", Kind::Point, "Rotate(A, α, C)"),
            ("B'", Kind::Point, "Rotate(B, α, C)"),
        ],
        points: &[("A'", 3.0, -3.0), ("B'", 2.0, -3.0)],
    },
    Scenario {
        name: "rotateByAngleToolMultiplePointsToNewPoint",
        tool: Tool::Rotate,
        script: "A = (1, -1)\nB = (3, -2)\n",
        drags: &[((25, 25), (175, 125))],
        clicks: &[(150, 150)],
        number: Some("270°"),
        expect: &[
            ("A", Kind::Point, "(1, -1)"),
            ("B", Kind::Point, "(3, -2)"),
            ("C", Kind::Point, "(3, -3)"),
            ("α", Kind::Number, "4.71238898038469"),
            ("A'", Kind::Point, "Rotate(A, α, C)"),
            ("B'", Kind::Point, "Rotate(B, α, C)"),
        ],
        points: &[("A'", 1.0, -5.0), ("B'", 2.0, -3.0)],
    },
    Scenario {
        name: "rotateByAngleToolObjectToExistingPoint",
        tool: Tool::Rotate,
        script: "U = (1,-1)\nV = (2,-2)\nW = (1,-2)\nt1 = Polygon(U, V, W)\nA = (3,-3)\n",
        drags: &[],
        clicks: &[(60, 80), (150, 150)],
        number: Some("270°"),
        expect: &[
            ("A", Kind::Point, "(3, -3)"),
            ("α", Kind::Number, "4.71238898038469"),
            ("t1'", Kind::Polygon, "Rotate(t1, α, A)"),
        ],
        points: &[],
    },
    Scenario {
        name: "rotateByAngleToolMultipleObjectsToNewPoint",
        tool: Tool::Rotate,
        script: "E1 = (1,-1)\nE2 = (3,-1)\nr1 = 0.5\nc = Circle(E1, r1)\nd = Circle(E2, r1)\nA = (2,-2)\n",
        drags: &[((10, 10), (190, 80))],
        clicks: &[(100, 100)],
        number: Some("180°"),
        expect: &[
            ("c", Kind::Circle, "Circle(E1, r1)"),
            ("d", Kind::Circle, "Circle(E2, r1)"),
            ("A", Kind::Point, "(2, -2)"),
            ("α", Kind::Number, "3.141592653589793"),
            ("c'", Kind::Circle, "Rotate(c, α, A)"),
            ("d'", Kind::Circle, "Rotate(d, α, A)"),
        ],
        points: &[],
    },
];

/// Runs a scenario and reports every difference from its expectations.
fn run(s: &Scenario, h: &mut Harness<'_, App>) -> Vec<String> {
    h.state_mut().tool.set_tool(s.tool);
    for &((x1, y1), (x2, y2)) in s.drags {
        let (from, to) = (px(x1, y1), px(x2, y2));
        drag(h, world(from.0, from.1), world(to.0, to.1));
    }
    for &(x, y) in s.clicks {
        click(h, world(px(x, y).0, px(x, y).1));
    }
    if let Some(text) = s.number {
        assert!(
            h.state().tool.asking().is_some(),
            "{}: the number dialog did not open",
            s.name
        );
        h.state_mut().number_text = text.into();
        h.key_press(egui::Key::Enter);
        h.run();
    }
    let mut diff = Vec::new();
    let app = h.state();
    let c = app.wb.construction();
    let labels: Vec<String> = c.ids().map(|id| c.label(id).to_owned()).collect();
    let wanted: Vec<&str> = s.expect.iter().map(|(l, _, _)| *l).collect();
    if labels.iter().map(String::as_str).collect::<Vec<_>>() != wanted {
        diff.push(format!(
            "{}: objects are {labels:?}, expected {wanted:?}",
            s.name
        ));
    }
    for (label, kind, def) in s.expect {
        let Some(id) = c.id(label) else {
            continue; // reported by the label list above
        };
        let name = s.name;
        if c.kind(id) != *kind {
            diff.push(format!(
                "{name}: `{label}` is a {}, expected {kind:?}",
                c.kind(id)
            ));
        }
        let actual = app.wb.definition(label);
        if actual != Some(*def) {
            diff.push(format!("{name}: `{label}` is {actual:?}, expected `{def}`"));
        }
    }
    for &(label, x, y) in s.points {
        let Some(id) = c.id(label) else {
            continue;
        };
        let name = s.name;
        let Value::Point { x: ax, y: ay } = c.value(id) else {
            diff.push(format!("{name}: `{label}` is not a point"));
            continue;
        };
        if (ax - x).abs() > 1e-9 || (ay - y).abs() > 1e-9 {
            diff.push(format!(
                "{name}: `{label}` is ({ax}, {ay}), expected ({x}, {y})",
                name = s.name
            ));
        }
    }
    diff
}

/// The gaps file: `[[gap]]` entries name a ported scenario that must keep
/// disagreeing (stale when it passes); `[[missing]]` entries list the
/// upstream methods whose tool the window does not have at all — an audit
/// list, so a scenario is either ported, a gap, or a missing tool, never
/// silently dropped.
#[derive(serde::Deserialize)]
struct GapsFile {
    #[serde(default)]
    gap: Vec<ToolGap>,
    #[serde(default)]
    missing: Vec<MissingTool>,
}

#[derive(serde::Deserialize)]
struct ToolGap {
    id: String,
    #[allow(dead_code)]
    category: String,
    reason: String,
}

#[derive(serde::Deserialize)]
struct MissingTool {
    #[allow(dead_code)]
    method: String,
    #[allow(dead_code)]
    tool: String,
    #[allow(dead_code)]
    reason: String,
}

const GAPS_FILE: &str = include_str!("upstream-tool-gaps.toml");

#[test]
fn ported_controller_scenarios_hold() {
    let gaps: GapsFile = toml::from_str(GAPS_FILE).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        !gaps.missing.is_empty(),
        "the missing-tool audit list must name the tools that are absent"
    );
    let mut failures = Vec::new();
    for s in SCENARIOS {
        let listed = gaps.gap.iter().find(|g| g.id == s.name);
        // A scenario may hit a bug that panics the window (a stale id in
        // the paint); that counts as a difference too, silently.
        let mut h = harness(s.script);
        let quiet = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let diff = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(s, &mut h)))
            .unwrap_or_else(|_| vec![format!("{}: the scenario panics the window", s.name)]);
        std::panic::set_hook(quiet);
        match listed {
            None => failures.extend(diff),
            Some(gap) => {
                if diff.is_empty() {
                    failures.push(format!(
                        "stale tool gap `{}` ({}): the scenario now passes, remove the entry",
                        gap.id, gap.reason
                    ));
                }
            }
        }
    }
    for gap in &gaps.gap {
        if !SCENARIOS.iter().any(|s| s.name == gap.id) {
            failures.push(format!(
                "tool gap `{}` matches no ported scenario; either port it or \
                 move it to the missing-tools section",
                gap.id
            ));
        }
    }
    let passing = SCENARIOS.len() - gaps.gap.len();
    println!(
        "upstream tool scenarios: {} ported, {passing} pass, {} gaps, {} upstream methods with no tool here",
        SCENARIOS.len(),
        gaps.gap.len(),
        gaps.missing.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
