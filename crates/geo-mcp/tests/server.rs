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

/// A frame 50 ms after the previous one: two of them outlast the drag
/// delay after a press (moves within it are dropped).
fn slow_frame(events: Vec<Value>) -> Value {
    json!({ "dt": 0.05, "events": events })
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
        // 2026-07-28 requires a cache lifetime and scope on list results.
        assert!(r["result"]["ttlMs"].as_u64().unwrap() > 0);
        assert_eq!(r["result"]["cacheScope"], "public");
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
        // Every version the server accepts, by `_meta` or `initialize`.
        assert_eq!(
            r["error"]["data"]["supported"],
            json!(["2026-07-28", "2025-11-25", "2025-06-18", "2024-11-05"])
        );
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
        protocol::serve(&input[..], &mut out, |l| s.handle_input(l)).unwrap();
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
            .filter_map(|l| protocol::batch_line(&mut session, protocol::Line::Text(l)))
            .map(|l| serde_json::from_str(&l).unwrap())
            .collect();
        assert_eq!(out.len(), 4);
        assert_eq!(out[0]["ok"], true);
        assert_eq!(out[1]["result"]["objects"][0]["name"], "A");
        assert_eq!(out[2]["ok"], false);
        assert_eq!(out[2]["tool"], "click");
        assert_eq!(out[3]["ok"], false);
    }

    #[test]
    fn a_line_over_the_limit_is_refused_and_the_next_one_is_read() {
        let mut s = Server::new();
        let mut input = Vec::new();
        input.extend_from_slice(request(1, "ping", json!({})).as_bytes());
        input.push(b'\n');
        input.extend_from_slice(&[b'x'; 5000]);
        input.push(b'\n');
        input.extend_from_slice(request(2, "ping", json!({})).as_bytes());
        let mut out = Vec::new();
        protocol::serve_bounded(&input[..], &mut out, 100, |l| s.handle_input(l)).unwrap();
        let lines: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["id"], 1);
        assert_eq!(lines[1]["error"]["code"], -32700);
        assert_eq!(lines[2]["id"], 2);
    }

    #[test]
    fn a_panic_in_the_window_is_reported_and_the_window_replaced() {
        let mut s = Server::new();
        call(
            &mut s,
            "reset",
            json!({"script": "A = (1, 2)\n", "width": 900, "profile": "touch"}),
        );
        s.session.panic_on_next_call();
        let e = call_err(&mut s, "state", json!({}));
        assert!(e.contains("panicked in `state`"), "{e}");
        assert!(e.contains("a panic asked for by a test"), "{e}");
        assert!(e.contains("fresh default window"), "{e}");
        let st = state(&mut s);
        assert!(objects(&st).is_empty());
        assert_eq!(
            st["window"],
            json!({"width": 1400.0, "height": 900.0, "profile": "desktop", "scale": 1.0, "focused": true})
        );
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

    /// Port of `ui_tests::gestures::a_release_and_a_press_in_one_frame_end_the_drag_and_start_a_new_press`:
    /// the same window points, worked out once from the opening view, as
    /// the original does. The first assertion fails with the earlier code
    /// (the release decided at the frame's end, at the new press).
    #[test]
    fn a_release_and_a_press_in_one_frame_end_the_drag_and_start_a_new_press() {
        let mut s = Server::new();
        let st = call(&mut s, "reset", json!({"script": "A = (0, 0)\n"}));
        let px = |x: f64, y: f64| {
            let o = &st["view"]["origin"];
            let k = st["view"]["scale"].as_f64().unwrap();
            (
                o[0].as_f64().unwrap() + x * k,
                o[1].as_f64().unwrap() - y * k,
            )
        };
        let from = px(0.0, 0.0);
        let to = (from.0 + 100.0, from.1);
        let empty = px(-6.0, 4.0);
        let mut frames = vec![
            frame(vec![ev("move", at(from.0, from.1))]),
            frame(vec![ev("down", at(from.0, from.1))]),
        ];
        // Moves 50 ms apart: the first one comes within the drag delay and
        // is dropped, the drag follows from the second one.
        for i in 1..=5 {
            let t = i as f64 / 5.0;
            frames.push(slow_frame(vec![ev(
                "move",
                at(from.0 + (to.0 - from.0) * t, from.1),
            )]));
        }
        // The release of the drag and a press on empty space in one frame.
        frames.push(frame(vec![
            ev("up", at(to.0, to.1)),
            ev("move", at(empty.0, empty.1)),
            ev("down", at(empty.0, empty.1)),
        ]));
        input(&mut s, frames, false);
        let st = state(&mut s);
        assert_eq!(point(&st, "A").0, 2.0, "the drag ended where it was");
        let cam = camera(&st);
        let mut frames = Vec::new();
        for i in 1..=5 {
            frames.push(slow_frame(vec![ev(
                "move",
                at(empty.0 + 20.0 * i as f64, empty.1),
            )]));
        }
        frames.push(frame(vec![ev("up", at(empty.0 + 100.0, empty.1))]));
        input(&mut s, frames, true);
        let st = state(&mut s);
        assert_ne!(camera(&st).0, cam.0, "the second press panned the view");
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
        // The precondition: A is drawn under the button.
        let o = &st["view"]["origin"];
        let k = st["view"]["scale"].as_f64().unwrap();
        let drawn = (
            o[0].as_f64().unwrap() + a.0 * k,
            o[1].as_f64().unwrap() - a.1 * k,
        );
        assert!(
            (drawn.0 - bx).abs() < 1.0 && (drawn.1 - by).abs() < 1.0,
            "A is drawn at {drawn:?}, the button's centre is ({bx}, {by})"
        );
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
    // Points A and B, the segment, the drag; two undos; the typed line.
    assert_eq!(
        st["undo"],
        json!({"can_undo": true, "can_redo": false, "undo_depth": 3, "redo_depth": 0})
    );
    assert_eq!(results[10]["changes"]["undo"]["redo_depth"], 2);
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

/// The value field of a free number sits inside its row without moving the
/// list's cursor: the next row starts where the number's row ends (with
/// `ui.put` it started 12 points early and covered its bottom).
#[test]
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

/// A list of 40 rows is taller than the panel: the rows in view are named
/// by their objects at any scroll offset, and the rows outside it are
/// listed too, where they would be, with `in_view: false`.
#[test]
fn rows_are_named_in_and_out_of_view_at_any_scroll_offset() {
    let mut s = Server::new();
    let script: String = (0..40).map(|i| format!("P_{{{i}}} = ({i}, 0)\n")).collect();
    call(&mut s, "reset", json!({ "script": script }));
    let names: Vec<String> = (0..40).map(|i| format!("P_{{{i}}}")).collect();
    let rows = |s: &mut Server| -> Vec<Value> {
        let list = call(s, "widgets", json!({}));
        list["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|w| w["id"].as_str().unwrap().starts_with("row:"))
            .cloned()
            .collect()
    };
    let mut first_in_view = Vec::new();
    for scroll in [0.0, -1100.0] {
        if scroll != 0.0 {
            let first = widget(&mut s, "row:P_{0}");
            let p = at(first[0] + 100.0, 300.0);
            let mut frames = vec![frame(vec![ev("move", p.clone())])];
            let mut wheel = ev("wheel", p);
            wheel["dy"] = json!(scroll);
            frames.push(frame(vec![wheel]));
            input(&mut s, frames, true);
        }
        let list = rows(&mut s);
        let ids: Vec<&str> = list.iter().map(|w| w["id"].as_str().unwrap()).collect();
        let expected: Vec<String> = names.iter().map(|n| format!("row:{n}")).collect();
        assert_eq!(ids, expected, "scrolled by {scroll}: every row, in order");
        for (w, n) in list.iter().zip(&names) {
            assert_eq!(w["object"], json!(n));
        }
        let in_view: Vec<usize> = (0..40)
            .filter(|&i| list[i].get("in_view").is_none())
            .collect();
        assert!(!in_view.is_empty() && in_view.len() < 40, "{in_view:?}");
        // The rows in view follow one another; the others are flagged.
        assert!(in_view.windows(2).all(|w| w[1] == w[0] + 1), "{in_view:?}");
        for (i, w) in list.iter().enumerate() {
            if !in_view.contains(&i) {
                assert_eq!(w["in_view"], json!(false), "{w}");
            }
        }
        // The algebra report names every row too.
        let st = call(&mut s, "state", json!({"verbose": true}));
        let algebra = st["algebra"].as_array().unwrap();
        assert_eq!(algebra.len(), 40);
        for (r, n) in algebra.iter().zip(&names) {
            assert_eq!(r["name"], json!(n));
            assert!(r["head"].is_string(), "{r}");
        }
        first_in_view.push(in_view[0]);
    }
    assert!(
        first_in_view[1] > first_in_view[0] + 10,
        "{first_in_view:?}"
    );
}

/// Through the frame loop the gesture rules hold (values from the desktop
/// metrics, which follow the reference app's `EuclidianController`): a still
/// press is a tap however long it is held (no long press for the mouse),
/// moves within the drag delay after the press are dropped
/// (`shouldCancelDrag`), the first move after it takes the point straight
/// to the pointer (no slop), and a release is a click unless the drag went
/// beyond the threshold (`isDraggingBeyondThreshold`, one move late). A
/// server that applied clicks and drags directly would fail these.
#[test]
fn the_drag_delay_and_the_click_threshold_decide_through_the_frame_loop() {
    let m = geo_app_core::metrics::Metrics::desktop();
    let mut s = Server::new();
    call(&mut s, "select_tool", json!({"tool": "point"}));
    let p = world(2.0, 2.0);
    input(
        &mut s,
        vec![
            frame(vec![ev("move", p.clone())]),
            frame(vec![ev("down", p.clone())]),
            json!({"dt": 3.0, "events": []}),
            json!({"dt": 0.0, "events": [ev("up", p)]}),
        ],
        true,
    );
    assert_eq!(objects(&state(&mut s)), ["A"], "a long press is a tap");

    // A is off the grid and stays off it here: a free point near a grid
    // crossing is captured to it (`transformCoords`).
    call(&mut s, "reset", json!({"script": "A = (0.5, 0.5)\n"}));
    let delay = m.drag_delay;
    let threshold = m.drag_threshold as f64;
    // Presses A, moves it by `by` in `moves` steps `dt` apart, releases.
    let press_and_move = |s: &mut Server, by: f64, moves: usize, dt: f64| {
        let st = state(s);
        let o = &st["view"]["origin"];
        let a = point(&st, "A");
        let k = st["view"]["scale"].as_f64().unwrap();
        let p = (
            o[0].as_f64().unwrap() + a.0 * k,
            o[1].as_f64().unwrap() - a.1 * k,
        );
        let mut frames = vec![
            frame(vec![ev("move", at(p.0, p.1))]),
            frame(vec![ev("down", at(p.0, p.1))]),
        ];
        for i in 1..=moves {
            let x = p.0 + by * i as f64 / moves as f64;
            frames.push(json!({"dt": dt, "events": [ev("move", at(x, p.1))]}));
        }
        frames.push(json!({"dt": 0.3, "events": [ev("up", at(p.0 + by, p.1))]}));
        input(s, frames, true);
        state(s)
    };
    let st = press_and_move(&mut s, 30.0, 4, 0.2 * delay);
    assert_eq!(
        point(&st, "A"),
        (0.5, 0.5),
        "moves within the delay dragged"
    );
    call(&mut s, "reset", json!({"script": "A = (0.5, 0.5)\n"}));
    let st = press_and_move(&mut s, threshold - 4.0, 2, delay + 0.05);
    let (x, k) = (point(&st, "A").0, st["view"]["scale"].as_f64().unwrap());
    assert!(
        (x - 0.5 - (threshold - 4.0) / k).abs() < 1e-6,
        "slop held back: {x}"
    );
    let st = press_and_move(&mut s, threshold + 24.0, 4, delay + 0.05);
    let k = st["view"]["scale"].as_f64().unwrap();
    assert!(
        point(&st, "A").0 > x + (threshold + 20.0) / k,
        "a drag beyond the threshold did not follow"
    );

    // The threshold decides the click: a press on a point with the
    // show/hide tool whose drag (two moves after the delay, so the threshold
    // is checked) ends 9 or 10 px away is a click and hides the point; 11 px
    // is a drag and hides nothing. The distances are the reference app's
    // (`DRAG_THRESHOLD` 10), not read from the metrics, so that this fails
    // with a threshold of 6 (9 px no click) or of 20 (11 px a click). (The
    // point tool used to show this; it now places its point on the press,
    // spec E1.)
    for (by, click) in [(9.0, true), (10.0, true), (11.0, false)] {
        call(&mut s, "reset", json!({"script": "A = (2, 2)\n"}));
        call(&mut s, "select_tool", json!({"tool": "show_hide"}));
        let st = state(&mut s);
        let o = &st["view"]["origin"];
        let k = st["view"]["scale"].as_f64().unwrap();
        let p = (
            o[0].as_f64().unwrap() + 2.0 * k,
            o[1].as_f64().unwrap() - 2.0 * k,
        );
        input(
            &mut s,
            vec![
                frame(vec![ev("move", at(p.0, p.1))]),
                frame(vec![ev("down", at(p.0, p.1))]),
                json!({"dt": delay + 0.05, "events": [ev("move", at(p.0 + by, p.1))]}),
                json!({"dt": 0.05, "events": [ev("move", at(p.0 + by, p.1))]}),
                json!({"dt": 0.05, "events": [ev("up", at(p.0 + by, p.1))]}),
            ],
            true,
        );
        let hidden = objects(&state(&mut s))[0]["visible"] == json!(false);
        assert_eq!(hidden, click, "a release {by} px from the press");
    }
}

/// Reading never changes the window: any number of `state` (both modes),
/// `widgets` and `frame_stats` calls between two inputs leave the results
/// and the next frame's counters as they are.
#[test]
fn reading_the_window_changes_nothing() {
    let calls = [
        ("select_tool", json!({"tool": "point"})),
        ("click", world(1.0, 1.0)),
        ("click", world(3.0, 1.0)),
        ("input", json!({"frames": [{"events": []}]})),
        ("frame_stats", json!({})),
        ("type_line", json!({"text": "r = 5"})),
        ("input", json!({"frames": [{"events": []}]})),
        ("frame_stats", json!({})),
    ];
    let run = |read: bool| {
        let mut s = Server::new();
        let mut out = Vec::new();
        for (name, args) in &calls {
            if read {
                for _ in 0..3 {
                    call(&mut s, "state", json!({}));
                    call(&mut s, "state", json!({"verbose": true}));
                    call(&mut s, "widgets", json!({}));
                }
            }
            out.push(call(&mut s, name, args.clone()));
        }
        out
    };
    assert_eq!(run(false), run(true));
}

#[test]
fn a_retina_window_lays_out_in_points_and_clicks_land() {
    let mut s = Server::new();
    let at_one = widget(&mut s, "group:1");
    let st = call(&mut s, "reset", json!({"scale": 2.0}));
    assert_eq!(st["window"]["scale"], 2.0);
    assert_eq!(
        widget(&mut s, "group:1"),
        at_one,
        "rectangles are in points"
    );
    call(&mut s, "select_tool", json!({"tool": "point"}));
    call(&mut s, "click", world(1.0, 1.0));
    assert_eq!(point(&state(&mut s), "A"), (1.0, 1.0));
    let e = call_err(&mut s, "reset", json!({"scale": 9}));
    assert!(e.contains("scale"), "{e}");
}

#[test]
fn a_touch_window_clicks_with_a_finger() {
    let mut s = Server::new();
    call(&mut s, "reset", json!({"profile": "touch"}));
    call(&mut s, "select_tool", json!({"tool": "point"}));
    let before = state(&mut s)["frames"].as_u64().unwrap();
    let r = call(&mut s, "click", world(1.0, 1.0));
    // Touch down and lift, one frame each: no hover frame before them.
    let settle = r["settle_frames"].as_u64().unwrap();
    assert_eq!(r["frames"].as_u64().unwrap() - before, 2 + settle);
    assert_eq!(r["changes"]["added"], json!(["A"]));
    let st = call(&mut s, "state", json!({"verbose": true}));
    assert_eq!(
        st["hovered"],
        Value::Null,
        "the pointer is gone after a lift"
    );
    // A mouse on the same window moves first, in a frame of its own.
    let before = st["frames"].as_u64().unwrap();
    let r = call(
        &mut s,
        "click",
        json!({"wx": -2, "wy": 1, "pointer": "mouse"}),
    );
    let settle = r["settle_frames"].as_u64().unwrap();
    assert_eq!(r["frames"].as_u64().unwrap() - before, 3 + settle);
    assert_eq!(r["changes"]["added"], json!(["B"]));
}

#[test]
fn viewport_commands_and_focus_reach_the_caller() {
    let mut s = Server::new();
    let r = call(&mut s, "click", json!({"widget": "fullscreen"}));
    assert_eq!(r["viewport_commands"], json!(["Fullscreen(true)"]));
    assert_eq!(
        state(&mut s)["window"]["width"],
        1400.0,
        "nothing carried out"
    );
    input(
        &mut s,
        vec![frame(vec![json!({"type": "focus", "focused": false})])],
        true,
    );
    assert_eq!(state(&mut s)["window"]["focused"], false);
}

#[test]
fn files_open_by_path_and_odd_paths_are_refused() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
    let mut s = Server::new();
    let st = call(
        &mut s,
        "open",
        json!({"path": format!("{dir}/triangle.geo")}),
    );
    assert!(!objects(&st).is_empty());
    // A device file is the other kind of non-regular path; Windows has no `/dev/null`.
    let odd: &[&str] = if cfg!(unix) {
        &[dir, "/dev/null"]
    } else {
        &[dir]
    };
    for &path in odd {
        let e = call_err(&mut s, "open", json!({ "path": path }));
        assert!(e.contains("not a regular file"), "{e}");
    }
    let e = call_err(
        &mut s,
        "open",
        json!({"text": "A = (1, 1)", "name": "../x.geo"}),
    );
    assert!(e.contains("plain file name"), "{e}");
    for path in ["/dev/null.png", "/dev/stdout", "shot.txt"] {
        let e = call_err(&mut s, "screenshot", json!({ "path": path }));
        assert!(!e.contains("GPU"), "{path}: {e}");
    }
}

#[test]
fn oversized_input_is_refused_before_anything_runs() {
    let mut s = Server::new();
    let frames = state(&mut s)["frames"].clone();
    let many: Vec<Value> = (0..=geo_mcp::limits::MAX_EVENTS)
        .map(|_| json!({"type": "gone"}))
        .collect();
    let e = call_err(
        &mut s,
        "input",
        json!({"frames": [{"events": []}, {"events": many}]}),
    );
    assert!(e.contains("events per frame"), "{e}");
    assert_eq!(state(&mut s)["frames"], frames, "a frame ran");
    let long = "x".repeat(geo_mcp::limits::MAX_TEXT + 1);
    let e = call_err(&mut s, "type_line", json!({ "text": long }));
    assert!(e.contains("longer than"), "{e}");
}

#[test]
fn values_read_as_the_algebra_view_shows_them() {
    let mut s = Server::new();
    let st = call(
        &mut s,
        "reset",
        json!({"script": "A = (1, 0)\nB = (0, 0)\nC = (0, 1)\nα = Angle(A, B, C)\na = 1\nf(x) = a*x\n"}),
    );
    let alpha = &st["objects"][3];
    assert_eq!(alpha["value"], "90°");
    assert_eq!(alpha["raw_value"], "1.57");
    let st = call(&mut s, "state", json!({"verbose": true}));
    let row = |name: &str| {
        st["algebra"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == name)
            .unwrap()
            .clone()
    };
    assert_eq!(row("α")["value"], "90°");
    assert_eq!(row("a")["head"], "a =");
    // A function changes with what it depends on.
    let r = call(
        &mut s,
        "drag",
        json!({"from": {"widget": "value:a"}, "to": {"widget": "value:a", "dx": 40}}),
    );
    assert_eq!(r["changes"]["changed"], json!(["a", "f"]));
}
