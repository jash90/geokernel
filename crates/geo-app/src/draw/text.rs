// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/AlignDrawText.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawText.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Canvas text layout is independent of the view and reused across pans.

use super::{Scene, LABEL_FONT};
use crate::geom::ToCore;
use crate::interact::ViewTransform;
use crate::latex::{self, FormulaCache};
use eframe::egui::epaint::text::{Galley, LayoutJob, TextFormat};
use eframe::egui::{Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind};
use geo_kernel::kernel::text::TextPresentation;
use geo_kernel::{Construction, Id};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Cache {
    font_marker: Option<Arc<Galley>>,
    generation: u64,
    entries: HashMap<Id, Layout>,
}

struct Layout {
    galley: Arc<Galley>,
    size: f32,
    italic: bool,
    baseline: f32,
    first_height: f32,
}

#[cfg(test)]
thread_local! {
    static BUILT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(super) fn layouts_built() -> u64 {
    BUILT.with(std::cell::Cell::get)
}

impl Layout {
    fn new(painter: &Painter, text: &str, size: f32, italic: bool) -> Self {
        #[cfg(test)]
        BUILT.with(|n| n.set(n.get() + 1));
        let mut format = TextFormat::simple(FontId::proportional(size), Color32::PLACEHOLDER);
        format.italics = italic;
        let galley = painter.layout_job(LayoutJob::single_section(text.to_owned(), format));
        let (baseline, first_height) = galley.rows.first().map_or((0.0, 0.0), |row| {
            let baseline = row.pos.y + row.glyphs.first().map_or(row.height(), |g| g.pos.y);
            (baseline, row.height())
        });
        Self {
            galley,
            size,
            italic,
            baseline,
            first_height,
        }
    }

    fn at(&self, anchor: Pos2, style: TextPresentation) -> Pos2 {
        Pos2::from(style.aligned_top_left(
            [anchor.x, anchor.y],
            self.galley.size().into(),
            self.first_height,
            self.baseline,
        ))
    }
}

fn position(c: &Construction, style: TextPresentation, t: &ViewTransform) -> Option<(f64, f64)> {
    style.pixel_anchor(c, |x, y| t.world_to_screen(x, y))
}

pub(super) fn paint(
    painter: &Painter,
    rect: Rect,
    c: &Construction,
    t: &ViewTransform,
    texts: impl Iterator<Item = Id>,
    color: impl Fn(Id) -> Color32,
    scene: &Scene<'_>,
) {
    let cache = painter.ctx().data_mut(|data| {
        let id = eframe::egui::Id::unique("geo_text_layouts");
        data.get_temp_mut_or_default::<Arc<Mutex<Cache>>>(id)
            .clone()
    });
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    // The font cache memoizes this request. A new galley means that the
    // pixel scale or font cache changed, so old atlas coordinates may be stale.
    let marker = painter.layout_no_wrap(
        String::new(),
        FontId::proportional(LABEL_FONT),
        Color32::PLACEHOLDER,
    );
    if cache.generation != scene.generation
        || cache
            .font_marker
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, &marker))
    {
        cache.generation = scene.generation;
        cache.font_marker = Some(marker);
        cache.entries.clear();
    }
    let mut text_rects = scene.text_rects.map(|r| r.borrow_mut());
    let mut formulas = None;
    for id in texts {
        if c.kind(id) == geo_kernel::Kind::InputBox {
            let caption = c.input_box_caption(id);
            let [x, y] = c.input_box_style(id).position.map(|v| v as f32);
            if caption.is_empty() || x > rect.width() || y > rect.height() || y < -30.0 {
                continue;
            }
            let entry = cache
                .entries
                .entry(id)
                .or_insert_with(|| Layout::new(painter, caption, LABEL_FONT, false));
            if entry.galley.text() != caption {
                *entry = Layout::new(painter, caption, LABEL_FONT, false);
            }
            let at = rect.min + eframe::egui::vec2(x, y + 15.0 - entry.galley.size().y / 2.0);
            painter.galley(at, entry.galley.clone(), color(id));
            continue;
        }
        let Some((text, style)) = c
            .text(id)
            .filter(|text| !text.is_empty())
            .zip(c.text_presentation(id))
        else {
            continue;
        };
        let Some((x, y)) = position(c, style, t) else {
            continue;
        };
        let anchor = Pos2::new(x as f32, y as f32);
        // An unaligned or right-aligned text extends right from its anchor.
        // A left-aligned text ends one pixel to the right of its anchor.
        if !anchor.is_finite()
            || match style.horizontal {
                Some(-1) => anchor.x + 1.0 < 0.0,
                Some(0) => false,
                _ => anchor.x > rect.width(),
            }
        {
            continue;
        }
        let size = style.drawing_size(LABEL_FONT);
        let (at, size, plain, formula) = if style.latex {
            let cache = formulas.get_or_insert_with(|| FormulaCache::new(painter));
            let formula = cache.get(
                painter,
                text,
                size,
                geo_latex::Style::Display,
                latex::face(style),
            );
            let at = Pos2::from(style.aligned_top_left(
                [anchor.x, anchor.y],
                formula.size.into(),
                0.0,
                0.0,
            ));
            (at, formula.size, None, Some(formula))
        } else {
            let entry = cache
                .entries
                .entry(id)
                .or_insert_with(|| Layout::new(painter, text, size, style.italic));
            if entry.size != size || entry.italic != style.italic || entry.galley.text() != text {
                *entry = Layout::new(painter, text, size, style.italic);
            }
            (
                entry.at(anchor, style),
                entry.galley.size(),
                Some(&entry.galley),
                None,
            )
        };
        let at = rect.min + at.to_vec2();
        let bounds = Rect::from_min_size(at, size);
        if rect.intersects(bounds) {
            if let Some(text_rects) = &mut text_rects {
                text_rects.push((id, bounds.translate(-rect.min.to_vec2()).core()));
            }
            let color = color(id);
            if let Some([r, g, b, a]) = c.appearance(id).text_background.filter(|rgba| rgba[3] != 0)
            {
                painter.rect_filled(bounds, 0.0, Color32::from_rgba_unmultiplied(r, g, b, a));
            }
            if let Some(formula) = formula {
                formula.paint(painter, at, color);
            } else if let Some(galley) = plain {
                painter.galley(at, galley.clone(), color);
                if style.bold {
                    painter.galley(at + eframe::egui::vec2(0.7, 0.0), galley.clone(), color);
                }
            }
            let highlight = if scene.selected == Some(id) {
                0.75
            } else if scene.hovered == Some(id) {
                0.35
            } else {
                continue;
            };
            painter.rect_stroke(
                bounds.expand(2.0),
                3.0,
                Stroke::new(1.5, color.gamma_multiply(highlight)),
                StrokeKind::Outside,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_kernel::kernel::text::TextPosition;
    use geo_kernel::svg::View;

    #[test]
    fn text_canvas_projects_to_integer_pixels_before_offsets() {
        // The fractional origin and the point from the Java text-canvas case.
        let t = ViewTransform::new(
            View {
                xmin: -200.75 / 50.0,
                xmax: (800.0 - 200.75) / 50.0,
                ymin: (180.75 - 600.0) / 50.0,
                ymax: 180.75 / 50.0,
            },
            800.0,
            600.0,
        );
        let c = geo_kernel::script::load("A = (1.016, 0.984)\n").unwrap();
        let mut style = TextPresentation::default();
        assert_eq!(position(&c, style, &t), Some((200.0, 180.0)));
        style.position = TextPosition::Point(c.id("A").unwrap());
        assert_eq!(position(&c, style, &t), Some((252.0, 132.0)));
        style.position = TextPosition::World { x: 1.016, y: 0.984 };
        style.offset = [2, -3];
        assert_eq!(position(&c, style, &t), Some((254.0, 129.0)));
        style.position = TextPosition::Screen {
            x: 200.75,
            y: 180.75,
        };
        assert_eq!(position(&c, style, &t), Some((200.0, 180.0)));
    }
}
