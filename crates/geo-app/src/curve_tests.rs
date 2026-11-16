//! Conics and function graphs on the canvas: the tessellated strokes cover
//! the curves and nothing across an asymptote, the drawings are made again
//! only when their object or the view changes, and extreme zooms draw
//! without panicking.

use crate::curves::{curves_plotted, curves_tessellated};
use crate::draw::{self, Scene};
use crate::geom::ToEgui;
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
            label_offset: &|_| None,
            label_rects: None,
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
    // A pan of 50 points, less than half the margin: the kept drawings
    // move, nothing is plotted or tessellated.
    let (n, m) = (curves_tessellated(), curves_plotted());
    stroke_triangles(&ctx, &wb, &transform(-7.0, 9.0, -6.0, 6.0), (None, None));
    assert_eq!((curves_tessellated() - n, curves_plotted() - m), (0, 0));
    // A pan past half the margin (150 points in all): every curve again.
    stroke_triangles(&ctx, &wb, &transform(-5.0, 11.0, -6.0, 6.0), (None, None));
    assert_eq!((curves_tessellated() - n, curves_plotted() - m), (3, 3));
    // A zoom: every curve again.
    let (n, m) = (curves_tessellated(), curves_plotted());
    stroke_triangles(&ctx, &wb, &transform(-4.0, 4.0, -3.0, 3.0), (None, None));
    assert_eq!((curves_tessellated() - n, curves_plotted() - m), (3, 3));
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

/// The polylines of every curve as `Curves` hands them out in one frame,
/// relative to the area.
fn drawn_paths(ctx: &Context, wb: &Workbench, t: &ViewTransform) -> Vec<(Id, Vec<Vec<Pos2>>)> {
    let mut paths = Vec::new();
    let mut out = ctx.run_ui(Default::default(), |ui| {
        let rect = Rect::from_min_size(ORIGIN, vec2(W, H));
        let curves = crate::curves::Curves::new(&ui.painter_at(rect), rect, t, wb.rebuilds());
        let c = wb.construction();
        for id in c.ids() {
            let color = wb.palette().color(id).egui();
            if let Some(drawn) = curves.get(c, id, color) {
                let moved = drawn
                    .paths()
                    .map(|p| p.into_iter().map(|q| q - ORIGIN.to_vec2()).collect())
                    .collect();
                paths.push((id, moved));
            }
        }
    });
    out.textures_delta.clear();
    paths
}

