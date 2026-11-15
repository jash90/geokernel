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
//! A scenario either belongs to a part of the pointer work that has landed
//! (it must pass), or it is not applicable yet: it needs a later part (the
//! camera with two scales, tools as one transaction, drag and view undo,
//! algebra rows, pinch). Fields that belong to a later part are left out
//! of the comparison: `undo_steps_added` (drag and tool undo) and the tool
//! fields of construction tools.

use geo_mcp::protocol::Server;
use serde_json::{json, Value};

const FIXTURE: &str = include_str!("fixtures/pointer_scenarios.json");

/// The graphics view of the observations, and where the standard view puts
/// the world origin in it.
const VIEW: (f64, f64) = (892.0, 747.0);
const ZERO: (f64, f64) = (446.0, 373.5);
const SCALE: f64 = 50.0;
/// The parts of the pointer work that have landed: their scenarios must pass.
const LANDED: &[u8] = &[1, 2];

/// Why a scenario that encodes a behaviour the owner keeps our way does not
/// apply.
const OURS: &str = "owner keeps ours";

/// Which part of the pointer work a scenario checks, or why it does not
/// apply yet.
fn part(name: &str) -> Result<u8, &'static str> {
    Ok(match name {
        "time_gate_early_move_ignored"
        | "time_gate_late_move_applied"
        | "no_distance_threshold_1px"
        | "early_motion_recovered"
        | "press_release_on_point_selects_only"
        | "pan_from_empty_space"
        | "touch_pan_one_finger" => 1,
        "point_wins_over_line_underneath"
        | "overlapping_points_newest_wins"
        | "near_points_newest_wins_not_nearest"
        | "press_inside_unfilled_circle_pans"
        | "drag_midpoint_refused"
        | "drag_intersection_refused"
        | "touch_drag_point" => 2,
        "point_tool_snaps_within_box"
        | "point_tool_no_snap_7px"
        | "point_tool_no_per_axis_snap"
        | "point_tool_ignores_minor_crossing"
        | "drag_free_point_snaps_at_grid_crossing" => 3,
        "drag_free_point_centre"
        | "drag_free_point_offset_grab_jumps_to_pointer"
        | "drag_line_translates_both_parents"
        | "drag_circle_translates_centre_and_point"
        | "drag_segment"
        | "drag_polygon_interior"
        | "drag_on_label_moves_label_only" => 5,
        "free_equation_line_not_draggable" => {
            return Err("the kernel has no lines typed as equations (`f: y=c`); \
                        a line with free points would rightly move")
        }
        "drag_point_on_line_slides" => {
            return Err("group 6: needs points on objects (a point on a line slides along it)")
        }
        "wheel_zoom_in_about_pointer"
        | "wheel_zoom_out_about_pointer"
        | "zoom_in_button_about_canvas_centre"
        | "zoom_to_fit_changes_aspect"
        | "move_view_tool_on_point_pans"
        | "axis_drag_move_tool_pans"
        | "axis_drag_move_view_tool_rescales_x"
        | "axis_drag_left_of_origin_shrinks_x"
        | "zoom_in_tool_click_about_click"
        | "zoom_in_tool_rectangle_fills_canvas"
        | "keyboard_open_keeps_centre"
        | "viewport_resize_keeps_centre" => {
            return Err("group 4: camera with two scales, wheel, buttons, axes, resize")
        }
        // The owner keeps our behaviour where following the reference
        // literally is a bad idea for a native app: a press on a polygon's
        // side drags the polygon, and a pinch zooms exactly under the
        // fingers (no 10 px dead band).
        "press_on_polygon_edge_does_nothing" => return Err(OURS),
        "pinch_spread_symmetric" => return Err(OURS),
        "enter_on_empty_input_row" => return Err("group 8: the input row"),
        "area_tool_on_polygon" => return Err("deferred: no texts in the kernel (area tool)"),
        _ => return Err("group 6: construction tools as one transaction"),
    })
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
    fn open(script: &str, profile: &str) -> Result<Window, String> {
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
            json!({"width": size.0, "height": size.1, "profile": profile, "script": script}),
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

/// The frames of one scenario event (the pointer events only).
fn frame_events(w: &Window, e: &Value) -> Result<Vec<Value>, String> {
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
            let exact = kind != "touch_move";
            let mut t = ev("touch", w.at(&e["pos"], exact)?);
            t["phase"] = json!(phase);
            // One finger unless the observation names several.
            t["id"] = json!(e["id"].as_u64().unwrap_or(0));
            vec![t]
        }
        other => return Err(format!("event `{other}` is not replayed yet")),
    })
}

