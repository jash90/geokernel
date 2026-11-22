//! The construction as the lines that built it.
//!
//! The kernel can neither delete an object nor print a definition, so the app
//! keeps the normalised script lines. Deleting, undo and redo rebuild a
//! `Construction` from them; a line whose inputs are gone fails to run and is
//! dropped, which deletes everything that depended on the removed object.
//!
//! As text the journal is a script: the definition lines, plus the moves and
//! the hidden objects as `SetCoords(A, x, y)`, `SetValue(n, v)` and
//! `SetVisibleInView(A, 1, false)` (the script commands of the same meaning
//! in GeoGebra scripts). These directives are read here, not by the kernel:
//! they change the state of objects the script already made. Their three
//! names are therefore reserved: a line that names an object or a function
//! `SetCoords`, `SetValue` or `SetVisibleInView`, or defines an object by
//! one of them, is refused (by the input bar too, where a typed directive
//! would otherwise become `f = SetCoords(...)`).
//!
//! A snapshot (`to_labelled_script`) also lists, after a definition, the
//! labels it made when they are not just the names before its `=` (the
//! sides of a polygon, the outputs of `Tangent`): `p = Polygon(A, B, C)
//! # labels: p, a_1, b, c`. The list is a comment, so the text stays a
//! plain script; `load_tolerant` checks it.
//! A crossing revealed by a move is saved as a slot definition after that
//! move. Its exact label and its origin survive further moves and replay.
//! A simultaneous move is saved as one `$movePoints` directive.

use geo_kernel::algo::measure::MeasureDef;
use geo_kernel::kernel::control::{decode_text, encode_text, visibility_boolean};
use geo_kernel::kernel::{Construction, Def, Id};
use geo_kernel::script::{
    boundary_slot_owner, restore_boundary_labels, split_comment, BoundaryOutputs,
};
use shared::Chain;
pub use shared::Labels;
use std::collections::{BTreeMap, BTreeSet};

mod shared;

/// The names of the directives, which no object may take.
pub const DIRECTIVES: [&str; 3] = ["SetCoords", "SetValue", "SetVisibleInView"];

/// Starts the list of labels after a definition in a snapshot.
const LABELS: &str = "# labels: ";
const MOVE_POINTS: &str = "$movePoints";
const ANGLE_STYLE: &str = "$angleStyle";
const SHOW_OBJECT: &str = "$showObject";

fn has_recorded_growth(src: &str) -> bool {
    src.lines().any(|line| {
        split_comment(line)
            .0
            .split_once('=')
            .and_then(|(_, rhs)| boundary_slot_owner(rhs.trim()))
            .is_some()
    })
}

/// One accepted line and the objects it created.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub labels: Vec<String>,
    /// Normalised: `name = definition`.
    pub line: String,
}

impl Entry {
    /// The definition after the first `:` or `=`, as typed. An equation
    /// line keeps its `:` (`a: 2x + 3y = 6`), so its definition is the
    /// whole equation.
    pub fn rhs(&self) -> &str {
        let rhs = self
            .line
            .find([':', '='])
            .and_then(|i| self.line.get(i + 1..))
            .map_or("", str::trim);
        geo_kernel::script::saved_definition(rhs).unwrap_or(rhs)
    }

    fn replay(&self, c: &mut Construction) -> Result<Option<Id>, String> {
        if self.rhs().starts_with("Intersect") {
            let labels: Vec<_> = self.labels.iter().map(String::as_str).collect();
            if let Some(result) = restore_boundary_labels(c, &self.line, &labels) {
                return result;
            }
        }
        exec_definition(c, &self.line)
    }
}

pub(crate) struct GrownOutputs<'a> {
    pub labels: Vec<String>,
    pub rhs: &'a str,
}

/// A new value of a free object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Setting {
    Point(f64, f64),
    Number(f64),
    Boolean(bool),
    AngleStyle(geo_kernel::kernel::angle::AngleStyle),
}

type Position<L> = (L, f64, f64);

#[derive(Clone, Debug, PartialEq)]
enum Op {
    Define(Entry),
    Set {
        label: String,
        to: Setting,
        ordered: bool,
    },
    MovePoints(Vec<Position<String>>),
}

/// Rewrites `f: Line(A, B)` as `f = Line(A, B)` and removes a trailing `#`
/// comment. Blank input gives an empty string. A `:` counts only when it comes
/// before any `=`; an equation (`f: 2x + 3y = 6`, whose `=` is part of the
/// definition) keeps its `:`, which the script of the kernel reads.
pub fn normalize(input: &str) -> String {
    normalize_definition(split_comment(input).0)
}

fn normalize_definition(input: &str) -> String {
    let line = input.trim();
    let colon = line.find(':');
    let eq = line.find('=');
    match (colon, eq) {
        (Some(c), None) => format!("{} = {}", line[..c].trim(), line[c + 1..].trim()),
        (Some(c), Some(e)) if c < e => format!("{}: {}", line[..c].trim(), line[c + 1..].trim()),
        _ => line.to_owned(),
    }
}

