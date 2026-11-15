//! Conics and function graphs in the SVG drawing.

use geo_kernel::script;
use geo_kernel::svg::{render, View};

const W: f64 = 400.0;
const H: f64 = 300.0;

fn view() -> View {
    View {
        xmin: -4.0,
        ymin: -3.0,
        xmax: 4.0,
        ymax: 3.0,
    }
}

/// The points of every polyline in the document.
fn polylines(svg: &str) -> Vec<Vec<(f64, f64)>> {
    svg.lines()
        .filter_map(|l| l.strip_prefix(r#"<polyline points=""#))
        .map(|rest| {
            let pts = &rest[..rest.find('"').unwrap()];
            pts.split(' ')
                .map(|p| {
                    let (x, y) = p.split_once(',').unwrap();
                    (x.parse().unwrap(), y.parse().unwrap())
                })
                .collect()
        })
        .collect()
}

fn within_image(paths: &[Vec<(f64, f64)>]) {
    for &(x, y) in paths.iter().flatten() {
        assert!((-11.0..=W + 11.0).contains(&x), "{x}");
        assert!((-11.0..=H + 11.0).contains(&y), "{y}");
    }
}

#[test]
fn a_function_with_a_pole_is_drawn_in_pieces() {
    let c = script::load("f(x) = 1/x\n").unwrap();
    let svg = render(&c, view(), W as u32, H as u32);
    let paths = polylines(&svg);
    assert!(paths.len() >= 2, "{svg}");
    within_image(&paths);
    // x = 0 is the middle column; no segment crosses it.
    for path in &paths {
        for w in path.windows(2) {
            assert!((w[0].0 - W / 2.0) * (w[1].0 - W / 2.0) >= 0.0, "{w:?}");
        }
    }
}

#[test]
fn conics_are_drawn_cut_to_the_image() {
    let c = script::load(
        "F = (-2, 0)\nG = (2, 0)\nr = 3\ns = 1\ne = Ellipse(F, G, r)\nh = Hyperbola(F, G, s)\n\
         L = (0, 1)\nM = (0, -1)\nN = (1, -1)\nk = Line(M, N)\np = Parabola(L, k)\n",
    )
    .unwrap();
    let svg = render(&c, view(), W as u32, H as u32);
    let paths = polylines(&svg);
    // The ellipse lies inside the view, the hyperbola has two branches and
    // the parabola is one arc.
    assert!(paths.len() >= 4, "{} paths: {svg}", paths.len());
    // The ellipse is closed.
    assert!(paths.iter().any(|p| p.len() > 10 && p[0] == p[p.len() - 1]));
    within_image(&paths);
}
