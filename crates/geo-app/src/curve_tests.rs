//! Conics and function graphs on the canvas: the tessellated strokes cover
//! the curves and nothing across an asymptote, the drawings are made again
//! only when their object or the view changes, and extreme zooms draw
//! without panicking.

use crate::curves::curves_tessellated;
use crate::draw::{self, Scene};
use crate::interact::ViewTransform;
use crate::workbench::Workbench;
use eframe::egui::{epaint, vec2, Context, Pos2, Rect};
use geo_kernel::kernel::Id;
use geo_kernel::svg::View;

const W: f32 = 800.0;
const H: f32 = 600.0;
/// Where the area starts in the window.
const ORIGIN: Pos2 = Pos2::new(20.0, 10.0);

fn transform(xmin: f64, xmax: f64, ymin: f64, ymax: f64) -> ViewTransform {
    ViewTransform::new(
        View {
            xmin,
            ymin,
            xmax,
            ymax,
        },
        W as f64,
        H as f64,
    )
}

fn standard() -> ViewTransform {
    transform(-8.0, 8.0, -6.0, 6.0)
}

/// The triangles of the frame's strokes (not text), in screen points.
fn stroke_triangles(
    ctx: &Context,
    wb: &Workbench,
    t: &ViewTransform,
    marks: (Option<Id>, Option<Id>),
) -> Vec<[Pos2; 3]> {
    let mut out = ctx.run_ui(Default::default(), |ui| {
        let rect = Rect::from_min_size(ORIGIN, vec2(W, H));
        let scene = Scene {
            visible: &|_| true,
            ghost: &|_| false,
            selected: marks.0,
            hovered: marks.1,
            palette: wb.palette(),
            cull: true,
            generation: wb.rebuilds(),
        };
        draw::paint(&ui.painter_at(rect), rect, wb.construction(), t, &scene);
    });
    out.textures_delta.clear();
    let mut tris = Vec::new();
    for p in ctx.tessellate(out.shapes, out.pixels_per_point) {
        let epaint::Primitive::Mesh(m) = p.primitive else {
            continue;
        };
        for tri in m.indices.chunks(3) {
            let v = [0, 1, 2].map(|k| m.vertices[tri[k] as usize]);
            // Text has texture coordinates; strokes and fills do not.
            if v.iter().all(|v| v.uv == epaint::WHITE_UV) && v.iter().any(|v| v.color.a() > 0) {
                tris.push(v.map(|v| v.pos));
            }
        }
    }
    tris
}