/// A script line that sets the state of an object instead of defining one.
#[derive(Clone, Debug, PartialEq)]
enum Directive<'a> {
    Set(&'a str, Setting),
    MovePoints(Vec<Position<&'a str>>),
    Visible(&'a str, bool),
    Condition(&'a str, &'a str),
}

/// The directive on a normalised line, if it is one (a definition always has
/// an `=`).
fn directive(line: &str) -> Option<Result<Directive<'_>, String>> {
    if line.contains('=') {
        return None;
    }
    let (name, rest) = line.split_once('(')?;
    let name = name.trim();
    if !matches!(name, MOVE_POINTS | ANGLE_STYLE | SHOW_OBJECT) && !DIRECTIVES.contains(&name) {
        return None;
    }
    let parse = || -> Option<Directive<'_>> {
        let args: Vec<&str> = rest.strip_suffix(')')?.split(',').map(str::trim).collect();
        let num = |s: &str| s.parse::<f64>().ok();
        match (name, args.as_slice()) {
            ("SetCoords", [l, x, y]) => Some(Directive::Set(l, Setting::Point(num(x)?, num(y)?))),
            (MOVE_POINTS, args) => {
                let (points, []) = args.as_chunks::<3>() else {
                    return None;
                };
                points
                    .iter()
                    .map(|&[label, x, y]| Some((label, num(x)?, num(y)?)))
                    .collect::<Option<Vec<_>>>()
                    .map(Directive::MovePoints)
            }
            ("SetValue", [l, v]) => Some(Directive::Set(
                l,
                match v.parse::<bool>() {
                    Ok(value) => Setting::Boolean(value),
                    Err(_) => Setting::Number(num(v)?),
                },
            )),
            (SHOW_OBJECT, [target, boolean]) => Some(Directive::Condition(target, boolean)),
            (ANGLE_STYLE, [l, style]) => Some(Directive::Set(
                l,
                Setting::AngleStyle(geo_kernel::kernel::angle::AngleStyle::from_xml(
                    style.parse().ok()?,
                )),
            )),
            ("SetVisibleInView", [l, "1", shown]) => {
                Some(Directive::Visible(l, shown.parse().ok()?))
            }
            _ => None,
        }
    };
    Some(parse().ok_or_else(|| format!("cannot read `{line}`")))
}

fn replay_move_points(
    c: &mut Construction,
    points: &[Position<impl AsRef<str>>],
    gone: Option<&BTreeSet<String>>,
) -> Result<Vec<Position<Id>>, String> {
    let mut positions = Vec::with_capacity(points.len());
    for (label, x, y) in points {
        let label = label.as_ref();
        if let Some(id) = c.id(label) {
            positions.push((id, *x, *y));
        } else if !gone.is_some_and(|gone| gone.contains(label)) {
            return Err(format!("`{label}` is missing"));
        }
    }
    if !positions.is_empty() {
        c.move_points(&positions).map_err(|e| e.to_string())?;
    }
    Ok(positions)
}

/// A line that `Log::load_tolerant` left out.
#[derive(Clone, Debug, PartialEq)]
pub struct Rejected {
    /// 1-based.
    pub line: usize,
    pub text: String,
    pub reason: String,
    /// It uses an object of a line rejected before it.
    pub dependent: bool,
}

/// The labels a definition line names before its `:` or `=`: `A`, `f` of
/// `f(x)`, or those of `{t_{1}, t_{2}}` (an empty slot names nothing).
/// An equation line names its label before the `:` of `a: 2x + 3y = 6`.
fn defined_labels(line: &str) -> impl Iterator<Item = &str> {
    let lhs = line
        .find([':', '='])
        .and_then(|i| line.get(..i))
        .map_or("", str::trim);
    let lhs = match lhs.strip_prefix('{').and_then(|l| l.strip_suffix('}')) {
        Some(list) => list,
        None => lhs.strip_suffix("(x)").map_or(lhs, str::trim_end),
    };
    lhs.split(',').map(str::trim).filter(|l| !l.is_empty())
}

/// Refuses a definition that takes the name of a directive, as a label or
/// a function, or that calls one (a directive defines nothing).
pub fn check_names(line: &str) -> Result<(), String> {
    if let Some(name) = defined_labels(line).find(|l| DIRECTIVES.contains(l)) {
        return Err(format!(
            "`{name}` is the name of a file directive and cannot name an object"
        ));
    }
    let rhs = line.split_once('=').map_or("", |(_, r)| r);
    let call = rhs.split('(').next().unwrap_or("").trim();
    if rhs.contains('(') && DIRECTIVES.contains(&call) {
        return Err(format!(
            "`{call}` is a file directive: it sets an object of a saved file and defines nothing"
        ));
    }
    Ok(())
}

/// The identifiers of a script line.
fn tokens(line: &str) -> impl Iterator<Item = &str> {
    line.split(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '{' | '}' | '\'')))
        .filter(|t| !t.is_empty())
}

/// Internal dependencies have a kernel id but no user-visible label.
pub(crate) fn object_ids(c: &Construction) -> impl Iterator<Item = Id> + '_ {
    outputs(c, 0)
}

fn outputs(c: &Construction, from: usize) -> impl Iterator<Item = Id> + '_ {
    c.ids().skip(from).filter(|&id| !is_internal(c, id))
}

pub(crate) fn is_internal(c: &Construction, id: Id) -> bool {
    is_internal_label(c.label(id))
}

pub(crate) fn is_internal_label(label: &str) -> bool {
    label.starts_with('\0')
}

