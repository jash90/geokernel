use super::*;
use crate::geom::Rgba;
use crate::properties::{EditError, Field, Input, MAX_EDIT_TARGETS};
use geo_kernel::kernel::angle::AngleStyle;
use geo_kernel::kernel::appearance::Playback;
use geo_kernel::kernel::control::NumberRounding;
use geo_kernel::kernel::text::TextFont;
use geo_kernel::kernel::EquationForm;

const MAX_INPUT: usize = 2048;
const MAX_PROPAGATION: usize = 8192;
const LINE_TYPES: [i32; 5] = [0, 10, 15, 20, 30];

fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn rgb(raw: &str) -> Option<[u8; 3]> {
    let raw = raw.strip_prefix('#').unwrap_or(raw);
    if raw.len() != 6 {
        return None;
    }
    let [_, r, g, b] = u32::from_str_radix(raw, 16).ok()?.to_be_bytes();
    Some([r, g, b])
}

fn coords(position: TextPosition) -> (f64, f64) {
    match position {
        TextPosition::Screen { x, y } | TextPosition::World { x, y } => (x, y),
        _ => (0.0, 0.0),
    }
}

fn playback(c: &Construction, id: Id) -> Playback {
    c.animation(id).map_or(
        Playback {
            speed: 1.0,
            mode: 1,
            playing: false,
        },
        |a| Playback {
            speed: a.speed,
            mode: a.mode.code(),
            playing: a.playing,
        },
    )
}

fn dynamic_channel(field: Field) -> usize {
    match field {
        Field::DynamicRed => 0,
        Field::DynamicGreen => 1,
        Field::DynamicBlue => 2,
        _ => 3,
    }
}

