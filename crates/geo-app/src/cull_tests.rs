//! Skipping the objects outside the view changes no pixel: for random views
//! of a construction with every drawn kind, the tessellated picture with
//! culling is the one without, vertex for vertex. And a view of a small
//! part of a large construction paints only what is in it.

use crate::draw::{self, Scene};
use crate::interact::ViewTransform;
use crate::view::{self, Camera};
use crate::workbench::Workbench;
use eframe::egui::{epaint, vec2, Context, Pos2, Rect};
use geo_kernel::kernel::{Id, Kind};

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
/// points, a segment, a line, a ray, a circle, an ellipse, a vector, an
/// angle, a polygon with its sides, a regular polygon, a polyline, a
/// midpoint and an intersection; once, a hyperbola, a parabola and two
/// functions (named `Unbounded...`). Labels are long enough to stick out of
/// the area.
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
            format!("{} = Ellipse({b}, {c}, {a})", p("e")),
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
    s += "UnboundedHyperbola = Hyperbola(PointA_{0}, PointB_{0}, PointC_{0})\n\
          UnboundedParabola = Parabola(PointC_{0}, l_{0})\n\
          Unboundedf(x) = 1 / (x - 3)\nUnboundedg(x) = sqrt(x) + sin(5x)\n";
    s
}

/// [`scene`] with objects whose screen coordinates overflow (a point far
/// out, a segment, a vector and a polygon through it, a huge circle) and
/// intersections that do not exist.
fn scene_with_extremes(cells: usize) -> String {
    let mut s = scene(cells);
    s += "Far = (10^300, -10^300)\nfarSeg = Segment(PointA_{0}, Far)\n\
          farVec = Vector(Far, PointB_{0})\nbig = 10^300\nhuge = Circle(PointA_{0}, big)\n\
          farPoly = Polygon(PointA_{0}, Far, PointC_{0})\n\
          Z1 = (0, 0)\nZ2 = (1, 0)\nZ3 = (0, 1)\nZ4 = (1, 1)\nCc = (1000, 1000)\nCd = (1001, 1000)\n\
          par1 = Line(Z1, Z2)\npar2 = Line(Z3, Z4)\nNone1 = Intersect(par1, par2)\n\
          away = Circle(Cc, Cd)\nNone2 = Intersect(par1, away)\n\
          farEllipse = Ellipse(PointA_{0}, Far, PointC_{0})\nfarHyp = Hyperbola(Z1, Far, Z3)\n";
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
    picture_with(ctx, wb, t, cull, bounded_only, (None, None))
}

/// [`picture`] with an object selected and one under the pointer.
fn picture_with(
    ctx: &Context,
    wb: &Workbench,
    t: &ViewTransform,
    cull: bool,
    bounded_only: bool,
    (selected, hovered): (Option<Id>, Option<Id>),
) -> (Vec<(Rect, epaint::Mesh)>, usize) {
    let c = wb.construction();
    let visible = |id| {
        !bounded_only
            || !(matches!(c.kind(id), Kind::Line | Kind::Ray)
                || c.label(id).starts_with("Unbounded"))
    };
    let mut out = ctx.run_ui(Default::default(), |ui| {
        let rect = Rect::from_min_size(Pos2::new(40.0, 30.0), vec2(W, H));
        let painter = ui.painter_at(rect);
        let scene = Scene {
            visible: &visible,
            ghost: &|_| false,
            selected,
            hovered,
            palette: wb.palette(),
            cull,
            generation: wb.rebuilds(),
            label_offset: &|_| None,
            label_rects: None,
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
    let ids: Vec<Id> = wb.construction().ids().collect();
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
        // Now and then an object selected and one under the pointer (their
        // highlights), any of them, in view or not.
        let pick = |rng: &mut Rng| {
            (!rng.next().is_multiple_of(3)).then(|| ids[(rng.next() % ids.len() as u64) as usize])
        };
        let marks = (pick(&mut rng), pick(&mut rng));
        let (culled, n_culled) = picture_with(&ctx, &wb, &t, true, false, marks);
        let (all, n_all) = picture_with(&ctx, &wb, &t, false, false, marks);
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

/// The shapes `draw::paint` gives, each as text (`NaN` reads as itself).
fn shapes_text(
    ctx: &Context,
    wb: &Workbench,
    t: &ViewTransform,
    cull: bool,
    (selected, hovered): (Option<Id>, Option<Id>),
) -> Vec<String> {
    let mut out = ctx.run_ui(Default::default(), |ui| {
        let rect = Rect::from_min_size(Pos2::new(40.0, 30.0), vec2(W, H));
        let scene = Scene {
            visible: &|_| true,
            ghost: &|_| false,
            selected,
            hovered,
            palette: wb.palette(),
            cull,
            generation: wb.rebuilds(),
            label_offset: &|_| None,
            label_rects: None,
        };
        draw::paint(&ui.painter_at(rect), rect, wb.construction(), t, &scene);
    });
    out.textures_delta.clear();
    out.shapes.iter().map(|s| format!("{s:?}")).collect()
}

/// Objects whose screen coordinates overflow (or that have none) are drawn
/// as without culling. Compared as shapes, not tessellated: the far point's
/// coordinates do not fit in an `f32`, and epaint's tessellator stops on the
/// `NaN`s that gives in debug builds, culling or not.
#[test]
fn culling_draws_objects_with_overflowing_coordinates_as_without() {
    let wb = Workbench::from_script(&scene_with_extremes(4)).unwrap();
    let ids: Vec<Id> = wb.construction().ids().collect();
    let ctx = Context::default();
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let mut fewer = 0;
    for case in 0..60 {
        let mut cam = fitted(&wb);
        let zoom = [1e-300, 0.05, 1.0, 40.0, 1e6][case % 5] * rng.range(0.7, 1.4);
        cam.zoom_at(
            zoom,
            rng.range(0.0, W as f64),
            rng.range(0.0, H as f64),
            W as f64,
            H as f64,
        );
        cam.pan_pixels(
            rng.range(-2.0, 2.0) * W as f64,
            rng.range(-2.0, 2.0) * H as f64,
        );
        let t = cam.transform(W as f64, H as f64);
        let pick = |rng: &mut Rng| ids[(rng.next() % ids.len() as u64) as usize];
        let marks = (Some(pick(&mut rng)), Some(pick(&mut rng)));
        let culled = shapes_text(&ctx, &wb, &t, true, marks);
        let all = shapes_text(&ctx, &wb, &t, false, marks);
        fewer += usize::from(culled.len() < all.len());
        // Culling only leaves shapes out (of objects outside the area),
        // and every shape it keeps is the same, in the same order.
        let mut rest = all.iter();
        for (i, s) in culled.iter().enumerate() {
            assert!(rest.any(|a| a == s), "case {case}: shape {i} changed: {s}");
        }
    }
    assert!(fewer > 0, "no view culled anything");
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
