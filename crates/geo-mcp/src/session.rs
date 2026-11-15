//! One headless window and the tools that drive it. Every tool that stands
//! for a user action turns it into what a platform layer sends (egui
//! events, touches through egui-winit's emulation, a dropped file) and runs
//! it frame by frame through the window's own frame loop. Three tools set
//! state directly instead: `reset` (a new window), `resize` (a new window
//! size for the next frames) and `set_view` (the camera).

use crate::limits::{
    MAX_COORD, MAX_DOCUMENT, MAX_DT, MAX_EVENTS, MAX_FRAMES, MAX_TEXT, SETTLE_MAX,
};
use crate::report::{self, Snapshot};
use crate::screenshot::Renderer;
use geo_app::headless::egui::{
    self, vec2, Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, TouchPhase,
};
use geo_app::headless::{self, Headless, Profile, WidgetKind, DT, SCALES};
use geo_app::Workbench;
use geo_app_core::app::OwnerRules;
use geo_app_core::view::Camera;
use serde_json::{json, Map, Value as Json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The window size a session starts with.
pub const DEFAULT_SIZE: (f32, f32) = (1400.0, 900.0);
/// The finger the conveniences touch with.
const FINGER: u64 = 0;

/// Why a tool call failed.
#[derive(Debug, PartialEq)]
pub enum CallError {
    /// No tool of that name.
    UnknownTool(String),
    /// The tool ran into a problem, told in the message.
    Failed(String),
}

type Res = Result<Json, String>;

/// What a click or a drag is made with.
#[derive(Clone, Copy, PartialEq)]
enum Pointer {
    Mouse,
    Touch,
}

/// The server's window.
pub struct Session {
    h: Headless,
    renderer: Option<Renderer>,
    /// The viewport commands of the frames run by the current call.
    commands: Vec<String>,
    panic_next: bool,
}

impl Default for Session {
    fn default() -> Self {
        Session::new()
    }
}

fn num(args: &Json, key: &str) -> Result<Option<f64>, String> {
    match args.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(v) => match v.as_f64() {
            Some(x) if x.is_finite() => Ok(Some(x)),
            _ => Err(format!("`{key}` must be a finite number")),
        },
    }
}

fn coord(args: &Json, key: &str) -> Result<Option<f64>, String> {
    let v = num(args, key)?;
    if v.is_some_and(|v| v.abs() > MAX_COORD) {
        return Err(format!("`{key}` is out of range"));
    }
    Ok(v)
}

fn text<'a>(args: &'a Json, key: &str) -> Result<Option<&'a str>, String> {
    match args.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::String(s)) => Ok(Some(s)),
        Some(_) => Err(format!("`{key}` must be a string")),
    }
}

/// A text of at most [`MAX_TEXT`] bytes.
fn short_text<'a>(args: &'a Json, key: &str) -> Result<Option<&'a str>, String> {
    let t = text(args, key)?;
    if t.is_some_and(|t| t.len() > MAX_TEXT) {
        return Err(format!("`{key}` is longer than {MAX_TEXT} bytes"));
    }
    Ok(t)
}

fn flag(args: &Json, key: &str) -> Result<Option<bool>, String> {
    match args.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(format!("`{key}` must be true or false")),
    }
}

fn count(args: &Json, key: &str, min: u64, max: u64) -> Result<Option<u64>, String> {
    match args.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(v) => match v.as_u64() {
            Some(n) if (min..=max).contains(&n) => Ok(Some(n)),
            _ => Err(format!(
                "`{key}` must be a whole number from {min} to {max}"
            )),
        },
    }
}

fn modifiers(args: &Json) -> Result<Modifiers, String> {
    let mut m = Modifiers::NONE;
    let Some(list) = args.get("modifiers") else {
        return Ok(m);
    };
    let list = list
        .as_array()
        .ok_or("`modifiers` must be a list of names")?;
    for name in list {
        match name.as_str() {
            Some("shift") => m.shift = true,
            Some("alt") => m.alt = true,
            Some("ctrl") => m.ctrl = true,
            Some("mac_cmd") => m.mac_cmd = true,
            // The platform's command key (Cmd on a Mac, Ctrl elsewhere),
            // what the window's shortcuts ask for.
            Some("command") => m.command = true,
            _ => return Err(format!("unknown modifier {name}")),
        }
    }
    Ok(m)
}

