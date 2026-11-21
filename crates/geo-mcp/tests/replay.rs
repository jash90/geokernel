//! Replays the observed pointer scenarios of `fixtures/pointer_scenarios.json`
//! through the headless window and compares the outcome field by field.
//!
//! The window is sized so that its graphics view is 892 x 747 points, the
//! size the observations were made at, so the standard view puts the world
//! origin where the reference put it (446, 373.5 from the view's top left
//! corner, 50 points per unit). The reference works with integer pixel
//! positions, so every event position is rounded to an integer pixel of the
//! view the way it rounded them (presses and releases are on integer pixels
//! already). Each scenario runs twice: one frame per event at the event's
//! time, and with idle frames at 1/60 s between the events (a rule that
//! polls the pointer at the end of a frame shows up only there).
//!
//! Every scenario of the fixture is named below: it belongs to a part of
//! the pointer work that has landed (it must pass), it encodes a behaviour
//! the owner keeps our way (our outcome is asserted instead, and the
//! reference app's too where a switch of `OwnerRules` gives it), or it is
//! skipped until a later group, named with its group. A fixture scenario
//! that is in no list fails the test, and the numbers replayed and skipped
//! are asserted. `undo_steps_added` is compared wherever our kept rules and
//! the reference app agree; `UNDO_DIFFERS` names the scenarios where they
//! do not.

use geo_mcp::protocol::Server;
use serde_json::{json, Value};

const FIXTURE: &str = include_str!("fixtures/pointer_scenarios.json");

/// The graphics view of the observations, and where the standard view puts
/// the world origin in it.
const VIEW: (f64, f64) = (892.0, 747.0);
const ZERO: (f64, f64) = (446.0, 373.5);
const SCALE: f64 = 50.0;

/// The landed scenarios, by group.
const LANDED: &[(u8, &[&str])] = &[
    (
        1,
        &[
            "time_gate_early_move_ignored",
            "time_gate_late_move_applied",
            "no_distance_threshold_1px",
            "early_motion_recovered",
            "press_release_on_point_selects_only",
            "pan_from_empty_space",
            "touch_pan_one_finger",
        ],
    ),
    (
        2,
        &[
            "point_wins_over_line_underneath",
            "overlapping_points_newest_wins",
            "near_points_newest_wins_not_nearest",
            "press_inside_unfilled_circle_pans",
            "drag_midpoint_refused",
            "drag_intersection_refused",
            "touch_drag_point",
        ],
    ),
    (
        3,
        &[
            "point_tool_snaps_within_box",
            "point_tool_no_snap_7px",
            "point_tool_no_per_axis_snap",
            "point_tool_ignores_minor_crossing",
            "drag_free_point_snaps_at_grid_crossing",
        ],
    ),
    (
        5,
        &[
            "drag_free_point_centre",
            "drag_free_point_offset_grab_jumps_to_pointer",
            "drag_line_translates_both_parents",
            "drag_circle_translates_centre_and_point",
            "drag_segment",
            "drag_polygon_interior",
            "drag_on_label_moves_label_only",
        ],
    ),
    (
        6,
        &[
            "tool_line_two_clicks_empty",
            "tool_segment_two_clicks_empty",
            "tool_ray_two_clicks_empty",
            "tool_vector_two_clicks_empty",
            "tool_circle_centre_point_two_clicks_empty",
            "tool_midpoint_two_clicks_empty",
            "tool_circle3_three_clicks",
            "tool_polygon_closed_on_first_vertex",
            "tool_angle_three_clicks",
            "tool_perpendicular_line_then_point",
            "tool_parallel_line_then_point",
            "tool_segment_reuses_point_within_radius",
            "point_tool_on_line_attaches",
            "point_tool_5px_off_line_is_free",
            "point_tool_on_line_crossing_intersects",
            "point_tool_on_circle_line_crossing",
            "drag_point_on_line_slides",
            "escape_mid_tool_removes_pending_point",
            "switch_tool_mid_sequence_removes_pending_point",
            "press_drag_release_makes_segment",
            "drag_existing_point_with_segment_tool",
            "drag_existing_point_with_point_tool",
            "segment_tool_twice_stays_selected",
            "delete_tool_click_point_removes_dependents",
            "delete_tool_area_drag_deletes_nothing",
            "delete_tool_stroke_erases_crossed_object",
        ],
    ),
];

