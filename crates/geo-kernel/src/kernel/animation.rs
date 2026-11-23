// Parts translated from GeoGebra, commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
// source/shared/common/src/main/java/org/geogebra/common/kernel/AnimationManager.java
// source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoNumeric.java
// Copyright (c) GeoGebra GmbH. Licensed under the EUPL 1.2.

//! Slider animation: a playing slider moves through its interval, one step
//! per frame, so that the whole interval takes ten seconds at speed 1.

use super::{Construction, Def, Id};
use crate::expr::math::java_round;
use crate::tol::STANDARD_PRECISION;

/// How an animated slider runs through its interval (the codes of saved
/// files).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationMode {
    /// Back and forth between the ends.
    Oscillating,
    /// Up, jumping back to the lower end.
    Increasing,
    /// Down, jumping back to the upper end.
    Decreasing,
    /// Up once, stopping at the upper end.
    IncreasingOnce,
}

impl AnimationMode {
    /// The mode of a saved code; the reference treats an unknown one as
    /// increasing (`setAnimationType`).
    pub fn from_code(code: i64) -> Self {
        match code {
            0 => Self::Oscillating,
            2 => Self::Decreasing,
            3 => Self::IncreasingOnce,
            _ => Self::Increasing,
        }
    }

    pub fn code(self) -> u8 {
        match self {
            Self::Oscillating => 0,
            Self::Increasing => 1,
            Self::Decreasing => 2,
            Self::IncreasingOnce => 3,
        }
    }
}

/// The animation of a slider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Animation {
    /// The increment the slider's value is rounded to, unless `auto_step`.
    pub step: f64,
    /// Whether the increment follows the interval instead (a new number of
    /// the reference has one): while playing, the pretty number above
    /// `0.0009` times the width and the speed.
    pub auto_step: bool,
    pub speed: f64,
    pub mode: AnimationMode,
    pub playing: bool,
    /// The unrounded position; `NaN` until the next step reads the value.
    position: f64,
    /// `1` or `-1`.
    direction: f64,
    /// The value the last frame set, `NaN` after a value set by hand: a
    /// frame's value is not journaled.
    frame_value: f64,
}

/// The time, in seconds, the interval takes at speed 1.
const STANDARD_ANIMATION_TIME: f64 = 10.0;
/// The frame rate of the first frame and the highest one.
pub const MAX_ANIMATION_FRAME_RATE: f64 = 30.0;
/// The lowest frame rate a frame is computed with.
pub const MIN_ANIMATION_FRAME_RATE: f64 = 6.0;
/// `Kernel.MIN_PRECISION`.
const MIN_PRECISION: f64 = 1e-5;

impl Animation {
    /// A playing animation with speed 1 and an automatic increment; `step`
    /// is the increment stored besides it.
    pub fn playing(step: f64, mode: AnimationMode) -> Self {
        Self {
            step,
            auto_step: true,
            speed: 1.0,
            mode,
            playing: true,
            position: f64::NAN,
            direction: if mode == AnimationMode::Decreasing {
                -1.0
            } else {
                1.0
            },
            frame_value: f64::NAN,
        }
    }
}

/// `MyMath.nextPrettyNumber(t, 0)`: 2, 5 or 10 times the power of ten
/// below `t`.
pub(super) fn next_pretty_number(t: f64) -> f64 {
    if t < 0.0 {
        return 1.0;
    }
    let pot = 10f64.powf(t.log10().floor());
    let n = t / pot;
    if n > 5.0 {
        10.0 * pot
    } else if n > 2.0 {
        5.0 * pot
    } else {
        2.0 * pot
    }
}

/// `GeoNumeric.AUTO_STEP_MUL`.
pub(super) const AUTO_STEP_MUL: f64 = 0.0009;

impl Animation {
    /// A saved animation: the increment is automatic without a step.
    pub fn saved(step: Option<f64>, speed: f64, mode: AnimationMode, playing: bool) -> Self {
        Self {
            step: step.unwrap_or(0.01),
            auto_step: step.is_none(),
            speed,
            playing,
            ..Self::playing(0.01, mode)
        }
    }
}

/// `Kernel.roundToScale`.
fn round_to_scale(x: f64, scale: f64) -> f64 {
    if scale == 1.0 {
        java_round(x) as f64
    } else {
        java_round(x / scale) as f64 * scale
    }
}

