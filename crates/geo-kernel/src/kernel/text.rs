// The pixel alignment rules below are adapted from GeoGebra, commit
// 112919f2d450ec7430a5cd3b65dba226fdc43b74, source/shared/common/src/main/java/
// org/geogebra/common/euclidian/draw/AlignDrawText.java and DrawText.java.
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

impl Construction {
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

    pub(crate) fn text_latex_override(&self, id: Id) -> Option<bool> {
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
        let current = self.text_presentations.entry(id).or_default();
        let template_changed = props.latex.is_some() && props.latex != current.latex;
        current.position = props.position.or(current.position);
        current.latex = props.latex.or(current.latex);
        current.font = props.font.or(current.font);
        current.offset = props.offset.or(current.offset);
        if template_changed {
            if let super::Def::Expr(def) = self.def(id) {
                if self.update_expr(id.0, def) {
                    self.propagate(id);
                }
            }
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
