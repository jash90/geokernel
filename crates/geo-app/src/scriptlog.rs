//! The construction as the lines that built it.
//!
//! The kernel can neither delete an object nor print a definition, so the app
//! keeps the normalised script lines. Deleting, undo and redo rebuild a
//! `Construction` from them; a line whose inputs are gone fails to run and is
//! dropped, which deletes everything that depended on the removed object.

use geo_kernel::kernel::Construction;

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

/// The entries that built a construction, in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Log {
    entries: Vec<Entry>,
}

impl Log {
    #[cfg(test)]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Records `line` as the origin of the objects with index `from..` in `c`.
    pub fn record(&mut self, c: &Construction, from: usize, line: &str) {
        let labels: Vec<String> = c
            .ids()
            .skip(from)
            .map(|id| c.label(id).to_owned())
            .collect();
        if !labels.is_empty() {
            self.entries.push(Entry {
                labels,
                line: line.to_owned(),
            });
        }
    }

    /// Every label with the definition of the entry that created it.
    pub fn definitions(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries
            .iter()
            .flat_map(|e| e.labels.iter().map(move |l| (l.as_str(), e.rhs())))
    }

    /// The definition text of the entry that created `label`.
    #[cfg(test)]
    pub fn rhs_of(&self, label: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.labels.iter().any(|l| l == label))
            .map(Entry::rhs)
    }

    /// Replaces the definition of the single-output entry that created `label`
    /// (a free point or number whose value changed).
    pub fn set_rhs(&mut self, label: &str, rhs: &str) {
        if let Some(e) = self
            .entries
            .iter_mut()
            .find(|e| e.labels.len() == 1 && e.labels[0] == label)
        {
            e.line = format!("{label} = {rhs}");
        }
    }

    /// Removes the entry that created `label`. Returns whether there was one.
    pub fn remove(&mut self, label: &str) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|e| !e.labels.iter().any(|l| l == label));
        self.entries.len() != before
    }

    /// Runs every entry on a fresh construction. Entries that fail (their
    /// inputs were removed) are dropped from the returned log.
    pub fn rebuild(&self) -> (Construction, Log) {
        let mut c = Construction::new();
        let mut kept = Log::default();
        for e in &self.entries {
            let before = c.len();
            if geo_kernel::script::exec(&mut c, &e.line).is_ok() {
                kept.record(&c, before, &e.line);
            }
        }
        (c, kept)
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

    #[test]
    fn normalize_accepts_both_forms() {
        assert_eq!(normalize("f = Line(A, B)"), "f = Line(A, B)");
        assert_eq!(normalize("f: Line(A, B)"), "f = Line(A, B)");
        assert_eq!(normalize("  f :Line(A,B)  # note"), "f = Line(A,B)");
        assert_eq!(normalize("# only a comment"), "");
        assert_eq!(normalize("   "), "");
        // A colon after the `=` is part of the definition, not a separator.
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
        assert_eq!(log.entries().len(), 5);
        assert_eq!(log.entries()[2].line, "f = Line(A, B)");
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
        let (c, kept) = log.rebuild();
        // f and M depended on A and are gone; B and C stay.
        let labels: Vec<&str> = c.ids().map(|id| c.label(id)).collect();
        assert_eq!(labels, ["B", "C"]);
        assert_eq!(kept.entries().len(), 2);
        assert!(!log.remove("zzz"));
    }

    #[test]
    fn set_rhs_changes_a_free_object_and_survives_rebuild() {
        let (_, mut log) = sample();
        log.set_rhs("A", "(1, 1)");
        let (c, _) = log.rebuild();
        let a = c.id("A").unwrap();
        assert_eq!(
            c.value(a),
            geo_kernel::kernel::Value::Point { x: 1.0, y: 1.0 }
        );
    }

    #[test]
    fn rebuild_of_an_unchanged_log_is_identical() {
        let (c1, log) = sample();
        let (c2, log2) = log.rebuild();
        assert_eq!(log, log2);
        assert_eq!(c1.len(), c2.len());
    }
}
