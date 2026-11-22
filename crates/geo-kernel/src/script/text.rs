//! A saved text's placement and style, attached after its value definition.

use crate::kernel::text::{TextFont, TextOverrides, TextPosition};
use crate::Construction;

pub const DIRECTIVE_NAME: &str = "$textProperties";

/// An object label and the presentation properties assigned to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Properties<'a> {
    pub label: &'a str,
    pub overrides: TextOverrides<&'a str>,
}

impl Properties<'_> {
    /// Applies to an existing text. Named anchors are resolved at replay time.
    pub fn apply(self, c: &mut Construction) -> Result<(), String> {
        let id = super::object(c, self.label)?;
        let o = self.overrides;
        let position = o
            .position
            .map(|p| p.try_map_point(|label| super::object(c, label)))
            .transpose()?;
        c.apply_text_properties(
            id,
            TextOverrides {
                position,
                latex: o.latex,
                font: o.font,
                offset: o.offset,
            },
        )
    }
}

impl std::fmt::Display for Properties<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let o = self.overrides;
        write!(f, "{DIRECTIVE_NAME}({}, ", self.label)?;
        match o.latex {
            Some(value) => write!(f, "{value}, ")?,
            None => write!(f, "-, ")?,
        }
        match o.font {
            Some(TextFont {
                size,
                serif,
                bold,
                italic,
            }) => {
                let flags = i32::from(serif) | (i32::from(bold) << 1) | (i32::from(italic) << 2);
                write!(f, "{size}, {flags}, ")?;
            }
            None => write!(f, "-, -, ")?,
        }
        match o.offset {
            Some([x, y]) => write!(f, "{x}, {y}, ")?,
            None => write!(f, "-, -, ")?,
        }
        match o.position {
            None => write!(f, "default)"),
            Some(TextPosition::Origin) => write!(f, "origin)"),
            Some(TextPosition::Undefined) => write!(f, "undefined)"),
            Some(TextPosition::Point(label)) => write!(f, "point, {label})"),
            Some(TextPosition::ScreenPoint(label)) => write!(f, "screenPoint, {label})"),
            Some(TextPosition::World { x, y }) => write!(f, "world, {x}, {y})"),
            Some(TextPosition::Screen { x, y }) => write!(f, "screen, {x}, {y})"),
        }
    }
}

fn finite(text: &str) -> Option<f64> {
    text.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn parse(line: &str) -> Option<Properties<'_>> {
    let mut args = line.strip_suffix(')')?.split(',').map(str::trim);
    let label = args.next().filter(|label| !label.is_empty())?;
    let latex = match args.next()? {
        "-" => None,
        s => Some(s.parse().ok()?),
    };
    let font = match (args.next()?, args.next()?) {
        ("-", "-") => None,
        (size, flags) => {
            let size = finite(size).filter(|v| *v > 0.0)?;
            let flags = flags.parse::<u8>().ok().filter(|flags| *flags < 8)?;
            Some(TextFont {
                size,
                serif: flags & 1 != 0,
                bold: flags & 2 != 0,
                italic: flags & 4 != 0,
            })
        }
    };
    let offset = match (args.next()?, args.next()?) {
        ("-", "-") => None,
        (x, y) => Some([x.parse().ok()?, y.parse().ok()?]),
    };
    let position = match args.next()? {
        "default" => None,
        "origin" => Some(TextPosition::Origin),
        "undefined" => Some(TextPosition::Undefined),
        "point" => Some(TextPosition::Point(args.next()?)),
        "screenPoint" => Some(TextPosition::ScreenPoint(args.next()?)),
        mode @ ("world" | "screen") => {
            let x = finite(args.next()?)?;
            let y = finite(args.next()?)?;
            Some(if mode == "world" {
                TextPosition::World { x, y }
            } else {
                TextPosition::Screen { x, y }
            })
        }
        _ => return None,
    };
    if args.next().is_some() {
        return None;
    }
    Some(Properties {
        label,
        overrides: TextOverrides {
            position,
            latex,
            font,
            offset,
        },
    })
}

/// Reads one presentation directive, or reports that the line is a definition.
pub fn parse_properties(line: &str) -> Option<Result<Properties<'_>, String>> {
    let args = line.strip_prefix(DIRECTIVE_NAME)?.strip_prefix('(')?;
    Some(parse(args).ok_or_else(|| format!("invalid text properties: `{line}`")))
}

pub(super) fn apply(c: &mut Construction, line: &str) -> Option<Result<(), String>> {
    Some(parse_properties(line)?.and_then(|props| props.apply(c)))
}
