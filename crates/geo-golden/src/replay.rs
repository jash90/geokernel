//! Replays a golden case on `geo-kernel` and compares every snapshot.

use crate::compare::{compare, Sign};
use crate::model::{Case, Step};
use geo_kernel::{script, Construction};

/// One disagreement between GeoGebra and the kernel.
#[derive(Debug, Clone, PartialEq)]
pub struct Mismatch {
    pub step: usize,
    pub label: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Default)]
pub struct CaseOutcome {
    pub mismatches: Vec<Mismatch>,
    pub sign_agree: usize,
    pub sign_disagree: usize,
}

fn failure(step: usize, label: &str, expected: &str, actual: String) -> Mismatch {
    Mismatch {
        step,
        label: label.into(),
        expected: expected.into(),
        actual,
    }
}

pub fn replay(case: &Case) -> CaseOutcome {
    let mut out = CaseOutcome::default();
    let mut c = Construction::new();
    for (i, line) in case.script.iter().enumerate() {
        if let Err(e) = script::exec(&mut c, line) {
            out.mismatches.push(failure(
                0,
                &format!("script line {}", i + 1),
                "the kernel runs the line",
                format!("`{line}`: {e}"),
            ));
            return out;
        }
    }
    for (i, (step, snapshot)) in case.steps.iter().zip(&case.snapshots).enumerate() {
        if let Err(e) = apply(&mut c, step) {
            out.mismatches
                .push(failure(i, "step", "the kernel applies the step", e));
            return out;
        }
        for (label, expected) in snapshot {
            let Some(id) = c.id(label) else {
                out.mismatches.push(failure(
                    i,
                    label,
                    &format!("{expected:?}"),
                    "no such object".into(),
                ));
                continue;
            };
            let actual = c.value(id);
            match compare(expected, c.kind(id), &actual) {
                Ok(Sign::Agrees) => out.sign_agree += 1,
                Ok(Sign::Flipped) => out.sign_disagree += 1,
                Ok(Sign::NotApplicable) => {}
                Err(why) => out.mismatches.push(failure(
                    i,
                    label,
                    &format!("{expected:?}"),
                    format!("{actual:?} ({why})"),
                )),
            }
        }
    }
    out
}

fn apply(c: &mut Construction, step: &Step) -> Result<(), String> {
    match step {
        Step::Initial => Ok(()),
        Step::Move { label, x, y } => {
            let id = c.id(label).ok_or(format!("unknown object `{label}`"))?;
            c.move_point(id, *x, *y)
                .map(drop)
                .map_err(|e| format!("move `{label}`: {e}"))
        }
        Step::SetNumber { label, value } => {
            let id = c.id(label).ok_or(format!("unknown object `{label}`"))?;
            c.set_number(id, *value)
                .map(drop)
                .map_err(|e| format!("set `{label}`: {e}"))
        }
    }
}
