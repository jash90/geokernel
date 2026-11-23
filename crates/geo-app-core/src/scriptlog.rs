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
//!
//! One kind of entry is not the journal of any script line: the start
//! point a locus froze on a line, written where the entry of the locus was
//! when the locus was deleted (`Op::Freeze`). It keeps the start over
//! every later rebuild, and a script does not save it.
//!
//! A crossing revealed by a move is saved as a slot definition after that
//! move. Its exact label and its origin survive further moves and replay.
//! A simultaneous move is saved as one `$movePoints` directive.
//!
//! A deletion rebuild leaves the removal of interval roots to the next
//! update of their command and closes the operations it replayed with a
//! `$keptLabels()` line: every later replay of the journal frees no label
//! before it, so it reproduces the labels the deletion kept.

use geo_kernel::algo::measure::MeasureDef;
use geo_kernel::kernel::control::{decode_text, encode_text, visibility_boolean};
use geo_kernel::kernel::text::TextPosition;
use geo_kernel::kernel::{Construction, Def, EquationForm, Id, Kind, Value};
use geo_kernel::script::equation::{parse_style, Style as EquationStyle};
use geo_kernel::script::saved::{self, mark_inline_arguments, Property, INLINE_ARGUMENTS};
use geo_kernel::script::text::{parse_properties, Properties as TextProperties};
use geo_kernel::script::{
    boundary_slot_owner, find_unquoted, first_slot_owner, free_label_for, growth_owner,
    restore_boundary_labels, split_comment, BoundaryOutputs,
};
use shared::Chain;
pub use shared::Labels;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

mod shared;

/// The names of the directives, which no object may take.
pub const DIRECTIVES: [&str; 3] = ["SetCoords", "SetValue", "SetVisibleInView"];

/// Starts the list of labels after a definition in a snapshot.
const LABELS: &str = "# labels: ";
const MOVE_POINTS: &str = "$movePoints";
const ANGLE_STYLE: &str = "$angleStyle";
const SHOW_OBJECT: &str = "$showObject";
/// Marks the end of the part of the journal that a deletion replayed
/// without freeing the labels of command outputs (see `Op::KeptLabels`).
const KEPT_LABELS: &str = "$keptLabels";

