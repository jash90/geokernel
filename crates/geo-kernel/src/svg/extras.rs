use crate::algo::charts::{ChartMark, Pie};
use crate::kernel::image::RasterAsset;
use crate::{Construction, Id};
use std::f64::consts::TAU;
use std::fmt::Write;

pub(super) fn image_source(out: &mut String, id: usize, asset: &RasterAsset) {
    let [width, height] = asset.size();
    let bytes = asset.bytes();
    let format = if bytes.starts_with(b"GIF") {
        "gif"
    } else if bytes.starts_with(b"\x89PNG") {
        "png"
    } else {
        "jpeg"
    };
    let _ = write!(
        out,
        r#"<image id="raster-{id}" width="{width}" height="{height}" href="data:image/{format};base64,"#
    );
    crate::base64::append(out, bytes);
    out.push_str("\"/>\n");
}

pub(super) fn image(
    out: &mut String,
    c: &Construction,
    id: Id,
    asset: usize,
    scale: f64,
    screen: impl Fn((f64, f64)) -> (f64, f64),
) {
    if !c.value(id).is_defined() {
        return;
    }
    let Some(image) = c.image(id) else { return };
    let Some([a, b, _, d]) = c.image_quad(id, scale) else {
        return;
    };
    let point = |[x, y]: [f64; 2]| {
        if image.placement.is_screen() {
            (x, y)
        } else {
            screen((x, y))
        }
    };
    let [(ax, ay), (bx, by), (dx, dy)] = [a, b, d].map(point);
    if ![ax, ay, bx, by, dx, dy].into_iter().all(f64::is_finite) {
        return;
    }
    let [w, h] = image.asset.size().map(f64::from);
    let Some([x, y, right, bottom]) = image.source_rect() else {
        return;
    };
    let (width, height) = (right - x, bottom - y);
    let (rx, ry, sx, sy) = ((bx - ax) / w, (by - ay) / w, (dx - ax) / h, (dy - ay) / h);
    let tx = ax + rx * x + sx * y;
    let ty = ay + ry * x + sy * y;
    let label = super::escape(c.label(id));
    let alpha = image.style.alpha;
    let rendering = if image.style.interpolate {
        "auto"
    } else {
        "pixelated"
    };
    let _ = writeln!(
        out,
        r##"<g data-label="{label}" opacity="{alpha}" transform="matrix({rx} {ry} {sx} {sy} {tx} {ty})"><svg width="{width}" height="{height}" viewBox="{x} {y} {width} {height}" overflow="hidden"><use href="#raster-{asset}" image-rendering="{rendering}"/></svg></g>"##
    );
}

pub(super) fn chart(
    out: &mut String,
    c: &Construction,
    id: Id,
    scale: f64,
    screen: impl Fn((f64, f64)) -> (f64, f64),
) {
    let Some(chart) = c.chart(id).filter(|_| c.value(id).is_defined()) else {
        return;
    };
    let style = c.drawing_style(id);
    let thickness = f64::from(style.line_thickness) * 0.5;
    let label = super::escape(c.label(id));
    let _ = writeln!(
        out,
        r##"<g data-label="{label}" stroke="#2e7d32" stroke-width="{thickness}" fill="none">"##
    );
    if let Some(pie) = &chart.pie {
        let (x, y) = screen(pie.center);
        let radius = pie.radius * scale;
        if radius > 0.0 && [x, y, radius].into_iter().all(f64::is_finite) {
            let at = |turn: f64| {
                (
                    x + radius * (turn * TAU).cos(),
                    y + radius * (turn * TAU).sin(),
                )
            };
            for (index, start, fraction) in pie.slices().filter(|&(_, _, fraction)| fraction > 0.0)
            {
                let [r, g, b] = Pie::slice_color(index);
                let (ax, ay) = at(start);
                let (mx, my) = at(start + 0.5 * fraction);
                let (bx, by) = at(start + fraction);
                let _ = writeln!(
                    out,
                    r##"<path fill="#{r:02x}{g:02x}{b:02x}" stroke="none" d="M {x:.2} {y:.2} L {ax:.2} {ay:.2} A {radius:.2} {radius:.2} 0 0 1 {mx:.2} {my:.2} A {radius:.2} {radius:.2} 0 0 1 {bx:.2} {by:.2} Z"/>"##
                );
            }
            if thickness > 0.0 {
                let _ = writeln!(out, r#"<circle cx="{x:.2}" cy="{y:.2}" r="{radius:.2}"/>"#);
                for (_, start, fraction) in pie.slices() {
                    line(out, (x, y), at(start + fraction));
                }
            }
            if style.show_label {
                let offset = radius * std::f64::consts::FRAC_1_SQRT_2;
                let _ = writeln!(
                    out,
                    r##"<text x="{:.2}" y="{:.2}" fill="#202124" stroke="none" font-family="sans-serif" font-size="14">{label}</text>"##,
                    x + offset,
                    y + offset + 14.0
                );
            }
        }
    } else {
        for mark in chart.render_marks(scale, scale) {
            match mark {
                ChartMark::Bar {
                    left,
                    right,
                    height,
                } => {
                    if left >= right {
                        line(out, screen((left, 0.0)), screen((left, height)));
                    } else if height != 0.0 {
                        rect(
                            out,
                            screen((left, 0.0)),
                            screen((right, height)),
                            "#2e7d32",
                            0.24,
                        );
                    }
                }
                ChartMark::Box {
                    x: [left, right],
                    y: [bottom, top],
                } => {
                    rect(
                        out,
                        screen((left, bottom)),
                        screen((right, top)),
                        "none",
                        1.0,
                    );
                }
                ChartMark::Segment([a, b]) => line(out, screen(a), screen(b)),
                ChartMark::Point { at, open } => {
                    let (x, y) = screen(at);
                    let fill = if open { "none" } else { "#2e7d32" };
                    let r = if chart.dot_stack.is_some() { 5 } else { 4 };
                    if x.is_finite() && y.is_finite() {
                        let _ = writeln!(
                            out,
                            r#"<circle cx="{x:.2}" cy="{y:.2}" r="{r}" fill="{fill}"/>"#
                        );
                    }
                }
                ChartMark::Cross(at) => {
                    let (x, y) = screen(at);
                    for d in [-4.0, 4.0] {
                        line(out, (x - 4.0, y - d), (x + 4.0, y + d));
                    }
                }
            }
        }
    }
    out.push_str("</g>\n");
}

fn rect(out: &mut String, (x, y): (f64, f64), (u, v): (f64, f64), fill: &str, alpha: f64) {
    if [x, y, u, v].into_iter().all(f64::is_finite) {
        let _ = writeln!(
            out,
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{fill}" fill-opacity="{alpha}"/>"#,
            x.min(u),
            y.min(v),
            (x - u).abs(),
            (y - v).abs()
        );
    }
}

fn line(out: &mut String, (x, y): (f64, f64), (u, v): (f64, f64)) {
    if [x, y, u, v].into_iter().all(f64::is_finite) {
        let _ = writeln!(
            out,
            r#"<line x1="{x:.2}" y1="{y:.2}" x2="{u:.2}" y2="{v:.2}"/>"#
        );
    }
}