/// The scenarios whose observed behaviour the owner does not follow: our
/// outcome is asserted, and the observed one with `rules` set to
/// `reference` where that switch exists.
const OURS: &[(&str, bool)] = &[
    // A press on a polygon's side drags the polygon
    // (`hit::SIDE_TAKES_THE_POLYGON`).
    ("press_on_polygon_edge_does_nothing", true),
    // A pinch zooms exactly under the fingers, with no 10 px dead band
    // (group 9 has no switch for it).
    ("pinch_spread_symmetric", false),
];

/// The scenarios skipped until a later group, with the group and why.
const SKIPPED: &[(u8, &str, &[&str])] = &[
    (
        4,
        "camera with two scales, wheel, buttons, axes, resize",
        &[
            "wheel_zoom_in_about_pointer",
            "wheel_zoom_out_about_pointer",
            "zoom_in_button_about_canvas_centre",
            "zoom_to_fit_changes_aspect",
            "move_view_tool_on_point_pans",
            "axis_drag_move_tool_pans",
            "axis_drag_move_view_tool_rescales_x",
            "axis_drag_left_of_origin_shrinks_x",
            "zoom_in_tool_click_about_click",
            "zoom_in_tool_rectangle_fills_canvas",
            "keyboard_open_keeps_centre",
            "viewport_resize_keeps_centre",
        ],
    ),
    (
        5,
        "the kernel has no lines typed as equations (`f: y=c`); a line with free \
         points would rightly move",
        &["free_equation_line_not_draggable"],
    ),
    (
        6,
        "deferred: no texts in the kernel (area tool)",
        &["area_tool_on_polygon"],
    ),
    (8, "the input row", &["enter_on_empty_input_row"]),
];

/// The scenarios whose `undo_steps_added` differs from ours because of a
/// rule the owner keeps, and that rule. Every other replayed scenario with
/// the field must match it.
const UNDO_DIFFERS: &[(&str, &str)] = &[(
    "pan_from_empty_space",
    "a pan is an undo step for the reference and the owner alike (spec F3), but the \
     camera joins the undo state only with group 4, so nothing is recorded yet",
)];

/// How a fixture scenario is treated.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Part {
    Landed(u8),
    Ours { has_switch: bool },
    Skipped(u8, &'static str),
}

fn part(name: &str) -> Option<Part> {
    for &(g, names) in LANDED {
        if names.contains(&name) {
            return Some(Part::Landed(g));
        }
    }
    for &(n, has_switch) in OURS {
        if n == name {
            return Some(Part::Ours { has_switch });
        }
    }
    for &(g, why, names) in SKIPPED {
        if names.contains(&name) {
            return Some(Part::Skipped(g, why));
        }
    }
    None
}

fn rpc(s: &mut Server, method: &str, params: Value) -> Value {
    let line = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).to_string();
    let reply = s.handle_line(&line).expect("a request gets a reply");
    serde_json::from_str(&reply).expect("the reply is JSON")
}

