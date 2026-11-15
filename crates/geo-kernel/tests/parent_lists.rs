//! Definitions with a stored list of parents (`Construction::add_with_list`):
//! a construction can be rebuilt from its definitions, and incremental
//! updates give the same values as building everything again.

use geo_kernel::{script, Construction, Def, Value};

/// Builds a fresh construction from the definitions of `orig`. A definition
/// with a list stores the same objects again; because lists are stored in
/// creation order, the handles come out equal.
fn rebuild(orig: &Construction) -> Construction {
    let mut fresh = Construction::new();
    for id in orig.ids() {
        let def = orig.def(id);
        let label = orig.label(id);
        let new = match def.list() {
            Some(list) => fresh.add_with_list(label, orig.list(list), |l| def.with_list(l)),
            None => fresh.add(label, def),
        };
        new.unwrap_or_else(|e| panic!("rebuilding `{label}` failed: {e}"));
    }
    fresh
}

fn values(c: &Construction) -> Vec<Value> {
    c.ids().map(|id| c.value(id)).collect()
}

fn defs(c: &Construction) -> Vec<Def> {
    c.ids().map(|id| c.def(id)).collect()
}

/// A small deterministic generator (xorshift), so the test needs no crates.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn coord(&mut self) -> f64 {
        // multiples of 1/4 in [-8, 8], with repeats, so that points coincide
        (self.next() % 65) as f64 / 4.0 - 8.0
    }
}

const SCRIPT: &str = "A = (0, 0)\nB = (4, 0)\nC = (4, 3)\nD = (2, 5)\nE = (0, 3)\n\
                      F = Midpoint(A, C)\n\
                      a = Area(A, B, C, D, E)\n\
                      b = Area(E, D, C, B, A, F)\n\
                      c = Area(A, A, B, B, C, C)\n";

/// Polygons read the vertices of their parents; the measures of a polygon
/// read the polygon's own parents too, so an equal-area move reaches them.
const POLYGONS: &str = "A = (0, 0)\nB = (4, 0)\nC = (4, 3)\nD = (2, 5)\nE = (0, 3)\nn = 5\n\
                        p = Polygon(A, B, C, D, E)\nt = Polygon(A, C, E)\n\
                        r = Polygon(D, E, n)\npl = PolyLine(E, D, C, B)\n\
                        ar = Area(p)\npe = Perimeter(p)\nM = Centroid(t)\nK = Centroid(r)\n\
                        W = Vertex(r, n)\nV = Vertex(t)\nw = InteriorAngles(t)\nu = Angle(p)\n\
                        q = Polygon(A, M, K)\npq = Perimeter(q)\n";

#[test]
fn constructions_with_lists_rebuild_from_their_definitions() {
    let orig = script::load(SCRIPT).unwrap();
    let fresh = rebuild(&orig);
    assert_eq!(defs(&fresh), defs(&orig));
    assert_eq!(values(&fresh), values(&orig));
    for id in orig.ids() {
        if let Some(list) = orig.def(id).list() {
            assert_eq!(fresh.list(list), orig.list(list));
        }
    }
}

#[test]
fn incremental_updates_match_a_full_rebuild() {
    let mut c = script::load(SCRIPT).unwrap();
    let free: Vec<_> = c.ids().filter(|&id| c.def(id).is_free()).collect();
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for step in 0..400 {
        let id = free[(rng.next() % free.len() as u64) as usize];
        let (x, y) = (rng.coord(), rng.coord());
        c.move_point(id, x, y).unwrap();
        let fresh = rebuild(&c);
        assert_eq!(
            values(&c),
            values(&fresh),
            "step {step}: moving {} to ({x}, {y})",
            c.label(id)
        );
    }
}

#[test]
fn polygons_rebuild_and_update_like_a_full_rebuild() {
    let mut c = script::load(POLYGONS).unwrap();
    assert_eq!(values(&rebuild(&c)), values(&c));
    let free: Vec<_> = c.ids().filter(|&id| c.def(id).is_free()).collect();
    let n = c.id("n").unwrap();
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    for step in 0..400 {
        let id = free[(rng.next() % free.len() as u64) as usize];
        if step % 3 == 0 {
            // Moves E parallel to AC: the area of `t` stays exactly the
            // same (all coordinates are multiples of 1/4), its centroid,
            // vertices and angles do not.
            let point = |label: &str| match c.value(c.id(label).unwrap()) {
                Value::Point { x, y } => (x, y),
                other => panic!("{label} is {other:?}"),
            };
            let ((ax, ay), (cx, cy), (ex, ey)) = (point("A"), point("C"), point("E"));
            let k = (rng.next() % 5) as f64 - 2.0;
            let e = c.id("E").unwrap();
            c.move_point(e, ex + k * (cx - ax), ey + k * (cy - ay))
                .unwrap();
        } else if id == n {
            let v = (rng.next() % 13) as f64 / 2.0;
            c.set_number(n, v).unwrap();
        } else {
            let (x, y) = (rng.coord(), rng.coord());
            c.move_point(id, x, y).unwrap();
        }
        let fresh = rebuild(&c);
        assert_eq!(values(&c), values(&fresh), "step {step}: {}", c.label(id));
        for id in c.ids() {
            let mine: Vec<_> = c.vertices(id).collect();
            let theirs: Vec<_> = fresh.vertices(id).collect();
            assert_eq!(mine, theirs, "step {step}: vertices of {}", c.label(id));
        }
    }
}

#[test]
fn value_and_def_keep_their_size() {
    // Both were 48 bytes before definitions with lists were added; a larger
    // value slows every update.
    assert!(std::mem::size_of::<Value>() <= 48);
    assert!(std::mem::size_of::<Def>() <= 48);
}
