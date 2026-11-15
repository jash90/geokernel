# geo-mcp

`geokernel-mcp` is a Model Context Protocol server that drives the geokernel
window without a window. An agent clicks, drags, touches and types through
the app and reads its exact state back. Every user action is turned into egui
events and run through the window's own frame loop (`geo_app::headless`), on
a clock the caller controls. The click-or-drag slop, the click time limit and
a release and a press in one frame therefore behave as they do on screen.

The server feeds the window what egui-winit feeds it: events, the clock, the
window size in points, the pixels per point, focus, the viewport info (never
full screen) and dropped files. Three tools set state directly instead:
`reset` (a new window), `resize` (a new size for the next frames) and
`set_view` (the camera). Viewport commands the window issues, such as full
screen, are reported in `viewport_commands` and not carried out.

The window code is shared through the `headless` feature of `geo-app`, which
is off by default. `cargo build -p geo-app` alone does not compile it. Cargo
unifies features within one build, so a workspace build that includes
`geo-mcp` compiles the module into `geo-app` too. In either case the desktop
window never calls it, so it does no work at run time.

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
- legacy, revisions `2025-11-25`, `2025-06-18` and `2024-11-05`: an
  `initialize` handshake comes first. `2025-03-26` is not offered because it
  requires accepting JSON-RPC batches.

`tools/list` in the modern revision carries `ttlMs` and `cacheScope`, which
that revision requires. The tool list never changes while the server runs.

A tool that fails returns a result with `isError: true`. Malformed lines,
unknown methods and unknown tools get JSON-RPC errors. Nothing panics the
server. A panic inside the window is reported as a failed call with the
panic's message, and the window is replaced by a fresh default one: 1400 x
900 points, desktop profile, 1 pixel per point, empty document.

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

## Trust

The server is a local tool for whoever runs it. It reads and writes the
paths the caller names: `open` reads a file, and `screenshot` writes a PNG.
An `open` error may quote a line of the file the caller pointed at.
`screenshot` writes only paths that end in `.png`, are in an existing
directory outside `/dev`, and are regular files if they already exist.
`open` reads only regular files: devices, FIFOs and directories are refused.

## Limits

All limits are set in `src/limits.rs`. A request beyond a limit is refused
with an error.

| Limit | Value |
| --- | --- |
| Request line | 16 MiB. A longer line gets an error and is skipped up to its newline. |
| Document for `reset`, `open` (inline, base64 or by path) | 8 MiB |
| Frames per call (`input` frames, `drag` steps) | 10,000 |
| Events per `input` frame | 1,000, checked before any frame runs |
| Text of a `text` event or `type_line` | 64 KiB |
| Coordinates | 1e6 |
| Delay between frames | 3,600 s |
| Settle frames after an action | 120 |

Texture changes are kept as the current images only, so memory does not grow
with the number of frames between screenshots.

## Tools

| Tool | What it does |
| --- | --- |
| `reset` | Opens a new window: `width` and `height` in points (default 1400 x 900), `scale` in pixels per point (default 1, which keeps geometry deterministic; a Retina display is 2), `profile` `desktop` or `touch`, an optional `script`. |
| `open` | Drops a file on the window, which reads it itself: `path`, inline `text`, or `base64` bytes (a `.ggb`). Inline documents are written to a scratch file named `name` first; the extension gives the format. |
| `state` | Returns objects in construction order (name, type, coords, value as the algebra view shows it, `raw_value` where that differs, definition, defined, visible, free, `label_offset` in pixels; a function's coords are its values at x = -2..2), the tool and its picks, the camera and view, undo and redo with their depths, the selection, the input bar, notices, errors, the keyboard, and the window size, scale, focus and profile. `verbose` adds the algebra rows as painted, the regions, the pointer automaton, hover and decoration (with `snap_step`, the grid step new and dragged free points are captured to, or null while capturing is off). Reading changes nothing. |
| `widgets` | Returns the named rectangles of the last frame, in points: tool groups and drop-down entries, undo, redo, menu, algebra rows (`row:A`, `marker:A`, `row_menu:A`, `value:r`; `in_view: false` marks a row scrolled out of the list), `input_bar`, view and style buttons, keys, the hint card, notices, menu entries and dialog controls. |
| `input` | Runs frames, each with its `dt` (default 1/60 s) and `events` applied in order within the frame. Event types: `move`, `down`, `up`, `gone`, `wheel`, `zoom`, `touch`, `key`, `text`, `modifiers`, `focus`. Returns what changed in each frame, with the viewport commands it issued. |
| `click` | Clicks a position, then settles. A mouse moves there in a frame of its own, then presses and releases, one frame each. A finger touches down and lifts, one frame each, and the pointer is gone after the lift. `pointer` (`mouse` or `touch`) defaults to the window's profile. |
| `drag` | Presses or touches down at `from`, moves to `to` in `steps` frames over `duration` seconds, then releases. `pointer` as for `click`. |
| `type_line` | Clicks the input bar with the window's pointer, types the text and presses Enter. |
| `select_tool` | Clicks the tool's toolbar group and, when the drop-down opens, its entry, with the window's pointer. |
| `undo`, `redo` | Clicks the toolbar button with the window's pointer. |
| `resize` | Changes the window size. |
| `set_view` | Sets the camera: the world centre `cx`, `cy` and the `scale` in points per unit. |
| `screenshot` | Renders the last frame to a PNG at `path`. Without a GPU adapter it fails with a clear message. |
| `frame_stats` | Returns the deterministic counters of the last frame (shapes, glyphs, meshes, vertices, indices, passes, rows built, rebuilds, repaint). Times, allocation counts and new galleys are left out because they vary between runs. |

Actions settle afterwards: they run empty frames, 1/60 s apart, while the
window asks for a frame at once. They stop after 120 frames. `settled:
false` says the cap was hit, and `settle_frames` says how many ran. A repaint
the window asks for later, such as a timer, is not followed. To advance the
clock past one, send `input` with a frame whose `dt` covers the delay.

A position can be given three ways:

- `x` and `y` in window points from the top left;
- `wx` and `wy` in world coordinates, through the view as last painted;
- `widget`, an id or a label from `widgets`, which gives its centre.

Optional `dx` and `dy` move any of these by that many points. In `input`,
the `modifiers` of a `down`, `up` or `wheel` event go on that event only; a
`modifiers` event changes the held keys (a `key` event and `click` do both).
A dropped file is the `open` tool. Touch events go
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
