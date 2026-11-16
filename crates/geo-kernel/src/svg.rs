//! Draws a construction as an SVG image.

use crate::algo::vector::start;
use crate::kernel::{Construction, Value};
use std::fmt::Write;

/// The rectangle of the plane that is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub xmin: f64,
    pub ymin: f64,
    pub xmax: f64,
    pub ymax: f64,
}

impl View {
    /// A view that contains every point, segment and circle, with a margin.
    pub fn fit(c: &Construction) -> View {
        let mut v = View {
            xmin: f64::INFINITY,
            ymin: f64::INFINITY,
            xmax: f64::NEG_INFINITY,
            ymax: f64::NEG_INFINITY,
        };
        let mut grow = |x: f64, y: f64| {
            v.xmin = v.xmin.min(x);
            v.ymin = v.ymin.min(y);
            v.xmax = v.xmax.max(x);
            v.ymax = v.ymax.max(y);
        };
        for id in c.ids() {
            match c.value(id) {
                Value::Point { x, y } => grow(x, y),
                Value::Segment { x1, y1, x2, y2 } => {
                    grow(x1, y1);
                    grow(x2, y2);
                }
                Value::Circle { cx, cy, r } => {
                    grow(cx - r, cy - r);
                    grow(cx + r, cy + r);
                }
                Value::Ray { x, y, .. } => grow(x, y),
                Value::Polygon { .. } | Value::PolyLine { .. } => {
                    for (x, y) in c.vertices(id).flatten() {
                        grow(x, y);
                    }
                }
                Value::Vector { x, y } => {
                    if let Some((x0, y0)) = start(c, id) {
                        grow(x0, y0);
                        grow(x0 + x, y0 + y);
                    }
                }
                Value::Arc { .. } | Value::Sector { .. } => {
                    if let Some(a) = c.arc(id) {
                        for (x, y) in arc_path(&a) {
                            grow(x, y);
                        }
                    }
                }
                _ => {}
            }
        }
        if !v.xmin.is_finite() {
            return View {
                xmin: -5.0,
                ymin: -5.0,
                xmax: 5.0,
                ymax: 5.0,
            };
        }
        let pad = (0.08 * (v.xmax - v.xmin).max(v.ymax - v.ymin)).max(0.5);
        View {
            xmin: v.xmin - pad,
            ymin: v.ymin - pad,
            xmax: v.xmax + pad,
            ymax: v.ymax + pad,
        }
    }

    /// The smallest view containing both.
    pub fn union(self, o: View) -> View {
        View {
            xmin: self.xmin.min(o.xmin),
            ymin: self.ymin.min(o.ymin),
            xmax: self.xmax.max(o.xmax),
            ymax: self.ymax.max(o.ymax),
        }
    }
}