impl Workbench {
    pub fn property_value(&self, id: Id, field: Field) -> String {
        use Field::*;
        let c = &self.construction;
        let a = c.appearance(id);
        let label = c.label(id);
        let saved = self.file_object_styles.get(label);
        let drawing = c.drawing_style(id);
        let slider = c.slider_style(id);
        let checkbox = c.checkbox_style(id);
        let text = c.text_presentation(id).unwrap_or_default();
        let image = c.image(id);
        let show = |value: f64| value.to_string();
        let position = || {
            if c.slider_default_slot(id).is_some() {
                c.slider_position(id, slider)
            } else if let Some(xy) = c.checkbox_position(id) {
                xy
            } else if c.kind(id) == Kind::InputBox {
                let [x, y] = c.input_box_style(id).position;
                (f64::from(x), f64::from(y))
            } else {
                coords(text.position)
            }
        };
        match field {
            Caption => a
                .caption
                .as_deref()
                .or_else(|| saved.map(|s| s.caption.as_str()))
                .or_else(|| c.control(id).map(|s| s.caption.as_str()))
                .unwrap_or_default()
                .to_owned(),
            Visible => self.is_visible(id).to_string(),
            ShowLabel => drawing.show_label.to_string(),
            LabelMode => a
                .label_mode
                .or_else(|| saved.and_then(|s| s.label_mode))
                .unwrap_or(if c.kind(id) == Kind::Angle || c.has_slider_bounds(id) {
                    1
                } else {
                    0
                })
                .clamp(0, 4)
                .to_string(),
            Animating => playback(c, id).playing.to_string(),
            Fixed => a
                .fixed
                .or_else(|| saved.and_then(|s| s.fixed))
                .unwrap_or(text.fixed)
                .to_string(),
            Auxiliary => a
                .auxiliary
                .or_else(|| saved.and_then(|s| s.auxiliary))
                .unwrap_or(false)
                .to_string(),
            CheckboxFixed => checkbox.fixed.to_string(),
            ImageBackground => image.is_some_and(|im| im.style.background).to_string(),
            AngleInterval => c.angle_style(id).unwrap_or_default().xml_code().to_string(),
            RightAngle => a
                .right_angle
                .or_else(|| saved.and_then(|s| s.emphasize_right_angle))
                .unwrap_or(true)
                .to_string(),
            Color => {
                let Rgba { r, g, b, .. } = self.palette().static_color(id);
                hex([r, g, b])
            }
            FillOpacity => show(
                (100.0
                    * a.fill_opacity
                        .or_else(|| image.map(|im| f64::from(im.style.alpha)))
                        .or_else(|| saved.and_then(|s| s.alpha))
                        .unwrap_or_else(|| crate::style::fill_opacity(c, id)))
                .round(),
            ),
            TextBackground => a
                .text_background
                .or_else(|| saved.and_then(|s| s.background))
                .filter(|color| color[3] != 0)
                .map_or(String::new(), |[r, g, b, _]| hex([r, g, b])),
            SliderTrackColor => hex(a.slider_track_color.unwrap_or([0, 0, 0])),
            PointSize => a
                .point_size
                .or_else(|| saved.and_then(|s| s.point_size))
                .unwrap_or(5)
                .to_string(),
            PointStyle => a
                .point_style
                .or_else(|| saved.and_then(|s| s.point_style))
                .unwrap_or(0)
                .clamp(0, 10)
                .to_string(),
            Thickness if c.has_slider_bounds(id) => show(f64::from(drawing.line_thickness) / 2.0),
            Thickness => drawing.line_thickness.to_string(),
            LineType => {
                let code = a
                    .line_type
                    .or_else(|| saved.and_then(|s| s.line.map(|line| line.kind)))
                    .unwrap_or(0);
                LINE_TYPES
                    .iter()
                    .position(|&ty| ty == code)
                    .unwrap_or(0)
                    .to_string()
            }
            LineOpacity => {
                let alpha = a
                    .line_opacity
                    .or_else(|| saved.and_then(|s| s.line?.opacity))
                    .unwrap_or(255);
                show((f64::from(alpha) * 100.0 / 255.0).round())
            }
            AngleRadius => a
                .angle_radius
                .or_else(|| saved.and_then(|s| s.arc_size))
                .unwrap_or(30)
                .to_string(),
            ImageInterpolate => image.is_some_and(|im| im.style.interpolate).to_string(),
            InputLength => c.input_box_style(id).length.to_string(),
            SegmentStart | SegmentEnd => {
                let i = usize::from(field == SegmentEnd);
                a.segment_ends[i]
                    .or_else(|| saved.and_then(|s| s.ends[i].map(|e| e as u8)))
                    .unwrap_or_default()
                    .min(7)
                    .to_string()
            }
            VectorArrow => a
                .vector_arrow
                .or_else(|| saved.and_then(|s| s.head_style.map(|h| h != 0)))
                .unwrap_or(true)
                .to_string(),
            SegmentDecoration => a
                .decoration
                .or_else(|| saved.and_then(|s| u8::try_from(s.decoration?).ok()))
                .unwrap_or_default()
                .min(6)
                .to_string(),
            TextSize => show(text.size * 100.0),
            TextBold => text.bold.to_string(),
            TextItalic => text.italic.to_string(),
            TextLatex => text.latex.to_string(),
            SliderKnob => show(slider.knob_size.unwrap_or(5.0)),
            Absolute => if c.slider_default_slot(id).is_some() {
                slider.absolute
            } else {
                matches!(
                    text.position,
                    TextPosition::Screen { .. } | TextPosition::ScreenPoint(_)
                )
            }
            .to_string(),
            Anchor => match text.position {
                TextPosition::Point(point) | TextPosition::ScreenPoint(point) => {
                    c.label(point).to_owned()
                }
                _ => String::new(),
            },
            PositionX => show(position().0),
            PositionY => show(position().1),
            LabelX => show(self.label_offset(id).unwrap_or_default().0),
            LabelY => show(self.label_offset(id).unwrap_or_default().1),
            Condition => self
                .log
                .condition_source(label)
                .unwrap_or_default()
                .to_owned(),
            DynamicSpace => {
                let space = if a.dynamic.edited {
                    i32::from(a.dynamic.space)
                } else {
                    saved
                        .and_then(|s| s.dynamic_color.as_ref())
                        .map_or(0, |d| d.space)
                };
                space.clamp(0, 2).to_string()
            }
            DynamicRed | DynamicGreen | DynamicBlue | DynamicAlpha => {
                let i = dynamic_channel(field);
                if a.dynamic.edited {
                    a.dynamic.channels[i].clone().unwrap_or_default()
                } else {
                    saved
                        .and_then(|s| s.dynamic_color.as_ref())
                        .and_then(|d| d.channels().nth(i))
                        .unwrap_or_default()
                        .to_owned()
                }
            }
            Layer => drawing.layer.to_string(),
            Tooltip => a
                .tooltip
                .or_else(|| saved.and_then(|s| s.tooltip_mode))
                .unwrap_or(0)
                .clamp(0, 4)
                .to_string(),
            Selectable => a
                .selection_allowed
                .or_else(|| saved.and_then(|s| s.selection_allowed))
                .unwrap_or(true)
                .to_string(),
            LineEquation => match c.equation_form(id) {
                Some(EquationForm::Explicit) => "1",
                Some(EquationForm::Parametric(_)) => "2",
                _ => "0",
            }
            .to_owned(),
            ConicEquation => match c.equation_form(id) {
                Some(EquationForm::Specific) => "1",
                Some(EquationForm::Parametric(_)) => "2",
                _ => "0",
            }
            .to_owned(),
            SliderMin | SliderMax => c.slider_bound_sources(id)[usize::from(field == SliderMax)]
                .clone()
                .unwrap_or_default(),
            DecimalPlaces => match a.input_rounding {
                Some(NumberRounding::Decimals(digits)) => digits.to_string(),
                _ => "–".to_owned(),
            },
            SignificantFigures => match a.input_rounding {
                Some(NumberRounding::SignificantFigures(digits)) => digits.to_string(),
                _ => "–".to_owned(),
            },
            SliderWidth => show(slider.width),
            SliderHorizontal => slider.horizontal.to_string(),
            SliderFixed => slider.fixed.to_string(),
            SliderStep => show(
                c.slider_animation_options(id)[0]
                    .and_then(|src| c.numeric_value(src))
                    .or(slider.step)
                    .unwrap_or(0.0),
            ),
            AnimationSpeed => show(playback(c, id).speed),
            AnimationMode => playback(c, id).mode.to_string(),
        }
    }

