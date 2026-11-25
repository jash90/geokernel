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
//! are asserted.

use geo_mcp::protocol::Server;
use serde_json::{json, Value};

const FIXTURE: &str = include_str!("fixtures/pointer_scenarios.json");
const EDITOR_FIXTURE: &str = include_str!("fixtures/editor_tool_scenarios.json");
const TOOL_CAPTURE: &str = include_str!("fixtures/tool_pointer_capture.json");

/// The graphics view of the observations, and where the standard view puts
/// the world origin in it.
const VIEW: (f64, f64) = (892.0, 747.0);
const ZERO: (f64, f64) = (446.0, 373.5);
const SCALE: f64 = 50.0;

/// The landed scenarios, by group.
const LANDED: &[(u8, &[&str])] = &[
    (
        19,
        &[
            "tool_label_visibility",
            "tool_copy_visual_style",
            "tool_image_fixture",
            "tool_function_inspector",
            "tool_freehand_shape",
            "tool_freehand_function",
        ],
    ),
    (16, &["tool_input_box_places_empty_widget"]),
    (17, &["tool_create_list_from_rectangle"]),
    (
        18,
        &[
            "tool_text_places_literal",
            "tool_pen_records_drag",
            "tool_locus_uses_path_point",
        ],
    ),
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
        4,
        &[
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
    (
        10,
        &[
            "tool_semicircle_reuses_diameter_points",
            "tool_circular_arc_three_clicks",
            "tool_circumcircular_arc_three_clicks",
            "tool_circular_sector_three_clicks",
            "tool_circumcircular_sector_three_clicks",
            "tool_ellipse_foci_and_point",
            "tool_hyperbola_foci_and_point",
            "tool_parabola_line_then_focus",
            "tool_conic_five_points",
            "tool_polyline_finished_on_first_vertex",
            "tool_slope_selects_line",
            "tool_vector_from_point_translates_endpoint",
            "tool_compasses_reuses_segment_radius",
            "tool_vector_polygon_from_vertices",
        ],
    ),
    (
        11,
        &[
            "compasses_uses_two_existing_points",
            "compasses_copies_a_circle_radius",
            "compasses_copies_an_arc_radius",
            "area_measures_a_polygon",
            "area_measures_a_circle",
            "area_picks_the_top_arc_over_a_coincident_circle",
            "area_picks_the_top_circle_over_a_coincident_arc",
            "vector_from_point_picks_vector_first",
            "parabola_uses_segment_directrix",
            "parabola_uses_ray_directrix",
            "conic_five_points_deselects_a_repeated_pick",
            "semicircle_reversed_click_order",
            "polyline_deselects_first_with_two_picks",
            "polyline_deselects_nonfirst_with_three_picks",
            "vector_polygon_copies_mid_use",
            "vector_polygon_copies_a_lone_polygon",
        ],
    ),
];

/// The scenarios whose observed behaviour the owner does not follow: our
/// outcome is asserted, and the observed one with `rules` set to
/// `reference` where that switch exists.
const OURS: &[(&str, bool)] = &[
    // Classic's rotate drag has no checkpoint in this recording. The
    // workbench treats it as one geometry gesture, including its undo.
    ("tool_move_rotate", false),
    // A press on a polygon's side drags the polygon
    // (`hit::SIDE_TAKES_THE_POLYGON`).
    ("press_on_polygon_edge_does_nothing", true),
    // A wheel zoom is an undo step right after the gesture
    // (`OwnerRules::view_changes_are_steps`); the reference stores the view
    // only at the next canvas press or when the pointer leaves the canvas,
    // so the observation (the undo pressed through the button, no canvas
    // press) counts none (spec F3).
    ("wheel_zoom_in_about_pointer", true),
    // A pinch zooms exactly under the fingers, with no dead band
    // (`OwnerRules::pinch_bands`, none); the
    // reference's lagging pinch (10 px zoom, 5 px pan) is the same gesture
    // under the reference rules, so both outcomes are asserted.
    ("pinch_spread_symmetric", true),
];

/// The scenarios skipped until a later group, with the group and why.
const SKIPPED: &[(u8, &str, &[&str])] = &[
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

/// A number the recorder wrote with finitely many decimals: `got` matches
/// when it rounds to it, that is when it is within half a unit of the last
/// recorded decimal (the recorder rounded — a scale of 50/1.1 came back as
/// `45.454545` — so a 1e-9 comparison of its output can never pass).
fn same_recorded(got: f64, want: &Value) -> bool {
    let Some(w) = want.as_f64() else {
        return false;
    };
    if !w.is_finite() || !got.is_finite() {
        return w == got;
    }
    let decimals = want.to_string().split('.').nth(1).map_or(0, |d| d.len());
    (got - w).abs() <= 0.5 * 10f64.powi(-(decimals as i32)) + 1e-12
}

/// A length or position in pixels of the view: the reference records whole
/// pixels of its DOM layout, ours are the `f32` rects of our own chrome,
/// which sit up to one pixel from its (the keyboard closes to a canvas of
/// 526 there, 525 here).
fn same_pixel(got: f64, want: &Value) -> bool {
    same_recorded(got, want) || want.as_f64().is_some_and(|w| (got - w).abs() <= 1.0)
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
        14 => "relation",
        15 => "segment",
        45 => "segment_fixed",
        46 => "angle_fixed",
        64 => "rigid_polygon",
        67 => "attach_detach",
        75 => "extremum",
        76 => "roots",
        501 => "point_on_object",
        16 => "polygon",
        17 => "text",
        18 => "ray",
        19 => "midpoint",
        36 => "angle",
        40 => "move_view",
        41 => "zoom_in",
        42 => "zoom_out",
        47 => "locus",
        12 => "conic5",
        20 => "circular_arc",
        21 => "circular_sector",
        22 => "circumcircular_arc",
        23 => "circumcircular_sector",
        24 => "semicircle",
        26 => "image",
        28 => "show_hide_label",
        35 => "copy_visual_style",
        39 => "move_rotate",
        68 => "function_inspector",
        73 => "freehand_shape",
        74 => "freehand_function",
        37 => "vector_from_point",
        49 => "area",
        50 => "slope",
        53 => "compasses",
        55 => "ellipse",
        56 => "hyperbola",
        57 => "parabola",
        61 => "input_box",
        62 => "pen_stroke",
        65 => "polyline",
        70 => "vector_polygon",
        71 => "create_list",
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
        "text" => vec![json!({"type": "text", "text": e["text"]})],
        "key" => {
            let key = e["key"].as_str().ok_or("a key event names no key")?;
            vec![json!({"type": "key", "key": key})]
        }
        // One wheel notch of the observation: the browser's deltaY (+-100),
        // at the pointer. The window's wheel zoom is proportional to the
        // scroll amount (the owner's rule), x1.1 for one notch: the move
        // first (egui zooms about the pointer), then the scroll. egui's
        // wheel delta points up when the browser's deltaY goes down.
        "wheel" => {
            let at = w.at(&e["pos"], false)?;
            let dy = -num(&e["deltaY"]);
            vec![
                ev("move", at),
                json!({"type": "wheel", "unit": "point", "dy": dy}),
            ]
        }
        // A button over the drawing, pressed by name.
        "button" => {
            let name = e["button"]
                .as_str()
                .ok_or("a button event names no button")?;
            vec![json!({"type": "button", "name": name})]
        }
        "focus_input_row" => vec![json!({"type": "focus_input_row"})],
        "resize_window" => {
            let v = e["viewport"]
                .as_array()
                .ok_or("a resize names no viewport")?;
            vec![json!({"type": "resize_window", "viewport": v})]
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
    for (observed, canonical) in [
        ("CircleArc(", "CircularArc"),
        ("CircleSector(", "CircularSector"),
        ("CircumcircleArc(", "CircumcircularArc"),
        ("CircumcircleSector(", "CircumcircularSector"),
    ] {
        if let Some(args) = d.strip_prefix(observed) {
            return format!("{canonical}({args}");
        }
    }
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

/// The measurements the app can expose. The reference web build creates
/// text and anchor points for Area labels, and uses a generic numeric label.
/// The app exposes conic measurements under the controller's area prefix.
fn supported_area_outcome(observed: &Value) -> Option<Value> {
    let mut expected = observed.clone();
    let created = expected.get_mut("created")?.as_array_mut()?;
    let displayed_text = created.iter().any(|o| o["type"] == "text");
    created.retain(|o| o["type"] == "numeric");
    if displayed_text && created.is_empty() {
        expected["undo_steps_added"] = json!(0);
    } else {
        for measure in created {
            let definition = measure["definition"].as_str().unwrap();
            let input = definition
                .strip_prefix("Area(")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            measure["label"] = json!(format!("area{}", input.replace(['_', '{', '}'], "")));
        }
    }
    Some(expected)
}

/// What a replay ended with: the state before and after, how many undo
/// steps it added, and the differences already found mid-way.
struct Outcome {
    before: Value,
    after: Value,
    origin0: (f64, f64),
    undo_added: i64,
    tool_frames: Vec<(Value, Value)>,
    tool_undo: Vec<Value>,
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
    let mut tool_frames = Vec::new();
    let capture = s["frames"].is_array();
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
        // The recorded dialog edit targets a ready text field. Give the
        // modal time to take keyboard focus after the canvas release.
        let dt = if matches!(mode, 45 | 46) && e["type"] == "text" && dt > 2.0 / 60.0 {
            frames.extend(std::iter::repeat_n(
                json!({"dt": 1.0 / 60.0, "events": []}),
                2,
            ));
            dt - 2.0 / 60.0
        } else {
            dt
        };
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
        if capture && matches!(e["type"].as_str(), Some("mouse_up" | "key")) {
            call(
                &mut w.server,
                "input",
                json!({"frames": std::mem::take(&mut frames), "settle": false}),
            )?;
            tool_frames.push((e["t"].clone(), w.state()));
        }
    }
    call(
        &mut w.server,
        "input",
        json!({"frames": frames, "settle": true}),
    )?;
    if expected.get("image_anchor").is_some() {
        insert_fixture_image(&mut w)?;
    }
    compare_editor(expected, &w.server.session.headless().app, &mut diffs);
    let after = w.state();
    let undo_added = undo_depth(&after) - depth0;
    let mut tool_undo = Vec::new();
    if capture {
        for _ in 0..undo_added {
            call(&mut w.server, "undo", json!({}))?;
            tool_undo.push(w.state());
        }
    }
    Ok(Outcome {
        before,
        after,
        origin0,
        undo_added,
        tool_frames,
        tool_undo,
        diffs,
    })
}

/// Supply a deterministic PNG to the open file prompt created by the click.
fn insert_fixture_image(w: &mut Window) -> Result<(), String> {
    use std::io::Write as _;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let number = NEXT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "geokernel-replay-{}-{number}.png",
        std::process::id()
    ));
    let mut file = std::fs::File::create_new(&path).map_err(|error| error.to_string())?;
    let bytes = include_bytes!("../../../tools/ggb-harness/cases/editor_tools/fixture.png");
    let result = file
        .write_all(bytes)
        .map_err(|error| error.to_string())
        .and_then(|()| {
            call(
                &mut w.server,
                "input",
                json!({
                    "frames": [
                        {"dt": 0.016, "events": [{"type": "text", "text": path.to_string_lossy()}]},
                        {"dt": 0.016, "events": [{"type": "key", "key": "Enter"}]}
                    ],
                    "settle": true
                }),
            )
        });
    let _ = std::fs::remove_file(path);
    result.map(|_| ())
}

fn compare_editor(expected: &Value, app: &geo_app_core::app::App, diffs: &mut Vec<String>) {
    let c = app.wb.construction();
    if let Some(points) = expected["editor_points"].as_object() {
        for (label, xy) in points {
            let want = (num(&xy[0]), num(&xy[1]));
            let got = c.id(label).map(|id| c.value(id));
            if !matches!(got, Some(geo_kernel::Value::Point { x, y }) if same(x, want.0) && same(y, want.1))
            {
                diffs.push(format!("{label}: {got:?} instead of {want:?}"));
            }
        }
    }
    if let Some(styles) = expected["editor_styles"].as_object() {
        for (label, wanted) in styles {
            let Some(id) = c.id(label) else {
                diffs.push(format!("missing style target {label}"));
                continue;
            };
            let actual = c.drawing_style(id);
            let rgb = app.wb.palette().color(id);
            let values = json!({
                "show_label": actual.show_label,
                "line_thickness": actual.line_thickness,
                "layer": actual.layer,
                "color": [rgb.r, rgb.g, rgb.b]
            });
            for (field, value) in wanted.as_object().unwrap() {
                if &values[field] != value {
                    diffs.push(format!(
                        "{label} {field}: {} instead of {value}",
                        values[field]
                    ));
                }
            }
        }
    }
    if let Some(samples) = expected["freehand_samples"].as_array() {
        for sample in samples {
            let x = num(&sample[0]);
            let got = c.id("f").and_then(|id| c.function_value(id, x));
            if !got.is_some_and(|y| same(y, num(&sample[1]))) {
                diffs.push(format!("f({x}) is {got:?} instead of {}", sample[1]));
            }
        }
    }
    if let Some(inspector) = expected["inspector"].as_object() {
        if let Some(got) = &app.inspector {
            let [min, max, step] = got.interval;
            let rows = got.rows(&app.wb, min, max, step);
            let want = inspector["rows"].as_array().unwrap();
            if inspector["label"] != got.label
                || rows.len() != want.len()
                || rows
                    .iter()
                    .zip(want)
                    .any(|(&(x, y), pair)| !same(x, num(&pair[0])) || !same(y, num(&pair[1])))
            {
                diffs.push(format!(
                    "inspector on {}: {rows:?} instead of {want:?}",
                    got.label
                ));
            }
        } else {
            diffs.push("inspector is closed".into());
        }
    }
    if let Some(image) = expected["image_anchor"].as_array() {
        let got = c
            .images()
            .next()
            .and_then(|(id, _)| c.image_quad(id, SCALE))
            .map(|quad| quad[3]);
        if !got.is_some_and(|[x, y]| same(x, num(&image[0])) && same(y, num(&image[1]))) {
            diffs.push(format!("image anchor {got:?} instead of {image:?}"));
        }
    }
}

/// The differences of `out` from `expected` (the observed outcome, or ours
/// in its format). `undo_steps_added` is compared only when `undo` is set.
fn compare(expected: &Value, out: &Outcome, undo: bool) -> Vec<String> {
    let (before, st) = (&out.before, &out.after);
    let mut diffs = out.diffs.clone();
    if let Some(picks) = expected.get("picks") {
        if &st["tool"]["picks"] != picks {
            diffs.push(format!(
                "tool picks {} instead of {picks}",
                st["tool"]["picks"]
            ));
        }
    }
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
        let origin = (num(&st["view"]["origin"][0]), num(&st["view"]["origin"][1]));
        let zero = (
            ZERO.0 + origin.0 - out.origin0.0,
            ZERO.1 + origin.1 - out.origin0.1,
        );
        // The origin's fields are recorded each only where the observation
        // measured it (an x-axis drag records no `yZero`).
        if let Some(want) = view.get("xZero") {
            if !same_pixel(zero.0, want) {
                diffs.push(format!("view xZero at {} instead of {want}", zero.0));
            }
        }
        if let Some(want) = view.get("yZero") {
            if !same_pixel(zero.1, want) {
                diffs.push(format!("view yZero at {} instead of {want}", zero.1));
            }
        }
        if !same_recorded(num(&st["view"]["scale"]), &view["scale"]) {
            diffs.push(format!(
                "scale {} instead of {}",
                st["view"]["scale"], view["scale"]
            ));
        }
        if let Some(want) = view.get("yscale") {
            if !same_recorded(num(&st["view"]["yscale"]), want) {
                diffs.push(format!("yscale {} instead of {want}", st["view"]["yscale"]));
            }
        }
    }
    // The view's own measures, from its rectangle and transform: what part
    // of the plane it shows, its size, and which world point its centre
    // holds.
    if expected.get("visible_world").is_some()
        || expected.get("canvas").is_some()
        || expected.get("world_at_canvas_centre").is_some()
    {
        let r = &st["view"]["rect"];
        let rect = [num(&r[0]), num(&r[1]), num(&r[2]), num(&r[3])];
        // The rect is the view's place in the window, then its size; the
        // transform of the state: origin and scale are the view's own.
        let (w, h) = (rect[2], rect[3]);
        let (ox, oy) = (
            num(&st["view"]["origin"][0]) - rect[0],
            num(&st["view"]["origin"][1]) - rect[1],
        );
        let (sx, sy_) = (num(&st["view"]["scale"]), num(&st["view"]["yscale"]));
        let world = |px: f64, py: f64| -> (f64, f64) { ((px - ox) / sx, (oy - py) / sy_) };
        if let Some(vis) = expected.get("visible_world") {
            let got = (
                world(0.0, 0.0).0,
                world(w, 0.0).0,
                world(0.0, h).1,
                world(0.0, 0.0).1,
            );
            let want = [&vis["xmin"], &vis["xmax"], &vis["ymin"], &vis["ymax"]];
            if ![got.0, got.1, got.2, got.3]
                .into_iter()
                .zip(want)
                .all(|(g, w)| same_recorded(g, w))
            {
                diffs.push(format!(
                    "visible world (xmin {}, xmax {}, ymin {}, ymax {}) instead of ({}, {}, {}, {})",
                    got.0, got.1, got.2, got.3,
                    vis["xmin"], vis["xmax"], vis["ymin"], vis["ymax"],
                ));
            }
        }
        if let Some(canvas) = expected.get("canvas") {
            let got = (w, h);
            let want = (num(&canvas[0]), num(&canvas[1]));
            if !same_pixel(got.0, &canvas[0]) || !same_pixel(got.1, &canvas[1]) {
                diffs.push(format!("canvas {got:?} instead of {want:?}"));
            }
        }
        if let Some(centre) = expected.get("world_at_canvas_centre") {
            let got = world(0.5 * w, 0.5 * h);
            let want = (num(&centre[0]), num(&centre[1]));
            if !same(got.0, want.0) || !same(got.1, want.1) {
                diffs.push(format!("canvas centre at {got:?} instead of {want:?}"));
            }
        }
    }
    // A world point the scenario says stayed under the pointer: it began at
    // the standard view's place of it (the window is opened standard) and
    // must still be there. The `approx` form is the reference's pinch,
    // which lags behind its fingers — its zoom fires only once the fingers
    // pass the dead band, so the point wanders a pixel or so, measured in
    // world units at the end view's scale (0.02 of them, about 1.5 px
    // there); ours holds the exact check.
    for (key, world) in [
        ("fixed_world_point", false),
        ("fixed_world_point_approx", true),
    ] {
        if let Some(p) = expected.get(key) {
            let before = (
                out.origin0.0 + num(&p[0]) * SCALE,
                out.origin0.1 - num(&p[1]) * SCALE,
            );
            let origin = (num(&st["view"]["origin"][0]), num(&st["view"]["origin"][1]));
            let after = (
                origin.0 + num(&p[0]) * num(&st["view"]["scale"]),
                origin.1 - num(&p[1]) * num(&st["view"]["yscale"]),
            );
            let (dx, dy) = ((before.0 - after.0).abs(), (before.1 - after.1).abs());
            // The pixel drift of the exact form, the world drift (0.02 of
            // a unit) of the approx one.
            let ok = if world {
                dx / num(&st["view"]["scale"]) <= 0.02 && dy / num(&st["view"]["yscale"]) <= 0.02
            } else {
                dx <= 1e-9 && dy <= 1e-9
            };
            if !ok {
                diffs.push(format!("{key} {p:?} moved from {before:?} to {after:?}"));
            }
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
        if let Some(n) = c.get("number") {
            let value = &o["coords"]["value"];
            if !same(num(value), num(n)) {
                diffs.push(format!("{label} is {value} instead of {n} {when}"));
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
fn ours_expected(name: &str, observed: &Value) -> Value {
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
        // The wheel zoom is one undo step the moment the run of wheel
        // events ends (the owner's rule, `OwnerRules::view_changes_are_steps`);
        // the view is the observed one — the zoom is about the pointer and
        // the same in both apps.
        "wheel_zoom_in_about_pointer" => json!({
            "view": {"xZero": 459.4, "yZero": 386.15, "scale": 55},
            "fixed_world_point": [-2.68, 2.53],
            "undo_steps_added": 1
        }),
        // The fingers spread from 200 to 300 px about the fixed midpoint
        // (-2.68, 2.53), drawn at (312, 247): the scale goes from 50 to 75
        // and that point stays under the fingers (the reference app lags
        // by its 10 px dead band: 74.25). egui measures the fingers in
        // `f32`, which stays within the recorded integer's half unit. The
        // pinch changed the view, so it is one undo step right after the
        // gesture (the owner's rule, `OwnerRules::view_changes_are_steps`).
        "pinch_spread_symmetric" => json!({
            "objects": {"A": [1, 1.01]},
            "view": {"xZero": 513, "yZero": 436.75, "scale": 75},
            "undo_steps_added": 1
        }),
        "tool_move_rotate" => {
            let mut expected = observed.clone();
            assert_eq!(expected["undo_steps_added"], 0);
            expected["undo_steps_added"] = json!(1);
            expected
        }
        other => panic!("no outcome of ours for {other}"),
    }
}

fn compare_tool_objects(reference: &Value, state: &Value, when: &str, diffs: &mut Vec<String>) {
    let reference = reference.as_object().unwrap();
    let mut expected: Vec<_> = reference
        .keys()
        .map(|name| match name.as_str() {
            "loc1" => "locus1",
            label => label,
        })
        .collect();
    expected.sort_unstable();
    let mut actual = scenario_labels(state);
    actual.sort_unstable();
    if actual != expected {
        diffs.push(format!(
            "{when}: objects {actual:?}, reference {expected:?}"
        ));
    }
    for (label, geo) in reference {
        let name = if label == "loc1" { "locus1" } else { label };
        let Some(actual) = object(state, name) else {
            continue;
        };
        let kind = geo["type"].as_str().unwrap();
        let actual_kind = if kind == "stroke" { "pen stroke" } else { kind };
        if actual["type"] != actual_kind || actual["defined"] != geo["defined"] {
            diffs.push(format!(
                "{when}: {name} is {} (defined: {}), reference {kind} (defined: {})",
                actual["type"], actual["defined"], geo["defined"]
            ));
        }
        if kind == "point" {
            for field in ["on_path", "in_region"] {
                if actual[field] != geo[field] {
                    diffs.push(format!(
                        "{when}: {name} {field} is {}, reference {}",
                        actual[field], geo[field]
                    ));
                }
            }
        }
        if geo["defined"] != true {
            continue;
        }
        let fields: &[&str] = match kind {
            "point" | "vector" => &["x", "y"],
            "circle" => &["cx", "cy", "r"],
            "segment" => &["x1", "y1", "x2", "y2"],
            "angle" | "number" => &["value"],
            "polygon" => &["area"],
            _ => &[],
        };
        for &field in fields {
            let actual = &actual["coords"][field];
            if !same(num(actual), num(&geo[field])) {
                diffs.push(format!(
                    "{when}: {name} {field} is {actual}, reference {}",
                    geo[field]
                ));
            }
        }
    }
}

fn compare_tool_capture(capture: &Value, out: &Outcome) -> Vec<String> {
    let frames = capture["frames"].as_array().unwrap();
    let undo = capture["undo"].as_array().unwrap();
    let desktop_only = match capture["name"].as_str() {
        Some("tool_text_places_literal") => Some("text1"),
        Some("tool_input_box_places_empty_widget") => Some("InputBox1"),
        Some("tool_create_list_from_rectangle") => Some("l1"),
        _ => None,
    };
    let compare_frame = |frame: &Value, state: &Value, when: &str, diffs: &mut Vec<String>| {
        if let Some(extra) = desktop_only {
            let mut state = state.clone();
            state["objects"]
                .as_array_mut()
                .unwrap()
                .retain(|obj| obj["name"] != extra);
            compare_tool_objects(&frame["objects"], &state, when, diffs);
        } else {
            compare_tool_objects(&frame["objects"], state, when, diffs);
        }
    };
    let mut diffs = out.diffs.clone();
    compare_tool_objects(&capture["initial"], &out.before, "initial", &mut diffs);
    if out.tool_frames.len() != frames.len() {
        diffs.push(format!(
            "{} frames, reference {}",
            out.tool_frames.len(),
            frames.len()
        ));
    }
    for (frame, (time, state)) in frames.iter().zip(&out.tool_frames) {
        let when = format!("frame {}", frame["t"]);
        if *time != frame["t"] {
            diffs.push(format!("{when}: replayed at {time}"));
        }
        compare_frame(frame, state, &when, &mut diffs);
    }
    if let Some(frame) = frames.last() {
        compare_frame(frame, &out.after, "settled", &mut diffs);
    }
    // Desktop submits the two dialogs and creates the list. Their one
    // additional undo step must return to the reference's initial state.
    let expected_undo: Vec<_> = if desktop_only.is_some() {
        assert!(undo.is_empty(), "reference acquired a new undo step");
        vec![&capture["initial"]]
    } else {
        undo.iter().collect()
    };
    if out.undo_added != expected_undo.len() as i64 || out.tool_undo.len() != expected_undo.len() {
        diffs.push(format!(
            "{} undo steps, expected {}",
            out.undo_added,
            expected_undo.len()
        ));
    }
    for (index, (expected, actual)) in expected_undo.into_iter().zip(&out.tool_undo).enumerate() {
        compare_tool_objects(expected, actual, &format!("undo {}", index + 1), &mut diffs);
    }
    if capture["tool_mode"] == 14
        && out.after["tool"]["relation"]
            .as_array()
            .is_none_or(Vec::is_empty)
    {
        diffs.push("missing relation report".to_owned());
    }
    diffs
}

#[test]
fn tool_capture_detects_changed_constraints_and_undo_objects() {
    let data: Value = serde_json::from_str(TOOL_CAPTURE).unwrap();
    let capture = data["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "tool_segment_fixed_creates_endpoint")
        .unwrap();
    let out = replay(capture, false, "ours").unwrap();
    assert!(compare_tool_capture(capture, &out).is_empty());
    let mut wrong_path = capture.clone();
    wrong_path["frames"][1]["objects"]["B"]["on_path"] = json!(false);
    let mut wrong_undo = capture.clone();
    wrong_undo["undo"][0]["A"]["x"] = json!(999.0);
    let discrepancies = [
        compare_tool_capture(&wrong_path, &out),
        compare_tool_capture(&wrong_undo, &out),
    ];
    assert!(
        discrepancies.iter().all(|diffs| !diffs.is_empty()),
        "{discrepancies:?}"
    );
}

#[test]
fn the_observed_pointer_scenarios_replay() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let editor: Value = serde_json::from_str(EDITOR_FIXTURE).unwrap();
    let scenarios = || {
        fixture["scenarios"]
            .as_array()
            .unwrap()
            .iter()
            .chain(editor["scenarios"].as_array().unwrap())
    };
    let mut table = Vec::new();
    let mut failed = Vec::new();
    let (mut replayed, mut ours) = (0, 0);
    let mut skipped = std::collections::BTreeMap::<u8, usize>::new();
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
    let captures: Value = serde_json::from_str(TOOL_CAPTURE).unwrap();
    let captures = captures["scenarios"].as_array().unwrap();
    let mut captured_passes = 0;
    let mut recaptured = 0;
    let mut bounded = 0;
    for capture in captures {
        let name = capture["name"].as_str().unwrap();
        if capture["initial"].get("$object_count").is_some() {
            // These constructions exceed the interactive shape allowance and
            // auto-fit outside the replay viewport. Core tests exercise the cap.
            bounded += 1;
            continue;
        }
        if let Some(existing) = scenarios().find(|scenario| scenario["name"] == name) {
            for key in ["tool_mode", "initial_script", "events"] {
                assert_eq!(capture[key], existing[key], "{name} {key}");
            }
            recaptured += 1;
        }
        let diffs = both(capture, "ours", &|out| compare_tool_capture(capture, out));
        if !diffs.is_empty() {
            failed.push(name.to_owned());
        }
        table.push(format!(
            "{name}: {}",
            if diffs.is_empty() {
                "pass".to_owned()
            } else {
                diffs.join("; ")
            }
        ));
        captured_passes += usize::from(!scenarios().any(|scenario| scenario["name"] == name));
    }
    assert_eq!((captured_passes, recaptured, bounded), (23, 5, 4));
    for s in scenarios() {
        let name = s["name"].as_str().unwrap();
        assert!(
            s["source"].as_str().is_some(),
            "{name}: missing observation source"
        );
        let area_outcome = (s["tool_mode"] == 49)
            .then(|| supported_area_outcome(&s["expected"]))
            .flatten();
        let expected = area_outcome.as_ref().unwrap_or(&s["expected"]);
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
                let diffs = both(s, "ours", &|out| compare(expected, out, true));
                if diffs.is_empty() {
                    format!("pass (group {g})")
                } else {
                    failed.push(name.to_owned());
                    format!("FAIL (group {g}): {}", diffs.join("; "))
                }
            }
            Some(Part::Ours { has_switch }) => {
                ours += 1;
                let mine = ours_expected(name, expected);
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
    let names: Vec<&str> = scenarios().map(|s| s["name"].as_str().unwrap()).collect();
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
    assert_eq!((replayed, ours), (104, 4), "scenarios replayed");
    let unique_replayed = replayed + ours + captured_passes;
    let unique_scenarios = names.len() + captures.len() - recaptured;
    assert_eq!((unique_replayed, unique_scenarios), (131, 138));
    assert_eq!(
        skipped.into_iter().collect::<Vec<_>>(),
        [(5, 1), (6, 1), (8, 1)],
        "scenarios skipped, by group"
    );
}

#[test]
fn replays_recorded_cas_view_interactions() {
    use geo_app::headless::{self, egui, Headless, Profile};
    use geo_app::Workbench;

    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/cas_view_scenarios.json")).unwrap();
    let recording: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../golden/casview/rows.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let click = |window: &mut Headless, label: &str| {
        let pos = window
            .widgets()
            .iter()
            .find(|widget| widget.visible && widget.label == label)
            .unwrap_or_else(|| panic!("missing {label}"))
            .rect
            .center();
        window.frame(
            headless::DT,
            vec![
                egui::Event::PointerMoved(pos),
                headless::press(pos, egui::PointerButton::Primary, true),
            ],
        );
        window.frame(
            headless::DT,
            vec![headless::press(pos, egui::PointerButton::Primary, false)],
        );
    };
    let dt = headless::DT;
    let mut count = 0;
    for (scenario, id) in fixture.as_object().unwrap() {
        let case = recording["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"] == *id)
            .unwrap();
        let script = case["script"]
            .as_array()
            .unwrap()
            .iter()
            .map(|line| line.as_str().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        let mut window = Headless::new(
            Workbench::from_script(&script).unwrap(),
            None,
            Profile::Desktop,
            (1200.0, 850.0),
            1.0,
        );
        window.frame(dt, Vec::new());
        click(&mut window, "Menu");
        window.frame(dt, Vec::new());
        click(&mut window, "Widok CAS");
        window.frame(dt, Vec::new());
        for step in case["casview"].as_array().unwrap() {
            let row = step["row"].as_u64().unwrap();
            let action = step["action"].as_str().unwrap();
            if action == "delete" {
                click(&mut window, &format!("Usuń wiersz {row}"));
            } else {
                if row == 1 || action == "edit" {
                    click(&mut window, &format!("Wiersz {row}"));
                }
                if action == "edit" {
                    let select_all = egui::Modifiers {
                        command: true,
                        ..egui::Modifiers::NONE
                    };
                    window.frame(dt, vec![headless::key(egui::Key::A, true, select_all)]);
                    window.frame(dt, vec![headless::key(egui::Key::A, false, select_all)]);
                }
                window.frame(
                    dt,
                    vec![egui::Event::Text(step["input"].as_str().unwrap().into())],
                );
                window.frame(
                    dt,
                    vec![headless::key(egui::Key::Enter, true, egui::Modifiers::NONE)],
                );
                window.frame(
                    dt,
                    vec![headless::key(
                        egui::Key::Enter,
                        false,
                        egui::Modifiers::NONE,
                    )],
                );
            }
            let actual: Vec<_> = window
                .app
                .wb
                .cas_rows()
                .iter()
                .map(|row| json!({"output": row.output.text, "latex": row.output.latex}))
                .collect();
            let expected: Vec<_> = step["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| json!({"output": row["output"], "latex": row["latex"]}))
                .collect();
            assert_eq!(actual, expected, "{scenario} {action} row {row}");
        }
        count += 1;
    }
    assert_eq!(count, 6);
}

#[test]
fn replays_recorded_spreadsheet_interactions() {
    use geo_app::headless::{self, egui, Headless, Profile};
    use geo_app::Workbench;
    use geo_kernel::spreadsheet::Address;
    use std::collections::BTreeMap;

    let fixture: BTreeMap<String, String> =
        serde_json::from_str(include_str!("fixtures/spreadsheet_scenarios.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/spreadsheet");
    let dt = headless::DT;
    let locate = |window: &Headless, label: &str| {
        window
            .widgets()
            .into_iter()
            .find(|w| w.visible && w.label == label)
            .unwrap_or_else(|| panic!("no visible {label}"))
            .rect
            .center()
    };
    let click = |window: &mut Headless, label: &str| {
        let point = locate(window, label);
        window.frame(
            dt,
            vec![
                egui::Event::PointerMoved(point),
                headless::press(point, egui::PointerButton::Primary, true),
            ],
        );
        window.frame(
            dt,
            vec![headless::press(point, egui::PointerButton::Primary, false)],
        );
        window.frame(dt, Vec::new());
    };
    let key = |window: &mut Headless, key, modifiers| {
        window.frame(dt, vec![headless::key(key, true, modifiers)]);
        window.frame(dt, vec![headless::key(key, false, modifiers)]);
    };
    let replace = |window: &mut Headless, text: &str| {
        key(window, egui::Key::A, egui::Modifiers::COMMAND);
        if text.is_empty() {
            key(window, egui::Key::Backspace, egui::Modifiers::NONE);
        } else {
            window.frame(dt, vec![egui::Event::Text(text.into())]);
        }
        key(window, egui::Key::Enter, egui::Modifiers::NONE);
    };

    for (scenario, id) in &fixture {
        let topic = id
            .strip_prefix("spreadsheet-")
            .unwrap()
            .strip_suffix("-001")
            .unwrap();
        let data: Value =
            serde_json::from_slice(&std::fs::read(root.join(format!("{topic}.json"))).unwrap())
                .unwrap();
        let case = &data["cases"][0];
        assert_eq!(case["id"], id.as_str());
        let mut window = Headless::new(
            Workbench::default(),
            None,
            Profile::Desktop,
            (1400.0, 850.0),
            1.0,
        );
        window.frame(dt, Vec::new());
        click(&mut window, "Menu");
        click(&mut window, "Arkusz kalkulacyjny");
        for step in case["spreadsheet"].as_array().unwrap() {
            match step["action"].as_str().unwrap() {
                "edit" => {
                    let label = step["cell"].as_str().unwrap();
                    click(&mut window, label);
                    replace(&mut window, step["input"].as_str().unwrap());
                    let source = Address::parse(label).unwrap();
                    let selected = window.app.spreadsheet_editor.as_ref().unwrap().selected;
                    assert_eq!(selected.row, source.row + 1, "{scenario}: Enter");
                    assert_eq!(selected.column, source.column, "{scenario}: Enter");
                    click(&mut window, label);
                    if let Some(value) = step["cells"][label].as_str() {
                        assert_eq!(
                            window.accessible_value(label),
                            Some(value),
                            "{scenario}: {label}"
                        );
                    }
                }
                "command" => {
                    let source = step["input"].as_str().unwrap();
                    if geo_kernel::spreadsheet::is_fill_line(source) {
                        click(&mut window, "A1");
                        replace(&mut window, &format!("={source}"));
                    } else {
                        window.app.wb.run_line(source).unwrap();
                        window.frame(dt, Vec::new());
                    }
                }
                "undo" => click(&mut window, "Cofnij"),
                "redo" => click(&mut window, "Ponów"),
                "drag" => {
                    click(&mut window, step["cell"].as_str().unwrap());
                    let from = locate(&window, "Wypełnij zakres");
                    let to = locate(&window, step["target"].as_str().unwrap());
                    window.frame(
                        dt,
                        vec![
                            egui::Event::PointerMoved(from),
                            headless::press(from, egui::PointerButton::Primary, true),
                        ],
                    );
                    window.frame(dt, vec![egui::Event::PointerMoved(to)]);
                    window.frame(
                        dt,
                        vec![headless::press(to, egui::PointerButton::Primary, false)],
                    );
                    window.frame(dt, Vec::new());
                }
                action => panic!("unexpected spreadsheet action {action}"),
            }
            let c = window.app.wb.construction();
            let actual: BTreeMap<_, _> = c
                .spreadsheet_cells()
                .map(|(address, _)| (address.label(), c.spreadsheet_display(address)))
                .collect();
            assert_eq!(json!(actual), step["cells"], "{scenario}: {step}");
        }
    }
    assert_eq!(fixture.len(), 6);
}
