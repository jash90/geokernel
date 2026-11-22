//! Textured quads for imported rasters.

use crate::interact::ViewTransform;
use eframe::egui::epaint::Vertex;
use eframe::egui::{
    vec2, Color32, ColorImage, Mesh, Painter, Pos2, Rect, Shape, TextureHandle, TextureOptions,
};
use geo_kernel::kernel::image::RasterImage;
use geo_kernel::{Construction, Id, Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Textures {
    handles: HashMap<(u64, bool), TextureHandle>,
    live: HashSet<u64>,
}

impl Textures {
    fn retain_live(&mut self, c: &Construction) {
        if !self.handles.is_empty() {
            self.live.clear();
            self.live
                .extend(c.images().map(|(_, image)| image.asset.id()));
            self.handles.retain(|(id, _), _| self.live.contains(id));
        }
    }

    fn get(
        &mut self,
        painter: &Painter,
        image: &RasterImage,
        interpolate: bool,
    ) -> eframe::egui::TextureId {
        self.handles
            .entry((image.asset.id(), interpolate))
            .or_insert_with(|| {
                let pixels = ColorImage::from_rgba_unmultiplied(
                    image.asset.size().map(|n| n as usize),
                    image.asset.rgba(),
                );
                let options = if interpolate {
                    TextureOptions::LINEAR
                } else {
                    TextureOptions::NEAREST
                };
                let manager = painter.ctx().tex_manager();
                let id = manager
                    .write()
                    .alloc("construction image".into(), pixels.into(), options);
                TextureHandle::new(manager, id)
            })
            .id()
    }
}

fn textures(painter: &Painter) -> Arc<Mutex<Textures>> {
    // This belongs to the window and lives across empty or offscreen frames.
    painter.ctx().data_mut(|data| {
        data.get_temp_mut_or_default::<Arc<Mutex<Textures>>>(eframe::egui::Id::unique(
            "construction raster textures",
        ))
        .clone()
    })
}

pub(crate) fn paint_background(
    painter: &Painter,
    rect: Rect,
    c: &Construction,
    view: &ViewTransform,
    visible: &dyn Fn(Id) -> bool,
    ghost: &dyn Fn(Id) -> bool,
) {
    let cache_id = eframe::egui::Id::unique("construction raster textures");
    if let Some(cache) = painter
        .ctx()
        .data(|data| data.get_temp::<Arc<Mutex<Textures>>>(cache_id))
    {
        cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain_live(c);
    }
    paint(painter, rect, c, view, None, visible, ghost);
}

/// `None` paints all background images. A layer paints its foreground images.
pub(crate) fn paint(
    painter: &Painter,
    rect: Rect,
    c: &Construction,
    view: &ViewTransform,
    layer: Option<u8>,
    visible: &dyn Fn(Id) -> bool,
    ghost: &dyn Fn(Id) -> bool,
) {
    let images = c.images();
    if images.len() == 0 {
        return;
    }
    let mut images: Vec<_> = images
        .filter(|&(id, image)| {
            let in_layer = if let Some(layer) = layer {
                !image.style.background && c.drawing_style(id).layer == layer
            } else {
                image.style.background
            };
            in_layer && visible(id) && c.value(id) == Value::Image
        })
        .collect();
    if images.is_empty() {
        return;
    }
    if layer.is_none() {
        images.sort_unstable_by_key(|&(id, _)| c.drawing_order(id));
    }
    let cache = textures(painter);
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    for (id, image) in images {
        let Some(quad) = c.image_quad_with_scales(id, view.xscale(), view.yscale()) else {
            continue;
        };
        let positions = quad.map(|[x, y]| {
            let (x, y) = if image.placement.is_screen() {
                (x, y)
            } else {
                view.world_to_screen(x, y)
            };
            rect.min + vec2(x as f32, y as f32)
        });
        if !positions.iter().all(|p| p.x.is_finite() && p.y.is_finite())
            || !Rect::from_points(&positions).intersects(rect)
        {
            continue;
        }
        let [w, h] = image.asset.size().map(f64::from);
        let Some([x0, y0, x1, y1]) = image.source_rect() else {
            continue;
        };
        let [u0, v0, u1, v1] = [x0 / w, y0 / h, x1 / w, y1 / h];
        let alpha = image.style.alpha * if ghost(id) { 0.3 } else { 1.0 };
        let color = Color32::from_white_alpha((alpha * 255.0).round() as u8);
        let [top_left, top_right, _, bottom_left] = positions;
        let right = top_right - top_left;
        let down = bottom_left - top_left;
        let near = |a: f32, b: f32| (a - b).abs() <= 1e-4;
        let translation = painter.ctx().pixels_per_point() == 1.0
            && near(top_left.x, top_left.x.round())
            && near(top_left.y, top_left.y.round())
            && near(right.x, w as f32)
            && near(right.y, 0.0)
            && near(down.x, 0.0)
            && near(down.y, h as f32);
        let texture = cache.get(painter, image, image.style.interpolate && !translation);
        let mesh = Mesh {
            texture_id: texture,
            indices: vec![0, 1, 2, 0, 2, 3],
            vertices: [(u0, v0), (u1, v0), (u1, v1), (u0, v1)]
                .into_iter()
                .map(|(u, v)| {
                    let (u, v) = (u as f32, v as f32);
                    Vertex {
                        pos: top_left + right * u + down * v,
                        uv: Pos2::new(u, v),
                        color,
                    }
                })
                .collect(),
        };
        painter.add(Shape::mesh(mesh));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headless;
    use crate::view::Camera;
    use crate::Workbench;
    use eframe::egui::{self, Event};

    #[test]
    fn image_meshes_follow_the_camera_and_anchors_and_reuse_their_texture() {
        let mut w = Workbench::default();
        w.load_bytes(
            include_bytes!("../../../../golden/ggb/saved/saved-images-001.ggb"),
            "raster",
        )
        .unwrap();
        let ctx = egui::Context::default();
        let canvas = Rect::from_min_size(Pos2::new(10.0, 20.0), vec2(800.0, 600.0));
        let mut camera = Camera::STANDARD;
        let frame = |w: &Workbench, camera: Camera| {
            let input = headless::raw_input(canvas, 0.0, 1.0 / 60.0, Vec::<Event>::new());
            let c = w.construction();
            let view = camera.transform(800.0, 600.0);
            let mut out = ctx.run_ui(input, |ui| {
                let p = ui
                    .ctx()
                    .layer_painter(egui::LayerId::background())
                    .with_clip_rect(canvas);
                paint_background(&p, canvas, c, &view, &|id| w.is_visible(id), &|_| false);
                paint(
                    &p,
                    canvas,
                    c,
                    &view,
                    Some(0),
                    &|id| w.is_visible(id),
                    &|_| false,
                );
            });
            let uploaded: HashMap<_, _> = out
                .textures_delta
                .set
                .drain()
                .filter(|(id, _)| *id != egui::TextureId::default())
                .filter_map(|(id, updates)| updates.last().map(|delta| (id, delta.options)))
                .collect();
            let freed = std::mem::take(&mut out.textures_delta.free);
            let meshes = out
                .shapes
                .into_iter()
                .filter_map(|shape| match shape.shape {
                    Shape::Mesh(mesh) => Some(mesh),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (uploaded, freed, meshes)
        };
        let (uploaded, _, before) = frame(&w, camera);
        assert_eq!(uploaded.len(), 4);
        assert_eq!(before.len(), 5);
        let [back, fixed, world, anchored, _] = before.as_slice() else {
            unreachable!()
        };
        assert_eq!(fixed.vertices[0].pos, canvas.min + vec2(41.0, 55.0));
        assert_eq!(uploaded[&fixed.texture_id], TextureOptions::NEAREST);
        assert_eq!(uploaded[&world.texture_id], TextureOptions::LINEAR);
        assert_eq!(world.texture_id, anchored.texture_id);
        assert_eq!(anchored.vertices[0].uv, Pos2::new(2.0 / 12.0, 1.0 / 8.0));
        assert_eq!(anchored.vertices[0].color, Color32::from_white_alpha(128));

        let id = w.construction().id("A").unwrap();
        assert!(w.move_point(id, -3.0, 0.0));
        camera.xscale *= 1.25;
        camera.yscale *= 1.5;
        camera.cx += 1.0;
        // At this zoom the centered image may need its other sampling mode.
        let (_, _, changed) = frame(&w, camera);
        let offscreen = Camera {
            cx: 100_000.0,
            ..camera
        };
        frame(&w, offscreen);
        frame(&w, offscreen);
        let (uploaded, freed, after) = frame(&w, camera);
        assert!(uploaded.is_empty() && freed.is_empty());
        assert_eq!(after, changed);
        assert_eq!(after.len(), 5);
        assert_eq!(after[1].vertices, fixed.vertices);
        assert_ne!(after[2].vertices, world.vertices);
        assert_ne!(after[3].vertices, anchored.vertices);
        assert_eq!(after[3].texture_id, anchored.texture_id);

        let id = w.construction().id("backImage").unwrap();
        assert!(w.delete(id));
        let (_, freed, _) = frame(&w, camera);
        assert!(freed.contains(&back.texture_id));
    }
}