fn button(args: &Json) -> Result<PointerButton, String> {
    match text(args, "button")? {
        None | Some("primary") => Ok(PointerButton::Primary),
        Some("secondary") => Ok(PointerButton::Secondary),
        Some("middle") => Ok(PointerButton::Middle),
        Some(b) => Err(format!("unknown button `{b}`")),
    }
}

fn dt(args: &Json, default: f64) -> Result<f64, String> {
    let dt = num(args, "dt")?.unwrap_or(default);
    if !(0.0..=MAX_DT).contains(&dt) {
        return Err(format!("`dt` must be from 0 to {MAX_DT} seconds"));
    }
    Ok(dt)
}

fn object(args: &Json) -> Result<&Map<String, Json>, String> {
    match args {
        Json::Object(m) => Ok(m),
        Json::Null => Err("arguments are missing".to_owned()),
        _ => Err("arguments must be an object".to_owned()),
    }
}

fn window_size(args: &Json, default: (f32, f32)) -> Result<(f32, f32), String> {
    let width = coord(args, "width")?.unwrap_or(default.0 as f64);
    let height = coord(args, "height")?.unwrap_or(default.1 as f64);
    let side = 100.0..=headless::MAX_SIDE as f64;
    if !side.contains(&width) || !side.contains(&height) {
        return Err(format!(
            "the window must be 100 to {} points on each side",
            headless::MAX_SIDE
        ));
    }
    Ok((width as f32, height as f32))
}

/// A path the caller names for reading: an existing regular file of at
/// most [`MAX_DOCUMENT`] bytes (not a device, a FIFO or a directory).
fn readable_file(path: &str) -> Result<PathBuf, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{path}: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{path}: not a regular file"));
    }
    if meta.len() > MAX_DOCUMENT {
        return Err(format!("{path}: larger than {MAX_DOCUMENT} bytes"));
    }
    Ok(PathBuf::from(path))
}

/// A path the caller names for writing a PNG: ending in `.png`, outside
/// `/dev`, in an existing directory, and a regular file if it exists.
fn png_path(path: &str) -> Result<PathBuf, String> {
    let p = Path::new(path);
    let is_png = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if !is_png || p.file_name().is_none() {
        return Err(format!("{path}: a screenshot path must end in .png"));
    }
    if p.starts_with("/dev") {
        return Err(format!("{path}: not a file path"));
    }
    let parent = match p.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    let dir = parent
        .canonicalize()
        .map_err(|e| format!("{}: {e}", parent.display()))?;
    if dir.starts_with("/dev") || !dir.is_dir() {
        return Err(format!("{path}: not in a directory"));
    }
    match std::fs::symlink_metadata(p) {
        Ok(m) if !m.is_file() => Err(format!("{path}: exists and is not a regular file")),
        _ => Ok(p.to_path_buf()),
    }
}

/// A plain file name for an inline document (its extension tells the
/// format).
fn file_name(name: &str) -> Result<&str, String> {
    let plain = !name.is_empty()
        && name.len() <= 255
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', '\0']);
    if plain {
        Ok(name)
    } else {
        Err(format!("`name` must be a plain file name, not `{name}`"))
    }
}

/// A directory of its own for one inline document, removed after use.
fn scratch_dir() -> Result<PathBuf, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("geokernel-mcp-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot stage the document: {e}"))?;
    Ok(dir)
}

impl Session {
    /// A desktop window of the default size at one pixel per point with an
    /// empty document, after its first frames.
    pub fn new() -> Session {
        let mut s = Session {
            h: Headless::new(
                Workbench::default(),
                None,
                Profile::Desktop,
                DEFAULT_SIZE,
                1.0,
            ),
            renderer: None,
            commands: Vec::new(),
            panic_next: false,
        };
        s.settle();
        s
    }

    /// The window (for tests).
    pub fn headless(&self) -> &Headless {
        &self.h
    }

