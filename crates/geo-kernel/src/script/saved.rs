//! Journal properties that are restored after the named objects exist.

use super::{object, split_call};
use crate::kernel::control::{
    decode_scripts, decode_text, encode_scripts, encode_text, ControlScript,
};
use crate::Construction;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Property {
    SliderBounds {
        label: String,
        bounds: [Option<String>; 2],
    },
    ObjectScripts {
        label: String,
        scripts: Vec<ControlScript>,
    },
}

impl Property {
    pub fn label(&self) -> &str {
        match self {
            Self::SliderBounds { label, .. } | Self::ObjectScripts { label, .. } => label,
        }
    }

    pub fn apply(&self, c: &mut Construction) -> Result<(), String> {
        match self {
            Self::SliderBounds { label, bounds } => {
                let id = object(c, label)?;
                let [min, max] = bounds;
                c.restore_slider_bounds(id, [min.as_deref(), max.as_deref()])
            }
            Self::ObjectScripts { label, scripts } => {
                c.object_scripts.insert(label.clone(), scripts.clone());
                Ok(())
            }
        }
    }

    /// Keep each endpoint that can be restored when the other fails to load
    /// or used an object that has since been deleted.
    pub fn restore(self, c: &mut Construction) -> (Option<Self>, Vec<String>) {
        match self.apply(c) {
            Ok(()) => (Some(self), Vec::new()),
            Err(error) => {
                let Self::SliderBounds {
                    label,
                    bounds: [Some(min), Some(max)],
                } = self
                else {
                    return (None, vec![error]);
                };
                let mut restored = None;
                let mut errors = Vec::new();
                for (endpoint, bounds) in [
                    ("minimum", [Some(min), None]),
                    ("maximum", [None, Some(max)]),
                ] {
                    let side = Self::SliderBounds {
                        label: label.clone(),
                        bounds,
                    };
                    match side.apply(c) {
                        Ok(()) => restored = Some(side),
                        Err(error) => errors.push(format!("{endpoint}: {error}")),
                    }
                }
                (restored, errors)
            }
        }
    }
}

impl fmt::Display for Property {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SliderBounds {
                label,
                bounds: [min, max],
            } => {
                let field = |source: &Option<String>| {
                    source.as_deref().map_or_else(|| "_".into(), encode_text)
                };
                write!(
                    f,
                    "$sliderBounds({}, {}, {})",
                    encode_text(label),
                    field(min),
                    field(max)
                )
            }
            Self::ObjectScripts { label, scripts } => {
                write!(f, "$objectScripts({}", encode_text(label))?;
                for field in encode_scripts(scripts) {
                    write!(f, ", {field}")?;
                }
                f.write_str(")")
            }
        }
    }
}

pub fn parse(line: &str) -> Option<Result<Property, String>> {
    if !line.starts_with("$sliderBounds(") && !line.starts_with("$objectScripts(") {
        return None;
    }
    Some((|| {
        let (form, args) = split_call(line).ok_or("invalid saved property")?;
        match (form, args.as_slice()) {
            ("$sliderBounds", [label, min, max]) => {
                let bound = |field| (field != "_").then(|| decode_text(field)).transpose();
                Ok(Property::SliderBounds {
                    label: decode_text(label)?,
                    bounds: [bound(*min)?, bound(*max)?],
                })
            }
            ("$objectScripts", [label, scripts @ ..]) => Ok(Property::ObjectScripts {
                label: decode_text(label)?,
                scripts: decode_scripts(scripts)?,
            }),
            _ => Err("invalid saved property fields".into()),
        }
    })())
}

/// The snapshot marker records which command operands use saved argument semantics.
pub const INLINE_ARGUMENTS: &str = "# inline arguments: ";

pub fn inline_argument_positions(comment: &str) -> Option<&str> {
    Some(
        comment
            .strip_prefix(INLINE_ARGUMENTS)?
            .split('#')
            .next()?
            .trim(),
    )
}

/// Turn an older wrapped command into its source and the affected operand indices.
pub(super) fn command_with_inline_arguments(rhs: &str) -> Option<(String, Vec<usize>)> {
    if !rhs.contains("$arg(") {
        return None;
    }
    let (command, mut args) = split_call(rhs)?;
    let mut inline = Vec::new();
    for (index, arg) in args.iter_mut().enumerate() {
        if let Some(inner) = arg
            .strip_prefix("$arg(")
            .and_then(|arg| arg.strip_suffix(')'))
        {
            *arg = inner.trim();
            inline.push(index + 1);
        }
    }
    (!inline.is_empty()).then(|| (format!("{command}({})", args.join(", ")), inline))
}

/// Write a definition as ordinary command text with a replay annotation.
pub fn mark_inline_arguments(line: String) -> String {
    if !line.contains("$arg(") {
        return line;
    }
    let (body, comment) = super::split_comment(&line);
    if comment.is_some() {
        return line;
    }
    let Some((label, rhs)) = body.split_once('=') else {
        return line;
    };
    let Some((call, inline)) = command_with_inline_arguments(rhs.trim()) else {
        return line;
    };
    use std::fmt::Write as _;
    let mut source = format!("{} = {call} {INLINE_ARGUMENTS}", label.trim_end());
    for (index, position) in inline.into_iter().enumerate() {
        if index > 0 {
            source.push(' ');
        }
        let _ = write!(source, "{position}");
    }
    source
}

pub(super) fn lower_marked_arguments(line: &str, positions: &str) -> Result<String, String> {
    let (label, rhs) = line
        .split_once('=')
        .ok_or("inline arguments need a definition")?;
    let (command, args) = split_call(rhs.trim()).ok_or("inline arguments need a command call")?;
    let mut wrapped = vec![false; args.len()];
    if positions.is_empty() {
        return Err("missing inline argument positions".into());
    }
    for position in positions.split_whitespace() {
        let slot = position
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1));
        let mark = slot
            .and_then(|index| wrapped.get_mut(index))
            .ok_or("invalid inline argument position")?;
        *mark = true;
    }
    let args: Vec<_> = args
        .into_iter()
        .zip(wrapped)
        .map(|(arg, wrapped)| {
            if wrapped {
                format!("$arg({arg})")
            } else {
                arg.to_owned()
            }
        })
        .collect();
    Ok(format!(
        "{} = {command}({})",
        label.trim_end(),
        args.join(", ")
    ))
}
