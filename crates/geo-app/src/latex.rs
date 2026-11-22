//! The per-context math layout and font-atlas cache shared by both panels.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use eframe::egui::epaint::text::{Galley, LayoutJob, TextFormat};
use eframe::egui::epaint::TextShape;
use eframe::egui::{
    pos2, vec2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2,
};
use geo_kernel::kernel::text::TextPresentation;
use geo_latex::{Face, Style, Transform};

const MAX_LAYOUTS: usize = 2048;
const MAX_CACHE_WEIGHT: usize = 6 * 1024 * 1024;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct StyleKey {
    size: u32,
    style: Style,
    face: Face,
}

#[derive(Default)]
struct Cache {
    marker: Option<Arc<Galley>>,
    layouts: HashMap<StyleKey, HashMap<Box<str>, Arc<Formula>>>,
    entries: usize,
    weight: usize,
}

pub(crate) struct FormulaCache(Arc<Mutex<Cache>>);

pub(crate) struct Formula {
    pub(crate) size: Vec2,
    items: Vec<Item>,
}

enum Item {
    Text {
        at: Pos2,
        galley: Arc<Galley>,
        color: Option<Color32>,
        bold_offset: Vec2,
    },
    Rect {
        rect: Rect,
        color: Option<Color32>,
    },
    Geometry {
        shape: Shape,
        color: Option<Color32>,
    },
}

#[cfg(test)]
thread_local! {
    static BUILT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn layouts_built() -> u64 {
    BUILT.with(std::cell::Cell::get)
}

pub(crate) fn face(style: TextPresentation) -> Face {
    Face {
        bold: style.bold,
        italic: style.italic,
        monospace: false,
    }
}

impl FormulaCache {
    pub(crate) fn new(painter: &Painter) -> Self {
        let cache = painter.ctx().data_mut(|data| {
            let id = eframe::egui::Id::unique("math_text_layouts");
            data.get_temp_mut_or_default::<Arc<Mutex<Cache>>>(id)
                .clone()
        });
        let marker = painter.layout_no_wrap(
            String::new(),
            FontId::proportional(16.0),
            Color32::PLACEHOLDER,
        );
        let mut stored = cache.lock().unwrap_or_else(|e| e.into_inner());
        // The atlas can be rebuilt when the font definitions or pixel scale change.
        if stored
            .marker
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, &marker))
        {
            *stored = Cache {
                marker: Some(marker),
                ..Cache::default()
            };
        }
        drop(stored);
        Self(cache)
    }

    pub(crate) fn get(
        &self,
        painter: &Painter,
        source: &str,
        size: f32,
        style: Style,
        face: Face,
    ) -> Arc<Formula> {
        let key = StyleKey {
            size: size.to_bits(),
            style,
            face,
        };
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(layout) = cache
            .layouts
            .get(&key)
            .and_then(|entries| entries.get(source))
        {
            return layout.clone();
        }
        #[cfg(test)]
        BUILT.with(|n| n.set(n.get() + 1));
        let layout = geo_latex::layout(source, size, style, face);
        let glyph_bytes = layout
            .ink
            .iter()
            .map(|ink| match &ink.shape {
                geo_latex::Shape::Glyphs { text, .. } => text.len() * 64,
                _ => 0,
            })
            .sum::<usize>();
        let weight = source.len() + layout.ink.len() * 128 + glyph_bytes;
        let formula = Arc::new(Formula::new(painter, layout));
        if weight <= MAX_CACHE_WEIGHT / 4 {
            if cache.entries >= MAX_LAYOUTS || cache.weight + weight > MAX_CACHE_WEIGHT {
                cache.layouts.clear();
                cache.weight = 0;
                cache.entries = 0;
            }
            cache
                .layouts
                .entry(key)
                .or_default()
                .insert(source.into(), formula.clone());
            cache.entries += 1;
            cache.weight += weight;
        }
        formula
    }
}

