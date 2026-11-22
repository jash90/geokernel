// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/AlignDrawText.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawText.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Canvas text layout is independent of the view and reused across pans.

use super::LABEL_FONT;
use crate::interact::ViewTransform;
use eframe::egui::epaint::text::{Galley, LayoutJob, TextFormat};
use eframe::egui::{vec2, Color32, FontId, Painter, Pos2, Rect};
use geo_kernel::kernel::text::{TextPosition, TextPresentation};
use geo_kernel::{Construction, Id, Value};
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

    fn at(&self, mut anchor: Pos2, style: TextPresentation) -> Pos2 {
        if !style.latex {
            anchor.y -= self.baseline;
        }
        if style.horizontal.is_some() || style.vertical.is_some() {
            let size = self.galley.size();
            anchor.x += match style.horizontal {
                Some(-1) => 1.0 - size.x,
                Some(0) => 3.0 - (size.x / 2.0).floor(),
                _ => 6.0,
            };
            let remaining = size.y - self.first_height;
            anchor.y += match (style.latex, style.vertical) {
                (true, Some(-1)) => 6.0,
                (true, Some(0)) => 7.0 - size.y / 2.0,
                (true, _) => -size.y,
                (false, Some(-1)) => self.first_height + 6.0,
                (false, Some(0)) => remaining / 2.0,
                (false, _) => -remaining,
            };
        }
        anchor
    }
}

fn position(c: &Construction, style: TextPresentation, t: &ViewTransform) -> Option<(f64, f64)> {
    let world = |x, y| {
        let (sx, sy) = t.world_to_screen(x, y);
        (
            sx + f64::from(style.offset[0]),
            sy + f64::from(style.offset[1]),
        )
    };
    Some(match style.position {
        TextPosition::Origin => world(0.0, 0.0),
        TextPosition::World { x, y } => world(x, y),
        TextPosition::Screen { x, y } => (x, y),
        TextPosition::Point(point) | TextPosition::ScreenPoint(point) => {
            let Value::Point { x, y } = c.value(point) else {
                return None;
            };
            if matches!(style.position, TextPosition::ScreenPoint(_)) {
                (f64::from(x as i32), f64::from(y as i32))
            } else {
                world(x, y)
            }
        }
        TextPosition::Undefined => return None,
    })
}

pub(super) fn paint(
    painter: &Painter,
    rect: Rect,
    c: &Construction,
    t: &ViewTransform,
    texts: impl Iterator<Item = Id>,
    color: impl Fn(Id) -> Color32,
    generation: u64,
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
    if cache.generation != generation
        || cache
            .font_marker
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, &marker))
    {
        cache.generation = generation;
        cache.font_marker = Some(marker);
        cache.entries.clear();
    }
    for id in texts {
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
        let anchor = rect.min + vec2(x as f32, y as f32);
        // An unaligned or right-aligned text extends right from its anchor.
        // A left-aligned text ends one pixel to the right of its anchor.
        if !anchor.is_finite()
            || match style.horizontal {
                Some(-1) => anchor.x + 1.0 < rect.left(),
                Some(0) => false,
                _ => anchor.x > rect.right(),
            }
        {
            continue;
        }
        let size = (f64::from(LABEL_FONT) * style.size)
            .floor()
            .clamp(4.0, 256.0) as f32;
        let entry = cache
            .entries
            .entry(id)
            .or_insert_with(|| Layout::new(painter, text, size, style.italic));
        if entry.size != size || entry.italic != style.italic || entry.galley.text() != text {
            *entry = Layout::new(painter, text, size, style.italic);
        }
        let at = entry.at(anchor, style);
        if rect.intersects(Rect::from_min_size(at, entry.galley.size())) {
            painter.galley(at, entry.galley.clone(), color(id));
        }
    }
}
