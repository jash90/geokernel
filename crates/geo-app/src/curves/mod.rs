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
//!   [`FUNCTION_COST`] ([`REPLOTS_PER_PAN_FRAME`] while the view pans);
//!   the others are drawn from their kept meshes moved and scaled (within
//!   [`MAX_STALE_SCALE`]; a curve seen for the first time is not drawn
//!   yet), and the next frames are asked for until none is left;
//! - a kept mesh is moved or scaled by copying only its runs of triangles
//!   that reach the area (see [`placed_runs`]), never tessellated again
//!   unless epaint laid it out otherwise;
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

/// [`REPLOTS_PER_FRAME`] for a frame of a pan, which also moves every kept
/// mesh: the curves due again are still drawn whole for a while (they are
/// within the margin), and once the view stops the full cap plots them.
pub const REPLOTS_PER_PAN_FRAME: usize = 12;

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
    static MOVED: Cell<u64> = const { Cell::new(0) };
    static COST: Cell<u64> = const { Cell::new(0) };
}

/// How many times this thread drew a curve moved past its plotted margin
/// (a strip of it may be missing that frame).
#[cfg(test)]
pub fn curves_uncovered() -> u64 {
    UNCOVERED.with(Cell::get)
}

/// Vertices this thread has copied to move or scale kept meshes so far.
#[cfg(test)]
pub fn curves_moved_vertices() -> u64 {
    MOVED.with(Cell::get)
}

/// Curves this thread has plotted so far.
#[cfg(test)]
pub fn curves_plotted() -> u64 {
    PLOTTED.with(Cell::get)
}

/// What the plots this thread has made so far count against
/// [`REPLOTS_PER_FRAME`] ([`plot_cost`] each), capped or not.
#[cfg(test)]
pub fn curves_plot_cost() -> u64 {
    COST.with(Cell::get)
}

