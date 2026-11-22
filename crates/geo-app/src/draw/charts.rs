//! Canvas geometry for the kernel's chart objects.

use crate::interact::ViewTransform;
use eframe::egui::emath::TSTransform;
use eframe::egui::epaint::{RectShape, TessellationOptions, Tessellator};
use eframe::egui::{
    vec2, Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, TextureHandle, Vec2,
};
use geo_kernel::algo::charts::{ChartData, ChartMark, Pie};
use geo_kernel::kernel::{Construction, DrawingStyle, Id, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod raster;

#[derive(Default)]
struct Cache {
    generation: u64,
    entries: HashMap<Id, Entry>,
}

#[derive(PartialEq)]
struct Key {
    revision: u64,
    scale: [f64; 2],
    color: Color32,
    thickness: u8,
    pixels_per_point: f32,
    options: TessellationOptions,
}

struct Entry {
    key: Key,
    origin: (f64, f64),
    drawing: Drawing,
    bounds: Rect,
    label: Option<Pos2>,
}

enum Drawing {
    Mesh(Arc<Mesh>),
    Raster(TextureHandle),
}

#[cfg(test)]
thread_local! {
    static BUILDS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn builds() -> u64 {
    BUILDS.with(std::cell::Cell::get)
}

pub(super) struct Frame<'a> {
    painter: &'a Painter,
    rect: Rect,
    view: ViewTransform,
    cache: Arc<Mutex<Cache>>,
    pixels_per_point: f32,
    options: TessellationOptions,
}

impl<'a> Frame<'a> {
    pub(super) fn new(
        painter: &'a Painter,
        rect: Rect,
        c: &Construction,
        view: &ViewTransform,
        generation: u64,
    ) -> Option<Self> {
        let cache_id = eframe::egui::Id::unique("construction chart drawings");
        if c.charts().len() == 0 {
            if let Some(cache) = painter
                .ctx()
                .data(|data| data.get_temp::<Arc<Mutex<Cache>>>(cache_id))
            {
                cache
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .entries
                    .clear();
            }
            return None;
        }
        let cache = painter.ctx().data_mut(|data| {
            data.get_temp_mut_or_default::<Arc<Mutex<Cache>>>(cache_id)
                .clone()
        });
        {
            let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
            if cache.generation != generation {
                cache.generation = generation;
                cache.entries.clear();
            }
        }
        Some(Self {
            painter,
            rect,
            view: *view,
            cache,
            pixels_per_point: painter.ctx().pixels_per_point(),
            options: painter.ctx().tessellation_options(|o| *o),
        })
    }

    pub(super) fn draw(
        &self,
        c: &Construction,
        id: Id,
        chart: &ChartData,
        color: Color32,
    ) -> Option<Pos2> {
        let DrawingStyle {
            line_thickness,
            show_label,
            ..
        } = c.drawing_style(id);
        let key = Key {
            revision: chart.revision(),
            scale: [self.view.xscale(), self.view.yscale()],
            color,
            thickness: line_thickness,
            pixels_per_point: self.pixels_per_point,
            options: self.options,
        };
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if cache.entries.get(&id).is_none_or(|entry| entry.key != key) {
            cache.entries.insert(id, build(key, chart, self.painter));
        }

        let drawing = &cache.entries[&id];
        let (x, y) = self
            .view
            .world_to_screen(drawing.origin.0, drawing.origin.1);
        let offset = self.rect.min.to_vec2() + vec2(x as f32, y as f32);
        let bounds = drawing.bounds.translate(offset);
        if self.rect.intersects(bounds) {
            match &drawing.drawing {
                Drawing::Mesh(mesh) if !mesh.is_empty() => {
                    let idx = self.painter.add(Shape::Mesh(mesh.clone()));
                    self.painter.ctx().graphics_mut(|layers| {
                        layers
                            .entry(self.painter.layer_id())
                            .mutate_shape(idx, |shape| {
                                shape.transform_after_tessellation =
                                    TSTransform::from_translation(offset);
                            });
                    });
                }
                Drawing::Raster(texture) => {
                    self.painter.image(
                        texture.id(),
                        bounds,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                Drawing::Mesh(_) => {}
            }
        }
        drawing
            .label
            .map(|p| p + offset)
            .filter(|_| show_label && c.value(id) == Value::PieChart)
    }
}

fn build(key: Key, chart: &ChartData, painter: &Painter) -> Entry {
    #[cfg(test)]
    BUILDS.with(|n| n.set(n.get() + 1));

    // Only the scales affect stroke geometry and density stacks. Use a
    // chart-local origin and let the painter place and clip it on every frame.
    let [sx, sy] = key.scale;
    let stacked;
    let marks = if chart.dot_stack.is_some() {
        stacked = chart.render_marks(sx, sy).collect::<Vec<_>>();
        stacked.as_slice()
    } else {
        chart.marks.as_slice()
    };
    let world = chart
        .pie
        .as_ref()
        .map(|pie| {
            let ((x, y), r) = (pie.center, pie.radius);
            [x - r, y - r, x + r, y + r]
        })
        .or_else(|| ChartMark::bounds(marks))
        .unwrap_or([0.0; 4]);
    let origin = (world[0].midpoint(world[2]), world[1].midpoint(world[3]));
    let screen =
        |(x, y): (f64, f64)| Pos2::new(((x - origin.0) * sx) as f32, ((origin.1 - y) * sy) as f32);
    let stroke = Stroke::new(f32::from(key.thickness) * 0.5, key.color);
    let mut options = key.options;
    options.round_line_segments_to_pixels = false;
    options.round_rects_to_pixels = false;
    let mut tess = Tessellator::new(key.pixels_per_point, options, [1, 1], Vec::new());
    let mut mesh = Mesh::default();
    if let Some(pie) = &chart.pie {
        let radius = vec2((pie.radius * sx) as f32, (pie.radius * sy) as f32);
        let label = pie_mesh(
            &key,
            &mut tess,
            &mut mesh,
            screen(pie.center),
            radius,
            pie,
            stroke,
        );
        return Entry {
            key,
            origin,
            bounds: mesh.calc_bounds(),
            drawing: Drawing::Mesh(Arc::new(mesh)),
            label,
        };
    }

    let radius: f32 = if chart.dot_stack.is_some() { 5.0 } else { 4.0 };
    let extent = Rect::from_two_pos(screen((world[0], world[1])), screen((world[2], world[3])))
        .expand(radius.max(4.0) + stroke.width + 2.0);
    let mut raster = (marks.len() >= 256)
        .then(|| raster::Layer::new(extent, key.pixels_per_point, painter.ctx()))
        .flatten();
    let mut draw = |shape| {
        if let Some(layer) = &mut raster {
            layer.draw(shape);
        } else {
            tess.tessellate_shape(shape, &mut mesh);
        }
    };
    for &mark in marks {
        let shape = match mark {
            ChartMark::Bar {
                left,
                right,
                height,
            } if height != 0.0 => {
                let rect = Rect::from_two_pos(screen((left, 0.0)), screen((right, height)));
                if !rect.is_finite() {
                    continue;
                }
                if left >= right {
                    Shape::line_segment([screen((left, 0.0)), screen((left, height))], stroke)
                } else {
                    Shape::Rect(RectShape::new(
                        rect,
                        0.0,
                        key.color.gamma_multiply(0.24),
                        stroke,
                        StrokeKind::Middle,
                    ))
                }
            }
            ChartMark::Box {
                x: [left, right],
                y: [bottom, top],
            } => {
                let rect = Rect::from_two_pos(screen((left, bottom)), screen((right, top)));
                if !rect.is_finite() {
                    continue;
                }
                Shape::rect_stroke(rect, 0.0, stroke, StrokeKind::Middle)
            }
            ChartMark::Segment(ends) => {
                let ends = ends.map(screen);
                if !ends.iter().all(|p| p.is_finite()) {
                    continue;
                }
                Shape::line_segment(ends, stroke)
            }
            ChartMark::Point { at, open } => {
                let p = screen(at);
                if !p.is_finite() {
                    continue;
                }
                if open {
                    Shape::circle_stroke(p, radius, stroke)
                } else {
                    Shape::circle_filled(p, radius, key.color)
                }
            }
            ChartMark::Cross(at) => {
                let p = screen(at);
                if p.is_finite() {
                    for d in [vec2(4.0, 4.0), vec2(4.0, -4.0)] {
                        draw(Shape::line_segment([p - d, p + d], stroke));
                    }
                }
                continue;
            }
            _ => continue,
        };
        draw(shape);
    }
    let (drawing, bounds) = if let Some(layer) = raster {
        let (texture, bounds) = layer.finish(painter.ctx());
        (Drawing::Raster(texture), bounds)
    } else {
        let bounds = mesh.calc_bounds();
        (Drawing::Mesh(Arc::new(mesh)), bounds)
    };
    Entry {
        key,
        origin,
        drawing,
        bounds,
        label: None,
    }
}

fn pie_mesh(
    key: &Key,
    tess: &mut Tessellator,
    mesh: &mut Mesh,
    center: Pos2,
    radius: Vec2,
    pie: &Pie,
    stroke: Stroke,
) -> Option<Pos2> {
    if !center.is_finite() || !radius.is_finite() || radius.min_elem() <= 0.0 {
        return None;
    }
    let at = |turn: f64| {
        let (sin, cos) = (std::f64::consts::TAU * turn).sin_cos();
        center + vec2(cos as f32 * radius.x, sin as f32 * radius.y)
    };
    for (index, turn, fraction) in pie.slices().filter(|&(_, _, fraction)| fraction > 0.0) {
        let [r, g, b] = Pie::slice_color(index);
        let color = Color32::from_rgb(r, g, b).gamma_multiply(f32::from(key.color.a()) / 255.0);
        let first = mesh.vertices.len() as u32;
        let steps = (fraction * f64::from(radius.max_elem()).sqrt() * 5.0)
            .ceil()
            .clamp(1.0, 256.0) as u32;
        mesh.colored_vertex(center, color);
        for step in 0..=steps {
            mesh.colored_vertex(
                at(turn + fraction * f64::from(step) / f64::from(steps)),
                color,
            );
            if step != 0 {
                mesh.add_triangle(first, first + step, first + step + 1);
            }
        }
    }
    if stroke.width > 0.0 {
        for (_, turn, fraction) in pie.slices() {
            let end = turn + fraction;
            if end.is_finite() {
                tess.tessellate_shape(Shape::line_segment([center, at(end)], stroke), mesh);
            }
        }
        tess.tessellate_shape(Shape::ellipse_stroke(center, radius, stroke), mesh);
    }
    Some(center + radius * std::f32::consts::FRAC_1_SQRT_2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{headless, view::Camera};
    use eframe::egui;

    #[test]
    fn pie_uses_both_screen_radii_and_draws_its_rim_and_rays() {
        let c = geo_kernel::script::load("p = PieChart({1, 1, 1, 1})").unwrap();
        let pie = c.chart(c.id("p").unwrap()).unwrap().pie.as_ref().unwrap();
        let center = Pos2::new(200.0, 150.0);
        let radius = vec2(90.0, 30.0);
        let key = Key {
            revision: 0,
            scale: [50.0; 2],
            color: Color32::BLACK,
            thickness: 3,
            pixels_per_point: 1.0,
            options: TessellationOptions::default(),
        };
        let mut tess = Tessellator::new(1.0, key.options, [1, 1], Vec::new());
        let mut fill = Mesh::default();
        let label = pie_mesh(
            &key,
            &mut tess,
            &mut fill,
            center,
            radius,
            pie,
            Stroke::NONE,
        );
        assert_eq!(fill.calc_bounds().size(), radius * 2.0);
        assert_eq!(
            label,
            Some(center + radius * std::f32::consts::FRAC_1_SQRT_2)
        );
        let mut outlined = Mesh::default();
        pie_mesh(
            &key,
            &mut tess,
            &mut outlined,
            center,
            radius,
            pie,
            Stroke::new(1.5, Color32::BLACK),
        );
        assert!(outlined.indices.len() > fill.indices.len());
        assert!(outlined.calc_bounds().contains_rect(fill.calc_bounds()));
    }

    #[test]
    fn histogram_meshes_follow_a_density_change() {
        let mut c = geo_kernel::script::load(
            "scale = 2\nchart = Histogram({0, 2, 5}, {0, 1, 2, 5}, true, scale)",
        )
        .unwrap();
        let canvas = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        let view = Camera::STANDARD.transform(800.0, 600.0);
        let ctx = egui::Context::default();
        let frame = |c: &Construction, view: &ViewTransform| {
            let input = headless::raw_input(canvas, 0.0, 1.0 / 60.0, Vec::new());
            let mut out = ctx.run_ui(input, |ui| {
                let painter = ui.ctx().layer_painter(egui::LayerId::background());
                let frame = Frame::new(&painter, canvas, c, view, 0).unwrap();
                let id = c.id("chart").unwrap();
                frame.draw(c, id, c.chart(id).unwrap(), Color32::GREEN);
            });
            out.textures_delta.clear();
            out.shapes
                .into_iter()
                .filter_map(|shape| match shape.shape {
                    Shape::Mesh(mesh) => {
                        Some(shape.transform_after_tessellation * mesh.calc_bounds())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let before = frame(&c, &view);
        let built = builds();
        assert_eq!(before, frame(&c, &view));
        assert_eq!(builds(), built);
        let mut camera = Camera::STANDARD;
        camera.pan_pixels(20.0, 10.0);
        assert_eq!(
            frame(&c, &camera.transform(800.0, 600.0)),
            [before[0].translate(vec2(20.0, 10.0))]
        );
        assert_eq!(builds(), built);
        c.set_number(c.id("scale").unwrap(), 3.0).unwrap();
        let after = frame(&c, &view);
        assert_eq!(builds(), built + 1);
        assert_eq!(before.len(), 1);
        assert_eq!(after.len(), before.len());
        for (before, after) in before.into_iter().zip(after) {
            assert_eq!(before.width(), after.width());
            assert!(before.height() < after.height());
        }
    }
}
