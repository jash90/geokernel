//! Label allocation for growing intersections, tangents and inequality vertices.

use super::boundary::{self, BoundaryDef};
use crate::algo::{implicit::tangent::Tangent, lines::LineDef, point::PointDef};
use crate::expr::inequality;
use crate::kernel::{Construction, Def, Id};
use crate::script;
use std::collections::BTreeSet;

const SLOT_COMMAND: &str = "$boundarySlot";

#[derive(Debug)]
struct Outputs {
    first: Id,
    prefix: String,
    /// The label typed before `=`, which the first output takes again
    /// after it was freed (`AlgoGeoPointsFunction.updateLabels`).
    name: String,
    /// The object of each slot, where known.
    ids: Vec<Option<Id>>,
    dependencies: BTreeSet<Id>,
}

impl Outputs {
    /// The object of `slot`, found again if a rebuilt registry lost it.
    fn slot_id(&mut self, c: &Construction, anchor: &GrowthAnchor, slot: usize) -> Option<Id> {
        if let Some(Some(id)) = self.ids.get(slot) {
            return Some(*id);
        }
        let def = anchor.slot(slot)?;
        let id = (self.first.index() + 1..c.len())
            .map(Id)
            .find(|&id| c.def(id) == def)?;
        self.ids[slot] = Some(id);
        Some(id)
    }
}

enum GrowthAnchor {
    Intersection(BoundaryDef),
    Tangent(Tangent),
    Inequality(Id),
}

impl GrowthAnchor {
    fn from_def(def: Def) -> Option<Self> {
        match def {
            Def::IntersectBoundary(d) if d.can_grow() => Some(Self::Intersection(d)),
            Def::Lines(LineDef::ImplicitTangent(d)) if d.can_grow() => Some(Self::Tangent(d)),
            Def::Point(PointDef::InequalityVertex { source, slot: 0 }) => {
                Some(Self::Inequality(source))
            }
            _ => None,
        }
    }

    fn size(&self, c: &Construction) -> Option<usize> {
        match *self {
            Self::Intersection(d) => d.grown_size(c),
            Self::Tangent(d) => d.grown_size(c),
            Self::Inequality(source) => Some(inequality::vertices(c, source).len()),
        }
    }

    /// `AlgoGeoPointsFunction.updateLabels`: a point beyond the command's
    /// own labels gets the next default point label.
    fn default_labels(&self) -> bool {
        matches!(self, Self::Intersection(d) if d.grows_with_default_labels())
    }

    fn slot(&self, slot: usize) -> Option<Def> {
        match *self {
            Self::Intersection(d) => d.grown_slot(slot).map(Def::IntersectBoundary),
            Self::Tangent(d) => d.grown_slot(slot),
            Self::Inequality(source) if slot < inequality::MAX_VERTICES => {
                Some(Def::Point(PointDef::InequalityVertex {
                    source,
                    slot: slot as u16,
                }))
            }
            _ => None,
        }
    }
}

/// Tracks unindexed intersections, implicit tangents and inequality vertices.
/// Callers keep the definitions returned by `grow` in the same journal as
/// the edit that produced them.
#[derive(Debug, Default)]
pub struct BoundaryOutputs {
    groups: Vec<Outputs>,
    revision: u64,
    /// Slots (owner label, zero-based slot) that the script being replayed
    /// adds with its own `$boundarySlot` lines: growth stops before them.
    recorded: BTreeSet<(String, usize)>,
}

impl BoundaryOutputs {
    /// Whether a definition can start a group of growing outputs.
    pub fn is_command(rhs: &str) -> bool {
        rhs.split_once('(').is_some_and(|(name, _)| {
            matches!(name.trim(), "Intersect" | "Tangent" | "Vertex" | "Corner")
        })
    }

    /// A registry for replaying `src`, whose recorded slot lines add their
    /// outputs themselves.
    pub fn for_script(src: &str) -> Self {
        let recorded = src
            .lines()
            .filter_map(|line| {
                let (_, rhs) = script::split_comment(line).0.split_once('=')?;
                let (owner, slot) = slot_args(rhs.trim())?;
                Some((owner.to_owned(), slot))
            })
            .collect();
        Self {
            groups: Vec::new(),
            revision: 0,
            recorded,
        }
    }

