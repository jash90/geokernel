//! The document the window edits: construction, selection, input history,
//! change statistics and the cached object list. No UI types.

use crate::format;
use crate::history::History;
use geo_kernel::kernel::{Construction, Def, Id, Value};
use std::time::{Duration, Instant};

/// What the last change did, for the status bar.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// Objects computed by the change.
    pub recomputed: usize,
    pub elapsed: Duration,
}

/// One precomputed row of the object list.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: Id,
    pub label: String,
    pub kind: String,
    pub value: String,
    /// The number, when the object is a free number that can be edited.
    pub free_number: Option<f64>,
}

#[derive(Default)]
pub struct Workbench {
    construction: Construction,
    /// Bumped whenever any object's value or the object set changes.
    version: u64,
    pub selected: Option<Id>,
    pub history: History,
    pub stats: Stats,
    rows: Vec<Row>,
    /// The `version` the rows were built for; `None` before the first build.
    rows_version: Option<u64>,
    rows_built: u64,
}

impl Workbench {
    pub fn new(construction: Construction) -> Self {
        Workbench {
            construction,
            ..Default::default()
        }
    }

    pub fn construction(&self) -> &Construction {
        &self.construction
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    /// Replaces the whole construction. History is kept; selection and
    /// statistics are reset.
    pub fn replace(&mut self, construction: Construction) {
        self.construction = construction;
        self.selected = None;
        self.stats = Stats::default();
        self.version += 1;
    }

    /// Runs one input line. On success the objects added are the recomputed
    /// count (the kernel does not report more for a new object).
    pub fn run_line(&mut self, line: &str) -> Result<(), String> {
        self.run_with(line, |c| geo_kernel::script::exec(c, line).map(|_| ()))
    }

    /// Runs `exec` on the construction. The caches are invalidated whenever
    /// the object count changed, also when `exec` failed after adding some
    /// objects.
    fn run_with(
        &mut self,
        line: &str,
        exec: impl FnOnce(&mut Construction) -> Result<(), String>,
    ) -> Result<(), String> {
        let before = self.construction.len();
        let start = Instant::now();
        let result = exec(&mut self.construction);
        let elapsed = start.elapsed();
        let added = self.construction.len().saturating_sub(before);
        if self.construction.len() != before {
            self.version += 1;
            self.stats = Stats {
                recomputed: added,
                elapsed,
            };
        }
        result?;
        self.history.push(line);
        Ok(())
    }

    /// Moves a free point. Returns whether anything changed.
    pub fn move_point(&mut self, id: Id, x: f64, y: f64) -> bool {
        let before = self.construction.value(id);
        let start = Instant::now();
        let Ok(n) = self.construction.move_point(id, x, y) else {
            return false;
        };
        self.after_change(before, id, n, start.elapsed())
    }

    /// Sets a free number. Returns whether anything changed.
    pub fn set_number(&mut self, id: Id, v: f64) -> bool {
        let before = self.construction.value(id);
        let start = Instant::now();
        let Ok(n) = self.construction.set_number(id, v) else {
            return false;
        };
        self.after_change(before, id, n, start.elapsed())
    }

    fn after_change(&mut self, before: Value, id: Id, n: usize, elapsed: Duration) -> bool {
        // The kernel counts dependents only, so a change with no dependents
        // returns 0; compare the value itself to tell it from "unchanged".
        if self.construction.value(id) == before {
            return false;
        }
        self.version += 1;
        self.stats = Stats {
            recomputed: n,
            elapsed,
        };
        true
    }

    /// The rows of the object list, rebuilt only when the version changed.
    pub fn rows(&mut self) -> &[Row] {
        if self.rows_version != Some(self.version) {
            self.rebuild_rows();
        }
        &self.rows
    }

    /// How many times the rows were rebuilt, to check caching in tests.
    #[cfg(test)]
    pub fn rows_built(&self) -> u64 {
        self.rows_built
    }

    fn rebuild_rows(&mut self) {
        let c = &self.construction;
        self.rows.clear();
        self.rows.extend(c.ids().map(|id| {
            let value = c.value(id);
            Row {
                id,
                label: c.label(id).to_owned(),
                kind: c.kind(id).to_string(),
                value: format::value(&value),
                free_number: match (c.def(id), value) {
                    (Def::Number(_), Value::Number(n)) => Some(n),
                    _ => None,
                },
            }
        }));
        self.rows_version = Some(self.version);
        self.rows_built += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> Workbench {
        Workbench::new(
            geo_kernel::script::load(include_str!("../../../examples/triangle.geo")).unwrap(),
        )
    }

    #[test]
    fn run_line_adds_an_object_and_records_stats() {
        let mut w = triangle();
        let n = w.construction().len();
        let v = w.version();
        w.run_line("Z = Midpoint(A, B)").unwrap();
        assert_eq!(w.construction().len(), n + 1);
        assert!(w.version() > v);
        assert_eq!(w.stats.recomputed, 1);
        assert_eq!(w.history.len(), 1);
    }

    #[test]
    fn failed_line_changes_nothing() {
        let mut w = triangle();
        let n = w.construction().len();
        let v = w.version();
        let err = w.run_line("Z = Midpoint(A, Nope)").unwrap_err();
        assert!(!err.is_empty());
        assert_eq!(w.construction().len(), n);
        assert_eq!(w.version(), v);
        assert_eq!(w.history.len(), 0);
        // Duplicate name is also an error.
        assert!(w.run_line("A = (9, 9)").is_err());
    }

    #[test]
    fn partial_failure_still_invalidates_caches() {
        let mut w = triangle();
        w.rows();
        let (n, v) = (w.construction().len(), w.version());
        let err = w
            .run_with("two outputs", |c| {
                geo_kernel::script::exec(c, "Z1 = Midpoint(A, B)")?;
                geo_kernel::script::exec(c, "Z2 = Midpoint(A, C)")?;
                Err("third output failed".into())
            })
            .unwrap_err();
        assert_eq!(err, "third output failed");
        assert_eq!(w.construction().len(), n + 2);
        assert!(w.version() > v);
        assert_eq!(w.stats.recomputed, 2);
        assert_eq!(w.rows().len(), n + 2);
        assert_eq!(w.history.len(), 0);
    }

    #[test]
    fn comment_line_is_ok_and_not_a_change() {
        let mut w = triangle();
        let v = w.version();
        w.run_line("# nothing").unwrap();
        assert_eq!(w.version(), v);
    }

    #[test]
    fn moving_a_point_updates_version_and_stats() {
        let mut w = triangle();
        let a = w.construction().id("A").unwrap();
        let v = w.version();
        assert!(w.move_point(a, 0.5, 0.5));
        assert!(w.version() > v);
        assert!(w.stats.recomputed > 0);
        let v = w.version();
        // Same position: nothing changes, stats are kept.
        let stats = w.stats;
        assert!(!w.move_point(a, 0.5, 0.5));
        assert_eq!(w.version(), v);
        assert_eq!(w.stats, stats);
    }

    #[test]
    fn moving_a_point_without_dependents_still_counts_as_a_change() {
        let mut w = Workbench::default();
        w.run_line("P = (1, 1)").unwrap();
        let p = w.construction().id("P").unwrap();
        let v = w.version();
        assert!(w.move_point(p, 2.0, 2.0));
        assert!(w.version() > v);
        assert_eq!(w.stats.recomputed, 0);
    }

    #[test]
    fn moving_a_dependent_object_is_rejected() {
        let mut w = triangle();
        let o = w.construction().id("O").unwrap();
        let v = w.version();
        assert!(!w.move_point(o, 1.0, 1.0));
        assert_eq!(w.version(), v);
    }

    #[test]
    fn set_number_recomputes_dependents() {
        let mut w = triangle();
        let r = w.construction().id("r").unwrap();
        let m = w.construction().id("m").unwrap();
        let before = w.construction().value(m);
        assert!(w.set_number(r, 4.0));
        assert_ne!(w.construction().value(m), before);
        assert!(w.stats.recomputed > 0);
        assert!(!w.set_number(r, 4.0));
        // A dependent number cannot be set.
        let d = w.construction().id("d").unwrap();
        assert!(!w.set_number(d, 1.0));
    }

    #[test]
    fn rows_follow_creation_order_and_describe_objects() {
        let mut w = triangle();
        let n = w.construction().len();
        let rows = w.rows().to_vec();
        assert_eq!(rows.len(), n);
        assert_eq!(rows[0].label, "A");
        assert_eq!(rows[0].kind, "point");
        assert_eq!(rows[0].value, "(0, 0)");
        let r = rows.iter().find(|r| r.label == "r").unwrap();
        assert_eq!(r.free_number, Some(3.0));
        assert_eq!(r.kind, "number");
        let d = rows.iter().find(|r| r.label == "d").unwrap();
        assert_eq!(d.free_number, None);
    }

    #[test]
    fn rows_are_cached_until_something_changes() {
        let mut w = triangle();
        w.rows();
        w.rows();
        assert_eq!(w.rows_built(), 1);
        let a = w.construction().id("A").unwrap();
        w.move_point(a, 1.0, 1.0);
        w.rows();
        w.rows();
        assert_eq!(w.rows_built(), 2);
        // A rejected change does not invalidate the cache.
        w.move_point(a, 1.0, 1.0);
        w.rows();
        assert_eq!(w.rows_built(), 2);
        w.run_line("Z = Midpoint(A, B)").unwrap();
        assert_eq!(w.rows().len(), w.construction().len());
        assert_eq!(w.rows_built(), 3);
    }

    #[test]
    fn undefined_objects_are_listed_as_undefined() {
        let mut w = triangle();
        let r = w.construction().id("r").unwrap();
        // A huge circle around C cannot meet the circumcircle.
        assert!(w.set_number(r, 100.0));
        let rows = w.rows().to_vec();
        let p = rows.iter().find(|r| r.label == "P").unwrap();
        assert_eq!(p.value, "undefined");
    }

    #[test]
    fn replace_resets_selection_and_stats_and_invalidates_rows() {
        let mut w = triangle();
        w.selected = w.construction().id("A");
        w.rows();
        w.replace(Construction::new());
        assert_eq!(w.selected, None);
        assert_eq!(w.stats, Stats::default());
        assert!(w.rows().is_empty());
    }
}