/// `DoubleUtil.checkDecimalFraction`: a value within `1e-13` of five
/// decimals is those five decimals.
pub(super) fn check_decimal_fraction(v: f64) -> f64 {
    const INV_MIN_PRECISION: f64 = 1e5;
    let fraction = v * INV_MIN_PRECISION;
    let rounded = java_round(fraction) as f64;
    let eps = STANDARD_PRECISION;
    if fraction == rounded || (fraction - eps < rounded && rounded < fraction + eps) {
        rounded / INV_MIN_PRECISION
    } else {
        v
    }
}

impl Construction {
    /// The animation of the slider `id`, if it has one.
    pub fn animation(&self, id: Id) -> Option<&Animation> {
        self.animations.get(&id)
    }

    /// Whether the animation runs: the timer is on and a slider plays.
    pub fn is_animating(&self) -> bool {
        self.animation_running && self.animations.values().any(|a| a.playing)
    }

    /// Gives the slider `id` a playing animation (`setAnimating(true)`) and
    /// starts the timer (`AnimationManager.startAnimation`).
    pub(crate) fn start_animation(&mut self, id: Id, animation: Animation) {
        self.animations.insert(id, animation);
        self.animation_running = true;
    }

    /// Restores the animation of the slider `id` from a saved file; the
    /// timer runs when the file says so (`<startAnimation>`).
    pub fn restore_animation(&mut self, id: Id, animation: Animation, running: bool) {
        self.animations.insert(id, animation);
        if running && animation.playing {
            self.animation_running = true;
        }
    }

    /// Takes over the animations of `old` after a rebuild of the same
    /// journal: every slider that animates in both keeps its state and
    /// position, and the value its last frame set, which frames do not
    /// journal; the timer runs as it ran. The reference rebuilds from
    /// a file that holds the value and the playing flag.
    pub fn carry_animations_from(&mut self, old: &Construction) {
        let mut carried = std::collections::BTreeSet::new();
        for (&old_id, animation) in &old.animations {
            let Some(id) = self.id(old.label(old_id)) else {
                continue;
            };
            if !self.animations.contains_key(&id) || !matches!(self.def(id), Def::Number(_)) {
                continue;
            }
            // only a value a frame set: one set by hand is in the journal,
            // which undo and redo follow
            if let crate::Value::Number(v) = old.value(old_id) {
                if v == animation.frame_value {
                    let _ = self.update_number(id, v, false);
                }
            }
            self.animations.insert(id, *animation);
            carried.insert(id);
        }
        // A slider the old construction did not animate was started by its
        // command during the rebuild (a redone SlowPlot): its timer keeps
        // running even when the carried sliders were paused.
        let started = self
            .animations
            .iter()
            .any(|(id, a)| a.playing && !carried.contains(id));
        self.animation_running = old.animation_running || (self.animation_running && started);
    }

    /// Whether a slider plays, the timer running or paused (the reference
    /// shows its play button then).
    pub fn has_animations(&self) -> bool {
        self.animations.values().any(|a| a.playing)
    }

    /// Whether the timer runs, playing sliders or not.
    pub fn animation_running(&self) -> bool {
        self.animation_running
    }

    /// Starts or stops the timer (`AnimationManager.startAnimation` and
    /// `stopAnimation`); it starts only with a playing slider.
    pub fn set_animation_running(&mut self, running: bool) {
        self.animation_running = running && self.animations.values().any(|a| a.playing);
    }

    /// Asks the application to hide `id`, as a command that hides an input
    /// does (`setEuclidianVisible(false)`); see
    /// [`Construction::take_hidden_requests`].
    pub(crate) fn request_hidden(&mut self, id: Id) {
        self.hidden_requests.push(id);
    }

    /// The objects commands asked to hide since the last call, oldest first.
    /// Visibility belongs to the application, which applies the requests of
    /// the lines it runs.
    pub fn take_hidden_requests(&mut self) -> Vec<Id> {
        std::mem::take(&mut self.hidden_requests)
    }

    /// Starts or stops the animation of `id`; `false` when it has none.
    pub fn set_animation_playing(&mut self, id: Id, playing: bool) -> bool {
        match self.animations.get_mut(&id) {
            Some(a) => {
                a.playing = playing;
                a.position = f64::NAN;
                true
            }
            None => false,
        }
    }

    /// Where the animation of `id` goes on from.
    pub(super) fn set_animation_position(&mut self, id: Id, position: f64) {
        if let Some(a) = self.animations.get_mut(&id) {
            a.position = position;
        }
    }

    /// A value set by hand is where the animation goes on from.
    pub(super) fn reset_animation_position(&mut self, id: Id) {
        if let Some(a) = self.animations.get_mut(&id) {
            a.position = f64::NAN;
            a.frame_value = f64::NAN;
        }
    }

