//! Definitions with a stored list of parents (`Construction::add_with_list`):
//! a construction can be rebuilt from its definitions, and incremental
//! updates give the same values as building everything again.

use geo_kernel::algo::polygon::PolygonDef;
use geo_kernel::{script, Construction, Def, Id, IdList, Value};
use std::collections::HashMap;

/// Builds a fresh construction from the definitions of `orig`. A definition
/// with a list stores the objects of its list again, or, when `share` is
/// set and an earlier definition read the same handle, reads the list that
/// was stored for that one. Handles are never assumed to come out equal.
fn rebuild_with(orig: &Construction, share: bool) -> Construction {
    let mut fresh = Construction::new();
    let mut stored: HashMap<IdList, IdList> = HashMap::new();
    for id in orig.ids() {
        let def = orig.def(id);
        let label = orig.label(id);
        let new = match def.list() {
            Some(list) if share && stored.contains_key(&list) => {
                fresh.add(label, def.with_list(stored[&list]))
            }
            Some(list) => fresh
                .add_with_list(label, orig.list(list), |l| def.with_list(l))
                .inspect(|&new| {
                    stored.insert(list, fresh_list(new, &fresh));
                }),
            None => fresh.add(label, def),
        };
        new.unwrap_or_else(|e| panic!("rebuilding `{label}` failed: {e}"));
    }
    fresh
}

fn fresh_list(id: Id, c: &Construction) -> IdList {
    c.def(id).list().expect("a definition with a list")
}

fn rebuild(orig: &Construction) -> Construction {
    rebuild_with(orig, true)
}

fn values(c: &Construction) -> Vec<Value> {
    c.ids().map(|id| c.value(id)).collect()
}

/// Asserts that two constructions have the same definitions, comparing the
/// lists by the objects they hold rather than by their handles.
fn assert_same_defs(a: &Construction, b: &Construction) {
    assert_eq!(a.len(), b.len());
    for (x, y) in a.ids().zip(b.ids()) {
        let (dx, dy) = (a.def(x), b.def(y));
        assert_eq!(a.label(x), b.label(y));
        match (dx.list(), dy.list()) {
            (Some(lx), Some(ly)) => {
                assert_eq!(a.list(lx), b.list(ly), "list of {}", a.label(x));
                // the rest of the definition, with one handle for both
                assert_eq!(dx.with_list(ly), dy, "{}", a.label(x));
            }
            (None, None) => assert_eq!(dx, dy, "{}", a.label(x)),
            _ => panic!("only one definition of {} has a list", a.label(x)),
        }
    }
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
    for share in [true, false] {
        let fresh = rebuild_with(&orig, share);
        assert_same_defs(&orig, &fresh);
        assert_eq!(values(&fresh), values(&orig));
    }
}

#[test]
fn definitions_may_share_one_list() {
    let mut c = script::load(SCRIPT).unwrap();
    let a = c.id("a").unwrap();
    let list = c.def(a).list().unwrap();
    // a copy of `a`, and a polygon over the same five points
    let copy = c.add("a2", c.def(a)).unwrap();
    let poly = c.add("p", Def::Polygon(PolygonDef::Polygon(list))).unwrap();
    assert_eq!(c.def(copy).list(), Some(list));
    assert_eq!(c.value(copy), c.value(a));
    let p_area = c.add("pa", Def::Polygon(PolygonDef::Area(poly))).unwrap();
    assert_eq!(c.value(p_area), c.value(a));
    let e = c.id("E").unwrap();
    for (step, (x, y)) in [(1.0, 4.0), (-2.0, 3.5), (0.0, 3.0)]
        .into_iter()
        .enumerate()
    {
        c.move_point(e, x, y).unwrap();
        assert_eq!(c.value(copy), c.value(a), "step {step}");
        assert_eq!(c.value(p_area), c.value(a), "step {step}");
        for share in [true, false] {
            let fresh = rebuild_with(&c, share);
            assert_same_defs(&c, &fresh);
            assert_eq!(values(&fresh), values(&c), "step {step}");
        }
    }
    // shared in the rebuild too, stored twice without sharing
    let shared = rebuild_with(&c, true);
    let ids = |f: &Construction| ["a", "a2", "p"].map(|l| f.def(f.id(l).unwrap()).list());
    let [x, y, z] = ids(&shared);
    assert!(x == y && y == z);
    let [x, y, z] = ids(&rebuild_with(&c, false));
    assert!(x != y && y != z);
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
    // value slows every update, and a change either way should be noticed.
    assert_eq!(std::mem::size_of::<Value>(), 48);
    assert_eq!(std::mem::size_of::<Def>(), 48);
    assert_eq!(std::mem::size_of::<geo_kernel::Kind>(), 1);
}

/// `Construction::inputs`: the parents in command order, then the list; an
/// expression and a free object have none.
#[test]
fn inputs_are_the_parents_then_the_list() {
    let c = script::load(
        "A = (0, 0)\nB = (4, 0)\nC = (0, 3)\nf = Line(A, B)\np = Polygon(A, B, C)\n\
         r = 2\nk = Circle(C, r)\nq = Polygon(A, B, 5)\nd = r + 1\n",
    )
    .unwrap();
    let ids = |labels: &[&str]| -> Vec<Id> { labels.iter().map(|l| c.id(l).unwrap()).collect() };
    let of = |l: &str| c.inputs(c.id(l).unwrap()).collect::<Vec<Id>>();
    assert_eq!(of("f"), ids(&["A", "B"]));
    assert_eq!(of("p"), ids(&["A", "B", "C"]));
    assert_eq!(of("k"), ids(&["C", "r"]));
    assert_eq!(of("A"), Vec::<Id>::new());
    assert_eq!(of("d"), Vec::<Id>::new());
    let q = of("q");
    assert_eq!(q[..2], ids(&["A", "B"])[..]);
}
