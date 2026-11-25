// The pixel alignment rules below are adapted from GeoGebra, commit
// 112919f2d450ec7430a5cd3b65dba226fdc43b74, source/shared/common/src/main/java/
// org/geogebra/common/euclidian/draw/AlignDrawText.java and DrawText.java,
// org/geogebra/common/euclidian/EuclidianStatic.java, and
// org/geogebra/common/util/StringUtil.java.
// Copyright (c) GeoGebra GmbH. Licensed under the EUPL 1.2.

//! The presentation of a text, alongside its separately stored string.

use super::{Construction, Id, Kind, Value};

/// A pixel position relative to the canvas or an anchor in the world.
/// The generic parameter lets script directives refer to a point by label.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextPosition<Point = Id> {
    Origin,
    World {
        x: f64,
        y: f64,
    },
    Point(Point),
    Screen {
        x: f64,
        y: f64,
    },
    /// A point whose coordinates are interpreted as pixels in legacy files.
    ScreenPoint(Point),
    Undefined,
}

impl<P> TextPosition<P> {
    fn point(self) -> Option<P> {
        match self {
            Self::Point(point) | Self::ScreenPoint(point) => Some(point),
            _ => None,
        }
    }

    pub(crate) fn try_map_point<Q, E>(
        self,
        map: impl FnOnce(P) -> Result<Q, E>,
    ) -> Result<TextPosition<Q>, E> {
        Ok(match self {
            Self::Origin => TextPosition::Origin,
            Self::World { x, y } => TextPosition::World { x, y },
            Self::Point(point) => TextPosition::Point(map(point)?),
            Self::Screen { x, y } => TextPosition::Screen { x, y },
            Self::ScreenPoint(point) => TextPosition::ScreenPoint(map(point)?),
            Self::Undefined => TextPosition::Undefined,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextFont {
    pub size: f64,
    pub serif: bool,
    pub bold: bool,
    pub italic: bool,
}

impl Default for TextFont {
    fn default() -> Self {
        Self {
            size: 1.0,
            serif: false,
            bold: false,
            italic: false,
        }
    }
}

/// Properties of a text, including the layout supplied by its command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextPresentation {
    pub position: TextPosition,
    /// A command supplies and controls the anchor, so the text cannot be dragged.
    pub fixed: bool,
    pub latex: bool,
    pub size: f64,
    pub serif: bool,
    pub bold: bool,
    pub italic: bool,
    pub offset: [i32; 2],
    pub horizontal: Option<i32>,
    pub vertical: Option<i32>,
}

impl Default for TextPresentation {
    fn default() -> Self {
        Self {
            position: TextPosition::Origin,
            fixed: false,
            latex: false,
            size: 1.0,
            serif: false,
            bold: false,
            italic: false,
            offset: [0, 0],
            horizontal: None,
            vertical: None,
        }
    }
}

impl TextPresentation {
    /// Saved font multiplier converted to drawing points.
    pub fn drawing_size(self, base: f32) -> f32 {
        (f64::from(base) * self.size).floor().clamp(4.0, 256.0) as f32
    }

    /// Placement before alignment, using the caller's world-to-pixel map.
    pub fn pixel_anchor(
        self,
        c: &Construction,
        project: impl Fn(f64, f64) -> (f64, f64),
    ) -> Option<(f64, f64)> {
        let offset = |x, y| (x + f64::from(self.offset[0]), y + f64::from(self.offset[1]));
        let world = |x, y| {
            let (sx, sy) = project(x, y);
            // Screen projection rounds halfway values up before applying offsets.
            offset((sx + 0.5).floor(), (sy + 0.5).floor())
        };
        let (x, y) = match self.position {
            TextPosition::Origin => {
                let (x, y) = project(0.0, 0.0);
                offset(x.trunc(), y.trunc())
            }
            TextPosition::World { x, y } => world(x, y),
            TextPosition::Screen { x, y } => (x.trunc(), y.trunc()),
            TextPosition::Point(point) | TextPosition::ScreenPoint(point) => {
                let Value::Point { x, y } = c.value(point) else {
                    return None;
                };
                if matches!(self.position, TextPosition::ScreenPoint(_)) {
                    (x.trunc(), y.trunc())
                } else {
                    world(x, y)
                }
            }
            TextPosition::Undefined => return None,
        };
        (x.is_finite() && y.is_finite()).then_some((x, y))
    }

    /// Top left of a formula or plain text block at its saved anchor.
    pub fn aligned_top_left(
        self,
        [mut x, mut y]: [f32; 2],
        [width, height]: [f32; 2],
        first_height: f32,
        baseline: f32,
    ) -> [f32; 2] {
        if self.horizontal.is_some() || self.vertical.is_some() {
            x += match self.horizontal {
                Some(-1) => 1.0 - width,
                Some(0) => 3.0 - (width / 2.0).floor(),
                _ => 6.0,
            };
            let remaining = height - first_height;
            y += match (self.latex, self.vertical) {
                (true, Some(-1)) => 6.0,
                (true, Some(0)) => 7.0 - height / 2.0,
                (true, _) => -height,
                (false, Some(-1)) => first_height + 6.0,
                (false, Some(0)) => remaining / 2.0,
                (false, _) => -remaining,
            };
            x = x.trunc();
            y = y.trunc();
        }
        if !self.latex {
            y -= baseline;
        }
        [x, y]
    }
}

/// Properties assigned after creation, for example by a saved construction.
/// Unassigned properties continue to follow their definition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextOverrides<P = Id> {
    pub position: Option<TextPosition<P>>,
    pub latex: Option<bool>,
    pub font: Option<TextFont>,
    pub offset: Option<[i32; 2]>,
}

impl<P> Default for TextOverrides<P> {
    fn default() -> Self {
        Self {
            position: None,
            latex: None,
            font: None,
            offset: None,
        }
    }
}

/// The headless drawable uses GFontCommon and TextLayoutCommon, with
/// StringUtil.estimateLength for string advances and DrawEquationStub for
/// formulas. The canvas measures its glyphs separately for painting.
fn headless_box(text: &str, size: f64, latex: bool) -> (f64, f64) {
    if !latex {
        let mut lines = text.split_inclusive('\n').peekable();
        let mut width = 0.0_f64;
        let mut count = 0;
        while let Some(line) = lines.next() {
            let line = if lines.peek().is_some() {
                line.strip_suffix('\n').unwrap_or(line)
            } else {
                line
            };
            count += 1;
            width = width.max(indexed_advance(line, size));
        }
        return (width, (1.5 * size * f64::from(count.max(1))).trunc());
    }

    // EuclidianStatic.blockSplit splits the reversed string, dropping
    // empty fields at its end, then inserts one empty leading text block.
    let text = if text.starts_with('$') {
        &text[text.len() - text.trim_start_matches('$').len() - 1..]
    } else {
        text
    };
    let mut previous = None;
    let mut blocks = text.split(|ch| {
        let delimiter = ch == '$' && previous != Some('\\');
        previous = Some(ch);
        delimiter
    });
    let first = blocks.next().unwrap_or_default();
    let mut blocks = blocks.peekable();
    let (mut width, mut line_width, mut lines) = (0.0_f64, 0.0, 1);
    if blocks.peek().is_some() {
        // The blocks alternate between text and zero-width formulas.
        for block in std::iter::once(first).chain(blocks.skip(1).step_by(2)) {
            for (i, line) in block.split('\n').enumerate() {
                if i != 0 {
                    lines += 1;
                    width = width.max(line_width);
                    line_width = 0.0;
                }
                line_width += indexed_advance(&line.replace("\\$", "$"), size);
            }
        }
    }
    (
        width.max(line_width),
        (size + (size * 0.5).trunc()) * f64::from(lines),
    )
}

fn indexed_advance(text: &str, size: f64) -> f64 {
    let mut text = text.encode_utf16();
    let index_size = (size * 0.9).trunc().max(8.0);
    let (mut width, mut depth) = (0.0, 0_usize);
    while let Some(unit) = text.next() {
        match unit {
            95 => match text.next() {
                Some(123) => depth += 1,
                // Even a single index character goes through
                // StringUtil.estimateLength, where '_' has negative advance
                // and '}' has none.
                Some(95) => width += (0.7_f64 - 1.0) * 0.5 * index_size,
                Some(125) | None => {}
                Some(_) => width += index_size * 0.5,
            },
            125 => depth = depth.saturating_sub(1),
            _ => width += if depth == 0 { size } else { index_size } * 0.5,
        }
    }
    width.round()
}

#[cfg(test)]
#[test]
fn text_pixel_bounds_measure_only_the_bounded_unicode_prefix() {
    let prefix = "x".repeat(4095);
    let c = crate::script::load(&format!(
        "head = \"{prefix}\"\nfull = \"{prefix}é{}\"",
        "x".repeat(4096)
    ))
    .unwrap();
    let bounds = |label| {
        c.text_pixel_rect(c.id(label).unwrap(), |x, y| (x, y))
            .unwrap()
    };
    assert_eq!(bounds("head"), bounds("full"));
}

impl Construction {
    /// Painted text bounds in pixels for callers without a text layout cache.
    pub fn text_pixel_rect(
        &self,
        id: Id,
        to_pixels: impl Fn(f64, f64) -> (f64, f64),
    ) -> Option<[f32; 4]> {
        let source = self.text(id).filter(|text| !text.is_empty())?;
        let source = &source[..source.floor_char_boundary(source.len().min(4096))];
        let style = self.text_presentation(id)?;
        let (x, y) = style.pixel_anchor(self, to_pixels)?;
        let size = style.drawing_size(16.0);
        let face = geo_latex::Face {
            bold: style.bold,
            italic: style.italic,
            monospace: false,
        };
        let (extent, baseline) = if style.latex {
            (
                geo_latex::layout(source, size, geo_latex::Style::Display, face).size(),
                0.0,
            )
        } else {
            let lines = geo_latex::plain_lines(source, size, face);
            let width = lines
                .iter()
                .map(|(width, ..)| *width)
                .fold(0.0_f32, f32::max);
            ([width, size * 1.2 * lines.len() as f32], size * 1.2)
        };
        let [x, y] = style.aligned_top_left([x as f32, y as f32], extent, baseline, 0.0);
        let [width, height] = extent;
        (width > 0.0 && height > 0.0).then_some([x, y, x + width, y + height])
    }

    /// How to display a text. An anchor point is read from the live construction.
    pub fn text_presentation(&self, id: Id) -> Option<TextPresentation> {
        if self.defs.get(id.0)?.kind() != Kind::Text {
            return None;
        }
        let mut style = crate::expr::text_presentation(self, id).unwrap_or_default();
        if let Some(saved) = self.text_presentations.get(&id) {
            style.position = saved.position.unwrap_or(style.position);
            style.latex = saved.latex.unwrap_or(style.latex);
            if let Some(font) = saved.font {
                style.size = font.size;
                style.serif = font.serif;
                style.bold = font.bold;
                style.italic = font.italic;
            }
            style.offset = saved.offset.unwrap_or(style.offset);
        }
        Some(style)
    }

    pub fn text_position(&self, id: Id) -> Option<TextPosition> {
        Some(self.text_presentation(id)?.position)
    }

    /// The world-coordinate corners of the headless text drawable, from
    /// bottom left counter-clockwise.
    pub(crate) fn text_corner(&self, id: Id, index: i32) -> Option<(f64, f64)> {
        const HEADLESS_FONT_SIZE: f64 = 12.0;
        const MARGIN: f64 = 3.0;
        let style = self.text_presentation(id)?;
        if matches!(
            style.position,
            TextPosition::Screen { .. } | TextPosition::ScreenPoint(_)
        ) {
            return None;
        }
        let text = self.text(id)?;
        if !crate::expr::IntersectionWork::new().charge(text.len() as u64) {
            return None;
        }
        let [xmin, _, _, ymax, xs, ys] = self.view();
        let (x_zero, y_zero) = (-xmin * xs, ymax * ys);
        let (x, y) = style.pixel_anchor(self, |x, y| (x_zero + x * xs, y_zero - y * ys))?;
        let size = (HEADLESS_FONT_SIZE * style.size).max(4.0).trunc();
        let (width, height) = headless_box(text, size, style.latex);
        let width = width + 2.0 * MARGIN;
        let height = height + 2.0 * MARGIN;
        let [x, y] = style
            .aligned_top_left(
                [x as f32, y as f32],
                [width as f32, height as f32],
                size as f32,
                0.0,
            )
            .map(f64::from);
        let left = (x - MARGIN - x_zero) / xs;
        let top = (y_zero - y + MARGIN + if style.latex { 0.0 } else { size }) / ys;
        let right = left + width / xs;
        let bottom = top - height / ys;
        let point = match index {
            1 => (left, bottom),
            2 => (right, bottom),
            3 => (right, top),
            4 => (left, top),
            _ => return None,
        };
        (point.0.is_finite() && point.1.is_finite()).then_some(point)
    }

    pub(super) fn saved_text_anchor(&self, id: Id) -> Option<Id> {
        self.text_presentations.get(&id)?.position?.point()
    }

    /// The saved or explicitly edited mode, before the expression default.
    pub fn text_latex_override(&self, id: Id) -> Option<bool> {
        self.text_presentations.get(&id)?.latex
    }

    pub(crate) fn apply_text_properties(
        &mut self,
        id: Id,
        props: TextOverrides,
    ) -> Result<(), String> {
        if self
            .defs
            .get(id.0)
            .is_none_or(|def| def.kind() != Kind::Text)
        {
            return Err("text properties need a text object".into());
        }
        if let Some(TextPosition::Point(point) | TextPosition::ScreenPoint(point)) = props.position
        {
            if self
                .defs
                .get(point.0)
                .is_none_or(|def| def.kind() != Kind::Point)
            {
                return Err("a text anchor must be a point".into());
            }
        }
        if let Some(position) = props.position {
            let old_anchor = self.saved_text_anchor(id);
            let new_anchor = position.point();
            if new_anchor != old_anchor {
                let mut parents = self.read_ids(&self.def(id));
                let remove = old_anchor.filter(|old| !parents.contains(old));
                let add = new_anchor.filter(|new| !parents.contains(new));
                if let Some(anchor) = add {
                    parents.push(anchor);
                    if self.dependency_needs_reorder(anchor, id) {
                        let changed = std::collections::BTreeMap::from([(id, parents)]);
                        let loci = self.locus_order_inputs()?;
                        let order = self.replacement_order(&changed, &loci, self.len())?;
                        self.install_replacement_order(order);
                    }
                    self.children[anchor.0].insert(id);
                }
                if let Some(anchor) = remove {
                    self.children[anchor.0].remove(id);
                }
            }
        }
        let current = self.text_presentations.entry(id).or_default();
        let template_changed = props.latex.is_some() && props.latex != current.latex;
        current.position = props.position.or(current.position);
        current.latex = props.latex.or(current.latex);
        current.font = props.font.or(current.font);
        current.offset = props.offset.or(current.offset);
        let value_changed = template_changed
            && matches!(self.def(id), super::Def::Expr(def) if self.update_expr(id.0, def));
        if value_changed || self.has_text_corner(id) {
            self.propagate(id);
        }
        Ok(())
    }

    pub(super) fn discard_text_presentations(&mut self, len: usize) {
        self.text_presentations.retain(|id, props| {
            if id.0 >= len {
                return false;
            }
            if let Some(TextPosition::Point(point) | TextPosition::ScreenPoint(point)) =
                props.position
            {
                if point.0 >= len {
                    props.position = Some(match self.values[point.0] {
                        Value::Point { x, y }
                            if matches!(props.position, Some(TextPosition::ScreenPoint(_))) =>
                        {
                            TextPosition::Screen {
                                x: f64::from(x as i32),
                                y: f64::from(y as i32),
                            }
                        }
                        Value::Point { x, y } => TextPosition::World { x, y },
                        _ => TextPosition::Undefined,
                    });
                }
            }
            true
        });
    }
}