    /// Makes the next tool call panic inside the window, to test the
    /// recovery from a panic.
    #[doc(hidden)]
    pub fn panic_on_next_call(&mut self) {
        self.panic_next = true;
    }

    /// Runs the tool `name` with `args`.
    pub fn call(&mut self, name: &str, args: &Json) -> Result<Json, CallError> {
        let empty = Json::Object(Map::new());
        let args = if args.is_null() { &empty } else { args };
        if std::mem::take(&mut self.panic_next) {
            panic!("a panic asked for by a test");
        }
        self.commands.clear();
        let result = match name {
            "reset" => self.reset(args),
            "open" => self.open(args),
            "state" => self.state(args),
            "widgets" => self.widgets(args),
            "input" => self.input(args),
            "click" => self.click(args),
            "drag" => self.drag(args),
            "type_line" => self.type_line(args),
            "select_tool" => self.select_tool(args),
            "undo" => self.press_toolbar("undo"),
            "redo" => self.press_toolbar("redo"),
            "resize" => self.resize(args),
            "set_view" => self.set_view(args),
            "screenshot" => self.screenshot(args),
            "frame_stats" => Ok(report::frame_stats(&self.h)),
            _ => return Err(CallError::UnknownTool(name.to_owned())),
        };
        result.map_err(CallError::Failed)
    }

    /// One frame; keeps its viewport commands.
    fn run(&mut self, dt: f64, events: Vec<Event>) {
        self.h.frame(dt, events);
        self.keep_commands();
    }

    fn keep_commands(&mut self) {
        let new = self.h.viewport_commands().iter().map(|c| format!("{c:?}"));
        self.commands.extend(new);
    }

    /// Empty frames while the window animates, at most [`SETTLE_MAX`].
    /// Returns how many ran and whether the window stopped animating.
    fn settle(&mut self) -> (usize, bool) {
        let mut n = 0;
        while n < SETTLE_MAX && self.h.animating() {
            self.run(DT, Vec::new());
            n += 1;
        }
        (n, !self.h.animating())
    }

    /// Runs `frames` (each `(dt, events)`) and settles; returns what
    /// changed over the whole action.
    fn act(&mut self, frames: Vec<(f64, Vec<Event>)>) -> Json {
        let before = Snapshot::take(&self.h);
        for (dt, events) in frames {
            self.run(dt, events);
        }
        let settled = self.settle();
        self.summary(&before, settled)
    }

    fn summary(&self, before: &Snapshot, (n, settled): (usize, bool)) -> Json {
        let changes = before.changes(&Snapshot::take(&self.h));
        let mut out = json!({
            "time": self.h.time(),
            "frames": self.h.frames(),
            "changes": changes,
            "settle_frames": n,
            "settled": settled,
        });
        if !self.commands.is_empty() {
            out["viewport_commands"] = json!(self.commands);
        }
        out
    }

    fn reset(&mut self, args: &Json) -> Res {
        object(args)?;
        let size = window_size(args, DEFAULT_SIZE)?;
        let scale = num(args, "scale")?.unwrap_or(1.0);
        if !SCALES.contains(&(scale as f32)) {
            return Err(format!(
                "`scale` (pixels per point) must be from {} to {}",
                SCALES.start(),
                SCALES.end()
            ));
        }
        let profile = match text(args, "profile")? {
            None | Some("desktop") => Profile::Desktop,
            Some("touch") => Profile::Touch,
            Some(p) => return Err(format!("unknown profile `{p}` (desktop or touch)")),
        };
        let rules = match text(args, "rules")? {
            None | Some("ours") => OwnerRules::OURS,
            Some("reference") => OwnerRules::REFERENCE,
            Some(r) => return Err(format!("unknown rules `{r}` (ours or reference)")),
        };
        let wb = match text(args, "script")? {
            Some(src) if src.len() as u64 > MAX_DOCUMENT => {
                return Err(format!("`script` is larger than {MAX_DOCUMENT} bytes"))
            }
            Some(src) => Workbench::from_script(src)?,
            None => Workbench::default(),
        };
        self.h = Headless::new(wb, None, profile, size, scale as f32);
        self.h.app.rules = rules;
        // The renderer's textures belong to the old context.
        self.renderer = None;
        self.settle();
        Ok(report::state(&self.h, false))
    }

