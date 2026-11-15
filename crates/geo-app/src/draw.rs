// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/plugin/EuclidianStyleConstants.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Draws the objects of a construction with `egui::Painter`.

use crate::arc;
use crate::geom::ToEgui;
use crate::highlight::{self, Highlight};
use crate::interact::ViewTransform;
use crate::labelpos;
use crate::shape;
use crate::style;
use crate::theme;
use eframe::egui::{vec2, Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke};
use geo_kernel::algo::vector;
use geo_kernel::kernel::{Construction, Id, Value};

/// Point radii in pixels: point size 5 for free points, 4 for dependent ones.
pub const FREE_POINT_RADIUS: f32 = 5.0;
const DEPENDENT_POINT_RADIUS: f32 = 4.0;
/// Line thickness 5, drawn half as wide.
pub const CURVE_WIDTH: f32 = 2.5;
const LABEL_FONT: f32 = 16.0;
const ANGLE_RADIUS: f32 = 30.0;

/// What the picture needs besides the construction.
pub struct Scene<'a> {
    pub visible: &'a dyn Fn(Id) -> bool,
    /// Hidden objects shown faintly (while the show/hide tool is active).
    pub ghost: &'a dyn Fn(Id) -> bool,
    pub selected: Option<Id>,
    pub hovered: Option<Id>,
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

type Label = (String, (f64, f64), Color32, Align2);

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
        let c = style::color(c, id).egui();
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

    let ids: Vec<Id> = c.ids().filter(|&id| (scene.visible)(id)).collect();
    let mut labels: Vec<Label> = Vec::new();

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
                let glow = Stroke::new(8.0, style::color(c, id).egui().gamma_multiply(alpha));
                painter.add(Shape::closed_line(points, glow));
            }
            continue;
        }
        if let Some(hl) = highlight::of_value(&c.value(id), t, w, h) {
            paint_highlight(painter, rect, hl, style::color(c, id).egui(), strong);
        }
    }

    // Angles first: their sectors lie under the lines.
    for &id in &ids {
        let (Some([a, b, v]), Value::Number(angle)) = (style::angle_points(c, id), c.value(id))
        else {
            continue;
        };
        let (sa, sb, sv) = (screen(a.0, a.1), screen(b.0, b.1), screen(v.0, v.1));
        // The sector is measured in the world; screen angles are mirrored.
        let Some((start, sweep)) = style::sector((sa.0, -sa.1), (sb.0, -sb.1), (sv.0, -sv.1))
        else {
            continue;
        };
        let vertex = origin + vec2(sb.0 as f32, sb.1 as f32);
        let col = color(id);
        let at = |ang: f64, r: f32| vertex + vec2(ang.cos() as f32, ang.sin() as f32) * r;
        if style::is_right(angle) {
            let side = ANGLE_RADIUS * 0.7;
            let corner = [
                vertex,
                at(start, side),
                at(start, side) + (at(start + sweep, side) - vertex),
                at(start + sweep, side),
            ];
            painter.add(Shape::convex_polygon(
                corner.to_vec(),
                col.gamma_multiply(0.1),
                Stroke::new(1.5, col),
            ));
            painter.circle_filled(at(start + sweep / 2.0, side * 0.7), 1.8, col);
        } else {
            let n = 32;
            let mut pts = vec![vertex];
            pts.extend((0..=n).map(|i| at(start + sweep * i as f64 / n as f64, ANGLE_RADIUS)));
            painter.add(Shape::Path(eframe::egui::epaint::PathShape {
                points: pts.clone(),
                closed: true,
                fill: col.gamma_multiply(0.1),
                stroke: Stroke::NONE.into(),
            }));
            painter.add(Shape::line(pts[1..].to_vec(), Stroke::new(1.5, col)));
        }
        // The label sits outside the sector, on its bisector, and grows away
        // from the vertex.
        let half = start + sweep / 2.0;
        let mid = at(half, ANGLE_RADIUS + 6.0) - origin;
        let (dx, dy) = (half.cos(), half.sin());
        let h = if dx > 0.3 {
            eframe::egui::Align::Min
        } else if dx < -0.3 {
            eframe::egui::Align::Max
        } else {
            eframe::egui::Align::Center
        };
        let v = if dy > 0.3 {
            eframe::egui::Align::Min
        } else if dy < -0.3 {
            eframe::egui::Align::Max
        } else {
            eframe::egui::Align::Center
        };
        labels.push((
            format!("{} = {}", c.label(id), crate::describe::degrees(angle)),
            (mid.x as f64, mid.y as f64),
            col,
            Align2([h, v]),
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
                labels.push((
                    c.label(id).to_owned(),
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
                labels.push((
                    c.label(id).to_owned(),
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
                labels.push((
                    c.label(id).to_owned(),
                    pos,
                    color(id),
                    Align2::CENTER_CENTER,
                ));
            });
        }
    }
    for &id in &ids {
        if let Value::Segment { x1, y1, x2, y2 } = c.value(id) {
            if ![x1, y1, x2, y2].iter().all(|v| v.is_finite()) {
                continue;
            }
            painter.line_segment([p(x1, y1), p(x2, y2)], curve(id));
            let pos = labelpos::segment_label(screen(x1, y1), screen(x2, y2), 12.0);
            labels.push((
                c.label(id).to_owned(),
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
        labels.push((
            c.label(id).to_owned(),
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
            labels.push((
                c.label(id).to_owned(),
                (sx + 6.0, sy - 6.0),
                color(id),
                Align2::LEFT_BOTTOM,
            ));
        }
    }
    let font = FontId::proportional(LABEL_FONT);
    for (text, (sx, sy), color, align) in labels {
        if sx < -20.0 || sy < -20.0 || sx > w + 20.0 || sy > h + 20.0 {
            continue;
        }
        painter.text(
            origin + vec2(sx as f32, sy as f32),
            align,
            text,
            font.clone(),
            color,
        );
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
}
