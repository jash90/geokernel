//! How objects look: their colour, the colours of their algebra marker and
//! the geometry of an angle's sector. No drawing calls.

use crate::geom::Rgba;
use crate::palette;
use geo_kernel::algo::measure::MeasureDef;
use geo_kernel::algo::polygon::PolygonDef;
use geo_kernel::kernel::{Construction, Def, Id, Kind, Value};

/// The drawn radius of a free point, in points (point size 5).
pub const FREE_POINT_RADIUS: f32 = 5.0;

/// How strongly the inside of a polygon and the sector of an angle are
/// tinted with the object's colour. In double precision, so that a file
/// gets the decimal `0.1` and not a widened 32-bit value.
pub const FILL_OPACITY: f64 = 0.1;

/// The opacity of the fill the canvas draws for an object, 0 for an outline.
pub fn fill_opacity(c: &Construction, id: Id) -> f64 {
    match c.kind(id) {
        Kind::Polygon | Kind::Angle => FILL_OPACITY,
        _ => 0.0,
    }
}

/// The default colour of an object: blue free points, grey dependent
/// points, green angles, brown polygons and their sides, near-black lines
/// and circles.
///
/// For one object. It looks through every polygon for a segment, so a
/// loop over all objects takes the colours from a [`Palette`] instead.
pub fn color(c: &Construction, id: Id) -> Rgba {
    color_with(c, id, |a, b| is_side(c, a, b))
}

fn color_with(c: &Construction, id: Id, side: impl Fn(Id, Id) -> bool) -> Rgba {
    let def = c.def(id);
    if def.kind() == Kind::Angle {
        return palette::ANGLE;
    }
    match (c.value(id), def) {
        (Value::Point { .. }, def) if def.is_free() => palette::FREE_POINT,
        (Value::Point { .. }, _) => palette::DEPENDENT_POINT,
        (Value::Polygon { .. }, _) => palette::POLYGON,
        (_, Def::Polygon(PolygonDef::RegularSide { .. })) => palette::POLYGON,
        (_, Def::Segment(a, b)) if side(a, b) => palette::POLYGON,
        _ => palette::CURVE,
    }
}

thread_local! {
    static SCANNED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Objects this thread has looked through for the sides of polygons so far
/// ([`color`] once per call, [`Sides::of`] once per cache). A frame that
/// takes its colours from a fresh [`Palette`] adds nothing; the frame
/// budget test reads it.
pub fn objects_scanned() -> u64 {
    SCANNED.with(std::cell::Cell::get)
}

fn scanned(n: usize) {
    SCANNED.with(|s| s.set(s.get() + n as u64));
}

/// Whether the segment from `a` to `b` is a side of a `Polygon(...)`.
fn is_side(c: &Construction, a: Id, b: Id) -> bool {
    scanned(c.len());
    c.ids().any(|p| polygon_sides(c, p).any(|s| s == (a, b)))
}

/// The sides `(from, to)` of `p` when it is a `Polygon(...)` of three or
/// more vertices; nothing otherwise.
fn polygon_sides(c: &Construction, p: Id) -> impl Iterator<Item = (Id, Id)> + '_ {
    let v: &[Id] = match c.def(p) {
        Def::Polygon(PolygonDef::Polygon(list)) if c.list(list).len() >= 3 => c.list(list),
        _ => &[],
    };
    (0..v.len()).map(move |i| (v[i], v[(i + 1) % v.len()]))
}

/// The state of a document a [`Sides`] was made for: the workbench's count
/// of rebuilt or replaced constructions, and the number of objects.
/// Definitions only change with one of the two, so the sides of the same
/// key are the sides of the construction.
pub type SidesKey = (u64, usize);

/// Every side `(from, to)` of every `Polygon(...)` of a construction, for
/// a [`Palette`]. Depends only on the definitions, so it stays valid until
/// objects are added or the construction is replaced; moves keep it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sides {
    pairs: std::collections::HashSet<(Id, Id)>,
    key: SidesKey,
}

impl Sides {
    /// Made in one pass over the objects and the polygons' vertex lists,
    /// for the document state `key`.
    pub fn of(c: &Construction, key: SidesKey) -> Sides {
        scanned(c.len());
        Sides {
            pairs: c.ids().flat_map(|p| polygon_sides(c, p)).collect(),
            key,
        }
    }

    pub fn contains(&self, a: Id, b: Id) -> bool {
        self.pairs.contains(&(a, b))
    }

    pub fn key(&self) -> SidesKey {
        self.key
    }
}

/// The colours of the objects of a construction (`Workbench::palette`):
/// constant time per object from cached [`Sides`], which are used only
/// when made for the document state the palette is for. A stale cache is
/// never read, in release builds too: the colours then come from the scan
/// of [`color`] (and debug builds stop, as that is a bug).
#[derive(Clone, Copy)]
pub struct Palette<'a> {
    c: &'a Construction,
    sides: Option<&'a Sides>,
}

impl<'a> Palette<'a> {
    pub fn new(c: &'a Construction, sides: &'a Sides, key: SidesKey) -> Palette<'a> {
        let fresh = sides.key == key && key.1 == c.len();
        debug_assert!(fresh, "polygon sides of {:?} read at {key:?}", sides.key);
        Palette {
            c,
            sides: fresh.then_some(sides),
        }
    }

    /// The colour of `id`, as [`color`] gives it.
    pub fn color(&self, id: Id) -> Rgba {
        match self.sides {
            Some(s) => color_with(self.c, id, |a, b| s.contains(a, b)),
            None => color(self.c, id),
        }
    }

    /// Whether the colours come from the cache.
    pub fn cached(&self) -> bool {
        self.sides.is_some()
    }
}

