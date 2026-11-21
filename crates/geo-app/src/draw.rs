// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/plugin/EuclidianStyleConstants.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Draws the objects of a construction with `egui::Painter`.

use crate::arc;
use crate::geom::{ToCore, ToEgui};
use crate::highlight::{self, Highlight};
use crate::interact::ViewTransform;
use crate::labelpos;
use crate::shape;
use crate::style;
use crate::theme;
use eframe::egui::{vec2, Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke};
use geo_app_core::geom;
use geo_kernel::algo::vector;
use geo_kernel::kernel::{Construction, Id, Kind, Value};
use geo_kernel::ConicType;
use std::borrow::Cow;
use std::cell::RefCell;

/// Point radii in pixels: point size 5 for free points, 4 for dependent ones.
pub use crate::style::FREE_POINT_RADIUS;
const DEPENDENT_POINT_RADIUS: f32 = 4.0;
/// Line thickness 5, drawn half as wide.
pub const CURVE_WIDTH: f32 = 2.5;
const LABEL_FONT: f32 = 16.0;

/// What the picture needs besides the construction.
pub struct Scene<'a> {
    pub visible: &'a dyn Fn(Id) -> bool,
    /// Hidden objects shown faintly (while the show/hide tool is active).
    pub ghost: &'a dyn Fn(Id) -> bool,
    pub selected: Option<Id>,
    pub hovered: Option<Id>,
    /// The objects' colours (constant time each).
    pub palette: style::Palette<'a>,
    /// Skip the objects that draw nothing in the area (see `near_view`).
    /// Off only in the tests that check that skipping changes nothing.
    pub cull: bool,
    /// How many times the construction was rebuilt (`Workbench::rebuilds`):
    /// the drawings of conics and functions are kept while it stays.
    pub generation: u64,
    /// How far each label was moved from its place, in pixels.
    pub label_offset: &'a dyn Fn(Id) -> Option<(f64, f64)>,
    /// Where the painted labels went, in the area's coordinates, in the
    /// order they were painted: what a press of the move tool tests first
    /// (`EuclidianView.getLabelHit`). Cleared and filled by each paint.
    pub label_rects: Option<&'a RefCell<Vec<(Id, geom::Rect)>>>,
}

/// How far outside the area, in pixels, an object's points may lie and
/// still be drawn: a point's disc and label (6 px off it), a segment's
/// label (12 px off it), a circle's label (12 px out) and every stroke all
/// stay within it, and so does the label rule below (an anchor at most
/// 20 px outside).
const NEAR: f64 = 32.0;
/// An angle's sector, highlight and label stay within this distance
/// of its vertex, including the label margin.
const NEAR_ANGLE: f64 = 48.0;

/// Whether `id` may draw something in the `w` x `h` area: false only when
/// every shape and label of it certainly lies outside, so skipping it
/// changes no pixel. An ellipse is tested by its bounding box, a conic
/// that is one point by the point. Lines, rays, functions and the other
/// conics are unbounded and always drawn (their drawing is kept between
/// frames, see `curves`);
/// anything this does not know, or with a coordinate that is not finite
/// (the drawing code decides about those), too.
fn near_view(c: &Construction, id: Id, t: &ViewTransform, w: f64, h: f64) -> bool {
    let near = |pts: &mut dyn Iterator<Item = (f64, f64)>, margin: f64| {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        let mut any = false;
        for (x, y) in pts {
            let (sx, sy) = t.world_to_screen(x, y);
            if !(sx.is_finite() && sy.is_finite()) {
                return true;
            }
            (x0, y0, x1, y1) = (x0.min(sx), y0.min(sy), x1.max(sx), y1.max(sy));
            any = true;
        }
        !any || (x1 >= -margin && x0 <= w + margin && y1 >= -margin && y0 <= h + margin)
    };
    match c.value(id) {
        Value::Point { x, y } => near(&mut std::iter::once((x, y)), NEAR),
        Value::Segment { x1, y1, x2, y2 } => near(&mut [(x1, y1), (x2, y2)].into_iter(), NEAR),
        Value::Circle { cx, cy, r } => {
            near(&mut [(cx - r, cy - r), (cx + r, cy + r)].into_iter(), NEAR)
        }
        Value::Vector { x, y } => match vector::start(c, id) {
            Some((x0, y0)) => near(&mut [(x0, y0), (x0 + x, y0 + y)].into_iter(), NEAR),
            None => true,
        },
        Value::Polygon { .. } | Value::PolyLine { .. } => near(&mut c.vertices(id).flatten(), NEAR),
        Value::Conic { .. } => match c.conic(id) {
            Some(d) if matches!(d.ty, ConicType::Ellipse | ConicType::Circle) => {
                let ([a, b], [e0, e1], [mx, my]) = (d.half_axes, d.eigenvec, d.midpoint);
                // Half the width and height of the box around the ellipse.
                let ex = (a * e0[0]).hypot(b * e1[0]);
                let ey = (a * e0[1]).hypot(b * e1[1]);
                near(
                    &mut [(mx - ex, my - ey), (mx + ex, my + ey)].into_iter(),
                    NEAR,
                )
            }
            Some(d) if d.ty == ConicType::SinglePoint => {
                near(&mut std::iter::once((d.midpoint[0], d.midpoint[1])), NEAR)
            }
            Some(d) if d.ty == ConicType::Empty || !d.defined => false,
            // The parameter ranges of an open conic are its cull test.
            Some(d) if matches!(d.ty, ConicType::Hyperbola | ConicType::Parabola) => {
                geo_kernel::plot::conic::reaches(&d, &geo_kernel::plot::Frame::fit(t.view(), w, h))
            }
            _ => true,
        },
        _ => true,
    }
}

