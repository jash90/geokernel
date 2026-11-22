//! The kept drawings of `curves.rs` against plots of the area: what an idle
//! window shows, labels, degenerate views, deep zooms in one frame, and
//! what rebuilds of the construction plot again.

use crate::curves::{curves_plotted, Curves, REPLOTS_PER_FRAME};
use crate::geom::ToEgui;
use crate::interact::ViewTransform;
use crate::workbench::Workbench;
use eframe::egui::{vec2, Context, Pos2, Rect, Vec2};
use geo_kernel::kernel::{Construction, Value};
use geo_kernel::plot::{self, Frame};
use geo_kernel::svg::View;

const W: f32 = 800.0;
const H: f32 = 600.0;
const ORIGIN: Pos2 = Pos2::new(20.0, 10.0);

/// 40 pixels a unit on the 800 x 600 area, the middle at (cx, cy).
fn at(cx: f64, cy: f64, scale: f64) -> ViewTransform {
    at_size(cx, cy, scale, vec2(W, H))
}

fn at_size(cx: f64, cy: f64, scale: f64, size: Vec2) -> ViewTransform {
    let (hw, hh) = (0.5 * size.x as f64 / scale, 0.5 * size.y as f64 / scale);
    ViewTransform::new(
        View {
            xmin: cx - hw,
            ymin: cy - hh,
            xmax: cx + hw,
            ymax: cy + hh,
        },
        size.x as f64,
        size.y as f64,
    )
}

/// One curve as one frame drew it, relative to the area.
struct Shown {
    label: String,
    paths: Vec<Vec<Pos2>>,
    vertices: Vec<Pos2>,
    /// The mesh's triangles, as indices into `vertices`.
    indices: Vec<u32>,
    text: Option<(f64, f64)>,
}

/// What one frame drew of the curves of `wb`, and whether it asked for the
/// next frame at once.
fn frame(ctx: &Context, wb: &Workbench, t: &ViewTransform, size: Vec2) -> (Vec<Shown>, bool) {
    frame_with(ctx, wb, t, size, ORIGIN, Default::default())
}

/// [`frame`] with the area at `origin` on the screen and the input `input`;
/// the paths and the mesh are given relative to the area.
fn frame_with(
    ctx: &Context,
    wb: &Workbench,
    t: &ViewTransform,
    size: Vec2,
    origin: Pos2,
    input: eframe::egui::RawInput,
) -> (Vec<Shown>, bool) {
    let mut shown = Vec::new();
    let mut out = ctx.run_ui(input, |ui| {
        let rect = Rect::from_min_size(origin, size);
        let curves = Curves::new(&ui.painter_at(rect), rect, t, wb.rebuilds());
        let c = wb.construction();
        for id in c.ids() {
            let color = wb.palette().color(id).egui();
            if let Some(d) = curves.get(c, id, color) {
                shown.push(Shown {
                    label: c.label(id).to_owned(),
                    paths: d
                        .paths()
                        .map(|p| p.into_iter().map(|q| q - origin.to_vec2()).collect())
                        .collect(),
                    vertices: d
                        .mesh
                        .vertices
                        .iter()
                        .map(|v| d.mesh_transform * v.pos - origin.to_vec2())
                        .collect(),
                    indices: d.mesh.indices.clone(),
                    text: d.label,
                });
            }
        }
    });
    let immediate = crate::headless::repaint(&out) == crate::headless::Repaint::Immediate;
    out.textures_delta.clear();
    (shown, immediate)
}

/// Frames until one asks for no immediate repaint (at most `max`); the
/// last frame's drawing and how many frames ran.
fn settle(ctx: &Context, wb: &Workbench, t: &ViewTransform, max: usize) -> (Vec<Shown>, usize) {
    for n in 1..=max {
        let (shown, again) = frame(ctx, wb, t, vec2(W, H));
        if !again {
            return (shown, n);
        }
    }
    panic!("still asking for frames after {max}");
}

