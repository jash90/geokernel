//! The window driven without a window: synthetic `egui::RawInput` through
//! `egui::Context::run_ui` and the window's own UI function
//! ([`crate::app::show`]), with a clock the caller owns, and no GPU.
//!
//! Shared by the frame benchmark (the free functions [`raw_input`], [`run`]
//! and the counters), the window tests (the touch emulation and the event
//! constructors) and the `geokernel-mcp` server ([`Headless`]). Only tests
//! and the `headless` cargo feature compile it.

mod touch;
mod widgets;

use crate::app::{self, App};
use crate::geom::ToEgui;
use crate::interact::ViewTransform;
use crate::workbench::Workbench;
pub use eframe::egui;
use eframe::egui::{
    epaint, vec2, ClippedPrimitive, Context, Event, FullOutput, Key, Modifiers, PointerButton,
    Pos2, RawInput, Rect, Shape, TexturesDelta, ViewportId,
};
use geo_app_core::metrics::Metrics;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
pub use touch::TouchEmulation;
pub use widgets::{Widget, WidgetKind};

/// The frame interval of a display at 60 Hz, the default step of the clock.
pub const DT: f64 = 1.0 / 60.0;

/// A press (`pressed`) or release of `button` at `pos`.
pub fn press(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// A press or release of `key` with `modifiers` held.
pub fn key(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    }
}

/// The input of one frame of a focused window of `screen`'s size at `time`
/// seconds. Held modifier keys are events (`Event::ModifiersChanged`).
pub fn raw_input(screen: Rect, time: f64, predicted_dt: f32, events: Vec<Event>) -> RawInput {
    RawInput {
        screen_rect: Some(screen),
        time: Some(time),
        predicted_dt,
        events,
        focused: true,
        ..Default::default()
    }
}

/// What one frame did besides its shapes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PassCounts {
    /// egui passes in this frame (more than one means a discarded pass).
    pub passes: u64,
    /// Algebra rows built and construction rebuilds during the frame.
    pub rows_built: u64,
    pub rebuilds: u64,
}

/// One frame of the whole window: `input` through `show`, the window's UI
/// function ([`crate::app::show`]; the benchmark passes it through its
/// adapter). Reads a few counters before and after, and allocates nothing
/// besides what the frame itself does.
pub fn run(
    ctx: &Context,
    app: &mut App,
    input: RawInput,
    mut show: impl FnMut(&mut App, &mut egui::Ui),
) -> (FullOutput, PassCounts) {
    let pass0 = ctx.cumulative_pass_nr();
    let rows0 = app.wb.rows_built();
    let rebuilds0 = app.wb.rebuilds();
    let out = ctx.run_ui(input, |ui| show(app, ui));
    let counts = PassCounts {
        passes: ctx.cumulative_pass_nr() - pass0,
        rows_built: app.wb.rows_built() - rows0,
        rebuilds: app.wb.rebuilds() - rebuilds0,
    };
    (out, counts)
}

/// The repaint a frame asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Repaint {
    #[default]
    None,
    Immediate,
    /// After this many milliseconds.
    After(f64),
}

/// The repaint the frame that produced `out` asked for.
pub fn repaint(out: &FullOutput) -> Repaint {
    match out.viewport_output.get(&ViewportId::ROOT) {
        Some(v) if v.repaint_delay == Duration::ZERO => Repaint::Immediate,
        Some(v) if v.repaint_delay < Duration::MAX => {
            Repaint::After(v.repaint_delay.as_secs_f64() * 1000.0)
        }
        _ => Repaint::None,
    }
}

/// The shapes of one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShapeCounts {
    /// Leaf shapes (nested `Shape::Vec` flattened).
    pub shapes: u64,
    pub text_shapes: u64,
    pub glyphs: u64,
    /// Text shapes whose galley was not painted in the previous frame,
    /// that is, laid out anew (or re-fetched after leaving the cache).
    pub new_galleys: u64,
}

fn walk(shape: &Shape, f: &mut ShapeCounts, seen: &mut HashSet<usize>, prev: &HashSet<usize>) {
    match shape {
        Shape::Noop => {}
        Shape::Vec(v) => v.iter().for_each(|s| walk(s, f, seen, prev)),
        Shape::Text(t) => {
            f.shapes += 1;
            f.text_shapes += 1;
            f.glyphs += t
                .galley
                .rows
                .iter()
                .map(|r| r.glyphs.len() as u64)
                .sum::<u64>();
            let ptr = Arc::as_ptr(&t.galley) as usize;
            if seen.insert(ptr) && !prev.contains(&ptr) {
                f.new_galleys += 1;
            }
        }
        _ => f.shapes += 1,
    }
}