    /// Drops a file on the window, as a platform layer reports a drop: the
    /// window reads the file itself. Inline documents are written to a
    /// scratch file of their own first.
    fn open(&mut self, args: &Json) -> Res {
        object(args)?;
        let name = text(args, "name")?;
        let (path, scratch) = match (
            text(args, "path")?,
            text(args, "text")?,
            text(args, "base64")?,
        ) {
            (Some(path), None, None) => (readable_file(path)?, None),
            (None, Some(t), None) => {
                if t.len() as u64 > MAX_DOCUMENT {
                    return Err(format!("`text` is larger than {MAX_DOCUMENT} bytes"));
                }
                let name = file_name(name.unwrap_or("inline.geo"))?;
                let dir = scratch_dir()?;
                (dir.join(name), Some((dir, t.as_bytes().to_vec())))
            }
            (None, None, Some(b)) => {
                if b.len() as u64 > MAX_DOCUMENT / 3 * 4 + 4 {
                    return Err(format!("`base64` holds more than {MAX_DOCUMENT} bytes"));
                }
                let bytes = crate::base64::decode(b)?;
                let name = file_name(name.unwrap_or("inline.ggb"))?;
                let dir = scratch_dir()?;
                (dir.join(name), Some((dir, bytes)))
            }
            _ => return Err("give exactly one of `path`, `text` or `base64`".to_owned()),
        };
        if let Some((_, bytes)) = &scratch {
            std::fs::write(&path, bytes).map_err(|e| format!("cannot stage the document: {e}"))?;
        }
        self.h.drop_files(DT, vec![path]);
        self.keep_commands();
        self.settle();
        if let Some((dir, _)) = scratch {
            let _ = std::fs::remove_dir_all(dir);
        }
        match &self.h.app.load_error {
            Some(e) => Err(e.clone()),
            None => Ok(report::state(&self.h, false)),
        }
    }

    fn state(&mut self, args: &Json) -> Res {
        object(args)?;
        let verbose = flag(args, "verbose")?.unwrap_or(false);
        Ok(report::state(&self.h, verbose))
    }

    fn widgets(&mut self, args: &Json) -> Res {
        object(args)?;
        let kind = text(args, "kind")?;
        let list: Vec<Json> = report::named_widgets(&self.h)
            .iter()
            .map(|(id, w)| report::widget_json(id, w))
            .filter(|w| kind.is_none_or(|k| w["kind"] == k))
            .collect();
        Ok(json!({ "widgets": list }))
    }

    /// The widget named `name` (an id from `widgets`, or a label).
    fn widget(&self, name: &str) -> Result<(String, headless::Widget), String> {
        let all = report::named_widgets(&self.h);
        all.iter()
            .find(|(id, _)| id == name)
            .or_else(|| all.iter().find(|(_, w)| w.label == name))
            .cloned()
            .ok_or_else(|| format!("no widget `{name}` on screen (see the `widgets` tool)"))
    }

    /// A position in window points: `x`/`y` (window points), `wx`/`wy`
    /// (world coordinates, through the view as last painted) or `widget`
    /// (the centre of a widget), each optionally moved by `dx`/`dy` points.
    fn position(&self, args: &Json) -> Result<Pos2, String> {
        let base = if let Some(name) = text(args, "widget")? {
            self.widget(name)?.1.rect.center()
        } else if let (Some(x), Some(y)) = (coord(args, "wx")?, coord(args, "wy")?) {
            let p = self
                .h
                .world_to_screen(x, y)
                .ok_or("the drawing has not been painted yet")?;
            if !(p.x.is_finite() && p.y.is_finite())
                || p.x.abs() as f64 > MAX_COORD
                || p.y.abs() as f64 > MAX_COORD
            {
                return Err("the world point is too far off screen".to_owned());
            }
            p
        } else if let (Some(x), Some(y)) = (coord(args, "x")?, coord(args, "y")?) {
            Pos2::new(x as f32, y as f32)
        } else {
            return Err("a position needs `x` and `y`, `wx` and `wy`, or `widget`".to_owned());
        };
        let dx = coord(args, "dx")?.unwrap_or(0.0) as f32;
        let dy = coord(args, "dy")?.unwrap_or(0.0) as f32;
        Ok(base + vec2(dx, dy))
    }