    /// Rebuilds the registry from a definition that has already executed.
    pub fn record(&mut self, c: &Construction, line: &str, first: Id, count: usize) {
        if let Some((prefix, _)) = boundary_command(line) {
            let name = line
                .split_once(['=', ':'])
                .map_or("", |(name, _)| name.trim());
            if count > 0 && GrowthAnchor::from_def(c.def(first)).is_some() {
                // Inline arguments are added before the first output.
                let slots = count.min(c.len() - first.index());
                self.groups.push(Outputs {
                    first,
                    prefix: prefix.to_owned(),
                    name: name.to_owned(),
                    ids: (first.index()..first.index() + slots)
                        .map(|index| Some(Id(index)))
                        .collect(),
                    dependencies: dependencies(c, first),
                });
            }
        } else if let Some((anchor, slot)) = line
            .split_once('=')
            .and_then(|(_, rhs)| slot_args(rhs.trim()))
        {
            if let Some(owner) = slot_owner(c, anchor, slot) {
                if let Some(group) = self.groups.iter_mut().find(|group| group.first == owner) {
                    if group.ids.len() <= slot {
                        group.ids.resize(slot + 1, None);
                    }
                    // A later object may have taken the label of a freed slot.
                    let def = GrowthAnchor::from_def(c.def(owner)).and_then(|a| a.slot(slot));
                    group.ids[slot] = (def == Some(c.def(first))).then_some(first);
                }
            }
        }
    }

    /// Adds outputs revealed by an edit and passes their definitions to
    /// `record`. Slots that were already labelled stay in the construction.
    pub fn grow(
        &mut self,
        c: &mut Construction,
        changed: Id,
        record: impl FnMut(&Construction, Id, &str),
    ) -> bool {
        self.grow_after(c, std::iter::once(changed), record)
    }

    /// Visits each affected group once after an edit of one or more inputs.
    /// Returns whether a label was freed (see [`Self::release_after`]).
    pub fn grow_after(
        &mut self,
        c: &mut Construction,
        changed: impl Iterator<Item = Id> + Clone,
        record: impl FnMut(&Construction, Id, &str),
    ) -> bool {
        self.update(c, changed, Some(record))
    }

    /// Only the removals of [`Self::grow_after`], for a replay whose
    /// journal gives every label a slot takes: `AlgoRoots.initPoints`
    /// removes each output from slot `max(n, 1)` on that no object reads
    /// once `n` roots remain, which frees its label. The object stays in
    /// the construction without a label; `AlgoGeoPointsFunction.updateLabels`
    /// labels it again when its slot has a root. Returns whether a label
    /// was freed.
    pub fn release_after(
        &mut self,
        c: &mut Construction,
        changed: impl Iterator<Item = Id> + Clone,
    ) -> bool {
        self.update(c, changed, None::<fn(&Construction, Id, &str)>)
    }