/// Counts the shapes of `out`; `galleys` holds the galleys of the previous
/// frame and is replaced by this frame's.
pub fn count_shapes(out: &FullOutput, galleys: &mut HashSet<usize>) -> ShapeCounts {
    let mut f = ShapeCounts::default();
    let mut seen = HashSet::new();
    for clipped in &out.shapes {
        walk(&clipped.shape, &mut f, &mut seen, galleys);
    }
    *galleys = seen;
    f
}

/// The meshes of one tessellated frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeshCounts {
    pub meshes: u64,
    pub vertices: u64,
    pub indices: u64,
}

pub fn count_meshes(prims: &[ClippedPrimitive]) -> MeshCounts {
    let mut m = MeshCounts::default();
    for p in prims {
        if let epaint::Primitive::Mesh(mesh) = &p.primitive {
            m.meshes += 1;
            m.vertices += mesh.vertices.len() as u64;
            m.indices += mesh.indices.len() as u64;
        }
    }
    m
}

/// The deterministic counters of one frame (the frame benchmark's counters
/// without its times and allocation counts).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameStats {
    pub shapes: ShapeCounts,
    pub meshes: MeshCounts,
    pub passes: PassCounts,
    pub texture_updates: u64,
    pub repaint: Repaint,
}

/// The input profile of a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Desktop,
    Touch,
}

impl Profile {
    pub fn metrics(self) -> Metrics {
        match self {
            Profile::Desktop => Metrics::desktop(),
            Profile::Touch => Metrics::touch(),
        }
    }
}

/// The largest window side accepted, in points.
pub const MAX_SIDE: f32 = 8192.0;

/// The whole window without a window: its egui context, the application
/// state, the window size, a clock that only moves when told to, and the
/// platform's touch-to-pointer emulation. Every input goes in as egui
/// events, one frame at a time, through the same UI function the window
/// runs.
pub struct Headless {
    ctx: Context,
    pub app: App,
    profile: Profile,
    size: (f32, f32),
    /// The time of the last frame, in seconds; `None` before the first.
    time: Option<f64>,
    galleys: HashSet<usize>,
    touch: TouchEmulation,
    /// The last frame's output (its shapes; its texture changes are moved
    /// into `textures`).
    last: Option<FullOutput>,
    stats: FrameStats,
    textures: TexturesDelta,
    frames: u64,
}

impl Headless {
    /// A window of `size` points with the document `wb` and the profile
    /// `profile`, set up as the desktop window sets up its context. No frame
    /// has run yet. AccessKit output is on, so that [`Headless::widgets`]
    /// can name what is on screen.
    pub fn new(
        wb: Workbench,
        load_error: Option<String>,
        profile: Profile,
        size: (f32, f32),
    ) -> Headless {
        let ctx = Context::default();
        let metrics = profile.metrics();
        crate::configure(&ctx, &metrics);
        ctx.enable_accesskit();
        let mut app = App::new(wb, load_error);
        app.set_metrics(metrics);
        Headless {
            ctx,
            app,
            profile,
            size: clamp_size(size),
            time: None,
            galleys: HashSet::new(),
            touch: TouchEmulation::default(),
            last: None,
            stats: FrameStats::default(),
            textures: TexturesDelta::default(),
            frames: 0,
        }
    }

    pub fn ctx(&self) -> &Context {
        &self.ctx
    }

    pub fn profile(&self) -> Profile {
        self.profile
    }

    pub fn size(&self) -> (f32, f32) {
        self.size
    }

    /// The window size the next frames get (clamped to a sane range).
    pub fn set_size(&mut self, size: (f32, f32)) {
        self.size = clamp_size(size);
    }

    /// The time of the last frame (0 before the first).
    pub fn time(&self) -> f64 {
        self.time.unwrap_or(0.0)
    }