fn snapshot_labels(comment: &str) -> Option<&str> {
    if saved::inline_argument_positions(comment).is_some() {
        comment.split_once(LABELS).map(|(_, labels)| labels)
    } else {
        comment.strip_prefix(LABELS)
    }
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
        let body = split_comment(&self.line).0;
        let rhs = body
            .find([':', '='])
            .and_then(|i| body.get(i + 1..))
            .map_or("", str::trim);
        geo_kernel::script::saved_definition(rhs).unwrap_or(rhs)
    }

    fn replay(&self, c: &mut Construction) -> Result<Option<Id>, String> {
        if BoundaryOutputs::is_command(self.rhs()) {
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
    /// The start point a locus froze on a line without one, written where
    /// the entry of the locus was when the locus was deleted
    /// ([`Log::keep_frozen_start`]): a replay installs it before the
    /// operations that follow, exactly where the locus froze it, so every
    /// later rebuild reproduces the points bit for bit. GeoGebra keeps such
    /// a start for the rest of the session (it installed the point on the
    /// line itself); this entry is what carries it over the rebuilds. A
    /// script does not save it (see `write_script`).
    Freeze {
        label: String,
        x: f64,
        y: f64,
    },
    MovePoints(Vec<Position<String>>),
    /// The end of the operations a deletion replayed: the outputs that the
    /// live session kept labelled through them keep their labels in every
    /// replay of them, and lose them only at a later update of their
    /// command, as in the reference. A replay frees a label there only for
    /// a line that takes it.
    KeptLabels,
    TextProperties(Box<str>),
    SavedProperty(Box<Property>),
    EquationStyle {
        label: String,
        form: EquationForm,
    },
}

/// Rewrites `f: Line(A, B)` as `f = Line(A, B)` and removes a trailing `#`
/// comment. Blank input gives an empty string. A `:` counts only when it comes
/// before any `=`; an equation (`f: 2x + 3y = 6`, whose `=` is part of the
/// definition) keeps its `:`, which the script of the kernel reads.
pub fn normalize(input: &str) -> String {
    let (body, comment) = split_comment(input);
    normalize_saved_definition(body, comment)
}

fn normalize_saved_definition(body: &str, comment: Option<&str>) -> String {
    let mut line = normalize_definition(body);
    if let Some(positions) = comment.and_then(saved::inline_argument_positions) {
        use std::fmt::Write as _;
        let _ = write!(line, " {INLINE_ARGUMENTS}{positions}");
    }
    line
}

fn normalize_definition(input: &str) -> String {
    let line = input.trim();
    let colon = find_unquoted(line, ':');
    let eq = find_unquoted(line, '=');
    match (colon, eq) {
        (Some(c), None) => format!("{} = {}", line[..c].trim(), line[c + 1..].trim()),
        (Some(c), Some(e)) if c + 1 == e => {
            format!("{} = {}", line[..c].trim(), line[e + 1..].trim())
        }
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
    TextProperties(TextProperties<'a>),
    SavedProperty(Property),
    EquationStyle(EquationStyle<'a>),
    KeptLabels,
}

/// The directive on a normalised line, if it is one (a definition always has
/// an `=`).
fn directive(line: &str) -> Option<Result<Directive<'_>, String>> {
    if let Some(property) = saved::parse(line) {
        return Some(property.map(Directive::SavedProperty));
    }
    if let Some(props) = parse_properties(line) {
        return Some(props.map(Directive::TextProperties));
    }
    if let Some(style) = parse_style(line) {
        return Some(style.map(Directive::EquationStyle));
    }
    if line.contains('=') {
        return None;
    }
    let (name, rest) = line.split_once('(')?;
    let name = name.trim();
    if !matches!(name, MOVE_POINTS | ANGLE_STYLE | SHOW_OBJECT | KEPT_LABELS)
        && !DIRECTIVES.contains(&name)
    {
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
            (KEPT_LABELS, [""]) => Some(Directive::KeptLabels),
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

fn replay_text_properties<'a>(
    c: &mut Construction,
    line: &'a str,
    gone: &BTreeSet<String>,
    previous: Option<&Construction>,
) -> Result<Option<Cow<'a, str>>, String> {
    let mut properties = parse_properties(line).expect("a recorded text directive")?;
    if gone.contains(properties.label) && c.id(properties.label).is_none() {
        return Ok(None);
    }
    let anchor = match properties.overrides.position {
        Some(TextPosition::Point(label) | TextPosition::ScreenPoint(label)) => Some(label),
        _ => None,
    };
    let detached = anchor.filter(|&label| gone.contains(label) && c.id(label).is_none());
    if let Some(label) = detached {
        let saved = previous.and_then(|prior| prior.id(label).map(|id| prior.value(id)));
        properties.overrides.position = Some(match saved {
            Some(Value::Point { x, y })
                if matches!(
                    properties.overrides.position,
                    Some(TextPosition::ScreenPoint(_))
                ) =>
            {
                TextPosition::Screen {
                    x: f64::from(x as i32),
                    y: f64::from(y as i32),
                }
            }
            Some(Value::Point { x, y }) => TextPosition::World { x, y },
            _ => TextPosition::Origin,
        });
    }
    properties.apply(c)?;
    Ok(Some(if detached.is_some() {
        Cow::Owned(properties.to_string())
    } else {
        Cow::Borrowed(line)
    }))
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
/// The index of the last `$keptLabels()` line of `src`, before which a
/// replay frees no label of a command output (see `Op::KeptLabels`); 0 if
/// there is none.
fn kept_labels_end(src: &str) -> usize {
    src.lines()
        .enumerate()
        .filter(|(_, raw)| {
            split_comment(raw)
                .0
                .trim()
                .strip_prefix(KEPT_LABELS)
                .is_some_and(|rest| rest.trim() == "()")
        })
        .last()
        .map_or(0, |(i, _)| i)
}

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
    split_comment(line)
        .0
        .split(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '{' | '}' | '\'')))
        .filter(|t| !t.is_empty())
}

fn uses_removed_object(entry: &Entry, gone: &BTreeSet<String>) -> bool {
    tokens(&entry.line).any(|token| gone.contains(token))
        || geo_kernel::script::image::corner_sources(entry.rhs())
            .any(|source| tokens(&source).any(|token| gone.contains(token)))
}

/// An identity to use until the active definition's later inputs exist.
fn replacement_seed(c: &Construction, id: Id) -> Option<String> {
    use crate::describe::exact;

    let label = c.label(id);
    let coordinates = match c.value(id) {
        Value::Point { x, y } | Value::Vector { x, y } => (exact(x), exact(y)),
        _ => ("0".into(), "0".into()),
    };
    let (x, y) = coordinates;
    let rhs = match c.kind(id) {
        Kind::Point => format!("$point({x}, {y})"),
        Kind::Vector => format!("$vector({x}, {y})"),
        Kind::Number => match c.value(id) {
            Value::Number(n) if n.is_finite() => exact(n),
            _ => "0".into(),
        },
        Kind::Angle => "$saved(0 rad, 0)".into(),
        Kind::Boolean => matches!(c.value(id), Value::Bool(true)).to_string(),
        Kind::Text => "\"\"".into(),
        Kind::List => "{}".into(),
        Kind::Function => return Some(format!("{label}(x) = x")),
        Kind::FunctionNVar => return Some(format!("{label}(x, y) = x + y")),
        _ => return None,
    };
    Some(format!("{label} = {rhs}"))
}

/// Internal dependencies have a kernel id but no user-visible label.
pub(crate) fn object_ids(c: &Construction) -> impl Iterator<Item = Id> + '_ {
    outputs(c, 0)
}

fn outputs(c: &Construction, from: usize) -> impl Iterator<Item = Id> + '_ {
    c.ids().skip(from).filter(|&id| !is_internal(c, id))
}

