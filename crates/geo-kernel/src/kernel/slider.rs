//! Bounds of free numeric sliders, including expressions loaded after the construction.

use super::{Construction, Def, Error, Id, Value};
use crate::Kind;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug)]
pub(super) enum Bound {
    Literal(f64),
    Object(Id),
}

impl Bound {
    fn read(self, c: &Construction) -> Option<f64> {
        match self {
            Self::Literal(v) => Some(v),
            Self::Object(id) => match c.value(id) {
                Value::Number(v) if v.is_finite() => Some(v),
                _ => None,
            },
        }
    }

    pub(super) fn dependency(self) -> Option<Id> {
        match self {
            Self::Object(id) if super::axis::get(id).is_none() => Some(id),
            _ => None,
        }
    }

    pub(super) fn exists_before(self, len: usize) -> bool {
        match self {
            Self::Object(id) => id.index() < len || super::axis::get(id).is_some(),
            Self::Literal(_) => true,
        }
    }
}

impl Construction {
    /// Active lower and upper bounds. An omitted or undefined endpoint is unbounded.
    pub fn slider_bounds(&self, id: Id) -> [Option<f64>; 2] {
        if let Some(bounds) = self.saved_slider_bounds.get(&id) {
            return bounds.map(|bound| bound.and_then(|bound| bound.read(self)));
        }
        self.sliders
            .binary_search_by_key(&id.index(), |&(owner, _, _)| owner)
            .ok()
            .map_or([None; 2], |slot| {
                let (_, min, max) = self.sliders[slot];
                [Some(min), Some(max)]
            })
    }

    /// The saved endpoint values or expressions used when writing a slider.
    pub fn slider_bound_sources(&self, id: Id) -> [Option<String>; 2] {
        self.saved_slider_bounds.get(&id).map_or_else(
            || {
                self.slider_bounds(id)
                    .map(|bound| bound.map(|v| v.to_string()))
            },
            |bounds| {
                bounds.map(|bound| {
                    bound.map(|bound| match bound {
                        Bound::Literal(v) => v.to_string(),
                        Bound::Object(id) => {
                            let label = self.label(id);
                            if label.starts_with('\0') {
                                self.construction_source(id).unwrap_or(label).to_owned()
                            } else {
                                label.to_owned()
                            }
                        }
                    })
                })
            },
        )
    }

    /// Whether both endpoints of the slider `id` exist, whatever their values
    /// (`OptimizerNumeric.hasBounds` checks the endpoint objects).
    pub fn has_slider_bounds(&self, id: Id) -> bool {
        if let Some(bounds) = self.saved_slider_bounds.get(&id) {
            return bounds.iter().all(Option::is_some);
        }
        self.sliders
            .binary_search_by_key(&id.index(), |&(owner, _, _)| owner)
            .is_ok()
    }

    fn remember_slider_slot(&mut self, id: Id) {
        let next = self.slider_default_slots.len();
        self.slider_default_slots.entry(id).or_insert(next);
    }

    /// The finite interval used when sampling a locus on a slider.
    pub fn slider_interval(&self, id: Id) -> Option<(f64, f64)> {
        let [min, max] = self.slider_bounds(id);
        Some((min?, max?))
    }

    /// Writes the value of the free number `id`, as a drag or a slider
    /// sample does (`GeoNumeric.setValue`).
    pub(crate) fn set_free_number(&mut self, id: Id, value: f64) {
        self.defs[id.0] = Def::Number(value);
        self.values[id.0] = Value::Number(value);
    }

    pub(super) fn clamp_slider(&self, id: Id, value: f64) -> f64 {
        clamp(value, self.slider_bounds(id))
    }

    /// Resolves a change to a live endpoint before the slider's children run.
    pub(super) fn update_slider_interval(&mut self, id: Id) -> bool {
        let Some((min, max)) = self.slider_interval(id) else {
            return false;
        };
        let old = self.value(id);
        let next = if min <= max {
            let current = match old {
                Value::Number(value) => value,
                _ => 1.0,
            };
            Value::Number(clamp(current, [Some(min), Some(max)]))
        } else {
            Value::Undefined
        };
        if next.identical(&old) {
            return false;
        }
        self.last_update_requires_ordered_replay = true;
        crate::expr::forget_decimal(self, id);
        self.construction_sources.remove(&id);
        if let Value::Number(value) = next {
            self.set_free_number(id, value);
            self.reset_animation_position(id);
        } else {
            self.values[id.index()] = next;
        }
        true
    }

