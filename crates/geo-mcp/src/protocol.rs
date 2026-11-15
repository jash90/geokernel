//! The Model Context Protocol over stdio: one JSON-RPC 2.0 message per
//! line. Both eras of the protocol are served:
//!
//! - modern (revision 2026-07-28): every request carries its protocol
//!   version and client capabilities in `params._meta`; `server/discover`
//!   tells what the server supports; results carry `resultType`;
//! - legacy (revisions 2025-11-25, 2025-06-18 and 2024-11-05): an
//!   `initialize` handshake first, then plain requests. 2025-03-26 is not
//!   offered: it requires accepting JSON-RPC batches, which this server
//!   does not.
//!
//! Also the batch mode: one tool call per line in, one result per line out,
//! through the same handlers.

use crate::limits::MAX_LINE;
use crate::session::{CallError, Session};
use serde_json::{json, Map, Value as Json};

/// The modern revision served.
pub const MODERN: &str = "2026-07-28";
/// The legacy revisions an `initialize` may name, newest first.
pub const LEGACY: [&str; 3] = ["2025-11-25", "2025-06-18", "2024-11-05"];
/// How long a client may keep the tool list (2026-07-28 `ttlMs`): it never
/// changes while the server runs.
const LIST_TTL_MS: u64 = 3_600_000;

const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const META_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const UNSUPPORTED_VERSION: i64 = -32022;

const INSTRUCTIONS: &str = "Drives the geokernel window without a window. Positions are window \
points from the top left (`x`, `y`), world coordinates (`wx`, `wy`) or a widget id from \
`widgets`. User actions go through the window's real frame loop; `input` gives full control \
over frames, events and the clock. Start with `reset`, read with `state` and `widgets`.";

fn server_info() -> Json {
    json!({"name": "geokernel-mcp", "version": env!("CARGO_PKG_VERSION")})
}

fn pos_schema() -> Json {
    json!({
        "x": {"type": "number", "description": "Window x in points (with y)."},
        "y": {"type": "number", "description": "Window y in points (with x)."},
        "wx": {"type": "number", "description": "World x (with wy), through the view as last painted."},
        "wy": {"type": "number", "description": "World y (with wx)."},
        "widget": {"type": "string", "description": "A widget id or label from `widgets`; its centre."},
        "dx": {"type": "number", "description": "Offset added to the position, in points."},
        "dy": {"type": "number", "description": "Offset added to the position, in points."}
    })
}

fn object_schema(properties: Json, required: &[&str]) -> Json {
    json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false})
}

fn with(mut base: Json, extra: Json) -> Json {
    if let (Some(b), Some(e)) = (base.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            b.insert(k.clone(), v.clone());
        }
    }
    base
}