/// The polylines and label a plot of exactly the area gives `label`.
fn area_plot(
    c: &Construction,
    label: &str,
    t: &ViewTransform,
) -> (Vec<Vec<Pos2>>, Option<(f64, f64)>) {
    let id = c.id(label).unwrap();
    let frame = Frame::fit(t.view(), W as f64, H as f64);
    let to = |path: Vec<(f64, f64)>| {
        path.into_iter()
            .map(|(x, y)| Pos2::new(x as f32, y as f32))
            .collect()
    };
    match c.value(id) {
        Value::Conic { .. } => {
            let p = plot::conic::plot(&c.conic(id).unwrap(), &frame);
            (p.paths.into_iter().map(to).collect(), p.label)
        }
        _ => {
            let mut f = |x: f64| c.function_value(id, x).unwrap_or(f64::NAN);
            let p = plot::function::plot(&mut f, &frame);
            (p.paths.into_iter().map(to).collect(), p.label)
        }
    }
}

/// The largest distance from a vertex of `a` in the area to the polylines
/// of `b`.
fn farthest(a: &[Vec<Pos2>], b: &[Vec<Pos2>]) -> f32 {
    farthest_above(a, b, H)
}

/// [`farthest`] for the vertices of `a` in the area above `y_max`.
fn farthest_above(a: &[Vec<Pos2>], b: &[Vec<Pos2>], y_max: f32) -> f32 {
    let inside = |q: &&Pos2| (0.0..=W).contains(&q.x) && (0.0..=y_max).contains(&q.y);
    let mut worst = 0.0f32;
    for q in a.iter().flatten().filter(inside) {
        let mut best = f32::INFINITY;
        for w in b.iter().flat_map(|p| p.windows(2)) {
            let (s, d) = (w[0], w[1] - w[0]);
            let len2 = d.length_sq();
            let t = if len2 > 0.0 {
                ((*q - s).dot(d) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            best = best.min((s + t * d - *q).length());
        }
        if b.iter().all(|p| p.len() == 1) {
            best = b
                .iter()
                .flatten()
                .map(|p| (*p - *q).length())
                .fold(best, f32::min);
        }
        worst = worst.max(best);
    }
    worst
}

/// Asserts that `shown` is, curve by curve, what plots of exactly the area
/// give, to `tol` pixels for a function: the polylines both ways and the
/// label.
fn assert_area_plots(c: &Construction, shown: &[Shown], t: &ViewTransform, tol: f32, what: &str) {
    for id in c.ids() {
        if !matches!(c.value(id), Value::Conic { .. } | Value::Function) {
            continue;
        }
        let label = c.label(id);
        let (want, text) = area_plot(c, label, t);
        let got = shown.iter().find(|s| s.label == label);
        let (paths, got_text) = got.map_or((Vec::new(), None), |s| (s.paths.clone(), s.text));
        // Strokes kept outside the area (in the plotted margin) do not show.
        // A conic is sampled on the frame with the margin: its chords start
        // elsewhere, so it is within the flatness of a plot of the area.
        let tol = match c.value(id) {
            Value::Conic { .. } => tol.max(plot::conic::FLATNESS as f32 + 1e-3),
            _ => tol,
        };
        let d = farthest(&paths, &want).max(farthest(&want, &paths));
        assert!(
            d <= tol,
            "{what}: {label} is {d} px from a plot of the area"
        );
        assert_eq!(got_text, text, "{what}: the label of {label}");
    }
}

#[test]
fn an_idle_window_shows_plots_of_the_area_whatever_the_pans_before() {
    let wb =
        Workbench::from_script("f(x) = ln(x)\ng(x) = sin(3x) + x / 2\nh(x) = 1 / x\n").unwrap();
    let c = wb.construction();
    for dx in [10.0_f64, 37.0, 60.0, 99.7, -50.0] {
        let ctx = Context::default();
        settle(&ctx, &wb, &at(0.0, 0.0, 40.0), 60);
        // The content moves right by `dx` pixels, in steps of 4.
        let steps = (dx / 4.0).abs().ceil() as usize;
        for k in 1..=steps {
            let moved = dx * k as f64 / steps as f64;
            frame(&ctx, &wb, &at(-moved / 40.0, 0.0, 40.0), vec2(W, H));
        }
        let t = at(-dx / 40.0, 0.0, 40.0);
        let (shown, _) = settle(&ctx, &wb, &t, 60);
        assert_area_plots(c, &shown, &t, 1e-3, &format!("after a pan by {dx} px"));
    }
}

#[test]
fn a_label_stays_when_the_outline_passes_outside_the_area() {
    // The outline passes 17.6 px outside the area; upstream draws the label.
    let wb =
        Workbench::from_script("F = (-17.17, 0)\nG = (17.17, 0)\na = 19.5\ne = Ellipse(F, G, a)\n")
            .unwrap();
    let c = wb.construction();
    let t = at(0.0, 0.0, 40.0);
    let (want, text) = area_plot(c, "e", &t);
    assert!(want.is_empty() && text.is_some(), "{want:?} {text:?}");
    let (shown, _) = settle(&Context::default(), &wb, &t, 10);
    assert_area_plots(c, &shown, &t, 1e-3, "the wide ellipse");
}

#[test]
fn a_degenerate_view_draws_plots_and_asks_for_nothing() {
    let wb = Workbench::from_script(
        "f(x) = sin(x)\ng(x) = x^2\nF = (0, 0)\nG = (1, 0)\na = 2\ne = Ellipse(F, G, a)\n",
    )
    .unwrap();
    let ctx = Context::default();
    settle(&ctx, &wb, &at(0.0, 0.0, 40.0), 10);
    for (cx, scale) in [(1e10, 1e9), (1e16, 1e3)] {
        let t = at(cx, cx, scale);
        let n = curves_plotted();
        for _ in 0..5 {
            let (shown, again) = frame(&ctx, &wb, &t, vec2(W, H));
            assert!(shown.is_empty(), "drew {} curves", shown.len());
            assert!(
                !again,
                "asked for another frame at ({cx}, {cx}) and scale {scale}"
            );
        }
        assert_eq!(
            curves_plotted(),
            n,
            "plotted at ({cx}, {cx}) and scale {scale}"
        );
    }
}

#[test]
fn deep_zooms_in_one_frame_hand_the_painter_bounded_vertices() {
    // More curves than one frame plots: the others are drawn from their kept
    // plots, or not at all.
    let script: String = (1..=REPLOTS_PER_FRAME + 8)
        .map(|j| format!("f{j}(x) = sin(x + {j}) + {j} / 10\n"))
        .collect();
    let wb = Workbench::from_script(&script).unwrap();
    for (from, to) in [(40.0, 1e-6), (1e-6, 1e9), (40.0, 1e9), (1e9, 40.0)] {
        let ctx = Context::default();
        settle(&ctx, &wb, &at(0.0, 0.0, from), 60);
        let (shown, _) = frame(&ctx, &wb, &at(0.0, 0.0, to), vec2(W, H));
        for s in &shown {
            for v in &s.vertices {
                assert!(
                    v.x.is_finite() && v.y.is_finite() && v.x.abs() < 1e5 && v.y.abs() < 1e5,
                    "{}: a vertex at {v:?} after a jump from {from} to {to}",
                    s.label
                );
            }
        }
    }
}

/// How many curves one frame of `wb` in the standard view plots.
fn plots(ctx: &Context, wb: &Workbench) -> u64 {
    let n = curves_plotted();
    frame(ctx, wb, &at(0.0, 0.0, 40.0), vec2(W, H));
    curves_plotted() - n
}

#[test]
fn rebuilds_plot_again_only_the_curves_whose_drawing_changed() {
    let mut script = String::from("A = (1, 1)\nB = (2, 2)\nr = 2\n");
    for j in 1..=REPLOTS_PER_FRAME + 6 {
        script += &format!("f{j}(x) = r sin(x + {j})\n");
    }
    script += "g(x) = x(B) x\n";
    let mut wb = Workbench::from_script(&script).unwrap();
    let ctx = Context::default();
    settle(&ctx, &wb, &at(0.0, 0.0, 40.0), 60);
    assert_eq!(plots(&ctx, &wb), 0, "idle");
    // A point no curve reads.
    wb.run_line("P = (3, 3)").unwrap();
    assert_eq!(plots(&ctx, &wb), 0, "add a point");
    assert!(wb.undo());
    assert_eq!(plots(&ctx, &wb), 0, "undo");
    assert!(wb.redo());
    assert_eq!(plots(&ctx, &wb), 0, "redo");
    // An object before all the curves goes: every later id moves.
    let a = wb.construction().id("A").unwrap();
    assert!(wb.delete(a));
    assert_eq!(plots(&ctx, &wb), 0, "delete A");
    assert!(wb.undo());
    assert_eq!(plots(&ctx, &wb), 0, "undo the deletion");
    // B goes and g with it.
    let b = wb.construction().id("B").unwrap();
    assert!(wb.delete(b));
    assert_eq!(plots(&ctx, &wb), 0, "delete B and g");
    assert!(wb.undo());
    assert_eq!(plots(&ctx, &wb), 0, "g is back as it was");
    // The label f1 comes back with another definition, then with the same
    // one again.
    let f1 = wb.construction().id("f1").unwrap();
    assert!(wb.delete(f1));
    assert_eq!(plots(&ctx, &wb), 0, "delete f1");
    wb.run_line("f1(x) = r cos(x)").unwrap();
    assert_eq!(plots(&ctx, &wb), 1, "f1 defined anew");
    let f1 = wb.construction().id("f1").unwrap();
    assert!(wb.delete(f1));
    wb.run_line("f1(x) = r cos(x)").unwrap();
    assert_eq!(plots(&ctx, &wb), 0, "f1 defined as it was");
}

#[test]
fn a_pan_moves_the_kept_drawings_within_the_flatness_of_a_plot_of_the_area() {
    let wb = Workbench::from_script(
        "F = (-2, 0)\nG = (2, 0)\nr = 3\nk = 1\ne = Ellipse(F, G, r)\nh = Hyperbola(F, G, k)\n\
         L = (0, 1)\nM = (-1, -1)\nN = (1, -1)\nd = Line(M, N)\np = Parabola(L, d)\n\
         A = (4, 4)\nB = (5, 2)\nC = (6, 0)\nD = (-5, -5)\nE = (-4, -5)\nq = Conic(A, B, C, D, E)\n\
         f(x) = 1 / x\ng(x) = sin(3x) + x / 2\nl(x) = ln(x)\n",
    )
    .unwrap();
    let c = wb.construction();
    for (dx, dy) in [(0.37, 0.0), (-1.13, 0.71), (1.9, -1.4), (0.0, 2.3)] {
        let ctx = Context::default();
        settle(&ctx, &wb, &at(0.0, 0.0, 40.0), 60);
        let t = at(dx, dy, 40.0);
        let n = curves_plotted();
        let (moved, _) = frame(&ctx, &wb, &t, vec2(W, H));
        assert_eq!(
            curves_plotted(),
            n,
            "the pan by ({dx}, {dy}) moved the kept drawings"
        );
        for id in c.ids() {
            let label = c.label(id);
            let limit = match c.value(id) {
                // `CurvePlotter`'s chords are at most 10 px long and bend
                // at most 10 degrees: 0.44 px from the graph.
                Value::Function => 0.44,
                Value::Conic { .. } => plot::conic::FLATNESS as f32,
                _ => continue,
            } + 1e-3;
            let (want, _) = area_plot(c, label, &t);
            let got = moved
                .iter()
                .find(|s| s.label == label)
                .map_or(Vec::new(), |s| s.paths.clone());
            // A graph that ends inside the view (`ln` near x = 0) ends where
            // the plot of the view it was made for ended it, which a plot
            // of this view may not; an idle window plots it again (see
            // above). Its points above the bottom quarter, away from that
            // end, are compared.
            let y_max = if label == "l" { 0.75 * H } else { H };
            let d = farthest_above(&got, &want, y_max).max(farthest_above(&want, &got, y_max));
            assert!(
                d <= limit,
                "{label} after a pan by ({dx}, {dy}): {d} px from a plot of the area"
            );
            assert!(got.iter().flatten().count() > 1, "{label} drew nothing");
        }
        let (shown, _) = settle(&ctx, &wb, &t, 60);
        assert_area_plots(
            c,
            &shown,
            &t,
            1e-3,
            &format!("idle after a pan by ({dx}, {dy})"),
        );
    }
}

#[test]
fn arcs_and_sectors_a_function_reads_are_keyed_by_value() {
    // A function that reads the length of an arc and one that reads the
    // area of a sector: an idle frame and a frame with a mouse move plot
    // neither; moving the arc's end point plots both again.
    let mut wb = Workbench::from_script(
        "M = (0, 0)\nA = (3, 0)\nB = (0, 3)\na1 = CircularArc(M, A, B)\n\
         s1 = CircularSector(M, A, B)\nL = Length(a1)\nS = Area(s1)\n\
         f(x) = L x\ng(x) = S x^2 - 5\n",
    )
    .unwrap();
    let ctx = Context::default();
    let t = at(0.0, 0.0, 40.0);
    settle(&ctx, &wb, &t, 60);
    assert_eq!(plots(&ctx, &wb), 0, "idle");
    let moved = eframe::egui::RawInput {
        events: vec![eframe::egui::Event::PointerMoved(Pos2::new(300.0, 200.0))],
        ..Default::default()
    };
    let n = curves_plotted();
    frame_with(&ctx, &wb, &t, vec2(W, H), ORIGIN, moved);
    assert_eq!(curves_plotted() - n, 0, "mouse move");
    assert_eq!(plots(&ctx, &wb), 0, "idle again");
    let b = wb.construction().id("B").unwrap();
    assert!(wb.move_point(b, 0.0, 4.0));
    assert_eq!(plots(&ctx, &wb), 2, "the arc changed");
    assert_eq!(plots(&ctx, &wb), 0);
}

#[test]
fn the_same_view_elsewhere_on_the_screen_is_drawn_there() {
    // The same size and camera with the area at two places on the screen:
    // relative to its area, each frame draws the same thing.
    let wb = Workbench::from_script(
        "F = (-2, 0)\nG = (2, 0)\na = 3\nk = Ellipse(F, G, a)\nf(x) = sin(x)\n",
    )
    .unwrap();
    let ctx = Context::default();
    let t = at(0.0, 0.0, 40.0);
    let (here, _) = settle(&ctx, &wb, &t, 60);
    let elsewhere = Pos2::new(220.0, 130.0);
    let (there, _) = frame_with(&ctx, &wb, &t, vec2(W, H), elsewhere, Default::default());
    let (back, _) = frame(&ctx, &wb, &t, vec2(W, H));
    assert_eq!(here.len(), 2);
    for frame in [&there, &back] {
        assert_eq!(frame.len(), here.len());
        for (a, b) in here.iter().zip(frame.iter()) {
            assert_eq!(a.label, b.label);
            let (pa, pb): (Vec<Pos2>, Vec<Pos2>) = (
                a.paths.iter().flatten().copied().collect(),
                b.paths.iter().flatten().copied().collect(),
            );
            assert_eq!(pa.len(), pb.len(), "{}", a.label);
            let off = pa
                .iter()
                .zip(&pb)
                .map(|(p, q)| (*p - *q).length())
                .fold(0.0f32, f32::max);
            assert!(off < 1e-3, "{}: {off} px off", a.label);
            assert_same_in_area(a, b);
        }
    }
}

/// The triangles of `s` that reach the area (within 1 px).
fn triangles_in_area(s: &Shown) -> Vec<[Pos2; 3]> {
    let area = Rect::from_min_size(Pos2::ZERO, vec2(W, H)).expand(1.0);
    s.indices
        .chunks(3)
        .map(|t| [0, 1, 2].map(|k| s.vertices[t[k] as usize]))
        .filter(|t| Rect::from_points(t).intersects(area))
        .collect()
}

/// `a` and `b` have the same triangles in the area, within 1e-3 px.
fn assert_same_in_area(a: &Shown, b: &Shown) {
    let (ta, tb) = (triangles_in_area(a), triangles_in_area(b));
    assert_eq!(ta.len(), tb.len(), "{}: triangles in the area", a.label);
    let mut used = vec![false; tb.len()];
    for t in &ta {
        let near = |u: &[Pos2; 3]| (0..3).all(|k| (u[k] - t[k]).length() < 1e-3);
        let k = (0..tb.len()).find(|&k| !used[k] && near(&tb[k]));
        let k = k.unwrap_or_else(|| panic!("{}: a triangle in the area is missing", a.label));
        used[k] = true;
    }
}
