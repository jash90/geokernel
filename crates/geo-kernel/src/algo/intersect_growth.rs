//! Label allocation for intersections whose output count can grow.

use super::boundary;
use crate::kernel::{Construction, Def, Id};
use crate::script;
use std::collections::BTreeSet;

const SLOT_COMMAND: &str = "$boundarySlot";

#[derive(Debug)]
struct Outputs {
    first: Id,
    prefix: String,
    slots: usize,
    dependencies: BTreeSet<Id>,
}

/// Tracks each unindexed line–curve, line–boundary or boundary–boundary invocation.
/// Callers keep the definitions returned by `grow` in the same journal as
/// the edit that produced them.
#[derive(Debug, Default)]
pub struct BoundaryOutputs {
    groups: Vec<Outputs>,
}

impl BoundaryOutputs {
    /// Rebuilds the registry from a definition that has already executed.
    pub fn record(&mut self, c: &Construction, line: &str, first: Id, count: usize) {
        if let Some((prefix, _)) = boundary_command(line) {
            if matches!(c.def(first), Def::IntersectBoundary(d) if d.can_grow()) {
                self.groups.push(Outputs {
                    first,
                    prefix: prefix.to_owned(),
                    slots: count,
                    dependencies: dependencies(c, first),
                });
            }
        } else if let Some((anchor, slot)) = line
            .split_once('=')
            .and_then(|(_, rhs)| slot_args(rhs.trim()))
        {
            if let Some(id) = c.id(anchor) {
                if let Some(group) = self.groups.iter_mut().find(|group| group.first == id) {
                    group.slots = group.slots.max(slot + 1);
                }
            }
        }
    }

    /// Adds crossings revealed by an edit and passes their definitions to
    /// `record`. Slots that were already labelled stay in the construction.
    pub fn grow(
        &mut self,
        c: &mut Construction,
        changed: Id,
        record: impl FnMut(&Construction, Id, &str),
    ) {
        self.grow_after(c, std::iter::once(changed), record);
    }

    /// Visits each affected group once after an edit of one or more inputs.
    pub fn grow_after(
        &mut self,
        c: &mut Construction,
        changed: impl Iterator<Item = Id> + Clone,
        mut record: impl FnMut(&Construction, Id, &str),
    ) {
        for group in &mut self.groups {
            if !changed.clone().any(|id| group.dependencies.contains(&id)) {
                continue;
            }
            let Def::IntersectBoundary(def) = c.def(group.first) else {
                continue;
            };
            let Some(count) = def.grown_size(c) else {
                continue;
            };
            for slot in group.slots..count {
                let Some(output) = def.grown_slot(slot) else {
                    break;
                };
                let label = script::index_label(c, &group.prefix);
                let Ok(id) = c.add(&label, Def::IntersectBoundary(output)) else {
                    break;
                };
                group.slots = slot + 1;
                let line = format!(
                    "{label} = {SLOT_COMMAND}({}, {})",
                    c.label(group.first),
                    slot + 1
                );
                record(c, id, &line);
            }
        }
    }
}

fn dependencies(c: &Construction, first: Id) -> BTreeSet<Id> {
    let mut dependencies = BTreeSet::new();
    let mut pending = c.read_ids(&c.def(first));
    while let Some(id) = pending.pop() {
        if dependencies.insert(id) {
            pending.extend(c.read_ids(&c.def(id)));
        }
    }
    dependencies
}

fn boundary_command(line: &str) -> Option<(&str, &str)> {
    let (name, rhs) = line.split_once(['=', ':'])?;
    let (name, rhs) = (name.trim(), rhs.trim());
    if name.starts_with('{') || !rhs.starts_with("Intersect") {
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
    (slot > 0).then_some((anchor.trim(), slot))
}

/// For a recorded slot definition, identifies the first output of its command.
pub fn boundary_slot_owner(rhs: &str) -> Option<&str> {
    slot_args(rhs).map(|(owner, _)| owner)
}

pub(crate) fn parse_slot(c: &Construction, rhs: &str) -> Option<Result<Def, String>> {
    if !rhs.starts_with(SLOT_COMMAND) {
        return None;
    }
    Some((|| {
        let (anchor, slot) = slot_args(rhs).ok_or("invalid boundary slot")?;
        let id = c
            .id(anchor)
            .ok_or_else(|| format!("unknown object `{anchor}`"))?;
        if let Def::IntersectBoundary(def) = c.def(id) {
            if let Some(def) = def.grown_slot(slot) {
                return Ok(Def::IntersectBoundary(def));
            }
        }
        Err(format!(
            "`{anchor}` does not start a growing boundary intersection"
        ))
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
    if command != "Intersect"
        || !labels.iter().all(|label| {
            label
                .strip_prefix(prefix)
                .and_then(|suffix| suffix.strip_prefix("_{"))
                .and_then(|suffix| suffix.strip_suffix('}'))
                .is_some_and(|suffix| suffix.parse::<usize>().is_ok_and(|n| n > 0))
        })
    {
        return None;
    }
    let defs = boundary::parse_multi(c, command, &args)?;
    if defs.len() != labels.len() || !defs.first().is_some_and(|def| def.can_grow()) {
        return None;
    }
    Some(script::add_named_outputs(
        c,
        labels,
        defs.into_iter().map(Def::IntersectBoundary).collect(),
    ))
}