/// A point: a filled disc with a thin dark ring.
pub fn point_shape(painter: &Painter, at: Pos2, fill: Color32, radius: f32) {
    painter.circle(
        at,
        radius,
        fill,
        Stroke::new(
            1.0,
            theme::POINT_RING.gamma_multiply(fill.a() as f32 / 255.0),
        ),
    );
}

/// A label's object, text (borrowed from the construction where it is the
/// bare label), anchor in the area, colour and alignment. The anchor is
/// kept in `f32`, as painted, which keeps the buffer of a frame's labels
/// the size it had before it carried the object.
type Label<'c> = (Id, Cow<'c, str>, (f32, f32), Color32, Align2);

fn label(
    id: Id,
    text: Cow<'_, str>,
    (x, y): (f64, f64),
    color: Color32,
    align: Align2,
) -> Label<'_> {
    (id, text, (x as f32, y as f32), color, align)
}

pub fn paint(painter: &Painter, rect: Rect, c: &Construction, t: &ViewTransform, scene: &Scene) {
    let (w, h) = (rect.width() as f64, rect.height() as f64);
    let origin = rect.min;
    let p = |x: f64, y: f64| -> Pos2 {
        let (sx, sy) = t.world_to_screen(x, y);
        origin + vec2(sx as f32, sy as f32)
    };
    let screen = |x: f64, y: f64| t.world_to_screen(x, y);
    let in_rect = |s: (f64, f64)| s.0 >= 0.0 && s.0 <= w && s.1 >= 0.0 && s.1 <= h;
    let color = |id: Id| {
        let c = scene.palette.color(id).egui();
        if (scene.ghost)(id) {
            c.gamma_multiply(0.3)
        } else {
            c
        }
    };
    let curve = |id: Id| Stroke::new(CURVE_WIDTH, color(id));

    // Used to draw an "infinite" line as a segment longer than the view.
    let v = t.view();
    let (vw, vh) = (v.xmax - v.xmin, v.ymax - v.ymin);
    let (mx, my) = (0.5 * (v.xmin + v.xmax), 0.5 * (v.ymin + v.ymax));
    let reach = (w / t.scale()).hypot(h / t.scale()).max(vw.hypot(vh));

    let mut angles = Vec::new();
    let ids: Vec<Id> = c
        .ids()
        .filter(|&id| {
            if !(scene.visible)(id) {
                return false;
            }
            // A moved label may lie anywhere: its object is never skipped.
            if let Some(angle) = c.angle_geometry(id) {
                let (x, y) = screen(angle.center.0, angle.center.1);
                if !scene.cull
                    || ((-NEAR_ANGLE..=w + NEAR_ANGLE).contains(&x)
                        && (-NEAR_ANGLE..=h + NEAR_ANGLE).contains(&y))
                    || (scene.label_offset)(id).is_some()
                {
                    angles.push((id, angle));
                }
                return false;
            }
            !scene.cull || near_view(c, id, t, w, h) || (scene.label_offset)(id).is_some()
        })
        .collect();
    let mut labels: Vec<Label> = Vec::new();
    // Curved outlines are tessellated and kept between frames.
    let is_curve = |id: Id| match c.value(id) {
        Value::Conic { .. } | Value::Function | Value::Arc { .. } | Value::Sector { .. } => true,
        Value::Undefined => matches!(c.kind(id), Kind::Arc | Kind::Sector),
        _ => false,
    };
    let curves = (ids.iter().any(|&id| is_curve(id))
        || [scene.hovered, scene.selected]
            .into_iter()
            .flatten()
            .any(is_curve))
    .then(|| crate::curves::Curves::new(painter, rect, t, scene.generation));

    // Highlights go under the objects, in the object's own colour.
    for (id, strong) in [(scene.hovered, false), (scene.selected, true)] {
        let Some(id) = id.filter(|i| (scene.visible)(*i)) else {
            continue;
        };
        if matches!(c.value(id), Value::Polygon { .. }) {
            if let Some(pts) = crate::hit::screen_vertices(c, id, t) {
                let alpha = if strong { 0.35 } else { 0.2 };
                let points = pts
                    .iter()
                    .map(|&(x, y)| origin + vec2(x as f32, y as f32))
                    .collect();
                let glow = Stroke::new(8.0, scene.palette.color(id).egui().gamma_multiply(alpha));
                painter.add(Shape::closed_line(points, glow));
            }
            continue;
        }
        if let Some(drawn) = curves.as_ref().and_then(|k| k.get(c, id, color(id))) {
            let alpha = if strong { 0.35 } else { 0.2 };
            let glow = Stroke::new(8.0, scene.palette.color(id).egui().gamma_multiply(alpha));
            for path in drawn.paths() {
                painter.add(Shape::line(path, glow));
            }
            continue;
        }
        if let Some(hl) = highlight::of_value(&c.value(id), t, w, h) {
            let col = scene.palette.color(id).egui();
            paint_highlight(painter, rect, hl, col, strong);
        }
    }

    // Angles first: their sectors lie under the lines.
    for (id, angle) in angles {
        let radius = angle.radius(t.scale()) as f32;
        if radius <= 0.0 {
            continue;
        }
        let vertex = p(angle.center.0, angle.center.1);
        let col = color(id);
        let fill = col.gamma_multiply(0.1);
        let stroke = Stroke::new(1.5, col);
        let at = |direction: f64, distance: f32| {
            let (sin, cos) = direction.sin_cos();
            vertex + vec2(cos as f32, -(sin as f32)) * distance
        };
        let half = angle.start + angle.sweep / 2.0;
        let highlight = if scene.selected == Some(id) {
            0.35
        } else if scene.hovered == Some(id) {
            0.2
        } else {
            0.0
        };
        let end = angle.start + angle.sweep;
        if angle.is_right() {
            let side = radius * std::f32::consts::FRAC_1_SQRT_2;
            let points = vec![
                vertex,
                at(angle.start, side),
                at(half, radius),
                at(end, side),
            ];
            if highlight > 0.0 {
                painter.add(Shape::closed_line(
                    points.clone(),
                    Stroke::new(8.0, col.gamma_multiply(highlight)),
                ));
            }
            painter.add(Shape::convex_polygon(points, fill, stroke));
        } else {
            // About 0.25 px maximum deviation at the default radius.
            let n = (angle.sweep / 0.25).ceil().max(1.0) as usize;
            let mut points = Vec::with_capacity(n + 2);
            points.push(vertex);
            points.extend(
                (0..=n).map(|i| at(angle.start + angle.sweep * i as f64 / n as f64, radius)),
            );
            let mut mesh = Mesh::default();
            mesh.vertices.reserve(points.len());
            mesh.indices.reserve(3 * n);
            for &point in &points {
                mesh.colored_vertex(point, fill);
            }
            for i in 1..=n as u32 {
                mesh.add_triangle(0, i, i + 1);
            }
            painter.add(Shape::mesh(mesh));
            if highlight > 0.0 {
                painter.add(Shape::closed_line(
                    points.clone(),
                    Stroke::new(8.0, col.gamma_multiply(highlight)),
                ));
            }
            painter.add(Shape::closed_line(points, stroke));
        }
        let mid = at(half, radius / 1.7) - origin;
        labels.push(label(
            id,
            Cow::Owned(format!(
                "{} = {}",
                c.label(id),
                crate::describe::degrees(angle.sweep)
            )),
            (f64::from(mid.x) - 3.0, f64::from(mid.y) + 5.0),
            col,
            Align2::LEFT_BOTTOM,
        ));
    }

    // Polygons: a translucent fill and an outline, under the lines. Their
    // sides are segments of their own, drawn with the other segments.
    for &id in &ids {
        let closed = match c.value(id) {
            Value::Polygon { .. } => true,
            Value::PolyLine { .. } => false,
            _ => continue,
        };
        let col = color(id);
        if closed {
            let Some(pts) = crate::hit::screen_vertices(c, id, t) else {
                continue;
            };
            let points: Vec<Pos2> = pts
                .iter()
                .map(|&(x, y)| origin + vec2(x as f32, y as f32))
                .collect();
            let mut mesh = Mesh::default();
            for &p in &points {
                mesh.colored_vertex(p, col.gamma_multiply(0.1));
            }
            for [a, b, d] in shape::triangulate(&pts) {
                mesh.add_triangle(a as u32, b as u32, d as u32);
            }
            painter.add(Shape::mesh(mesh));
            painter.add(Shape::closed_line(points, Stroke::new(CURVE_WIDTH, col)));
        } else {
            // An undefined vertex leaves a gap.
            let mut run: Vec<Pos2> = Vec::new();
            for v in c.vertices(id).chain([None]) {
                match v {
                    Some((x, y)) => run.push(p(x, y)),
                    None if run.len() >= 2 => {
                        painter.add(Shape::line(std::mem::take(&mut run), curve(id)));
                    }
                    None => run.clear(),
                }
            }
        }
    }

    for &id in &ids {
        if let Some((a, b, k)) = c.value(id).unit_line() {
            let d = a * mx + b * my + k;
            let (fx, fy) = (mx - a * d, my - b * d);
            painter.line_segment(
                [
                    p(fx - reach * b, fy + reach * a),
                    p(fx + reach * b, fy - reach * a),
                ],
                curve(id),
            );
            if let Some(entry) = labelpos::line_entry(screen(fx, fy), (b, a), w, h) {
                let pos = labelpos::line_label(entry, w, h, 24.0, 12.0);
                labels.push(label(
                    id,
                    Cow::Borrowed(c.label(id)),
                    pos,
                    color(id),
                    Align2::CENTER_CENTER,
                ));
            }
        }
    }
    for &id in &ids {
        // A ray: a half-line from its start point along (b, -a).
        if let Value::Ray { x, y, a, b, .. } = c.value(id) {
            let len = a.hypot(b);
            if !(len > 0.0 && x.is_finite() && y.is_finite()) {
                continue;
            }
            let far = (reach + (x - mx).hypot(y - my)) / len;
            painter.line_segment([p(x, y), p(x + far * b, y - far * a)], curve(id));
            let s = screen(x, y);
            let entry = if in_rect(s) {
                Some(labelpos::Entry {
                    x: s.0,
                    y: s.1,
                    dx: b / len,
                    dy: a / len,
                })
            } else {
                labelpos::line_entry(s, (b, a), w, h)
            };
            if let Some(e) = entry {
                let pos = labelpos::line_label(e, w, h, 24.0, 12.0);
                labels.push(label(
                    id,
                    Cow::Borrowed(c.label(id)),
                    pos,
                    color(id),
                    Align2::CENTER_CENTER,
                ));
            }
        }
    }
    for &id in &ids {
        if let Value::Circle { cx, cy, r } = c.value(id) {
            paint_circle(painter, rect, t, (cx, cy, r), curve(id), &mut |pos| {
                labels.push(label(
                    id,
                    Cow::Borrowed(c.label(id)),
                    pos,
                    color(id),
                    Align2::CENTER_CENTER,
                ));
            });
        }
    }
    if let Some(curves) = &curves {
        for id in ids.iter().copied().filter(|&id| is_curve(id)) {
            let Some(drawn) = curves.get(c, id, color(id)) else {
                continue;
            };
            if !drawn.mesh.is_empty() {
                painter.add(Shape::Mesh(drawn.mesh.clone()));
            }
            if let Some(at) = drawn.point {
                point_shape(painter, at, color(id), DEPENDENT_POINT_RADIUS);
            }
            // An ellipse smaller than a pixel: a dot of the stroke width.
            if let Some(at) = drawn.dot {
                painter.circle_filled(at, 0.5 * CURVE_WIDTH, color(id));
            }
            if let Some(pos) = drawn.label {
                labels.push(label(
                    id,
                    Cow::Borrowed(c.label(id)),
                    pos,
                    color(id),
                    Align2::LEFT_BOTTOM,
                ));
            }
        }
    }
    for &id in &ids {
        if let Value::Segment { x1, y1, x2, y2 } = c.value(id) {
            if ![x1, y1, x2, y2].iter().all(|v| v.is_finite()) {
                continue;
            }
            painter.line_segment([p(x1, y1), p(x2, y2)], curve(id));
            let pos = labelpos::segment_label(screen(x1, y1), screen(x2, y2), 12.0);
            labels.push(label(
                id,
                Cow::Borrowed(c.label(id)),
                pos,
                color(id),
                Align2::CENTER_CENTER,
            ));
        }
    }
    for &id in &ids {
        // A vector: an arrow from its start point (the origin if it has none).
        let (Value::Vector { x, y }, Some((x0, y0))) = (c.value(id), vector::start(c, id)) else {
            continue;
        };
        let (from, to) = (p(x0, y0), p(x0 + x, y0 + y));
        painter.line_segment([from, to], curve(id));
        let d = to - from;
        if d.length() > 0.0 {
            let u = d / d.length();
            let n = vec2(-u.y, u.x);
            let base = to - 12.0 * u;
            painter.add(Shape::convex_polygon(
                vec![to, base + 4.5 * n, base - 4.5 * n],
                color(id),
                Stroke::NONE,
            ));
        }
        let pos = labelpos::segment_label(screen(x0, y0), screen(x0 + x, y0 + y), 12.0);
        labels.push(label(
            id,
            Cow::Borrowed(c.label(id)),
            pos,
            color(id),
            Align2::CENTER_CENTER,
        ));
    }
    for &id in &ids {
        if let Value::Point { x, y } = c.value(id) {
            if !(x.is_finite() && y.is_finite()) {
                continue;
            }
            let radius = if c.def(id).is_free() {
                FREE_POINT_RADIUS
            } else {
                DEPENDENT_POINT_RADIUS
            };
            point_shape(painter, p(x, y), color(id), radius);
            let (sx, sy) = screen(x, y);
            labels.push(label(
                id,
                Cow::Borrowed(c.label(id)),
                (sx + 6.0, sy - 6.0),
                color(id),
                Align2::LEFT_BOTTOM,
            ));
        }
    }
    let font = FontId::proportional(LABEL_FONT);
    let mut rects = scene.label_rects.map(|r| {
        let mut r = r.borrow_mut();
        r.clear();
        r
    });
    for (id, text, (sx, sy), color, align) in labels {
        // `labelOffsetX/Y` of the object, added to where its label goes.
        let (sx, sy) = match (scene.label_offset)(id) {
            Some((ox, oy)) => (sx + ox as f32, sy + oy as f32),
            None => (sx, sy),
        };
        let (wf, hf) = (w as f32, h as f32);
        if sx < -20.0 || sy < -20.0 || sx > wf + 20.0 || sy > hf + 20.0 {
            continue;
        }
        let r = painter.text(origin + vec2(sx, sy), align, text, font.clone(), color);
        if let Some(rects) = rects.as_mut() {
            rects.push((id, r.translate(-origin.to_vec2()).core()));
        }
    }
}