/// Lower the inline radius of a compasses circle to existing kernel
/// definitions. Both additions belong to the circle's single journal entry.
pub(crate) fn exec_definition(c: &mut Construction, line: &str) -> Result<Option<Id>, String> {
    if line.contains('\0') {
        return Err("an object definition contains a null character".into());
    }
    let Some((label, rhs)) = line.split_once('=') else {
        return geo_kernel::script::exec(c, line);
    };
    let inline_radius = || {
        let args = rhs.trim().strip_prefix("Circle(")?.strip_suffix(')')?;
        let (center, radius) = args.split_once(',')?;
        let (cmd, operands) = radius.trim().split_once('(')?;
        matches!(cmd, "Radius" | "Segment").then_some((
            center.trim(),
            cmd,
            operands.strip_suffix(')')?,
        ))
    };
    let Some((center, cmd, operands)) = inline_radius() else {
        return geo_kernel::script::exec(c, line);
    };
    let object = |name: &str| {
        c.id(name.trim())
            .ok_or_else(|| format!("unknown object `{}`", name.trim()))
    };
    let def = if cmd == "Radius" {
        Def::Measure(MeasureDef::Radius(object(operands)?))
    } else {
        let (a, b) = operands
            .split_once(',')
            .ok_or("segment radius needs two points")?;
        // A distance is the segment's numeric value. The helper therefore
        // has no drawable geometry, including in Show/Hide mode.
        Def::Distance(object(a)?, object(b)?)
    };
    let label = label.trim();
    let helper = format!("\0radius:{label}");
    let checkpoint = c.checkpoint();
    c.add(&helper, def).map_err(|e| e.to_string())?;
    let result = geo_kernel::script::exec(c, &format!("{label} = Circle({center}, {helper})"));
    if result.is_err() {
        c.rollback(checkpoint).map_err(|e| e.to_string())?;
    }
    result
}

/// The journal of a construction, plus which objects are hidden.
///
/// Copies share their storage (see `shared.rs`): a clone, as an undo step
/// takes, costs a pointer and the steps after it take memory for what
/// they change only.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Log {
    ops: Chain<Op>,
    /// Labels hidden in the drawing (part of the state that undo restores).
    pub hidden: Labels,
    conditions: BTreeMap<String, VisibilityCondition>,
    /// Labels removed by `remove` since the last rebuild: lines that use them
    /// are dropped by the rebuild on purpose.
    removed: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct VisibilityCondition {
    source: String,
    boolean: String,
}

impl Log {
    pub(crate) fn condition(&self, label: &str) -> Option<&str> {
        self.conditions
            .get(label)
            .map(|condition| condition.boolean.as_str())
    }