/// The tools, as `tools/list` returns them.
pub fn tools() -> Vec<Json> {
    let position = object_schema(pos_schema(), &[]);
    let modifiers = json!({
        "type": "array",
        "items": {"enum": ["shift", "alt", "ctrl", "mac_cmd", "command"]},
        "description": "Held modifiers; `command` is the platform's shortcut key."
    });
    let button =
        json!({"enum": ["primary", "secondary", "middle"], "description": "Default primary."});
    let pointer = json!({"enum": ["mouse", "touch"], "description": "Default: touch on a touch window, mouse otherwise."});
    let event = json!({
        "type": "object",
        "description": "One event. type: move | down | up (x/y, wx/wy or widget; button, modifiers) | gone | wheel (dx, dy, unit point|line|page) | zoom (factor) | touch (phase start|move|end|cancel, id, position; egui-winit's pointer events for the followed finger are added unless raw) | key (key: egui key name such as Escape, Enter, Z; action tap|press|release; modifiers) | text (text) | modifiers (modifiers) | focus (focused).",
        "properties": with(pos_schema(), json!({
            "type": {"enum": ["move", "down", "up", "gone", "wheel", "zoom", "touch", "key", "text", "modifiers", "focus"]},
            "button": button, "modifiers": modifiers,
            "unit": {"enum": ["point", "line", "page"]}, "factor": {"type": "number"},
            "phase": {"enum": ["start", "move", "end", "cancel"]},
            "id": {"type": "integer", "minimum": 0}, "raw": {"type": "boolean"},
            "key": {"type": "string"}, "action": {"enum": ["tap", "press", "release"]},
            "text": {"type": "string"}, "focused": {"type": "boolean"}
        })),
        "required": ["type"]
    });
    let def = |name: &str, description: &str, schema: Json| json!({"name": name, "description": description, "inputSchema": schema});
    vec![
        def("reset", "A new window: size in points (default 1400 x 900), pixels per point (`scale`, default 1 for deterministic geometry; a Retina display is 2), input profile (desktop or touch), optional initial script. Settles and returns the state.",
            object_schema(json!({
                "width": {"type": "number"}, "height": {"type": "number"},
                "scale": {"type": "number", "minimum": 0.5, "maximum": 4},
                "profile": {"enum": ["desktop", "touch"]},
                "script": {"type": "string", "description": "A geokernel script to start with."}
            }), &[])),
        def("open", "Opens a document as dropping a file on the window does: a file path, inline script text, or file bytes as base64 (a .ggb). `name` gives the format by its extension.",
            object_schema(json!({
                "path": {"type": "string"}, "text": {"type": "string"}, "base64": {"type": "string"},
                "name": {"type": "string", "description": "File name; default inline.geo for text, inline.ggb for base64."}
            }), &[])),
        def("state", "The window's exact state: objects in construction order (name, type, coords, value, definition, defined, visible, free), active tool and pending picks, camera and view, undo and redo, selection, input bar, notices and errors, keyboard, window size and profile. verbose adds algebra rows, regions, the pointer automaton, hover and decoration.",
            object_schema(json!({"verbose": {"type": "boolean"}}), &[])),
        def("widgets", "Named rectangles of the last frame ([x, y, width, height] in window points): tool group buttons and open drop-down entries with tool ids, undo/redo/menu, algebra rows with their marker, menu and value field, the input bar, view and style buttons, keyboard keys, the hint card and its close cross, the notice, menu entries and dialog controls.",
            object_schema(json!({"kind": {"type": "string", "description": "Only widgets of this kind (tool_group, tool_entry, row, key, ...)."}}), &[])),
        def("input", "Runs frames through the window's frame loop. Each frame: dt (seconds since the previous frame, default 1/60) and a list of events, applied in order within the frame. Returns per frame what changed. settle runs empty frames afterwards while the window animates.",
            object_schema(json!({
                "frames": {"type": "array", "items": {"type": "object", "properties": {
                    "dt": {"type": "number", "minimum": 0}, "events": {"type": "array", "items": event}
                }, "additionalProperties": false}},
                "settle": {"type": "boolean"}, "state": {"type": "boolean", "description": "Also return the final state."}
            }), &["frames"])),
        def("click", "Clicks a position (count times), then settles. A mouse moves there in a frame of its own, then presses and releases, one frame each; a finger touches down and lifts, one frame each, through egui-winit's touch emulation. `pointer` defaults to the window's profile.",
            with(position.clone(), json!({"properties": with(pos_schema(), json!({
                "button": button, "count": {"type": "integer", "minimum": 1, "maximum": 3}, "modifiers": modifiers,
                "pointer": pointer
            }))}))),
        def("drag", "Presses (or touches down) at `from`, moves to `to` in `steps` frames over `duration` seconds, releases, then settles. `pointer` defaults to the window's profile.",
            object_schema(json!({
                "from": position, "to": position, "steps": {"type": "integer", "minimum": 1, "maximum": 1000},
                "duration": {"type": "number", "minimum": 0}, "button": button, "pointer": pointer
            }), &["from", "to"])),
        def("type_line", "Clicks the input bar, types the text and (unless submit is false) presses Enter, then settles.",
            object_schema(json!({"text": {"type": "string"}, "submit": {"type": "boolean"}}), &["text"])),
        def("select_tool", "Selects a tool by clicking its toolbar group and, for a group of several tools, the drop-down entry.",
            object_schema(json!({"tool": {"enum": crate::report::tool_ids()}}), &["tool"])),
        def("undo", "Clicks the toolbar's undo button.", object_schema(json!({}), &[])),
        def("redo", "Clicks the toolbar's redo button.", object_schema(json!({}), &[])),
        def("resize", "Changes the window size (points) for the next frames, runs a frame and settles.",
            object_schema(json!({"width": {"type": "number"}, "height": {"type": "number"}}), &[])),
        def("set_view", "Sets the camera: the world point at the centre of the drawing (cx, cy) and the scale in points per unit.",
            object_schema(json!({"cx": {"type": "number"}, "cy": {"type": "number"}, "scale": {"type": "number"}}), &[])),
        def("screenshot", "Renders the last frame to a PNG at `path` and returns the path. Needs a GPU adapter; without one it fails with a clear message.",
            object_schema(json!({"path": {"type": "string"}}), &["path"])),
        def("frame_stats", "The frame benchmark's deterministic counters of the last frame (shapes, glyphs, meshes, vertices, indices, passes, rows built, rebuilds, repaint).",
            object_schema(json!({}), &[])),
    ]
}