/// `color` laid over white with opacity `alpha`.
pub fn over_white(color: Rgba, alpha: f32) -> Rgba {
    let mix = |v: u8| (255.0 - (255.0 - v as f32) * alpha).round() as u8;
    Rgba::from_rgb(mix(color.r), mix(color.g), mix(color.b))
}

/// The marker of an object's row in the algebra view: a light fill and a
/// darker ring of the object's colour.
pub fn marker(c: &Construction, id: Id) -> (Rgba, Rgba) {
    marker_of(color(c, id))
}

/// [`marker`] for an object of colour `base`.
pub fn marker_of(base: Rgba) -> (Rgba, Rgba) {
    (over_white(base, 0.35), over_white(base, 0.75))
}

/// The three points `A`, `B` (the vertex), `C` of an angle `Angle(A, B, C)`.
pub fn angle_points(c: &Construction, id: Id) -> Option<[(f64, f64); 3]> {
    let Def::Measure(MeasureDef::Angle(a, b, v)) = c.def(id) else {
        return None;
    };
    let p = |id: Id| match c.value(id) {
        Value::Point { x, y } => Some((x, y)),
        _ => None,
    };
    Some([p(a)?, p(b)?, p(v)?])
}

/// The sector of the angle at `b` from the arm `ba` counter-clockwise to `bc`
/// (world coordinates, as the kernel measures it), as a start direction and
/// a sweep in radians on the screen, where y points down.
pub fn sector(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<(f64, f64)> {
    let (ux, uy) = (a.0 - b.0, a.1 - b.1);
    let (vx, vy) = (c.0 - b.0, c.1 - b.1);
    if ux.hypot(uy) == 0.0 || vx.hypot(vy) == 0.0 {
        return None;
    }
    // World y points up; screen y points down, so world counter-clockwise is
    // a negative screen angle.
    let start = (-uy).atan2(ux);
    let mut sweep = (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    if sweep < 0.0 {
        sweep += std::f64::consts::TAU;
    }
    Some((start, -sweep))
}

/// Whether an angle in radians is a right angle (drawn as a square).
pub fn is_right(angle: f64) -> bool {
    (angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI};

    #[test]
    fn markers_are_light_fills_of_the_object_colour() {
        let c = geo_kernel::script::load("A = (0, 0)\nB = (1, 0)\nM = Midpoint(A, B)\n").unwrap();
        let a = c.id("A").unwrap();
        let m = c.id("M").unwrap();
        assert_eq!(color(&c, a), palette::FREE_POINT);
        assert_eq!(color(&c, m), palette::DEPENDENT_POINT);
        let (fill, ring) = marker(&c, a);
        assert!(fill.r > 180 && fill.b > 250, "{fill:?}");
        assert!(ring.r < fill.r);
        assert_eq!(over_white(Rgba::BLACK, 0.0), Rgba::WHITE);
    }

    #[test]
    fn sectors_run_from_the_first_arm_to_the_second() {
        // A right angle at the origin from the x axis to the y axis.
        let (start, sweep) = sector((1.0, 0.0), (0.0, 0.0), (0.0, 1.0)).unwrap();
        assert!(start.abs() < 1e-12);
        assert!((sweep + FRAC_PI_2).abs() < 1e-12);
        // The other way round is the reflex angle.
        let (start, sweep) = sector((0.0, 1.0), (0.0, 0.0), (1.0, 0.0)).unwrap();
        assert!((start - FRAC_PI_2).abs() < 1e-12 || (start + FRAC_PI_2).abs() < 1e-12);
        assert!((sweep + 1.5 * PI).abs() < 1e-12);
        assert!(sector((0.0, 0.0), (0.0, 0.0), (1.0, 0.0)).is_none());
        assert!(is_right(FRAC_PI_2) && !is_right(1.5));
    }

    #[test]
    fn angle_points_come_from_the_definition() {
        let c = geo_kernel::script::load(
            "A = (1, 0)\nB = (0, 0)\nC = (0, 1)\nα = Angle(A, B, C)\nd = Distance(A, C)\n",
        )
        .unwrap();
        let alpha = c.id("α").unwrap();
        assert_eq!(
            angle_points(&c, alpha),
            Some([(1.0, 0.0), (0.0, 0.0), (0.0, 1.0)])
        );
        assert_eq!(color(&c, alpha), palette::ANGLE);
        assert_eq!(angle_points(&c, c.id("d").unwrap()), None);
    }

    #[test]
    fn polygons_and_their_sides_are_brown_other_segments_are_not() {
        let c = geo_kernel::script::load(
            "A = (0, 0)\nB = (4, 0)\nC = (1, 3)\nq = Polygon(A, B, C)\nD = (5, 5)\ns = Segment(C, D)\n\
             n = 4\nr = Polygon(A, B, n)\n",
        )
        .unwrap();
        let brown: Vec<&str> = c
            .ids()
            .filter(|&id| color(&c, id) == palette::POLYGON)
            .map(|id| c.label(id))
            .collect();
        assert!(brown.contains(&"q") && brown.contains(&"r"), "{brown:?}");
        // The three sides of q and the four of r.
        assert_eq!(brown.len(), 2 + 3 + 4, "{brown:?}");
        assert_eq!(color(&c, c.id("s").unwrap()), palette::CURVE);
    }
}
