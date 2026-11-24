//! Bounds of free numeric sliders, including expressions.

use super::{Construction, Def, Error, Id, Value};
use crate::Kind;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug)]
pub(super) enum Bound {
    Literal(f64),
    Object(Id),
    /// An endpoint that reads the slider is evaluated without a listener edge.
    Passive(Id),
}

impl Bound {
    fn read(self, c: &Construction) -> Option<f64> {
        match self {
            Self::Literal(v) => v.is_finite().then_some(v),
            Self::Object(id) | Self::Passive(id) => match c.value(id) {
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
            Self::Object(id) | Self::Passive(id) => {
                id.index() < len || super::axis::get(id).is_some()
            }
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
                        Bound::Object(id) | Bound::Passive(id) => {
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

    pub(super) fn slider_upper_bound(&self, id: Id) -> Option<Bound> {
        match self.saved_slider_bounds.get(&id) {
            Some(bounds) => bounds[1],
            None => self.slider_bounds(id)[1].map(Bound::Literal),
        }
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

    pub(super) fn remember_slider_slot(&mut self, id: Id) {
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
        self.values[id.0] = if value.is_nan() {
            Value::Undefined
        } else {
            Value::Number(value)
        };
    }

    pub(super) fn clamp_slider(&self, id: Id, value: f64) -> f64 {
        clamp(value, self.slider_bounds(id))
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
                match self.slider_bound(Some(id), source) {
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
        let old_parents: Vec<_> = self.slider_bound_parents(id).collect();
        let parents: Vec<_> = bound_parents(restored).collect();
        if parents
            .iter()
            .any(|&parent| self.dependency_needs_reorder(parent, id))
        {
            let result = self.locus_order_inputs().and_then(|loci| {
                self.replacement_order(&[(id, parents.clone())].into(), &loci, self.len())
            });
            match result {
                Ok(order) => {
                    self.install_replacement_order(order);
                }
                Err(error) => {
                    self.discard_since(checkpoint);
                    return Err(error.into());
                }
            }
        }
        for parent in old_parents {
            self.children[parent.0].remove(id);
        }
        for parent in parents {
            self.children[parent.0].insert(id);
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

    pub(super) fn install_redefined_slider_bounds(
        &mut self,
        id: Id,
        bounds: [Option<Bound>; 2],
        old_max: Option<Bound>,
    ) -> bool {
        let mut requires_replay = false;
        // CmdSlider sets the lower endpoint first. That can make the
        // slider undefined before the upper endpoint brings it back.
        if old_max.is_some() {
            self.saved_slider_bounds.insert(id, [bounds[0], old_max]);
            if self.update_slider_bounds(id) {
                self.propagate(id);
                requires_replay = self.last_update_requires_replay;
            }
        }
        self.saved_slider_bounds.insert(id, bounds);
        self.update_slider_bounds(id);
        requires_replay
    }

    /// Explicit step and speed expressions from the slider command.
    pub fn slider_animation_options(&self, id: Id) -> [Option<&str>; 2] {
        self.slider_animation_options
            .get(&id)
            .map_or([None; 2], |options| {
                options.each_ref().map(|arg| arg.as_deref())
            })
    }

    pub(crate) fn set_slider_animation_options(&mut self, id: Id, args: &[&str]) {
        if !args.is_empty() {
            let options = self.slider_animation_options.entry(id).or_default();
            for (option, &arg) in options.iter_mut().zip(args) {
                *option = Some(arg.to_owned());
            }
        }
    }

    fn slider_bound(&mut self, owner: Option<Id>, source: &str) -> Result<Bound, String> {
        if matches!(crate::script::ungroup(source), "?" | "NaN") {
            return Ok(Bound::Literal(f64::NAN));
        }
        if let Some(v) = crate::expr::plain_number(source).filter(|v| v.is_finite()) {
            return Ok(Bound::Literal(v));
        }
        if let Some(owner) = owner.filter(|&id| source == self.label(id)) {
            return Ok(Bound::Passive(owner));
        }
        let id = if let Some(id) = self.resolve_label(source) {
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
        let Some(owner) = owner else {
            return Ok(Bound::Object(id));
        };
        let mut seen = HashSet::new();
        let mut parents = vec![id];
        let mut remaining = super::MAX_REDEFINITION_EDGES;
        while let Some(parent) = parents.pop() {
            if parent == owner {
                // The live expression can be read during a set. A listener
                // edge here would make a cycle in the construction graph.
                return if self.replacement_target == Some(owner) {
                    Ok(Bound::Passive(id))
                } else {
                    Err("slider endpoint is circular".into())
                };
            }
            if seen.insert(parent) {
                let reads = self.reads(parent);
                let endpoint_parents = self.slider_bound_parents(parent);
                remaining = remaining
                    .checked_sub(1 + reads.len())
                    .ok_or("slider endpoint exceeds the dependency budget")?;
                parents.extend(reads);
                for endpoint in endpoint_parents {
                    remaining = remaining
                        .checked_sub(1)
                        .ok_or("slider endpoint exceeds the dependency budget")?;
                    parents.push(endpoint);
                }
            }
        }
        Ok(Bound::Object(id))
    }

    /// Adds a numeric slider with live endpoint expressions (`CmdSlider`).
    /// The command transaction rolls back hidden endpoint objects on error.
    pub(crate) fn add_expression_slider(
        &mut self,
        label: &str,
        [min, max]: [&str; 2],
    ) -> Result<Id, String> {
        let owner = self.replacement_target;
        let bounds = [
            self.slider_bound(owner, min)?,
            self.slider_bound(owner, max)?,
        ];
        let id = self
            .add(label, Def::Number(0.0))
            .map_err(|e| e.to_string())?;
        self.saved_slider_bounds.insert(id, bounds.map(Some));
        for parent in self.slider_bound_parents(id) {
            if let Some(children) = self.children.get_mut(parent.0) {
                children.insert(id);
            }
        }
        self.remember_slider_slot(id);
        self.update_slider_bounds(id);
        Ok(id)
    }

    pub(super) fn slider_bound_parents(&self, id: Id) -> impl Iterator<Item = Id> + use<> {
        bound_parents(
            self.saved_slider_bounds
                .get(&id)
                .copied()
                .unwrap_or([None; 2]),
        )
    }

    /// `GeoNumeric.resolveMinMax` cascades only when the slider value changes.
    pub(super) fn update_slider_bounds(&mut self, id: Id) -> bool {
        let [Some(min), Some(max)] = self.slider_bounds(id) else {
            return false;
        };
        let old = self.values[id.0];
        let value = if min <= max {
            Value::Number(self.number(id).unwrap_or(1.0).clamp(min, max))
        } else {
            Value::Undefined
        };
        if old.identical(&value) {
            return false;
        }
        crate::expr::forget_decimal(self, id);
        self.values[id.0] = value;
        if let Value::Number(value) = value {
            self.defs[id.0] = Def::Number(value);
            self.set_animation_position(id, value);
        }
        true
    }

    /// Adds a slider, `Slider(min, max)` (`CmdSlider`): a free number with an
    /// interval, which `Locus(Q, slider)` runs through and which
    /// [`Construction::set_number`] keeps its value in. A new number is 0,
    /// put in the interval (`GeoNumeric.setIntervalMin` and `Max`).
    pub fn add_slider(&mut self, label: &str, min: f64, max: f64) -> Result<Id, Error> {
        if min.is_nan() || max.is_nan() {
            if min.is_infinite() || max.is_infinite() {
                return Err(Error::NotFinite);
            }
            // GeoNumeric.resolveMinMax leaves the initial value in place
            // when either endpoint is undefined.
            let id = self.add(label, Def::Number(0.0))?;
            self.saved_slider_bounds
                .insert(id, [Some(Bound::Literal(min)), Some(Bound::Literal(max))]);
            self.remember_slider_slot(id);
            return Ok(id);
        }
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

fn bound_parents(bounds: [Option<Bound>; 2]) -> impl Iterator<Item = Id> {
    bounds.into_iter().flatten().filter_map(Bound::dependency)
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

#[cfg(test)]
mod tests {
    use crate::{script, Value};

    #[test]
    fn redefining_a_bound_source_resolves_each_endpoint_in_order() {
        let mut c =
            crate::script::load("u = -4\nlo = u + 0\nhi = u + 3\na = Slider(lo, hi)").unwrap();
        crate::script::exec(&mut c, "u = -0.5").unwrap();
        assert_eq!(c.number(c.id("a").unwrap()), Some(1.0));
    }

    #[test]
    fn an_omitted_step_is_frozen_after_both_bounds_cascade() {
        let mut c = crate::script::load("h = 0.2\na = Slider(0, 6, h, 2)").unwrap();
        let a = c.id("a").unwrap();
        c.set_number(a, 5.0).unwrap();
        crate::script::exec(&mut c, "h = (a + 1)/10").unwrap();
        crate::script::exec(&mut c, "a = Slider(0, 2)").unwrap();
        assert_eq!(c.slider_animation_options(a)[0], Some("0.3"));
    }

    #[test]
    fn redefining_a_number_as_its_own_slider_ceiling_clamps_later_moves() {
        let mut c = script::load("a = 3\nd = 2a").unwrap();
        let [a, d] = ["a", "d"].map(|label| c.id(label).unwrap());
        assert_eq!(script::exec(&mut c, "a = Slider(0, a)").unwrap(), Some(a));
        c.set_number(a, 7.0).unwrap();
        assert_eq!(c.value(a), Value::Number(3.0));
        assert_eq!(c.value(d), Value::Number(6.0));
    }
}