    /// The pointer a click or a drag uses: `pointer`, by default what the
    /// window's profile is made for.
    fn pointer(&self, args: &Json) -> Result<Pointer, String> {
        match text(args, "pointer")? {
            None => Ok(match self.h.profile() {
                Profile::Desktop => Pointer::Mouse,
                Profile::Touch => Pointer::Touch,
            }),
            Some("mouse") => Ok(Pointer::Mouse),
            Some("touch") => Ok(Pointer::Touch),
            Some(p) => Err(format!("unknown pointer `{p}` (mouse or touch)")),
        }
    }

    /// The egui events of one event description.
    fn event(&mut self, e: &Json, out: &mut Vec<Event>) -> Result<(), String> {
        object(e)?;
        let kind = text(e, "type")?.ok_or("an event needs a `type`")?;
        match kind {
            "move" => out.push(Event::PointerMoved(self.position(e)?)),
            "down" | "up" => out.push(Event::PointerButton {
                pos: self.position(e)?,
                button: button(e)?,
                pressed: kind == "down",
                modifiers: modifiers(e)?,
            }),
            "gone" => out.push(Event::PointerGone),
            "wheel" => {
                let unit = match text(e, "unit")? {
                    None | Some("point") => MouseWheelUnit::Point,
                    Some("line") => MouseWheelUnit::Line,
                    Some("page") => MouseWheelUnit::Page,
                    Some(u) => return Err(format!("unknown wheel unit `{u}`")),
                };
                let dx = coord(e, "dx")?.unwrap_or(0.0) as f32;
                let dy = coord(e, "dy")?.unwrap_or(0.0) as f32;
                out.push(Event::MouseWheel {
                    unit,
                    delta: vec2(dx, dy),
                    phase: TouchPhase::Move,
                    source: egui::MouseWheelSource::Wheel,
                    modifiers: modifiers(e)?,
                });
            }
            "zoom" => {
                let f = num(e, "factor")?.ok_or("`zoom` needs a `factor`")?;
                if !(0.01..=100.0).contains(&f) {
                    return Err("`factor` must be from 0.01 to 100".to_owned());
                }
                out.push(Event::Zoom(f as f32));
            }
            "touch" => {
                let phase = match text(e, "phase")? {
                    Some("start") => TouchPhase::Start,
                    Some("move") => TouchPhase::Move,
                    Some("end") => TouchPhase::End,
                    Some("cancel") => TouchPhase::Cancel,
                    _ => {
                        return Err("`touch` needs a `phase`: start, move, end or cancel".to_owned())
                    }
                };
                let id = count(e, "id", 0, 1 << 32)?.ok_or("`touch` needs a finger `id`")?;
                let pos = self.position(e)?;
                if flag(e, "raw")?.unwrap_or(false) {
                    out.push(Event::Touch {
                        device_id: egui::TouchDeviceId(1),
                        id: egui::TouchId(id),
                        phase,
                        pos,
                        force: None,
                    });
                } else {
                    out.extend(self.h.touch(id, phase, pos));
                }
            }
            "key" => {
                let name = text(e, "key")?.ok_or("`key` needs a `key` name")?;
                let key = Key::from_name(name).ok_or_else(|| format!("unknown key `{name}`"))?;
                let m = modifiers(e)?;
                let (down, up) = match text(e, "action")? {
                    None | Some("tap") => (true, true),
                    Some("press") => (true, false),
                    Some("release") => (false, true),
                    Some(a) => return Err(format!("unknown key action `{a}`")),
                };
                if m != Modifiers::NONE {
                    out.push(Event::ModifiersChanged(m));
                }
                if down {
                    out.push(headless::key(key, true, m));
                }
                if up {
                    out.push(headless::key(key, false, m));
                }
                if m != Modifiers::NONE && up {
                    out.push(Event::ModifiersChanged(Modifiers::NONE));
                }
            }
            "modifiers" => out.push(Event::ModifiersChanged(modifiers(e)?)),
            "text" => out.push(Event::Text(
                short_text(e, "text")?
                    .ok_or("`text` needs a `text`")?
                    .to_owned(),
            )),
            // egui-winit reports the change as an event and keeps the
            // focus as input state (`RawInput::focused`); the window does
            // both.
            "focus" => out.push(Event::WindowFocused(
                flag(e, "focused")?.ok_or("`focus` needs `focused`")?,
            )),
            _ => return Err(format!("unknown event type `{kind}`")),
        }
        Ok(())
    }

