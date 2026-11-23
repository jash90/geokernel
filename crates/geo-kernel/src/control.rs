// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoNumeric.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoBoolean.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Controls carried by an imported construction. Script text is inert data.

use crate::{Construction, Def, Id, Kind};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Control {
    pub caption: String,
    pub scripts: Vec<ControlScript>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ControlScript {
    pub language: String,
    pub event: String,
    pub text: String,
}

impl Construction {
    pub fn control(&self, id: Id) -> Option<&Control> {
        let index = self
            .controls
            .binary_search_by_key(&id, |(id, _)| *id)
            .ok()?;
        Some(&self.controls[index].1)
    }

    /// Imported object scripts, including those attached to controls.
    pub fn object_scripts(&self, label: &str) -> &[ControlScript] {
        if let Some(scripts) = self.object_scripts.get(label) {
            return scripts;
        }
        self.id(label)
            .and_then(|id| self.control(id))
            .map_or(&[], |control| &control.scripts)
    }

    /// Changes a free checkbox and updates its mathematical dependents.
    pub fn set_boolean(&mut self, id: Id, value: bool) -> Result<usize, crate::kernel::Error> {
        self.last_update_requires_ordered_replay = false;
        let def = self
            .defs
            .get_mut(id.index())
            .ok_or(crate::kernel::Error::UnknownObject)?;
        let Def::Boolean(old) = def else {
            return Err(crate::kernel::Error::NotFree(self.label(id).to_owned()));
        };
        if *old == value {
            return Ok(0);
        }
        *old = value;
        self.values[id.index()] = crate::Value::Bool(value);
        Ok(self.propagate(id))
    }
}

// Encode arbitrary captions and inert script text as single-line fields.
pub fn encode_text(text: &str) -> String {
    encode_bytes(text.as_bytes())
}

pub(crate) fn encode_bytes(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(1 + 2 * bytes.len());
    encoded.push('_');
    for byte in bytes {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

pub fn decode_text(encoded: &str) -> Result<String, String> {
    String::from_utf8(decode_bytes(encoded)?).map_err(|_| "invalid control text".into())
}

pub(crate) fn decode_bytes(encoded: &str) -> Result<Vec<u8>, String> {
    let encoded = encoded.strip_prefix('_').ok_or("invalid control text")?;
    if !encoded.is_ascii() || !encoded.len().is_multiple_of(2) {
        return Err("invalid control text".into());
    }
    (0..encoded.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&encoded[i..i + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "invalid control text".into())
}

/// Uses a named boolean directly or adds an anonymous boolean expression.
pub fn visibility_boolean(c: &mut Construction, owner: &str, source: &str) -> Result<Id, String> {
    if let Some(id) = c.id(source) {
        return (c.kind(id) == Kind::Boolean)
            .then_some(id)
            .ok_or_else(|| format!("`{source}` is not a boolean"));
    }
    let label = format!("\0condition:{owner}");
    if crate::expr::argument_object(c, source, &label, Kind::Boolean) {
        Ok(c.id(&label).expect("the boolean was just added"))
    } else {
        Err("the visibility expression cannot be evaluated as a boolean".into())
    }
}

pub(crate) fn define(
    c: &mut Construction,
    label: &str,
    form: &str,
    args: &[&str],
) -> Result<Id, String> {
    let (def, caption, scripts) = match (form, args) {
        ("$button", [caption, scripts @ ..]) => (Def::Button, caption, scripts),
        ("$checkBox", [value, caption, scripts @ ..]) => (
            Def::Boolean(value.parse().map_err(|_| "invalid checkbox value")?),
            caption,
            scripts,
        ),
        ("$inputBox", [linked, caption, scripts @ ..]) => {
            let linked = if linked.is_empty() {
                None
            } else {
                Some(crate::script::object(c, linked)?)
            };
            (Def::InputBox(linked), caption, scripts)
        }
        _ => return Err("invalid control definition".into()),
    };
    let caption = decode_text(caption)?;
    let scripts = decode_scripts(scripts)?;
    let id = c.add(label, def).map_err(|e| e.to_string())?;
    c.controls.push((id, Control { caption, scripts }));
    Ok(id)
}

pub(crate) fn decode_scripts(fields: &[&str]) -> Result<Vec<ControlScript>, String> {
    let (scripts, []) = fields.as_chunks::<3>() else {
        return Err("invalid object script fields".into());
    };
    scripts
        .iter()
        .map(|&[language, event, text]| {
            Ok(ControlScript {
                language: decode_text(language)?,
                event: decode_text(event)?,
                text: decode_text(text)?,
            })
        })
        .collect()
}

pub fn encode_scripts(scripts: &[ControlScript]) -> impl Iterator<Item = String> + '_ {
    scripts.iter().flat_map(|script| {
        [&script.language, &script.event, &script.text].map(|field| encode_text(field))
    })
}

type Position = (f64, f64);

/// The drawable properties of a numeric slider.
#[derive(Clone, Debug, PartialEq)]
pub struct SliderStyle {
    pub position: Option<Position>,
    pub absolute: bool,
    pub horizontal: bool,
    pub width: f64,
    pub fixed: bool,
    pub knob_size: Option<f64>,
    pub step: Option<f64>,
    pub step_expression: Option<std::sync::Arc<str>>,
}

impl SliderStyle {
    const DEFAULT: Self = Self {
        position: None,
        absolute: true,
        horizontal: true,
        width: 200.0,
        fixed: false,
        knob_size: None,
        step: None,
        step_expression: None,
    };
}

static DEFAULT_SLIDER_STYLE: SliderStyle = SliderStyle::DEFAULT;

impl Default for SliderStyle {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The drawable properties of a free checkbox.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheckboxStyle {
    pub position: Option<[i32; 2]>,
    pub fixed: bool,
}

/// Slider geometry in canvas pixels, shared by picking, painting and SVG.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderGeometry {
    pub start: Position,
    pub end: Position,
    pub knob: Position,
    pub radius: f64,
    pub horizontal: bool,
}

pub const CHECKBOX_SIZE: f64 = 26.0;
pub const CHECKBOX_INSET: f64 = 5.0;
pub const CHECKBOX_HIT_SIZE: f64 = CHECKBOX_SIZE + 12.0;

impl SliderGeometry {
    /// Pointer distance and track length in the same screen units.
    pub fn pointer(self, at: Position) -> (f64, f64) {
        if self.horizontal {
            (at.0 - self.start.0, self.end.0 - self.start.0)
        } else {
            (self.start.1 - at.1, self.start.1 - self.end.1)
        }
    }

    /// Label anchor at the knob, with room for the handle's outline.
    pub fn label_anchor(&self) -> Position {
        if self.horizontal {
            (
                (self.knob.0 - 9.0).max(0.0),
                self.knob.1 - self.radius - 5.0,
            )
        } else {
            (self.knob.0 + self.radius + 5.0, self.knob.1 + self.radius)
        }
    }
}

impl Construction {
    pub fn set_slider_style(&mut self, id: Id, style: SliderStyle) {
        self.slider_styles.insert(id, style);
    }

    pub fn slider_style(&self, id: Id) -> &SliderStyle {
        self.slider_styles.get(&id).unwrap_or(&DEFAULT_SLIDER_STYLE)
    }

    /// The classic desktop placement in GeoNumeric.initScreenLocation.
    pub fn slider_position(&self, id: Id, style: &SliderStyle) -> Position {
        style.position.unwrap_or_else(|| {
            let count = self
                .slider_default_slots
                .get(&id)
                .copied()
                .unwrap_or_else(|| {
                    self.sliders
                        .partition_point(|&(index, ..)| index < id.index())
                });
            if style.absolute {
                let y = 50 + 40 * count;
                (30.0, (y / 400 * 10 + y % 400) as f64)
            } else {
                (-5.0, 10.0 - count as f64)
            }
        })
    }

    pub fn slider_geometry(
        &self,
        id: Id,
        scales: Position,
        to_screen: impl FnOnce(Position) -> Position,
    ) -> Option<SliderGeometry> {
        let crate::Value::Number(value) = self.value(id) else {
            return None;
        };
        let (min, max) = self.slider_interval(id)?;
        let style = self.slider_style(id);
        let position = self.slider_position(id, style);
        let (start, width) = if style.absolute {
            ((position.0.max(0.0), position.1.max(0.0)), style.width)
        } else {
            (
                to_screen(position),
                style.width * if style.horizontal { scales.0 } else { scales.1 },
            )
        };
        let span = max - min;
        let fraction = if span == 0.0 {
            0.0
        } else if span.is_finite() {
            (value - min) / span
        } else {
            (0.5 * value - 0.5 * min) / (0.5 * max - 0.5 * min)
        };
        let along = |distance| {
            if style.horizontal {
                (start.0 + distance, start.1)
            } else {
                (start.0, start.1 - distance)
            }
        };
        let end = along(width);
        let knob = along(width * fraction.clamp(0.0, 1.0));
        let finite = [start.0, start.1, end.0, end.1, knob.0, knob.1]
            .into_iter()
            .all(|x: f64| (x as f32).is_finite());
        (finite && start != end && width > 0.0).then_some(SliderGeometry {
            start,
            end,
            knob,
            radius: (style.knob_size.unwrap_or(5.0) + 0.5).clamp(1.0, 64.0),
            horizontal: style.horizontal,
        })
    }

    /// Converts a position along the track into a min-relative increment.
    pub fn slider_pointer_value(
        &self,
        id: Id,
        (offset, track): Position,
        click: bool,
    ) -> Option<f64> {
        use super::animation::{check_decimal_fraction, next_pretty_number, AUTO_STEP_MUL};
        let (min, max) = self.slider_interval(id).filter(|(min, max)| min <= max)?;
        if !(offset.is_finite() && track.is_finite() && track > 0.0) {
            return None;
        }
        let offset = offset.clamp(0.0, track);
        let width = max - min;
        let animation = self.animation(id);
        let style = self.slider_style(id);
        let step = style
            .step
            .or_else(|| {
                self.arithmetic_value(style.step_expression.as_deref()?)
                    .filter(|step| step.is_finite())
            })
            .or_else(|| animation.filter(|a| !a.auto_step).map(|a| a.step))
            .unwrap_or_else(|| {
                let speed = animation.map_or(1.0, |a| a.speed);
                next_pretty_number(width * speed * AUTO_STEP_MUL).max(0.1)
            });
        // EuclidianController.getSliderValue multiplies before dividing.
        let product = offset * width;
        let distance = if product.is_finite() {
            product / track
        } else {
            (offset / track) * width
        };
        let steps = distance / step;
        let value = if !width.is_finite() {
            let fraction = offset / track;
            min * (1.0 - fraction) + max * fraction
        } else if step > 0.0 && steps.is_finite() {
            min + crate::expr::math::java_round(steps) as f64 * step
        } else {
            min + distance
        };
        let mut value = if step > crate::tol::MIN_PRECISION {
            check_decimal_fraction(value)
        } else {
            value
        };
        if click {
            let old = self.number(id)?;
            // Clicking advances one step toward the rounded target, even
            // when the current value is between two notches.
            if (value - old).abs() >= crate::tol::STANDARD_PRECISION {
                value = check_decimal_fraction(if value > old {
                    (old + step).min(max)
                } else {
                    (old - step).max(min)
                });
            }
        }
        value.is_finite().then(|| value.clamp(min, max))
    }

    pub fn set_checkbox_style(&mut self, id: Id, style: CheckboxStyle) {
        self.checkbox_styles.insert(id, style);
    }

    pub fn checkbox_style(&self, id: Id) -> CheckboxStyle {
        self.checkbox_styles.get(&id).copied().unwrap_or_default()
    }

    /// The classic desktop placement in GeoBoolean.initScreenLocation.
    pub fn checkbox_position(&self, id: Id) -> Option<Position> {
        let index = self.free_booleans.binary_search(&id).ok()?;
        let [x, y] = self.checkbox_style(id).position.unwrap_or_else(|| {
            let y = 5 + 30 * index;
            [5, (y / 400 * 10 + y % 400) as i32]
        });
        Some((f64::from(x), f64::from(y)))
    }

    pub fn checkbox_caption(&self, id: Id) -> &str {
        self.control(id)
            .map(|control| control.caption.as_str())
            .filter(|caption| !caption.is_empty())
            .unwrap_or_else(|| self.label(id))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_small_explicit_slider_step_is_not_rounded_away() {
        let c = crate::script::load("a = Slider(0, 0.000000000001, 0.00000000000001)").unwrap();
        let a = c.id("a").unwrap();
        let value = c.slider_pointer_value(a, (0.054, 1.0), false).unwrap();
        assert!((value - 5e-14).abs() < 1e-28, "{value}");
    }

    #[test]
    fn a_saved_step_expression_reads_the_current_construction() {
        // "_6e" is the encoded name of the number holding the step.
        let mut c = crate::script::load(
            "n = 2\na = Slider(0, 10)\n\
             $sliderStyle(_61, true, 30, 50, 200, true, false, _, _6e)",
        )
        .unwrap();
        let a = c.id("a").unwrap();
        let n = c.id("n").unwrap();
        assert_eq!(c.slider_pointer_value(a, (0.54, 1.0), false), Some(6.0));
        c.set_number(n, 0.5).unwrap();
        assert_eq!(c.slider_pointer_value(a, (0.54, 1.0), false), Some(5.5));
    }
}
