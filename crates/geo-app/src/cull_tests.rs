//! Skipping the objects outside the view changes no pixel: for random views
//! of a construction with every drawn kind, the tessellated picture with
//! culling is the one without, vertex for vertex. And a view of a small
//! part of a large construction paints only what is in it.

use crate::draw::{self, Scene};
use crate::interact::ViewTransform;
use crate::view::{self, Camera};
use crate::workbench::Workbench;
use eframe::egui::{epaint, vec2, Context, Pos2, Rect};
use geo_kernel::kernel::Kind;

const W: f32 = 900.0;
const H: f32 = 640.0;

/// A small deterministic generator (xorshift).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Uniform in `[lo, hi)`.
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f64 / 1_000_000.0
    }
}

/// `cells` cells in a square grid, each with every kind the canvas draws:
/// points, a segment, a line, a ray, a circle, a vector, an angle, a
/// polygon with its sides, a regular polygon, a polyline, a midpoint and an
/// intersection. Labels are long enough to stick out of the area.
fn scene(cells: usize) -> String {
    let cols = (cells as f64).sqrt().ceil() as usize;
    let mut s = String::new();
    for i in 0..cells {
        let (x, y) = ((i % cols) as f64 * 6.0, (i / cols) as f64 * 6.0);
        let p = |n: &str| format!("{n}_{{{i}}}");
        let (a, b, c) = (p("PointA"), p("PointB"), p("PointC"));
        s += &format!(
            "{a} = ({x}, {y})\n{b} = ({}, {y})\n{c} = ({x}, {})\n",
            x + 2.0,
            y + 2.0
        );
        for line in [
            format!("{} = Segment({a}, {c})", p("s")),
            format!("{} = Line({b}, {c})", p("l")),
            format!("{} = Ray({a}, {b})", p("r")),
            format!("{} = Circle({a}, {b})", p("k")),
            format!("{} = Vector({b}, {c})", p("v")),
            format!("{} = Angle({b}, {a}, {c})", p("alpha")),
            format!("{} = Polygon({a}, {b}, {c})", p("t")),
            format!("{} = Polygon({c}, {a}, 5)", p("q")),
            format!("{} = PolyLine({a}, {c}, {b})", p("pl")),
            format!("{} = Midpoint({b}, {c})", p("M")),
            format!("{} = Intersect({}, {})", p("I"), p("l"), p("k")),
        ] {
            s += &line;
            s.push('\n');
        }
    }
    s
}

/// What `draw::paint` gives for `t`: the tessellated primitives (clip
/// rectangle, vertices, indices) and the number of shapes.
fn picture(
    ctx: &Context,
    wb: &Workbench,
    t: &ViewTransform,
    cull: bool,
    bounded_only: bool,
) -> (Vec<(Rect, epaint::Mesh)>, usize) {
    let c = wb.construction();
    let visible = |id| !bounded_only || !matches!(c.kind(id), Kind::Line | Kind::Ray);
    let mut out = ctx.run_ui(Default::default(), |ui| {
        let rect = Rect::from_min_size(Pos2::new(40.0, 30.0), vec2(W, H));
        let painter = ui.painter_at(rect);
        let scene = Scene {
            visible: &visible,
            ghost: &|_| false,
            selected: None,
            hovered: None,
            palette: wb.palette(),
            cull,
        };
        draw::paint(&painter, rect, wb.construction(), t, &scene);
    });
    out.textures_delta.clear();
    let shapes = out.shapes.len();
    let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
    let meshes = prims
        .into_iter()
        .filter_map(|p| match p.primitive {
            epaint::Primitive::Mesh(m) => Some((p.clip_rect, m)),
            epaint::Primitive::Callback(_) => None,
        })
        .collect();
    (meshes, shapes)
}

fn fitted(wb: &Workbench) -> Camera {
    Camera::fit(view::content_bounds(wb.construction()), W as f64, H as f64)
}

#[test]
fn culling_never_changes_the_picture_of_a_random_view() {
    let wb = Workbench::from_script(&scene(16)).unwrap();
    let ctx = Context::default();
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let (mut fewer, mut empty) = (0, 0);
    for case in 0..120 {
        let mut cam = fitted(&wb);
        // Zoomed out far, about right, or far in (a few cells, or inside
        // one), about a random spot, then moved by up to two areas.
        let zoom = [0.05, 0.3, 1.0, 3.0, 12.0, 60.0][case % 6] * rng.range(0.7, 1.4);
        let (sx, sy) = (rng.range(0.0, W as f64), rng.range(0.0, H as f64));
        cam.zoom_at(zoom, sx, sy, W as f64, H as f64);
        cam.pan_pixels(
            rng.range(-2.0, 2.0) * W as f64,
            rng.range(-2.0, 2.0) * H as f64,
        );
        let t = cam.transform(W as f64, H as f64);
        let (culled, n_culled) = picture(&ctx, &wb, &t, true, false);
        let (all, n_all) = picture(&ctx, &wb, &t, false, false);
        assert_eq!(culled.len(), all.len(), "case {case}: number of meshes");
        for (i, ((r1, m1), (r2, m2))) in culled.iter().zip(&all).enumerate() {
            assert_eq!(r1, r2, "case {case}, mesh {i}: clip");
            assert!(m1 == m2, "case {case}, mesh {i}: the picture changed");
        }
        assert!(n_culled <= n_all, "case {case}");
        fewer += usize::from(n_culled < n_all);
        empty += usize::from(culled.iter().all(|(_, m)| m.vertices.is_empty()));
    }
    // The views did cull, and most of them showed something.
    assert!(fewer > 40, "only {fewer} views culled anything");
    assert!(empty < 60, "{empty} views were empty");
}

#[test]
fn a_zoomed_in_view_of_a_large_construction_paints_only_what_is_in_it() {
    let shapes = |cells: usize, zoom: f64, bounded_only: bool| {
        let wb = Workbench::from_script(&scene(cells)).unwrap();
        let mut cam = fitted(&wb);
        cam.zoom_at(zoom, W as f64 / 2.0, H as f64 / 2.0, W as f64, H as f64);
        let t = cam.transform(W as f64, H as f64);
        picture(&Context::default(), &wb, &t, true, bounded_only).1
    };
    // The same window onto the middle of the grid, about one cell wide, at
    // 4 x 4 cells and at 12 x 12 (nine times the objects, the same scale).
    // Without the lines and rays, which are unbounded and always drawn,
    // the shapes are those of the cells in the window, whatever the size.
    let small = shapes(16, 4.0, true);
    let large = shapes(144, 12.0, true);
    assert!(
        large <= small + small / 4,
        "bounded shapes: {large} in the large view, {small} in the small one"
    );
    let all = shapes(144, 1.0, true);
    assert!(large * 20 < all, "{large} shapes zoomed in, {all} for all");
    // With the lines and rays, two shapes per cell more at most (a line
    // or ray, and its label where it crosses the window).
    let with_lines = shapes(144, 12.0, false);
    println!("bounded shapes: small {small}, large {large}, all {all}; with lines {with_lines}");
    assert!(
        with_lines <= large + 4 * 144,
        "{with_lines} with lines and rays"
    );
}
