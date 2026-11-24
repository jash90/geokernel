//! Journal properties that are restored after the named objects exist.

use super::{object, split_call};
use crate::kernel::control::{
    decode_bytes, decode_scripts, decode_text, encode_scripts, encode_text, CheckboxStyle,
    ControlScript, NumberRounding, SliderLabel, SliderStyle,
};
use crate::kernel::image::{RasterAsset, MAX_ASSET_BYTES};
use crate::kernel::DrawingStyle;
use crate::Construction;
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum Property {
    ResetCircleRecords,
    ResetArcRecords,
    ResetTransposeFlags,
    ImageAsset {
        name: Arc<str>,
        asset: Arc<RasterAsset>,
    },
    DrawingStyle {
        label: String,
        style: DrawingStyle,
    },
    NumberRounding(NumberRounding),
    SliderLabel {
        label: String,
        presentation: SliderLabel,
    },
    RegularVertex {
        label: String,
        x: f64,
        y: f64,
    },
    SliderBounds {
        label: String,
        bounds: [Option<String>; 2],
    },
    SliderStyle {
        label: String,
        style: SliderStyle,
    },
    CheckboxStyle {
        label: String,
        style: CheckboxStyle,
    },
    ObjectScripts {
        label: String,
        scripts: Vec<ControlScript>,
    },
    /// The animation of a slider a file saved as playing; `running` when
    /// the file also started the timer (`<startAnimation>`).
    SliderAnimation {
        label: String,
        step: Option<f64>,
        speed: f64,
        mode: u8,
        running: bool,
    },
}

impl Property {
    pub fn label(&self) -> Option<&str> {
        match self {
            Self::DrawingStyle { label, .. }
            | Self::RegularVertex { label, .. }
            | Self::SliderBounds { label, .. }
            | Self::SliderLabel { label, .. }
            | Self::SliderStyle { label, .. }
            | Self::CheckboxStyle { label, .. }
            | Self::ObjectScripts { label, .. }
            | Self::SliderAnimation { label, .. } => Some(label),
            Self::ImageAsset { .. }
            | Self::NumberRounding(_)
            | Self::ResetCircleRecords
            | Self::ResetArcRecords
            | Self::ResetTransposeFlags => None,
        }
    }

