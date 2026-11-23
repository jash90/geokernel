//! Runs all cases against the known-gaps list and builds the summary.

use crate::gaps::Gaps;
use crate::model::Case;
use crate::replay::replay;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default)]
pub struct Report {
    pub loaded: usize,
    pub harness_cases: usize,
    pub agreed: usize,
    pub known_gaps: usize,
    pub sign_agree: usize,
    pub sign_disagree: usize,
    /// Defined lines of the ggb-harness cases whose raw coefficients equal
    /// GeoGebra's.
    pub scale_agree: usize,
    /// Defined lines whose raw coefficients differ from GeoGebra's.
    pub scale_disagree: usize,
    /// `scale_disagree` split by group and by the command that defines the line.
    pub scale_disagree_by: BTreeMap<(String, String), usize>,
    /// Defined conics compared, and how many agree in the raw matrix and up
    /// to a factor (the raw matrix decides the comparison).
    pub conic_compared: usize,
    pub conic_raw_agree: usize,
    pub conic_scaled_agree: usize,
    /// Human-readable failures; the test passes only when this is empty.
    pub failures: Vec<String>,
}

impl Report {
    /// The result line, followed by one line per group and defining command
    /// whose lines differ in scale from GeoGebra's.
    pub fn summary(&self) -> String {
        let mut s = format!(
            "golden: {} cases loaded ({} from ggb-harness), {} agreed, {} known gaps, \
             line signs: {} agree / {} disagree, line scale: {} agree / {} disagree",
            self.loaded,
            self.harness_cases,
            self.agreed,
            self.known_gaps,
            self.sign_agree,
            self.sign_disagree,
            self.scale_agree,
            self.scale_disagree
        );
        s.push_str(&format!(
            ", conic matrices: {} compared, {} agree raw, {} agree up to scale",
            self.conic_compared, self.conic_raw_agree, self.conic_scaled_agree
        ));
        for ((group, command), n) in &self.scale_disagree_by {
            s.push_str(&format!("\n  line scale disagrees: {group} {command}: {n}"));
        }
        s
    }
}

pub fn evaluate(cases: &[Case], gaps: &Gaps) -> Report {
    let mut r = Report {
        loaded: cases.len(),
        ..Report::default()
    };
    for id in gaps.keys() {
        if !cases.iter().any(|c| &c.id == id) {
            r.failures
                .push(format!("known gap `{id}` does not match any case"));
        }
    }
    for case in cases {
        if case.generator == "ggb-harness" {
            r.harness_cases += 1;
        }
        let out = replay(case);
        r.sign_agree += out.sign_agree;
        r.sign_disagree += out.sign_disagree;
        r.conic_compared += out.conic_compared;
        r.conic_raw_agree += out.conic_raw_agree;
        r.conic_scaled_agree += out.conic_scaled_agree;
        // Only values recorded from GeoGebra say what its raw coefficients are.
        if case.generator == "ggb-harness" {
            r.scale_agree += out.scale_agree;
            r.scale_disagree += out.scale_disagree.len();
            for label in &out.scale_disagree {
                let key = (case.group.clone(), command_of(case, label));
                *r.scale_disagree_by.entry(key).or_default() += 1;
            }
        }
        let listed = gaps.get(&case.id);
        match (out.mismatches.is_empty(), listed) {
            (true, None) => r.agreed += 1,
            (true, Some(gap)) => r.failures.push(format!(
                "stale known gap `{}` ({}): the case now agrees, remove the entry",
                case.id, gap.reason
            )),
            (false, Some(gap)) => {
                // A listed case must disagree exactly where the list says, so a
                // new disagreement cannot hide behind an old one.
                let actual: BTreeSet<String> = out
                    .mismatches
                    .iter()
                    .map(|m| format!("{}:{}", m.step, m.label))
                    .collect();
                // The statement covers the listed mismatches only; an unlisted
                // one is reported by the set comparison below.
                let wrong_answer = gap.kernel.as_ref().and_then(|kernel| {
                    out.mismatches
                        .iter()
                        .filter(|m| gap.mismatches.contains(&format!("{}:{}", m.step, m.label)))
                        .find(|m| !kernel.matches(&m.actual))
                        .map(|m| (kernel, m))
                });
                if let Some((kernel, m)) = wrong_answer {
                    r.failures.push(format!(
                        "known gap `{}` step {} label `{}`: the entry states {kernel:?}, \
                         the kernel answers {}",
                        case.id, m.step, m.label, m.actual
                    ));
                } else if actual == gap.mismatches {
                    r.known_gaps += 1;
                } else {
                    let unlisted: Vec<&String> = actual.difference(&gap.mismatches).collect();
                    let fixed: Vec<&String> = gap.mismatches.difference(&actual).collect();
                    r.failures.push(format!(
                        "known gap `{}` changed: new disagreements {unlisted:?}, \
                         now agreeing {fixed:?}; update its `mismatches` list",
                        case.id
                    ));
                }
            }
            (false, None) => {
                for m in &out.mismatches {
                    r.failures.push(format!(
                        "case `{}` step {} label `{}`: expected {}, actual {}",
                        case.id, m.step, m.label, m.expected, m.actual
                    ));
                }
            }
        }
    }
    r
}

