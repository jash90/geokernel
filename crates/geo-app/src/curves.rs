//! Conics and function graphs on the canvas. Their polylines come from
//! `geo_kernel::plot` (shared with the SVG writer) and are tessellated
//! once into a mesh, kept per object:
//!
//! - an idle frame, or a drag of a point a curve does not depend on, only
//!   compares a few numbers per curve and hands the painter the same mesh;
//! - a pan (the same scale) moves the kept mesh: a curve is plotted with a
//!   margin of [`margin`] pixels around the area, so a pan of less than
//!   that shows nothing unplotted, and a curve is plotted again only once
//!   the pan passes half the margin (or, at once, the whole margin);
//! - a zoom plots again; at most [`REPLOTS_PER_FRAME`] curves per frame,
//!   the others are drawn from their kept polylines moved and scaled to
//!   the new view, and the next frames are asked for until all are fresh;
//! - a curve whose kept drawing lies outside the area costs only that test.
//!
//! What a curve is made from (the key of its drawing):
//! - a conic: its matrix and type, and the order of its two lines when it
//!   is a pair of lines (that order follows the conic's history and decides
//!   where the label goes);
//! - a function: the values of every object it depends on, directly or
//!   not (`Construction::reads`, followed once per rebuild), the elements
//!   of the lists among them included;
//! - both: the area's size, the pixel density and the stroke colour.

use eframe::egui::epaint::{PathShape, Tessellator};
use eframe::egui::{vec2, Color32, Context, Mesh, Painter, Pos2, Rect, Stroke, Vec2};
use geo_kernel::expr::Element;
use geo_kernel::kernel::{Construction, Id, Value};
use geo_kernel::plot::{self, Frame};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The most curves plotted again in one frame because the view moved; an
/// edited curve is always plotted at once.
pub const REPLOTS_PER_FRAME: usize = 24;

/// How far around the area, in points, a curve is plotted: a quarter of the
/// longer side.
pub fn margin(width: f64, height: f64) -> f64 {
    (0.25 * width.max(height)).ceil()
}

thread_local! {
    static PLOTTED: Cell<u64> = const { Cell::new(0) };
    static TESSELLATED: Cell<u64> = const { Cell::new(0) };
}

/// Curves this thread has plotted so far.
#[cfg(test)]
pub fn curves_plotted() -> u64 {
    PLOTTED.with(Cell::get)
}

/// Curves this thread has tessellated so far: each plot, and each frame a
/// curve waiting to be plotted again is drawn from its moved and scaled
/// polylines. A frame in which no curve and not the scale changed adds
/// nothing; the frame budget test and the canvas tests read it.
#[cfg(test)]
pub fn curves_tessellated() -> u64 {
    TESSELLATED.with(Cell::get)
}

/// Drops every kept drawing of `ctx`: the next frame plots afresh.
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
    /// The polylines, plotted `margin` points around the area.
    paths: Vec<Vec<Pos2>>,
    mesh: Mesh,
    point: Option<Pos2>,
    dot: Option<Pos2>,
    /// A conic's label anchor; a function's label is found per frame.
    label: Option<(f64, f64)>,
    function: bool,
    /// Around everything above; `Rect::NOTHING` when nothing is drawn.
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

/// What a curve's drawing depends on, apart from the view.
#[derive(Clone, PartialEq)]
struct Key {
    /// The area's width and height and the pixel density.
    area: [u64; 3],
    color: Color32,
    data: Data,
}

#[derive(Clone, PartialEq)]
enum Data {
    /// The bits of a conic's matrix, its type and its two lines.
    Conic([u64; 6], u8, [u64; 6]),
    /// The bits of the values a function depends on.
    Function(Vec<u64>),
}

struct Entry {
    key: Key,
    plotted: Arc<Plotted>,
    /// The last drawing handed out (`None`: nothing of it in the area),
    /// with its placement.
    shown: Option<(Pos2, f32, Option<Arc<Drawn>>)>,
}

#[derive(Default)]
struct Cache {
    generation: Option<u64>,
    entries: HashMap<Id, Entry>,
    /// What each function depends on, directly or not, for this
    /// generation.
    closures: HashMap<Id, Vec<Id>>,
}

/// How a kept plot relates to the current view.
enum Fit {
    /// The same scale, moved by this much, every visible point plotted.
    Covered(Vec2, bool),
    /// Moved and scaled (`origin + scale * p`); parts may be missing.
    Stale(Pos2, f32),
}