    pub(crate) fn restore_slider_bounds(
        &mut self,
        id: Id,
        bounds: [Option<&str>; 2],
    ) -> Result<(), String> {
        let Some(&Def::Number(value)) = self.defs.get(id.index()) else {
            return Err("slider bounds need a free number".into());
        };
        let checkpoint = self.checkpoint();
        let mut restored = [None; 2];
        for (slot, source) in restored.iter_mut().zip(bounds) {
            if let Some(source) = source {
                match self.slider_bound(id, source) {
                    Ok(bound) => *slot = Some(bound),
                    Err(error) => {
                        self.discard_since(checkpoint);
                        return Err(error);
                    }
                }
            }
        }
        if restored.iter().all(Option::is_none) {
            return Err("slider has no saved endpoints".into());
        }
        let old_bounds = self.slider_bounds(id);
        let old_value = match self.values[id.index()] {
            Value::Number(v) => v,
            _ => f64::NAN,
        };
        let parents =
            |bounds: [Option<Bound>; 2]| bounds.into_iter().flatten().filter_map(Bound::dependency);
        let old = self
            .saved_slider_bounds
            .get(&id)
            .copied()
            .unwrap_or_default();
        let order = if parents(restored)
            .any(|parent| self.evaluation_rank(parent) >= self.evaluation_rank(id))
        {
            let mut reads: Vec<_> = parents(restored).collect();
            reads.sort_unstable();
            reads.dedup();
            match self.replacement_order(&[(id, reads)].into(), &Default::default(), self.len()) {
                Ok(order) => Some(order),
                Err(error) => {
                    self.discard_since(checkpoint);
                    return Err(error.into());
                }
            }
        } else {
            None
        };
        // A bound notifies the number when it changes. Keep it out of
        // reads(): the slider is still a free input to algorithms such as
        // Minimize, which only recompute when the slider's value changes.
        for parent in parents(old) {
            self.children[parent.index()].remove(id);
        }
        for parent in parents(restored) {
            self.children[parent.index()].insert(id);
        }
        if let Some(order) = order {
            self.evaluation_order = order;
        }
        self.saved_slider_bounds.insert(id, restored);
        if restored.iter().all(Option::is_some) {
            self.remember_slider_slot(id);
        }
        let [min, max] = self.slider_bounds(id);
        // An animation goes on from the value the bounds set last: the
        // reference sets the lower end, then the upper one, and each sets
        // the number again in an ordered interval
        // (`GeoNumeric.resolveMinMax`) or leaves it undefined in a reversed
        // one, without touching the animation.
        let mut position = None;
        if let ([Some(_), Some(old_max)], Some(min), Some(max)) = (old_bounds, min, max) {
            let mut value = old_value;
            for [lo, hi] in [[min, old_max], [min, max]] {
                if lo <= hi {
                    let from = if value.is_nan() { 1.0 } else { value };
                    value = clamp(from, [Some(lo), Some(hi)]);
                    position = Some(value);
                } else {
                    value = f64::NAN;
                }
            }
        }
        match position {
            Some(position) => self.set_animation_position(id, position),
            None => self.reset_animation_position(id),
        }
        if min.zip(max).is_some_and(|(min, max)| min > max) {
            self.values[id.index()] = Value::Undefined;
            self.propagate(id);
        } else if clamp(value, [min, max]).to_bits() != value.to_bits() {
            self.set_number(id, value).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn slider_bound(&mut self, owner: Id, source: &str) -> Result<Bound, String> {
        if let Some(v) = crate::expr::plain_number(source).filter(|v| v.is_finite()) {
            return Ok(Bound::Literal(v));
        }
        let id = if let Some(id) = self.id(source) {
            id
        } else {
            let label = format!("\0bound{}", self.len());
            if !crate::expr::argument_object(self, source, &label, Kind::Number) {
                return Err("endpoint is not a supported number expression".into());
            }
            let id = self.id(&label).expect("the bound was just added");
            self.remember_construction_source(id, source);
            id
        };
        if self.kind(id) != Kind::Number {
            return Err("endpoint is not a number".into());
        }
        let mut remaining = super::MAX_REDEFINITION_EDGES;
        let mut seen = HashSet::new();
        let mut parents = vec![id];
        while let Some(id) = parents.pop() {
            remaining = remaining
                .checked_sub(1)
                .ok_or("the slider endpoint exceeds the dependency budget")?;
            if id == owner {
                return Err("slider endpoint is circular".into());
            }
            if seen.insert(id) {
                parents.extend(self.reads(id));
                if let Some(bounds) = self.saved_slider_bounds.get(&id) {
                    parents.extend(
                        bounds
                            .iter()
                            .flatten()
                            .filter_map(|bound| bound.dependency()),
                    );
                }
            }
        }
        Ok(Bound::Object(id))
    }

    /// Adds a slider, `Slider(min, max)` (`CmdSlider`): a free number with an
    /// interval, which `Locus(Q, slider)` runs through and which
    /// [`Construction::set_number`] keeps its value in. A new number is 0,
    /// put in the interval (`GeoNumeric.setIntervalMin` and `Max`).
    pub fn add_slider(&mut self, label: &str, min: f64, max: f64) -> Result<Id, Error> {
        if !(min.is_finite() && max.is_finite()) {
            return Err(Error::NotFinite);
        }
        let reversed = min > max;
        let value = if reversed {
            0.0
        } else {
            0.0_f64.clamp(min, max)
        };
        self.add_saved_slider(label, value, min, max)
    }

    /// Restores an existing slider with its saved value and interval.
    pub fn add_saved_slider(
        &mut self,
        label: &str,
        value: f64,
        min: f64,
        max: f64,
    ) -> Result<Id, Error> {
        if !(value.is_finite() && min.is_finite() && max.is_finite()) {
            return Err(Error::NotFinite);
        }
        let id = self.add(
            label,
            Def::Number(if min <= max {
                clamp(value, [Some(min), Some(max)])
            } else {
                value
            }),
        )?;
        self.sliders.push((id.0, min, max));
        self.remember_slider_slot(id);
        if min > max {
            self.values[id.0] = Value::Undefined;
        }
        Ok(id)
    }
}

fn clamp(value: f64, [min, max]: [Option<f64>; 2]) -> f64 {
    if let Some(min) = min.filter(|&min| value < min) {
        min
    } else if let Some(max) = max.filter(|&max| value > max) {
        max
    } else {
        value
    }
}
