//! Textured quads for imported rasters.

use crate::interact::ViewTransform;
use eframe::egui::cache::{ComputerMut, FrameCache};
use eframe::egui::epaint::{mutex::RwLock, textures::TextureManager, Vertex};
use eframe::egui::{
    vec2, Color32, ColorImage, Mesh, Painter, Pos2, Rect, Shape, TextureHandle, TextureOptions,
};
use geo_kernel::kernel::image::RasterAsset;
use geo_kernel::{Construction, Id, Value};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};

#[derive(Clone, Copy)]
struct TextureKey<'a> {
    asset: &'a Arc<RasterAsset>,
    manager: &'a Arc<RwLock<TextureManager>>,
    interpolate: bool,
}

impl Hash for TextureKey<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(self.asset).hash(state);
        self.interpolate.hash(state);
    }
}

#[derive(Default)]
struct Upload;

impl ComputerMut<TextureKey<'_>, (Weak<RasterAsset>, TextureHandle)> for Upload {
    fn compute(&mut self, key: TextureKey<'_>) -> (Weak<RasterAsset>, TextureHandle) {
        let image =
            ColorImage::from_rgba_unmultiplied(key.asset.size.map(|n| n as usize), &key.asset.rgba);
        let options = if key.interpolate {
            TextureOptions::LINEAR
        } else {
            TextureOptions::NEAREST
        };
        let id = key
            .manager
            .write()
            .alloc("construction image".into(), image.into(), options);
        // The weak pointer keeps the cache key's address reserved until eviction.
        (
            Arc::downgrade(key.asset),
            TextureHandle::new(key.manager.clone(), id),
        )
    }
}

type Textures = FrameCache<(Weak<RasterAsset>, TextureHandle), Upload>;

pub(crate) fn paint(
    painter: &Painter,
    rect: Rect,
    c: &Construction,
    view: &ViewTransform,
    background: bool,
    visible: impl Fn(Id) -> bool,
    ghost: impl Fn(Id) -> bool,
) {
    let images = c.images();
    if images.len() == 0 {
        return;
    }
    let manager = painter.ctx().tex_manager();
    for (id, image) in images {
        if image.style.background != background || !visible(id) || c.value(id) != Value::Image {
            continue;
        }
        let Some(quad) = c.image_quad(id, view.scale()) else {
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
        let [w, h] = image.asset.size.map(f64::from);
        let [mut u0, mut v0, mut u1, mut v1] = [0.0, 0.0, 1.0, 1.0];
        if let Some([x, y, width, height]) =
            image.style.crop.filter(|_| !image.placement.is_screen())
        {
            u0 = (x.trunc() / w).clamp(0.0, 1.0);
            v0 = (y.trunc() / h).clamp(0.0, 1.0);
            u1 = ((x.trunc() + width.trunc()) / w).clamp(0.0, 1.0);
            v1 = ((y.trunc() + height.trunc()) / h).clamp(0.0, 1.0);
            if u1 <= u0 || v1 <= v0 {
                continue;
            }
        }
        let alpha = image.style.alpha * if ghost(id) { 0.3 } else { 1.0 };
        let color = Color32::from_white_alpha((alpha * 255.0).round() as u8);
        let texture = painter.ctx().memory_mut(|mem| {
            mem.caches
                .cache::<Textures>()
                .get(TextureKey {
                    asset: &image.asset,
                    manager: &manager,
                    interpolate: image.style.interpolate,
                })
                .1
                .id()
        });
        let [top_left, top_right, _, bottom_left] = positions;
        let right = top_right - top_left;
        let down = bottom_left - top_left;
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
                for background in [true, false] {
                    paint(
                        &p,
                        canvas,
                        c,
                        &view,
                        background,
                        |id| w.is_visible(id),
                        |_| false,
                    );
                }
            });
            let uploaded = out
                .textures_delta
                .set
                .iter()
                .filter(|(id, _)| **id != egui::TextureId::default())
                .count();
            out.textures_delta.clear();
            let meshes = out
                .shapes
                .into_iter()
                .filter_map(|shape| match shape.shape {
                    Shape::Mesh(mesh) => Some(mesh),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (uploaded, meshes)
        };
        let (uploaded, before) = frame(&w, camera);
        assert_eq!(uploaded, 3);
        assert_eq!(before.len(), 5);
        let [_, fixed, world, anchored, _] = before.as_slice() else {
            unreachable!()
        };
        assert_eq!(fixed.vertices[0].pos, canvas.min + vec2(41.0, 55.0));
        assert_eq!(fixed.texture_id, world.texture_id);
        assert_eq!(world.texture_id, anchored.texture_id);
        assert_eq!(anchored.vertices[0].uv, Pos2::new(2.0 / 12.0, 1.0 / 8.0));
        assert_eq!(anchored.vertices[0].color, Color32::from_white_alpha(128));

        let id = w.construction().id("A").unwrap();
        assert!(w.move_point(id, -3.0, 0.0));
        camera.scale *= 1.25;
        camera.cx += 1.0;
        let (uploaded, after) = frame(&w, camera);
        assert_eq!(uploaded, 0);
        assert_eq!(after.len(), 5);
        assert_eq!(after[1].vertices, fixed.vertices);
        assert_ne!(after[2].vertices, world.vertices);
        assert_ne!(after[3].vertices, anchored.vertices);
        assert_eq!(after[3].texture_id, anchored.texture_id);
    }
}