/// The curves of one frame: the shared cache and the view they are drawn
/// in.
pub struct Curves {
    cache: Arc<Mutex<Cache>>,
    ctx: Context,
    frame: Frame,
    origin: Pos2,
    area: [u64; 3],
    pixels_per_point: f32,
    options: eframe::egui::epaint::TessellationOptions,
    replots: Cell<usize>,
}

impl Curves {
    /// The curves of a frame drawing `rect` with `t`. `generation` counts
    /// the rebuilds of the construction: ids and definitions stay the same
    /// while it does.
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
        {
            let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
            if guard.generation != Some(generation) {
                guard.entries.clear();
                guard.closures.clear();
                guard.generation = Some(generation);
            }
        }
        let (w, h) = (rect.width() as f64, rect.height() as f64);
        let pixels_per_point = ctx.pixels_per_point();
        let options = ctx.tessellation_options(|o| *o);
        Curves {
            cache,
            ctx,
            frame: Frame::fit(t.view(), w, h),
            origin: rect.min,
            area: [w.to_bits(), h.to_bits(), pixels_per_point.to_bits() as u64],
            pixels_per_point,
            options,
            replots: Cell::new(0),
        }
    }

    /// The drawing of the conic or function `id` in `color`; `None` for
    /// any other object, and for a curve whose drawing lies outside the
    /// area.
    pub fn get(&self, c: &Construction, id: Id, color: Color32) -> Option<Arc<Drawn>> {
        let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        // The fields are borrowed apart from each other.
        let cache = &mut *guard;
        let data = match c.value(id) {
            Value::Conic { .. } => {
                let d = c.conic(id)?;
                let [l0, l1] = d.lines;
                let lines = [l0[0], l0[1], l0[2], l1[0], l1[1], l1[2]];
                Data::Conic(
                    d.matrix.map(f64::to_bits),
                    d.ty as u8,
                    lines.map(f64::to_bits),
                )
            }
            Value::Function => {
                let closure: &[Id] = cache.closures.entry(id).or_insert_with(|| closure(c, id));
                // Compare with what the kept drawing was made from without
                // allocating; make the new list only when it differs.
                let kept = cache.entries.get(&id).is_some_and(|e| {
                    e.key.area == self.area
                        && e.key.color == color
                        && matches!(&e.key.data, Data::Function(bits) if same_bits(c, closure, bits))
                });
                if kept {
                    let entry = cache.entries.get_mut(&id)?;
                    return self.show(c, id, entry);
                }
                let mut bits = Vec::new();
                for &a in closure {
                    feed(c, a, &mut |b| bits.push(b));
                }
                Data::Function(bits)
            }
            _ => return None,
        };
        let key = Key {
            area: self.area,
            color,
            data,
        };
        match cache.entries.get_mut(&id) {
            Some(entry) if entry.key == key => self.show(c, id, entry),
            _ => {
                let plotted = Arc::new(self.plot(c, id, color));
                let entry = cache.entries.entry(id).or_insert(Entry {
                    key: key.clone(),
                    plotted: plotted.clone(),
                    shown: None,
                });
                *entry = Entry {
                    key,
                    plotted,
                    shown: None,
                };
                self.show(c, id, entry)
            }
        }
    }

    /// The drawing of a kept plot in this view, plotting it again when the
    /// view has left it and the frame still may.
    fn show(&self, c: &Construction, id: Id, entry: &mut Entry) -> Option<Arc<Drawn>> {
        let mut fit = self.fit(&entry.plotted);
        let wants = match fit {
            Fit::Covered(_, due) => due,
            Fit::Stale(..) => true,
        };
        if wants && self.replots.get() < REPLOTS_PER_FRAME {
            self.replots.set(self.replots.get() + 1);
            entry.plotted = Arc::new(self.plot(c, id, entry.key.color));
            entry.shown = None;
            fit = self.fit(&entry.plotted);
        }
        let (origin, scale) = match fit {
            Fit::Covered(d, _) => (self.origin + d, 1.0),
            Fit::Stale(o, k) => {
                // Drawn from the kept polylines this frame; asked again.
                self.ctx.request_repaint();
                if k == 0.0 {
                    return None;
                }
                (self.origin + o.to_vec2(), k)
            }
        };
        let p = &entry.plotted;
        // The kept drawing, where it goes now, against the area (a stroke
        // and a label beside it may reach a little beyond).
        let area = Rect::from_min_size(
            self.origin,
            vec2(self.frame.width() as f32, self.frame.height() as f32),
        )
        .expand(24.0);
        let bounds = Rect::from_two_pos(
            origin + scale * p.bounds.min.to_vec2(),
            origin + scale * p.bounds.max.to_vec2(),
        );
        if p.bounds == Rect::NOTHING || !bounds.intersects(area) {
            return None;
        }
        if let Some((o, k, drawn)) = &entry.shown {
            if *o == origin && *k == scale {
                return drawn.clone();
            }
        }
        // A curve plotted with a margin may have a box over the area and no
        // stroke in it: it draws nothing (as a fresh plot of the area would,
        // and as the cull before `get` decides).
        let place = |q: Pos2| origin + scale * q.to_vec2();
        let near = area.shrink(24.0 - plot::PAD as f32);
        let meets = p.paths.iter().any(|path| {
            path.windows(2)
                .any(|w| segment_meets(place(w[0]), place(w[1]), near))
        }) || p
            .point
            .iter()
            .chain(&p.dot)
            .any(|&q| near.contains(place(q)));
        if !meets {
            entry.shown = Some((origin, scale, None));
            return None;
        }
        let mesh = if scale == 1.0 {
            let mut m = p.mesh.clone();
            m.translate(origin.to_vec2());
            m
        } else {
            let points = p.paths.iter().map(|path| {
                path.iter()
                    .map(|&q| origin + scale * q.to_vec2())
                    .collect::<Vec<_>>()
            });
            self.tessellate(points, entry.key.color)
        };
        let mut drawn = Drawn {
            mesh: Arc::new(mesh),
            plotted: p.clone(),
            origin,
            scale,
            point: p.point.map(place),
            dot: p.dot.map(place),
            label: p.label.map(|(x, y)| {
                let q = place(Pos2::new(x as f32, y as f32)) - self.origin;
                (q.x as f64, q.y as f64)
            }),
        };
        if p.function {
            drawn.label = self.function_label(&drawn);
        }
        let drawn = Arc::new(drawn);
        entry.shown = Some((origin, scale, Some(drawn.clone())));
        Some(drawn)
    }

    /// Where a function's label goes: its first drawn point in the area
    /// (`CurvePlotter` places it at the first sample on screen).
    fn function_label(&self, drawn: &Drawn) -> Option<(f64, f64)> {
        let (w, h) = (self.frame.width(), self.frame.height());
        drawn.plotted.paths.iter().flatten().find_map(|&q| {
            let s = drawn.place(q) - self.origin;
            let (x, y) = (s.x as f64, s.y as f64);
            ((0.0..=w).contains(&x) && (0.0..=h).contains(&y)).then(|| (x.trunc(), y.trunc()))
        })
    }

    fn fit(&self, p: &Plotted) -> Fit {
        match p.frame.shift_to(&self.frame) {
            Some((dx, dy)) => {
                let far = dx.abs().max(dy.abs());
                if far <= p.margin {
                    Fit::Covered(vec2(dx as f32, dy as f32), far > 0.5 * p.margin)
                } else {
                    Fit::Stale(Pos2::new(dx as f32, dy as f32), 1.0)
                }
            }
            None => {
                let k = self.frame.scale() / p.frame.scale();
                let (wx, wy) = p.frame.screen_to_world(0.0, 0.0);
                let (ox, oy) = self.frame.world_to_screen(wx, wy);
                if k.is_finite() && ox.is_finite() && oy.is_finite() {
                    Fit::Stale(Pos2::new(ox as f32, oy as f32), k as f32)
                } else {
                    Fit::Stale(Pos2::ZERO, 0.0)
                }
            }
        }
    }

    fn plot(&self, c: &Construction, id: Id, color: Color32) -> Plotted {
        count(&PLOTTED);
        let margin = margin(self.frame.width(), self.frame.height());
        let wide = self.frame.grown(margin);
        let back = |(x, y): (f64, f64)| Pos2::new((x - margin) as f32, (y - margin) as f32);
        let (paths, point, dot, label, function) = match c.value(id) {
            Value::Conic { .. } => match c.conic(id) {
                Some(d) => {
                    let p = plot::conic::plot(&d, &wide);
                    let label = p.label.map(|(x, y)| (x - margin, y - margin));
                    (p.paths, p.point.map(back), p.dot.map(back), label, false)
                }
                None => (Vec::new(), None, None, None, false),
            },
            _ => {
                let mut f = |x: f64| c.function_value(id, x).unwrap_or(f64::NAN);
                let p = plot::function::plot(&mut f, &wide);
                (p.paths, None, None, None, true)
            }
        };
        let paths: Vec<Vec<Pos2>> = paths
            .into_iter()
            .map(|path| path.into_iter().map(back).collect())
            .collect();
        let mut bounds = Rect::NOTHING;
        for &q in paths.iter().flatten().chain(&point).chain(&dot) {
            bounds.extend_with(q);
        }
        if let Some((x, y)) = label {
            if bounds != Rect::NOTHING {
                bounds.extend_with(Pos2::new(x as f32, y as f32));
            }
        }
        let mesh = self.tessellate(paths.iter().cloned(), color);
        Plotted {
            frame: self.frame,
            paths,
            mesh,
            point,
            dot,
            label,
            function,
            bounds,
            margin,
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

/// Every object `id` depends on, directly or not, each once, in a fixed
/// order.
fn closure(c: &Construction, id: Id) -> Vec<Id> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let mut stack = c.reads(id);
    while let Some(a) = stack.pop() {
        if a != id && seen.insert(a) {
            out.push(a);
            stack.extend(c.reads(a));
        }
    }
    out
}

/// Whether the values of `ids` give `bits`.
fn same_bits(c: &Construction, ids: &[Id], bits: &[u64]) -> bool {
    let mut i = 0;
    let mut same = true;
    for &a in ids {
        feed(c, a, &mut |b| {
            same &= bits.get(i) == Some(&b);
            i += 1;
        });
        if !same {
            return false;
        }
    }
    i == bits.len()
}

/// A value this file does not know is never the same as before.
static FRESH: AtomicU64 = AtomicU64::new(0);

/// Hands the bits of the value of `id` to `out`, a tag first.
fn feed(c: &Construction, id: Id, out: &mut dyn FnMut(u64)) {
    match c.value(id) {
        Value::Conic { ty } => match c.conic(id) {
            Some(d) => {
                out(11 + ((ty as u64) << 8));
                d.matrix.iter().for_each(|v| out(v.to_bits()));
            }
            None => out(11),
        },
        Value::List { len } => match c.elements(id) {
            Some(list) => {
                out(15 + ((len as u64) << 8));
                feed_list(list, out);
            }
            None => out(15),
        },
        v => feed_value(&v, out),
    }
}

/// The elements of a list, nested lists in brackets of tags.
fn feed_list(list: geo_kernel::expr::List<'_>, out: &mut dyn FnMut(u64)) {
    for e in list.iter() {
        match e {
            Element::Value(_, v) => feed_value(&v, out),
            Element::List(inner) => {
                out(16 + ((inner.len() as u64) << 8));
                feed_list(inner, out);
                out(17);
            }
        }
    }
}

/// The bits of a value that carries its numbers.
fn feed_value(v: &Value, out: &mut dyn FnMut(u64)) {
    let mut all = |tag: u64, values: &[f64]| {
        out(tag);
        for v in values {
            out(v.to_bits());
        }
    };
    match *v {
        Value::Point { x, y } => all(1, &[x, y]),
        Value::Vector { x, y } => all(2, &[x, y]),
        Value::Line { a, b, c } => all(3, &[a, b, c]),
        Value::Ray { x, y, a, b, c } => all(4, &[x, y, a, b, c]),
        Value::Segment { x1, y1, x2, y2 } => all(5, &[x1, y1, x2, y2]),
        Value::Circle { cx, cy, r } => all(6, &[cx, cy, r]),
        Value::Number(n) => all(7, &[n]),
        Value::Bool(b) => all(8, &[b as u8 as f64]),
        Value::Polygon { area } => all(9, &[area]),
        Value::PolyLine { length } => all(10, &[length]),
        Value::Function => all(12, &[]),
        Value::Undefined => all(13, &[]),
        // Read with the construction (`feed`); never the same here.
        _ => all(14, &[FRESH.fetch_add(1, Ordering::Relaxed) as f64]),
    }
}