fn call(s: &mut Server, name: &str, args: Value) -> Result<Value, String> {
    let r = rpc(s, "tools/call", json!({"name": name, "arguments": args}));
    let result = &r["result"];
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    if result["isError"] != false {
        return Err(format!("{name}: {text}"));
    }
    Ok(serde_json::from_str(text).expect("a tool result is JSON"))
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

/// Exact outcomes are compared to 1e-9, relative to the larger magnitude
/// (at least 1).
fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

/// The window of the replay: the top left corner of its graphics view.
struct Window {
    server: Server,
    view_min: (f64, f64),
}

impl Window {
    /// A window whose graphics view is exactly `VIEW`, with `script` loaded.
    fn open(script: &str, profile: &str, rules: &str) -> Result<Window, String> {
        let mut server = Server::new();
        call(&mut server, "reset", json!({"profile": profile}))?;
        let graphics = |st: &Value| -> Result<[f64; 4], String> {
            let r = &st["regions"]["graphics"];
            Ok([num(&r[0]), num(&r[1]), num(&r[2]), num(&r[3])])
        };
        let st = call(&mut server, "state", json!({"verbose": true}))?;
        let [_, _, gw, gh] = graphics(&st)?;
        let (w, h) = (num(&st["window"]["width"]), num(&st["window"]["height"]));
        let size = (w - gw + VIEW.0, h - gh + VIEW.1);
        call(
            &mut server,
            "reset",
            json!({
                "width": size.0, "height": size.1, "profile": profile, "script": script,
                "rules": rules
            }),
        )?;
        let st = call(&mut server, "state", json!({"verbose": true}))?;
        let [gx, gy, gw, gh] = graphics(&st)?;
        assert_eq!((gw, gh), VIEW, "the graphics view of the replay window");
        let origin = &st["view"]["origin"];
        assert!(
            same(num(&origin[0]), gx + ZERO.0) && same(num(&origin[1]), gy + ZERO.1),
            "the standard view puts the origin at {origin}, not at {ZERO:?} in the view"
        );
        assert_eq!(num(&st["view"]["scale"]), SCALE);
        Ok(Window {
            server,
            view_min: (gx, gy),
        })
    }

    /// The window position of the world point `(x, y)` in the standard
    /// view, rounded to an integer pixel; `exact` asks for a position that
    /// needs no rounding (a press or a release).
    fn at(&self, pos: &Value, exact: bool) -> Result<Value, String> {
        let (u, v) = (ZERO.0 + num(&pos[0]) * SCALE, ZERO.1 - num(&pos[1]) * SCALE);
        if exact && ((u - u.round()).abs() > 1e-6 || (v - v.round()).abs() > 1e-6) {
            return Err(format!("{pos} is not on an integer pixel ({u}, {v})"));
        }
        Ok(json!({"x": self.view_min.0 + u.round(), "y": self.view_min.1 + v.round()}))
    }

    fn state(&mut self) -> Value {
        call(&mut self.server, "state", json!({})).expect("state")
    }
}

fn tool_name(mode: i64) -> Option<&'static str> {
    Some(match mode {
        0 => "move",
        1 => "point",
        2 => "line",
        3 => "parallel",
        4 => "perpendicular",
        5 => "intersect",
        6 => "delete",
        7 => "vector",
        10 => "circle",
        11 => "circle3",
        15 => "segment",
        16 => "polygon",
        18 => "ray",
        19 => "midpoint",
        36 => "angle",
        40 => "move_view",
        41 => "zoom_in",
        42 => "zoom_out",
        _ => return None,
    })
}

/// The frames of one scenario event (the pointer events only). `fingers`
/// holds the last position of each finger: a lift that names no position
/// lifts where the finger last was.
fn frame_events(
    w: &Window,
    e: &Value,
    fingers: &mut Vec<(u64, Value)>,
) -> Result<Vec<Value>, String> {
    let kind = e["type"].as_str().unwrap_or_default();
    let ev = |t: &str, at: Value| {
        let mut at = at;
        at["type"] = json!(t);
        at
    };
    Ok(match kind {
        "mouse_down" => {
            let at = w.at(&e["pos"], true)?;
            vec![ev("move", at.clone()), ev("down", at)]
        }
        "mouse_move" => vec![ev("move", w.at(&e["pos"], false)?)],
        // The release is not a move: its position goes with the release.
        "mouse_up" => vec![ev("up", w.at(&e["pos"], true)?)],
        "touch_down" | "touch_move" | "touch_up" => {
            let phase = match kind {
                "touch_down" => "start",
                "touch_move" => "move",
                _ => "end",
            };
            // One finger unless the observation names several.
            let id = e["id"].as_u64().unwrap_or(0);
            let at = match e.get("pos") {
                Some(pos) => w.at(pos, kind != "touch_move")?,
                None => fingers
                    .iter()
                    .find(|(f, _)| *f == id)
                    .map(|(_, at)| at.clone())
                    .ok_or(format!("finger {id} lifts with no position"))?,
            };
            fingers.retain(|(f, _)| *f != id);
            if kind != "touch_up" {
                fingers.push((id, at.clone()));
            }
            let mut t = ev("touch", at);
            t["phase"] = json!(phase);
            t["id"] = json!(id);
            vec![t]
        }
        "key" => {
            let key = e["key"].as_str().ok_or("a key event names no key")?;
            vec![json!({"type": "key", "key": key})]
        }
        other => return Err(format!("event `{other}` is not replayed yet")),
    })
}