    pub(crate) fn set_condition(
        &mut self,
        c: &mut Construction,
        target: &str,
        source: &str,
    ) -> Result<(), String> {
        if c.id(target).is_none() {
            return Err(format!("`{target}` is missing"));
        }
        if self.conditions.contains_key(target) {
            return Err(format!("`{target}` already has a visibility condition"));
        }
        let id = visibility_boolean(c, target, source)?;
        self.conditions.insert(
            target.to_owned(),
            VisibilityCondition {
                source: source.to_owned(),
                boolean: c.label(id).to_owned(),
            },
        );
        Ok(())
    }
    fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.ops.items().into_iter().filter_map(|op| match op {
            Op::Define(e) => Some(e),
            Op::Set { .. } | Op::MovePoints(_) => None,
        })
    }

    /// The accepted lines, newest first, without collecting them.
    pub(crate) fn entries_rev(&self) -> impl Iterator<Item = &Entry> {
        self.ops.items_rev().filter_map(|op| match op {
            Op::Define(e) => Some(e),
            Op::Set { .. } | Op::MovePoints(_) => None,
        })
    }

    /// Records `line` as the origin of the objects with index `from..` in `c`.
    pub fn record(&mut self, c: &Construction, from: usize, line: &str) {
        let labels: Vec<String> = c
            .ids()
            .skip(from)
            .filter(|&id| !is_internal(c, id))
            .map(|id| c.label(id).to_owned())
            .collect();
        if !labels.is_empty() {
            self.ops.push(Op::Define(Entry {
                labels,
                line: line.to_owned(),
            }));
        }
    }

    /// Records a new value of a free object.
    pub fn record_set(&mut self, label: &str, to: Setting) {
        self.push_set(label, to, false);
    }

    pub(crate) fn record_update(&mut self, c: &Construction, label: &str, to: Setting) {
        self.push_set(label, to, c.last_update_changed_automatic_root());
    }

    fn push_set(&mut self, label: &str, to: Setting, ordered: bool) {
        let set = Op::Set {
            label: label.to_owned(),
            to,
            ordered,
        };
        // A hidden Newton start point follows each successful root. Keep
        // any edit that moved it, since the next edit starts from there.
        if !ordered
            && matches!(
                self.ops.last(),
                Some(Op::Set { label: previous, ordered: false, .. }) if previous == label
            )
        {
            self.ops.replace_last(set);
        } else {
            self.ops.push(set);
        }
    }

    pub(crate) fn record_move_points(&mut self, c: &Construction, points: &[Position<Id>]) {
        if !points.is_empty() {
            let points = points
                .iter()
                .map(|&(id, x, y)| (c.label(id).to_owned(), x, y))
                .collect();
            self.ops.push(Op::MovePoints(points));
        }
    }

    pub(crate) fn boundary_outputs(&self, c: &Construction) -> BoundaryOutputs {
        let entries: Vec<_> = self
            .entries_rev()
            .filter(|e| e.rhs().starts_with("Intersect") || boundary_slot_owner(e.rhs()).is_some())
            .collect();
        let mut groups = BoundaryOutputs::default();
        for e in entries.into_iter().rev() {
            if let Some(first) = e.labels.first().and_then(|label| c.id(label)) {
                groups.record(c, &e.line, first, e.labels.len());
            }
        }
        groups
    }

    pub(crate) fn grow_boundary_outputs(
        &mut self,
        c: &mut Construction,
        groups: &mut BoundaryOutputs,
        changed: impl Iterator<Item = Id> + Clone,
    ) {
        groups.grow_after(c, changed, |c, id, line| self.record(c, id.index(), line));
    }

    pub(crate) fn grown_groups(&self) -> Vec<GrownOutputs<'_>> {
        let mut births: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        let mut groups = Vec::new();
        for e in self.entries_rev() {
            if let Some(owner) = boundary_slot_owner(e.rhs()) {
                births
                    .entry(owner)
                    .or_default()
                    .extend(e.labels.iter().rev().cloned());
            } else if let Some(born) = e
                .labels
                .first()
                .and_then(|label| births.remove(label.as_str()))
            {
                let mut labels = e.labels.clone();
                labels.extend(born.into_iter().rev());
                groups.push(GrownOutputs {
                    labels,
                    rhs: e.rhs(),
                });
            }
        }
        groups
    }

    /// Every label with the definition of the line that created it.
    pub fn definitions(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries()
            .flat_map(|e| e.labels.iter().map(move |l| (l.as_str(), e.rhs())))
    }

    /// The definition text of the entry that created `label`.
    pub fn rhs_of(&self, label: &str) -> Option<&str> {
        let definition = |name| {
            self.definitions()
                .find(|(label, _)| *label == name)
                .map(|(_, rhs)| rhs)
        };
        let rhs = definition(label)?;
        match boundary_slot_owner(rhs) {
            Some(owner) => definition(owner),
            None => Some(rhs),
        }
    }

    /// The label lists of the lines that created several objects.
    pub fn multi_output(&self) -> impl Iterator<Item = &[String]> {
        self.entries()
            .filter(|e| e.labels.len() > 1)
            .map(|e| e.labels.as_slice())
    }

    /// The other objects created by the same line as `label`.
    #[cfg(test)]
    pub fn siblings(&self, label: &str) -> Vec<String> {
        self.multi_output()
            .find(|ls| ls.iter().any(|l| l == label))
            .map(|ls| ls.iter().filter(|l| *l != label).cloned().collect())
            .unwrap_or_default()
    }

    /// Removes the line that created `label` (and with it every object the
    /// line created) and the moves of those objects. Returns whether there
    /// was such a line. Lines that use a removed object go on the next rebuild.
    pub fn remove(&mut self, label: &str) -> bool {
        let Some(entry) = self.entries().find(|e| e.labels.iter().any(|l| l == label)) else {
            return false;
        };
        let grown = (entry.rhs().starts_with("Intersect")
            || boundary_slot_owner(entry.rhs()).is_some())
        .then(|| {
            self.grown_groups()
                .into_iter()
                .find(|group| group.labels.iter().any(|member| member == label))
        })
        .flatten();
        let (grouped, labels) = match grown {
            Some(group) => (true, group.labels),
            None => (false, entry.labels.clone()),
        };
        self.ops.retain(|op| match op {
            Op::Define(e) if grouped => !e.labels.iter().any(|label| labels.contains(label)),
            Op::Define(e) => e.labels != labels,
            Op::Set { label, .. } => !labels.contains(label),
            Op::MovePoints(_) => true,
        });
        self.removed.extend(labels);
        true
    }

    /// Replays the journal on a fresh construction. Every line must create
    /// the labels it created before; a line that fails is dropped only when
    /// it uses an object that was removed (or dropped itself), otherwise the
    /// whole rebuild fails and the caller keeps its previous state.
    pub fn rebuild(&self) -> Result<(Construction, Log), String> {
        let mut c = Construction::new();
        let mut kept = Log::default();
        let mut gone = self.removed.clone();
        // While nothing is dropped or merged, `kept` is the first `same`
        // ops of the journal and shares their storage (see `shared.rs`).
        let ops = self.ops.items();
        let mut same = Some(0);
        let diverge = |kept: &mut Log, same: &mut Option<usize>| {
            if let Some(n) = same.take() {
                kept.ops = self.ops.prefix(n);
            }
        };
        for (i, op) in ops.iter().enumerate() {
            match op {
                Op::Define(e) => {
                    let before = c.len();
                    match e.replay(&mut c) {
                        Ok(_) => {
                            let made: Vec<&str> =
                                outputs(&c, before).map(|id| c.label(id)).collect();
                            if made != e.labels {
                                return Err(format!(
                                    "`{}` created {made:?} instead of {:?}",
                                    e.line, e.labels
                                ));
                            }
                            // What `record` would make of it is `e` itself.
                            match &mut same {
                                Some(n) => *n = i + 1,
                                None => kept.record(&c, before, &e.line),
                            }
                        }
                        Err(err) => {
                            if c.len() != before || !tokens(&e.line).any(|t| gone.contains(t)) {
                                return Err(format!("`{}`: {err}", e.line));
                            }
                            gone.extend(e.labels.iter().cloned());
                            diverge(&mut kept, &mut same);
                        }
                    }
                }
                Op::Set { label, to, ordered } => {
                    let Some(id) = c.id(label) else {
                        if gone.contains(label) {
                            diverge(&mut kept, &mut same);
                            continue;
                        }
                        return Err(format!("`{label}` is missing"));
                    };
                    let result = match *to {
                        Setting::Point(x, y) => c.move_point(id, x, y),
                        Setting::Number(v) => c.set_number(id, v),
                        Setting::Boolean(v) => c.set_boolean(id, v),
                        Setting::AngleStyle(style) => c.set_angle_style(id, style),
                    };
                    result.map_err(|e| format!("`{label}`: {e}"))?;
                    // A set right after a set of the same object is merged
                    // into it by `record_set` (after a `remove` they can
                    // meet): the journal and `kept` part there.
                    let keep_order = *ordered || c.last_update_changed_automatic_root();
                    let merges = !keep_order
                        && i > 0
                        && matches!(
                            &ops[i - 1],
                            Op::Set { label: previous, ordered: false, .. } if previous == label
                        );
                    match same {
                        Some(n) if n == i && !merges && keep_order == *ordered => {
                            same = Some(i + 1)
                        }
                        _ => {
                            diverge(&mut kept, &mut same);
                            kept.push_set(label, *to, keep_order);
                        }
                    }
                }
                Op::MovePoints(points) => {
                    let positions = replay_move_points(&mut c, points, Some(&gone))?;
                    match &mut same {
                        Some(n) if positions.len() == points.len() => *n = i + 1,
                        _ => {
                            diverge(&mut kept, &mut same);
                            kept.record_move_points(&c, &positions);
                        }
                    }
                }
            }
        }
        if let Some(n) = same {
            kept.ops = self.ops.prefix(n);
        }
        kept.hidden = self.hidden.clone();
        self.replay_conditions(&mut c, &mut kept);
        Ok((c, kept))
    }

    /// A copy that shares no storage with `self`: what every clone was
    /// before the journal shared its storage. For the tests that compare
    /// the two.
    #[cfg(test)]
    pub(crate) fn deep_copy(&self) -> Log {
        let mut ops = Chain::default();
        for op in self.ops.items() {
            ops.push(op.clone());
        }
        let mut hidden = Labels::default();
        for l in &self.hidden {
            hidden.insert(l.clone());
        }
        Log {
            ops,
            hidden,
            conditions: self.conditions.clone(),
            removed: self.removed.clone(),
        }
    }

    /// `rebuild` as it was before the journal shared its storage: every
    /// kept line recorded anew. The oracle of the tests of `rebuild`.
    #[cfg(test)]
    pub(crate) fn rebuild_reference(&self) -> Result<(Construction, Log), String> {
        let mut c = Construction::new();
        let mut kept = Log::default();
        let mut gone = self.removed.clone();
        for op in self.ops.items() {
            match op {
                Op::Define(e) => {
                    let before = c.len();
                    match e.replay(&mut c) {
                        Ok(_) => {
                            let made: Vec<&str> =
                                outputs(&c, before).map(|id| c.label(id)).collect();
                            if made != e.labels {
                                return Err(format!(
                                    "`{}` created {made:?} instead of {:?}",
                                    e.line, e.labels
                                ));
                            }
                            kept.record(&c, before, &e.line);
                        }
                        Err(err) => {
                            if c.len() != before || !tokens(&e.line).any(|t| gone.contains(t)) {
                                return Err(format!("`{}`: {err}", e.line));
                            }
                            gone.extend(e.labels.iter().cloned());
                        }
                    }
                }
                Op::Set { label, to, ordered } => {
                    let Some(id) = c.id(label) else {
                        if gone.contains(label) {
                            continue;
                        }
                        return Err(format!("`{label}` is missing"));
                    };
                    let result = match *to {
                        Setting::Point(x, y) => c.move_point(id, x, y),
                        Setting::Number(v) => c.set_number(id, v),
                        Setting::Boolean(v) => c.set_boolean(id, v),
                        Setting::AngleStyle(style) => c.set_angle_style(id, style),
                    };
                    result.map_err(|e| format!("`{label}`: {e}"))?;
                    kept.push_set(
                        label,
                        *to,
                        *ordered || c.last_update_changed_automatic_root(),
                    );
                }
                Op::MovePoints(points) => {
                    let positions = replay_move_points(&mut c, points, Some(&gone))?;
                    kept.record_move_points(&c, &positions);
                }
            }
        }
        kept.hidden = self.hidden.clone();
        self.replay_conditions(&mut c, &mut kept);
        Ok((c, kept))
    }

    /// The number of entries (definitions and moves).
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.ops.len()
    }

    /// `self` with its last definition line broken, so that replaying it
    /// fails inside `rebuild`: the line names a command that does not
    /// exist, or (`wrong_label`) makes another object than it did. The
    /// lines before it stay shared. For the tests of the replay's errors.
    #[cfg(test)]
    pub(crate) fn broken(&self, wrong_label: bool) -> Log {
        let items = self.ops.items();
        let Some(last) = items.iter().rposition(|op| matches!(op, Op::Define(_))) else {
            return self.clone();
        };
        let mut ops = self.ops.prefix(last);
        for (i, op) in items[last..].iter().enumerate() {
            let mut op = (*op).clone();
            if let (0, Op::Define(e)) = (i, &mut op) {
                e.line = if wrong_label {
                    "zzWrong = (0, 0)".to_owned()
                } else {
                    format!("{} = NoSuchCommand(1)", e.labels[0])
                };
            }
            ops.push(op);
        }
        Log {
            ops,
            hidden: self.hidden.clone(),
            conditions: self.conditions.clone(),
            removed: self.removed.clone(),
        }
    }

    /// The storage `logs` take together, in journal entries.
    #[cfg(test)]
    pub(crate) fn distinct_entries<'a>(logs: impl IntoIterator<Item = &'a Log>) -> usize {
        Chain::distinct_nodes(logs.into_iter().map(|l| &l.ops))
    }

    /// The journal as a script that `load` reads back into the same
    /// construction and an equal log: every definition line and every move
    /// in their order, then the hidden objects. Numbers are written with
    /// Rust's shortest exact form, so they read back bit for bit.
    ///
    /// It walks the whole journal: call it to save, not on every change.
    pub fn to_script(&self) -> String {
        self.write_script(false)
    }

    /// `to_script`, with the labels each definition made where they are not
    /// the names before its `=` (see the module docs), for `load_tolerant`
    /// to check.
    pub fn to_labelled_script(&self) -> String {
        self.write_script(true)
    }

    fn write_script(&self, labels: bool) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        for op in self.ops.items() {
            // Writing to a `String` cannot fail.
            let _ = match op {
                Op::Define(e)
                    if labels
                        && !defined_labels(&e.line).eq(e.labels.iter().map(String::as_str)) =>
                {
                    writeln!(out, "{} {LABELS}{}", e.line, e.labels.join(", "))
                }
                Op::Define(e) => writeln!(out, "{}", e.line),
                Op::Set {
                    label,
                    to: Setting::Point(x, y),
                    ..
                } => writeln!(out, "SetCoords({label}, {x}, {y})"),
                Op::Set {
                    label,
                    to: Setting::Number(v),
                    ..
                } => writeln!(out, "SetValue({label}, {v})"),
                Op::Set {
                    label,
                    to: Setting::Boolean(v),
                    ..
                } => writeln!(out, "SetValue({label}, {v})"),
                Op::Set {
                    label,
                    to: Setting::AngleStyle(style),
                    ..
                } => writeln!(out, "{ANGLE_STYLE}({label}, {})", style.xml_code()),
                Op::MovePoints(points) => {
                    let _ = write!(out, "{MOVE_POINTS}(");
                    for (i, (label, x, y)) in points.iter().enumerate() {
                        if i > 0 {
                            out.push_str(", ");
                        }
                        let _ = write!(out, "{label}, {x}, {y}");
                    }
                    writeln!(out, ")")
                }
            };
        }
        for label in &self.hidden {
            let _ = writeln!(out, "SetVisibleInView({label}, 1, false)");
        }
        for (target, condition) in &self.conditions {
            let _ = writeln!(
                out,
                "{SHOW_OBJECT}({target}, {})",
                encode_text(&condition.source)
            );
        }
        out
    }

    fn replay_conditions(&self, c: &mut Construction, kept: &mut Log) {
        for (target, condition) in &self.conditions {
            // A deleted object can be a target or be read by a condition.
            let _ = kept.set_condition(c, target, &condition.source);
        }
    }

    /// Applies a directive line to `c` and records it.
    fn apply(&mut self, c: &mut Construction, d: Directive) -> Result<Vec<Id>, String> {
        let (id, label, to) = match d {
            Directive::Set(label, to) => {
                let id = c.id(label).ok_or_else(|| format!("`{label}` is missing"))?;
                (id, label, to)
            }
            Directive::MovePoints(points) => {
                let points = replay_move_points(c, &points, None)?;
                self.record_move_points(c, &points);
                return Ok(points.into_iter().map(|(id, _, _)| id).collect());
            }
            Directive::Visible(label, shown) => {
                if c.id(label).is_none() {
                    return Err(format!("`{label}` is missing"));
                }
                if shown {
                    self.hidden.remove(label);
                } else {
                    self.hidden.insert(label.to_owned());
                }
                return Ok(Vec::new());
            }
            Directive::Condition(target, condition) => {
                self.set_condition(c, target, &decode_text(condition)?)?;
                return Ok(Vec::new());
            }
        };
        let result = match to {
            Setting::Point(x, y) => c.move_point(id, x, y),
            Setting::Number(v) => c.set_number(id, v),
            Setting::Boolean(v) => c.set_boolean(id, v),
            Setting::AngleStyle(style) => c.set_angle_style(id, style),
        };
        result.map_err(|e| format!("`{label}`: {e}"))?;
        if matches!(to, Setting::AngleStyle(_)) {
            self.push_set(label, to, true);
        } else {
            self.record_update(c, label, to);
        }
        Ok(vec![id])
    }

    /// Builds a log and construction from a script, keeping every line that
    /// loads, in one pass. For texts the app wrote itself (an autosave): a
    /// kernel of another version may refuse some line, and the rest must not
    /// be lost. Left out, and returned instead of failing the whole text:
    ///
    /// - a line the kernel refuses (whatever it made of it is removed again);
    /// - with `checked` (a snapshot with its header), a definition that makes
    ///   other labels than the list after it, or the names before its `=`
    ///   when it has no list: a later reference would otherwise reach the
    ///   wrong object;
    /// - a line that uses or names a label of a line left out before it
    ///   (`dependent`): such a label stays reserved to the end, so neither a
    ///   later definition nor a directive can take it over;
    /// - a directive for a missing object.
    pub fn load_tolerant(src: &str, checked: bool) -> (Construction, Log, Vec<Rejected>) {
        let mut c = Construction::new();
        let mut log = Log::default();
        let mut groups = BoundaryOutputs::default();
        let grow = !checked && !has_recorded_growth(src);
        let mut rejected = Vec::new();
        let mut gone: BTreeSet<String> = BTreeSet::new();
        for (i, raw) in src.lines().enumerate() {
            let (body, comment) = split_comment(raw);
            let listed = comment.and_then(|comment| comment.strip_prefix(LABELS));
            let line = normalize_definition(body);
            if line.is_empty() {
                continue;
            }
            let parsed = directive(&line);
            let is_directive = parsed.is_some();
            let named: Vec<&str> = match listed {
                Some(list) => list
                    .split(',')
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .collect(),
                None if is_directive => Vec::new(),
                None => defined_labels(&line).collect(),
            };
            let taken = tokens(&line)
                .chain(named.iter().copied())
                .find(|t| gone.contains(*t));
            let result = if let Some(label) = taken {
                Err(format!("`{label}` was left out before"))
            } else {
                match parsed {
                    Some(d) => d.and_then(|d| {
                        let changed = log.apply(&mut c, d)?;
                        if grow && !changed.is_empty() {
                            log.grow_boundary_outputs(&mut c, &mut groups, changed.into_iter());
                        }
                        Ok(())
                    }),
                    None => check_names(&line).and_then(|()| {
                        let expected = (checked || listed.is_some()).then_some(named.as_slice());
                        let before = c.len();
                        let first = log.define_checked(&mut c, &line, expected)?;
                        if let Some(id) = first.filter(|_| grow && c.len() > before) {
                            groups.record(&c, &line, id, c.len() - before);
                        }
                        Ok(())
                    }),
                }
            };
            if let Err(reason) = result {
                let dependent = taken.is_some();
                gone.extend(named.iter().map(|l| (*l).to_owned()));
                rejected.push(Rejected {
                    line: i + 1,
                    text: line.clone(),
                    reason,
                    dependent,
                });
            }
        }
        (c, log, rejected)
    }

    /// Runs a definition line and records it. When `expected` is given the
    /// line must make exactly those labels. On any failure what the line
    /// made is removed again.
    fn define_checked(
        &mut self,
        c: &mut Construction,
        line: &str,
        expected: Option<&[&str]>,
    ) -> Result<Option<Id>, String> {
        let at = c.checkpoint();
        let before = c.len();
        // The checkpoint was taken just above and nothing rolls the
        // construction back in between, so the kernel cannot refuse it; if it
        // ever did, the line is still reported as left out, with the reason.
        let undo = |c: &mut Construction, reason: String| match c.rollback(at) {
            Ok(()) => reason,
            Err(e) => format!("{reason}; {e}"),
        };
        let result = expected
            .and_then(|labels| restore_boundary_labels(c, line, labels))
            .unwrap_or_else(|| exec_definition(c, line));
        let first = result.map_err(|reason| undo(c, reason))?;
        if let Some(expected) = expected {
            if !outputs(c, before)
                .map(|id| c.label(id))
                .eq(expected.iter().copied())
            {
                let made: Vec<&str> = outputs(c, before).map(|id| c.label(id)).collect();
                let reason = format!("it made {made:?} instead of {expected:?}");
                return Err(undo(c, reason));
            }
        }
        self.record(c, before, line);
        Ok(first)
    }

    /// Builds a log and construction from a whole script. The error carries
    /// the 1-based line number like `script::load`.
    pub fn load(src: &str) -> Result<(Construction, Log), String> {
        let mut c = Construction::new();
        let mut log = Log::default();
        let mut groups = BoundaryOutputs::default();
        let grow = !has_recorded_growth(src);
        for (i, raw) in src.lines().enumerate() {
            let (body, comment) = split_comment(raw);
            let line = normalize_definition(body);
            if line.is_empty() {
                continue;
            }
            let at = |e: String| format!("line {}: {e}", i + 1);
            if let Some(d) = directive(&line) {
                let changed = log.apply(&mut c, d.map_err(at)?).map_err(at)?;
                if grow && !changed.is_empty() {
                    log.grow_boundary_outputs(&mut c, &mut groups, changed.into_iter());
                }
                continue;
            }
            check_names(&line).map_err(at)?;
            let before = c.len();
            let labels = comment
                .and_then(|comment| comment.strip_prefix(LABELS))
                .map(|labels| labels.split(',').map(str::trim).collect::<Vec<_>>());
            let first = labels
                .as_ref()
                .and_then(|labels| restore_boundary_labels(&mut c, &line, labels))
                .unwrap_or_else(|| exec_definition(&mut c, &line))
                .map_err(at)?;
            log.record(&c, before, &line);
            if let Some(id) = first.filter(|_| grow && c.len() > before) {
                groups.record(&c, &line, id, c.len() - before);
            }
        }
        Ok((c, log))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_kernel::kernel::Value;

    #[test]
    fn normalize_accepts_both_forms() {
        assert_eq!(normalize("f = Line(A, B)"), "f = Line(A, B)");
        assert_eq!(normalize("f: Line(A, B)"), "f = Line(A, B)");
        assert_eq!(normalize("  f :Line(A,B)  # note"), "f = Line(A,B)");
        assert_eq!(normalize("# only a comment"), "");
        assert_eq!(normalize("   "), "");
        assert_eq!(normalize("A = (1, 2)"), "A = (1, 2)");
        // an equation keeps its `:`, which tells the kernel it is one
        assert_eq!(normalize("a: 2x + 3y = 6"), "a: 2x + 3y = 6");
        assert_eq!(normalize(" a : y = 2x + 1 # line"), "a: y = 2x + 1");
    }

    #[test]
    fn an_equation_line_defines_its_label_and_keeps_its_equation() {
        let (c, log) = Log::load("m = 2\nb = 1\na: y = m x + b\n").unwrap();
        let a = c.id("a").unwrap();
        assert_eq!(c.kind(a), geo_kernel::Kind::Line);
        assert_eq!(log.rhs_of("a"), Some("y = m x + b"));
        let replayed = log.rebuild().unwrap();
        assert_eq!(replayed.0.value(replayed.0.id("a").unwrap()), c.value(a));
        assert_eq!(defined_labels("a: 2x + 3y = 6").collect::<Vec<_>>(), ["a"]);
    }

    fn sample() -> (Construction, Log) {
        Log::load("A = (0, 0)\nB = (2, 0)\n# c\nf: Line(A, B)\nM = Midpoint(A, B)\nC = (0, 3)\n")
            .unwrap()
    }

    #[test]
    fn load_records_normalised_entries() {
        let (c, log) = sample();
        assert_eq!(c.len(), 5);
        assert_eq!(log.entries().count(), 5);
        assert_eq!(log.entries().nth(2).unwrap().line, "f = Line(A, B)");
        assert_eq!(log.rhs_of("f"), Some("Line(A, B)"));
        assert_eq!(log.rhs_of("A"), Some("(0, 0)"));
        assert_eq!(log.rhs_of("nope"), None);
    }

    #[test]
    fn load_reports_the_line_number() {
        let err = Log::load("A = (0, 0)\n\nB = Line(A, Q)\n").err().unwrap();
        assert!(err.starts_with("line 3:"), "{err}");
    }

    #[test]
    fn removing_an_entry_cascades_through_dependents_on_rebuild() {
        let (_, mut log) = sample();
        assert!(log.remove("A"));
        let (c, kept) = log.rebuild().unwrap();
        let labels: Vec<&str> = c.ids().map(|id| c.label(id)).collect();
        assert_eq!(labels, ["B", "C"]);
        assert_eq!(kept.entries().count(), 2);
        assert!(!log.remove("zzz"));
    }

    #[test]
    fn a_line_that_fails_for_another_reason_fails_the_rebuild() {
        let (_, mut log) = sample();
        log.ops.push(Op::Define(Entry {
            labels: vec!["Z".into()],
            line: "Z = Line(A, Nope)".into(),
        }));
        let err = log.rebuild().err().unwrap();
        assert!(err.contains("Nope"), "{err}");
    }

    #[test]
    fn moves_are_replayed_in_order_and_consecutive_ones_collapse() {
        let (_, mut log) = sample();
        log.record_set("A", Setting::Point(1.0, 1.0));
        log.record_set("A", Setting::Point(2.0, 2.0));
        assert_eq!(log.ops.len(), 6);
        let (c, _) = log.rebuild().unwrap();
        let a = c.id("A").unwrap();
        assert_eq!(c.value(a), Value::Point { x: 2.0, y: 2.0 });
    }

    #[test]
    fn rebuild_of_an_unchanged_log_is_identical() {
        let (c1, log) = sample();
        let (c2, log2) = log.rebuild().unwrap();
        assert_eq!(log, log2);
        assert_eq!(c1.len(), c2.len());
    }

    #[test]
    fn the_script_of_a_log_reads_back_to_the_same_state() {
        let (mut c, mut log) = sample();
        let a = c.id("A").unwrap();
        c.move_point(a, 0.1 + 0.2, 1e-7).unwrap();
        log.record_set("A", Setting::Point(0.1 + 0.2, 1e-7));
        log.hidden.insert("M".into());
        let text = log.to_script();
        assert!(
            text.contains("SetCoords(A, 0.30000000000000004, 0.0000001)\n"),
            "{text}"
        );
        assert!(text.ends_with("SetVisibleInView(M, 1, false)\n"), "{text}");
        let (c2, log2) = Log::load(&text).unwrap();
        assert_eq!(log2, log);
        let values = |c: &Construction| c.ids().map(|id| c.value(id)).collect::<Vec<_>>();
        assert_eq!(values(&c2), values(&c));
    }

    #[test]
    fn directives_are_checked() {
        for (src, err) in [
            ("SetCoords(A, 1)\n", "line 1: cannot read `SetCoords(A, 1)`"),
            ("SetCoords(A, 1, 2)\n", "line 1: `A` is missing"),
            (
                "A = (0, 0)\nSetVisibleInView(A, 2, false)\n",
                "line 2: cannot read",
            ),
            (
                "A = (0, 0)\nB = (1, 1)\nM = Midpoint(A, B)\nSetCoords(M, 1, 2)\n",
                "line 4: `M`",
            ),
            ("n = 2\nSetValue(n, x)\n", "line 2: cannot read"),
        ] {
            let e = Log::load(src).err().unwrap();
            assert!(e.starts_with(err), "{src:?}: {e}");
        }
        let (_, log) =
            Log::load("A = (0, 0)\nSetVisibleInView(A, 1, false)\nSetVisibleInView(A, 1, true)\n")
                .unwrap();
        assert!(log.hidden.is_empty());
    }

    #[test]
    fn a_tolerant_load_keeps_what_loads_and_reports_the_rest() {
        let src = "A = (0, 0)\nQ = Nope(A)\nR = Midpoint(Q, A)\nSetCoords(Q, 1, 2)\n\
                   SetCoords(A, 3, 4)\n{T, U} = Nope(A)\nW = Segment(T, A)\nB = (1, 1)\n";
        let (c, log, rejected) = Log::load_tolerant(src, false);
        let labels: Vec<&str> = c.ids().map(|id| c.label(id)).collect();
        assert_eq!(labels, ["A", "B"]);
        assert_eq!(c.value(c.id("A").unwrap()), Value::Point { x: 3.0, y: 4.0 });
        let lines: Vec<(usize, bool)> = rejected.iter().map(|r| (r.line, r.dependent)).collect();
        assert_eq!(
            lines,
            [(2, false), (3, true), (4, true), (6, false), (7, true)]
        );
        assert!(rejected[0].reason.contains("Nope"), "{:?}", rejected[0]);
        // What was kept is what a strict load of the kept lines gives.
        let (_, strict) = Log::load(&log.to_script()).unwrap();
        assert_eq!(strict, log);
        // A text without errors gives the same as a strict load.
        let (_, all, none) = Log::load_tolerant("A = (0, 0)\nB = (1, 1)\n", true);
        assert!(none.is_empty());
        assert_eq!(all, Log::load("A = (0, 0)\nB = (1, 1)\n").unwrap().1);
    }

    #[test]
    fn the_defined_labels_of_each_form() {
        fn labels(l: &str) -> Vec<&str> {
            defined_labels(l).collect()
        }
        assert_eq!(labels("A = (1, 2)"), ["A"]);
        assert_eq!(labels("{t_{1}, t_{2}} = Tangent(P, k)"), ["t_{1}", "t_{2}"]);
        assert_eq!(labels("{, B} = Intersect(f, g)"), ["B"]);
        assert_eq!(labels("f(x) = x^2"), ["f"]);
        assert_eq!(labels("SetCoords(A, 1, 2)"), Vec::<&str>::new());
    }

    #[test]
    fn siblings_of_a_single_output_line_are_empty() {
        let (_, log) = sample();
        assert!(log.siblings("A").is_empty());
    }
}