impl Formula {
    fn new(painter: &Painter, layout: geo_latex::Layout) -> Self {
        let [width, height] = layout.size();
        let mut items = Vec::with_capacity(layout.ink.len());
        for ink in layout.ink {
            let mut transform = ink.transform;
            transform.origin[1] += layout.height;
            let color = ink.color.map(|[r, g, b]| Color32::from_rgb(r, g, b));
            match ink.shape {
                geo_latex::Shape::Glyphs { text, size, face } => {
                    let (at, galley) = text_galley(painter, text, size, face, transform);
                    let [x, y] = transform.vector([
                        if face.bold {
                            (size * 0.042).max(0.65)
                        } else {
                            0.0
                        },
                        0.0,
                    ]);
                    items.push(Item::Text {
                        at,
                        galley,
                        color,
                        bold_offset: vec2(x, y),
                    });
                }
                geo_latex::Shape::Stroke { points, width } => {
                    let points = points
                        .into_iter()
                        .map(|p| Pos2::from(transform.point(p)))
                        .collect();
                    let scale = (transform.x[0] * transform.y[1] - transform.x[1] * transform.y[0])
                        .abs()
                        .sqrt();
                    let shape = Shape::line(
                        points,
                        Stroke::new((scale * width).max(0.3), Color32::PLACEHOLDER),
                    );
                    items.push(Item::Geometry { shape, color });
                }
                geo_latex::Shape::Fill { points }
                    if transform.translation_only() && points.len() == 4 =>
                {
                    let rect = Rect::from_two_pos(
                        Pos2::from(transform.point(points[0])),
                        Pos2::from(transform.point(points[2])),
                    );
                    items.push(Item::Rect { rect, color });
                }
                geo_latex::Shape::Fill { points } => {
                    let mut points: Vec<_> = points
                        .into_iter()
                        .map(|p| Pos2::from(transform.point(p)))
                        .collect();
                    // epaint's filled path expects clockwise vertices.
                    if transform.x[0] * transform.y[1] < transform.x[1] * transform.y[0] {
                        points.reverse();
                    }
                    let shape = Shape::convex_polygon(points, Color32::PLACEHOLDER, Stroke::NONE);
                    items.push(Item::Geometry { shape, color });
                }
            }
        }
        Self {
            size: vec2(width, height),
            items,
        }
    }

    pub(crate) fn paint(&self, painter: &Painter, top_left: Pos2, base: Color32) {
        if !painter
            .clip_rect()
            .intersects(Rect::from_min_size(top_left, self.size).expand(2.0))
        {
            return;
        }
        for item in &self.items {
            match item {
                Item::Text {
                    at,
                    galley,
                    color,
                    bold_offset,
                } => {
                    let color = color.unwrap_or(base);
                    let at = top_left + at.to_vec2();
                    painter.add(TextShape::new(at, galley.clone(), color));
                    if *bold_offset != Vec2::ZERO {
                        painter.add(TextShape::new(at + *bold_offset, galley.clone(), color));
                    }
                }
                Item::Rect { rect, color } => {
                    painter.rect_filled(
                        rect.translate(top_left.to_vec2()),
                        0.0,
                        color.unwrap_or(base),
                    );
                }
                Item::Geometry { shape, color } => {
                    let mut shape = shape.clone();
                    if let Shape::Path(path) = &mut shape {
                        let color = color.unwrap_or(base);
                        if path.stroke.is_empty() {
                            path.fill = color;
                        } else {
                            path.stroke.color = eframe::egui::epaint::ColorMode::Solid(color);
                        }
                    }
                    shape.translate(top_left.to_vec2());
                    painter.add(shape);
                }
            }
        }
    }
}