/// The kernel has no lines or circles typed as equations: `f: y=c` and
/// `g: x=c` become the line through two points of it near the edges of the
/// view (where no press lands; points outside it would make the first frame
/// fit the view), `c: x^2+y^2=r2` the circle about
/// a free origin with the radius as a number. They stand in only where the
/// object is not what the scenario drags; their helper objects are not
/// counted as objects of the scenario (`HELPERS`).
fn equation_line(line: &str) -> String {
    if let Some((name, c)) = line.split_once(": y=") {
        return format!(
            "{name}_{{a}} = $point(-8, {c})\n{name}_{{b}} = $point(8, {c})\n\
             {name} = Line({name}_{{a}}, {name}_{{b}})"
        );
    }
    if let Some((name, c)) = line.split_once(": x=") {
        return format!(
            "{name}_{{a}} = $point({c}, -7)\n{name}_{{b}} = $point({c}, 7)\n\
             {name} = Line({name}_{{a}}, {name}_{{b}})"
        );
    }
    if let Some((name, r2)) = line.split_once(": x^2+y^2=") {
        let r = r2.parse::<f64>().expect("a squared radius").sqrt();
        return format!(
            "{name}_{{a}} = $point(0, 0)\n{name}_{{b}} = {r}\n\
             {name} = Circle({name}_{{a}}, {name}_{{b}})"
        );
    }
    line.to_owned()
}

/// Whether `label` is a helper object of a stand-in (see `equation_line`).
fn helper(label: &str) -> bool {
    label.ends_with("_{a}") || label.ends_with("_{b}")
}

