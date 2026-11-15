//! Input that tries to make the script layer do quadratic work.

use geo_kernel::script::{exec, load};
use geo_kernel::Construction;
use std::time::{Duration, Instant};

const BOUND: Duration = Duration::from_secs(30);

#[test]
fn a_huge_label_list_is_refused_before_it_is_scanned() {
    let n = 1_000_000;
    let labels: Vec<String> = (0..n).map(|i| format!("o{i}")).collect();
    let mut c = Construction::new();
    exec(&mut c, "A = (0, 0)").unwrap();
    exec(&mut c, "B = (1, 0)").unwrap();
    let start = Instant::now();
    let line = format!("{{{}}} = AngularBisector(x, y)", labels.join(", "));
    assert!(exec(&mut c, &line).is_err());
    let line = format!("{{{}}} = Tangent(A, B)", labels.join(", "));
    assert!(exec(&mut c, &line).is_err());
    assert!(start.elapsed() < BOUND, "{:?}", start.elapsed());
    assert_eq!(c.len(), 2);
}

#[test]
fn duplicate_and_taken_labels_are_still_caught() {
    let base = "A = (0, 0)\nB = (4, 0)\nC = (0, 3)\nf = Line(A, B)\ng = Line(A, C)\n";
    let mut c = load(base).unwrap();
    let e = exec(&mut c, "{b, b} = AngularBisector(f, g)").unwrap_err();
    assert!(e.contains("used twice"), "{e}");
    let e = exec(&mut c, "{f, h} = AngularBisector(f, g)").unwrap_err();
    assert!(e.contains("already exists"), "{e}");
    assert_eq!(c.len(), 5);
    exec(&mut c, "{b, h} = AngularBisector(f, g)").unwrap();
    assert_eq!(c.len(), 7);
}

#[test]
fn many_polygons_get_their_labels_in_the_same_sequence_quickly() {
    // The side labels of a triangle are named after the vertices: a, b, c,
    // then a_1, b_1, c_1, ... (`a_{10}` from 10 on). The sequence must not
    // change, and the 20000th polygon must not cost more than the first.
    let mut text = String::from("A = (0, 0)\nB = (4, 0)\nC = (0, 3)\n");
    let n = 20_000;
    for i in 0..n {
        text.push_str(&format!("p{i} = Polygon(A, B, C)\n"));
    }
    let start = Instant::now();
    let c = load(&text).unwrap();
    assert!(start.elapsed() < BOUND, "{:?}", start.elapsed());
    // polygon i has the sides `a`, `b`, `c` with index i (none for i = 0)
    assert!(c.id("a").is_some() && c.id("a_1").is_some() && c.id("a_9").is_some());
    assert!(c.id("a_{10}").is_some() && c.id("a_{19999}").is_some());
    assert!(c.id("a_{20000}").is_none());
    assert_eq!(c.len(), 3 + n + 3 * n);
}

#[test]
fn many_tangent_pairs_get_their_labels_quickly() {
    let mut text = String::from("A = (0, 0)\nr = 1\nk = Circle(A, r)\nP = (3, 0)\n");
    for i in 0..5_000 {
        text.push_str(&format!("t_x{i} = Tangent(P, k)\n"));
    }
    let start = Instant::now();
    let c = load(&text).unwrap();
    assert!(start.elapsed() < BOUND, "{:?}", start.elapsed());
    assert!(c.id("t_{1}").is_some() && c.id("t_{10000}").is_some());
}
