//! The server through its protocol layer: JSON-RPC lines in, lines out.

use geo_mcp::protocol::{self, Server};
use serde_json::{json, Value};

fn request(id: i64, method: &str, params: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
}

fn rpc(s: &mut Server, method: &str, params: Value) -> Value {
    let reply = s
        .handle_line(&request(1, method, params))
        .expect("a request gets a reply");
    serde_json::from_str(&reply).expect("the reply is JSON")
}

/// Calls a tool and returns its structured result; the call must succeed.
fn call(s: &mut Server, name: &str, args: Value) -> Value {
    let r = rpc(s, "tools/call", json!({"name": name, "arguments": args}));
    let result = &r["result"];
    assert_eq!(result["isError"], false, "{name} failed: {r}");
    let text = result["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed, result["structuredContent"]);
    parsed
}

/// Calls a tool that must fail and returns its message.
fn call_err(s: &mut Server, name: &str, args: Value) -> String {
    let r = rpc(s, "tools/call", json!({"name": name, "arguments": args}));
    assert_eq!(r["result"]["isError"], true, "{name} did not fail: {r}");
    r["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn objects(state: &Value) -> Vec<String> {
    state["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["name"].as_str().unwrap().to_owned())
        .collect()
}

fn point(state: &Value, name: &str) -> (f64, f64) {
    let o = state["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["name"] == name)
        .unwrap_or_else(|| panic!("no {name}"));
    let c = &o["coords"];
    (c["x"].as_f64().unwrap(), c["y"].as_f64().unwrap())
}

fn camera(state: &Value) -> (f64, f64, f64) {
    let c = &state["camera"];
    (
        c["cx"].as_f64().unwrap(),
        c["cy"].as_f64().unwrap(),
        c["scale"].as_f64().unwrap(),
    )
}

fn state(s: &mut Server) -> Value {
    call(s, "state", json!({}))
}

/// The widget with id `id`: its rectangle `[x, y, w, h]`.
fn widget(s: &mut Server, id: &str) -> [f64; 4] {
    let list = call(s, "widgets", json!({}));
    let w = list["widgets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["id"] == id)
        .unwrap_or_else(|| panic!("no widget {id}: {list}"))
        .clone();
    let r: Vec<f64> = w["rect"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    [r[0], r[1], r[2], r[3]]
}

fn at(x: f64, y: f64) -> Value {
    json!({"x": x, "y": y})
}

fn world(x: f64, y: f64) -> Value {
    json!({"wx": x, "wy": y})
}

fn ev(kind: &str, pos: Value) -> Value {
    let mut e = pos;
    e["type"] = json!(kind);
    e
}

/// A frame of events at the default interval.
fn frame(events: Vec<Value>) -> Value {
    json!({ "events": events })
}

fn input(s: &mut Server, frames: Vec<Value>, settle: bool) -> Value {
    call(s, "input", json!({"frames": frames, "settle": settle}))
}

mod protocol_tests {
    use super::*;

    #[test]
    fn initialize_negotiates_a_legacy_revision() {
        let mut s = Server::new();
        let r = rpc(
            &mut s,
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}),
        );
        assert_eq!(r["jsonrpc"], "2.0");
        assert_eq!(r["id"], 1);
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(r["result"]["serverInfo"]["name"], "geokernel-mcp");
        assert!(r["result"]["capabilities"]["tools"].is_object());
        assert!(r["result"].get("resultType").is_none());
        // An unknown revision gets the newest legacy one.
        let r = rpc(
            &mut s,
            "initialize",
            json!({"protocolVersion": "2099-01-01"}),
        );
        assert_eq!(r["result"]["protocolVersion"], protocol::LEGACY[0]);
        // Notifications get no reply.
        let n = json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string();
        assert_eq!(s.handle_line(&n), None);
        assert_eq!(s.handle_line("   "), None);
        assert_eq!(rpc(&mut s, "ping", json!({}))["result"], json!({}));
    }

    #[test]
    fn tools_list_names_every_tool_with_a_schema() {
        let mut s = Server::new();
        let r = rpc(&mut s, "tools/list", json!({}));
        let names: Vec<&str> = r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                assert_eq!(t["inputSchema"]["type"], "object", "{t}");
                assert!(t["description"].as_str().unwrap().len() > 20);
                t["name"].as_str().unwrap()
            })
            .collect();
        assert_eq!(
            names,
            [
                "reset",
                "open",
                "state",
                "widgets",
                "input",
                "click",
                "drag",
                "type_line",
                "select_tool",
                "undo",
                "redo",
                "resize",
                "set_view",
                "screenshot",
                "frame_stats"
            ]
        );
    }

    #[test]
    fn modern_requests_carry_their_version_and_get_a_result_type() {
        let mut s = Server::new();
        let meta = json!({
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientCapabilities": {}
        });
        let r = rpc(&mut s, "server/discover", json!({ "_meta": meta }));
        assert_eq!(r["result"]["resultType"], "complete");
        assert_eq!(r["result"]["supportedVersions"], json!(["2026-07-28"]));
        assert!(r["result"]["capabilities"]["tools"].is_object());
        let r = rpc(&mut s, "tools/list", json!({ "_meta": meta }));
        assert_eq!(r["result"]["resultType"], "complete");
        let r = rpc(
            &mut s,
            "tools/call",
            json!({"_meta": meta, "name": "state", "arguments": {}}),
        );
        assert_eq!(r["result"]["resultType"], "complete");
        assert_eq!(r["result"]["isError"], false);
        // An unknown version, and a request without capabilities.
        let r = rpc(
            &mut s,
            "tools/list",
            json!({"_meta": {"io.modelcontextprotocol/protocolVersion": "1900-01-01", "io.modelcontextprotocol/clientCapabilities": {}}}),
        );
        assert_eq!(r["error"]["code"], -32022);
        assert_eq!(r["error"]["data"]["requested"], "1900-01-01");
        assert!(r["error"]["data"]["supported"]
            .as_array()
            .unwrap()
            .contains(&json!("2026-07-28")));
        let r = rpc(
            &mut s,
            "tools/list",
            json!({"_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}}),
        );
        assert_eq!(r["error"]["code"], -32602);
        // A modern client does not initialize.
        let r = rpc(&mut s, "initialize", json!({ "_meta": meta }));
        assert_eq!(r["error"]["code"], -32601);
    }

    #[test]
    fn protocol_faults_are_json_rpc_errors_and_tool_faults_are_results() {
        let mut s = Server::new();
        let parse: Value = serde_json::from_str(&s.handle_line("{not json").unwrap()).unwrap();
        assert_eq!(parse["error"]["code"], -32700);
        assert_eq!(parse["id"], Value::Null);
        let batch: Value = serde_json::from_str(&s.handle_line("[1, 2]").unwrap()).unwrap();
        assert_eq!(batch["error"]["code"], -32600);
        let bad_id = json!({"jsonrpc": "2.0", "id": {"a": 1}, "method": "ping"}).to_string();
        let r: Value = serde_json::from_str(&s.handle_line(&bad_id).unwrap()).unwrap();
        assert_eq!(r["error"]["code"], -32600);
        let no_version = json!({"id": 3, "method": "ping"}).to_string();
        let r: Value = serde_json::from_str(&s.handle_line(&no_version).unwrap()).unwrap();
        assert_eq!(
            (r["id"].clone(), r["error"]["code"].clone()),
            (json!(3), json!(-32600))
        );
        assert_eq!(rpc(&mut s, "nope", json!({}))["error"]["code"], -32601);
        let params = json!({"jsonrpc": "2.0", "id": 4, "method": "tools/list", "params": [1]});
        let r: Value = serde_json::from_str(&s.handle_line(&params.to_string()).unwrap()).unwrap();
        assert_eq!(r["error"]["code"], -32602);
        let unknown = rpc(
            &mut s,
            "tools/call",
            json!({"name": "fly", "arguments": {}}),
        );
        assert_eq!(unknown["error"]["code"], -32602);
        let args = rpc(
            &mut s,
            "tools/call",
            json!({"name": "state", "arguments": 3}),
        );
        assert_eq!(args["error"]["code"], -32602);
        // A tool that fails is a result with isError.
        let e = call_err(&mut s, "select_tool", json!({"tool": "lasso"}));
        assert!(e.contains("unknown tool `lasso`"), "{e}");
        let e = call_err(&mut s, "click", json!({"x": 1}));
        assert!(e.contains("position"), "{e}");
    }

    #[test]
    fn serve_writes_one_line_per_reply_and_survives_bad_bytes() {
        let mut s = Server::new();
        let mut input = Vec::new();
        input.extend_from_slice(request(1, "ping", json!({})).as_bytes());
        input.extend_from_slice(b"\n\xff\xfe\n\n");
        input.extend_from_slice(
            json!({"jsonrpc": "2.0", "method": "x"})
                .to_string()
                .as_bytes(),
        );
        input.extend_from_slice(b"\n");
        input.extend_from_slice(request(2, "tools/list", json!({})).as_bytes());
        let mut out = Vec::new();
        protocol::serve(&input[..], &mut out, |l| s.handle_line(l)).unwrap();
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 3, "{text}");
        assert_eq!(lines[0]["id"], 1);
        assert_eq!(lines[1]["error"]["code"], -32700);
        assert_eq!(lines[2]["id"], 2);
    }

    #[test]
    fn batch_mode_runs_the_same_handlers() {
        let mut session = geo_mcp::Session::new();
        let lines = [
            r#"{"tool": "reset", "arguments": {"script": "A = (1, 2)\n"}}"#,
            "# a comment",
            r#"{"tool": "state"}"#,
            r#"{"name": "click", "arguments": {"y": 3}}"#,
            "nonsense",
        ];
        let out: Vec<Value> = lines
            .iter()
            .filter_map(|l| protocol::batch_line(&mut session, l))
            .map(|l| serde_json::from_str(&l).unwrap())
            .collect();
        assert_eq!(out.len(), 4);
        assert_eq!(out[0]["ok"], true);
        assert_eq!(out[1]["result"]["objects"][0]["name"], "A");
        assert_eq!(out[2]["ok"], false);
        assert_eq!(out[2]["tool"], "click");
        assert_eq!(out[3]["ok"], false);
    }
}

/// A small deterministic generator for the fuzz tests.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn value(&mut self, depth: u32) -> Value {
        const KEYS: [&str; 24] = [
            "x", "y", "wx", "wy", "dx", "widget", "frames", "events", "type", "dt", "id", "phase",
            "key", "text", "tool", "from", "to", "steps", "width", "height", "profile", "scale",
            "base64", "button",
        ];
        const WORDS: [&str; 12] = [
            "move", "down", "up", "touch", "start", "end", "key", "Escape", "point", "segment",
            "zoom_in", "",
        ];
        let numbers = [0.0, 1.0, -1.0, 3.5, 450.0, 1e9, -1e300, 0.016];
        match self.below(if depth > 2 { 5 } else { 7 }) {
            0 => Value::Null,
            1 => json!(self.below(2) == 0),
            2 => json!(numbers[self.below(numbers.len() as u64) as usize]),
            3 => json!(WORDS[self.below(WORDS.len() as u64) as usize]),
            4 => json!(self.below(5)),
            5 => Value::Array((0..self.below(4)).map(|_| self.value(depth + 1)).collect()),
            _ => {
                let mut m = serde_json::Map::new();
                for _ in 0..self.below(5) {
                    let k = KEYS[self.below(KEYS.len() as u64) as usize];
                    m.insert(k.to_owned(), self.value(depth + 1));
                }
                Value::Object(m)
            }
        }
    }
}

#[test]
fn garbage_lines_get_error_replies_and_never_panic() {
    let mut s = Server::new();
    let mut g = Lcg(7);
    let valid = request(9, "tools/call", json!({"name": "state", "arguments": {}}));
    for i in 0..400 {
        let line: String = match i % 4 {
            // Random printable and control characters.
            0 => (0..g.below(80))
                .map(|_| char::from_u32(g.below(0x250) as u32).unwrap_or('?'))
                .collect(),
            // A valid message cut short.
            1 => valid
                .chars()
                .take(g.below(valid.len() as u64) as usize)
                .collect(),
            // A valid message with one character replaced.
            2 => {
                let mut c: Vec<char> = valid.chars().collect();
                let k = g.below(c.len() as u64) as usize;
                c[k] = ['{', '"', ']', ':', '0', 'x'][g.below(6) as usize];
                c.into_iter().collect()
            }
            // Deep nesting.
            _ => "[".repeat(g.below(300) as usize + 1),
        };
        if let Some(reply) = s.handle_line(&line) {
            let r: Value = serde_json::from_str(&reply).expect("every reply is JSON");
            assert_eq!(r["jsonrpc"], "2.0");
            assert!(
                r.get("result").is_some() != r.get("error").is_some(),
                "{reply}"
            );
        }
    }
}

#[test]
fn random_tool_arguments_fail_cleanly() {
    let mut s = Server::new();
    let mut g = Lcg(11);
    let tools = [
        "reset",
        "open",
        "state",
        "widgets",
        "input",
        "click",
        "drag",
        "type_line",
        "select_tool",
        "undo",
        "redo",
        "resize",
        "set_view",
        "frame_stats",
    ];
    for i in 0..300 {
        let name = tools[i % tools.len()];
        let args = match g.value(0) {
            v @ Value::Object(_) => v,
            _ => json!({}),
        };
        let r = rpc(
            &mut s,
            "tools/call",
            json!({"name": name, "arguments": args}),
        );
        let text = r["result"]["content"][0]["text"].as_str().unwrap_or("");
        assert!(!text.contains("internal error"), "{name} {args}: {text}");
    }
}

mod window_tests {
    use super::*;

    /// Port of `ui_tests::gestures::a_release_and_a_press_in_one_frame_end_the_drag_and_start_a_new_press`.
    #[test]
    fn a_release_and_a_press_in_one_frame_end_the_drag_and_start_a_new_press() {
        let mut s = Server::new();
        call(&mut s, "reset", json!({"script": "A = (0, 0)\n"}));
        let from = world(0.0, 0.0);
        let to = json!({"wx": 0.0, "wy": 0.0, "dx": 100.0});
        let empty = world(-6.0, 4.0);
        let mut frames = vec![
            frame(vec![ev("move", from.clone())]),
            frame(vec![ev("down", from.clone())]),
        ];
        for i in 1..=5 {
            frames.push(frame(vec![ev(
                "move",
                json!({"wx": 0.0, "wy": 0.0, "dx": 20.0 * i as f64}),
            )]));
        }
        // The release of the drag and a press on empty space in one frame.
        frames.push(frame(vec![
            ev("up", to),
            ev("move", empty.clone()),
            ev("down", empty.clone()),
        ]));
        input(&mut s, frames, false);
        let st = state(&mut s);
        assert_eq!(point(&st, "A").0, 2.0, "the drag ended where it was");
        let cam = camera(&st);
        let mut frames = Vec::new();
        for i in 1..=5 {
            frames.push(frame(vec![ev(
                "move",
                json!({"wx": -6.0, "wy": 4.0, "dx": 20.0 * i as f64}),
            )]));
        }
        input(&mut s, frames, false);
        // The pan moved the view: the release goes where the pointer is.
        let release = {
            let st = state(&mut s);
            let st_cam = camera(&st);
            assert_ne!(st_cam.0, cam.0, "the second press panned the view");
            json!({"wx": -6.0, "wy": 4.0, "dx": 100.0 - (st_cam.0 - cam.0) * 50.0})
        };
        input(&mut s, vec![frame(vec![ev("up", release)])], true);
        let st = state(&mut s);
        assert_ne!(camera(&st).0, cam.0);
        assert_eq!(point(&st, "A").0, 2.0);
        // The drag of A is one undo step of its own.
        call(&mut s, "undo", json!({}));
        let st = state(&mut s);
        assert_eq!(point(&st, "A").0, 0.0);
        assert_eq!(st["undo"]["can_undo"], false);
    }

    /// Port of `ui_tests::pointer::a_drag_from_a_view_button_neither_pans_nor_takes_the_point_under_it`.
    #[test]
    fn a_drag_from_a_view_button_neither_pans_nor_takes_the_point_under_it() {
        let mut s = Server::new();
        let [x, y, w, h] = widget(&mut s, "zoom_in");
        let (bx, by) = (x + w / 2.0, y + h / 2.0);
        // A point under the button: the world point there in the view the
        // empty document opens with, which a document that fits keeps.
        let st = state(&mut s);
        let origin = &st["view"]["origin"];
        let scale = st["view"]["scale"].as_f64().unwrap();
        let (wx, wy) = (
            (bx - origin[0].as_f64().unwrap()) / scale,
            (origin[1].as_f64().unwrap() - by) / scale,
        );
        call(
            &mut s,
            "reset",
            json!({ "script": format!("A = ({wx}, {wy})\n") }),
        );
        let st = state(&mut s);
        assert_eq!(widget(&mut s, "zoom_in"), [x, y, w, h]);
        let (a, cam) = (point(&st, "A"), camera(&st));
        call(
            &mut s,
            "drag",
            json!({"from": {"widget": "zoom_in"}, "to": {"widget": "zoom_in", "dx": -200, "dy": -120}}),
        );
        let st = state(&mut s);
        assert_eq!(camera(&st), cam, "the drag panned the view or zoomed");
        assert_eq!(
            point(&st, "A"),
            a,
            "the drag took the point under the button"
        );
    }

    /// Port of `ui_tests::pointer::a_tap_on_the_tool_hint_places_no_point_under_it`.
    #[test]
    fn a_tap_on_the_tool_hint_places_no_point_under_it() {
        let mut s = Server::new();
        call(&mut s, "select_tool", json!({"tool": "point"}));
        let [x, y, _, _] = widget(&mut s, "hint_card");
        // The hint text: the second line of the card.
        let hint = (x + 16.0 + 20.0, y + 29.0 + 8.0);
        let tap = |p: (f64, f64)| {
            frame(vec![
                ev("move", at(p.0, p.1)),
                ev("down", at(p.0, p.1)),
                ev("up", at(p.0, p.1)),
            ])
        };
        input(&mut s, vec![tap(hint)], true);
        call(&mut s, "click", at(hint.0 + 10.0, hint.1));
        assert!(
            objects(&state(&mut s)).is_empty(),
            "a tap on the hint placed a point"
        );
        // Just above the card the drawing takes the tap.
        input(&mut s, vec![tap((hint.0, y - 12.0))], true);
        assert_eq!(objects(&state(&mut s)), ["A"]);
    }
}

/// The calls of a whole session: points with the point tool, a segment
/// between them, a drag with the move tool, two undos, a typed line.
fn scenario() -> Vec<(&'static str, Value)> {
    vec![
        ("reset", json!({})),
        ("select_tool", json!({"tool": "point"})),
        ("click", world(1.0, 1.0)),
        ("click", world(4.0, 2.0)),
        ("select_tool", json!({"tool": "segment"})),
        ("click", world(1.0, 1.0)),
        ("click", world(4.0, 2.0)),
        ("select_tool", json!({"tool": "move"})),
        (
            "drag",
            json!({"from": world(1.0, 1.0), "to": world(-2.0, 3.0), "steps": 12}),
        ),
        ("undo", json!({})),
        ("undo", json!({})),
        ("type_line", json!({"text": "C = (3, -1)"})),
        ("state", json!({"verbose": true})),
        ("widgets", json!({})),
        ("frame_stats", json!({})),
    ]
}

#[test]
fn a_full_session_places_drags_undoes_and_types() {
    let mut s = Server::new();
    let mut results = Vec::new();
    for (name, args) in scenario() {
        results.push(call(&mut s, name, args));
    }
    // The point tool made A and B; the segment tool reused them.
    assert_eq!(results[3]["changes"]["added"], json!(["B"]));
    assert_eq!(results[6]["changes"]["added"], json!(["f"]));
    // The drag moved A (one step), and dragging a free point moved the
    // segment with it.
    let dragged = &results[8]["changes"]["changed"];
    assert_eq!(dragged, &json!(["A", "f"]));
    // Two undos: the drag, then the segment.
    assert_eq!(results[9]["changes"]["changed"], json!(["A", "f"]));
    assert_eq!(results[10]["changes"]["removed"], json!(["f"]));
    let st = &results[12];
    assert_eq!(objects(st), ["A", "B", "C"]);
    assert_eq!(point(st, "A"), (1.0, 1.0));
    assert_eq!(point(st, "B"), (4.0, 2.0));
    assert_eq!(point(st, "C"), (3.0, -1.0));
    assert_eq!(st["tool"]["id"], "move");
    assert_eq!(st["undo"], json!({"can_undo": true, "can_redo": false}));
    assert_eq!(
        st["keyboard_open"], true,
        "the input bar opened the keyboard"
    );
    assert_eq!(st["objects"][2]["definition"], Value::Null);
    let rows: Vec<&str> = st["algebra"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["head"].as_str().unwrap())
        .collect();
    assert_eq!(rows, ["A = (1, 1)", "B = (4, 2)", "C = (3, -1)"]);
    let ids: Vec<&str> = results[13]["widgets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["id"].as_str().unwrap())
        .collect();
    for id in [
        "group:0",
        "group:8",
        "undo",
        "redo",
        "menu",
        "row:A",
        "marker:B",
        "row_menu:C",
        "input_bar",
        "zoom_in",
        "fit",
        "style_bar",
        "key:7",
        "keyboard_close",
    ] {
        assert!(ids.contains(&id), "{id} missing from {ids:?}");
    }
    assert!(results[14]["shapes"].as_u64().unwrap() > 0);
}

#[test]
fn the_same_calls_give_byte_identical_results() {
    let run = || {
        let mut s = Server::new();
        scenario()
            .into_iter()
            .enumerate()
            .map(|(i, (name, args))| {
                let line = request(
                    i as i64,
                    "tools/call",
                    json!({"name": name, "arguments": args}),
                );
                s.handle_line(&line).unwrap()
            })
            .collect::<Vec<String>>()
    };
    let (a, b) = (run(), run());
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x, y);
    }
}