/// The command name of the script line that creates `label`; an output
/// `p_{n}` of a command with several outputs belongs to the line whose label
/// starts with `p` up to its first `_`. `?` when no line matches.
fn command_of(case: &Case, label: &str) -> String {
    let prefix = |l: &str| l.split('_').next().unwrap_or(l).to_string();
    let wanted = label.split("_{").next().unwrap_or(label);
    for line in &case.script {
        let Some((lhs, rhs)) = line.split_once('=') else {
            continue;
        };
        let lhs = crate::model::line_label(lhs);
        let lhs = lhs.as_str();
        if lhs == label || (wanted != label && prefix(lhs) == wanted) {
            let rhs = rhs.trim();
            return match rhs.split_once('(') {
                Some((name, _)) if !name.trim().is_empty() => name.trim().to_string(),
                _ => "?".to_string(),
            };
        }
    }
    "?".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{gaps, model::parse_file};

    fn case(script: &str, expected_x: f64) -> Vec<Case> {
        let json = format!(
            r#"{{"schema":1,"generator":"ggb-harness","upstream_commit":"c","cases":[
            {{"id":"c1","group":"g","script":[{script}],"steps":[{{"action":"initial"}}],
            "snapshots":[{{"M":{{"type":"point","defined":true,"x":{expected_x},"y":0.0}}}}]}}]}}"#
        );
        parse_file(&json, "t").unwrap().cases
    }

    const SCRIPT: &str = r#""M = (1, 0)""#;

    fn listed(id: &str, mismatch: &str) -> gaps::Gaps {
        gaps::parse(&format!(
            "[[gap]]\nid=\"{id}\"\nreason=\"r\"\nmismatches=[\"{mismatch}\"]\n"
        ))
        .unwrap()
    }

    #[test]
    fn agreeing_case_passes() {
        let r = evaluate(&case(SCRIPT, 1.0), &gaps::Gaps::new());
        assert!(r.failures.is_empty());
        assert_eq!((r.agreed, r.harness_cases), (1, 1));
    }

    #[test]
    fn disagreeing_unlisted_case_fails_with_details() {
        let r = evaluate(&case(SCRIPT, 2.0), &gaps::Gaps::new());
        assert_eq!(r.failures.len(), 1);
        assert!(r.failures[0].contains("c1") && r.failures[0].contains("step 0"));
        assert!(r.failures[0].contains("`M`"));
    }

    #[test]
    fn listed_disagreeing_case_is_a_known_gap() {
        let r = evaluate(&case(SCRIPT, 2.0), &listed("c1", "0:M"));
        assert!(r.failures.is_empty());
        assert_eq!(r.known_gaps, 1);
    }

    #[test]
    fn listed_case_must_disagree_exactly_where_listed() {
        // Listed for another object: the real disagreement must not hide.
        let r = evaluate(&case(SCRIPT, 2.0), &listed("c1", "0:Q"));
        assert_eq!(r.known_gaps, 0);
        assert!(r.failures[0].contains("changed") && r.failures[0].contains("0:M"));
    }

    #[test]
    fn a_gap_that_states_the_kernel_answer_checks_it() {
        let stated = |extra: &str| {
            gaps::parse(&format!(
                "[[gap]]\nid=\"c1\"\nreason=\"r\"\nmismatches=[\"0:M\"]\n{extra}\n"
            ))
            .unwrap()
        };
        // The kernel's point is defined: neither statement holds.
        for extra in ["undefined = true", "failure = \"invalid arguments\""] {
            let r = evaluate(&case(SCRIPT, 2.0), &stated(extra));
            assert_eq!(r.known_gaps, 0, "{extra}");
            assert!(r.failures[0].contains("states"), "{extra}");
        }
        // An unlisted mismatch is reported as a changed list, not as a
        // wrong stated answer.
        let mut unlisted = stated("undefined = true");
        unlisted.get_mut("c1").unwrap().mismatches = ["0:Q".to_owned()].into();
        let r = evaluate(&case(SCRIPT, 2.0), &unlisted);
        assert_eq!(r.known_gaps, 0);
        assert!(r.failures[0].contains("changed") && r.failures[0].contains("0:M"));
        assert!(!r.failures[0].contains("states"));
        // A refused line with the stated error passes, another error fails.
        let refused = case(r#""M = Foo(1)""#, 1.0);
        let error = |e: &str| stated(&format!("failure = \"{e}\""));
        let actual = &replay(&refused[0]).mismatches[0].actual;
        let message = actual.rsplit("`: ").next().unwrap().to_owned();
        let mut gaps = error(&message);
        gaps.get_mut("c1").unwrap().mismatches = ["0:script line 1".to_owned()].into();
        assert_eq!(evaluate(&refused, &gaps).known_gaps, 1);
        let mut gaps = error("something else");
        gaps.get_mut("c1").unwrap().mismatches = ["0:script line 1".to_owned()].into();
        assert_eq!(evaluate(&refused, &gaps).known_gaps, 0);
    }

    #[test]
    fn stale_known_gap_fails() {
        let r = evaluate(&case(SCRIPT, 1.0), &listed("c1", "0:M"));
        assert!(r.failures.iter().any(|f| f.contains("stale")));
        let unknown = listed("nope", "0:M");
        assert!(!evaluate(&case(SCRIPT, 1.0), &unknown).failures.is_empty());
    }

    fn line_case(raw: (f64, f64, f64)) -> Vec<Case> {
        let json = format!(
            r#"{{"schema":1,"generator":"ggb-harness","upstream_commit":"c","cases":[
            {{"id":"c1","group":"g","script":["A = (0, 0)","B = (2, 0)","l = Line(A, B)"],
            "steps":[{{"action":"initial"}}],"snapshots":[{{
            "A":{{"type":"point","defined":true,"x":0.0,"y":0.0}},
            "B":{{"type":"point","defined":true,"x":2.0,"y":0.0}},
            "l":{{"type":"line","defined":true,"a":{},"b":{},"c":{}}}}}]}}]}}"#,
            raw.0, raw.1, raw.2
        );
        parse_file(&json, "t").unwrap().cases
    }

    #[test]
    fn line_scale_is_counted_but_does_not_fail() {
        // Line(A, B) with A = (0, 0), B = (2, 0) is (0, 2, 0) in GeoGebra.
        let r = evaluate(&line_case((0.0, 2.0, 0.0)), &gaps::Gaps::new());
        assert!(r.failures.is_empty());
        assert_eq!((r.scale_agree, r.scale_disagree), (1, 0));
        assert!(r.summary().contains("line scale: 1 agree / 0 disagree"));

        let r = evaluate(&line_case((0.0, 1.0, 0.0)), &gaps::Gaps::new());
        assert!(r.failures.is_empty());
        assert_eq!((r.scale_agree, r.scale_disagree), (0, 1));
        let key = ("g".to_string(), "Line".to_string());
        assert_eq!(r.scale_disagree_by.get(&key), Some(&1));
        assert!(r.summary().contains("line scale disagrees: g Line: 1"));
    }

    #[test]
    fn outputs_of_a_command_with_several_results_map_to_its_line() {
        let mut c = line_case((0.0, 2.0, 0.0)).remove(0);
        c.script.push("t_1 = Tangent(A, l)".into());
        let c = &c;
        assert_eq!(command_of(c, "l"), "Line");
        assert_eq!(command_of(c, "t_{2}"), "Tangent");
        assert_eq!(command_of(c, "A"), "?");
        assert_eq!(command_of(c, "zz"), "?");
    }

    #[test]
    fn unparseable_script_line_is_a_disagreement() {
        let r = evaluate(&case(r#""M = Frobnicate(1)""#, 1.0), &gaps::Gaps::new());
        assert_eq!(r.failures.len(), 1);
        assert!(r.failures[0].contains("script line 1"));
    }
}