    fn input(&mut self, args: &Json) -> Res {
        object(args)?;
        let frames = args
            .get("frames")
            .and_then(Json::as_array)
            .ok_or("`frames` must be a list of frames")?;
        if frames.len() > MAX_FRAMES {
            return Err(format!("at most {MAX_FRAMES} frames per call"));
        }
        // The sizes are checked before any frame runs.
        for (i, f) in frames.iter().enumerate() {
            object(f).map_err(|e| format!("frame {i}: {e}"))?;
            dt(f, DT).map_err(|e| format!("frame {i}: {e}"))?;
            match f.get("events") {
                None | Some(Json::Null) => {}
                Some(Json::Array(a)) if a.len() <= MAX_EVENTS => {}
                Some(Json::Array(_)) => {
                    return Err(format!("frame {i}: at most {MAX_EVENTS} events per frame"))
                }
                Some(_) => return Err(format!("frame {i}: `events` must be a list")),
            }
        }
        let settle = flag(args, "settle")?.unwrap_or(false);
        let with_state = flag(args, "state")?.unwrap_or(false);
        let mut report = Vec::with_capacity(frames.len());
        for (i, f) in frames.iter().enumerate() {
            let frame_dt = dt(f, DT)?;
            let list = f
                .get("events")
                .and_then(Json::as_array)
                .map_or(&[][..], |a| &a[..]);
            // A frame that fails to parse runs nothing and leaves the touch
            // emulation as it was; the frames before it have run.
            let touch = self.h.touch_state();
            let mut events = Vec::new();
            for e in list {
                if let Err(err) = self.event(e, &mut events) {
                    self.h.set_touch_state(touch);
                    return Err(format!("frame {i}: {err} (frames before it ran)"));
                }
            }
            let before = Snapshot::take(&self.h);
            self.run(frame_dt, events);
            let mut entry = before.changes(&Snapshot::take(&self.h));
            entry.insert("frame".into(), json!(i));
            entry.insert("t".into(), json!(self.h.time()));
            let commands = self.h.viewport_commands();
            if !commands.is_empty() {
                let list: Vec<String> = commands.iter().map(|c| format!("{c:?}")).collect();
                entry.insert("viewport_commands".into(), json!(list));
            }
            report.push(Json::Object(entry));
        }
        let (settle_frames, _) = if settle { self.settle() } else { (0, false) };
        let mut out = json!({
            "frames": report,
            "settle_frames": settle_frames,
            // Whether the window is still animating (wants a frame at once).
            "settled": !self.h.animating(),
            "time": self.h.time(),
        });
        if with_state {
            out["state"] = report::state(&self.h, false);
        }
        Ok(out)
    }