    fn update(
        &mut self,
        c: &mut Construction,
        changed: impl Iterator<Item = Id> + Clone,
        mut record: Option<impl FnMut(&Construction, Id, &str)>,
    ) -> bool {
        let revision = c.definition_revision();
        if self.revision != revision {
            self.groups.retain(|group| group.first.index() < c.len());
            for group in &mut self.groups {
                group.dependencies = dependencies(c, group.first);
            }
            self.revision = revision;
        }
        let mut released = false;
        for group in &mut self.groups {
            if !changed.clone().any(|id| group.dependencies.contains(&id)) {
                continue;
            }
            let Some(anchor) = GrowthAnchor::from_def(c.def(group.first)) else {
                continue;
            };
            let frees = anchor.default_labels();
            if !frees && record.is_none() {
                continue;
            }
            let Some(count) = anchor.size(c) else {
                continue;
            };
            if frees {
                // With no root left, a point that another object reads keeps
                // every other point from being reset: they are all removed,
                // the first one too.
                if count == 0
                    && (0..group.ids.len()).any(|slot| {
                        group
                            .slot_id(c, &anchor, slot)
                            .is_some_and(|id| c.has_dependents(id))
                    })
                {
                    released |= c.free_owner_label(group.first);
                }
                for slot in count.max(1)..group.ids.len() {
                    if let Some(id) = group.slot_id(c, &anchor, slot) {
                        released |= c.free_output_label(id);
                    }
                }
            }
            let Some(record) = record.as_mut() else {
                continue;
            };
            let start = if frees { 0 } else { group.ids.len() };
            for slot in start..count {
                let owner = owner_name(c, group.first).to_owned();
                let existing = if slot < group.ids.len() {
                    match group.slot_id(c, &anchor, slot) {
                        Some(id) if c.is_freed_output(id) => Some(id),
                        _ => continue,
                    }
                } else {
                    None
                };
                if self.recorded.contains(&(owner.clone(), slot)) {
                    break;
                }
                let label = if slot == 0 {
                    // `labels[0]`; a reference that renames the object
                    // holding it is not reproduced
                    if c.resolve_label(&group.name).is_some() {
                        continue;
                    }
                    group.name.clone()
                } else if frees {
                    crate::algo::polygon::default_point_label(c)
                } else {
                    script::index_label(c, &group.prefix)
                };
                let id = match existing {
                    Some(id) => {
                        if c.relabel_output(id, &label).is_err() {
                            break;
                        }
                        id
                    }
                    None => {
                        let Some(output) = anchor.slot(slot) else {
                            break;
                        };
                        let Ok(id) = c.add_output(&label, output, group.first) else {
                            break;
                        };
                        group.ids.push(Some(id));
                        id
                    }
                };
                let line = format!("{label} = {SLOT_COMMAND}({owner}, {})", slot + 1);
                record(c, id, &line);
            }
        }
        released
    }
}

/// For a recorded slot line of a command whose outputs free their labels,
/// the object of that slot if the construction has it already: the line
/// gives it its label again.
pub(crate) fn existing_slot(c: &Construction, owner: Id, def: Def) -> Option<Id> {
    GrowthAnchor::from_def(c.def(owner))
        .filter(GrowthAnchor::default_labels)
        .and_then(|_| {
            (owner.index()..c.len())
                .map(Id)
                .find(|&id| c.def(id) == def)
        })
}

fn dependencies(c: &Construction, first: Id) -> BTreeSet<Id> {
    const MAX_EDGES: usize = 1_000_000;
    let mut dependencies = BTreeSet::new();
    let mut remaining = MAX_EDGES;
    let mut pending = vec![first];
    while let Some(id) = pending.pop() {
        if dependencies.insert(id) {
            let reads = c.read_ids(&c.def(id));
            let Some(next) = remaining.checked_sub(reads.len()) else {
                return BTreeSet::new();
            };
            remaining = next;
            pending.extend(reads);
        }
    }
    dependencies.remove(&first);
    dependencies
}

fn boundary_command(line: &str) -> Option<(&str, &str)> {
    let (name, rhs) = line.split_once(['=', ':'])?;
    let (name, rhs) = (name.trim(), rhs.trim());
    if name.starts_with('{') || !BoundaryOutputs::is_command(rhs) {
        return None;
    }
    let prefix = name
        .split('_')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("a");
    Some((prefix, rhs))
}

fn slot_args(rhs: &str) -> Option<(&str, usize)> {
    let (anchor, slot) = rhs
        .strip_prefix(SLOT_COMMAND)?
        .strip_prefix('(')?
        .strip_suffix(')')?
        .split_once(',')?;
    let slot = slot.trim().parse::<usize>().ok()?.checked_sub(1)?;
    Some((anchor.trim(), slot))
}

/// The first output a slot line names: the first slot names a first output
/// whose label was freed by the label it had.
fn slot_owner(c: &Construction, anchor: &str, slot: usize) -> Option<Id> {
    if slot == 0 {
        c.freed_owner(anchor)
    } else {
        c.id(anchor)
    }
}

/// The label slot lines give the first output `first`.
fn owner_name(c: &Construction, first: Id) -> &str {
    c.former_owner_label(first)
        .unwrap_or_else(|| c.label(first))
}