fn definition_outputs<'a>(
    c: &'a Construction,
    from: usize,
    first: Option<Id>,
    line: &str,
) -> impl Iterator<Item = Id> + 'a {
    let target = first
        .filter(|&id| id.index() < from)
        .and_then(|_| geo_kernel::script::assignment_target(c, line));
    first
        .filter(|&id| target == Some(id) && !is_internal(c, id))
        .into_iter()
        .chain(outputs(c, from))
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

    pub(crate) fn condition_source(&self, label: &str) -> Option<&str> {
        self.conditions
            .get(label)
            .map(|condition| condition.source.as_str())
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
    pub(crate) fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.ops.items().into_iter().filter_map(|op| match op {
            Op::Define(e) => Some(e),
            _ => None,
        })
    }

    pub(crate) fn saved_properties(&self) -> impl Iterator<Item = &Property> {
        self.ops.items().into_iter().filter_map(|op| match op {
            Op::SavedProperty(property) => Some(property.as_ref()),
            _ => None,
        })
    }

    /// The accepted lines, newest first, without collecting them.
    pub(crate) fn entries_rev(&self) -> impl Iterator<Item = &Entry> {
        self.ops.items_rev().filter_map(|op| match op {
            Op::Define(e) => Some(e),
            _ => None,
        })
    }

    /// Records new objects, a slider assignment or a protocol step.
    pub fn record(&mut self, c: &Construction, from: usize, line: &str) {
        self.record_definition(c, from, None, line);
    }

    pub(crate) fn record_definition(
        &mut self,
        c: &Construction,
        from: usize,
        first: Option<Id>,
        line: &str,
    ) {
        let labels: Vec<String> = definition_outputs(c, from, first, line)
            .map(|id| c.label(id).to_owned())
            .collect();
        if !labels.is_empty()
            || geo_kernel::script::is_construction_step_command(line)
            || geo_kernel::script::is_slider_assignment(c, line)
        {
            self.ops.push(Op::Define(Entry {
                labels,
                line: mark_inline_arguments(line.to_owned()),
            }));
        }
    }

    /// Records a new value of a free object.
    pub fn record_set(&mut self, label: &str, to: Setting) {
        self.push_set(label, to, false);
    }

    pub(crate) fn record_update(&mut self, c: &Construction, label: &str, to: Setting) {
        self.push_set(label, to, c.last_update_requires_replay());
    }

    pub(crate) fn reset_snapshot_records(&mut self, c: &mut Construction) {
        if c.reset_circle_records() {
            self.ops
                .push(Op::SavedProperty(Box::new(Property::ResetCircleRecords)));
        }
        if c.reset_arc_records()
            && !matches!(self.ops.last(), Some(Op::SavedProperty(p)) if **p == Property::ResetArcRecords)
        {
            self.ops
                .push(Op::SavedProperty(Box::new(Property::ResetArcRecords)));
        }
    }

    fn push_set(&mut self, label: &str, to: Setting, ordered: bool) {
        let set = Op::Set {
            label: label.to_owned(),
            to,
            ordered,
        };
        // A later edit can retain an intermediate geometric record or
        // a hidden Newton start point, so replay the edit that changed it.
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

    pub(crate) fn record_text_properties(&mut self, props: TextProperties<'_>) {
        let op = Op::TextProperties(props.to_string().into());
        // Coalesce successive changes to the same text properties, as
        // during a drag. A different property still needs its own edit.
        let replaces = match self.ops.last() {
            Some(Op::TextProperties(previous)) => parse_properties(previous)
                .and_then(Result::ok)
                .is_some_and(|old| {
                    old.label == props.label
                        && old.overrides.position.is_some() == props.overrides.position.is_some()
                        && old.overrides.offset.is_some() == props.overrides.offset.is_some()
                        && old.overrides.latex.is_some() == props.overrides.latex.is_some()
                        && old.overrides.font.is_some() == props.overrides.font.is_some()
                }),
            _ => false,
        };
        if replaces {
            self.ops.replace_last(op);
        } else {
            self.ops.push(op);
        }
    }

    pub(crate) fn boundary_outputs(&self, c: &Construction) -> BoundaryOutputs {
        let entries: Vec<_> = self
            .entries_rev()
            .filter(|e| {
                BoundaryOutputs::is_command(e.rhs()) || boundary_slot_owner(e.rhs()).is_some()
            })
            .collect();
        // A first output whose label was freed and given again is named by
        // the line that gave it.
        let renamed: BTreeMap<&str, &str> = entries
            .iter()
            .filter_map(|e| Some((first_slot_owner(e.rhs())?, e.labels.first()?.as_str())))
            .collect();
        let mut groups = BoundaryOutputs::default();
        for e in entries.into_iter().rev() {
            let Some(mut label) = e.labels.first().map(String::as_str) else {
                continue;
            };
            if BoundaryOutputs::is_command(e.rhs()) {
                for _ in 0..renamed.len() {
                    match renamed.get(label) {
                        Some(next) => label = next,
                        None => break,
                    }
                }
            }
            if let Some(first) = growth_owner(c, label) {
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
    ) -> bool {
        let mut recorded = false;
        let released = groups.grow_after(c, changed, |c, id, line| {
            recorded = true;
            // A freed slot labelled again is an older object.
            self.record_definition(c, c.len(), Some(id), line);
        });
        if released {
            // A later set must not replace the one that freed the labels.
            self.keep_last_set();
        }
        released || recorded
    }

    pub(crate) fn grow_after_step(
        &mut self,
        c: &mut Construction,
        groups: &mut BoundaryOutputs,
    ) -> bool {
        let mut recorded = false;
        let released = groups.grow_after_step(c, |c, id, line| {
            recorded = true;
            self.record_definition(c, c.len(), Some(id), line);
        });
        released || recorded
    }

    fn keep_last_set(&mut self) {
        if let Some(Op::Set {
            label,
            to,
            ordered: false,
        }) = self.ops.last()
        {
            let set = Op::Set {
                label: label.clone(),
                to: *to,
                ordered: true,
            };
            self.ops.replace_last(set);
        }
    }

    pub(crate) fn grown_groups(&self) -> Vec<GrownOutputs<'_>> {
        let mut births: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        let mut groups = Vec::new();
        for e in self.entries_rev() {
            if let Some(owner) = boundary_slot_owner(e.rhs()) {
                // The slots born under the label a first output takes again
                // belong to the label it had.
                let later = first_slot_owner(e.rhs())
                    .and(e.labels.first())
                    .and_then(|label| births.remove(label.as_str()))
                    .unwrap_or_default();
                let born = births.entry(owner).or_default();
                born.extend(later);
                born.extend(e.labels.iter().rev().cloned());
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
            self.entries_rev()
                .find(|entry| entry.labels.iter().any(|label| label == name))
                .map(Entry::rhs)
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
    /// was such a line. Lines that use a removed object go on the next
    /// rebuild, which also drops the start points those lines froze.
    pub fn remove(&mut self, label: &str, c: &Construction) -> bool {
        let Some(entry) = self
            .entries_rev()
            .find(|e| e.labels.iter().any(|l| l == label))
        else {
            return false;
        };
        let appended_list_point = boundary_slot_owner(entry.rhs()).is_some()
            && c.id(label).is_some_and(|id| {
                matches!(
                    c.def(id),
                    Def::Point(geo_kernel::algo::point::PointDef::Coordinates { .. })
                )
            });
        let grown = (!appended_list_point
            && (BoundaryOutputs::is_command(entry.rhs())
                || boundary_slot_owner(entry.rhs()).is_some()))
        .then(|| {
            self.grown_groups()
                .into_iter()
                .find(|group| group.labels.iter().any(|member| member == label))
        })
        .flatten();
        let mut labels: BTreeSet<_> = grown
            .map_or_else(|| entry.labels.clone(), |group| group.labels)
            .into_iter()
            .collect();
        let mut defined = BTreeSet::new();
        let repeated: BTreeSet<_> = self
            .entries()
            .flat_map(|entry| entry.labels.iter().map(String::as_str))
            .filter(|&label| !defined.insert(label))
            .collect();
        if !repeated.is_empty() {
            // Follow the current graph before replay can see a superseded rule.
            let mut pending: Vec<_> = labels.iter().filter_map(|label| c.id(label)).collect();
            let mut visited = BTreeSet::new();
            let mut remaining = 1_000_000usize;
            while let Some(id) = pending.pop() {
                if !visited.insert(id) {
                    continue;
                }
                labels.insert(c.label(id).to_owned());
                for child in c.direct_dependents(id) {
                    let Some(next) = remaining.checked_sub(1) else {
                        return false;
                    };
                    remaining = next;
                    pending.push(child);
                }
            }
            if let Some(compacted) = self.compact_removed_inputs(c, &labels, &repeated) {
                self.ops = compacted;
            }
        }
        self.ops.retain(|op| match op {
            Op::Define(e) => !e.labels.iter().any(|label| labels.contains(label)),
            Op::Set { label, .. } | Op::EquationStyle { label, .. } => !labels.contains(label),
            Op::SavedProperty(property) => {
                !property.label().is_some_and(|label| labels.contains(label))
            }
            Op::Freeze { .. } | Op::MovePoints(_) | Op::TextProperties(_) | Op::KeptLabels => true,
        });
        self.removed.extend(labels);
        true
    }

    /// Replace superseded lines whose inputs will be deleted. Install the
    /// active line at the first broken entry when its inputs exist there.
    /// Otherwise keep the original position with a seed and replay the
    /// active assignment where its inputs were originally available.
    fn compact_removed_inputs(
        &self,
        c: &Construction,
        gone: &BTreeSet<String>,
        repeated: &BTreeSet<&str>,
    ) -> Option<Chain<Op>> {
        let ops = self.ops.items();
        let mut active = BTreeMap::new();
        let mut broken = BTreeMap::new();
        for (at, op) in ops.iter().enumerate() {
            if let Op::Define(entry) = op {
                if let [label] = entry.labels.as_slice() {
                    if repeated.contains(label.as_str()) && !gone.contains(label) {
                        active.insert(label.as_str(), (at, entry));
                        if uses_removed_object(entry, gone) {
                            broken.entry(label.as_str()).or_insert(at);
                        }
                    }
                }
            }
        }
        broken.retain(|label, first| *first < active[label].0);
        let start = broken.values().copied().min()?;
        let mut kept = self.ops.prefix(start);
        let mut born: BTreeSet<&str> = ops[..start]
            .iter()
            .filter_map(|op| match op {
                Op::Define(entry) => Some(entry.labels.iter().map(String::as_str)),
                _ => None,
            })
            .flatten()
            .filter(|label| !gone.contains(*label))
            .collect();
        let mut moved = BTreeSet::new();
        let superseded = |label: &str, at| {
            broken
                .get(label)
                .is_some_and(|&first| first <= at && at < active[label].0)
        };
        for (at, &op) in ops.iter().enumerate().skip(start) {
            let next = match op {
                Op::Define(entry) => {
                    if let [label] = entry.labels.as_slice() {
                        let label = label.as_str();
                        if broken.get(label) == Some(&at) {
                            let current = active[label].1;
                            let ready = tokens(current.rhs()).all(|token| {
                                token == label
                                    || born.contains(token)
                                    || c.id(token).is_none_or(|id| id.index() >= c.len())
                            });
                            if ready {
                                kept.push(Op::Define(current.clone()));
                                moved.insert(label);
                            } else if !born.contains(label) {
                                let line = c.id(label).and_then(|id| replacement_seed(c, id))?;
                                kept.push(Op::Define(Entry {
                                    labels: entry.labels.clone(),
                                    line,
                                }));
                            }
                            born.insert(label);
                            continue;
                        }
                        if superseded(label, at) || moved.contains(label) {
                            continue;
                        }
                    }
                    if !entry.labels.iter().any(|label| gone.contains(label)) {
                        born.extend(entry.labels.iter().map(String::as_str));
                    }
                    op.clone()
                }
                Op::Set { label, to, .. }
                    if superseded(label, at) && !matches!(to, Setting::AngleStyle(_)) =>
                {
                    continue
                }
                Op::MovePoints(points) => Op::MovePoints(
                    points
                        .iter()
                        .filter(|(label, _, _)| !superseded(label, at))
                        .cloned()
                        .collect(),
                ),
                _ => op.clone(),
            };
            kept.push(next);
        }
        Some(kept)
    }

    /// Writes the start point a deleted locus froze on `line` into the
    /// journal at `at`, where the entry of the locus was. A replay installs
    /// it before the operations that follow — exactly where the locus
    /// froze it, so moves recorded before it replay from the live foot of
    /// the line and moves after it from the frozen start — and every later
    /// rebuild (further deletions, the undo and redo of them) reproduces
    /// the points bit for bit. GeoGebra keeps such a start when the locus
    /// that installed it is deleted, because it lives on the line; this
    /// entry is what makes it survive the rebuilds.
    pub(crate) fn keep_frozen_start(&mut self, at: usize, line: &str, x: f64, y: f64) {
        self.ops.insert(
            at,
            Op::Freeze {
                label: line.to_owned(),
                x,
                y,
            },
        );
    }

    /// The journal position of the active definition of `label`, with every
    /// label that entry made: the position a rebuild has to place at what
    /// that entry froze, before the operations that follow it.
    pub(crate) fn define_entry(&self, label: &str) -> Option<(usize, &[String])> {
        let ops = self.ops.items();
        let at = ops
            .iter()
            .rposition(|op| matches!(op, Op::Define(e) if e.labels.iter().any(|l| l == label)))?;
        match &ops[at] {
            Op::Define(e) => Some((at, &e.labels[..])),
            _ => None,
        }
    }

    /// Replays the journal on a fresh construction. Every line must create
    /// the labels it created before; a line that fails is dropped only when
    /// it uses an object that was removed (or dropped itself), otherwise the
    /// whole rebuild fails and the caller keeps its previous state. The
    /// start points a deleted locus froze (`Op::Freeze`) are installed
    /// where their entries are, before the operations that follow them.
    pub fn rebuild(&self) -> Result<(Construction, Log), String> {
        self.rebuild_with_text_anchors(None)
    }

    /// When a placed text loses its anchor, keep the last location.
    pub(crate) fn rebuild_with_text_anchors(
        &self,
        previous: Option<&Construction>,
    ) -> Result<(Construction, Log), String> {
        let mut c = Construction::new();
        let mut kept = Log::default();
        let mut gone = self.removed.clone();
        // The journal gives every label a slot takes; the freed labels
        // follow the edits again. A deletion leaves freeing to the next
        // live update, as the reference frees an output only when its
        // command updates: the replay frees a label only for a line that
        // takes it, which the live session freed first.
        let deleting = !gone.is_empty();
        let defer_until = if deleting {
            usize::MAX
        } else {
            self.ops
                .items()
                .iter()
                .rposition(|op| matches!(op, Op::KeptLabels))
                .unwrap_or(0)
        };
        let mut groups = BoundaryOutputs::default();
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
            let deferring = i < defer_until;
            groups.set_deferred(deferring);
            match op {
                Op::KeptLabels => match &mut same {
                    Some(n) => *n = i + 1,
                    None => kept.ops.push(Op::KeptLabels),
                },
                Op::Define(e) => {
                    if deferring {
                        for label in &e.labels {
                            free_label_for(&mut c, label);
                        }
                    }
                    let before = c.len();
                    match e.replay(&mut c) {
                        Ok(first) => {
                            let made: Vec<&str> = definition_outputs(&c, before, first, &e.line)
                                .map(|id| c.label(id))
                                .collect();
                            if made != e.labels {
                                return Err(format!(
                                    "`{}` created {made:?} instead of {:?}",
                                    e.line, e.labels
                                ));
                            }
                            if let Some(id) = first {
                                groups.record(&c, &e.line, id, c.len() - before);
                                if id.index() < before {
                                    groups.release_after(&mut c, std::iter::once(id));
                                }
                            }
                            // What `record` would make of it is `e` itself.
                            match &mut same {
                                Some(n) => *n = i + 1,
                                None => kept.record_definition(&c, before, first, &e.line),
                            }
                        }
                        Err(err) => {
                            if c.len() != before || !uses_removed_object(e, &gone) {
                                return Err(format!("`{}`: {err}", e.line));
                            }
                            gone.extend(
                                e.labels
                                    .iter()
                                    .filter(|label| c.id(label).is_none())
                                    .cloned(),
                            );
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
                    groups.release_after(&mut c, std::iter::once(id));
                    // A set right after a set of the same object is merged
                    // into it by `record_set` (after a `remove` they can
                    // meet): the journal and `kept` part there.
                    let keep_order = *ordered || c.last_update_requires_replay();
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
                    groups.release_after(&mut c, positions.iter().map(|&(id, _, _)| id));
                    match &mut same {
                        Some(n) if positions.len() == points.len() => *n = i + 1,
                        _ => {
                            diverge(&mut kept, &mut same);
                            kept.record_move_points(&c, &positions);
                        }
                    }
                }
                Op::TextProperties(line) => {
                    let replayed = replay_text_properties(&mut c, line, &gone, previous)?;
                    if matches!(replayed, Some(Cow::Borrowed(_))) {
                        if let Some(n) = &mut same {
                            *n = i + 1;
                            continue;
                        }
                    }
                    diverge(&mut kept, &mut same);
                    if let Some(line) = replayed {
                        kept.ops.push(Op::TextProperties(line.into()));
                    }
                }
                Op::SavedProperty(property) => {
                    if property.label().is_none_or(|label| !gone.contains(label))
                        && property.apply(&mut c).is_ok()
                    {
                        match &mut same {
                            Some(n) => *n = i + 1,
                            None => kept.ops.push((*op).clone()),
                        }
                    } else {
                        diverge(&mut kept, &mut same);
                        if property.label().is_none_or(|label| !gone.contains(label)) {
                            if let Some(property) = property.as_ref().clone().restore(&mut c).0 {
                                kept.ops.push(Op::SavedProperty(Box::new(property)));
                            }
                        }
                    }
                }
                Op::EquationStyle { label, form } => {
                    if gone.contains(label) && c.id(label).is_none() {
                        diverge(&mut kept, &mut same);
                        continue;
                    }
                    EquationStyle {
                        label,
                        form: form.clone(),
                    }
                    .apply(&mut c)?;
                    match &mut same {
                        Some(n) => *n = i + 1,
                        None => kept.ops.push((*op).clone()),
                    }
                }
                Op::Freeze { label, x, y } => {
                    if c.set_frozen_line_start(label, *x, *y) {
                        // What `keep_frozen_start` wrote is the entry itself.
                        match &mut same {
                            Some(n) => *n = i + 1,
                            None => kept.ops.push(Op::Freeze {
                                label: label.clone(),
                                x: *x,
                                y: *y,
                            }),
                        }
                    } else if c.id(label).is_some() || gone.contains(label) {
                        // The line was deleted, with everything that used
                        // it, or no longer takes a frozen start: the entry
                        // goes with it.
                        diverge(&mut kept, &mut same);
                    } else {
                        return Err(format!("`{label}` is missing"));
                    }
                }
            }
        }
        if let Some(n) = same {
            kept.ops = self.ops.prefix(n);
        }
        if deleting && groups.frees_labels(&c) && !matches!(kept.ops.last(), Some(Op::KeptLabels)) {
            kept.ops.push(Op::KeptLabels);
        }
        kept.hidden = self.hidden.clone();
        // A replay keeps the visibility of the journal, whatever the
        // commands it runs asked for.
        c.take_hidden_requests();
        self.replay_conditions(&mut c, &mut kept);
        kept.prune_image_assets(&mut c);
        Ok((c, kept))
    }

    /// Hides the objects the commands of the line just run asked to hide
    /// (`SlowPlot` hides its source graph); hidden helpers have no row.
    pub(crate) fn apply_hidden_requests(&mut self, c: &mut Construction) {
        for id in c.take_hidden_requests() {
            let label = c.label(id);
            if !label.starts_with('\0') && c.id(label) == Some(id) {
                self.hidden.insert(label.to_owned());
            }
        }
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
    pub(crate) fn rebuild_reference(
        &self,
        previous: &Construction,
    ) -> Result<(Construction, Log), String> {
        let mut c = Construction::new();
        let mut kept = Log::default();
        let mut gone = self.removed.clone();
        // The journal gives every label a slot takes; the freed labels
        // follow the edits again. A deletion leaves freeing to the next
        // live update, as the reference frees an output only when its
        // command updates: the replay frees a label only for a line that
        // takes it, which the live session freed first.
        let deleting = !gone.is_empty();
        let defer_until = if deleting {
            usize::MAX
        } else {
            self.ops
                .items()
                .iter()
                .rposition(|op| matches!(op, Op::KeptLabels))
                .unwrap_or(0)
        };
        let mut groups = BoundaryOutputs::default();
        for (i, op) in self.ops.items().into_iter().enumerate() {
            let deferring = i < defer_until;
            groups.set_deferred(deferring);
            match op {
                Op::KeptLabels => kept.ops.push(Op::KeptLabels),
                Op::Define(e) => {
                    if deferring {
                        for label in &e.labels {
                            free_label_for(&mut c, label);
                        }
                    }
                    let before = c.len();
                    match e.replay(&mut c) {
                        Ok(first) => {
                            let made: Vec<&str> = definition_outputs(&c, before, first, &e.line)
                                .map(|id| c.label(id))
                                .collect();
                            if made != e.labels {
                                return Err(format!(
                                    "`{}` created {made:?} instead of {:?}",
                                    e.line, e.labels
                                ));
                            }
                            if let Some(id) = first {
                                groups.record(&c, &e.line, id, c.len() - before);
                                if id.index() < before {
                                    groups.release_after(&mut c, std::iter::once(id));
                                }
                            }
                            kept.record_definition(&c, before, first, &e.line);
                        }
                        Err(err) => {
                            if c.len() != before || !uses_removed_object(e, &gone) {
                                return Err(format!("`{}`: {err}", e.line));
                            }
                            gone.extend(
                                e.labels
                                    .iter()
                                    .filter(|label| c.id(label).is_none())
                                    .cloned(),
                            );
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
                    groups.release_after(&mut c, std::iter::once(id));
                    kept.push_set(label, *to, *ordered || c.last_update_requires_replay());
                }
                Op::MovePoints(points) => {
                    let positions = replay_move_points(&mut c, points, Some(&gone))?;
                    groups.release_after(&mut c, positions.iter().map(|&(id, _, _)| id));
                    kept.record_move_points(&c, &positions);
                }
                Op::TextProperties(line) => {
                    if let Some(line) = replay_text_properties(&mut c, line, &gone, Some(previous))?
                    {
                        kept.ops.push(Op::TextProperties(line.into()));
                    }
                }
                Op::SavedProperty(property) => {
                    if property.label().is_none_or(|label| !gone.contains(label)) {
                        if let Some(property) = property.as_ref().clone().restore(&mut c).0 {
                            kept.ops.push(Op::SavedProperty(Box::new(property)));
                        }
                    }
                }
                Op::EquationStyle { label, form } => {
                    if !gone.contains(label) || c.id(label).is_some() {
                        EquationStyle {
                            label,
                            form: form.clone(),
                        }
                        .apply(&mut c)?;
                        kept.ops.push(op.clone());
                    }
                }
                Op::Freeze { label, x, y } => {
                    if c.set_frozen_line_start(label, *x, *y) {
                        kept.ops.push(Op::Freeze {
                            label: label.clone(),
                            x: *x,
                            y: *y,
                        });
                    } else if c.id(label).is_none() && !gone.contains(label) {
                        return Err(format!("`{label}` is missing"));
                    }
                }
            }
        }
        if deleting && groups.frees_labels(&c) && !matches!(kept.ops.last(), Some(Op::KeptLabels)) {
            kept.ops.push(Op::KeptLabels);
        }
        kept.hidden = self.hidden.clone();
        self.replay_conditions(&mut c, &mut kept);
        kept.prune_image_assets(&mut c);
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

    fn prune_image_assets(&mut self, c: &mut Construction) {
        let before = c.image_asset_count();
        c.prune_image_assets();
        if c.image_asset_count() != before {
            self.ops.retain(|op| match op {
                Op::SavedProperty(property) => match property.as_ref() {
                    Property::ImageAsset { name, .. } => c.has_image_asset(name),
                    _ => true,
                },
                _ => true,
            });
        }
    }

    fn record_asset(&mut self, asset: Option<Property>) {
        if let Some(asset) = asset {
            self.ops.push(Op::SavedProperty(Box::new(asset)));
        }
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
                // A file does not save the start a locus froze (GeoGebra
                // does not either): after a reload the line has no start
                // until the next locus computation installs a fresh one,
                // on the line as it is then.
                Op::Freeze { .. } => Ok(()),
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
                Op::KeptLabels => writeln!(out, "{KEPT_LABELS}()"),
                Op::TextProperties(line) => writeln!(out, "{line}"),
                Op::SavedProperty(property) => writeln!(out, "{property}"),
                Op::EquationStyle { label, form } => {
                    writeln!(
                        out,
                        "{}",
                        EquationStyle {
                            label,
                            form: form.clone()
                        }
                    )
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
            Directive::SavedProperty(property) => {
                property.apply(c)?;
                self.ops.push(Op::SavedProperty(Box::new(property)));
                return Ok(Vec::new());
            }
            Directive::TextProperties(props) => {
                props.apply(c)?;
                self.ops.push(Op::TextProperties(props.to_string().into()));
                return Ok(Vec::new());
            }
            Directive::KeptLabels => {
                self.ops.push(Op::KeptLabels);
                return Ok(Vec::new());
            }
            Directive::EquationStyle(style) => {
                let label = style.label.to_owned();
                let form = style.form.clone();
                style.apply(c)?;
                self.ops.push(Op::EquationStyle { label, form });
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
        let mut groups = BoundaryOutputs::for_script(src);
        let grow = !checked;
        let mut rejected = Vec::new();
        let mut gone: BTreeSet<String> = BTreeSet::new();
        let defer_until = kept_labels_end(src);
        for (i, raw) in src.lines().enumerate() {
            groups.set_deferred(i < defer_until);
            if let Some(asset) = saved::load_image_asset(&mut c, raw) {
                match asset {
                    Ok(asset) => log.record_asset(asset),
                    Err(reason) => rejected.push(Rejected {
                        line: i + 1,
                        text: "$imageAsset(…)".into(),
                        reason,
                        dependent: false,
                    }),
                }
                continue;
            }
            let (body, comment) = split_comment(raw);
            let listed = comment.and_then(snapshot_labels);
            let line = normalize_saved_definition(body, comment);
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
                None if is_directive
                    || geo_kernel::script::is_construction_step_command(&line)
                    || geo_kernel::script::is_slider_assignment(&c, &line) =>
                {
                    Vec::new()
                }
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
                        } else if !changed.is_empty() {
                            groups.release_after(&mut c, changed.into_iter());
                        }
                        Ok(())
                    }),
                    None => check_names(&line).and_then(|()| {
                        if i < defer_until {
                            for label in &named {
                                free_label_for(&mut c, label);
                            }
                        }
                        let expected = (checked || listed.is_some()).then_some(named.as_slice());
                        let before = c.len();
                        let step_before = c.step();
                        let slider = geo_kernel::script::slider_assignment_target(&c, &line);
                        let first = log.define_checked(&mut c, &line, expected)?;
                        if let Some(id) = first {
                            groups.record(&c, &line, id, c.len() - before);
                        }
                        if grow && c.step() != step_before {
                            log.grow_after_step(&mut c, &mut groups);
                        }
                        match slider {
                            Some(id) if grow => {
                                log.grow_boundary_outputs(&mut c, &mut groups, std::iter::once(id));
                            }
                            Some(id) => {
                                groups.release_after(&mut c, std::iter::once(id));
                            }
                            None => {}
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
        log.prune_image_assets(&mut c);
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
        // Older snapshots could have annotated a bare read with its label.
        if let Some(id) = c.id(split_comment(line).0.trim()) {
            return Ok(Some(id));
        }
        // Validate the expected label before changing an existing definition.
        if let Some(expected) = expected {
            if geo_kernel::script::is_slider_assignment(c, line) {
                if !expected.is_empty() {
                    return Err("a slider assignment creates no objects".into());
                }
            } else if let Some(id) = geo_kernel::script::assignment_target(c, line) {
                let label = c.label(id);
                if expected != [label] {
                    return Err(format!(
                        "it would redefine {label:?} instead of creating {expected:?}"
                    ));
                }
            }
        }
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
        let first = match result {
            Ok(first) => first,
            Err(reason) => return Err(undo(c, reason)),
        };
        if let Some(expected) = expected {
            if !definition_outputs(c, before, first, line)
                .map(|id| c.label(id))
                .eq(expected.iter().copied())
            {
                let made: Vec<&str> = definition_outputs(c, before, first, line)
                    .map(|id| c.label(id))
                    .collect();
                let reason = format!("it made {made:?} instead of {expected:?}");
                return Err(undo(c, reason));
            }
        }
        self.record_definition(c, before, first, line);
        Ok(first)
    }

    /// Builds a log and construction from a whole script. The error carries
    /// the 1-based line number like `script::load`.
    pub fn load(src: &str) -> Result<(Construction, Log), String> {
        Self::load_with_assets(src, &[])
    }

    pub(crate) fn load_with_assets(
        src: &str,
        assets: &[Property],
    ) -> Result<(Construction, Log), String> {
        let mut c = Construction::new();
        let mut log = Log::default();
        for asset in assets {
            asset.apply(&mut c)?;
            log.record_asset(Some(asset.clone()));
        }
        let mut groups = BoundaryOutputs::for_script(src);
        // Growth during replay stops before the slots that the script
        // records itself (`BoundaryOutputs::for_script`).
        let grow = true;
        let defer_until = kept_labels_end(src);
        for (i, raw) in src.lines().enumerate() {
            groups.set_deferred(i < defer_until);
            let at = |e: String| format!("line {}: {e}", i + 1);
            if let Some(asset) = saved::load_image_asset(&mut c, raw) {
                log.record_asset(asset.map_err(at)?);
                continue;
            }
            let (body, comment) = split_comment(raw);
            let line = normalize_saved_definition(body, comment);
            if line.is_empty() {
                continue;
            }
            if let Some(d) = directive(&line) {
                let changed = log.apply(&mut c, d.map_err(at)?).map_err(at)?;
                if grow && !changed.is_empty() {
                    log.grow_boundary_outputs(&mut c, &mut groups, changed.into_iter());
                }
                continue;
            }
            check_names(&line).map_err(at)?;
            let before = c.len();
            let step_before = c.step();
            let labels = comment
                .and_then(snapshot_labels)
                .map(|labels| labels.split(',').map(str::trim).collect::<Vec<_>>());
            let slider = geo_kernel::script::slider_assignment_target(&c, &line);
            if i < defer_until {
                let named: Vec<&str> = match &labels {
                    Some(labels) => labels.clone(),
                    None => defined_labels(&line).collect(),
                };
                for label in named {
                    free_label_for(&mut c, label);
                }
            }
            let first = labels
                .as_ref()
                .and_then(|labels| restore_boundary_labels(&mut c, &line, labels))
                .unwrap_or_else(|| exec_definition(&mut c, &line))
                .map_err(at)?;
            log.record_definition(&c, before, first, &line);
            log.apply_hidden_requests(&mut c);
            if let Some(id) = first.filter(|_| grow) {
                groups.record(&c, &line, id, c.len() - before);
            }
            if grow && c.step() != step_before {
                log.grow_after_step(&mut c, &mut groups);
            }
            if let Some(id) = slider.filter(|_| grow) {
                log.grow_boundary_outputs(&mut c, &mut groups, std::iter::once(id));
            }
        }
        log.prune_image_assets(&mut c);
        Ok((c, log))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_bare_read_does_not_replace_the_definition_when_loading() {
        let (_, original) = Log::load("A = (1, 2)").unwrap();
        let (c, log) = Log::load("A = (1, 2)\nA").unwrap();
        assert_eq!(c.value(c.id("A").unwrap()), Value::Point { x: 1.0, y: 2.0 });
        assert_eq!(log.to_labelled_script(), original.to_labelled_script());
        assert_eq!(log.rhs_of("A"), Some("(1, 2)"));
    }

    #[test]
    fn journal_position_tracks_the_active_assignment() {
        let (c, mut log) = Log::load("a = 1\nb = a + 1\nn = 4\na = 2 n").unwrap();
        let (at, labels) = log.define_entry("a").unwrap();
        assert!(at > log.define_entry("n").unwrap().0);
        assert_eq!(labels, ["a"]);
        assert!(log.remove("a", &c));
        let (c, _) = log.rebuild().unwrap();
        assert!(c.id("a").is_none());
        assert!(c.id("b").is_none());
        assert_eq!(c.value(c.id("n").unwrap()), Value::Number(4.0));
    }

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
        let (c, mut log) = sample();
        assert!(log.remove("A", &c));
        let (c, kept) = log.rebuild().unwrap();
        let labels: Vec<&str> = c.ids().map(|id| c.label(id)).collect();
        assert_eq!(labels, ["B", "C"]);
        assert_eq!(kept.entries().count(), 2);
        assert!(!log.remove("zzz", &c));
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