/// The labels of the scenario's objects, helpers left out, in order.
fn scenario_labels(st: &Value) -> Vec<String> {
    st["objects"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|o| o["name"].as_str())
                .filter(|l| !helper(l))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// A side of a polygon, `Segment(P, Q, poly)` in the reference (`d`):
/// ours is an output of the polygon's line and has that line's definition.
/// What differs when the side `o` does not run from `P` to `Q` with that
/// definition.
fn side_differs(d: &str, o: &Value, st: &Value) -> Option<String> {
    let args = d.strip_prefix("Segment(").and_then(|a| a.strip_suffix(')'));
    let parts: Vec<&str> = args.map_or(Vec::new(), |a| a.split(", ").collect());
    let [p, q, poly] = parts[..] else {
        return Some(format!("{d} is no side"));
    };
    let at =
        |label: &str| object(st, label).map(|o| [num(&o["coords"]["x"]), num(&o["coords"]["y"])]);
    let c = &o["coords"];
    let ends = [
        Some([num(&c["x1"]), num(&c["y1"])]),
        Some([num(&c["x2"]), num(&c["y2"])]),
    ];
    let polygon = object(st, poly).map(|o| &o["definition"]);
    (ends != [at(p), at(q)] || Some(&o["definition"]) != polygon)
        .then(|| format!("a side {c} defined as {} instead of {d}", o["definition"]))
}

/// A definition in our command names: the tools write `Perpendicular` and
/// `Parallel` where the reference writes `OrthogonalLine` and `Line` with
/// a point and a line (the kernel takes both spellings).
fn our_definition(d: &str, st: &Value) -> String {
    if let Some(args) = d.strip_prefix("OrthogonalLine(") {
        return format!("Perpendicular({args}");
    }
    if let Some(args) = d.strip_prefix("Line(").and_then(|a| a.strip_suffix(')')) {
        if let Some((p, l)) = args.split_once(", ") {
            let kind = |label: &str| object(st, label).map(|o| o["type"].clone());
            if kind(p) == Some(json!("point")) && kind(l) == Some(json!("line")) {
                return format!("Parallel({p}, {l})");
            }
        }
    }
    d.to_owned()
}

/// What a replay ended with: the state before and after, how many undo
/// steps it added, and the differences already found mid-way.
struct Outcome {
    before: Value,
    after: Value,
    origin0: (f64, f64),
    undo_added: i64,
    diffs: Vec<String>,
}

fn undo_depth(st: &Value) -> i64 {
    st["undo"]["undo_depth"].as_i64().expect("undo depth")
}

/// Runs the scenario with `rules` (`ours` or `reference`); `idle` adds
/// empty frames at 1/60 s between events.
fn replay(s: &Value, idle: bool, rules: &str) -> Result<Outcome, String> {
    let script: Vec<String> = s["initial_script"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        // Every label is shown here: the command that shows one is moot.
        .filter(|l| !l.starts_with("ShowLabel("))
        .map(equation_line)
        .collect();
    let touch = s["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["type"].as_str().is_some_and(|t| t.starts_with("touch")));
    let profile = if touch { "touch" } else { "desktop" };
    let mut w = Window::open(&(script.join("\n") + "\n"), profile, rules)?;
    let before = w.state();
    let origin0 = (
        num(&before["view"]["origin"][0]),
        num(&before["view"]["origin"][1]),
    );
    let mode = s["tool_mode"].as_i64().unwrap_or(0);
    if mode != 0 {
        let tool = tool_name(mode).ok_or(format!("tool mode {mode}"))?;
        call(&mut w.server, "select_tool", json!({ "tool": tool }))?;
    }
    let depth0 = undo_depth(&w.state());
    let events = s["events"].as_array().unwrap();
    let expected = &s["expected"];
    let mut diffs = Vec::new();
    let mut last_t = None::<f64>;
    let mut frames = Vec::new();
    let mut fingers = Vec::new();
    for (i, e) in events.iter().enumerate() {
        let t = num(&e["t"]) / 1000.0;
        let dt = match last_t {
            None => 1.0 / 60.0,
            Some(prev) => {
                let mut dt = t - prev;
                if idle {
                    while dt > 1.5 / 60.0 {
                        frames.push(json!({"dt": 1.0 / 60.0, "events": []}));
                        dt -= 1.0 / 60.0;
                    }
                }
                dt
            }
        };
        last_t = Some(t);
        if e["type"] == "select_tool" {
            // A tool chosen in the toolbar between the pointer events.
            frames.push(json!({"dt": dt, "events": []}));
            call(
                &mut w.server,
                "input",
                json!({"frames": std::mem::take(&mut frames), "settle": false}),
            )?;
            let mode = e["mode"].as_i64().unwrap_or(-1);
            let tool = tool_name(mode).ok_or(format!("tool mode {mode}"))?;
            call(&mut w.server, "select_tool", json!({ "tool": tool }))?;
            continue;
        }
        frames.push(json!({"dt": dt, "events": frame_events(&w, e, &mut fingers)?}));
        let after_1 = ["objects_after_event_1", "after_first_click"];
        if i == 1 && after_1.iter().any(|k| expected.get(k).is_some()) {
            call(
                &mut w.server,
                "input",
                json!({"frames": std::mem::take(&mut frames), "settle": false}),
            )?;
            let st = w.state();
            if let Some(objects) = expected.get("objects_after_event_1") {
                compare_objects(&st, objects, "after event 1", &mut diffs);
            }
            if let Some(created) = expected.get("after_first_click") {
                compare_created(&before, &st, created, "after the first click", &mut diffs);
            }
        }
    }
    call(
        &mut w.server,
        "input",
        json!({"frames": frames, "settle": true}),
    )?;
    let after = w.state();
    let undo_added = undo_depth(&after) - depth0;
    Ok(Outcome {
        before,
        after,
        origin0,
        undo_added,
        diffs,
    })
}

/// The differences of `out` from `expected` (the observed outcome, or ours
/// in its format). `undo_steps_added` is compared only when `undo` is set.
fn compare(expected: &Value, out: &Outcome, undo: bool) -> Vec<String> {
    let (before, st) = (&out.before, &out.after);
    let mut diffs = out.diffs.clone();
    if let Some(objects) = expected.get("objects") {
        compare_objects(st, objects, "", &mut diffs);
        // No object listed: the scenario ends with no objects at all.
        if objects.as_object().is_some_and(|o| o.is_empty()) && !scenario_labels(st).is_empty() {
            diffs.push(format!("objects {:?} instead of none", scenario_labels(st)));
        }
    }
    if let Some(remaining) = expected.get("remaining") {
        let want: Vec<&str> = remaining
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        if scenario_labels(st) != want {
            diffs.push(format!(
                "{:?} remain instead of {want:?}",
                scenario_labels(st)
            ));
        }
    }
    if let Some(mode) = expected.get("tool_mode_after") {
        let mode = mode.as_i64().unwrap();
        let want = tool_name(mode).unwrap_or("?");
        if st["tool"]["id"] != want {
            diffs.push(format!("tool {} after instead of {want}", st["tool"]["id"]));
        }
    }
    if let Some(labels) = expected.get("created_labels") {
        let want: Vec<&str> = labels
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let got = created_labels(before, st);
        if got != want {
            diffs.push(format!("created {got:?} instead of {want:?}"));
        }
    }
    if let Some(sel) = expected.get("selected") {
        // Every selected object, in any order (ours selects at most one).
        let mut want: Vec<&str> = sel
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let mut got: Vec<&str> = st["selected"].as_str().into_iter().collect();
        want.sort_unstable();
        got.sort_unstable();
        if got != want {
            diffs.push(format!("selected {got:?} instead of {want:?}"));
        }
    }
    if let Some(view) = expected.get("view") {
        // Ours may name a tolerance for a view egui computes in `f32`.
        let tol = view.get("tolerance").map_or(0.0, num);
        let same = |a: f64, b: f64| same(a, b) || (a - b).abs() <= tol * b.abs().max(1.0);
        let origin = (num(&st["view"]["origin"][0]), num(&st["view"]["origin"][1]));
        let zero = (
            ZERO.0 + origin.0 - out.origin0.0,
            ZERO.1 + origin.1 - out.origin0.1,
        );
        if view.get("xZero").is_some()
            && (!same(zero.0, num(&view["xZero"])) || !same(zero.1, num(&view["yZero"])))
        {
            diffs.push(format!(
                "view origin at {zero:?} instead of ({}, {})",
                view["xZero"], view["yZero"]
            ));
        }
        if !same(num(&st["view"]["scale"]), num(&view["scale"])) {
            diffs.push(format!(
                "scale {} instead of {}",
                st["view"]["scale"], view["scale"]
            ));
        }
    }
    if let Some(created) = expected.get("created") {
        compare_created(before, st, created, "", &mut diffs);
    }
    if let Some(lines) = expected.get("line_implicit") {
        for (label, want) in lines.as_object().unwrap() {
            let Some(o) = object(st, label) else {
                diffs.push(format!("no {label}"));
                continue;
            };
            let c = &o["coords"];
            // Ours is a x + b y + c = 0, the observation a x + b y = c.
            let (a, b, k) = (num(&c["a"]), num(&c["b"]), -num(&c["c"]));
            let (wa, wb, wk) = (num(&want[0]), num(&want[1]), num(&want[2]));
            let f = wb / b;
            if !same(a * f, wa) || !same(b * f, wb) || !same(k * f, wk) {
                diffs.push(format!("{label} is {a} x + {b} y = {k}, not {want}"));
            }
        }
    }
    if let Some(labels) = expected.get("unchanged") {
        for label in labels.as_array().unwrap() {
            let label = label.as_str().unwrap();
            let (a, b) = (object(before, label), object(st, label));
            // An object the scenario made (the angle of
            // `tool_angle_three_clicks`) has no earlier state; `created`
            // checks it.
            if a.is_some() && a.map(|o| &o["coords"]) != b.map(|o| &o["coords"]) {
                diffs.push(format!("{label} changed"));
            }
        }
    }
    if let Some(offsets) = expected.get("label_offset_px") {
        for (label, want) in offsets.as_object().unwrap() {
            let got = object(st, label).map(|o| o["label_offset"].clone());
            let ok = got.as_ref().is_some_and(|g| {
                same(num(&g[0]), num(&want[0])) && same(num(&g[1]), num(&want[1]))
            });
            if !ok {
                diffs.push(format!("label offset of {label} is {got:?}, not {want}"));
            }
        }
    }
    if let (true, Some(n)) = (undo, expected.get("undo_steps_added")) {
        if out.undo_added != n.as_i64().unwrap() {
            diffs.push(format!(
                "{} undo steps added instead of {n}",
                out.undo_added
            ));
        }
    }
    diffs
}

/// The labels of the scenario objects that `st` has and `before` had not,
/// in order.
fn created_labels(before: &Value, st: &Value) -> Vec<String> {
    let old = scenario_labels(before);
    scenario_labels(st)
        .into_iter()
        .filter(|l| !old.contains(l))
        .collect()
}

/// Compares the objects created since `before` with `created`: exactly
/// these labels (in any order), each at its point or with its definition.
fn compare_created(
    before: &Value,
    st: &Value,
    created: &Value,
    when: &str,
    diffs: &mut Vec<String>,
) {
    let created = created.as_array().unwrap();
    let mut want: Vec<&str> = created.iter().filter_map(|c| c["label"].as_str()).collect();
    let mut got = created_labels(before, st);
    want.sort_unstable();
    got.sort_unstable();
    if got != want {
        diffs.push(format!("created {got:?} instead of {want:?} {when}"));
    }
    for c in created {
        let label = c["label"].as_str().unwrap();
        let Some(o) = object(st, label) else {
            continue;
        };
        if let Some(p) = c.get("point") {
            let (x, y) = (num(&o["coords"]["x"]), num(&o["coords"]["y"]));
            if !same(x, num(&p[0])) || !same(y, num(&p[1])) {
                diffs.push(format!(
                    "{label} created at ({x}, {y}) instead of {p} {when}"
                ));
            }
        }
        if let Some(d) = c.get("definition").and_then(Value::as_str) {
            if d.starts_with("Segment(") && d.matches(", ").count() == 2 {
                if let Some(diff) = side_differs(d, o, st) {
                    diffs.push(format!("{label}: {diff} {when}"));
                }
                continue;
            }
            let want = our_definition(d, st);
            if o["definition"] != want.as_str() {
                diffs.push(format!(
                    "{label} defined as {} instead of {want} {when}",
                    o["definition"]
                ));
            }
        }
    }
}

fn object<'a>(st: &'a Value, label: &str) -> Option<&'a Value> {
    st["objects"]
        .as_array()?
        .iter()
        .find(|o| o["name"] == label)
}

fn compare_objects(st: &Value, want: &Value, when: &str, diffs: &mut Vec<String>) {
    for (label, p) in want.as_object().unwrap() {
        let Some(o) = object(st, label) else {
            diffs.push(format!("no {label}"));
            continue;
        };
        let (x, y) = (num(&o["coords"]["x"]), num(&o["coords"]["y"]));
        if !same(x, num(&p[0])) || !same(y, num(&p[1])) {
            diffs.push(format!("{label} at ({x}, {y}) instead of {p} {when}"));
        }
    }
}

/// Our outcome, in the fixture's format, for a scenario the owner keeps
/// our way.
fn ours_expected(name: &str) -> Value {
    match name {
        // The press at (3, 1.01) is captured to the crossing (3, 1); the
        // polygon follows the pointer to (4.2, 0.21) by its free vertices
        // (`moveDependent`), as one undo step, and is selected.
        "press_on_polygon_edge_does_nothing" => json!({
            "objects": {"A": [2.2, 0.22], "B": [6.2, 0.22], "C": [2.2, 3.22]},
            "selected": ["p"],
            "view": {"xZero": 446, "yZero": 373.5, "scale": 50},
            "undo_steps_added": 1
        }),
        // The fingers spread from 200 to 300 px about the fixed midpoint
        // (-2.68, 2.53), drawn at (312, 247): the scale goes from 50 to 75
        // and that point stays under the fingers (the reference app lags
        // by its 10 px dead band: 74.25). egui measures the fingers in
        // `f32`: within 1e-4 (0.05 px at the origin).
        "pinch_spread_symmetric" => json!({
            "objects": {"A": [1, 1.01]},
            "view": {"xZero": 513, "yZero": 436.75, "scale": 75, "tolerance": 1e-4},
            "undo_steps_added": 0
        }),
        other => panic!("no outcome of ours for {other}"),
    }
}

#[test]
fn the_observed_pointer_scenarios_replay() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let mut table = Vec::new();
    let mut failed = Vec::new();
    let (mut replayed, mut ours) = (0, 0);
    let mut skipped = std::collections::BTreeMap::<u8, usize>::new();
    let undo_differs = std::cell::RefCell::new(Vec::new());
    // Both frame pacings; the differences of the second are marked.
    let both = |s: &Value, rules: &str, run: &dyn Fn(&Outcome) -> Vec<String>| {
        let mut diffs = Vec::new();
        for idle in [false, true] {
            match replay(s, idle, rules) {
                Ok(out) => diffs.extend(run(&out).into_iter().map(|d| {
                    if idle {
                        format!("{d} (with idle frames)")
                    } else {
                        d
                    }
                })),
                Err(e) => diffs.push(e),
            }
        }
        diffs
    };
    for s in fixture["scenarios"].as_array().unwrap() {
        let name = s["name"].as_str().unwrap();
        let expected = &s["expected"];
        let status = match part(name) {
            None => {
                failed.push(name.to_owned());
                "FAIL: in no list (landed, ours or skipped)".to_owned()
            }
            Some(Part::Skipped(g, why)) => {
                *skipped.entry(g).or_default() += 1;
                format!("skipped (group {g}: {why})")
            }
            Some(Part::Landed(g)) => {
                replayed += 1;
                let differs = UNDO_DIFFERS.iter().any(|(n, _)| *n == name);
                let diffs = both(s, "ours", &|out| {
                    let mut d = compare(expected, out, !differs);
                    if differs {
                        let want = expected["undo_steps_added"].as_i64();
                        if want == Some(out.undo_added) {
                            d.push("undo steps now agree: drop it from UNDO_DIFFERS".into());
                        } else {
                            undo_differs.borrow_mut().push(name.to_owned());
                        }
                    }
                    d
                });
                if diffs.is_empty() {
                    format!("pass (group {g})")
                } else {
                    failed.push(name.to_owned());
                    format!("FAIL (group {g}): {}", diffs.join("; "))
                }
            }
            Some(Part::Ours { has_switch }) => {
                ours += 1;
                let mine = ours_expected(name);
                let mut diffs = both(s, "ours", &|out| compare(&mine, out, true));
                if has_switch {
                    diffs.extend(
                        both(s, "reference", &|out| compare(expected, out, true))
                            .into_iter()
                            .map(|d| format!("{d} (reference rules)")),
                    );
                }
                let how = if has_switch {
                    "ours asserted, the observed one with the reference rules"
                } else {
                    "ours asserted, no switch"
                };
                if diffs.is_empty() {
                    format!("pass (owner keeps ours: {how})")
                } else {
                    failed.push(name.to_owned());
                    format!("FAIL (owner keeps ours): {}", diffs.join("; "))
                }
            }
        };
        table.push(format!("{name}: {status}"));
    }
    println!("{}", table.join("\n"));
    assert!(
        failed.is_empty(),
        "failed: {failed:?}\n{}",
        table.join("\n")
    );
    // Every listed scenario is in the fixture, once.
    let names: Vec<&str> = fixture["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    let listed = LANDED
        .iter()
        .flat_map(|(_, n)| n.iter())
        .chain(OURS.iter().map(|(n, _)| n))
        .chain(SKIPPED.iter().flat_map(|(_, _, n)| n.iter()));
    let mut count = 0;
    for n in listed {
        assert_eq!(names.iter().filter(|m| *m == n).count(), 1, "{n}");
        count += 1;
    }
    assert_eq!(count, names.len());
    assert_eq!((replayed, ours), (52, 2), "scenarios replayed");
    assert_eq!(
        skipped.into_iter().collect::<Vec<_>>(),
        [(4, 12), (5, 1), (6, 1), (8, 1)],
        "scenarios skipped, by group"
    );
    // Each pacing records the difference once.
    let want: Vec<String> = UNDO_DIFFERS
        .iter()
        .flat_map(|(n, _)| [n.to_string(), n.to_string()])
        .collect();
    assert_eq!(undo_differs.into_inner(), want, "undo steps that differ");
}