/// The largest distance from a point of `a` inside the area to the
/// polylines of `b`.
fn farthest(a: &[Vec<Pos2>], b: &[Vec<Pos2>]) -> f32 {
    let inside = |q: &Pos2| q.x >= 0.0 && q.x <= W && q.y >= 0.0 && q.y <= H;
    let mut worst = 0.0f32;
    for q in a.iter().flatten().filter(|q| inside(q)) {
        let mut best = f32::INFINITY;
        for w in b.iter().flat_map(|p| p.windows(2)) {
            let (s, e) = (w[0], w[1]);
            let d = e - s;
            let len2 = d.length_sq();
            let t = if len2 > 0.0 {
                ((*q - s).dot(d) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            best = best.min((s + t * d - *q).length());
        }
        worst = worst.max(best);
    }
    worst
}

#[test]
fn a_pan_that_moves_the_kept_drawings_shows_what_a_fresh_plot_shows() {
    let wb = Workbench::from_script(
        "F = (-2, 0)\nG = (2, 0)\nr = 3\nk = 1\ne = Ellipse(F, G, r)\nh = Hyperbola(F, G, k)\n\
         L = (0, 1)\nM = (-1, -1)\nN = (1, -1)\nd = Line(M, N)\np = Parabola(L, d)\n\
         f(x) = 1 / x\ng(x) = sin(3x) + x / 2\n",
    )
    .unwrap();
    let ctx = Context::default();
    drawn_paths(&ctx, &wb, &standard());
    for (dx, dy) in [(0.37, 0.0), (-1.13, 0.71), (1.9, -1.4)] {
        let t = transform(-8.0 + dx, 8.0 + dx, -6.0 + dy, 6.0 + dy);
        let (p, n) = (curves_plotted(), curves_tessellated());
        let moved = drawn_paths(&ctx, &wb, &t);
        assert_eq!(
            (curves_plotted() - p, curves_tessellated() - n),
            (0, 0),
            "the pan by ({dx}, {dy}) moved the kept drawings"
        );
        crate::curves::forget(&ctx);
        let fresh = drawn_paths(&ctx, &wb, &t);
        crate::curves::forget(&ctx);
        drawn_paths(&ctx, &wb, &standard());
        assert_eq!(moved.len(), fresh.len());
        let c = wb.construction();
        for ((id, a), (id2, b)) in moved.iter().zip(&fresh) {
            assert_eq!(id, id2);
            // Each is within its chord limit of the curve; their vertices
            // lie on it: a conic's chords stay 0.25 px from the curve,
            // `CurvePlotter`'s are at most 10 px long and bend at most 10
            // degrees, 0.44 px.
            let limit = match c.value(*id) {
                geo_kernel::kernel::Value::Function => 0.44,
                _ => geo_kernel::plot::conic::FLATNESS as f32,
            } + 1e-3;
            let d = farthest(a, b).max(farthest(b, a));
            assert!(
                d <= limit,
                "{} after a pan by ({dx}, {dy}): {d} px from a fresh plot",
                c.label(*id)
            );
            assert!(
                a.iter().flatten().count() > 10,
                "{} drew little",
                c.label(*id)
            );
        }
    }
}

/// Asks `Curves` for every curve of `wb` in one frame of an area of
/// `size`, in `color` when given (the palette's otherwise).
fn touch(
    ctx: &Context,
    wb: &Workbench,
    t: &ViewTransform,
    size: eframe::egui::Vec2,
    color: Option<eframe::egui::Color32>,
) {
    let mut out = ctx.run_ui(Default::default(), |ui| {
        let rect = Rect::from_min_size(ORIGIN, size);
        let curves = crate::curves::Curves::new(&ui.painter_at(rect), rect, t, wb.rebuilds());
        let c = wb.construction();
        for id in c.ids() {
            let col = color.unwrap_or_else(|| wb.palette().color(id).egui());
            let _ = curves.get(c, id, col);
        }
    });
    out.textures_delta.clear();
}

/// How many curves one frame plots.
fn plotted_by(f: impl FnOnce()) -> u64 {
    let n = curves_plotted();
    f();
    curves_plotted() - n
}

fn area() -> eframe::egui::Vec2 {
    vec2(W, H)
}

#[test]
fn a_function_is_plotted_again_when_what_it_reads_through_others_changes() {
    // g reads f, f reads r: a change of r reaches g.
    let mut wb =
        Workbench::from_script("r = 2\nf(x) = r x\ng(x) = f(x) + 1\nh(x) = x^2\n").unwrap();
    let ctx = Context::default();
    let t = standard();
    touch(&ctx, &wb, &t, area(), None);
    assert_eq!(plotted_by(|| touch(&ctx, &wb, &t, area(), None)), 0);
    let r = wb.construction().id("r").unwrap();
    assert!(wb.set_number(r, 3.0));
    assert_eq!(
        plotted_by(|| touch(&ctx, &wb, &t, area(), None)),
        2,
        "f and g"
    );
}

#[test]
fn a_function_reading_a_point_coordinate_follows_the_point() {
    let mut wb = Workbench::from_script("A = (1, 2)\nf(x) = x(A) x\ng(x) = x^2\n").unwrap();
    let ctx = Context::default();
    let t = standard();
    touch(&ctx, &wb, &t, area(), None);
    assert_eq!(plotted_by(|| touch(&ctx, &wb, &t, area(), None)), 0);
    let a = wb.construction().id("A").unwrap();
    assert!(wb.move_point(a, 2.0, 2.0));
    assert_eq!(
        plotted_by(|| touch(&ctx, &wb, &t, area(), None)),
        1,
        "f only"
    );
}

#[test]
fn a_function_reading_a_list_is_kept_while_the_list_stays() {
    // A list in a function's own expression is not supported yet; f reads
    // the list through a number.
    let mut wb =
        Workbench::from_script("a = 1\nl = {a, 2}\nn = Sum(l)\nf(x) = n x\ng(x) = x^2\n").unwrap();
    let ctx = Context::default();
    let t = standard();
    touch(&ctx, &wb, &t, area(), None);
    let c = wb.construction();
    assert!(
        matches!(
            c.value(c.id("l").unwrap()),
            geo_kernel::kernel::Value::List { .. }
        ),
        "l is a list"
    );
    for _ in 0..3 {
        assert_eq!(plotted_by(|| touch(&ctx, &wb, &t, area(), None)), 0, "idle");
    }
    // One element changes: f only.
    let a = wb.construction().id("a").unwrap();
    assert!(wb.set_number(a, 3.0));
    assert_eq!(
        plotted_by(|| touch(&ctx, &wb, &t, area(), None)),
        1,
        "f only"
    );
    assert_eq!(plotted_by(|| touch(&ctx, &wb, &t, area(), None)), 0, "idle");
}

#[test]
fn a_function_with_a_condition_follows_what_the_condition_reads() {
    let mut wb = Workbench::from_script("a = 1\nf(x) = If(a > 0, x, -x)\ng(x) = x^2\n").unwrap();
    let ctx = Context::default();
    let t = standard();
    touch(&ctx, &wb, &t, area(), None);
    assert_eq!(plotted_by(|| touch(&ctx, &wb, &t, area(), None)), 0);
    let a = wb.construction().id("a").unwrap();
    assert!(wb.set_number(a, -1.0));
    assert_eq!(
        plotted_by(|| touch(&ctx, &wb, &t, area(), None)),
        1,
        "f only"
    );
}

#[test]
fn the_area_size_pixel_density_and_colour_are_part_of_the_key() {
    // More curves than one frame plots again for a view change: a change of
    // the key plots every one at once.
    let script: String = (1..=30).map(|j| format!("f{j}(x) = x + {j}\n")).collect();
    let wb = Workbench::from_script(&script).unwrap();
    let ctx = Context::default();
    let t = standard();
    touch(&ctx, &wb, &t, area(), None);
    assert_eq!(plotted_by(|| touch(&ctx, &wb, &t, area(), None)), 0);
    // A taller area, the same view.
    let taller = vec2(W, H + 40.0);
    assert_eq!(
        plotted_by(|| touch(&ctx, &wb, &t, taller, None)),
        30,
        "size"
    );
    assert_eq!(plotted_by(|| touch(&ctx, &wb, &t, taller, None)), 0);
    // Twice the pixel density: the strokes are tessellated for it.
    ctx.set_pixels_per_point(2.0);
    assert_eq!(
        plotted_by(|| touch(&ctx, &wb, &t, taller, None)),
        30,
        "density"
    );
    assert_eq!(plotted_by(|| touch(&ctx, &wb, &t, taller, None)), 0);
    // Another colour.
    let red = Some(eframe::egui::Color32::RED);
    assert_eq!(
        plotted_by(|| touch(&ctx, &wb, &t, taller, red)),
        30,
        "colour"
    );
    assert_eq!(plotted_by(|| touch(&ctx, &wb, &t, taller, red)), 0);
}
