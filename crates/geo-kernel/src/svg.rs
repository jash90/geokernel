//! Draws a construction as an SVG image.

use crate::algo::conic_nd::ConicType;
use crate::algo::vector::start;
use crate::kernel::text::TextPosition;
use crate::kernel::{Construction, Id, Kind, Value};
use std::collections::HashMap;
use std::fmt::Write;

mod extras;

/// The colour the drawing gives a text object (the app palette's curve
/// colour, which the canvas paints texts with); point labels keep the
/// group's own.
const TEXT_COLOR: geo_latex::Color = [0x1c, 0x1c, 0x1f];

fn hex([r, g, b]: geo_latex::Color) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// The rectangle of the plane that is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub xmin: f64,
    pub ymin: f64,
    pub xmax: f64,
    pub ymax: f64,
}

impl View {
    /// A view fitted to points and bounded shapes, with a margin. A text
    /// does not take part: one parked at a huge coordinate would swallow
    /// the drawing; only a construction of texts alone is fitted to them.
    pub fn fit(c: &Construction) -> View {
        let empty = || View {
            xmin: f64::INFINITY,
            ymin: f64::INFINITY,
            xmax: f64::NEG_INFINITY,
            ymax: f64::NEG_INFINITY,
        };
        let mut v = empty();
        let mut list_items_left = crate::expr::MAX_FRAME_CANVAS_LIST_ITEMS;
        let mut texts = empty();
        let mut grow = |x: f64, y: f64| {
            if !(x.is_finite() && y.is_finite()) {
                return;
            }
            v.xmin = v.xmin.min(x);
            v.ymin = v.ymin.min(y);
            v.xmax = v.xmax.max(x);
            v.ymax = v.ymax.max(y);
        };
        let mut grow_text = |x: f64, y: f64| {
            texts.xmin = texts.xmin.min(x);
            texts.ymin = texts.ymin.min(y);
            texts.xmax = texts.xmax.max(x);
            texts.ymax = texts.ymax.max(y);
        };
        for id in c.ids().filter(|&id| !c.label(id).starts_with('\0')) {
            if let Some([xmin, ymin, xmax, ymax]) = c.chart(id).and_then(|chart| chart.bounds()) {
                grow(xmin, ymin);
                grow(xmax, ymax);
                continue;
            }
            if c.image(id)
                .is_some_and(|image| !image.placement.is_screen())
            {
                if let Some(quad) = c.image_quad(id, 50.0) {
                    for [x, y] in quad {
                        grow(x, y);
                    }
                }
            }
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
                Value::Conic {
                    ty: ConicType::SinglePoint | ConicType::Circle | ConicType::Ellipse,
                } => {
                    if let Some(conic) = c.conic(id) {
                        let [x, y] = conic.midpoint;
                        if conic.ty == ConicType::SinglePoint {
                            grow(x, y);
                            continue;
                        }
                        let [a, b] = conic.half_axes;
                        let [[ux, uy], [vx, vy]] = conic.eigenvec;
                        let dx = (a * ux).hypot(b * vx);
                        let dy = (a * uy).hypot(b * vy);
                        let [xmin, ymin, xmax, ymax] = [x - dx, y - dy, x + dx, y + dy];
                        if [xmin, ymin, xmax, ymax].into_iter().all(f64::is_finite) {
                            grow(xmin, ymin);
                            grow(xmax, ymax);
                        }
                    }
                }
                Value::Ray { x, y, .. } => grow(x, y),
                Value::PenStroke { .. } => {
                    if let Some([xmin, xmax, ymin, ymax]) = c.stroke(id).and_then(|s| s.bounds()) {
                        grow(xmin, ymin);
                        grow(xmax, ymax);
                    }
                }
                Value::Polygon { .. } | Value::PolyLine { .. } => {
                    for (x, y) in c.vertices(id).flatten() {
                        grow(x, y);
                    }
                }
                Value::List { .. } if list_items_left > 0 => {
                    if let Some(list) = c.elements(id) {
                        list.canvas_members(&mut list_items_left, |member| {
                            if let Some(ends) = member.bounds(c) {
                                for (x, y) in ends {
                                    grow(x, y);
                                }
                            }
                            std::ops::ControlFlow::Continue(())
                        });
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
                Value::Text => {
                    if let Some(TextPosition::World { x, y }) = c.text_position(id) {
                        if x.is_finite() && y.is_finite() {
                            grow_text(x, y);
                        }
                    }
                }
                _ => {}
            }
        }
        let v = if v.xmin.is_finite() { v } else { texts };
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
    render_visible(c, view, width, height, |_| true)
}

/// Renders the objects selected by a caller's visibility state.
pub fn render_visible(
    c: &Construction,
    view: View,
    width: u32,
    height: u32,
    visible: impl Fn(crate::Id) -> bool,
) -> String {
    render_visible_with_label_offsets(c, view, width, height, visible, |_| None)
}

/// Renders visible objects using the caller's saved label positions.
pub fn render_visible_with_label_offsets(
    c: &Construction,
    view: View,
    width: u32,
    height: u32,
    visible: impl Fn(Id) -> bool,
    label_offset: impl Fn(Id) -> Option<(f64, f64)>,
) -> String {
    let (w, h) = (width as f64, height as f64);
    let (vw, vh) = (view.xmax - view.xmin, view.ymax - view.ymin);
    let scale = (w / vw).min(h / vh);
    // Centre the view inside the image.
    let ox = 0.5 * (w - vw * scale);
    let oy = 0.5 * (h - vh * scale);
    let sx = |x: f64| ox + (x - view.xmin) * scale;
    let sy = |y: f64| h - oy - (y - view.ymin) * scale;

    let screen = |(x, y)| (sx(x), sy(y));
    let mut out = String::new();
    let mut definitions = String::new();
    let mut assets = HashMap::new();
    let mut ids: Vec<_> = c
        .ids()
        .filter(|&id| !c.label(id).starts_with('\0') && visible(id))
        .collect();
    for &id in &ids {
        if let Some(source) = c.construction_source(id) {
            let _ = writeln!(definitions, "{} = {}", escape(c.label(id)), escape(source));
        }
    }
    let _ = writeln!(
        out,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"##,
    );
    if !definitions.is_empty() {
        let _ = writeln!(out, "<desc>{}</desc>", definitions.trim_end());
    }
    let _ = writeln!(
        out,
        r##"<rect width="100%" height="100%" fill="#ffffff"/>"##
    );
    if c.images().len() != 0 {
        out.push_str("<defs>\n");
        for &id in &ids {
            if let Some(image) = c.image(id) {
                let index = assets.len();
                assets.entry(image.asset.id()).or_insert_with(|| {
                    extras::image_source(&mut out, index, &image.asset);
                    index
                });
            }
        }
        out.push_str("</defs>\n");
    }
    ids.sort_by_key(|&id| c.drawing_order(id));
    for &id in &ids {
        if let Some(image) = c.image(id).filter(|image| image.style.background) {
            extras::image(&mut out, c, id, assets[&image.asset.id()], scale, screen);
        }
    }
    let mut allowances = (crate::expr::MAX_FRAME_CANVAS_LIST_ITEMS, 8);
    for layer in c.drawing_layers() {
        let same_layer = |id| c.drawing_style(id).layer == layer;
        for &id in ids.iter().filter(|&&id| same_layer(id)) {
            if let Some(image) = c.image(id).filter(|image| !image.style.background) {
                extras::image(&mut out, c, id, assets[&image.asset.id()], scale, screen);
            } else if c.kind(id) != Kind::Function && c.chart(id).is_some() {
                extras::chart(&mut out, c, id, scale, screen);
            }
        }
        render_geometry(
            &mut out,
            c,
            view,
            (w, h),
            ids.iter().copied().filter(|&id| same_layer(id)),
            &label_offset,
            &mut allowances,
        );
    }
    out.push_str("</svg>\n");
    out
}

fn render_geometry(
    out: &mut String,
    c: &Construction,
    view: View,
    (w, h): (f64, f64),
    ids: impl Iterator<Item = Id>,
    label_offset: &impl Fn(Id) -> Option<(f64, f64)>,
    (list_items_left, regions_left): &mut (usize, usize),
) {
    let frame = crate::plot::Frame::fit(view, w, h);
    let scale = frame.scale();
    let ox = 0.5 * (w - (view.xmax - view.xmin) * scale);
    let oy = 0.5 * (h - (view.ymax - view.ymin) * scale);
    let sx = |x: f64| ox + (x - view.xmin) * scale;
    let sy = |y: f64| h - oy - (y - view.ymin) * scale;
    let (vw, vh) = (view.xmax - view.xmin, view.ymax - view.ymin);
    let mut lines = String::new();
    let mut polygons = String::new();
    let mut angles = String::new();
    let mut segments = String::new();
    let mut circles = String::new();
    let mut points = String::new();
    let mut texts = String::new();
    let mut controls = String::new();

    // Used to draw an "infinite" line as a segment longer than the view.
    let (mx, my) = (0.5 * (view.xmin + view.xmax), 0.5 * (view.ymin + view.ymax));
    let reach = vw.hypot(vh);
    let write_line = |out: &mut String, [(x1, y1), (x2, y2)]: [(f64, f64); 2]| {
        let pixels = [sx(x1), sy(y1), sx(x2), sy(y2)];
        if pixels.iter().all(|v| v.is_finite()) {
            let [x1, y1, x2, y2] = pixels;
            let _ = writeln!(
                out,
                r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}"/>"#
            );
        }
    };
    let write_circle = |out: &mut String, cx, cy, r: f64| {
        let _ = writeln!(
            out,
            r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}"/>"#,
            sx(cx),
            sy(cy),
            r * scale,
        );
    };
    let write_conic = |out: &mut String, d: &crate::ConicData| {
        let drawn = crate::plot::conic::plot(d, &frame);
        write_paths(out, &drawn.paths);
        for (point, radius) in [(drawn.point, 4), (drawn.dot, 1)] {
            if let Some((px, py)) = point {
                let _ = write!(
                    out,
                    r##"<circle cx="{px:.2}" cy="{py:.2}" r="{radius}" fill="#202124" stroke="none"/>"##,
                );
            }
        }
    };
    for id in ids {
        if c.chart(id).is_some() {
            if c.kind(id) == Kind::Function {
                extras::chart(&mut segments, c, id, scale, |(x, y)| (sx(x), sy(y)));
            }
            continue;
        }
        match c.value(id) {
            v @ Value::Line { .. } => {
                let Some((a, b, k)) = v.unit_line() else {
                    continue;
                };
                let d = a * mx + b * my + k;
                let (fx, fy) = (mx - a * d, my - b * d);
                write_line(
                    &mut lines,
                    [
                        (fx - reach * b, fy + reach * a),
                        (fx + reach * b, fy - reach * a),
                    ],
                );
            }
            Value::Segment { x1, y1, x2, y2 } => write_line(&mut segments, [(x1, y1), (x2, y2)]),
            Value::Circle { cx, cy, r } if r.is_finite() => write_circle(&mut circles, cx, cy, r),
            Value::Point { x, y } if x.is_finite() && y.is_finite() => {
                let free = c.def(id).is_free();
                let (fill, radius) = if free {
                    ("#1a73e8", 5.0)
                } else {
                    ("#5f6368", 4.0)
                };
                let _ = writeln!(
                    points,
                    r##"<circle cx="{px:.2}" cy="{py:.2}" r="{radius}" fill="{fill}" stroke="#ffffff" stroke-width="1"/>"##,
                    px = sx(x),
                    py = sy(y),
                );
                if c.drawing_style(id).show_label {
                    let _ = writeln!(
                        points,
                        r#"<text x="{:.2}" y="{:.2}">{}</text>"#,
                        sx(x) + 8.0,
                        sy(y) - 8.0,
                        escape(c.label(id)),
                    );
                } else {
                    points.push('\n');
                }
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
            Value::Vector { x: vx, y: vy } if vx.is_finite() && vy.is_finite() => {
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
                write_conic(&mut segments, &d);
            }
            Value::ImplicitCurve => {
                if let Some(coefficients) = c.implicit_coefficients(id) {
                    write_paths(
                        &mut segments,
                        &crate::plot::implicit::plot(coefficients, &frame),
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
            Value::Number(_) if c.kind(id) == Kind::Angle => {
                let Some(g) = c.angle_geometry(id) else {
                    continue;
                };
                let radius = g.radius(scale);
                if radius <= 0.0 {
                    continue;
                }
                let (cx, cy) = (sx(g.center.0), sy(g.center.1));
                let at = |a: f64, r: f64| (cx + r * a.cos(), cy - r * a.sin());
                let _ = write!(angles, r#"<path d="M {cx:.2},{cy:.2}"#);
                if g.is_right() {
                    let side = radius * std::f64::consts::FRAC_1_SQRT_2;
                    let a = at(g.start, side);
                    let b = at(g.start + g.sweep / 2.0, radius);
                    let d = at(g.start + g.sweep, side);
                    for (x, y) in [a, b, d] {
                        let _ = write!(angles, " L {x:.2},{y:.2}");
                    }
                } else {
                    let (x, y) = at(g.start, radius);
                    let _ = write!(angles, " L {x:.2},{y:.2}");
                    // Two arcs are required when the endpoints coincide.
                    let sections = if g.sweep == std::f64::consts::TAU {
                        2
                    } else {
                        1
                    };
                    let sweep = g.sweep / f64::from(sections);
                    let large = u8::from(sweep > std::f64::consts::PI);
                    for i in 1..=sections {
                        let (x, y) = at(g.start + f64::from(i) * sweep, radius);
                        let _ = write!(
                            angles,
                            " A {radius:.2},{radius:.2} 0 {large} 0 {x:.2},{y:.2}"
                        );
                    }
                }
                let _ = writeln!(angles, r#" Z"/>"#);
            }
            Value::Locus { .. } => {
                let Some(locus) = c.locus(id) else {
                    continue;
                };
                write_locus(&locus.points, &mut segments, |x, y| (sx(x), sy(y)));
            }
            Value::PenStroke { .. } => {
                if let Some(stroke) = c.stroke(id) {
                    write_stroke(stroke, &mut segments, |x, y| (sx(x), sy(y)));
                }
            }
            Value::Number(value) if c.kind(id) == Kind::Number => {
                if let Some(slider) = c.slider_geometry(id, (scale, scale), |(x, y)| (sx(x), sy(y)))
                {
                    let (x1, y1) = slider.start;
                    let (x2, y2) = slider.end;
                    let (cx, cy) = slider.knob;
                    let r = slider.radius;
                    let _ = writeln!(
                        controls,
                        r##"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="#8493a3" stroke-width="5" stroke-linecap="round"/><circle cx="{cx:.2}" cy="{cy:.2}" r="{r:.2}" fill="#1a73e8" stroke="#1c1c1f"/>"##,
                    );
                    if c.drawing_style(id).show_label {
                        let (x, y) = slider.label_anchor();
                        let (dx, dy) = label_offset(id).unwrap_or_default();
                        let (x, y) = (x + dx, y + dy);
                        let _ = writeln!(
                            controls,
                            r#"<text x="{x:.2}" y="{y:.2}">{}</text>"#,
                            escape(&c.slider_label(id, value))
                        );
                    }
                }
            }
            Value::Bool(value) => {
                use crate::kernel::control::{CHECKBOX_INSET, CHECKBOX_SIZE};
                if let Some((x, y)) = c.checkbox_position(id) {
                    let (x, y) = (x + CHECKBOX_INSET, y + CHECKBOX_INSET);
                    let size = CHECKBOX_SIZE;
                    let fill = if value { "#1a73e8" } else { "#ffffff" };
                    let _ = writeln!(
                        controls,
                        r##"<rect x="{x:.2}" y="{y:.2}" width="{size:.2}" height="{size:.2}" fill="{fill}" stroke="#5f6368"/>"##
                    );
                    if value {
                        let _ = writeln!(
                            controls,
                            r##"<path d="M {:.2} {:.2} l 5 5 10 -10" fill="none" stroke="#ffffff" stroke-width="2"/>"##,
                            x + 6.0,
                            y + 13.0,
                        );
                    }
                    if c.drawing_style(id).show_label {
                        let caption = c.checkbox_caption(id);
                        let (x, y) = (x + size + 9.0, y + size / 2.0);
                        if let Some(source) = crate::kernel::control::latex_caption(&caption) {
                            let formula = geo_latex::layout(
                                source,
                                16.0,
                                geo_latex::Style::Display,
                                geo_latex::Face::default(),
                            );
                            geo_latex::write_svg(
                                &mut controls,
                                &formula,
                                x - 4.0,
                                y - f64::from(formula.size()[1]) / 2.0,
                                TEXT_COLOR,
                            );
                        } else {
                            let _ = writeln!(
                                controls,
                                r#"<text x="{x:.2}" y="{:.2}">{}</text>"#,
                                y + 5.0,
                                escape(&caption),
                            );
                        }
                    }
                }
            }
            Value::Curve { .. } => {
                for path in crate::plot::cartesian(c, id, &frame) {
                    let mut points = String::new();
                    for (x, y) in path {
                        let _ = write!(points, "{x:.2},{y:.2} ");
                    }
                    let _ = writeln!(
                        segments,
                        r#"<polyline points="{}" fill="none"/>"#,
                        points.trim_end()
                    );
                }
            }
            Value::FunctionNVar if *regions_left > 0 && c.is_plane_inequality(id) => {
                *regions_left -= 1;
                let tiles = crate::plot::inequality::tiles(c, id, &frame);
                if !tiles.is_empty() {
                    let _ = write!(polygons, r#"<path class="inequality" d=""#);
                    for [left, top, right, bottom] in tiles {
                        let _ = write!(
                            polygons,
                            "M{left:.2},{top:.2}H{right:.2}V{bottom:.2}H{left:.2}Z"
                        );
                    }
                    let _ = writeln!(polygons, r#""/>"#);
                }
            }
            Value::List { .. } => {
                use std::ops::ControlFlow;
                if let Some(list) = c.elements(id) {
                    list.canvas_members(list_items_left, |member| {
                        if member.object.is_some() {
                            return ControlFlow::Continue(());
                        }
                        match member.value {
                            Value::Point { x, y } if x.is_finite() && y.is_finite() => {
                                let (x, y) = (sx(x), sy(y));
                                let _ =
                                    writeln!(points, r#"<circle cx="{x:.2}" cy="{y:.2}" r="4"/>"#);
                            }
                            Value::Circle { cx, cy, r }
                                if [cx, cy, r].into_iter().all(f64::is_finite) =>
                            {
                                write_circle(&mut circles, cx, cy, r);
                            }
                            Value::Conic { .. } => {
                                if let Some(d) = member.conic(c) {
                                    write_conic(&mut segments, &d);
                                }
                            }
                            _ => {
                                if let Some(ends) = member.straight_endpoints(frame.visible(0.0)) {
                                    write_line(&mut segments, ends);
                                }
                            }
                        }
                        ControlFlow::Continue(())
                    });
                }
            }
            Value::InputBox => {
                let [x, y, width, height] = c.input_box_rect(id);
                let box_position = c.input_box_style(id).position.map(f64::from);
                let (label_x, y) = label_offset(id).unwrap_or((box_position[0], y));
                let x = label_x + (x - box_position[0]);
                let caption = escape(c.input_box_caption(id));
                let text = escape(&c.input_box_text(id));
                let _ = writeln!(
                    controls,
                    r##"<rect x="{x:.2}" y="{y:.2}" width="{width:.2}" height="{height:.2}" fill="#ffffff" stroke="#777777"/>"##,
                );
                let _ = writeln!(
                    controls,
                    r#"<svg x="{x:.2}" y="{y:.2}" width="{width:.2}" height="{height:.2}" overflow="hidden"><text x="6" y="20">{text}</text></svg>"#,
                );
                if !caption.is_empty() {
                    let _ = writeln!(
                        controls,
                        r#"<text x="{:.2}" y="{:.2}">{caption}</text>"#,
                        label_x,
                        y + 20.0
                    );
                }
            }
            Value::Text => {
                let Some((source, style)) = c
                    .text(id)
                    .filter(|text| !text.is_empty())
                    .zip(c.text_presentation(id))
                else {
                    continue;
                };
                let Some((x, y)) = style.pixel_anchor(c, |x, y| (sx(x), sy(y))) else {
                    continue;
                };
                let size = style.drawing_size(16.0);
                if style.latex {
                    let formula = geo_latex::layout(
                        source,
                        size,
                        geo_latex::Style::Display,
                        geo_latex::Face {
                            bold: style.bold,
                            italic: style.italic,
                            monospace: false,
                        },
                    );
                    let [x, y] =
                        style.aligned_top_left([x as f32, y as f32], formula.size(), 0.0, 0.0);
                    geo_latex::write_svg(
                        &mut texts,
                        &formula,
                        f64::from(x),
                        f64::from(y),
                        TEXT_COLOR,
                    );
                } else {
                    // The plain text's width comes from the same font
                    // metrics the formula renderer uses, not a per-character
                    // estimate, so both kinds of text place alike.
                    let face = geo_latex::Face {
                        bold: style.bold,
                        italic: style.italic,
                        monospace: false,
                    };
                    let lines = geo_latex::plain_lines(source, size, face);
                    let width = lines
                        .iter()
                        .map(|(width, ..)| *width)
                        .fold(0.0_f32, f32::max);
                    let line_height = size * 1.2;
                    let [x, y] = style.aligned_top_left(
                        [x as f32, y as f32],
                        [width, line_height * lines.len() as f32],
                        line_height,
                        0.0,
                    );
                    let font_family = if style.serif {
                        r#" font-family="serif""#
                    } else {
                        ""
                    };
                    let font_style = if style.italic { "italic" } else { "normal" };
                    let weight = if style.bold { "bold" } else { "normal" };
                    for (row, line) in source.lines().enumerate() {
                        let y = y + row as f32 * line_height;
                        let _ = writeln!(
                            texts,
                            r#"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" font-style="{font_style}" font-weight="{weight}"{font_family} fill="{}" xml:space="preserve">{}</text>"#,
                            hex(TEXT_COLOR),
                            escape(line)
                        );
                    }
                }
            }
            Value::Point { .. }
            | Value::Vector { .. }
            | Value::Circle { .. }
            | Value::Number(_)
            | Value::FunctionNVar
            | Value::HomogeneousPoint { .. }
            | Value::NormalizedPointAtInfinity { .. }
            | Value::UndefinedLine { .. }
            | Value::Button
            | Value::Image
            | Value::PieChart
            | Value::Undefined => {}
        }
    }

    let _ = writeln!(
        out,
        r##"<g fill="#8b5a2b" fill-opacity="0.1" stroke="none">"##
    );
    out.push_str(&polygons);
    let _ = writeln!(out, "</g>");
    if !angles.is_empty() {
        let _ = writeln!(
            out,
            r##"<g fill="#2e7d32" fill-opacity="0.1" stroke="#2e7d32" stroke-width="1.5">"##
        );
        out.push_str(&angles);
        let _ = writeln!(out, "</g>");
    }
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
    out.push_str(&texts);
    out.push_str(&controls);
    let _ = writeln!(out, "</g>");
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace(
            |ch: char| {
                ch < ' ' && !matches!(ch, '\t' | '\r' | '\n')
                    || matches!(ch, '\u{fffe}' | '\u{ffff}')
            },
            "\u{fffd}",
        )
}

/// The pieces of a locus into `out` (the segments group): every run of
/// samples joined by `line_to` as a polyline, and a run of a single sample
/// as the round cap of the stroke, a dot — the drawing draws one too
/// (`CurvePlotterUtils.moveTo`; a polyline of one point would be nothing).
fn write_locus(
    points: &[crate::LocusPoint],
    out: &mut String,
    at: impl Fn(f64, f64) -> (f64, f64),
) {
    let mut run: Vec<(f64, f64)> = Vec::new();
    fn flush(run: &mut Vec<(f64, f64)>, out: &mut String) {
        match run.as_slice() {
            [] => {}
            // the round cap of the 2 px stroke, like the drawing's dot
            &[(x, y)] => {
                let _ = writeln!(
                    out,
                    r##"<circle cx="{x:.2}" cy="{y:.2}" r="1" fill="#202124"/>"##
                );
            }
            _ => {
                let mut pts = String::new();
                for &(x, y) in run.iter() {
                    let _ = write!(pts, "{x:.2},{y:.2} ");
                }
                let _ = writeln!(
                    out,
                    r##"<polyline points="{}" fill="none"/>"##,
                    pts.trim_end()
                );
            }
        }
        run.clear();
    }
    for p in points {
        if !p.line_to {
            flush(&mut run, out);
        }
        // an undefined point (NaN) is a break too
        if p.x.is_finite() && p.y.is_finite() {
            run.push(at(p.x, p.y));
        } else {
            flush(&mut run, out);
        }
    }
    flush(&mut run, out);
}

/// A pen stroke as one path with round caps, from the path operations of
/// [`crate::algo::stroke::StrokeData::path`]; a lone point is a line to
/// itself, which the round cap shows as a dot.
fn write_stroke(
    stroke: &crate::algo::stroke::StrokeData,
    out: &mut String,
    at: impl Fn(f64, f64) -> (f64, f64),
) {
    use crate::algo::stroke::PathOp;
    let mut d = String::new();
    for op in stroke.path() {
        let _ = match op {
            PathOp::Move([x, y]) => {
                let (x, y) = at(x, y);
                write!(d, "M{x:.2} {y:.2}")
            }
            PathOp::Line([x, y]) => {
                let (x, y) = at(x, y);
                write!(d, "L{x:.2} {y:.2}")
            }
            PathOp::Quad([cx, cy], [x, y]) => {
                let ((cx, cy), (x, y)) = (at(cx, cy), at(x, y));
                write!(d, "Q{cx:.2} {cy:.2} {x:.2} {y:.2}")
            }
            PathOp::Cubic([ax, ay], [bx, by], [x, y]) => {
                let ((ax, ay), (bx, by), (x, y)) = (at(ax, ay), at(bx, by), at(x, y));
                write!(d, "C{ax:.2} {ay:.2} {bx:.2} {by:.2} {x:.2} {y:.2}")
            }
        };
    }
    if !d.is_empty() {
        let _ = writeln!(
            out,
            r##"<path d="{d}" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"##
        );
    }
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

#[cfg(test)]
mod angle_tests {
    use super::*;

    #[test]
    fn angle_paths_use_the_right_square_and_a_counterclockwise_reflex_arc() {
        let c = crate::script::load(
            "A = (2, 0)\nB = (0, 0)\nC = (0, 2)\nright = Angle(A, B, C)\nreflex = Angle(C, B, A)\nn = Angle(2)\n",
        ).unwrap();
        let svg = render(
            &c,
            View {
                xmin: -4.0,
                ymin: -4.0,
                xmax: 4.0,
                ymax: 4.0,
            },
            400,
            400,
        );
        assert_eq!(svg.matches("<path").count(), 2);
        assert!(svg.contains("M 200.00,200.00 L 221.21,200.00 L 221.21,178.79 L 200.00,178.79 Z"));
        assert!(svg.contains("A 30.00,30.00 0 1 0 230.00,200.00 Z"));
    }

    /// A piece of a locus with a single sample is drawn as the dot of its
    /// stroke cap, not dropped (`DrawLocus` draws one as a dot).
    #[test]
    fn a_lone_locus_sample_is_drawn_as_a_dot() {
        use crate::LocusPoint;
        let points = [
            LocusPoint {
                x: 10.0,
                y: 20.0,
                line_to: false,
            },
            LocusPoint {
                x: 30.0,
                y: 20.0,
                line_to: true,
            },
            // a lone point, then an undefined stretch, then a lone point
            LocusPoint {
                x: 50.0,
                y: 40.0,
                line_to: false,
            },
            LocusPoint {
                x: f64::NAN,
                y: f64::NAN,
                line_to: true,
            },
            LocusPoint {
                x: 60.0,
                y: 41.0,
                line_to: false,
            },
        ];
        let mut out = String::new();
        write_locus(&points, &mut out, |x, y| (x, y));
        assert_eq!(out.matches("<polyline").count(), 1, "{out}");
        assert_eq!(out.matches("<circle").count(), 2, "{out}");
        assert!(
            out.contains(r##"<circle cx="50.00" cy="40.00" r="1" fill="#202124"/>"##),
            "{out}"
        );
        assert!(
            out.contains(r#"<polyline points="10.00,20.00 30.00,20.00""#),
            "{out}"
        );
    }
}

#[cfg(test)]
mod chart_export_tests {
    use super::*;

    #[test]
    fn exports_chart_marks() {
        let c = crate::script::load(
            "f(x) = x\ngraph = LineGraph({-3, 3}, {0, 0})\npie = PieChart({1, 2})\nbars = BarChart({1, 2}, {2, 3})\nzeroWidth = BarChart({1, 2}, 0)\nbox = BoxPlot(0, 1, {1, 2, 3})\ndots = DotPlot({1, 2, 3})",
        ).unwrap();
        let svg = render(&c, View::fit(&c), 640, 480);
        for label in ["pie", "bars", "box", "dots"] {
            assert!(svg.contains(&format!(r#"data-label="{label}""#)), "{svg}");
        }
        assert!(svg.contains("<path") && svg.contains("<rect") && svg.contains("<circle"));
        let (_, zero_width) = svg.split_once(r#"data-label="zeroWidth""#).unwrap();
        assert!(zero_width.split_once("</g>").unwrap().0.contains("<line"));

        let pie = svg.find(r#"data-label="pie""#).unwrap();
        let function = svg.find("<polyline").unwrap();
        let graph = svg.find(r#"data-label="graph""#).unwrap();
        assert!(pie < function && function < graph);
    }
}

#[cfg(test)]
mod control_tests {
    use super::*;

    #[test]
    fn slider_track_and_knob_follow_the_view_scale_and_value() {
        let c = crate::script::load(
            "a = Slider(0, 8, 0.5)\na = 2\n\
             $sliderStyle(_61, false, 0, 0, 2, true, false, _, 0.5)\n\
             b = Slider(0, 8)\nb = 6\n\
             $sliderStyle(_62, true, 30, 160, 100, false, false, _, _)\n\
             k = 2",
        )
        .unwrap();
        let svg = render(
            &c,
            View {
                xmin: -5.0,
                xmax: 5.0,
                ymin: -5.0,
                ymax: 5.0,
            },
            200,
            200,
        );
        assert!(svg.contains(r#"x1="100.00" y1="100.00" x2="140.00" y2="100.00""#));
        assert!(svg.contains(r#"cx="110.00" cy="100.00""#));
        assert!(svg.contains(r#"x1="30.00" y1="160.00" x2="30.00" y2="60.00""#));
        assert!(svg.contains(r#"cx="30.00" cy="85.00""#));
        assert!(svg.contains(">a = 2</text>"));
        assert!(!svg.contains(">k = 2</text>"));
    }
}

#[cfg(test)]
#[test]
fn fit_line_svg_draws_the_large_diagonal_and_skips_the_infinite_offset() {
    let c = crate::script::load(
        "t = 9*10^153\ng = FitLine({(-t,-t),(t,t)})\nu = 2^341\nh = FitLine({(0,4*u),(u,5*u)})",
    )
    .unwrap();
    let view = View {
        xmin: -5.0,
        xmax: 5.0,
        ymin: -5.0,
        ymax: 5.0,
    };
    let svg = |label: &str| render_visible(&c, view, 400, 300, |id| c.label(id) == label);
    let diagonal = svg("g");
    assert!(
        diagonal.contains(r#"<line x1="500.00" y1="-150.00" x2="-100.00" y2="450.00"/>"#),
        "{diagonal}"
    );
    assert!(!svg("h").contains("<line "));
}

#[cfg(test)]
#[test]
fn input_box_svg_draws_at_saved_position_with_escaped_text() {
    let c = crate::script::load("note = \"x < y\"\nfield = $inputBox(note, _)").unwrap();
    let id = c.id("field").unwrap();
    let svg = render_visible_with_label_offsets(
        &c,
        View::fit(&c),
        400,
        300,
        |_| true,
        |object| (object == id).then_some((48.0, 64.0)),
    );
    let x = 48.0 + c.input_box_rect(id)[0] - 30.0;
    assert!(svg.contains(&format!("<rect x=\"{x:.2}\" y=\"64.00\"")));
    assert!(svg.contains("<text x=\"48.00\" y=\"84.00\">field</text>"));
    assert!(svg.contains(">x &lt; y</text>"));
}

#[cfg(test)]
#[test]
fn canvas_list_svg_draws_anonymous_points_only() {
    let c = crate::script::load("A = (0, 0)\nl = {A, (1, 2), (3, 4)}\nn = {1, 2}").unwrap();
    let view = View::fit(&c);
    let render =
        |labels: &[&str]| render_visible(&c, view, 400, 300, |id| labels.contains(&c.label(id)));
    assert_eq!(render(&["A", "l", "n"]).matches("<circle ").count(), 3);
    assert_eq!(render(&["l"]).matches("<circle ").count(), 2);
    assert_eq!(render(&["n"]).matches("<circle ").count(), 0);
}

#[cfg(test)]
#[test]
fn canvas_remaining_svg_draws_a_cartesian_curve_and_a_boolean_region() {
    let c =
        crate::script::load("curve = Curve(t, t^2, t, -2, 2)\nregion(x,y) = y > x\nf(x,y) = x + y")
            .unwrap();
    let view = View::fit(&c);
    let svg = |label: &str| render_visible(&c, view, 400, 300, |id| c.label(id) == label);
    assert!(svg("curve").contains("<polyline"));
    assert!(svg("region").contains("class=\"inequality\""));
    assert!(!svg("f").contains("class=\"inequality\""));
}

#[test]
fn canvas_svg_shares_region_and_list_limits_across_layers() {
    use std::fmt::Write as _;

    let mut script = String::new();
    for i in 0..32 {
        writeln!(script, "L{i} = Sequence((k/100, 0), k, 1, 256)").unwrap();
    }
    for i in 0..16 {
        writeln!(script, "region{i}(x,y) = y > x + {}", i as f64 / 10.0).unwrap();
    }
    let mut c = crate::script::load(&script).unwrap();
    for label in (16..32)
        .map(|i| format!("L{i}"))
        .chain((8..16).map(|i| format!("region{i}")))
    {
        let id = c.id(&label).unwrap();
        let mut style = c.drawing_style(id);
        style.layer = 1;
        c.set_drawing_style(id, style);
    }
    let svg = render(&c, View::fit(&c), 800, 600);
    assert_eq!(svg.matches("class=\"inequality\"").count(), 8);
    assert_eq!(svg.matches("<circle ").count(), 4096);
}