    /// The frames of `clicks` clicks at `pos`: a mouse moves there in a
    /// frame of its own, then presses and releases, one frame each; a
    /// finger touches down and lifts (egui-winit's emulation adds the
    /// pointer events, and the pointer is gone after each lift).
    fn tap_frames(
        &mut self,
        pos: Pos2,
        pointer: Pointer,
        b: PointerButton,
        m: Modifiers,
        clicks: u64,
    ) -> Result<Vec<(f64, Vec<Event>)>, String> {
        let mut frames = Vec::new();
        let mut first = Vec::new();
        // Held modifiers are input state too: pressed before the first
        // event and let go after the last.
        if m != Modifiers::NONE {
            first.push(Event::ModifiersChanged(m));
        }
        match pointer {
            Pointer::Mouse => {
                first.push(Event::PointerMoved(pos));
                frames.push((DT, first));
                let btn = |pressed| Event::PointerButton {
                    pos,
                    button: b,
                    pressed,
                    modifiers: m,
                };
                for _ in 0..clicks {
                    frames.push((DT, vec![btn(true)]));
                    frames.push((DT, vec![btn(false)]));
                }
            }
            Pointer::Touch => {
                if b != PointerButton::Primary {
                    return Err("a finger has only the primary button".to_owned());
                }
                for i in 0..clicks {
                    let mut down = if i == 0 {
                        std::mem::take(&mut first)
                    } else {
                        Vec::new()
                    };
                    down.extend(self.h.touch(FINGER, TouchPhase::Start, pos));
                    frames.push((DT, down));
                    frames.push((DT, self.h.touch(FINGER, TouchPhase::End, pos)));
                }
            }
        }
        if m != Modifiers::NONE {
            frames.push((DT, vec![Event::ModifiersChanged(Modifiers::NONE)]));
        }
        Ok(frames)
    }

    fn click(&mut self, args: &Json) -> Res {
        object(args)?;
        let pos = self.position(args)?;
        let pointer = self.pointer(args)?;
        let b = button(args)?;
        let clicks = count(args, "count", 1, 3)?.unwrap_or(1);
        let m = modifiers(args)?;
        let frames = self.tap_frames(pos, pointer, b, m, clicks)?;
        Ok(self.act(frames))
    }

    fn drag(&mut self, args: &Json) -> Res {
        object(args)?;
        let from = self.position(args.get("from").ok_or("`drag` needs `from`")?)?;
        let to = self.position(args.get("to").ok_or("`drag` needs `to`")?)?;
        let pointer = self.pointer(args)?;
        let b = button(args)?;
        let steps = count(args, "steps", 1, 1000)?.unwrap_or(10) as usize;
        let duration = num(args, "duration")?.unwrap_or(steps as f64 * DT);
        if !(0.0..=MAX_DT).contains(&duration) {
            return Err(format!("`duration` must be from 0 to {MAX_DT} seconds"));
        }
        let step_dt = duration / steps as f64;
        let at = |i: usize| from + (to - from) * (i as f32 / steps as f32);
        let mut frames = Vec::with_capacity(steps + 3);
        match pointer {
            Pointer::Mouse => {
                let btn = |pos, pressed| Event::PointerButton {
                    pos,
                    button: b,
                    pressed,
                    modifiers: Modifiers::NONE,
                };
                frames.push((DT, vec![Event::PointerMoved(from)]));
                frames.push((DT, vec![btn(from, true)]));
                for i in 1..=steps {
                    frames.push((step_dt, vec![Event::PointerMoved(at(i))]));
                }
                frames.push((DT, vec![btn(to, false)]));
            }
            Pointer::Touch => {
                if b != PointerButton::Primary {
                    return Err("a finger has only the primary button".to_owned());
                }
                frames.push((DT, self.h.touch(FINGER, TouchPhase::Start, from)));
                for i in 1..=steps {
                    frames.push((step_dt, self.h.touch(FINGER, TouchPhase::Move, at(i))));
                }
                frames.push((DT, self.h.touch(FINGER, TouchPhase::End, to)));
            }
        }
        Ok(self.act(frames))
    }

    /// Clicks `pos` with the window's own pointer and settles.
    fn click_at(&mut self, pos: Pos2) -> Result<(), String> {
        let pointer = self.pointer(&Json::Null)?;
        let frames = self.tap_frames(pos, pointer, PointerButton::Primary, Modifiers::NONE, 1)?;
        for (dt, events) in frames {
            self.run(dt, events);
        }
        self.settle();
        Ok(())
    }