fn inside(t: &[Pos2; 3], p: Pos2) -> bool {
    let cross = |a: Pos2, b: Pos2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    let area = (t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[1].y - t[0].y) * (t[2].x - t[0].x);
    if area == 0.0 {
        return false;
    }
    let (d1, d2, d3) = (cross(t[0], t[1]), cross(t[1], t[2]), cross(t[2], t[0]));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

fn covered(tris: &[[Pos2; 3]], p: Pos2) -> bool {
    tris.iter().any(|t| inside(t, p))
}

fn screen(t: &ViewTransform, x: f64, y: f64) -> Pos2 {
    let (sx, sy) = t.world_to_screen(x, y);
    ORIGIN + vec2(sx as f32, sy as f32)
}

/// Whether `p` lies in the area, `margin` points inside its border.
fn in_area(p: Pos2, margin: f32) -> bool {
    p.x >= ORIGIN.x + margin
        && p.x <= ORIGIN.x + W - margin
        && p.y >= ORIGIN.y + margin
        && p.y <= ORIGIN.y + H - margin
}

/// Every sampled point of `f` inside the area lies on a stroke.
fn check_graph_covered(script: &str, f: impl Fn(f64) -> f64) {
    let wb = Workbench::from_script(script).unwrap();
    let t = standard();
    let tris = stroke_triangles(&Context::default(), &wb, &t, (None, None));
    assert!(!tris.is_empty(), "{script}: nothing drawn");
    let mut checked = 0;
    for i in 0..=2000 {
        let x = -8.0 + 16.0 * i as f64 / 2000.0;
        let y = f(x);
        if !y.is_finite() {
            continue;
        }
        let p = screen(&t, x, y);
        // Away from the border (the label rule) and where the graph is not
        // steeper than the stroke is wide.
        let slope = (f(x + 1e-4) - f(x - 1e-4)) / 2e-4;
        if in_area(p, 4.0) && slope.abs() < 20.0 {
            assert!(covered(&tris, p), "{script}: ({x}, {y}) is not drawn");
            checked += 1;
        }
    }
    assert!(checked > 100, "{script}: only {checked} points checked");
}

/// No stroke covers the column of an asymptote at `x0`.
fn check_column_empty(script: &str, x0: f64) {
    let wb = Workbench::from_script(script).unwrap();
    let t = standard();
    let tris = stroke_triangles(&Context::default(), &wb, &t, (None, None));
    let top = screen(&t, x0, 6.0);
    for k in 0..(H as i32) {
        let p = Pos2::new(top.x, ORIGIN.y + k as f32);
        assert!(
            !covered(&tris, p),
            "{script}: the asymptote at {x0} is drawn at {p:?}"
        );
    }
}

#[test]
fn function_graphs_are_covered_by_their_strokes() {
    check_graph_covered("f(x) = x^2 / 4\n", |x| x * x / 4.0);
    check_graph_covered("f(x) = 1 / x\n", |x| 1.0 / x);
    check_graph_covered("f(x) = tan(x)\n", f64::tan);
    check_graph_covered("f(x) = sqrt(x)\n", f64::sqrt);
}

#[test]
fn asymptotes_and_gaps_are_not_drawn() {
    check_column_empty("f(x) = 1 / x\n", 0.0);
    for k in [-2.0, -1.0, 0.0, 1.0] {
        check_column_empty(
            "f(x) = tan(x)\n",
            std::f64::consts::FRAC_PI_2 + k * std::f64::consts::PI,
        );
    }
    // sqrt(x) draws nothing left of the origin.
    let wb = Workbench::from_script("f(x) = sqrt(x)\n").unwrap();
    let t = standard();
    let tris = stroke_triangles(&Context::default(), &wb, &t, (None, None));
    let edge = screen(&t, 0.0, 0.0).x - 3.0;
    for tri in &tris {
        assert!(tri.iter().all(|p| p.x >= edge), "{tri:?}");
    }
}

#[test]
fn conics_are_covered_by_their_strokes() {
    let wb = Workbench::from_script(
        "F = (-2, 0)\nG = (2, 0)\nr = 3\nk = 1\ne = Ellipse(F, G, r)\nh = Hyperbola(F, G, k)\n\
         L = (0, 1)\nM = (-1, -1)\nN = (1, -1)\nd = Line(M, N)\np = Parabola(L, d)\n",
    )
    .unwrap();
    let t = standard();
    let tris = stroke_triangles(&Context::default(), &wb, &t, (None, None));
    let mut checked = 0;
    let (b5, b3) = (5f64.sqrt(), 3f64.sqrt());
    for i in 0..1000 {
        let s = std::f64::consts::TAU * i as f64 / 1000.0;
        let u = -3.0 + 6.0 * i as f64 / 1000.0;
        for (x, y) in [
            // x²/9 + y²/5 = 1
            (3.0 * s.cos(), b5 * s.sin()),
            // x² - y²/3 = 1
            (u.cosh(), b3 * u.sinh()),
            (-u.cosh(), b3 * u.sinh()),
            // y = x²/4
            (u * 3.0, u * u * 9.0 / 4.0),
        ] {
            let p = screen(&t, x, y);
            if in_area(p, 4.0) {
                assert!(covered(&tris, p), "({x}, {y}) is not drawn");
                checked += 1;
            }
        }
    }
    assert!(checked > 1000, "{checked}");
    // The two branches of the hyperbola are not joined: between them, away
    // from the ellipse and the parabola, nothing is drawn.
    for (y, half) in [(1.5, 0.9), (4.0, 2.0), (-4.0, 2.0)] {
        for i in 0..=40 {
            let x = -half + 2.0 * half * i as f64 / 40.0;
            let q = screen(&t, x, y);
            let hit: Vec<_> = tris.iter().filter(|tr| inside(tr, q)).collect();
            assert!(hit.is_empty(), "({x}, {y}) {q:?} is drawn by {hit:?}");
        }
    }
}

#[test]
fn drawings_are_made_again_only_when_their_object_or_the_view_changes() {
    let mut wb = Workbench::from_script(
        "F = (-2, 0)\nG = (2, 0)\nr = 3\ne = Ellipse(F, G, r)\nf(x) = r * sin(x)\n\
         g(x) = x^2\nP = (5, 5)\n",
    )
    .unwrap();
    let ctx = Context::default();
    let t = standard();
    let n0 = curves_tessellated();
    stroke_triangles(&ctx, &wb, &t, (None, None));
    assert_eq!(curves_tessellated() - n0, 3, "the ellipse and two graphs");
    // An idle frame, and one with a curve selected and one hovered.
    let n = curves_tessellated();
    stroke_triangles(&ctx, &wb, &t, (None, None));
    let c = wb.construction();
    let (e, f) = (c.id("e").unwrap(), c.id("f").unwrap());
    stroke_triangles(&ctx, &wb, &t, (Some(e), Some(f)));
    assert_eq!(curves_tessellated(), n, "idle frames tessellate nothing");
    // A point no curve reads moves.
    let p = wb.construction().id("P").unwrap();
    assert!(wb.move_point(p, 6.0, 4.0));
    stroke_triangles(&ctx, &wb, &t, (None, None));
    assert_eq!(
        curves_tessellated(),
        n,
        "an unrelated drag tessellates nothing"
    );
    // A focus moves: the ellipse only.
    let fid = wb.construction().id("F").unwrap();
    assert!(wb.move_point(fid, -2.5, 0.0));
    stroke_triangles(&ctx, &wb, &t, (None, None));
    assert_eq!(curves_tessellated() - n, 1);
    // r changes: the ellipse and f, not g.
    let n = curves_tessellated();
    let r = wb.construction().id("r").unwrap();
    assert!(wb.set_number(r, 3.5));
    stroke_triangles(&ctx, &wb, &t, (None, None));
    assert_eq!(curves_tessellated() - n, 2);
    // The view moves: every curve.
    let n = curves_tessellated();
    stroke_triangles(&ctx, &wb, &transform(-7.0, 9.0, -6.0, 6.0), (None, None));
    assert_eq!(curves_tessellated() - n, 3);
}

#[test]
fn off_screen_ellipses_are_culled_before_they_are_plotted() {
    let wb = Workbench::from_script("F = (100, 0)\nG = (104, 0)\nr = 3\ne = Ellipse(F, G, r)\n")
        .unwrap();
    let n = curves_tessellated();
    stroke_triangles(&Context::default(), &wb, &standard(), (None, None));
    assert_eq!(curves_tessellated(), n);
}

#[test]
fn off_screen_hyperbolas_and_parabolas_are_culled_before_they_are_plotted() {
    // The hyperbola opens left and right around (100, 0); the parabola opens
    // to the right of x = 200. Neither comes near the standard view.
    let wb = Workbench::from_script(
        "F = (98, 0)\nG = (102, 0)\nk = 1\nh = Hyperbola(F, G, k)\n\
         L = (201, 0)\nM = (199, -1)\nN = (199, 1)\nd = Line(M, N)\np = Parabola(L, d)\n",
    )
    .unwrap();
    let ctx = Context::default();
    let n = curves_tessellated();
    stroke_triangles(&ctx, &wb, &standard(), (None, None));
    assert_eq!(curves_tessellated(), n);
    // Seen from near them, both are plotted.
    stroke_triangles(
        &ctx,
        &wb,
        &transform(90.0, 210.0, -45.0, 45.0),
        (None, None),
    );
    assert_eq!(curves_tessellated(), n + 2);
}

#[test]
fn extreme_zooms_draw_without_panicking() {
    let wb = Workbench::from_script(
        "F = (-2, 0)\nG = (2, 0)\nr = 3\nk = 1\ne = Ellipse(F, G, r)\nh = Hyperbola(F, G, k)\n\
         L = (0, 1)\nM = (-1, -1)\nN = (1, -1)\nd = Line(M, N)\np = Parabola(L, d)\n\
         f(x) = 1 / x\ng(x) = tan(x)\nq(x) = sqrt(x)\n",
    )
    .unwrap();
    let ctx = Context::default();
    for (half, cx, cy) in [
        (1e-6 * 8.0, 0.0, 0.0),
        (1e-6 * 8.0, 3.0, 0.0),
        (1e-6 * 8.0, 1.0, 1.0),
        (1e9 * 8.0, 0.0, 0.0),
        (1e9 * 8.0, 1e12, -1e12),
    ] {
        let t = transform(cx - half, cx + half, cy - 0.75 * half, cy + 0.75 * half);
        for tri in stroke_triangles(&ctx, &wb, &t, (None, None)) {
            assert!(tri.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
        }
    }
}