/// A JSON-RPC response with a result.
fn reply(id: &Json, result: Json) -> Json {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// A JSON-RPC error response.
fn error(id: &Json, code: i64, message: &str, data: Option<Json>) -> Json {
    let mut e = json!({"code": code, "message": message});
    if let Some(d) = data {
        e["data"] = d;
    }
    json!({"jsonrpc": "2.0", "id": id, "error": e})
}

/// Runs a tool and turns the outcome into a `tools/call` result (`None`:
/// there is no such tool). A panic inside the window is caught and reported
/// as a failed call, and the window is replaced by a fresh default one.
fn call_tool(session: &mut Session, name: &str, args: &Json) -> Option<Result<Json, String>> {
    let outcome =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| session.call(name, args)));
    match outcome {
        Ok(Ok(v)) => Some(Ok(v)),
        Ok(Err(CallError::Failed(e))) => Some(Err(e)),
        Ok(Err(CallError::UnknownTool(_))) => None,
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "no message".to_owned());
            *session = Session::new();
            let (w, h) = crate::session::DEFAULT_SIZE;
            Some(Err(format!(
                "internal error: the window panicked in `{name}`: {message}. It was replaced \
                 by a fresh default window ({w} x {h} points, desktop profile, 1 pixel per \
                 point, empty document): earlier state is gone"
            )))
        }
    }
}

/// One line of input as read.
pub enum Line<'a> {
    Text(&'a str),
    NotUtf8,
    /// Longer than the limit; skipped up to its newline.
    TooLong,
}

/// The MCP server: a session and the protocol state.
pub struct Server {
    pub session: Session,
}

impl Default for Server {
    fn default() -> Self {
        Server::new()
    }
}

impl Server {
    pub fn new() -> Server {
        Server {
            session: Session::new(),
        }
    }

    /// Handles one line as read by [`serve`].
    pub fn handle_input(&mut self, line: Line) -> Option<String> {
        let message = match line {
            Line::Text(l) => return self.handle_line(l),
            Line::NotUtf8 => "Parse error: not UTF-8".to_owned(),
            Line::TooLong => "Parse error: the line is too long".to_owned(),
        };
        Some(error(&Json::Null, PARSE_ERROR, &message, None).to_string())
    }