/// For a slot line that labels a freed first output again, the label that
/// output had.
pub fn first_slot_owner(rhs: &str) -> Option<&str> {
    slot_args(rhs).and_then(|(owner, slot)| (slot == 0).then_some(owner))
}

/// The first output of a command recorded under `label`, also when its
/// label was freed.
pub fn growth_owner(c: &Construction, label: &str) -> Option<Id> {
    c.freed_owner(label).or_else(|| c.id(label))
}

/// For a recorded slot definition, identifies the first output of its command.
pub fn boundary_slot_owner(rhs: &str) -> Option<&str> {
    slot_args(rhs).map(|(owner, _)| owner)
}

pub(crate) fn parse_slot(c: &Construction, rhs: &str) -> Option<Result<(Def, Id), String>> {
    if !rhs.starts_with(SLOT_COMMAND) {
        return None;
    }
    Some((|| {
        let (anchor, slot) = slot_args(rhs).ok_or("invalid boundary slot")?;
        let id = slot_owner(c, anchor, slot).ok_or_else(|| {
            if slot == 0 && c.id(anchor).is_some() {
                format!("`{anchor}` has no growing output at slot 1")
            } else {
                format!("unknown object `{anchor}`")
            }
        })?;
        GrowthAnchor::from_def(c.def(id))
            .and_then(|def| def.slot(slot))
            .map(|def| (def, id))
            .ok_or_else(|| format!("`{anchor}` has no growing output at slot {}", slot + 1))
    })())
}

/// Replay a generated command with its original labels, even if deletion
/// has freed an earlier label in the same indexed series.
pub fn restore_boundary_labels(
    c: &mut Construction,
    line: &str,
    labels: &[&str],
) -> Option<Result<Option<Id>, String>> {
    let (prefix, rhs) = boundary_command(line)?;
    let (command, args) = script::split_call(rhs)?;
    if !labels.iter().all(|label| {
        label
            .strip_prefix(prefix)
            .and_then(|suffix| suffix.strip_prefix("_{"))
            .and_then(|suffix| suffix.strip_suffix('}'))
            .is_some_and(|suffix| suffix.parse::<usize>().is_ok_and(|n| n > 0))
    }) {
        return None;
    }
    if matches!(command, "Intersect" | "Tangent") {
        let defs = if command == "Intersect" {
            boundary::parse_multi(c, command, &args)?
                .into_iter()
                .map(Def::IntersectBoundary)
                .collect()
        } else {
            Tangent::multi(c, command, &args)?
        };
        return (defs.len() == labels.len()
            && defs
                .first()
                .is_some_and(|&def| GrowthAnchor::from_def(def).is_some()))
        .then(|| script::add_named_outputs(c, labels, defs));
    }
    let start = c.checkpoint();
    let source = PointDef::inequality_source(c, command, &args)
        .or_else(|| PointDef::inline_inequality_source(c, command, &args))?;
    let count = inequality::vertices(c, source).len().max(1);
    let result = (count == labels.len())
        .then(|| script::add_named_outputs(c, labels, PointDef::inequality_slots(source, count)));
    if !matches!(result, Some(Ok(_))) {
        c.discard_since(start);
    }
    result
}

#[cfg(test)]
mod tests {
    use crate::{script, Value};

    #[test]
    fn script_slider_assignments_reveal_interval_roots() {
        let src = "b = Slider(1, 13)\nb = 4\nf(x) = sin(x)\nh(x) = 0\nR = Intersect(f, h, 1, b)";
        for tail in ["\nb = 10", "\nb = 7\nA = $boundarySlot(R, 2)\nb = 10"] {
            let c = script::load(&format!("{src}{tail}")).unwrap();
            for (label, x) in [("A", 2.0), ("B", 3.0)] {
                let id = c
                    .id(label)
                    .unwrap_or_else(|| panic!("no {label} after {tail:?}"));
                let Value::Point { x: px, .. } = c.value(id) else {
                    panic!("{label} is undefined");
                };
                assert!(
                    (px - x * std::f64::consts::PI).abs() < 1e-6,
                    "{label}: {px}"
                );
            }
        }
    }
}