/// The kernel has no lines typed as equations: `f: y=c` becomes the line
/// through two points of it near the edges of the view (where no press
/// lands; points outside it would make the first frame fit the view).
/// It stands in only where the line is not what the scenario drags.
fn equation_line(line: &str) -> String {
    match line.split_once(": y=") {
        Some((name, c)) => {
            format!(
                "{name}_{{a}} = $point(-8, {c})\n{name}_{{b}} = $point(8, {c})\n\
                 {name} = Line({name}_{{a}}, {name}_{{b}})"
            )
        }
        None => line.to_owned(),
    }
}

/// Runs the scenario; `idle` adds empty frames at 1/60 s between events.
/// Returns the differences from the observed outcome.
fn replay(s: &Value, idle: bool) -> Result<Vec<String>, String> {
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
    let mut w = Window::open(&(script.join("\n") + "\n"), profile)?;
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
    let events = s["events"].as_array().unwrap();
    let expected = &s["expected"];
    let mut diffs = Vec::new();
    let mut last_t = None::<f64>;
    let mut frames = Vec::new();
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
        frames.push(json!({"dt": dt, "events": frame_events(&w, e)?}));
        if i == 1 && expected.get("objects_after_event_1").is_some() {
            call(
                &mut w.server,
                "input",
                json!({"frames": std::mem::take(&mut frames), "settle": false}),
            )?;
            let st = w.state();
            compare_objects(
                &st,
                &expected["objects_after_event_1"],
                "after event 1",
                &mut diffs,
            );
        }
    }
    call(
        &mut w.server,
        "input",
        json!({"frames": frames, "settle": true}),
    )?;
    let st = w.state();
    if let Some(objects) = expected.get("objects") {
        compare_objects(&st, objects, "", &mut diffs);
    }
    if let Some(sel) = expected.get("selected") {
        let want = sel
            .as_array()
            .unwrap()
            .first()
            .cloned()
            .unwrap_or(Value::Null);
        if st["selected"] != want {
            diffs.push(format!("selected {} instead of {want}", st["selected"]));
        }
    }
    if let Some(view) = expected.get("view") {
        let origin = (num(&st["view"]["origin"][0]), num(&st["view"]["origin"][1]));
        let zero = (ZERO.0 + origin.0 - origin0.0, ZERO.1 + origin.1 - origin0.1);
        if !same(zero.0, num(&view["xZero"])) || !same(zero.1, num(&view["yZero"])) {
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
        for c in created.as_array().unwrap() {
            let label = c["label"].as_str().unwrap();
            let Some(o) = object(&st, label) else {
                diffs.push(format!("{label} was not created"));
                continue;
            };
            if let Some(p) = c.get("point") {
                let (x, y) = (num(&o["coords"]["x"]), num(&o["coords"]["y"]));
                if !same(x, num(&p[0])) || !same(y, num(&p[1])) {
                    diffs.push(format!("{label} created at ({x}, {y}) instead of {p}"));
                }
            }
        }
    }
    if let Some(lines) = expected.get("line_implicit") {
        for (label, want) in lines.as_object().unwrap() {
            let Some(o) = object(&st, label) else {
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
            let (a, b) = (object(&before, label), object(&st, label));
            if a.map(|o| &o["coords"]) != b.map(|o| &o["coords"]) {
                diffs.push(format!("{label} changed"));
            }
        }
    }
    if let Some(offsets) = expected.get("label_offset_px") {
        for (label, want) in offsets.as_object().unwrap() {
            let got = object(&st, label).map(|o| o["label_offset"].clone());
            let ok = got.as_ref().is_some_and(|g| {
                same(num(&g[0]), num(&want[0])) && same(num(&g[1]), num(&want[1]))
            });
            if !ok {
                diffs.push(format!("label offset of {label} is {got:?}, not {want}"));
            }
        }
    }
    Ok(diffs)
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

#[test]
fn the_observed_pointer_scenarios_replay() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let mut table = Vec::new();
    let mut failed = Vec::new();
    for s in fixture["scenarios"].as_array().unwrap() {
        let name = s["name"].as_str().unwrap();
        let status = match part(name) {
            Err(OURS) => format!("not applicable ({OURS})"),
            Err(why) => format!("not applicable yet ({why})"),
            Ok(g) if !LANDED.contains(&g) => format!("pending (group {g})"),
            Ok(g) => {
                let mut diffs = Vec::new();
                for idle in [false, true] {
                    match replay(s, idle) {
                        Ok(d) => diffs.extend(d.into_iter().map(|d| {
                            if idle {
                                format!("{d} (with idle frames)")
                            } else {
                                d
                            }
                        })),
                        Err(e) => diffs.push(e),
                    }
                }
                if diffs.is_empty() {
                    format!("pass (group {g})")
                } else {
                    failed.push(name.to_owned());
                    format!("FAIL (group {g}): {}", diffs.join("; "))
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
}