    /// Handles one line of input; returns the line to write back, if any
    /// (notifications and blank lines get none).
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let msg: Json = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(e) => {
                return Some(
                    error(&Json::Null, PARSE_ERROR, &format!("Parse error: {e}"), None).to_string(),
                )
            }
        };
        self.handle(&msg).map(|r| r.to_string())
    }

    /// Handles one parsed message.
    pub fn handle(&mut self, msg: &Json) -> Option<Json> {
        let Some(obj) = msg.as_object() else {
            let what = if msg.is_array() {
                "Invalid Request: batches are not supported"
            } else {
                "Invalid Request: not an object"
            };
            return Some(error(&Json::Null, INVALID_REQUEST, what, None));
        };
        // A notification (no id) never gets a reply, not even an error.
        let id = obj.get("id")?;
        if !(id.is_string() || id.is_i64() || id.is_u64()) {
            return Some(error(
                &Json::Null,
                INVALID_REQUEST,
                "Invalid Request: bad id",
                None,
            ));
        }
        if obj.get("jsonrpc") != Some(&json!("2.0")) {
            return Some(error(
                id,
                INVALID_REQUEST,
                "Invalid Request: jsonrpc must be \"2.0\"",
                None,
            ));
        }
        let Some(method) = obj.get("method").and_then(Json::as_str) else {
            return Some(error(
                id,
                INVALID_REQUEST,
                "Invalid Request: no method",
                None,
            ));
        };
        let empty = Json::Object(Map::new());
        let params = match obj.get("params") {
            None => &empty,
            Some(p @ Json::Object(_)) => p,
            Some(_) => return Some(error(id, INVALID_PARAMS, "params must be an object", None)),
        };
        // The era of the request: modern requests name their version.
        let meta = params.get("_meta");
        let version = meta.and_then(|m| m.get(META_VERSION));
        let modern = version.is_some();
        if let Some(v) = version {
            if v.as_str() != Some(MODERN) {
                // Every version served: the modern one by `_meta`, the
                // legacy ones by `initialize`.
                let mut supported = vec![MODERN];
                supported.extend(LEGACY);
                return Some(error(
                    id,
                    UNSUPPORTED_VERSION,
                    "Unsupported protocol version",
                    Some(json!({"supported": supported, "requested": v})),
                ));
            }
            if meta.and_then(|m| m.get(META_CAPABILITIES)).is_none() {
                return Some(error(
                    id,
                    INVALID_PARAMS,
                    &format!("_meta needs `{META_CAPABILITIES}`"),
                    None,
                ));
            }
        }
        let result = match method {
            "initialize" if !modern => Ok(self.initialize(params)),
            "server/discover" => Ok(json!({
                "supportedVersions": [MODERN],
                "capabilities": {"tools": {}},
                "_meta": {"io.modelcontextprotocol/serverInfo": server_info()},
                "ttlMs": LIST_TTL_MS,
                "cacheScope": "public",
            })),
            "ping" => Ok(json!({})),
            // 2026-07-28 requires `ttlMs` and `cacheScope` on list results.
            "tools/list" if modern => Ok(json!({
                "tools": tools(),
                "ttlMs": LIST_TTL_MS,
                "cacheScope": "public",
            })),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => self.tools_call(params),
            _ => Err((METHOD_NOT_FOUND, format!("Method not found: {method}"))),
        };
        Some(match result {
            Ok(mut r) => {
                if modern {
                    r["resultType"] = json!("complete");
                }
                reply(id, r)
            }
            Err((code, message)) => error(id, code, &message, None),
        })
    }

    fn initialize(&mut self, params: &Json) -> Json {
        let asked = params.get("protocolVersion").and_then(Json::as_str);
        let version = LEGACY
            .iter()
            .find(|v| Some(**v) == asked)
            .copied()
            .unwrap_or(LEGACY[0]);
        json!({
            "protocolVersion": version,
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": server_info(),
            "instructions": INSTRUCTIONS,
        })
    }

    fn tools_call(&mut self, params: &Json) -> Result<Json, (i64, String)> {
        let name = params
            .get("name")
            .and_then(Json::as_str)
            .ok_or((INVALID_PARAMS, "tools/call needs a tool `name`".to_owned()))?;
        let args = match params.get("arguments") {
            None | Some(Json::Null) => Json::Object(Map::new()),
            Some(a @ Json::Object(_)) => a.clone(),
            Some(_) => return Err((INVALID_PARAMS, "`arguments` must be an object".to_owned())),
        };
        match call_tool(&mut self.session, name, &args) {
            None => Err((INVALID_PARAMS, format!("Unknown tool: {name}"))),
            Some(Ok(v)) => Ok(json!({
                "content": [{"type": "text", "text": v.to_string()}],
                "structuredContent": v,
                "isError": false,
            })),
            Some(Err(e)) => Ok(json!({
                "content": [{"type": "text", "text": e}],
                "isError": true,
            })),
        }
    }
}

