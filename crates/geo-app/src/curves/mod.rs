//! Conics and function graphs on the canvas. Their polylines come from
//! `geo_kernel::plot` (shared with the SVG writer) and are tessellated
//! once into a mesh, kept per object (by label, so that a rebuild of the
//! construction, which renumbers objects, keeps them):
//!
//! - an idle frame, or a drag of a point a curve does not depend on, only
//!   compares a few numbers per curve and hands the painter the same mesh;
//! - an edited curve (its key changed, see [`key`]) is plotted at once; a
//!   new colour only tessellates the kept polylines again;
//! - a curve is plotted with a margin of [`margin`] pixels around the area,
//!   so a pan (or a resize that keeps the scale) by less than that moves
//!   the kept mesh and shows nothing unplotted; past half the margin the
//!   curve is plotted again;
//! - a function's graph inside the area is plotted exactly as a plot of the
//!   area alone (`plot::function::plot_with_margin`), so once the view stops
//!   moving, a function whose kept plot was made for another view is
//!   plotted again for this one; a conic's polylines do not depend on where
//!   the view is, and its label is placed for each view;
//! - a view change (a zoom, a pan past the margin, a resize, another pixel
//!   density, a curve seen for the first time) plots at most
//!   [`REPLOTS_PER_FRAME`] conics a frame, a function counting as
//!   [`FUNCTION_COST`]; the others are drawn from their
//!   kept polylines moved and scaled (within [`MAX_STALE_SCALE`]; a curve
//!   seen for the first time is not drawn yet), and the next frames are
//!   asked for until none is left;
//! - a curve whose drawing lies outside the area costs only that test, and
//!   a view whose pixels the coordinates cannot tell apart draws nothing.

mod key;

use eframe::egui::epaint::{PathShape, Tessellator};
use eframe::egui::{vec2, Color32, Context, Mesh, Painter, Pos2, Rect, Stroke, Vec2};
use geo_kernel::kernel::{Construction, Id, Value};
use geo_kernel::plot::{self, Frame};
use geo_kernel::ConicData;
use key::{Closure, Data};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// The most plots one frame makes because the view moved or curves are
/// new, counting a conic as one and a function (its area and the four
/// strips around it, see `plot::function::plot_with_margin`) as
/// [`FUNCTION_COST`]; an edited curve is always plotted at once.
pub const REPLOTS_PER_FRAME: usize = 24;

/// What plotting a function counts against [`REPLOTS_PER_FRAME`].
pub const FUNCTION_COST: usize = 4;

/// The most a kept drawing is scaled (up or down) while its curve waits to
/// be plotted again; past it the curve is not drawn that frame (its points
/// would collapse, or run far off the area).
pub const MAX_STALE_SCALE: f32 = 4.0;

/// How far around the area a kept drawing that is drawn scaled may reach,
/// in lengths of the area's longer side.
const STALE_REACH: f32 = 8.0;

/// Rebuilds forget the kept drawings not asked for in this many frames.
const KEEP_FRAMES: u64 = 600;

/// How far around the area, in points, a curve is plotted: a quarter of the
/// longer side.
pub fn margin(width: f64, height: f64) -> f64 {
    (0.25 * width.max(height)).ceil()
}

thread_local! {
    static PLOTTED: Cell<u64> = const { Cell::new(0) };
    static TESSELLATED: Cell<u64> = const { Cell::new(0) };
    static UNCOVERED: Cell<u64> = const { Cell::new(0) };
}

/// How many times this thread drew a curve moved past its plotted margin
/// (a strip of it may be missing that frame).
#[cfg(test)]
pub fn curves_uncovered() -> u64 {
    UNCOVERED.with(Cell::get)
}

/// Curves this thread has plotted so far.
#[cfg(test)]
pub fn curves_plotted() -> u64 {
    PLOTTED.with(Cell::get)
}

/// Curves this thread has tessellated so far: each plot, each new colour,
/// and each frame a curve waiting to be plotted again is drawn from its
/// scaled polylines. A frame in which no curve and not the scale changed
/// adds nothing; the frame budget test and the canvas tests read it.
#[cfg(test)]
pub fn curves_tessellated() -> u64 {
    TESSELLATED.with(Cell::get)
}

/// Drops every kept drawing of `ctx`: the next frames plot afresh.
#[cfg(test)]
pub fn forget(ctx: &Context) {
    ctx.data_mut(|d| d.remove::<Arc<Mutex<Cache>>>(cache_id()));
}

