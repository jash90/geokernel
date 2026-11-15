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

use geo_kernel::kernel::Construction;
use shared::Chain;
pub use shared::Labels;
use std::collections::BTreeSet;

mod shared;

/// The names of the directives, which no object may take.
pub const DIRECTIVES: [&str; 3] = ["SetCoords", "SetValue", "SetVisibleInView"];

/// Starts the list of labels after a definition in a snapshot.
const LABELS: &str = "# labels: ";

/// One accepted line and the objects it created.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub labels: Vec<String>,
    /// Normalised: `name = definition`.
    pub line: String,
}

impl Entry {
    /// The definition after the first `=`, as typed.
    pub fn rhs(&self) -> &str {
        self.line.split_once('=').map_or("", |(_, r)| r.trim())
    }
}

/// A new value of a free object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Setting {
    Point(f64, f64),
    Number(f64),
}

#[derive(Clone, Debug, PartialEq)]
enum Op {
    Define(Entry),
    Set { label: String, to: Setting },
}

/// Rewrites `f: Line(A, B)` as `f = Line(A, B)` and removes a trailing `#`
/// comment. Blank input gives an empty string. A `:` counts only when it comes
/// before any `=`.
pub fn normalize(input: &str) -> String {
    let line = input.split('#').next().unwrap_or("").trim();
    let colon = line.find(':');
    let eq = line.find('=');
    match (colon, eq) {
        (Some(c), e) if e.is_none_or(|e| c < e) => {
            format!("{} = {}", line[..c].trim(), line[c + 1..].trim())
        }
        _ => line.to_owned(),
    }
}