/// Renders the construction into an SVG document of the given pixel size.
pub fn render(c: &Construction, view: View, width: u32, height: u32) -> String {
    let (w, h) = (width as f64, height as f64);
    let (vw, vh) = (view.xmax - view.xmin, view.ymax - view.ymin);
    let scale = (w / vw).min(h / vh);
    // Centre the view inside the image.
    let ox = 0.5 * (w - vw * scale);
    let oy = 0.5 * (h - vh * scale);
    let sx = |x: f64| ox + (x - view.xmin) * scale;
    let sy = |y: f64| h - oy - (y - view.ymin) * scale;

    let mut lines = String::new();
    let mut polygons = String::new();
    let mut segments = String::new();
    let mut circles = String::new();
    let mut points = String::new();

    // Used to draw an "infinite" line as a segment longer than the view.
    let (mx, my) = (0.5 * (view.xmin + view.xmax), 0.5 * (view.ymin + view.ymax));
    let reach = vw.hypot(vh);
    // Conics and function graphs, cut to the image.
    let frame = crate::plot::Frame::fit(view, w, h);

    for id in c.ids() {
        match c.value(id) {
            v @ Value::Line { .. } => {
                let Some((a, b, k)) = v.unit_line() else {
                    continue;
                };
                let d = a * mx + b * my + k;
                let (fx, fy) = (mx - a * d, my - b * d);
                let _ = writeln!(
                    lines,
                    r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}"/>"##,
                    sx(fx - reach * b),
                    sy(fy + reach * a),
                    sx(fx + reach * b),
                    sy(fy - reach * a),
                );
            }
            Value::Segment { x1, y1, x2, y2 } => {
                let _ = writeln!(
                    segments,
                    r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}"/>"##,
                    sx(x1),
                    sy(y1),
                    sx(x2),
                    sy(y2),
                );
            }
            Value::Circle { cx, cy, r } => {
                let _ = writeln!(
                    circles,
                    r##"<circle cx="{:.2}" cy="{:.2}" r="{:.2}"/>"##,
                    sx(cx),
                    sy(cy),
                    r * scale,
                );
            }
            Value::Point { x, y } => {
                let free = c.def(id).is_free();
                let (fill, radius) = if free {
                    ("#1a73e8", 5.0)
                } else {
                    ("#5f6368", 4.0)
                };
                let _ = writeln!(
                    points,
                    r##"<circle cx="{px:.2}" cy="{py:.2}" r="{radius}" fill="{fill}" stroke="#ffffff" stroke-width="1"/><text x="{tx:.2}" y="{ty:.2}">{label}</text>"##,
                    px = sx(x),
                    py = sy(y),
                    tx = sx(x) + 8.0,
                    ty = sy(y) - 8.0,
                    label = escape(c.label(id)),
                );
            }
            Value::Ray { x, y, a, b, .. } => {
                // A half-line from the start point along (b, -a), long
                // enough to leave the view.
                let far = (reach + (x - mx).hypot(y - my)) / a.hypot(b);
                let _ = writeln!(
                    lines,
                    r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}"/>"##,
                    sx(x),
                    sy(y),
                    sx(x + far * b),
                    sy(y - far * a),
                );
            }
            Value::Vector { x: vx, y: vy } => {
                let Some((x0, y0)) = start(c, id) else {
                    continue;
                };
                let (px, py) = (sx(x0), sy(y0));
                let (qx, qy) = (sx(x0 + vx), sy(y0 + vy));
                let _ = writeln!(
                    segments,
                    r##"<line x1="{px:.2}" y1="{py:.2}" x2="{qx:.2}" y2="{qy:.2}"/>"##,
                );
                // Arrow head at the end point, in pixels.
                let len = (qx - px).hypot(qy - py);
                if len > 0.0 {
                    let (ux, uy) = ((qx - px) / len, (qy - py) / len);
                    let (bx, by) = (qx - 10.0 * ux, qy - 10.0 * uy);
                    let _ = writeln!(
                        segments,
                        r##"<polygon points="{qx:.2},{qy:.2} {:.2},{:.2} {:.2},{:.2}" fill="#202124"/>"##,
                        bx - 4.0 * uy,
                        by + 4.0 * ux,
                        bx + 4.0 * uy,
                        by - 4.0 * ux,
                    );
                }
            }
            Value::Polygon { .. } => {
                let mut pts = String::new();
                for (x, y) in c.vertices(id).flatten() {
                    let _ = write!(pts, "{:.2},{:.2} ", sx(x), sy(y));
                }
                let _ = writeln!(polygons, r##"<polygon points="{}"/>"##, pts.trim_end());
            }
            Value::PolyLine { .. } => {
                // An undefined point leaves a hole, as in GeoGebra's drawing.
                let mut pts = String::new();
                for v in c.vertices(id).chain([None]) {
                    match v {
                        Some((x, y)) => {
                            let _ = write!(pts, "{:.2},{:.2} ", sx(x), sy(y));
                        }
                        None if !pts.is_empty() => {
                            let _ = writeln!(
                                segments,
                                r##"<polyline points="{}" fill="none"/>"##,
                                pts.trim_end()
                            );
                            pts.clear();
                        }
                        None => {}
                    }
                }
            }
            Value::Conic { .. } => {
                let Some(d) = c.conic(id) else {
                    continue;
                };
                let drawn = crate::plot::conic::plot(&d, &frame);
                write_paths(&mut segments, &drawn.paths);
                if let Some((px, py)) = drawn.point {
                    let _ = writeln!(
                        segments,
                        r##"<circle cx="{px:.2}" cy="{py:.2}" r="4" fill="#202124" stroke="none"/>"##,
                    );
                }
                // A dot of the stroke width (2).
                if let Some((px, py)) = drawn.dot {
                    let _ = writeln!(
                        segments,
                        r##"<circle cx="{px:.2}" cy="{py:.2}" r="1" fill="#202124" stroke="none"/>"##,
                    );
                }
            }
            Value::Function => {
                let mut f = |x: f64| c.function_value(id, x).unwrap_or(f64::NAN);
                let drawn = crate::plot::function::plot(&mut f, &frame);
                write_paths(&mut segments, &drawn.paths);
            }
            Value::Arc { .. } | Value::Sector { .. } => {
                let Some(a) = c.arc(id) else {
                    continue;
                };
                let mut pts = String::new();
                for (x, y) in arc_path(&a) {
                    let _ = write!(pts, "{:.2},{:.2} ", sx(x), sy(y));
                }
                let pts = pts.trim_end();
                if a.sector {
                    let _ = writeln!(polygons, r##"<polygon points="{pts}"/>"##);
                }
                let _ = writeln!(segments, r##"<polyline points="{pts}" fill="none"/>"##);
            }
            Value::Number(_) | Value::Bool(_) | Value::List { .. } | Value::Undefined => {}
        }
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"##
    );
    let _ = writeln!(
        out,
        r##"<rect width="100%" height="100%" fill="#ffffff"/>"##
    );
    let _ = writeln!(
        out,
        r##"<g fill="#8b5a2b" fill-opacity="0.1" stroke="none">"##
    );
    out.push_str(&polygons);
    let _ = writeln!(out, "</g>");
    let _ = writeln!(out, r##"<g stroke="#9aa0a6" stroke-width="1">"##);
    out.push_str(&lines);
    let _ = writeln!(out, "</g>");
    let _ = writeln!(
        out,
        r##"<g stroke="#1a73e8" stroke-width="1.5" fill="none">"##
    );
    out.push_str(&circles);
    let _ = writeln!(out, "</g>");
    let _ = writeln!(out, r##"<g stroke="#202124" stroke-width="2">"##);
    out.push_str(&segments);
    let _ = writeln!(out, "</g>");
    let _ = writeln!(
        out,
        r##"<g font-family="sans-serif" font-size="14" fill="#202124">"##
    );
    out.push_str(&points);
    let _ = writeln!(out, "</g>");
    let _ = writeln!(out, "</svg>");
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The points of an arc or sector, sampled from its start to its end
/// parameter on its circle or ellipse; a sector also runs through the
/// centre at both ends. A part that degenerated to a segment is that
/// segment; any other degenerate part gives no points (an arc of two rays
/// has no finite value and is not drawn).
fn arc_path(a: &crate::algo::arc::ArcData) -> Vec<(f64, f64)> {
    use crate::algo::conic_nd::ConicType;
    const N: usize = 120;
    let d = &a.conic;
    match d.ty {
        ConicType::Circle | ConicType::Ellipse => {
            let [e0, e1] = d.eigenvec;
            let [bx, by] = d.midpoint;
            let [ha, hb] = d.half_axes;
            // more samples for a longer part, at least a few
            let n = ((N as f64 * a.extent / std::f64::consts::TAU).ceil() as usize).max(4);
            let mut path: Vec<(f64, f64)> = (0..=n)
                .map(|i| {
                    let t = a.start + a.extent * i as f64 / n as f64;
                    let (u, v) = (ha * t.cos(), hb * t.sin());
                    (bx + u * e0[0] + v * e1[0], by + u * e0[1] + v * e1[1])
                })
                .collect();
            if a.sector {
                path.insert(0, (bx, by));
                path.push((bx, by));
            }
            path
        }
        ConicType::ParallelLines if !a.sector && a.positive => {
            a.segment().map(|[p, q]| vec![p, q]).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// Writes each path, already in pixels, as an open polyline.
fn write_paths(out: &mut String, paths: &[crate::plot::Path]) {
    for path in paths {
        let mut pts = String::new();
        for &(x, y) in path {
            let _ = write!(pts, "{x:.2},{y:.2} ");
        }
        let _ = writeln!(
            out,
            r##"<polyline points="{}" fill="none"/>"##,
            pts.trim_end()
        );
    }
}