fn cache_id() -> eframe::egui::Id {
    eframe::egui::Id::unique("geo_curves")
}

fn count(c: &'static std::thread::LocalKey<Cell<u64>>) {
    c.with(|n| n.set(n.get() + 1));
}

/// A plot kept for a curve, in the coordinates of the area it was made in
/// (`frame`), without the area's origin.
struct Plotted {
    frame: Frame,
    pixels_per_point: f32,
    /// The polylines, plotted `margin` points around the area.
    paths: Vec<Vec<Pos2>>,
    /// The polylines tessellated where `mesh_at + p` puts them (the area's
    /// origin when they were plotted).
    mesh: Arc<Mesh>,
    mesh_at: Pos2,
    point: Option<Pos2>,
    dot: Option<Pos2>,
    /// A function's label in `frame`; `None` for a conic.
    label: Option<(f64, f64)>,
    /// The conic, whose label is placed for each view.
    conic: Option<ConicData>,
    function: bool,
    /// Around the polylines and points; `Rect::NOTHING` when there are none.
    bounds: Rect,
    /// How far a pan may go before a strip shows that was not plotted.
    margin: f64,
}

/// One curve as drawn this frame, in screen points unless noted.
pub struct Drawn {
    pub mesh: Arc<Mesh>,
    plotted: Arc<Plotted>,
    /// Where the kept plot's area point `p` goes: `origin + scale * p`.
    origin: Pos2,
    scale: f32,
    pub point: Option<Pos2>,
    pub dot: Option<Pos2>,
    /// Relative to the area.
    pub label: Option<(f64, f64)>,
}

impl Drawn {
    /// The polylines, for the highlight.
    pub fn paths(&self) -> impl Iterator<Item = Vec<Pos2>> + '_ {
        self.plotted
            .paths
            .iter()
            .map(|path| path.iter().map(|&p| self.place(p)).collect())
    }

    fn place(&self, p: Pos2) -> Pos2 {
        self.origin + self.scale * p.to_vec2()
    }
}

struct Entry {
    data: Data,
    color: Color32,
    plotted: Arc<Plotted>,
    /// The last drawing handed out (`None`: nothing of it in the area) and
    /// the view it was for.
    shown: Option<(Frame, Option<Arc<Drawn>>)>,
    /// The frame it was last asked for in.
    seen: u64,
}

#[derive(Default)]
struct Cache {
    generation: Option<u64>,
    /// How many frames have drawn curves.
    frames: u64,
    /// The view of the last frame.
    view: Option<Frame>,
    /// By label.
    entries: HashMap<String, Entry>,
    /// What each function depends on, for this generation.
    closures: HashMap<Id, Closure>,
}

/// How a kept plot relates to the current view.
enum Fit {
    /// The same scale, moved by this much; whether every visible point was
    /// plotted, and whether the view has passed half the margin.
    Moved(Vec2, bool, bool),
    /// Another scale: the kept area point `p` goes to `origin + k * p`.
    Scaled(Pos2, f32),
}

/// The curves of one frame: the shared cache and the view they are drawn
/// in.
pub struct Curves {
    cache: Arc<Mutex<Cache>>,
    ctx: Context,
    frame: Frame,
    origin: Pos2,
    pixels_per_point: f32,
    options: eframe::egui::epaint::TessellationOptions,
    replots: Cell<usize>,
    /// The frame number.
    now: u64,
    /// Whether the last frame had the same view: the view stopped moving.
    still: bool,
    /// Whether the view can be drawn at all ([`Frame::resolves`]).
    usable: bool,
}

impl Curves {
    /// The curves of a frame drawing `rect` with `t`. `generation` counts
    /// the rebuilds of the construction: ids stay the same while it does.
    pub fn new(
        painter: &Painter,
        rect: Rect,
        t: &crate::interact::ViewTransform,
        generation: u64,
    ) -> Curves {
        let ctx = painter.ctx().clone();
        let cache = ctx.data_mut(|d| {
            d.get_temp_mut_or_default::<Arc<Mutex<Cache>>>(cache_id())
                .clone()
        });
        let frame = Frame::fit(t.view(), rect.width() as f64, rect.height() as f64);
        let usable = frame.resolves();
        let (now, still) = {
            let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
            let cache = &mut *guard;
            cache.frames += 1;
            let now = cache.frames;
            if cache.generation != Some(generation) {
                cache.closures.clear();
                cache.entries.retain(|_, e| e.seen + KEEP_FRAMES >= now);
                cache.generation = Some(generation);
            }
            let still = cache.view == Some(frame);
            cache.view = usable.then_some(frame);
            (now, still)
        };
        let pixels_per_point = ctx.pixels_per_point();
        let options = ctx.tessellation_options(|o| *o);
        Curves {
            cache,
            ctx,
            frame,
            origin: rect.min,
            pixels_per_point,
            options,
            replots: Cell::new(0),
            now,
            still,
            usable,
        }
    }