#[test]
fn a_two_finger_pinch_zooms_and_leaves_the_points() {
    let mut s = Server::new();
    call(
        &mut s,
        "reset",
        json!({"profile": "touch", "script": "A = (0, 0)\nB = (3, 1)\n"}),
    );
    let st = state(&mut s);
    assert_eq!(st["window"]["profile"], "touch");
    let scale = camera(&st).2;
    let touch = |phase: &str, id: u64, x: f64, y: f64| json!({"type": "touch", "phase": phase, "id": id, "wx": x, "wy": y});
    let mut frames = vec![
        frame(vec![touch("start", 0, -2.0, -2.0)]),
        frame(vec![touch("start", 1, 2.0, -2.0)]),
    ];
    for i in 1..=4 {
        let d = 0.5 * i as f64;
        frames.push(frame(vec![
            touch("move", 0, -2.0 - d, -2.0),
            touch("move", 1, 2.0 + d, -2.0),
        ]));
    }
    // The fingers' world positions follow the zoom; lift them where they
    // are now (window points do not move).
    input(&mut s, frames, false);
    let r = input(
        &mut s,
        vec![
            frame(vec![
                json!({"type": "touch", "phase": "end", "id": 1, "x": 0, "y": 0}),
            ]),
            frame(vec![
                json!({"type": "touch", "phase": "end", "id": 0, "x": 0, "y": 0}),
            ]),
        ],
        true,
    );
    assert!(r["frames"].is_array());
    let st = state(&mut s);
    assert!(
        camera(&st).2 > scale * 1.2,
        "the view zoomed in: {:?}",
        camera(&st)
    );
    assert_eq!(point(&st, "A"), (0.0, 0.0));
    assert_eq!(point(&st, "B"), (3.0, 1.0));
    assert_eq!(st["undo"]["can_undo"], false);
}

