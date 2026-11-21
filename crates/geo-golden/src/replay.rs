//! Replays a golden case on `geo-kernel` and compares every snapshot.

use crate::compare::{
    compare_arc, compare_conic, compare_list, compare_sampled, line_scale_agrees, Sign,
};
use crate::model::{Case, Step};
use geo_kernel::{script, Construction, Kind};

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
    /// Defined lines whose raw coefficients equal GeoGebra's (see
    /// [`line_scale_agrees`]); counted, never a failure.
    pub scale_agree: usize,
    /// Labels of the defined lines whose raw coefficients differ from
    /// GeoGebra's, once per snapshot.
    pub scale_disagree: Vec<String>,
    /// Defined conics whose raw matrix agrees with GeoGebra's.
    pub conic_raw_agree: usize,
    /// Defined conics whose matrix agrees with GeoGebra's up to a factor.
    pub conic_scaled_agree: usize,
    /// Defined conics on both sides.
    pub conic_compared: usize,
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
            if matches!(c.kind(id), geo_kernel::Kind::Arc | geo_kernel::Kind::Sector) {
                let data = c.arc(id);
                if let Err(why) = compare_arc(expected, c.kind(id), &actual, data.as_ref()) {
                    out.mismatches.push(failure(
                        i,
                        label,
                        &format!("{expected:?}"),
                        format!("{actual:?} ({why})"),
                    ));
                }
                continue;
            }
            if c.kind(id) == geo_kernel::Kind::Conic {
                let data = c.conic(id);
                let (result, agreement) =
                    compare_conic(expected, c.kind(id), &actual, data.as_ref());
                if let Some(a) = agreement {
                    out.conic_compared += 1;
                    out.conic_raw_agree += usize::from(a.raw);
                    out.conic_scaled_agree += usize::from(a.up_to_scale);
                }
                if let Err(why) = result {
                    out.mismatches.push(failure(
                        i,
                        label,
                        &format!("{expected:?}"),
                        format!("{actual:?} ({why})"),
                    ));
                }
                continue;
            }
            match line_scale_agrees(expected, &actual) {
                Some(true) => out.scale_agree += 1,
                Some(false) => out.scale_disagree.push(label.clone()),
                None => {}
            }
            let vertices: Vec<_> = c.vertices(id).collect();
            let sample = |x| c.function_value(id, x);
            let outcome = if c.kind(id) == Kind::List {
                compare_list(expected, Kind::List, c.elements(id))
            } else {
                compare_sampled(expected, c.kind(id), &actual, &vertices, Some(&sample))
            };
            match outcome {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_file;

    #[test]
    fn only_legacy_files_accept_an_angle_as_a_number() {
        let fixture = r#"{"schema":1,"generator":"format-sample","upstream_commit":null,
          "cases":[{"id":"sample","group":"expr","script":["a = 0°"],
          "steps":[{"action":"initial"}],
          "snapshots":[{"a":{"type":"number","defined":true,"value":0}}]}]}"#;
        let legacy = parse_file(fixture, "legacy example").unwrap();
        assert!(replay(&legacy.cases[0]).mismatches.is_empty());

        let current = fixture.replace("\"schema\":1", "\"schema\":2");
        let current = parse_file(&current, "current example").unwrap();
        let outcome = replay(&current.cases[0]);
        assert_eq!(outcome.mismatches.len(), 1);
        assert_eq!(outcome.mismatches[0].label, "a");
        assert!(outcome.mismatches[0]
            .actual
            .contains("type mismatch: expected number, got angle"));

        let typed = fixture
            .replace("\"schema\":1", "\"schema\":2")
            .replace("\"type\":\"number\"", "\"type\":\"angle\"");
        let typed = parse_file(&typed, "angle example").unwrap();
        assert!(replay(&typed.cases[0]).mismatches.is_empty());
    }
}
