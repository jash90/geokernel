//! One headless window and the tools that drive it. Every tool that stands
//! for a user action turns it into egui events, frame by frame, and runs
//! them through the window's own frame loop; only `reset`, `open` and
//! `set_view` set state directly (a new window, a dropped file, a camera).

use crate::report::{self, Snapshot};
use crate::screenshot::Renderer;
use geo_app::headless::egui::{
    self, vec2, Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, TouchPhase,
};
use geo_app::headless::{self, Headless, Profile, WidgetKind, DT};
use geo_app::Workbench;
use geo_app_core::view::Camera;
use serde_json::{json, Map, Value as Json};

/// The window size a session starts with.
pub const DEFAULT_SIZE: (f32, f32) = (1400.0, 900.0);
/// The most empty frames a settle runs.
pub const SETTLE_MAX: usize = 120;
/// The most frames one `input` call takes.
const MAX_FRAMES: usize = 10_000;
/// Coordinates beyond this many points are refused.
const MAX_COORD: f64 = 1.0e6;

/// Why a tool call failed.
#[derive(Debug, PartialEq)]
pub enum CallError {
    /// No tool of that name.
    UnknownTool(String),
    /// The tool ran into a problem, told in the message.
    Failed(String),
}

type Res = Result<Json, String>;

/// The server's window.
pub struct Session {
    h: Headless,
    renderer: Option<Renderer>,
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
    if !(0.0..=3600.0).contains(&dt) {
        return Err("`dt` must be from 0 to 3600 seconds".to_owned());
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

impl Session {
    /// A desktop window of the default size with an empty document, after
    /// its first frames.
    pub fn new() -> Session {
        let mut s = Session {
            h: Headless::new(Workbench::default(), None, Profile::Desktop, DEFAULT_SIZE),
            renderer: None,
        };
        s.settle();
        s
    }

    /// The window (for tests).
    pub fn headless(&self) -> &Headless {
        &self.h
    }

    /// Runs the tool `name` with `args`.
    pub fn call(&mut self, name: &str, args: &Json) -> Result<Json, CallError> {
        let empty = Json::Object(Map::new());
        let args = if args.is_null() { &empty } else { args };
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

    fn settle(&mut self) -> usize {
        self.h.settle(DT, SETTLE_MAX)
    }

    /// One frame, then empty frames while the window animates.
    fn frame_and_settle(&mut self) {
        self.h.frame(DT, Vec::new());
        self.settle();
    }

    /// Runs `frames` (each `(dt, events)`) and settles; returns what
    /// changed over the whole action.
    fn act(&mut self, frames: Vec<(f64, Vec<Event>)>) -> Json {
        let before = Snapshot::take(&self.h);
        for (dt, events) in frames {
            self.h.frame(dt, events);
        }
        self.settle();
        self.summary(&before)
    }

    fn summary(&self, before: &Snapshot) -> Json {
        let changes = before.changes(&Snapshot::take(&self.h));
        json!({"time": self.h.time(), "frames": self.h.frames(), "changes": changes})
    }

    fn reset(&mut self, args: &Json) -> Res {
        object(args)?;
        let width = coord(args, "width")?.unwrap_or(DEFAULT_SIZE.0 as f64);
        let height = coord(args, "height")?.unwrap_or(DEFAULT_SIZE.1 as f64);
        if !(100.0..=headless::MAX_SIDE as f64).contains(&width)
            || !(100.0..=headless::MAX_SIDE as f64).contains(&height)
        {
            return Err(format!(
                "the window must be 100 to {} points on each side",
                headless::MAX_SIDE
            ));
        }
        let profile = match text(args, "profile")? {
            None | Some("desktop") => Profile::Desktop,
            Some("touch") => Profile::Touch,
            Some(p) => return Err(format!("unknown profile `{p}` (desktop or touch)")),
        };
        let wb = match text(args, "script")? {
            Some(src) => Workbench::from_script(src)?,
            None => Workbench::default(),
        };
        self.h = Headless::new(wb, None, profile, (width as f32, height as f32));
        // The renderer's textures belong to the old context.
        self.renderer = None;
        self.settle();
        Ok(report::state(&mut self.h, false))
    }

    fn open(&mut self, args: &Json) -> Res {
        object(args)?;
        let name = text(args, "name")?;
        let (bytes, name) = match (
            text(args, "path")?,
            text(args, "text")?,
            text(args, "base64")?,
        ) {
            (Some(path), None, None) => {
                let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
                let file = std::path::Path::new(path)
                    .file_name()
                    .map_or_else(|| path.to_owned(), |f| f.to_string_lossy().into_owned());
                (bytes, name.map_or(file, str::to_owned))
            }
            (None, Some(t), None) => (
                t.as_bytes().to_vec(),
                name.unwrap_or("inline.geo").to_owned(),
            ),
            (None, None, Some(b)) => (
                crate::base64::decode(b)?,
                name.unwrap_or("inline.ggb").to_owned(),
            ),
            _ => return Err("give exactly one of `path`, `text` or `base64`".to_owned()),
        };
        let result = self.h.open_bytes(&bytes, &name);
        self.frame_and_settle();
        result?;
        Ok(report::state(&mut self.h, false))
    }

    fn state(&mut self, args: &Json) -> Res {
        object(args)?;
        let verbose = flag(args, "verbose")?.unwrap_or(false);
        Ok(report::state(&mut self.h, verbose))
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
                text(e, "text")?.ok_or("`text` needs a `text`")?.to_owned(),
            )),
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
        let settle = flag(args, "settle")?.unwrap_or(false);
        let with_state = flag(args, "state")?.unwrap_or(false);
        let mut report = Vec::with_capacity(frames.len());
        for (i, f) in frames.iter().enumerate() {
            let frame_dt = dt(f, DT).map_err(|e| format!("frame {i}: {e}"))?;
            let list = match f.get("events") {
                None | Some(Json::Null) => &[][..],
                Some(Json::Array(a)) => &a[..],
                Some(_) => return Err(format!("frame {i}: `events` must be a list")),
            };
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
            self.h.frame(frame_dt, events);
            let mut entry = before.changes(&Snapshot::take(&self.h));
            entry.insert("frame".into(), json!(i));
            entry.insert("t".into(), json!(self.h.time()));
            report.push(Json::Object(entry));
        }
        let settled = if settle { self.settle() } else { 0 };
        let mut out = json!({
            "frames": report,
            "settled": settled,
            "time": self.h.time(),
        });
        if with_state {
            out["state"] = report::state(&mut self.h, false);
        }
        Ok(out)
    }

    fn click(&mut self, args: &Json) -> Res {
        object(args)?;
        let pos = self.position(args)?;
        let b = button(args)?;
        let clicks = count(args, "count", 1, 3)?.unwrap_or(1);
        let m = modifiers(args)?;
        let btn = |pressed| Event::PointerButton {
            pos,
            button: b,
            pressed,
            modifiers: m,
        };
        // Held modifiers are input state too: they are pressed before the
        // pointer moves and let go after the last release.
        let mut first = Vec::new();
        if m != Modifiers::NONE {
            first.push(Event::ModifiersChanged(m));
        }
        first.push(Event::PointerMoved(pos));
        let mut frames = vec![(DT, first)];
        for _ in 0..clicks {
            frames.push((DT, vec![btn(true)]));
            frames.push((DT, vec![btn(false)]));
        }
        if m != Modifiers::NONE {
            frames.push((DT, vec![Event::ModifiersChanged(Modifiers::NONE)]));
        }
        Ok(self.act(frames))
    }

    fn drag(&mut self, args: &Json) -> Res {
        object(args)?;
        let from = self.position(args.get("from").ok_or("`drag` needs `from`")?)?;
        let to_args = args.get("to").ok_or("`drag` needs `to`")?;
        let b = button(args)?;
        let steps = count(args, "steps", 1, 1000)?.unwrap_or(10) as usize;
        let duration = num(args, "duration")?.unwrap_or(steps as f64 * DT);
        if !(0.0..=3600.0).contains(&duration) {
            return Err("`duration` must be from 0 to 3600 seconds".to_owned());
        }
        let step_dt = duration / steps as f64;
        let to = self.position(to_args)?;
        let btn = |pos, pressed| Event::PointerButton {
            pos,
            button: b,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let mut frames = vec![
            (DT, vec![Event::PointerMoved(from)]),
            (DT, vec![btn(from, true)]),
        ];
        for i in 1..=steps {
            let p = from + (to - from) * (i as f32 / steps as f32);
            frames.push((step_dt, vec![Event::PointerMoved(p)]));
        }
        frames.push((DT, vec![btn(to, false)]));
        Ok(self.act(frames))
    }

    fn type_line(&mut self, args: &Json) -> Res {
        object(args)?;
        let line = text(args, "text")?.ok_or("`type_line` needs `text`")?;
        let submit = flag(args, "submit")?.unwrap_or(true);
        let before = Snapshot::take(&self.h);
        let field = self.widget("input_bar")?.1.rect.center();
        for pressed in [None, Some(true), Some(false)] {
            let e = match pressed {
                None => Event::PointerMoved(field),
                Some(p) => headless::press(field, PointerButton::Primary, p),
            };
            self.h.frame(DT, vec![e]);
        }
        self.settle();
        self.h.frame(DT, vec![Event::Text(line.to_owned())]);
        if submit {
            self.h.frame(
                DT,
                vec![
                    headless::key(Key::Enter, true, Modifiers::NONE),
                    headless::key(Key::Enter, false, Modifiers::NONE),
                ],
            );
        }
        self.settle();
        Ok(self.summary(&before))
    }

    /// Clicks the centre of `rect` (move, press, release) and settles.
    fn click_at(&mut self, pos: Pos2) {
        self.h.frame(DT, vec![Event::PointerMoved(pos)]);
        self.h
            .frame(DT, vec![headless::press(pos, PointerButton::Primary, true)]);
        self.h.frame(
            DT,
            vec![headless::press(pos, PointerButton::Primary, false)],
        );
        self.settle();
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
        self.click_at(group.1.rect.center());
        // A group of several tools opens its drop-down: pick the entry.
        if let Some((_, entry)) = report::named_widgets(&self.h)
            .into_iter()
            .find(|(_, w)| w.kind == WidgetKind::ToolEntry && w.tool == Some(tool))
        {
            self.click_at(entry.rect.center());
        }
        if self.h.app.tool.tool() != tool {
            return Err(format!("the click did not select `{id}`"));
        }
        Ok(self.summary(&before))
    }

    fn press_toolbar(&mut self, name: &str) -> Res {
        let before = Snapshot::take(&self.h);
        let (_, w) = self.widget(name)?;
        self.click_at(w.rect.center());
        let mut out = self.summary(&before);
        out["enabled"] = json!(w.enabled);
        Ok(out)
    }

    fn resize(&mut self, args: &Json) -> Res {
        object(args)?;
        let (w, h) = self.h.size();
        let width = coord(args, "width")?.unwrap_or(w as f64);
        let height = coord(args, "height")?.unwrap_or(h as f64);
        if !(100.0..=headless::MAX_SIDE as f64).contains(&width)
            || !(100.0..=headless::MAX_SIDE as f64).contains(&height)
        {
            return Err(format!(
                "the window must be 100 to {} points on each side",
                headless::MAX_SIDE
            ));
        }
        let before = Snapshot::take(&self.h);
        self.h.set_size((width as f32, height as f32));
        self.frame_and_settle();
        Ok(self.summary(&before))
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
        self.frame_and_settle();
        Ok(self.summary(&before))
    }

    fn screenshot(&mut self, args: &Json) -> Res {
        object(args)?;
        let path = text(args, "path")?.ok_or("`screenshot` needs `path` (a .png file)")?;
        if self.renderer.is_none() {
            self.renderer = Some(Renderer::new()?);
        }
        let renderer = self.renderer.as_mut().expect("just made");
        let mut textures = self.h.take_textures();
        let out = self.h.last_output().ok_or("no frame has run yet")?;
        let (w, h) = renderer.render(self.h.ctx(), out, &mut textures, path)?;
        Ok(json!({"path": path, "width": w, "height": h}))
    }
}