#[test]
fn a_ggb_file_opens_from_base64() {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../golden/ggb/polygon/polygon-triangle-001.ggb"
    ))
    .unwrap();
    let mut s = Server::new();
    let st = call(
        &mut s,
        "open",
        json!({"base64": geo_mcp::base64_encode(&bytes), "name": "triangle.ggb"}),
    );
    assert!(!objects(&st).is_empty());
    assert_eq!(st["load_error"], Value::Null);
    let e = call_err(&mut s, "open", json!({"base64": "AAAA", "name": "x.ggb"}));
    assert!(!e.is_empty());
    assert!(
        state(&mut s)["load_error"].is_string(),
        "the window shows the error"
    );
}

#[test]
fn set_view_resize_and_world_positions_agree() {
    let mut s = Server::new();
    call(
        &mut s,
        "set_view",
        json!({"cx": 2.0, "cy": -1.0, "scale": 25.0}),
    );
    let st = state(&mut s);
    assert_eq!(camera(&st), (2.0, -1.0, 25.0));
    let [x, y, w, h] = {
        let r = st["view"]["rect"].as_array().unwrap();
        [0, 1, 2, 3].map(|i| r[i].as_f64().unwrap())
    };
    let origin = &st["view"]["origin"];
    assert_eq!(origin[0].as_f64().unwrap(), x + w / 2.0 - 2.0 * 25.0);
    assert_eq!(origin[1].as_f64().unwrap(), y + h / 2.0 - 25.0);
    let r = call(&mut s, "resize", json!({"width": 800, "height": 600}));
    assert!(r["changes"].is_object());
    let st = state(&mut s);
    assert_eq!(st["window"]["width"], 800.0);
    // The window keeps the top left world point on a resize: the centre
    // moved.
    let (cx, cy, scale) = camera(&st);
    assert_eq!(scale, 25.0);
    assert_ne!((cx, cy), (2.0, -1.0));
    // A click with the point tool at a world position lands there.
    call(&mut s, "select_tool", json!({"tool": "point"}));
    call(&mut s, "click", world(cx + 1.0, cy + 2.0));
    assert_eq!(point(&state(&mut s), "A"), (cx + 1.0, cy + 2.0));
}