/// The batch mode: each non-blank line that does not start with `#` is a
/// tool call `{"tool": "click", "arguments": {...}}` (or `"name"` for
/// `"tool"`); each gets one line back, `{"tool": ..., "ok": true, "result":
/// ...}` or `{"tool": ..., "ok": false, "error": ...}`.
pub fn batch_line(session: &mut Session, line: Line) -> Option<String> {
    let line = match line {
        Line::Text(l) => l.trim(),
        Line::NotUtf8 => return Some(batch_failure("not UTF-8")),
        Line::TooLong => return Some(batch_failure("the line is too long")),
    };
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let failed =
        |tool: &Json, e: String| json!({"tool": tool, "ok": false, "error": e}).to_string();
    let call: Json = match serde_json::from_str(line) {
        Ok(c) => c,
        Err(e) => return Some(failed(&Json::Null, format!("not JSON: {e}"))),
    };
    let tool = call.get("tool").or_else(|| call.get("name"));
    let Some(name) = tool.and_then(Json::as_str) else {
        return Some(failed(&Json::Null, "a call needs `tool`".to_owned()));
    };
    let tool = json!(name);
    let args = match call.get("arguments") {
        None | Some(Json::Null) => Json::Object(Map::new()),
        Some(a @ Json::Object(_)) => a.clone(),
        Some(_) => return Some(failed(&tool, "`arguments` must be an object".to_owned())),
    };
    Some(match call_tool(session, name, &args) {
        None => failed(&tool, format!("unknown tool `{name}`")),
        Some(Ok(v)) => json!({"tool": tool, "ok": true, "result": v}).to_string(),
        Some(Err(e)) => failed(&tool, e),
    })
}

fn batch_failure(e: &str) -> String {
    json!({"tool": null, "ok": false, "error": e}).to_string()
}

/// Reads one line of at most `max` bytes into `buf`; a longer one is read
/// to its newline without being kept. `None` at the end of the input,
/// otherwise whether the line fitted.
fn read_line(
    input: &mut impl std::io::BufRead,
    buf: &mut Vec<u8>,
    max: usize,
) -> std::io::Result<Option<bool>> {
    buf.clear();
    let mut too_long = false;
    let mut any = false;
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            return Ok(any.then_some(!too_long));
        }
        any = true;
        let (n, done) = match available.iter().position(|&b| b == b'\n') {
            Some(i) => (i + 1, true),
            None => (available.len(), false),
        };
        if !too_long {
            if buf.len() + n > max {
                too_long = true;
                buf.clear();
            } else {
                buf.extend_from_slice(&available[..n]);
            }
        }
        input.consume(n);
        if done {
            return Ok(Some(!too_long));
        }
    }
}

/// Reads lines from `input` (each at most [`MAX_LINE`] bytes) and writes
/// the replies of `handle` to `out`, one per line, flushing after each.
pub fn serve(
    input: impl std::io::BufRead,
    out: impl std::io::Write,
    handle: impl FnMut(Line) -> Option<String>,
) -> std::io::Result<()> {
    serve_bounded(input, out, MAX_LINE, handle)
}

/// [`serve`] with a line limit of `max` bytes.
pub fn serve_bounded(
    mut input: impl std::io::BufRead,
    mut out: impl std::io::Write,
    max: usize,
    mut handle: impl FnMut(Line) -> Option<String>,
) -> std::io::Result<()> {
    let mut buf = Vec::new();
    while let Some(fits) = read_line(&mut input, &mut buf, max)? {
        let line = if !fits {
            Line::TooLong
        } else {
            match std::str::from_utf8(&buf) {
                Ok(l) => Line::Text(l),
                Err(_) => Line::NotUtf8,
            }
        };
        if let Some(reply) = handle(line) {
            writeln!(out, "{reply}")?;
            out.flush()?;
        }
    }
    Ok(())
}