/// What plotting `id` counts against [`REPLOTS_PER_FRAME`].
fn plot_cost(c: &Construction, id: Id) -> usize {
    match c.value(id) {
        Value::Function => FUNCTION_COST,
        _ => 1,
    }
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
/// A run of a mesh's triangles: `indices[tris]`, which name only vertices
/// in `vertices`, whose positions lie in `bounds`.
struct Run {
    tris: std::ops::Range<u32>,
    vertices: std::ops::Range<u32>,
    bounds: Rect,
}

/// Triangles per [`Run`].
const RUN_TRIANGLES: usize = 64;

/// The runs of `mesh`. The tessellator lays a stroke's triangles and
/// vertices along the path, so a run names a short stretch of vertices.
fn runs(mesh: &Mesh) -> Arc<[Run]> {
    mesh.indices
        .chunks(3 * RUN_TRIANGLES)
        .enumerate()
        .map(|(k, tris)| {
            let (lo, hi) = tris
                .iter()
                .fold((u32::MAX, 0), |(lo, hi), &i| (lo.min(i), hi.max(i)));
            let mut bounds = Rect::NOTHING;
            for v in &mesh.vertices[lo as usize..=hi as usize] {
                bounds.extend_with(v.pos);
            }
            let start = (k * 3 * RUN_TRIANGLES) as u32;
            Run {
                tris: start..start + tris.len() as u32,
                vertices: lo..hi + 1,
                bounds,
            }
        })
        .collect()
}

/// The stroke of the open polylines `paths` as epaint tessellates it, and
/// whether it has four vertices for each point of every polyline of two
/// points or more, laid out as a thick stroke (epaint adds a point at a
/// sharp corner, and lays a thin stroke out with three), as
/// [`placed_runs`] needs to scale it.
fn stroke_mesh(
    tess: &mut Tessellator,
    paths: impl Iterator<Item = Vec<Pos2>>,
    stroke: Stroke,
) -> (Mesh, bool) {
    let mut mesh = Mesh::default();
    let mut fours = true;
    for points in paths {
        let (n, before) = (points.len(), mesh.vertices.len());
        tess.tessellate_path(&PathShape::line(points, stroke), &mut mesh);
        let added = &mesh.vertices[before..];
        // A thick stroke: per point a clear outer vertex, two of the stroke's
        // colour, a clear outer vertex. A thin one (at a low pixel density)
        // has three per point, clear, faded, clear, and no four of them in a
        // row look like that.
        fours = fours
            && added.len() == if n >= 2 { 4 * n } else { 0 }
            && added.as_chunks::<4>().0.iter().all(|q| {
                q[0].color == Color32::TRANSPARENT
                    && q[1].color == stroke.color
                    && q[2].color == stroke.color
                    && q[3].color == Color32::TRANSPARENT
            });
    }
    (mesh, fours)
}

/// Where a kept mesh goes: a point `p` of it to `origin + scale * (p -
/// from)`. A scaled stroke keeps its width (see [`placed_runs`]).
#[derive(Clone, Copy)]
struct Placing {
    from: Pos2,
    origin: Pos2,
    scale: f32,
}

impl Placing {
    fn point(&self, p: Pos2) -> Pos2 {
        self.origin + self.scale * (p - self.from)
    }
}

/// The runs of `mesh` (see [`runs`]) that reach `near` placed by `at`,
/// placed. Inside `near` it draws what the whole mesh placed draws: a
/// triangle left out lies wholly outside. Runs next to each other share
/// their vertices, each copied once.
///
/// A moved mesh may have any layout. A scaled mesh must be a stroke of
/// open polylines as epaint tessellates it with four vertices per point,
/// the middle two at the same distance on either side of the point, which
/// `fours` says (see [`stroke_mesh`]); otherwise it is `None`. Each point
/// is placed and its four vertices keep their offsets from it, which is
/// the stroke of the placed polyline (a uniform scale keeps every
/// direction, so the normals and the joins are the same).
fn placed_runs(mesh: &Mesh, runs: &[Run], fours: bool, at: Placing, near: Rect) -> Option<Mesh> {
    let quads = at.scale != 1.0;
    if quads && !fours {
        return None;
    }
    let len = mesh.vertices.len() as u32;
    let grow = if at.scale == 1.0 {
        0.0
    } else {
        2.0 * crate::draw::CURVE_WIDTH
    };
    let keep = |run: &&Run| {
        let b = Rect::from_two_pos(at.point(run.bounds.min), at.point(run.bounds.max));
        b.expand(grow).intersects(near)
    };
    // The vertices of a run. Scaled, a stretch starts and ends on a point
    // (four vertices, and the polylines start on one).
    let span = |run: &Run| {
        let (lo, hi) = (run.vertices.start, run.vertices.end);
        let (lo, hi) = if quads {
            (lo & !3, (hi + 3) & !3)
        } else {
            (lo, hi)
        };
        (lo, hi.min(len))
    };
    // The vertices and indices to copy, to allocate once (the same walk
    // as below).
    let (mut nv, mut ni, mut from, mut copied) = (0u32, 0usize, 0u32, 0u32);
    for run in runs.iter().filter(keep) {
        let (lo, hi) = span(run);
        if lo < from || lo > copied {
            from = lo;
            copied = lo;
        }
        if hi > copied {
            nv += hi - copied;
            copied = hi;
        }
        ni += run.tris.len();
    }
    MOVED.with(|n| n.set(n.get() + u64::from(nv)));
    let mut out = Mesh::with_texture(mesh.texture_id);
    out.vertices.reserve_exact(nv as usize);
    out.indices.reserve_exact(ni);
    let d = at.origin - at.from;
    // `mesh.vertices[from..copied]` are at `out.vertices[base..]`.
    let (mut from, mut copied, mut base) = (0u32, 0u32, 0u32);
    for run in runs.iter().filter(keep) {
        let (lo, hi) = span(run);
        if lo < from || lo > copied {
            // Not joined to what was copied last: a new stretch.
            from = lo;
            copied = lo;
            base = out.vertices.len() as u32;
        }
        if hi > copied {
            let src = &mesh.vertices[copied as usize..hi as usize];
            if !quads {
                out.vertices.extend(src.iter().map(|v| {
                    let mut v = *v;
                    v.pos += d;
                    v
                }));
            } else {
                for quad in src.as_chunks::<4>().0 {
                    let mid = quad[1].pos.lerp(quad[2].pos, 0.5);
                    let shift = at.point(mid) - mid;
                    out.vertices.extend(quad.iter().map(|v| {
                        let mut v = *v;
                        v.pos += shift;
                        v
                    }));
                }
            }
            copied = hi;
        }
        out.indices.extend(
            mesh.indices[run.tris.start as usize..run.tris.end as usize]
                .iter()
                .map(|&i| i - from + base),
        );
    }
    Some(out)
}

/// The stroke of the polylines `paths` (in the coordinates of the area
/// the kept stroke was made in, without its origin) placed by `at`, as far
/// as it shows in `area`. Moved, or scaled for a frame or two, it is the
/// kept stroke (`mesh` with its `runs`, and whether it has four vertices
/// per point) placed, only the runs that reach the area (the clip);
/// scaled, a stroke not laid out as [`placed_runs`] needs is the stretches
/// of the polylines that reach the area, tessellated by `tessellate`.
fn placed_stroke(
    (mesh, runs, fours): (&Mesh, &[Run], bool),
    paths: &[Vec<Pos2>],
    at: Placing,
    area: Rect,
    tessellate: impl FnOnce(Vec<Vec<Pos2>>) -> Mesh,
) -> Mesh {
    if let Some(placed) = placed_runs(mesh, runs, fours, at, area.expand(1.0)) {
        return placed;
    }
    let near = area.expand(plot::PAD as f32);
    let mut pieces = Vec::new();
    for path in paths {
        near_pieces(
            path.iter().map(|&q| at.origin + at.scale * q.to_vec2()),
            near,
            &mut pieces,
        );
    }
    tessellate(pieces)
}

struct Plotted {
    frame: Frame,
    pixels_per_point: f32,
    /// The polylines, plotted `margin` points around the area.
    paths: Vec<Vec<Pos2>>,
    /// The polylines tessellated where `mesh_at + p` puts them (the area's
    /// origin when they were plotted).
    mesh: Arc<Mesh>,
    mesh_at: Pos2,
    /// The mesh's triangles in runs, each with the bounds of its vertices,
    /// so that a moved drawing copies only the runs that reach the area.
    runs: Arc<[Run]>,
    /// Whether the mesh has four vertices for each point of the polylines
    /// (of two points or more), as `placed_runs` needs to scale it (see
    /// `stroke_mesh`).
    points_of_four: bool,
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
    /// The last drawing handed out (`None`: nothing of it in the area), the
    /// view it was for and where on the screen that view's area was.
    shown: Option<(Frame, Pos2, Option<Arc<Drawn>>)>,
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
    /// Whether the view moves at the same scale and size as the last
    /// frame's (a pan): the frame moves every kept mesh, so it plots less.
    panning: bool,
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
        let (now, still, panning) = {
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
            let panning = cache.view.is_some_and(|v| {
                !still
                    && (v.scale() - frame.scale()).abs() <= 1e-9 * frame.scale()
                    && v.width() == frame.width()
                    && v.height() == frame.height()
            });
            cache.view = usable.then_some(frame);
            (now, still, panning)
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
            panning,
            usable,
        }
    }

    /// Whether the frame may still plot the curve `id` because of the view;
    /// if so, it is counted.
    fn may_plot(&self, c: &Construction, id: Id) -> bool {
        let cap = if self.panning {
            REPLOTS_PER_PAN_FRAME
        } else {
            REPLOTS_PER_FRAME
        };
        let n = self.replots.get() + plot_cost(c, id);
        let may = n <= cap;
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
        // The drawing is in screen coordinates: the same view in an area
        // elsewhere on the screen is placed again.
        if let Some((frame, origin, drawn)) = &entry.shown {
            if *frame == self.frame && *origin == self.origin {
                return drawn.clone();
            }
        }
        let drawn = self.place(entry);
        entry.shown = Some((self.frame, self.origin, drawn.clone()));
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
        } else {
            let at = Placing {
                from: p.mesh_at,
                origin,
                scale,
            };
            let kept = (&*p.mesh, &*p.runs, p.points_of_four);
            Arc::new(placed_stroke(kept, &p.paths, at, area, |pieces| {
                self.tessellate(pieces.into_iter(), entry.color).0
            }))
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
        COST.with(|n| n.set(n.get() + plot_cost(c, id) as u64));
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
        let (mesh, points_of_four) = self.tessellate(at_origin(&paths, self.origin), color);
        let mesh = Arc::new(mesh);
        Plotted {
            frame: self.frame,
            pixels_per_point: self.pixels_per_point,
            paths,
            runs: runs(&mesh),
            points_of_four,
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
        // Tessellated at this frame's density, which may lay the stroke out
        // differently: its own runs.
        let (mesh, points_of_four) = self.tessellate(at_origin(&p.paths, p.mesh_at), color);
        Plotted {
            frame: p.frame,
            pixels_per_point: p.pixels_per_point,
            paths: p.paths.clone(),
            runs: runs(&mesh),
            mesh: Arc::new(mesh),
            mesh_at: p.mesh_at,
            points_of_four,
            point: p.point,
            dot: p.dot,
            label: p.label,
            conic: p.conic,
            function: p.function,
            bounds: p.bounds,
            margin: p.margin,
        }
    }

    /// The stroke of `paths` in `color`, and whether [`placed_runs`] can
    /// scale it (see [`stroke_mesh`]).
    fn tessellate(&self, paths: impl Iterator<Item = Vec<Pos2>>, color: Color32) -> (Mesh, bool) {
        count(&TESSELLATED);
        let stroke = Stroke::new(crate::draw::CURVE_WIDTH, color);
        let mut tess = Tessellator::new(self.pixels_per_point, self.options, [1, 1], Vec::new());
        stroke_mesh(&mut tess, paths, stroke)
    }
}

/// Adds to `out` the stretches of the polyline `path` made of segments that
/// meet `near` (a single point if it lies in `near`). A segment left out
/// does not meet `near`, so where a stretch ends (its cap, or the join it
/// lost) lies outside `near`: inside the area a stroke of the stretches
/// draws what a stroke of the whole polyline draws, as long as `near`
/// reaches past the area by more than the stroke.
fn near_pieces(path: impl Iterator<Item = Pos2>, near: Rect, out: &mut Vec<Vec<Pos2>>) {
    let mut piece: Vec<Pos2> = Vec::new();
    let mut last: Option<Pos2> = None;
    let mut single = true;
    for q in path {
        if let Some(a) = last {
            single = false;
            if segment_meets(a, q, near) {
                if piece.is_empty() {
                    piece.push(a);
                }
                piece.push(q);
            } else if !piece.is_empty() {
                out.push(std::mem::take(&mut piece));
            }
        }
        last = Some(q);
    }
    if !piece.is_empty() {
        out.push(piece);
    } else if let (true, Some(q)) = (single, last) {
        if near.contains(q) {
            out.push(vec![q]);
        }
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

#[cfg(test)]
mod placing_tests {
    use super::*;
    use eframe::egui::epaint::TessellationOptions;

    fn stroke(paths: &[Vec<Pos2>], ppp: f32) -> (Mesh, bool) {
        let stroke = Stroke::new(crate::draw::CURVE_WIDTH, Color32::BLACK);
        let mut tess = Tessellator::new(ppp, TessellationOptions::default(), [1, 1], Vec::new());
        stroke_mesh(&mut tess, paths.iter().cloned(), stroke)
    }

    /// A wave, a closed loop, a single point and a segment, in the
    /// coordinates of an area at `from`.
    fn paths(from: Pos2) -> Vec<Vec<Pos2>> {
        let wave = (0..400)
            .map(|i| from + vec2(i as f32 * 2.0, 200.0 + 80.0 * (i as f32 * 0.05).sin()))
            .collect();
        let lp: Vec<Pos2> = (0..=90)
            .map(|i| {
                let t = i as f32 / 90.0 * std::f32::consts::TAU;
                from + vec2(400.0 + 150.0 * t.cos(), 300.0 + 60.0 * t.sin())
            })
            .collect();
        vec![
            wave,
            lp,
            vec![from + vec2(5.0, 5.0)],
            vec![from + vec2(10.0, 500.0), from + vec2(790.0, 20.0)],
        ]
    }

    /// Asserts that `part` holds every triangle of `whole` that reaches
    /// `near`, and that some do.
    fn assert_reaches(whole: &Mesh, part: &Mesh, near: Rect, what: &str) {
        assert!(part.vertices.len() < whole.vertices.len(), "{what}");
        let key = |m: &Mesh, t: &[u32]| {
            let mut k: Vec<(i64, i64)> = t
                .iter()
                .map(|&i| {
                    let p = m.vertices[i as usize].pos;
                    ((p.x * 64.0).round() as i64, (p.y * 64.0).round() as i64)
                })
                .collect();
            k.sort();
            k
        };
        let have: std::collections::HashSet<_> =
            part.indices.chunks(3).map(|t| key(part, t)).collect();
        let mut reached = 0;
        for t in whole.indices.chunks(3) {
            let mut b = Rect::NOTHING;
            for &i in t {
                b.extend_with(whole.vertices[i as usize].pos);
            }
            if b.intersects(near) {
                reached += 1;
                assert!(
                    have.contains(&key(whole, t)),
                    "{what}: a triangle is missing"
                );
            }
        }
        assert!(reached > 0, "{what}");
    }

    /// Asserts that `got` is `want` up to rounding (`scale` times larger).
    fn assert_same_mesh(got: &Mesh, want: &Mesh, scale: f32, what: &str) {
        assert_eq!(got.indices, want.indices, "{what}");
        assert_eq!(got.vertices.len(), want.vertices.len(), "{what}");
        let off = got
            .vertices
            .iter()
            .zip(&want.vertices)
            .map(|(a, b)| (a.pos - b.pos).length() / scale.max(1.0))
            .fold(0.0f32, f32::max);
        assert!(off < 2e-3, "{what}: {off}");
        assert!(
            got.vertices
                .iter()
                .zip(&want.vertices)
                .all(|(a, b)| a.color == b.color),
            "{what}"
        );
    }

    #[test]
    fn a_placed_stroke_is_a_fresh_stroke_at_any_pixel_density() {
        // Below about 0.36 pixels per point a curve's stroke is thin (three
        // vertices per point, epaint's rule below): moved, the kept mesh is
        // placed as it is; scaled, the polylines are tessellated again.
        let from = Pos2::new(20.0, 10.0);
        for ppp in [0.3f32, 0.35, 0.36, 1.5, 3.0] {
            let thin = crate::draw::CURVE_WIDTH <= 0.9 * (1.0 / ppp);
            assert_eq!(thin, ppp < 0.36, "{ppp}");
            // Three points with a sharp corner: thin, as many vertices as
            // four per point, but not laid out so.
            let corner = vec![
                from + vec2(0.0, 0.0),
                from + vec2(300.0, 10.0),
                from + vec2(0.0, 20.0),
            ];
            let (sharp, fours) = stroke(&[corner], ppp);
            assert!(!fours, "{ppp}");
            assert_eq!(sharp.vertices.len() == 12, thin, "{ppp}");
            let (mesh, fours) = stroke(&paths(from), ppp);
            assert_eq!(fours, !thin, "{ppp}");
            let runs = runs(&mesh);
            let kept = (&mesh, &*runs, fours);
            let in_area = paths(Pos2::ZERO);
            for (origin, scale) in [
                (Pos2::new(-317.25, 41.5), 1.0),
                (Pos2::new(311.0, -7.0), 1.0),
                (Pos2::new(-90.0, -60.0), 1.25),
                (Pos2::new(140.0, 95.0), 0.8),
            ] {
                let what = format!("{ppp} {origin:?} {scale}");
                let at = Placing {
                    from,
                    origin,
                    scale,
                };
                let placed: Vec<Vec<Pos2>> = in_area
                    .iter()
                    .map(|p| p.iter().map(|&q| origin + scale * q.to_vec2()).collect())
                    .collect();
                let (want, _) = stroke(&placed, ppp);
                // An area that holds the whole stroke.
                let area = Rect::from_min_size(origin - vec2(500.0, 500.0), vec2(5000.0, 5000.0));
                let mut tessellated = false;
                let got = placed_stroke(kept, &in_area, at, area, |pieces| {
                    tessellated = true;
                    stroke(&pieces, ppp).0
                });
                assert_eq!(tessellated, thin && scale != 1.0, "{what}");
                assert_same_mesh(&got, &want, scale, &what);

                // Only part of it near a small rectangle.
                let near = Rect::from_min_size(origin + vec2(150.0, 120.0), vec2(90.0, 70.0));
                if let Some(part) = placed_runs(&mesh, &runs, fours, at, near) {
                    assert_reaches(&got, &part, near, &what);
                }
            }
        }
    }

    #[test]
    fn a_placed_mesh_is_the_stroke_of_the_placed_polylines() {
        let from = Pos2::new(20.0, 10.0);
        for ppp in [1.0, 2.0] {
            // A sharp corner adds a point: such a mesh is not scaled.
            let corner = vec![
                from + vec2(0.0, 0.0),
                from + vec2(300.0, 10.0),
                from + vec2(0.0, 20.0),
            ];
            assert!(!stroke(&[corner], ppp).1);
            let (mesh, fours) = stroke(&paths(from), ppp);
            assert!(fours);
            let runs = runs(&mesh);
            for (origin, scale) in [
                (from, 1.0),
                (Pos2::new(-317.25, 41.5), 1.0),
                (Pos2::new(-90.0, -60.0), 1.25),
                (Pos2::new(140.0, 95.0), 0.8),
                (Pos2::new(-900.0, -700.0), 3.7),
            ] {
                let at = Placing {
                    from,
                    origin,
                    scale,
                };
                let placed: Vec<Vec<Pos2>> = paths(from)
                    .iter()
                    .map(|p| p.iter().map(|&q| at.point(q)).collect())
                    .collect();
                let (want, _) = stroke(&placed, ppp);
                let got = placed_runs(&mesh, &runs, fours, at, Rect::EVERYTHING).unwrap();
                assert_same_mesh(&got, &want, scale, &format!("{origin:?} {scale}"));

                // Only part of it near a small rectangle: every triangle
                // of the whole stroke that reaches it is there.
                let near = Rect::from_min_size(origin + vec2(150.0, 120.0), vec2(90.0, 70.0));
                let part = placed_runs(&mesh, &runs, fours, at, near).unwrap();
                assert_reaches(&got, &part, near, &format!("{origin:?} {scale}"));
            }
        }
    }
}