/// A script line that sets the state of an object instead of defining one.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Directive<'a> {
    Coords(&'a str, f64, f64),
    Value(&'a str, f64),
    Visible(&'a str, bool),
}

/// The directive on a normalised line, if it is one (a definition always has
/// an `=`).
fn directive(line: &str) -> Option<Result<Directive<'_>, String>> {
    if line.contains('=') {
        return None;
    }
    let (name, rest) = line.split_once('(')?;
    let name = name.trim();
    if !DIRECTIVES.contains(&name) {
        return None;
    }
    let parse = || -> Option<Directive<'_>> {
        let args: Vec<&str> = rest.strip_suffix(')')?.split(',').map(str::trim).collect();
        let num = |s: &str| s.parse::<f64>().ok();
        match (name, args.as_slice()) {
            ("SetCoords", [l, x, y]) => Some(Directive::Coords(l, num(x)?, num(y)?)),
            ("SetValue", [l, v]) => Some(Directive::Value(l, num(v)?)),
            ("SetVisibleInView", [l, "1", shown]) => {
                Some(Directive::Visible(l, shown.parse().ok()?))
            }
            _ => None,
        }
    };
    Some(parse().ok_or_else(|| format!("cannot read `{line}`")))
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

/// The labels a definition line names before its `=`: `A`, `f` of
/// `f(x)`, or those of `{t_{1}, t_{2}}` (an empty slot names nothing).
fn defined_labels(line: &str) -> impl Iterator<Item = &str> {
    let lhs = line.split_once('=').map_or("", |(l, _)| l.trim());
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
    /// Labels removed by `remove` since the last rebuild: lines that use them
    /// are dropped by the rebuild on purpose.
    removed: BTreeSet<String>,
}

impl Log {
    fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.ops.items().into_iter().filter_map(|op| match op {
            Op::Define(e) => Some(e),
            Op::Set { .. } => None,
        })
    }

    /// Records `line` as the origin of the objects with index `from..` in `c`.
    pub fn record(&mut self, c: &Construction, from: usize, line: &str) {
        let labels: Vec<String> = c
            .ids()
            .skip(from)
            .map(|id| c.label(id).to_owned())
            .collect();
        if !labels.is_empty() {
            self.ops.push(Op::Define(Entry {
                labels,
                line: line.to_owned(),
            }));
        }
    }

    /// Records a new value of a free object. A move right after a move of the
    /// same object replaces it: only the state at the next line matters.
    pub fn record_set(&mut self, label: &str, to: Setting) {
        if let Some(Op::Set { label: l, .. }) = self.ops.last() {
            if l == label {
                self.ops.replace_last(Op::Set {
                    label: label.to_owned(),
                    to,
                });
                return;
            }
        }
        self.ops.push(Op::Set {
            label: label.to_owned(),
            to,
        });
    }

    /// Every label with the definition of the line that created it.
    pub fn definitions(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries()
            .flat_map(|e| e.labels.iter().map(move |l| (l.as_str(), e.rhs())))
    }

    /// The definition text of the entry that created `label`.
    pub fn rhs_of(&self, label: &str) -> Option<&str> {
        self.definitions()
            .find(|(l, _)| *l == label)
            .map(|(_, r)| r)
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
        let labels = entry.labels.clone();
        self.ops.retain(|op| match op {
            Op::Define(e) => e.labels != labels,
            Op::Set { label, .. } => !labels.contains(label),
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
                    match geo_kernel::script::exec(&mut c, &e.line) {
                        Ok(_) => {
                            let made: Vec<&str> =
                                c.ids().skip(before).map(|id| c.label(id)).collect();
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
                Op::Set { label, to } => {
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
                    };
                    result.map_err(|e| format!("`{label}`: {e}"))?;
                    // A set right after a set of the same object is merged
                    // into it by `record_set` (after a `remove` they can
                    // meet): the journal and `kept` part there.
                    let merges = |n: usize| {
                        n > 0 && matches!(ops[n - 1], Op::Set { label: l, .. } if l == label)
                    };
                    match same {
                        Some(n) if n == i && !merges(n) => same = Some(i + 1),
                        _ => {
                            diverge(&mut kept, &mut same);
                            kept.record_set(label, *to);
                        }
                    }
                }
            }
        }
        if let Some(n) = same {
            kept.ops = self.ops.prefix(n);
        }
        kept.hidden = self.hidden.clone();
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
                    match geo_kernel::script::exec(&mut c, &e.line) {
                        Ok(_) => {
                            let made: Vec<&str> =
                                c.ids().skip(before).map(|id| c.label(id)).collect();
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
                Op::Set { label, to } => {
                    let Some(id) = c.id(label) else {
                        if gone.contains(label) {
                            continue;
                        }
                        return Err(format!("`{label}` is missing"));
                    };
                    let result = match *to {
                        Setting::Point(x, y) => c.move_point(id, x, y),
                        Setting::Number(v) => c.set_number(id, v),
                    };
                    result.map_err(|e| format!("`{label}`: {e}"))?;
                    kept.record_set(label, *to);
                }
            }
        }
        kept.hidden = self.hidden.clone();
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
                } => writeln!(out, "SetCoords({label}, {x}, {y})"),
                Op::Set {
                    label,
                    to: Setting::Number(v),
                } => writeln!(out, "SetValue({label}, {v})"),
            };
        }
        for label in &self.hidden {
            let _ = writeln!(out, "SetVisibleInView({label}, 1, false)");
        }
        out
    }

    /// Applies a directive line to `c` and records it.
    fn apply(&mut self, c: &mut Construction, d: Directive) -> Result<(), String> {
        let label = match d {
            Directive::Coords(l, ..) | Directive::Value(l, _) | Directive::Visible(l, _) => l,
        };
        let id = c.id(label).ok_or_else(|| format!("`{label}` is missing"))?;
        let result = match d {
            Directive::Coords(_, x, y) => c.move_point(id, x, y).map(|_| Setting::Point(x, y)),
            Directive::Value(_, v) => c.set_number(id, v).map(|_| Setting::Number(v)),
            Directive::Visible(_, shown) => {
                if shown {
                    self.hidden.remove(label);
                } else {
                    self.hidden.insert(label.to_owned());
                }
                return Ok(());
            }
        };
        let to = result.map_err(|e| format!("`{label}`: {e}"))?;
        self.record_set(label, to);
        Ok(())
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
        let mut rejected = Vec::new();
        let mut gone: BTreeSet<String> = BTreeSet::new();
        for (i, raw) in src.lines().enumerate() {
            let (body, listed) = match raw.split_once(LABELS) {
                Some((body, list)) => (body, Some(list)),
                None => (raw, None),
            };
            let line = normalize(body);
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
                    Some(d) => d.and_then(|d| log.apply(&mut c, d)),
                    None => check_names(&line).and_then(|()| {
                        let expected = (checked || listed.is_some()).then_some(named.as_slice());
                        log.define_checked(&mut c, &line, expected)
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
    ) -> Result<(), String> {
        let at = c.checkpoint();
        let before = c.len();
        // The checkpoint was taken just above and nothing rolls the
        // construction back in between, so the kernel cannot refuse it; if it
        // ever did, the line is still reported as left out, with the reason.
        let undo = |c: &mut Construction, reason: String| match c.rollback(at) {
            Ok(()) => reason,
            Err(e) => format!("{reason}; {e}"),
        };
        if let Err(e) = geo_kernel::script::exec(c, line) {
            return Err(undo(c, e));
        }
        if let Some(expected) = expected {
            if !c
                .ids()
                .skip(before)
                .map(|id| c.label(id))
                .eq(expected.iter().copied())
            {
                let made: Vec<&str> = c.ids().skip(before).map(|id| c.label(id)).collect();
                let reason = format!("it made {made:?} instead of {expected:?}");
                return Err(undo(c, reason));
            }
        }
        self.record(c, before, line);
        Ok(())
    }

    /// Builds a log and construction from a whole script. The error carries
    /// the 1-based line number like `script::load`.
    pub fn load(src: &str) -> Result<(Construction, Log), String> {
        let mut c = Construction::new();
        let mut log = Log::default();
        for (i, raw) in src.lines().enumerate() {
            let line = normalize(raw);
            if line.is_empty() {
                continue;
            }
            let at = |e: String| format!("line {}: {e}", i + 1);
            if let Some(d) = directive(&line) {
                log.apply(&mut c, d.map_err(at)?).map_err(at)?;
                continue;
            }
            check_names(&line).map_err(at)?;
            let before = c.len();
            geo_kernel::script::exec(&mut c, &line).map_err(at)?;
            log.record(&c, before, &line);
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