    /// One frame of every playing slider at `frame_rate` frames per second
    /// (clamped to the reference's range), the newest first, then one
    /// update of everything that depends on the sliders that changed
    /// (`AnimationManager.sliderStep`). Returns whether a value changed.
    pub fn animate(&mut self, frame_rate: f64) -> bool {
        if !self.animation_running {
            return false;
        }
        let frame_rate = if frame_rate.is_nan() {
            MAX_ANIMATION_FRAME_RATE
        } else {
            frame_rate.clamp(MIN_ANIMATION_FRAME_RATE, MAX_ANIMATION_FRAME_RATE)
        };
        let playing: Vec<Id> = self
            .animations
            .iter()
            .rev()
            .filter(|(_, a)| a.playing)
            .map(|(&id, _)| id)
            .collect();
        let mut changed = false;
        for id in playing {
            if let Some(value) = self.animation_step(id, frame_rate) {
                // `setValue(value, false)`: the position stays unrounded
                let position = self.animations.get(&id).map(|a| a.position);
                // already kept in the interval, as `setValue` does it once
                changed |= self.update_number(id, value, false).is_ok();
                if let (Some(a), Some(position)) = (self.animations.get_mut(&id), position) {
                    a.position = position;
                    a.frame_value = value;
                }
            }
        }
        // the timer stops with the last playing slider
        if !self.animations.values().any(|a| a.playing) {
            self.animation_running = false;
        }
        changed
    }

    /// `GeoNumeric.doAnimationStep`: the new value of the slider, `None`
    /// when it keeps its value.
    fn animation_step(&mut self, id: Id, frame_rate: f64) -> Option<f64> {
        let [Some(min), Some(max)] = self.slider_bounds(id) else {
            return None;
        };
        // an undefined slider (a reversed interval) reads as NaN
        let old = match (self.defs.get(id.0), self.values[id.0]) {
            (Some(Def::Number(_)), crate::Value::Number(v)) => v,
            (Some(Def::Number(_)), _) => f64::NAN,
            _ => return None,
        };
        let a = self.animations.get_mut(&id)?;
        let width = max - min;
        let speed = if a.speed.is_nan() {
            0.0
        } else {
            a.speed.clamp(-100.0, 100.0)
        };
        let step = width * speed * a.direction / (STANDARD_ANIMATION_TIME * frame_rate);
        if a.position.is_nan() {
            a.position = old;
        }
        a.position += step;
        match a.mode {
            AnimationMode::Increasing | AnimationMode::Decreasing => {
                if a.position > max {
                    a.position -= width;
                } else if a.position < min {
                    a.position += width;
                }
            }
            AnimationMode::IncreasingOnce => {
                if a.position > max {
                    a.playing = false;
                    a.position = f64::NAN;
                    let changed = max != old;
                    return changed.then(|| self.clamp_slider(id, max));
                } else if a.position < min {
                    a.playing = false;
                    a.position = f64::NAN;
                    return Some(self.clamp_slider(id, min));
                }
            }
            AnimationMode::Oscillating => {
                if a.position >= max {
                    a.position = max;
                    a.direction = -a.direction;
                } else if a.position <= min {
                    a.position = min;
                    a.direction = -a.direction;
                }
            }
        }
        // `getAnimationStep` while animating
        let increment = if a.auto_step {
            next_pretty_number(width * speed * AUTO_STEP_MUL)
        } else {
            a.step
        };
        let mut value = min + round_to_scale(a.position - min, increment);
        if increment > MIN_PRECISION {
            value = check_decimal_fraction(value);
        }
        // `setValue` keeps the value in the interval with ordered tests,
        // which a reversed interval also passes through
        let value = self.clamp_slider(id, value);
        (value != old).then_some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_follows_the_slider_increment() {
        assert_eq!(round_to_scale(0.0033, 0.01), 0.0);
        assert_eq!(round_to_scale(0.0067, 0.01), 0.01);
        assert_eq!(check_decimal_fraction(0.30000000000000004), 0.3);
        assert_eq!(check_decimal_fraction(0.123456), 0.123456);
        assert_eq!(next_pretty_number(0.0009), 0.001);
        assert_eq!(next_pretty_number(0.003), 0.005);
    }

    #[test]
    fn a_once_animation_stops_at_the_upper_end() {
        let mut c = Construction::new();
        let a = c.add_slider("a", 0.0, 1.0).unwrap();
        c.start_animation(a, Animation::playing(0.01, AnimationMode::IncreasingOnce));
        let mut frames = 0;
        while c.is_animating() && frames < 1000 {
            c.animate(30.0);
            frames += 1;
        }
        assert_eq!(c.value(a), crate::Value::Number(1.0));
        assert!(frames < 400, "{frames}");
    }
}