    pub fn auxiliary_object(&self, id: Id) -> bool {
        self.construction
            .appearance(id)
            .auxiliary
            .or_else(|| {
                self.file_object_styles
                    .get(self.construction.label(id))?
                    .auxiliary
            })
            .unwrap_or(false)
    }
}

enum Change<'a> {
    Saved(Property),
    Angle(AngleStyle),
    Text(TextOverrides),
    Visibility(bool),
    Condition(&'a str),
}

impl Workbench {
    /// Validates every target before changing the document. Edits that can
    /// propagate through the construction are charged against both its size
    /// and the number of edited objects.
    pub fn edit_property(
        &mut self,
        labels: &[String],
        field: Field,
        raw: &str,
        view: Option<&crate::interact::ViewTransform>,
    ) -> Result<(), EditError> {
        if labels.len() > MAX_EDIT_TARGETS {
            return Err(EditError::SelectionTooLarge);
        }
        let raw = raw.trim();
        if raw.len() > MAX_INPUT {
            return Err(EditError::InvalidValue);
        }
        use Field::*;
        // Graph edits, formula preparation and playback can revisit unrelated
        // objects. Keyed appearance records only need the target-count limit.
        let charged_targets = match field {
            AngleInterval | LineEquation | ConicEquation | SliderMin | SliderMax | Condition
            | Animating | AnimationSpeed | AnimationMode | DynamicRed | DynamicGreen
            | DynamicBlue | DynamicAlpha | Anchor | TextSize | TextBold | TextItalic
            | TextLatex => labels.len(),
            Absolute | PositionX | PositionY => labels
                .iter()
                .filter(|label| {
                    self.construction
                        .id(label)
                        .is_some_and(|id| self.construction.kind(id) == Kind::Text)
                })
                .count(),
            _ => 0,
        };
        if charged_targets.saturating_mul(self.construction.len()) > MAX_PROPAGATION {
            return Err(EditError::SelectionTooLarge);
        }
        if self.tool_use.is_some() {
            self.cancel_tool_use();
        }
        let mut changes = Vec::new();
        for label in labels {
            let id = self
                .construction
                .id(label)
                .ok_or(EditError::MissingTarget)?;
            if !field.supports(&self.construction, id)
                || (field == Visible && self.log.condition_source(label).is_some())
            {
                return Err(EditError::InvalidValue);
            }
            if self.property_value(id, field) != raw {
                changes.push((
                    id,
                    label.as_str(),
                    self.prepare_property(id, label, field, raw, view)?,
                ));
            }
        }
        if changes.is_empty() {
            return Ok(());
        }
        let before = self.state(self.copy(&self.log));
        let old_len = self.construction.len();
        for (id, label, change) in changes {
            let result = match change {
                Change::Saved(property) => property.apply(&mut self.construction).map(|_| {
                    let appearance = self.construction.appearance(id);
                    let mark = match (&property, field) {
                        (Property::DrawingStyle { .. }, Layer) if !appearance.edited_layer => {
                            Some(Layer)
                        }
                        (Property::DrawingStyle { .. }, Thickness)
                            if !appearance.edited_thickness =>
                        {
                            Some(Thickness)
                        }
                        (Property::SliderStyle { .. }, _) if !appearance.edited_slider => {
                            Some(SliderWidth)
                        }
                        (Property::CheckboxStyle { .. }, _) if !appearance.edited_checkbox => {
                            Some(CheckboxFixed)
                        }
                        _ => None,
                    };
                    self.log.record_saved_property(property);
                    if let Some(mark) = mark {
                        let mut style = appearance.clone();
                        match mark {
                            Layer => style.edited_layer = true,
                            Thickness => style.edited_thickness = true,
                            SliderWidth => style.edited_slider = true,
                            _ => style.edited_checkbox = true,
                        }
                        self.construction
                            .set_appearance(id, style.clone())
                            .expect("the file projection does not change a color channel");
                        self.log.record_saved_property(Property::Appearance {
                            label: label.to_owned(),
                            style: Box::new(style),
                        });
                    }
                }),
                Change::Angle(style) => self
                    .construction
                    .set_angle_style(id, style)
                    .map(|_| {
                        self.log
                            .record_set(label, crate::scriptlog::Setting::AngleStyle(style))
                    })
                    .map_err(|error| error.to_string()),
                Change::Text(mut overrides) => {
                    let anchor = match overrides.position {
                        Some(TextPosition::Point(point) | TextPosition::ScreenPoint(point)) => {
                            self.construction.label(point).to_owned()
                        }
                        _ => String::new(),
                    };
                    let position = overrides.position.take().map(|position| {
                        position
                            .try_map_point(|_| Ok::<_, std::convert::Infallible>(anchor.as_str()))
                            .unwrap()
                    });
                    let text = TextProperties {
                        label,
                        overrides: TextOverrides {
                            position,
                            latex: overrides.latex,
                            font: overrides.font,
                            offset: overrides.offset,
                        },
                    };
                    text.apply(&mut self.construction)
                        .map(|_| self.log.record_text_properties(text))
                }
                Change::Visibility(shown) => {
                    if shown {
                        self.log.hidden.remove(label);
                    } else {
                        self.log.hidden.insert(label.to_owned());
                    }
                    Ok(())
                }
                Change::Condition(source) => {
                    self.log
                        .edit_condition(&mut self.construction, label, source)
                }
            };
            if result.is_err() {
                let _ = self.restore(before.log);
                return Err(EditError::InvalidValue);
            }
        }
        if self.construction.len() != old_len {
            self.objects_changed();
        }
        self.version += 1;
        self.record_undo(before);
        Ok(())
    }

