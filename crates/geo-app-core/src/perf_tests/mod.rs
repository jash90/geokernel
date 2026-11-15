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
    /// The document replaced by a file (`load_script`).
    Load,
    /// The document replaced by its own snapshot (`restore_text`).
    Restore,
}

/// Runs `steps` random edits on `wb`, the way the window makes them, and
/// calls `check` after each with what was done. Now and then an edit runs
/// with every rebuild failing (`fail_rebuilds`), or with replays failing
/// inside `rebuild` (`break_replay`), as an undo, a delete or a rollback
/// can fail.
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
        let edit = match rng.below(42) {
            0..=15 => Edit::Line,
            16..=21 => Edit::Move,
            22..=24 => Edit::Number,
            25..=26 => Edit::Toggle,
            27..=29 => Edit::Delete,
            30..=33 => Edit::Undo,
            34..=37 => Edit::Redo,
            38..=39 => Edit::CancelledDrag,
            40 => Edit::Load,
            _ => Edit::Restore,
        };
        wb.fail_rebuilds = (step % 13 == 12).then(|| format!("failure at step {step}"));
        wb.break_replay = (step % 7 == 6).then_some(step % 14 == 6);
        match edit {
            Edit::Line => {
                let line = match rng.below(16) {
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
                    9 if points.len() >= 3 => {
                        let v: Vec<String> = (0..3).map(|_| rng.pick(&points)).collect();
                        format!("ang{step} = Angle({}, {}, {})", v[0], v[1], v[2])
                    }
                    10 if points.len() >= 2 => {
                        let (a, b) = (rng.pick(&points), rng.pick(&points));
                        let cmd = ["Vector", "Ray"][rng.below(2)];
                        format!("{}{step} = {cmd}({a}, {b})", &cmd[..1].to_lowercase())
                    }
                    // Decimal numbers: their text is the exact decimal.
                    11 => format!("d{step} = {}", ["0.1", "1.25", "-2.5", "0.3"][rng.below(4)]),
                    12 if !numbers.is_empty() => {
                        format!("e{step} = {} + 2", rng.pick(&numbers))
                    }
                    13 if !numbers.is_empty() => {
                        format!("f{step}(x) = {} x^2 + 1", rng.pick(&numbers))
                    }
                    14 => format!("f{step}(x) = x^3 - x"),
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
                // The same value (for a decimal number, a set that may
                // change its text), an exact decimal, or any value.
                let v = match (rng.below(4), wb.construction().value(id)) {
                    (0, Value::Number(old)) => old,
                    (1, _) => [0.1, 1.25, 0.3, -2.5][rng.below(4)],
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
            Edit::Load => {
                let script = "A = (1, 2)\nB = (3, -1)\nC = (0, 4)\nn = 0.5\n\
                              q = Polygon(A, B, C)\ns = Segment(A, B)\nang = Angle(A, B, C)\n";
                wb.load_script(script).unwrap();
            }
            Edit::Restore => {
                let text = wb.snapshot_text();
                wb.restore_text(&text).unwrap();
            }
            _ => {
                wb.fail_rebuilds = None;
                wb.break_replay = None;
                continue;
            }
        }
        wb.fail_rebuilds = None;
        wb.break_replay = None;
        check(wb, edit);
    }
}

/// The sessions make what the tests above claim to cover: every kind of
/// object, decimal and dependent numbers, functions, loads and restores,
/// and replays that fail in each way.
#[test]
fn the_sessions_cover_every_kind_and_every_failure() {
    let mut seen = std::collections::BTreeMap::<&str, usize>::new();
    for seed in 1..=12u64 {
        let mut wb = Workbench::default();
        session(&mut wb, seed * 7919, 80, |wb, edit| {
            let c = wb.construction();
            let mut note =
                |what: &'static str, yes: bool| *seen.entry(what).or_default() += usize::from(yes);
            note("load", edit == Edit::Load);
            note("restore", edit == Edit::Restore);
            for id in c.ids() {
                let free = c.def(id).is_free();
                note("angle", crate::style::angle_points(c, id).is_some());
                note("vector", c.kind(id) == Kind::Vector);
                note("ray", c.kind(id) == Kind::Ray);
                note("function", c.kind(id) == Kind::Function);
                note(
                    "expression",
                    c.kind(id) == Kind::Number && !free && c.label(id).starts_with('e'),
                );
                note(
                    "decimal number",
                    free && matches!(c.value(id), Value::Number(v) if v.fract() != 0.0 && (v * 4.0).fract() != 0.0),
                );
            }
            let notice = wb.notice.as_deref().unwrap_or("");
            note("failed rebuild", notice.contains("failure at step"));
            note(
                "unknown command on replay",
                notice.contains("NoSuchCommand"),
            );
            note("other object on replay", notice.contains("instead of"));
        });
    }
    for (what, n) in &seen {
        assert!(*n > 0, "the sessions never made: {what} ({seen:?})");
    }
    assert_eq!(seen.len(), 11, "{seen:?}");
}
