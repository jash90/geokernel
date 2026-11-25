//! Optional presentation overrides and numeric expressions owned by color channels.

use super::{Construction, Id};
use crate::expr;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub caption: Option<String>,
    pub label_mode: Option<i32>,
    pub fixed: Option<bool>,
    pub auxiliary: Option<bool>,
    pub selection_allowed: Option<bool>,
    pub tooltip: Option<i32>,
    pub point_size: Option<i32>,
    pub point_style: Option<i32>,
    pub line_type: Option<i32>,
    pub line_opacity: Option<i32>,
    pub fill_opacity: Option<f64>,
    pub text_background: Option<[u8; 4]>,
    pub angle_radius: Option<i32>,
    pub right_angle: Option<bool>,
    pub label_offset: [Option<i32>; 2],
    pub segment_ends: [Option<u8>; 2],
    pub vector_arrow: Option<bool>,
    pub decoration: Option<u8>,
    pub slider_track_color: Option<[u8; 3]>,
    pub image_background: Option<bool>,
    pub image_interpolate: Option<bool>,
    pub playback: Option<Playback>,
    pub input_rounding: Option<super::control::NumberRounding>,
    pub edited_layer: bool,
    pub edited_thickness: bool,
    pub edited_slider: bool,
    pub edited_checkbox: bool,
    pub dynamic: DynamicColor,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DynamicColor {
    pub edited: bool,
    pub space: u8,
    pub channels: [Option<String>; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Playback {
    pub speed: f64,
    pub mode: u8,
    pub playing: bool,
}

pub(super) enum Channel {
    Literal(f64),
    Object(Id),
    Expression(Box<expr::Entry>),
}

impl DynamicColor {
    /// The object's static color in the selected coordinate system.
    pub fn components(&self, rgb: [u8; 3]) -> [f64; 3] {
        let [r, g, b] = rgb.map(|v| f64::from(v) / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let chroma = max - min;
        let light = (max + min) * 0.5;
        let hue = if chroma == 0.0 {
            0.0
        } else if max == r {
            ((g - b) / chroma).rem_euclid(6.0) / 6.0
        } else if max == g {
            ((b - r) / chroma + 2.0) / 6.0
        } else {
            ((r - g) / chroma + 4.0) / 6.0
        };
        match self.space {
            1 => [hue, if max == 0.0 { 0.0 } else { chroma / max }, max],
            2 => [
                hue,
                if chroma == 0.0 {
                    0.0
                } else {
                    chroma / (1.0 - (2.0 * light - 1.0).abs())
                },
                light,
            ],
            _ => [r, g, b],
        }
    }
}

static DEFAULT: Appearance = Appearance {
    caption: None,
    label_mode: None,
    fixed: None,
    auxiliary: None,
    selection_allowed: None,
    tooltip: None,
    point_size: None,
    point_style: None,
    line_type: None,
    line_opacity: None,
    fill_opacity: None,
    text_background: None,
    angle_radius: None,
    right_angle: None,
    label_offset: [None; 2],
    segment_ends: [None; 2],
    vector_arrow: None,
    decoration: None,
    slider_track_color: None,
    image_background: None,
    image_interpolate: None,
    playback: None,
    input_rounding: None,
    edited_layer: false,
    edited_thickness: false,
    edited_slider: false,
    edited_checkbox: false,
    dynamic: DynamicColor {
        edited: false,
        space: 0,
        channels: [None, None, None, None],
    },
};

impl Construction {
    pub fn angle_radius(&self, id: Id, angle: super::angle::AngleGeometry, scale: f64) -> f64 {
        self.appearance(id).angle_radius.map_or_else(
            || angle.radius(scale),
            |cap| angle.radius_with_cap(scale, f64::from(cap.clamp(10, 100))),
        )
    }

    pub fn appearance(&self, id: Id) -> &Appearance {
        self.appearances.get(&id).unwrap_or(&DEFAULT)
    }

    pub fn appearances(&self) -> impl Iterator<Item = (Id, &Appearance)> {
        self.appearances.iter().map(|(&id, style)| (id, style))
    }

    pub fn set_appearance(&mut self, id: Id, style: Appearance) -> Result<(), String> {
        self.set_appearance_inner(id, style, false)
    }

    /// When replaying after deletion, an invalid color source is discarded
    /// while the other presentation fields are retained.
    pub(crate) fn restore_appearance(&mut self, id: Id, style: Appearance) {
        let _ = self.set_appearance_inner(id, style, true);
    }

    fn set_appearance_inner(
        &mut self,
        id: Id,
        mut style: Appearance,
        tolerate_missing: bool,
    ) -> Result<(), String> {
        if !self.exists(id) {
            return Err("object is missing".into());
        }
        let mut changed = [const { None }; 4];
        for (i, update) in changed.iter_mut().enumerate() {
            if style.dynamic.channels[i] == self.appearance(id).dynamic.channels[i] {
                continue;
            }
            let Some(source) = style.dynamic.channels[i].as_deref() else {
                *update = Some(None);
                continue;
            };
            if let Some(channel) = self.prepare_color_channel(source) {
                *update = Some(Some(channel));
            } else if tolerate_missing {
                style.dynamic.channels[i] = None;
                *update = Some(None);
            } else {
                return Err("invalid numeric color channel".into());
            }
        }
        if changed.iter().any(Option::is_some) {
            let channels = self.appearance_channels.entry(id).or_default();
            for (channel, update) in channels.iter_mut().zip(changed) {
                if let Some(update) = update {
                    *channel = update;
                }
            }
            if channels.iter().all(Option::is_none) {
                self.appearance_channels.remove(&id);
            }
        }
        if let Some(caption) = &style.caption {
            if let Ok(index) = self.controls.binary_search_by_key(&id, |(id, _)| *id) {
                self.controls[index].1.caption.clone_from(caption);
            }
        }
        if let Some(image) = self.images.instances.get_mut(&id) {
            image.style.background = style.image_background.unwrap_or(image.style.background);
            image.style.interpolate = style.image_interpolate.unwrap_or(image.style.interpolate);
            if let Some(alpha) = style.fill_opacity {
                image.style.alpha = alpha.clamp(0.0, 1.0) as f32;
            }
        }
        if style.playback != self.appearance(id).playback {
            if let Some(playback) = style.playback {
                use super::animation::{Animation, AnimationMode};
                let step = self.slider_style(id).step;
                let mode = AnimationMode::from_code(i64::from(playback.mode));
                self.restore_animation(
                    id,
                    Animation::saved(step, playback.speed, mode, playback.playing),
                    playback.playing,
                );
                if !self.has_animations() {
                    self.set_animation_running(false);
                }
            }
        }
        self.appearances.insert(id, style);
        Ok(())
    }

    fn prepare_color_channel(&self, source: &str) -> Option<Channel> {
        if let Ok(value) = source.parse::<f64>() {
            return value.is_finite().then_some(Channel::Literal(value));
        }
        if let Some(id) = self.id(source) {
            return self.kind(id).is_number().then_some(Channel::Object(id));
        }
        expr::presentation_number(self, source).map(Channel::Expression)
    }

    fn color_channel(&self, channel: Option<&Channel>) -> Option<f64> {
        let value = match channel? {
            Channel::Literal(value) => *value,
            Channel::Object(id) => self.number(*id)?,
            Channel::Expression(entry) => expr::presentation_value(self, entry)?,
        };
        value.is_finite().then(|| {
            let x = value.rem_euclid(2.0);
            x.min(2.0 - x)
        })
    }

    pub fn dynamic_fill_opacity(&self, id: Id) -> Option<f64> {
        self.color_channel(self.appearance_channels.get(&id)?[3].as_ref())
    }

    pub fn dynamic_color(&self, id: Id, rgb: [u8; 3]) -> Option<[u8; 3]> {
        let channels = self.appearance_channels.get(&id)?;
        let space = self.appearance(id).dynamic.space;
        if channels[..3].iter().all(Option::is_none) {
            return None;
        }
        let base = self.appearance(id).dynamic.components(rgb);
        let [x, y, z] =
            std::array::from_fn(|i| self.color_channel(channels[i].as_ref()).unwrap_or(base[i]));
        let rgb = match space {
            1 | 2 => {
                let chroma = if space == 1 {
                    z * y
                } else {
                    (1.0 - (2.0 * z - 1.0).abs()) * y
                };
                let hue = (x * 6.0).rem_euclid(6.0);
                let mid = chroma * (1.0 - (hue.rem_euclid(2.0) - 1.0).abs());
                let rgb = match hue as u8 {
                    0 => [chroma, mid, 0.0],
                    1 => [mid, chroma, 0.0],
                    2 => [0.0, chroma, mid],
                    3 => [0.0, mid, chroma],
                    4 => [mid, 0.0, chroma],
                    _ => [chroma, 0.0, mid],
                };
                let offset = if space == 1 {
                    z - chroma
                } else {
                    z - chroma * 0.5
                };
                rgb.map(|c| c + offset)
            }
            _ => [x, y, z],
        };
        Some(rgb.map(|v| (255.0 * v).round() as u8))
    }
}
