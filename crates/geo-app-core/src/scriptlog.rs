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
//! they change the state of objects the script already made.

use geo_kernel::kernel::Construction;
use std::collections::BTreeSet;

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
    if !matches!(name, "SetCoords" | "SetValue" | "SetVisibleInView") {
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

/// The identifiers of a script line.
fn tokens(line: &str) -> impl Iterator<Item = &str> {
    line.split(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '{' | '}' | '\'')))
        .filter(|t| !t.is_empty())
}

/// The journal of a construction, plus which objects are hidden.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Log {
    ops: Vec<Op>,
    /// Labels hidden in the drawing (part of the state that undo restores).
    pub hidden: BTreeSet<String>,
    /// Labels removed by `remove` since the last rebuild: lines that use them
    /// are dropped by the rebuild on purpose.
    removed: BTreeSet<String>,
}

impl Log {
    fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.ops.iter().filter_map(|op| match op {
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
        if let Some(Op::Set { label: l, to: t }) = self.ops.last_mut() {
            if l == label {
                *t = to;
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
        for op in &self.ops {
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

    /// The journal as a script that `load` reads back into the same
    /// construction and an equal log: every definition line and every move
    /// in their order, then the hidden objects. Numbers are written with
    /// Rust's shortest exact form, so they read back bit for bit.
    ///
    /// It walks the whole journal: call it to save, not on every change.
    pub fn to_script(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        for op in &self.ops {
            // Writing to a `String` cannot fail.
            let _ = match op {
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
            let before = c.len();
            geo_kernel::script::exec(&mut c, &line).map_err(|e| format!("line {}: {e}", i + 1))?;
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
    fn siblings_of_a_single_output_line_are_empty() {
        let (_, log) = sample();
        assert!(log.siblings("A").is_empty());
    }
}
