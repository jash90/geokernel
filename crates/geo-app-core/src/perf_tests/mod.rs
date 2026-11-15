//! Tests of the caches that keep the per-frame and per-step costs of the
//! window independent of the size of the construction: the colours
//! (`colours`), the algebra rows (`rows`) and the undo steps (`undo`). Each
//! checks the cached result against the result computed from scratch, over
//! random edit sessions.

mod colours;
mod rows;
mod undo;

use crate::workbench::Workbench;
use geo_kernel::kernel::{Kind, Value};

/// A small deterministic generator (xorshift), so the sessions are the same
/// on every run.
pub(crate) struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    /// A coordinate, now and then `-0` (a change from `0` by its bits).
    pub fn coord(&mut self) -> f64 {
        match self.below(12) {
            0 => -0.0,
            1 => 0.0,
            _ => (self.below(2001) as f64 - 1000.0) / 7.0,
        }
    }

    pub fn pick(&mut self, from: &[String]) -> String {
        from[self.below(from.len())].clone()
    }
}

fn labels(wb: &Workbench, keep: impl Fn(Kind, bool) -> bool) -> Vec<String> {
    let c = wb.construction();
    c.ids()
        .filter(|&id| keep(c.kind(id), c.def(id).is_free()))
        .map(|id| c.label(id).to_owned())
        .collect()
}

/// One edit of a random session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Edit {
    Line,
    Move,
    Number,
    Toggle,
    Delete,
    Undo,
    Redo,
    /// A drag that is cancelled (Escape, or the window losing the pointer).
    CancelledDrag,
}

/// Runs `steps` random edits on `wb`, the way the window makes them, and
/// calls `check` after each with what was done.
pub(crate) fn session(
    wb: &mut Workbench,
    seed: u64,
    steps: usize,
    mut check: impl FnMut(&mut Workbench, Edit),
) {
    let mut rng = Rng(seed);
    for step in 0..steps {
        let points = labels(wb, |k, _| k == Kind::Point);
        let free_points = labels(wb, |k, free| k == Kind::Point && free);
        let curves = labels(wb, |k, _| {
            matches!(k, Kind::Line | Kind::Segment | Kind::Circle)
        });
        let numbers = labels(wb, |k, free| k == Kind::Number && free);
        let edit = match rng.below(20) {
            0..=7 => Edit::Line,
            8..=10 => Edit::Move,
            11 => Edit::Number,
            12 => Edit::Toggle,
            13 => Edit::Delete,
            14..=15 => Edit::Undo,
            16..=17 => Edit::Redo,
            _ => Edit::CancelledDrag,
        };
        match edit {
            Edit::Line => {
                let line = match rng.below(9) {
                    0 | 1 => format!("P{step} = ({}, {})", rng.coord(), rng.coord()),
                    2 => format!("n{step} = {}", rng.below(9)),
                    3 if points.len() >= 2 => {
                        let (a, b) = (rng.pick(&points), rng.pick(&points));
                        let cmd = ["Segment", "Line", "Circle", "Midpoint"][rng.below(4)];
                        format!("{cmd}({a}, {b})")
                    }
                    4 if points.len() >= 3 => {
                        let v: Vec<String> = (0..3).map(|_| rng.pick(&points)).collect();
                        format!("q{step} = Polygon({}, {}, {})", v[0], v[1], v[2])
                    }
                    5 if points.len() >= 2 => {
                        // Possibly the side of a polygon made before or after.
                        let (a, b) = (rng.pick(&points), rng.pick(&points));
                        format!("s{step} = Segment({a}, {b})")
                    }
                    6 if curves.len() >= 2 => {
                        let (a, b) = (rng.pick(&curves), rng.pick(&curves));
                        format!("I{step} = Intersect({a}, {b})")
                    }
                    7 if !numbers.is_empty() && !points.is_empty() => {
                        let (n, p) = (rng.pick(&numbers), rng.pick(&points));
                        format!("k{step} = Circle({p}, {n})")
                    }
                    8 => format!("w{step} = {}°", rng.below(360)),
                    _ => format!("R{step} = ({}, {})", rng.coord(), rng.coord()),
                };
                // Failing lines (a point with itself) are part of it.
                let _ = wb.run_line(&line);
            }
            Edit::Move | Edit::CancelledDrag if !free_points.is_empty() => {
                let id = wb.construction().id(&rng.pick(&free_points)).unwrap();
                wb.begin_gesture();
                for _ in 0..rng.below(3) + 1 {
                    wb.move_point(id, rng.coord(), rng.coord());
                }
                if edit == Edit::Move {
                    wb.end_gesture();
                } else {
                    wb.cancel_gesture();
                }
            }
            Edit::Number if !numbers.is_empty() => {
                let id = wb.construction().id(&rng.pick(&numbers)).unwrap();
                let v = match (rng.below(3), wb.construction().value(id)) {
                    (0, Value::Number(old)) => old,
                    _ => rng.below(20) as f64 / 3.0,
                };
                wb.set_number(id, v);
            }
            Edit::Toggle | Edit::Delete if !wb.construction().is_empty() => {
                let n = wb.construction().len();
                let id = wb.construction().ids().nth(rng.below(n)).unwrap();
                if edit == Edit::Toggle {
                    wb.toggle_visible(id);
                } else {
                    wb.delete(id);
                }
            }
            Edit::Undo => {
                wb.undo();
            }
            Edit::Redo => {
                wb.redo();
            }
            _ => continue,
        }
        check(wb, edit);
    }
}
