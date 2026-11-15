//! Conics and function graphs on the canvas. Their polylines come from
//! `geo_kernel::plot` (shared with the SVG writer) and are tessellated
//! once into a mesh, kept per object until the object or the view changes:
//! an idle frame, or a drag of a point a curve does not depend on, only
//! compares a few numbers per curve and hands the painter the same mesh.
//!
//! What a curve is made from:
//! - a conic: its matrix (everything the drawing reads follows from it);
//! - a function: the values of every object it depends on, directly or
//!   not (`Construction::inputs`, followed once per rebuild).
//!
//! The view part of the key is the visible rectangle, the area's size and
//! position, the pixel density and the stroke colour.

use eframe::egui::epaint::{PathShape, Tessellator};
use eframe::egui::{vec2, Color32, Mesh, Painter, Pos2, Rect, Stroke};
use geo_kernel::kernel::{Construction, Id, Value};
use geo_kernel::plot::{self, Frame};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

thread_local! {
    static TESSELLATED: Cell<u64> = const { Cell::new(0) };
}

/// Curves this thread has plotted and tessellated so far. A frame in which
/// no curve and not the view changed adds nothing; the frame budget test
/// and the canvas tests read it.
#[cfg(test)]
pub fn curves_tessellated() -> u64 {
    TESSELLATED.with(Cell::get)
}

/// One curve as drawn: its tessellated stroke, its polylines in screen
/// points (for the highlight), the single point of a conic that is one
/// point, and its label anchor, all in the area's coordinates plus its
/// origin where noted.
pub struct Drawn {
    /// The stroke, in screen points.
    pub mesh: Arc<Mesh>,
    /// The polylines, in screen points.
    pub paths: Vec<Vec<Pos2>>,
    /// Relative to the area.
    pub point: Option<(f64, f64)>,
    /// Relative to the area.
    pub label: Option<(f64, f64)>,
}

/// What a curve's drawing depends on.
#[derive(Clone, PartialEq)]
struct Key {
    view: [u64; 9],
    color: Color32,
    data: Data,
}

#[derive(Clone, PartialEq)]
enum Data {
    /// The bits of a conic's matrix and its type.
    Conic([u64; 6], u8),
    /// The bits of the values a function depends on.
    Function(Vec<u64>),
}

#[derive(Default)]
struct Cache {
    generation: Option<u64>,
    entries: HashMap<Id, (Key, Arc<Drawn>)>,
    /// What each function depends on, directly or not, for this
    /// generation.
    closures: HashMap<Id, Vec<Id>>,
}

/// The curves of one frame: the shared cache and the view they are drawn
/// in.
pub struct Curves {
    cache: Arc<Mutex<Cache>>,
    frame: Frame,
    origin: Pos2,
    view: [u64; 9],
    pixels_per_point: f32,
    options: eframe::egui::epaint::TessellationOptions,
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
        let ctx = painter.ctx();
        let cache = ctx.data_mut(|d| {
            d.get_temp_mut_or_default::<Arc<Mutex<Cache>>>(eframe::egui::Id::unique("geo_curves"))
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
        let v = t.view();
        let pixels_per_point = ctx.pixels_per_point();
        let view = [
            v.xmin.to_bits(),
            v.ymin.to_bits(),
            v.xmax.to_bits(),
            v.ymax.to_bits(),
            w.to_bits(),
            h.to_bits(),
            rect.min.x.to_bits() as u64,
            rect.min.y.to_bits() as u64,
            pixels_per_point.to_bits() as u64,
        ];
        Curves {
            cache,
            frame: Frame::fit(v, w, h),
            origin: rect.min,
            view,
            pixels_per_point,
            options: ctx.tessellation_options(|o| *o),
        }
    }

    /// The drawing of the conic or function `id` in `color`; `None` for
    /// any other object.
    pub fn get(&self, c: &Construction, id: Id, color: Color32) -> Option<Arc<Drawn>> {
        let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        // The fields are borrowed apart from each other.
        let cache = &mut *guard;
        let data = match c.value(id) {
            Value::Conic { .. } => {
                let d = c.conic(id)?;
                Data::Conic(d.matrix.map(f64::to_bits), d.ty as u8)
            }
            Value::Function => {
                let closure: &[Id] = cache.closures.entry(id).or_insert_with(|| closure(c, id));
                // Compare with what the cached drawing was made from without
                // allocating; make the new list only when it differs.
                if let Some((key, drawn)) = cache.entries.get(&id) {
                    if key.view == self.view && key.color == color {
                        if let Data::Function(bits) = &key.data {
                            if same_bits(c, closure, bits) {
                                return Some(drawn.clone());
                            }
                        }
                    }
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
            view: self.view,
            color,
            data,
        };
        if let Some((old, drawn)) = cache.entries.get(&id) {
            if *old == key {
                return Some(drawn.clone());
            }
        }
        let drawn = Arc::new(self.make(c, id, color));
        cache.entries.insert(id, (key, drawn.clone()));
        Some(drawn)
    }

    fn make(&self, c: &Construction, id: Id, color: Color32) -> Drawn {
        TESSELLATED.with(|n| n.set(n.get() + 1));
        let (paths, point, label) = match c.value(id) {
            Value::Conic { .. } => match c.conic(id) {
                Some(d) => {
                    let p = plot::conic::plot(&d, &self.frame);
                    (p.paths, p.point, p.label)
                }
                None => (Vec::new(), None, None),
            },
            _ => {
                let mut f = |x: f64| c.function_value(id, x).unwrap_or(f64::NAN);
                let p = plot::function::plot(&mut f, &self.frame);
                (p.paths, None, p.label)
            }
        };
        let stroke = Stroke::new(crate::draw::CURVE_WIDTH, color);
        let mut tess = Tessellator::new(self.pixels_per_point, self.options, [1, 1], Vec::new());
        let mut mesh = Mesh::default();
        let mut screen = Vec::with_capacity(paths.len());
        for path in paths {
            let points: Vec<Pos2> = path
                .iter()
                .map(|&(x, y)| self.origin + vec2(x as f32, y as f32))
                .collect();
            let shape = PathShape::line(points, stroke);
            tess.tessellate_path(&shape, &mut mesh);
            screen.push(shape.points);
        }
        Drawn {
            mesh: Arc::new(mesh),
            paths: screen,
            point,
            label,
        }
    }
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
    let mut all = |tag: u64, values: &[f64]| {
        out(tag);
        for v in values {
            out(v.to_bits());
        }
    };
    match c.value(id) {
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
        Value::Conic { ty } => match c.conic(id) {
            Some(d) => all(11 + ((ty as u64) << 8), &d.matrix),
            None => all(11, &[]),
        },
        Value::Function => all(12, &[]),
        Value::Undefined => all(13, &[]),
        #[allow(unreachable_patterns)]
        _ => all(14, &[FRESH.fetch_add(1, Ordering::Relaxed) as f64]),
    }
}