#[test]
fn a_screenshot_is_written_or_says_why_not() {
    let mut s = Server::new();
    let path = std::env::temp_dir().join(format!("geokernel-mcp-test-{}.png", std::process::id()));
    let path = path.to_str().unwrap().to_owned();
    let r = rpc(
        &mut s,
        "tools/call",
        json!({"name": "screenshot", "arguments": {"path": path}}),
    );
    if r["result"]["isError"] == true {
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("GPU adapter"), "{text}");
    } else {
        assert_eq!(r["result"]["structuredContent"]["path"], path.as_str());
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let _ = std::fs::remove_file(&path);
    }
}

#[test]
fn the_headless_feature_is_not_a_default_of_the_app() {
    let manifest = include_str!("../../geo-app/Cargo.toml");
    let features = manifest
        .split("[features]")
        .nth(1)
        .expect("geo-app has a [features] table")
        .split("\n[")
        .next()
        .unwrap();
    assert!(features.contains("headless = []"), "{features}");
    for line in features.lines() {
        let line = line.trim();
        if line.starts_with("default") {
            assert!(!line.contains("headless"), "{line}");
        }
    }
    // Only this crate turns it on.
    for dep in manifest.lines().filter(|l| l.contains("features")) {
        assert!(!dep.contains("headless"), "{dep}");
    }
}

/// The value field of a free number is put into the row with `ui.put`,
/// which moves the list's cursor to the field's bottom: the next row starts
/// 12 points early and covers the bottom of the number's row.
#[test]
#[ignore = "app bug: the row after a free number starts inside it (algebra.rs, ui.put of the value field)"]
fn a_free_number_row_is_as_tall_as_it_is_painted() {
    let mut s = Server::new();
    call(&mut s, "reset", json!({"script": "r = 2\nA = (0, 0)\n"}));
    let number = widget(&mut s, "row:r");
    let next = widget(&mut s, "row:A");
    assert_eq!(
        number[3], 50.0,
        "the number's row is painted 50 points tall"
    );
    assert_eq!(
        next[1],
        number[1] + number[3],
        "the next row starts inside it"
    );
}