    /// Whether the frame may still plot the curve `id` because of the view;
    /// if so, it is counted.
    fn may_plot(&self, c: &Construction, id: Id) -> bool {
        let cost = match c.value(id) {
            Value::Function => FUNCTION_COST,
            _ => 1,
        };
        let n = self.replots.get() + cost;
        let may = n <= REPLOTS_PER_FRAME;
        if may {
            self.replots.set(n);
        }
        may
    }

    /// The drawing of the conic or function `id` in `color`; `None` for
    /// any other object, for a curve whose drawing lies outside the area,
    /// and for one not plotted yet.
    pub fn get(&self, c: &Construction, id: Id, color: Color32) -> Option<Arc<Drawn>> {
        if !self.usable {
            return None;
        }
        let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        // The fields are borrowed apart from each other.
        let cache = &mut *guard;
        let label = c.label(id);
        let data = match c.value(id) {
            Value::Conic { .. } => key::conic_data(&c.conic(id)?),
            Value::Function => {
                let closure = cache
                    .closures
                    .entry(id)
                    .or_insert_with(|| Closure::of(c, id));
                // Compare with what the kept drawing was made from without
                // allocating; make the new key only when it differs.
                let kept = cache.entries.get(label).is_some_and(
                    |e| matches!(&e.data, Data::Function(bits) if closure.same(c, bits)),
                );
                if kept {
                    let entry = cache.entries.get_mut(label)?;
                    return self.kept(c, id, entry, color);
                }
                closure.data(c)
            }
            _ => return None,
        };
        match cache.entries.get_mut(label) {
            Some(entry) if entry.data == data => self.kept(c, id, entry, color),
            Some(entry) => {
                // Edited: plotted at once.
                *entry = Entry {
                    data,
                    color,
                    plotted: Arc::new(self.plot(c, id, color)),
                    shown: None,
                    seen: self.now,
                };
                self.show(entry)
            }
            None if self.may_plot(c, id) => {
                let entry = cache.entries.entry(label.to_owned()).or_insert(Entry {
                    data,
                    color,
                    plotted: Arc::new(self.plot(c, id, color)),
                    shown: None,
                    seen: self.now,
                });
                self.show(entry)
            }
            None => {
                // Seen for the first time and over the cap: not drawn yet.
                self.ctx.request_repaint();
                None
            }
        }
    }

    /// A kept drawing whose key still holds: recoloured when the colour
    /// changed, plotted again when the view has left it and the frame still
    /// may.
    fn kept(
        &self,
        c: &Construction,
        id: Id,
        entry: &mut Entry,
        color: Color32,
    ) -> Option<Arc<Drawn>> {
        entry.seen = self.now;
        if entry.color != color {
            entry.color = color;
            entry.plotted = Arc::new(self.recolored(&entry.plotted, color));
            entry.shown = None;
        }
        let pending = self.pending(&entry.plotted);
        if pending.0 && self.may_plot(c, id) {
            entry.plotted = Arc::new(self.plot(c, id, color));
            entry.shown = None;
        }
        if self.pending(&entry.plotted).1 {
            self.ctx.request_repaint();
        }
        self.show(entry)
    }

    /// Whether a kept plot should be plotted again now, and whether it is
    /// not yet what an idle window shows (the next frame is asked for).
    fn pending(&self, p: &Plotted) -> (bool, bool) {
        let moved = match self.fit(p) {
            Fit::Moved(_, covered, due) => !covered || due,
            Fit::Scaled(..) => true,
        };
        let wants = moved || p.pixels_per_point != self.pixels_per_point;
        // A function's graph in the area is a plot of this view only.
        let other_view = p.function && p.frame != self.frame;
        (wants || (other_view && self.still), wants || other_view)
    }

