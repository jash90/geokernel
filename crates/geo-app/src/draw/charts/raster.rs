//! A retained raster layer for a dense chart with a small projected extent.

use eframe::egui::{
    Color32, ColorImage, Context, Pos2, Rect, Shape, Stroke, TextureHandle, TextureOptions,
};
use vello_cpu::{
    color::AlphaColor,
    kurbo::{self, Affine, Shape as _},
    Pixmap, RenderContext, Resources,
};

pub(super) struct Layer {
    raster: RenderContext,
    bounds: Rect,
}

impl Layer {
    pub(super) fn new(bounds: Rect, pixels_per_point: f32, ctx: &Context) -> Option<Self> {
        let to_pixels = |p: Pos2| p * pixels_per_point;
        let min = to_pixels(bounds.min).floor();
        let max = to_pixels(bounds.max).ceil();
        let size = max - min;
        let side = ctx.input(|input| input.max_texture_side).min(2048) as f32;
        // A chart too large for a small texture still uses its vector mesh.
        if !size.is_finite()
            || size.min_elem() <= 0.0
            || size.max_elem() > side
            || size.x * size.y > 512.0 * 1024.0
        {
            return None;
        }
        let mut raster = RenderContext::new(size.x as u16, size.y as u16);
        raster.set_transform(
            Affine::translate((-f64::from(min.x), -f64::from(min.y)))
                * Affine::scale(f64::from(pixels_per_point)),
        );
        let min = min / pixels_per_point;
        Some(Self {
            raster,
            bounds: Rect::from_min_size(min, size / pixels_per_point),
        })
    }

    pub(super) fn draw(&mut self, shape: Shape) {
        match shape {
            Shape::Rect(shape) => {
                let rect = kurbo::Rect::from_points(point(shape.rect.min), point(shape.rect.max));
                if shape.fill != Color32::TRANSPARENT {
                    self.raster.set_paint(paint(shape.fill));
                    self.raster.fill_rect(&rect);
                }
                self.stroke(&rect.to_path(0.1), shape.stroke);
            }
            Shape::LineSegment { points, stroke } => {
                let line = kurbo::Line::new(point(points[0]), point(points[1]));
                self.stroke(&line.to_path(0.1), stroke);
            }
            Shape::Circle(shape) => {
                let path =
                    kurbo::Circle::new(point(shape.center), f64::from(shape.radius)).to_path(0.1);
                if shape.fill != Color32::TRANSPARENT {
                    self.raster.set_paint(paint(shape.fill));
                    self.raster.fill_path(&path);
                }
                self.stroke(&path, shape.stroke);
            }
            _ => unreachable!("chart mark shape"),
        }
    }

    fn stroke(&mut self, path: &kurbo::BezPath, stroke: Stroke) {
        if !stroke.is_empty() {
            self.raster
                .set_stroke(kurbo::Stroke::new(f64::from(stroke.width)));
            self.raster.set_paint(paint(stroke.color));
            self.raster.stroke_path(path);
        }
    }

    pub(super) fn finish(self, ctx: &Context) -> (TextureHandle, Rect) {
        let Layer { raster, bounds } = self;
        let (width, height) = (raster.width(), raster.height());
        let mut pixmap = Pixmap::new(width, height);
        raster.render(&mut pixmap, &mut Resources::new());
        let pixels = pixmap
            .data_as_u8_slice()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|px| Color32::from_rgba_premultiplied(px[0], px[1], px[2], px[3]))
            .collect();
        let image = ColorImage::new([usize::from(width), usize::from(height)], pixels);
        (
            ctx.load_texture("chart layer", image, TextureOptions::LINEAR),
            bounds,
        )
    }
}

fn point(p: Pos2) -> kurbo::Point {
    (f64::from(p.x), f64::from(p.y)).into()
}

fn paint(color: Color32) -> AlphaColor<vello_cpu::color::Srgb> {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    AlphaColor::from_rgba8(r, g, b, a)
}