    fn prepare_property<'a>(
        &self,
        id: Id,
        label: &'a str,
        field: Field,
        raw: &'a str,
        view: Option<&crate::interact::ViewTransform>,
    ) -> Result<Change<'a>, EditError> {
        use Field::*;
        let c = &self.construction;
        let invalid = EditError::InvalidValue;
        let integer = |min, max| {
            raw.parse::<i32>()
                .ok()
                .filter(|v| (min..=max).contains(v))
                .ok_or(invalid)
        };
        let number = |min, max| {
            raw.replace(',', ".")
                .parse::<f64>()
                .ok()
                .filter(|v| (min..=max).contains(v))
                .ok_or(invalid)
        };
        let mut numeric = 0.0;
        let mut integral = 0;
        let mut checked = false;
        match field.input_for(c, id) {
            Input::Integer(min, max) => integral = integer(min, max)?,
            Input::Number(min, max) => numeric = number(min, max)?,
            Input::Choice(choices) => integral = integer(0, choices.len() as i32 - 1)?,
            Input::Check => checked = raw.parse().map_err(|_| invalid)?,
            Input::Text | Input::Color => {}
        }
        let saved = |p| Ok(Change::Saved(p));
        let label_owned = || label.to_owned();
        let mut drawing = c.drawing_style(id);
        let mut slider = c.slider_style(id).clone();
        let mut checkbox = c.checkbox_style(id);
        let mut input = c.input_box_style(id);
        let mut appearance = c.appearance(id).clone();
        if matches!(
            field,
            DynamicSpace | DynamicRed | DynamicGreen | DynamicBlue | DynamicAlpha
        ) && !appearance.dynamic.edited
        {
            let saved = self
                .file_object_styles
                .get(label)
                .and_then(|s| s.dynamic_color.as_ref());
            if let Some(saved) = saved {
                appearance.dynamic.space = saved.space.clamp(0, 2) as u8;
                for (channel, value) in appearance.dynamic.channels.iter_mut().zip(saved.channels())
                {
                    *channel = Some(value.to_owned());
                }
            }
            appearance.dynamic.edited = true;
        }
        let text = c.text_presentation(id);
        let mut text_changes = TextOverrides::default();
        match field {
            Visible => return Ok(Change::Visibility(checked)),
            Condition => return Ok(Change::Condition(raw)),
            AngleInterval => return Ok(Change::Angle(AngleStyle::from_xml(integral))),
            LineEquation | ConicEquation => {
                let form = match (field, integral) {
                    (LineEquation, 1) => EquationForm::Explicit,
                    (LineEquation | ConicEquation, 2) => EquationForm::Parametric("t".to_owned()),
                    (ConicEquation, 1) => EquationForm::Specific,
                    _ => EquationForm::Implicit,
                };
                return saved(Property::EquationForm {
                    label: label_owned(),
                    form,
                });
            }
            SliderMin | SliderMax => {
                let mut bounds = c.slider_bound_sources(id);
                let i = usize::from(field == SliderMax);
                bounds[i] = (!raw.is_empty()).then(|| raw.to_owned());
                return saved(Property::SliderBounds {
                    label: label_owned(),
                    bounds,
                });
            }
            Color => drawing.color = Some(rgb(raw).ok_or(invalid)?),
            ShowLabel => {
                drawing.show_label = checked;
                drawing.label_edited = true;
            }
            Thickness if c.has_slider_bounds(id) => {
                drawing.line_thickness = if numeric <= 0.0 {
                    2
                } else {
                    (numeric * 2.0).round() as u8
                };
            }
            Thickness => {
                if integral == 0
                    && !matches!(c.kind(id), Kind::Polygon | Kind::Sector | Kind::Angle)
                {
                    return Err(invalid);
                }
                drawing.line_thickness = integral as u8;
            }
            Layer => drawing.layer = integral as u8,
            Caption => {
                appearance.caption = Some(if raw == label {
                    String::new()
                } else {
                    raw.to_owned()
                })
            }
            LabelMode => appearance.label_mode = Some(integral),
            Fixed => appearance.fixed = Some(checked),
            Auxiliary => appearance.auxiliary = Some(checked),
            Selectable => appearance.selection_allowed = Some(checked),
            RightAngle => appearance.right_angle = Some(checked),
            ImageBackground => appearance.image_background = Some(checked),
            ImageInterpolate => appearance.image_interpolate = Some(checked),
            FillOpacity => appearance.fill_opacity = Some(f64::from(integral) / 100.0),
            TextBackground => {
                appearance.text_background = Some(if raw.is_empty() {
                    [0; 4]
                } else {
                    let [r, g, b] = rgb(raw).ok_or(invalid)?;
                    [r, g, b, 255]
                });
            }
            SliderTrackColor => appearance.slider_track_color = Some(rgb(raw).ok_or(invalid)?),
            PointSize => appearance.point_size = Some(integral),
            PointStyle => appearance.point_style = Some(integral),
            LineType => appearance.line_type = Some(LINE_TYPES[integral as usize]),
            LineOpacity => {
                appearance.line_opacity = Some((f64::from(integral) * 255.0 / 100.0).round() as i32)
            }
            AngleRadius => appearance.angle_radius = Some(integral),
            Tooltip => appearance.tooltip = Some(integral),
            LabelX | LabelY => {
                appearance.label_offset[usize::from(field == LabelY)] = Some(integral)
            }
            SegmentStart | SegmentEnd => {
                appearance.segment_ends[usize::from(field == SegmentEnd)] = Some(integral as u8)
            }
            SegmentDecoration => appearance.decoration = Some(integral as u8),
            VectorArrow => appearance.vector_arrow = Some(checked),
            CheckboxFixed => checkbox.fixed = checked,
            InputLength => input.length = integral,
            DynamicSpace => appearance.dynamic.space = integral as u8,
            DynamicRed | DynamicGreen | DynamicBlue | DynamicAlpha => {
                appearance.dynamic.channels[dynamic_channel(field)] =
                    (!raw.is_empty()).then(|| raw.to_owned());
            }
            Animating | AnimationSpeed | AnimationMode => {
                let mut value = playback(c, id);
                match field {
                    Animating => value.playing = checked,
                    AnimationSpeed => value.speed = numeric,
                    _ => value.mode = integral as u8,
                }
                appearance.playback = Some(value);
            }
            TextSize | TextBold | TextItalic => {
                let text = text.ok_or(invalid)?;
                let mut font = TextFont {
                    size: text.size,
                    serif: text.serif,
                    bold: text.bold,
                    italic: text.italic,
                };
                match field {
                    TextSize => font.size = numeric / 100.0,
                    TextBold => font.bold = checked,
                    TextItalic => font.italic = checked,
                    _ => unreachable!("font properties only"),
                }
                text_changes.font = Some(font);
            }
            TextLatex => text_changes.latex = Some(checked),
            Anchor => {
                let placement = if raw.is_empty() {
                    TextPosition::Origin
                } else {
                    let point = c
                        .id(raw)
                        .filter(|&point| c.kind(point) == Kind::Point)
                        .ok_or(invalid)?;
                    if text.is_some_and(|text| text.position.is_screen()) {
                        TextPosition::ScreenPoint(point)
                    } else {
                        TextPosition::Point(point)
                    }
                };
                text_changes.position = Some(placement);
            }
            Absolute => {
                let convert = |(x, y)| {
                    view.map_or((x, y), |view| {
                        if checked {
                            view.world_to_screen(x, y)
                        } else {
                            view.screen_to_world(x, y)
                        }
                    })
                };
                if let Some(text) = text {
                    let placement = match text.position {
                        TextPosition::Point(point) | TextPosition::ScreenPoint(point)
                            if checked =>
                        {
                            TextPosition::ScreenPoint(point)
                        }
                        TextPosition::Point(point) | TextPosition::ScreenPoint(point) => {
                            TextPosition::Point(point)
                        }
                        position => {
                            let (x, y) = convert(coords(position));
                            if checked {
                                TextPosition::Screen { x, y }
                            } else {
                                TextPosition::World { x, y }
                            }
                        }
                    };
                    text_changes.position = Some(placement);
                } else {
                    let xy = c.slider_position(id, &slider);
                    slider.position = Some(convert(xy));
                    slider.absolute = checked;
                    if let Some(view) = view {
                        let scale = if slider.horizontal {
                            view.xscale()
                        } else {
                            view.yscale()
                        };
                        slider.width = if checked {
                            slider.width * scale
                        } else {
                            slider.width / scale
                        };
                    }
                }
            }
            PositionX | PositionY => {
                let at = usize::from(field == PositionY);
                let replace = |(x, y)| if at == 0 { (numeric, y) } else { (x, numeric) };
                if let Some(text) = text {
                    if matches!(
                        text.position,
                        TextPosition::Point(_) | TextPosition::ScreenPoint(_)
                    ) {
                        return Err(invalid);
                    }
                    let (x, y) = replace(coords(text.position));
                    text_changes.position = Some(if text.position.is_screen() {
                        TextPosition::Screen { x, y }
                    } else {
                        TextPosition::World { x, y }
                    });
                } else if let Some(xy) = c.checkbox_position(id) {
                    let (x, y) = replace(xy);
                    checkbox.position = Some([x.round() as i32, y.round() as i32]);
                } else if c.kind(id) == Kind::InputBox {
                    input.position[at] = numeric.round() as i32;
                } else {
                    slider.position = Some(replace(c.slider_position(id, &slider)));
                }
            }
            DecimalPlaces => {
                appearance.input_rounding = Some(NumberRounding::Decimals(integral as u32))
            }
            SignificantFigures => {
                appearance.input_rounding =
                    Some(NumberRounding::SignificantFigures(integral as u32))
            }
            SliderWidth => slider.width = numeric,
            SliderHorizontal => slider.horizontal = checked,
            SliderFixed => slider.fixed = checked,
            SliderKnob => slider.knob_size = Some(numeric),
            SliderStep => {
                return saved(Property::SliderIncrement {
                    label: label_owned(),
                    step: (numeric > 0.0).then_some(numeric),
                });
            }
        }

        if drawing != c.drawing_style(id) {
            return saved(Property::DrawingStyle {
                label: label_owned(),
                style: drawing,
            });
        }
        if &slider != c.slider_style(id) {
            return saved(Property::SliderStyle {
                label: label_owned(),
                style: slider,
            });
        }
        if checkbox != c.checkbox_style(id) {
            return saved(Property::CheckboxStyle {
                label: label_owned(),
                style: checkbox,
            });
        }
        if input != c.input_box_style(id) {
            return saved(Property::InputBoxStyle {
                label: label_owned(),
                style: input,
            });
        }
        if text_changes != TextOverrides::default() {
            return Ok(Change::Text(text_changes));
        }
        saved(Property::Appearance {
            label: label_owned(),
            style: Box::new(appearance),
        })
    }
}