    /// The drawing of a kept plot in this view.
    fn show(&self, entry: &mut Entry) -> Option<Arc<Drawn>> {
        if let Some((frame, drawn)) = &entry.shown {
            if *frame == self.frame {
                return drawn.clone();
            }
        }
        let drawn = self.place(entry);
        entry.shown = Some((self.frame, drawn.clone()));
        drawn
    }

    fn place(&self, entry: &Entry) -> Option<Arc<Drawn>> {
        let p = &entry.plotted;
        let (w, h) = (self.frame.width() as f32, self.frame.height() as f32);
        let area = Rect::from_min_size(self.origin, vec2(w, h));
        let (origin, scale) = match self.fit(p) {
            Fit::Moved(d, covered, _) => {
                if !covered {
                    count(&UNCOVERED);
                }
                (self.origin + d, 1.0)
            }
            Fit::Scaled(o, k) => {
                let sane = (1.0 / MAX_STALE_SCALE..=MAX_STALE_SCALE).contains(&k)
                    && o.x.is_finite()
                    && o.y.is_finite();
                if !sane {
                    return None;
                }
                (self.origin + o.to_vec2(), k)
            }
        };
        let place = |q: Pos2| origin + scale * q.to_vec2();
        // The label: a conic's is placed for this view; a function's is the
        // plotter's when the plot was made for this view, otherwise its
        // first drawn point in the area.
        let label = match &p.conic {
            Some(d) => plot::conic::label(d, &self.frame),
            None if p.frame == self.frame => p.label,
            None => self.function_label(p, &place),
        };
        let label_shows = label.is_some_and(|(x, y)| {
            let pad = plot::PAD;
            (-pad..=w as f64 + pad).contains(&x) && (-pad..=h as f64 + pad).contains(&y)
        });
        // The strokes, where they go now, against the area (a stroke may
        // reach a little beyond).
        let bounds = Rect::from_two_pos(place(p.bounds.min), place(p.bounds.max));
        let strokes = p.bounds != Rect::NOTHING && bounds.intersects(area.expand(24.0)) && {
            let near = area.expand(plot::PAD as f32);
            p.paths.iter().any(|path| {
                path.windows(2)
                    .any(|w| segment_meets(place(w[0]), place(w[1]), near))
            }) || p
                .point
                .iter()
                .chain(&p.dot)
                .any(|&q| near.contains(place(q)))
        };
        if !strokes && !label_shows {
            return None;
        }
        let reach = STALE_REACH * w.max(h);
        if scale != 1.0 && !area.expand(reach).contains_rect(bounds) {
            return None;
        }
        let mesh = if !strokes {
            Arc::new(Mesh::default())
        } else if scale == 1.0 && origin == p.mesh_at {
            p.mesh.clone()
        } else if scale == 1.0 {
            let mut m = (*p.mesh).clone();
            m.translate(origin - p.mesh_at);
            Arc::new(m)
        } else {
            // Scaled for a frame or two: only the polylines that reach the
            // area are tessellated (most of a function's margin strips do
            // not).
            let near = area.expand(plot::PAD as f32);
            let points = p
                .paths
                .iter()
                .map(|path| path.iter().map(|&q| place(q)).collect::<Vec<_>>())
                .filter(|path| match path.as_slice() {
                    [q] => near.contains(*q),
                    _ => path.windows(2).any(|w| segment_meets(w[0], w[1], near)),
                });
            Arc::new(self.tessellate(points, entry.color))
        };
        Some(Arc::new(Drawn {
            mesh,
            plotted: p.clone(),
            origin,
            scale,
            point: p.point.map(place),
            dot: p.dot.map(place),
            label,
        }))
    }

    /// Where a function's label goes while it is drawn from a plot made for
    /// another view: its first drawn point in the area (`CurvePlotter`
    /// places it at the first sample on screen).
    fn function_label(&self, p: &Plotted, place: &dyn Fn(Pos2) -> Pos2) -> Option<(f64, f64)> {
        let (w, h) = (self.frame.width(), self.frame.height());
        p.paths.iter().flatten().find_map(|&q| {
            let s = place(q) - self.origin;
            let (x, y) = (s.x as f64, s.y as f64);
            ((0.0..=w).contains(&x) && (0.0..=h).contains(&y)).then(|| (x.trunc(), y.trunc()))
        })
    }