/// Draws a circle (an arc of it when it is too big for the painter) and
/// reports where its label goes.
fn paint_circle(
    painter: &Painter,
    rect: Rect,
    t: &ViewTransform,
    (cx, cy, r): (f64, f64, f64),
    stroke: Stroke,
    label: &mut dyn FnMut((f64, f64)),
) {
    let (w, h) = (rect.width() as f64, rect.height() as f64);
    let origin = rect.min;
    let in_rect = |s: (f64, f64)| s.0 >= 0.0 && s.0 <= w && s.1 >= 0.0 && s.1 <= h;
    let radius = r * t.scale();
    if !(radius.is_finite() && cx.is_finite() && cy.is_finite()) {
        return;
    }
    let (sx, sy) = t.world_to_screen(cx, cy);
    if radius > arc::ARC_RADIUS {
        // Too big for the painter: draw the part near the area.
        let Some(pts) = arc::visible_arc(sx, sy, radius, w, h) else {
            return;
        };
        let points: Vec<Pos2> = pts
            .iter()
            .map(|&(x, y)| origin + vec2(x as f32, y as f32))
            .collect();
        painter.add(Shape::line(points, stroke));
        // Label on the arc point nearest to the upper right corner.
        let best = pts
            .iter()
            .filter(|&&(x, y)| x > 14.0 && x < w - 14.0 && y > 14.0 && y < h - 14.0)
            .max_by(|a, b| (a.0 - a.1).total_cmp(&(b.0 - b.1)));
        if let Some(&(x, y)) = best {
            let (ux, uy) = ((x - sx) / radius, (y - sy) / radius);
            label((x + 12.0 * ux, y + 12.0 * uy));
        }
        return;
    }
    painter.circle_stroke(origin + vec2(sx as f32, sy as f32), radius as f32, stroke);
    // Prefer the upper right of the circle, then the other diagonals.
    for deg in [-45.0f64, -135.0, 45.0, 135.0] {
        let a = deg.to_radians();
        let pos = (
            sx + (radius + 12.0) * a.cos(),
            sy + (radius + 12.0) * a.sin(),
        );
        if in_rect(pos) {
            label(pos);
            break;
        }
    }
}

