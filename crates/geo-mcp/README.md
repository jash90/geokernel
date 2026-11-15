# geo-mcp

`geokernel-mcp` is a Model Context Protocol server that drives the geokernel
window without a window. An agent clicks, drags, touches and types through
the app and reads its exact state back. Every user action is turned into egui
events and run through the window's own frame loop (`geo_app::headless`), on
a clock the caller controls. The click-or-drag slop, the click time limit and
a release and a press in one frame therefore behave as they do on screen.

Build it with `cargo build --release -p geo-mcp`. The binary is
`target/release/geokernel-mcp`.

## Modes

**MCP server** (no arguments): JSON-RPC 2.0 over stdio, one message per line.
Stdout carries only protocol messages, and diagnostics go to stderr. Both
eras of the protocol are served:

- modern, revision `2026-07-28`: each request carries
  `io.modelcontextprotocol/protocolVersion` and
  `io.modelcontextprotocol/clientCapabilities` in `params._meta`, and
  `server/discover` returns the supported versions;
- legacy, revisions `2025-11-25`, `2025-06-18`, `2025-03-26` and
  `2024-11-05`: an `initialize` handshake comes first.

A tool that fails returns a result with `isError: true`. Malformed lines,
unknown methods and unknown tools get JSON-RPC errors. Nothing panics the
server: a panic inside the window is reported as a failed call, and the
window is replaced by a new one.

**Batch mode**: `geokernel-mcp --batch calls.jsonl`, or `--batch` alone to
read from stdin. Each line holds one call, `{"tool": "click", "arguments":
{...}}`, and gets one result line back, `{"tool": ..., "ok": true, "result":
...}` or `{"tool": ..., "ok": false, "error": ...}`. Blank lines and lines
that start with `#` are skipped. The same calls give byte-identical output on
every run.

## Client configuration

Add this entry to `.mcp.json`, with the command pointing at the built
binary:

```json
{
  "mcpServers": {
    "geokernel": {
      "command": "target/release/geokernel-mcp",
      "args": []
    }
  }
}
```

## Tools

| Tool | What it does |
| --- | --- |
| `reset` | Opens a new window: `width` and `height` in points (default 1400 x 900), `profile` `desktop` or `touch`, an optional `script`. |
| `open` | Opens a document the way a dropped file does: `path`, inline `text`, or `base64` bytes (a `.ggb`). `name` gives the format. |
| `state` | Returns objects in construction order (name, type, coords, value, definition, defined, visible, free), the tool and its picks, the camera and view, undo and redo, the selection, the input bar, notices, errors, the keyboard, and the window size and profile. `verbose` adds the algebra rows, the regions, the pointer automaton, hover and decoration. |
| `widgets` | Returns the named rectangles of the last frame: tool groups and drop-down entries, undo, redo, menu, algebra rows (`row:A`, `marker:A`, `row_menu:A`, `value:r`), `input_bar`, view and style buttons, keys, the hint card, notices, menu entries and dialog controls. |
| `input` | Runs frames, each with its `dt` (default 1/60 s) and `events` applied in order within the frame. Event types: `move`, `down`, `up`, `gone`, `wheel`, `zoom`, `touch`, `key`, `text`, `modifiers`, `focus`. Returns what changed in each frame. |
| `click` | Moves the pointer to a position, presses and releases, one frame each, then settles. |
| `drag` | Presses at `from`, moves to `to` in `steps` frames over `duration` seconds, then releases. |
| `type_line` | Clicks the input bar, types the text and presses Enter. |
| `select_tool` | Clicks the tool's toolbar group and, when the drop-down opens, its entry. |
| `undo`, `redo` | Clicks the toolbar button. |
| `resize` | Changes the window size. |
| `set_view` | Sets the camera: the world centre `cx`, `cy` and the `scale` in points per unit. |
| `screenshot` | Renders the last frame to a PNG at `path`. Without a GPU adapter it fails with a clear message. |
| `frame_stats` | Returns the deterministic counters of the last frame (shapes, glyphs, meshes, vertices, indices, passes, rows built, rebuilds, repaint). |

A position can be given three ways:

- `x` and `y` in window points from the top left;
- `wx` and `wy` in world coordinates, through the view as last painted;
- `widget`, an id or a label from `widgets`, which gives its centre.

Optional `dx` and `dy` move any of these by that many points. Touch events go
through egui-winit's touch-to-pointer emulation unless `raw` is set, so a
touch window behaves as it does on a phone.

## Example session

Five calls in batch mode:

```text
{"tool": "reset", "arguments": {"script": "A = (1, 1)\nB = (4, 2)\n"}}
{"tool": "select_tool", "arguments": {"tool": "segment"}}
{"tool": "click", "arguments": {"wx": 1, "wy": 1}}
{"tool": "click", "arguments": {"wx": 4, "wy": 2}}
{"tool": "state"}
```

The results, shortened:

```text
{"ok":true,"result":{"objects":[{"name":"A",...},{"name":"B",...}],"tool":{"id":"move",...},...},"tool":"reset"}
{"ok":true,"result":{"changes":{"tool":"segment"},...},"tool":"select_tool"}
{"ok":true,"result":{"changes":{"picks":["A"]},...},"tool":"click"}
{"ok":true,"result":{"changes":{"added":["f"],"picks":[],"selected":"f",...},...},"tool":"click"}
{"ok":true,"result":{"objects":[...,{"name":"f","type":"segment","definition":"Segment(A, B)",...}],...},"tool":"state"}
```

The same session over MCP:

1. `initialize`, or modern requests with `_meta`.
2. `tools/call` with `{"name": "reset", "arguments": {...}}`, then the
   other calls in the same way.

Each result carries the tool's JSON both as `structuredContent` and as text.