    pub fn apply(&self, c: &mut Construction) -> Result<(), String> {
        match self {
            Self::ResetCircleRecords => {
                c.reset_circle_records();
                Ok(())
            }
            Self::ResetArcRecords => {
                c.reset_arc_records();
                Ok(())
            }
            Self::ResetTransposeFlags => {
                c.reset_transpose_flags();
                Ok(())
            }
            Self::ImageAsset { name, asset } => c.register_image_asset(name.clone(), asset.clone()),
            Self::DrawingStyle { label, style } => {
                c.set_drawing_style(object(c, label)?, *style);
                Ok(())
            }
            Self::NumberRounding(rounding) => {
                c.set_number_rounding(*rounding);
                Ok(())
            }
            Self::SliderLabel {
                label,
                presentation,
            } => {
                c.set_slider_label(object(c, label)?, presentation.clone());
                Ok(())
            }
            Self::RegularVertex { label, x, y } => c
                .restore_regular_vertex(object(c, label)?, *x, *y)
                .map_err(|e| e.to_string()),
            Self::SliderBounds { label, bounds } => {
                let id = object(c, label)?;
                let [min, max] = bounds;
                c.restore_slider_bounds(id, [min.as_deref(), max.as_deref()])
            }
            Self::SliderStyle { label, style } => {
                let id = object(c, label)?;
                if !matches!(c.def(id), crate::Def::Number(_)) {
                    return Err("slider style needs a free number".into());
                }
                c.set_slider_style(id, style.clone());
                Ok(())
            }
            Self::CheckboxStyle { label, style } => {
                let id = object(c, label)?;
                if !matches!(c.def(id), crate::Def::Boolean(_)) {
                    return Err("checkbox style needs a free boolean".into());
                }
                c.set_checkbox_style(id, *style);
                Ok(())
            }
            Self::ObjectScripts { label, scripts } => {
                c.object_scripts.insert(label.clone(), scripts.clone());
                Ok(())
            }
            Self::SliderAnimation {
                label,
                step,
                speed,
                mode,
                running,
            } => {
                use crate::kernel::animation::{Animation, AnimationMode};
                let id = object(c, label)?;
                if !matches!(c.def(id), crate::Def::Number(_)) {
                    return Err("an animation needs a free number".into());
                }
                let mode = AnimationMode::from_code(i64::from(*mode));
                c.restore_animation(id, Animation::saved(*step, *speed, mode, true), *running);
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
            Self::ResetCircleRecords => f.write_str("$resetCircleRecords()"),
            Self::ResetArcRecords => f.write_str("$resetArcRecords()"),
            Self::ResetTransposeFlags => f.write_str("$resetTransposeFlags()"),
            Self::ImageAsset { name, asset } => {
                write!(f, "$imageAsset({}, _", encode_text(name))?;
                for byte in asset.bytes() {
                    write!(f, "{byte:02x}")?;
                }
                f.write_str(")")
            }
            Self::DrawingStyle { label, style } => write!(
                f,
                "$drawingStyle({}, {}, {}, {})",
                encode_text(label),
                style.layer,
                style.show_label,
                style.line_thickness
            ),
            Self::NumberRounding(rounding) => {
                let (mode, digits) = match rounding {
                    NumberRounding::Decimals(digits) => ("decimals", digits),
                    NumberRounding::SignificantFigures(digits) => ("figures", digits),
                };
                write!(f, "$numberRounding({mode}, {digits})")
            }
            Self::SliderLabel {
                label,
                presentation,
            } => write!(
                f,
                "$sliderLabel({}, {}, {})",
                encode_text(label),
                presentation.mode,
                encode_text(&presentation.caption),
            ),
            Self::RegularVertex { label, x, y } => {
                write!(f, "$regularVertex({}, {x:?}, {y:?})", encode_text(label))
            }
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
            Self::SliderAnimation {
                label,
                step,
                speed,
                mode,
                running,
            } => write!(
                f,
                "$sliderAnimation({}, {}, {speed:?}, {mode}, {running})",
                encode_text(label),
                step.map_or_else(|| "_".into(), |s| format!("{s:?}"))
            ),
            Self::SliderStyle { label, style } => {
                let field = |v: Option<f64>| v.map_or_else(|| "_".into(), |v| format!("{v:?}"));
                let (x, y) = style.position.unzip();
                write!(
                    f,
                    "$sliderStyle({}, {}, {}, {}, {:?}, {}, {}, {}, {})",
                    encode_text(label),
                    style.absolute,
                    field(x),
                    field(y),
                    style.width,
                    style.horizontal,
                    style.fixed,
                    field(style.knob_size),
                    style
                        .step_expression
                        .as_deref()
                        .map_or_else(|| field(style.step), encode_text),
                )
            }
            Self::CheckboxStyle { label, style } => {
                let field = |v: Option<i32>| v.map_or_else(|| "_".into(), |v| v.to_string());
                let (x, y) = style.position.map(|[x, y]| (x, y)).unzip();
                write!(
                    f,
                    "$checkboxStyle({}, {}, {}, {})",
                    encode_text(label),
                    field(x),
                    field(y),
                    style.fixed,
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

/// Load a raster declaration before normalizing or copying the script line.
pub fn load_image_asset(
    c: &mut Construction,
    raw: &str,
) -> Option<Result<Option<Property>, String>> {
    let raw = raw.trim_start().strip_prefix("$imageAsset(")?;
    Some((|| {
        let body = raw.split_once('#').map_or(raw, |(body, _)| body).trim_end();
        let (name, bytes) = body
            .strip_suffix(')')
            .and_then(|body| body.split_once(','))
            .ok_or("invalid saved image asset")?;
        let (name, bytes) = (name.trim(), bytes.trim());
        if name.len() > 2 * 65535 + 1 || bytes.len() as u64 > 2 * MAX_ASSET_BYTES + 1 {
            return Err("image asset is too large".into());
        }
        c.reserve_image_input(((bytes.len() + name.len()) / 2) as u64)?;
        let name: Arc<str> = decode_text(name)?.into();
        let asset = c.decode_image_asset(name.clone(), decode_bytes(bytes)?)?;
        Ok(asset.map(|asset| Property::ImageAsset { name, asset }))
    })())
}

pub fn parse(line: &str) -> Option<Result<Property, String>> {
    if line == "$resetCircleRecords()" {
        return Some(Ok(Property::ResetCircleRecords));
    }
    if line == "$resetArcRecords()" {
        return Some(Ok(Property::ResetArcRecords));
    }
    if line == "$resetTransposeFlags()" {
        return Some(Ok(Property::ResetTransposeFlags));
    }
    if !line.starts_with("$sliderBounds(")
        && !line.starts_with("$sliderAnimation(")
        && !line.starts_with("$numberRounding(")
        && !line.starts_with("$sliderLabel(")
        && !line.starts_with("$sliderStyle(")
        && !line.starts_with("$checkboxStyle(")
        && !line.starts_with("$objectScripts(")
        && !line.starts_with("$drawingStyle(")
        && !line.starts_with("$regularVertex(")
    {
        return None;
    }
    Some((|| {
        let (form, args) = split_call(line).ok_or("invalid saved property")?;
        match (form, args.as_slice()) {
            ("$drawingStyle", [label, layer, shown, thickness]) => {
                let style = DrawingStyle {
                    layer: layer.parse().map_err(|_| "invalid drawing layer")?,
                    show_label: shown.parse().map_err(|_| "invalid label visibility")?,
                    line_thickness: thickness.parse().map_err(|_| "invalid line thickness")?,
                };
                Ok(Property::DrawingStyle {
                    label: decode_text(label)?,
                    style,
                })
            }
            ("$numberRounding", [mode, digits]) => {
                let digits = digits.parse().map_err(|_| "invalid rounding precision")?;
                Ok(Property::NumberRounding(match *mode {
                    "decimals" => NumberRounding::Decimals(digits),
                    "figures" => NumberRounding::SignificantFigures(digits),
                    _ => return Err("invalid number rounding mode".into()),
                }))
            }
            ("$sliderLabel", [label, mode, caption]) => Ok(Property::SliderLabel {
                label: decode_text(label)?,
                presentation: SliderLabel {
                    mode: mode.parse().map_err(|_| "invalid slider label mode")?,
                    caption: decode_text(caption)?.into(),
                },
            }),
            ("$regularVertex", [label, x, y]) => Ok(Property::RegularVertex {
                label: decode_text(label)?,
                x: x.parse().map_err(|_| "invalid vertex x coordinate")?,
                y: y.parse().map_err(|_| "invalid vertex y coordinate")?,
            }),
            ("$sliderBounds", [label, min, max]) => {
                let bound = |field| (field != "_").then(|| decode_text(field)).transpose();
                Ok(Property::SliderBounds {
                    label: decode_text(label)?,
                    bounds: [bound(*min)?, bound(*max)?],
                })
            }
            ("$sliderStyle", [label, absolute, x, y, width, horizontal, fixed, knob, step]) => {
                let number = |field: &str| {
                    field
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .ok_or("invalid slider number")
                };
                let optional = |field: &str| (field != "_").then(|| number(field)).transpose();
                let flag = |field: &str| field.parse::<bool>().map_err(|_| "invalid slider flag");
                Ok(Property::SliderStyle {
                    label: decode_text(label)?,
                    style: SliderStyle {
                        absolute: flag(absolute)?,
                        position: optional(x)?.zip(optional(y)?),
                        width: number(width)?,
                        horizontal: flag(horizontal)?,
                        fixed: flag(fixed)?,
                        knob_size: optional(knob)?,
                        step: if step.starts_with('_') {
                            None
                        } else {
                            optional(step)?
                        },
                        step_expression: (step.starts_with('_') && *step != "_")
                            .then(|| decode_text(step).map(Arc::from))
                            .transpose()?,
                    },
                })
            }
            ("$checkboxStyle", [label, x, y, fixed]) => {
                let coordinate = |field: &str| {
                    (field != "_")
                        .then(|| field.parse::<i32>())
                        .transpose()
                        .map_err(|_| "invalid checkbox coordinate")
                };
                Ok(Property::CheckboxStyle {
                    label: decode_text(label)?,
                    style: CheckboxStyle {
                        position: coordinate(x)?.zip(coordinate(y)?).map(|(x, y)| [x, y]),
                        fixed: fixed.parse().map_err(|_| "invalid checkbox flag")?,
                    },
                })
            }
            ("$sliderAnimation", [label, step, speed, mode, running]) => {
                let number = |field: &str| -> Result<f64, String> {
                    field
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| "invalid animation number".into())
                };
                Ok(Property::SliderAnimation {
                    label: decode_text(label)?,
                    step: (*step != "_").then(|| number(step)).transpose()?,
                    speed: number(speed)?,
                    mode: mode.parse().map_err(|_| "invalid animation mode")?,
                    running: running.parse().map_err(|_| "invalid animation state")?,
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