fn paint_highlight(painter: &Painter, rect: Rect, hl: Highlight, color: Color32, strong: bool) {
    let p = |x: f64, y: f64| rect.min + vec2(x as f32, y as f32);
    let alpha = if strong { 0.35 } else { 0.2 };
    let glow = Stroke::new(8.0, color.gamma_multiply(alpha));
    match hl {
        Highlight::Point { x, y } => {
            // A light disc with a ring, as the reference shows under the pointer.
            painter.circle(
                p(x, y),
                FREE_POINT_RADIUS + 5.0,
                color.gamma_multiply(alpha * 0.7),
                Stroke::new(1.0, color.gamma_multiply(0.6)),
            );
        }
        Highlight::Stroke { x1, y1, x2, y2 } => {
            painter.line_segment([p(x1, y1), p(x2, y2)], glow);
        }
        Highlight::Circle { x, y, r } if r > arc::ARC_RADIUS => {
            let (w, h) = (rect.width() as f64, rect.height() as f64);
            if let Some(pts) = arc::visible_arc(x, y, r, w, h) {
                let points = pts.iter().map(|&(x, y)| p(x, y)).collect();
                painter.add(Shape::line(points, glow));
            }
        }
        Highlight::Circle { x, y, r } => {
            painter.circle_stroke(p(x, y), r as f32, glow);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::Camera;
    use eframe::egui::Context;

    /// How many shapes a frame of `script` puts on the canvas.
    fn shapes(script: &str) -> usize {
        let wb = crate::workbench::Workbench::from_script(script).unwrap();
        let c = wb.construction();
        let t = Camera::STANDARD.transform(800.0, 600.0);
        let ctx = Context::default();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            let rect = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
            let scene = Scene {
                visible: &|_| true,
                ghost: &|_| false,
                selected: None,
                hovered: None,
                palette: wb.palette(),
                cull: true,
                generation: wb.rebuilds(),
                label_offset: &|_| None,
                label_rects: None,
            };
            paint(ui.painter(), rect, c, &t, &scene);
        });
        out.textures_delta.clear();
        out.shapes.len()
    }

    #[test]
    fn a_huge_circle_is_drawn_as_an_arc_not_skipped() {
        // Radius about 5e5 units = 2.5e7 px, far above the old 1e5 px cut-off,
        // and its rim passes through the middle of the picture.
        let base = shapes("A = (0, 0)\nB = (1, 0)\n");
        let with = shapes("A = (0, 0)\nB = (1, 0)\nC = (0, 1000000)\nk = Circle(A, B, C)\n");
        assert!(with > base + 1, "no arc was painted ({with} vs {base})");
    }

    #[test]
    fn an_angle_draws_a_sector_and_a_label() {
        let base = shapes("A = (1, 0)\nB = (0, 0)\nC = (1, 1)\n");
        let with = shapes("A = (1, 0)\nB = (0, 0)\nC = (1, 1)\nα = Angle(A, B, C)\n");
        assert!(with >= base + 3, "{with} vs {base}");
    }

    #[test]
    fn a_function_draws_its_graph_and_label() {
        let base = shapes("A = (5, 5)\n");
        assert_eq!(shapes("A = (5, 5)\nf(x) = x^2\n"), base + 2);
    }

    #[test]
    fn a_conic_draws_its_curve_and_label() {
        let base = shapes("A = (0, 0)\nB = (2, 0)\nC = (1, 2)\n");
        let with = shapes("A = (0, 0)\nB = (2, 0)\nC = (1, 2)\ne = Ellipse(A, B, C)\n");
        assert_eq!(with, base + 2);
    }
}