    fn fit(&self, p: &Plotted) -> Fit {
        match p.frame.shift_to(&self.frame) {
            Some((dx, dy)) => {
                // The current area in the kept plot's pixels, against the
                // kept area grown by `m`.
                let (pw, ph) = (p.frame.width(), p.frame.height());
                let (w, h) = (self.frame.width(), self.frame.height());
                let within =
                    |m: f64| -dx >= -m && w - dx <= pw + m && -dy >= -m && h - dy <= ph + m;
                Fit::Moved(
                    vec2(dx as f32, dy as f32),
                    within(p.margin),
                    !within(0.5 * p.margin),
                )
            }
            None => {
                let k = self.frame.scale() / p.frame.scale();
                let (wx, wy) = p.frame.screen_to_world(0.0, 0.0);
                let (ox, oy) = self.frame.world_to_screen(wx, wy);
                Fit::Scaled(Pos2::new(ox as f32, oy as f32), k as f32)
            }
        }
    }

    fn plot(&self, c: &Construction, id: Id, color: Color32) -> Plotted {
        count(&PLOTTED);
        let margin = margin(self.frame.width(), self.frame.height());
        let at = |(x, y): (f64, f64)| Pos2::new(x as f32, y as f32);
        let (paths, point, dot, label, conic, function) = match c.value(id) {
            Value::Conic { .. } => match c.conic(id) {
                Some(d) => {
                    let p = plot::conic::plot(&d, &self.frame.grown(margin));
                    let back = |(x, y): (f64, f64)| at((x - margin, y - margin));
                    let paths: Vec<Vec<Pos2>> = p
                        .paths
                        .into_iter()
                        .map(|path| path.into_iter().map(back).collect())
                        .collect();
                    (
                        paths,
                        p.point.map(back),
                        p.dot.map(back),
                        None,
                        Some(d),
                        false,
                    )
                }
                None => (Vec::new(), None, None, None, None, false),
            },
            _ => {
                let mut f = |x: f64| c.function_value(id, x).unwrap_or(f64::NAN);
                let p = plot::function::plot_with_margin(&mut f, &self.frame, margin);
                let paths = p
                    .paths
                    .into_iter()
                    .map(|path| path.into_iter().map(at).collect())
                    .collect();
                (paths, None, None, p.label, None, true)
            }
        };
        let mut bounds = Rect::NOTHING;
        for &q in paths.iter().flatten().chain(&point).chain(&dot) {
            bounds.extend_with(q);
        }
        let mesh = Arc::new(self.tessellate(at_origin(&paths, self.origin), color));
        Plotted {
            frame: self.frame,
            pixels_per_point: self.pixels_per_point,
            paths,
            mesh,
            mesh_at: self.origin,
            point,
            dot,
            label,
            conic,
            function,
            bounds,
            margin,
        }
    }

    /// The same plot in another colour.
    fn recolored(&self, p: &Plotted, color: Color32) -> Plotted {
        Plotted {
            frame: p.frame,
            pixels_per_point: p.pixels_per_point,
            paths: p.paths.clone(),
            mesh: Arc::new(self.tessellate(at_origin(&p.paths, p.mesh_at), color)),
            mesh_at: p.mesh_at,
            point: p.point,
            dot: p.dot,
            label: p.label,
            conic: p.conic,
            function: p.function,
            bounds: p.bounds,
            margin: p.margin,
        }
    }

    fn tessellate(&self, paths: impl Iterator<Item = Vec<Pos2>>, color: Color32) -> Mesh {
        count(&TESSELLATED);
        let stroke = Stroke::new(crate::draw::CURVE_WIDTH, color);
        let mut tess = Tessellator::new(self.pixels_per_point, self.options, [1, 1], Vec::new());
        let mut mesh = Mesh::default();
        for points in paths {
            tess.tessellate_path(&PathShape::line(points, stroke), &mut mesh);
        }
        mesh
    }
}

/// The polylines `paths` moved by `origin`.
fn at_origin(paths: &[Vec<Pos2>], origin: Pos2) -> impl Iterator<Item = Vec<Pos2>> + '_ {
    paths
        .iter()
        .map(move |path| path.iter().map(|&q| q + origin.to_vec2()).collect())
}

/// Whether the segment from `a` to `b` has a point in `r` (Liang and
/// Barsky).
fn segment_meets(a: Pos2, b: Pos2, r: Rect) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d.x, a.x - r.min.x),
        (d.x, r.max.x - a.x),
        (-d.y, a.y - r.min.y),
        (d.y, r.max.y - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else if p < 0.0 {
            t0 = t0.max(q / p);
        } else {
            t1 = t1.min(q / p);
        }
    }
    t0 <= t1
}
