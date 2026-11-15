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
fn value_and_def_keep_their_size() {
    // Both were 48 bytes before definitions with lists were added; a larger
    // value slows every update.
    assert!(std::mem::size_of::<Value>() <= 48);
    assert!(std::mem::size_of::<Def>() <= 48);
}