    /// Frames run since the window was made.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// The window rectangle.
    pub fn screen(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(self.size.0, self.size.1))
    }

    /// Runs one frame `dt` seconds after the previous one (the first frame
    /// is at time 0) with `events`.
    pub fn frame(&mut self, dt: f64, events: Vec<Event>) -> Repaint {
        let time = match self.time {
            Some(t) => t + dt.max(0.0),
            None => 0.0,
        };
        self.time = Some(time);
        let input = raw_input(self.screen(), time, DT as f32, events);
        let (mut out, passes) = run(&self.ctx, &mut self.app, input, app::show);
        self.frames += 1;
        let shapes = count_shapes(&out, &mut self.galleys);
        let texture_updates = out.textures_delta.set.len() as u64;
        self.textures
            .append(std::mem::take(&mut out.textures_delta));
        let repaint = repaint(&out);
        self.stats = FrameStats {
            shapes,
            meshes: MeshCounts::default(),
            passes,
            texture_updates,
            repaint,
        };
        self.last = Some(out);
        repaint
    }

    /// Runs empty frames `dt` apart while the last frame asked to be
    /// repainted at once, at most `max` of them, as a window does while it
    /// animates. Returns how many ran.
    pub fn settle(&mut self, dt: f64, max: usize) -> usize {
        let mut n = 0;
        while n < max && (self.last.is_none() || self.stats.repaint == Repaint::Immediate) {
            self.frame(dt, Vec::new());
            n += 1;
        }
        n
    }

    /// The events a platform layer sends for one touch event of finger `id`
    /// (see [`TouchEmulation`]).
    pub fn touch(&mut self, id: u64, phase: egui::TouchPhase, pos: Pos2) -> Vec<Event> {
        self.touch.touch(id, phase, pos)
    }

    /// The touch emulation's state, to restore after a frame that was not
    /// run.
    pub fn touch_state(&self) -> TouchEmulation {
        self.touch.clone()
    }

    pub fn set_touch_state(&mut self, touch: TouchEmulation) {
        self.touch = touch;
    }

    /// The last frame's output, if a frame ran.
    pub fn last_output(&self) -> Option<&FullOutput> {
        self.last.as_ref()
    }

    /// The texture changes of all frames since the last call (a renderer
    /// that starts late needs all of them, from the font atlas on).
    pub fn take_textures(&mut self) -> TexturesDelta {
        std::mem::take(&mut self.textures)
    }

    /// The counters of the last frame, with its shapes tessellated now.
    pub fn frame_stats(&self) -> FrameStats {
        let mut stats = self.stats;
        if let Some(out) = &self.last {
            let prims = self
                .ctx
                .tessellate(out.shapes.clone(), out.pixels_per_point);
            stats.meshes = count_meshes(&prims);
        }
        stats
    }

    /// The drawing area and its transform as last painted.
    pub fn view(&self) -> Option<(Rect, ViewTransform)> {
        self.app.view.map(|(r, t)| (r.egui(), t))
    }

    /// Where the world point `(x, y)` is drawn now, in window points.
    pub fn world_to_screen(&self, x: f64, y: f64) -> Option<Pos2> {
        let (r, t) = self.view()?;
        let (sx, sy) = t.world_to_screen(x, y);
        Some(r.min + vec2(sx as f32, sy as f32))
    }

    /// The world point under the window position `p`.
    pub fn screen_to_world(&self, p: Pos2) -> Option<(f64, f64)> {
        let (r, t) = self.view()?;
        let d = p - r.min;
        Some(t.screen_to_world(d.x as f64, d.y as f64))
    }

    /// Opens a script or construction file given as bytes, as dropping the
    /// file on the window does (`name` tells the format by its extension).
    pub fn open_bytes(&mut self, bytes: &[u8], name: &str) -> Result<(), String> {
        let result = self.app.wb.load_bytes(bytes, name);
        app::opened(&mut self.app, result.clone());
        result
    }

    /// The named rectangles on screen in the last frame.
    pub fn widgets(&self) -> Vec<Widget> {
        let update = self
            .last
            .as_ref()
            .and_then(|o| o.platform_output.accesskit_update.as_ref());
        widgets::collect(&self.ctx, &self.app, update)
    }

    /// The window's regions as last laid out.
    pub fn regions(&self) -> crate::layout::Regions {
        self.app.regions
    }

    /// The rectangle of the graphics view as last laid out.
    pub fn graphics(&self) -> Rect {
        self.app.regions.graphics.egui()
    }
}

impl Drop for Headless {
    fn drop(&mut self) {
        // Texture changes no renderer took are dropped on purpose.
        self.textures.clear();
    }
}

fn clamp_size((w, h): (f32, f32)) -> (f32, f32) {
    let side = |v: f32| {
        if v.is_finite() {
            v.clamp(1.0, MAX_SIDE)
        } else {
            1.0
        }
    };
    (side(w), side(h))
}