fn text_galley(
    painter: &Painter,
    text: String,
    size: f32,
    face: Face,
    mut transform: Transform,
) -> (Pos2, Arc<Galley>) {
    // Rasterize a tall delimiter closer to its final size before stretching
    // the ink. Reuse the same font atlas and cap the oversampling for large arrays.
    let stretch = vec2(transform.x[0], transform.x[1])
        .length()
        .max(vec2(transform.y[0], transform.y[1]).length());
    let raster_size = (size * stretch.clamp(1.0, 4.0)).min(256.0);
    if raster_size != size {
        let factor = size / raster_size;
        transform.x = transform.x.map(|n| n * factor);
        transform.y = transform.y.map(|n| n * factor);
    }
    let size = raster_size;
    let mut job = LayoutJob::default();
    let mut start = 0;
    let mut mono = face.monospace;
    let normal = FontId::proportional(size);
    let font = |mono| {
        if mono {
            FontId::monospace(size)
        } else {
            normal.clone()
        }
    };
    let format = |mono| {
        let mut f = TextFormat::simple(font(mono), Color32::PLACEHOLDER);
        f.italics = face.italic;
        f
    };
    job.break_on_newline = false;
    if !mono {
        // Hack supplies mathematical glyphs (including sums and integrals)
        // missing from egui's proportional family.
        painter.fonts_mut(|fonts| {
            for (i, ch) in text.char_indices() {
                let next = !fonts.has_glyph(&normal, ch)
                    && fonts.has_glyph(&FontId::new(size, FontFamily::Monospace), ch);
                if next != mono {
                    if i > start {
                        job.append(&text[start..i], 0.0, format(mono));
                    }
                    mono = next;
                    start = i;
                }
            }
        });
    }
    job.append(&text[start..], 0.0, format(mono));
    let mut galley = painter.layout_job(job);
    let baseline = galley.rows.first().map_or(0.0, |row| {
        row.pos.y + row.glyphs.first().map_or(row.height(), |g| g.pos.y)
    });
    if transform.translation_only() {
        return (
            pos2(transform.origin[0], transform.origin[1] - baseline),
            galley,
        );
    }
    // Keep UVs in atlas texels. epaint normalizes them against the current
    // atlas during tessellation, just like an ordinary shaped text run.
    let galley_data = Arc::make_mut(&mut galley);
    galley_data.mesh_bounds = Rect::NOTHING;
    for row in &mut galley_data.rows {
        let offset = row.pos.to_vec2() - vec2(0.0, baseline);
        row.pos = Pos2::ZERO;
        let visuals = &mut Arc::make_mut(&mut row.row).visuals;
        visuals.mesh_bounds = Rect::NOTHING;
        for vertex in &mut visuals.mesh.vertices {
            let at = vertex.pos + offset;
            let [x, y] = transform.vector([at.x, at.y]);
            vertex.pos = pos2(x, y);
            visuals.mesh_bounds.extend_with(vertex.pos);
        }
        galley_data.mesh_bounds = galley_data.mesh_bounds.union(visuals.mesh_bounds);
    }
    (Pos2::from(transform.origin), galley)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_separates_point_size_style_and_face() {
        let ctx = eframe::egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let painter = ui.painter();
            let cache = FormulaCache::new(painter);
            let source = r"\frac{x}{2}";
            let face = Face::default();
            let first = cache.get(painter, source, 16.0, Style::Display, face);
            let same = cache.get(painter, source, 16.0, Style::Display, face);
            let style = cache.get(painter, source, 16.0, Style::Text, face);
            let size = cache.get(painter, source, 32.0, Style::Display, face);
            let bold = cache.get(
                painter,
                source,
                16.0,
                Style::Display,
                Face { bold: true, ..face },
            );
            assert!(Arc::ptr_eq(&first, &same));
            assert!(first.size.y > style.size.y);
            assert!(size.size.y > first.size.y);
            assert!(!Arc::ptr_eq(&first, &bold));
        });
        output.textures_delta.clear();
    }

    /// Reference rendered at 1 point per pixel on the Apple Silicon Metal backend.
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn formula_pixels_match_the_metal_reference() {
        use egui_kittest::Harness;

        let mut harness = Harness::builder()
            .with_size(vec2(460.0, 210.0))
            .with_pixels_per_point(1.0)
            .wgpu()
            .build_ui(|ui| {
                let painter = ui.painter();
                painter.rect_filled(painter.clip_rect(), 0.0, Color32::WHITE);
                let cache = FormulaCache::new(painter);
                for (source, x, y) in [
                    (r"x_i^2 = \frac{1+\sqrt[3]{\alpha}}{2}\cdot\pi", 20.0, 12.0),
                    (r"\begin{pmatrix}a&b\\c&d\\\frac{1}{2}& x^2\end{pmatrix}", 268.0, 12.0),
                    (r"\sum_{i=0}^n \mathbf{x}_i+\int\limits_0^1 \overrightarrow{AB}\,dx", 20.0, 78.0),
                    (r"\begin{cases}x^2&x\ge 0\\0&\text{otherwise}\end{cases}", 268.0, 112.0),
                    (r"\textcolor{#2455af}{\overline{a}} + \bgcolor{#f5e8df}{\text{units}} + \unknown{ab}", 20.0, 160.0),
                ] {
                    cache.get(painter, source, 20.0, Style::Display, Face::default())
                        .paint(painter, pos2(x, y), Color32::from_rgb(28, 28, 31));
                }
            });
        harness.run();
        let actual = harness.render().expect("Metal formula rendering");
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/testdata/latex-metal.png");
        if std::env::var_os("UPDATE_LATEX_SNAPSHOT").is_some() {
            actual.save(&path).unwrap();
        }
        let expected = image::open(&path).unwrap().to_rgba8();
        assert_eq!(actual.dimensions(), expected.dimensions());
        let changed = actual
            .pixels()
            .zip(expected.pixels())
            .filter(|(a, b)| a.0.iter().zip(b.0).any(|(a, b)| a.abs_diff(b) > 12))
            .count();
        assert!(
            changed <= 60,
            "{changed} pixels differ by more than 12 levels"
        );
    }
}