    fn type_line(&mut self, args: &Json) -> Res {
        object(args)?;
        let line = short_text(args, "text")?.ok_or("`type_line` needs `text`")?;
        let submit = flag(args, "submit")?.unwrap_or(true);
        let before = Snapshot::take(&self.h);
        let field = self.widget("input_bar")?.1.rect.center();
        self.click_at(field)?;
        self.run(DT, vec![Event::Text(line.to_owned())]);
        if submit {
            self.run(
                DT,
                vec![
                    headless::key(Key::Enter, true, Modifiers::NONE),
                    headless::key(Key::Enter, false, Modifiers::NONE),
                ],
            );
        }
        let settled = self.settle();
        Ok(self.summary(&before, settled))
    }

    fn select_tool(&mut self, args: &Json) -> Res {
        object(args)?;
        let id = text(args, "tool")?.ok_or("`select_tool` needs `tool`")?;
        let tool = report::tool_from_id(id).ok_or_else(|| {
            format!(
                "unknown tool `{id}`; tools: {}",
                report::tool_ids().join(", ")
            )
        })?;
        let before = Snapshot::take(&self.h);
        let group = report::named_widgets(&self.h)
            .into_iter()
            .find(|(_, w)| w.kind == WidgetKind::ToolGroup && w.group == Some(tool.group()))
            .ok_or("the tool group button is not on screen")?;
        self.click_at(group.1.rect.center())?;
        // A group of several tools opens its drop-down: pick the entry.
        if let Some((_, entry)) = report::named_widgets(&self.h)
            .into_iter()
            .find(|(_, w)| w.kind == WidgetKind::ToolEntry && w.tool == Some(tool))
        {
            self.click_at(entry.rect.center())?;
        }
        if self.h.app.tool.tool() != tool {
            return Err(format!("the click did not select `{id}`"));
        }
        Ok(self.summary(&before, (0, !self.h.animating())))
    }

    fn press_toolbar(&mut self, name: &str) -> Res {
        let before = Snapshot::take(&self.h);
        let (_, w) = self.widget(name)?;
        self.click_at(w.rect.center())?;
        let mut out = self.summary(&before, (0, !self.h.animating()));
        out["enabled"] = json!(w.enabled);
        Ok(out)
    }

    fn resize(&mut self, args: &Json) -> Res {
        object(args)?;
        let size = window_size(args, self.h.size())?;
        let before = Snapshot::take(&self.h);
        self.h.set_size(size);
        self.run(DT, Vec::new());
        let settled = self.settle();
        Ok(self.summary(&before, settled))
    }

    fn set_view(&mut self, args: &Json) -> Res {
        object(args)?;
        let old = self.h.app.camera.unwrap_or(Camera::STANDARD);
        let cx = coord(args, "cx")?.unwrap_or(old.cx);
        let cy = coord(args, "cy")?.unwrap_or(old.cy);
        let scale = num(args, "scale")?.unwrap_or(old.scale);
        if !(1.0e-6..=1.0e9).contains(&scale) {
            return Err("`scale` (points per unit) must be from 1e-6 to 1e9".to_owned());
        }
        let before = Snapshot::take(&self.h);
        self.h.app.camera = Some(Camera { cx, cy, scale });
        self.h.app.view_request = None;
        self.run(DT, Vec::new());
        let settled = self.settle();
        Ok(self.summary(&before, settled))
    }

    fn screenshot(&mut self, args: &Json) -> Res {
        object(args)?;
        let path = text(args, "path")?.ok_or("`screenshot` needs `path` (a .png file)")?;
        let path = png_path(path)?;
        if self.h.last_output().is_none() {
            return Err("no frame has run yet".to_owned());
        }
        let fresh = self.renderer.is_none();
        if fresh {
            self.renderer = Some(Renderer::new()?);
        }
        let mut textures = self.h.take_textures(fresh);
        let renderer = self.renderer.as_mut().expect("just made");
        let out = self.h.last_output().expect("checked");
        let result = renderer.render(self.h.ctx(), out, &mut textures, &path);
        textures.clear();
        match result {
            Ok((w, h)) => Ok(json!({"path": path.display().to_string(), "width": w, "height": h})),
            Err(e) => {
                // A renderer that missed texture changes starts over.
                self.renderer = None;
                Err(e)
            }
        }
    }
}
