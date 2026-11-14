//! Runs all cases against the known-gaps list and builds the summary.

use crate::gaps::Gaps;
use crate::model::Case;
use crate::replay::replay;
use std::collections::BTreeSet;

#[derive(Debug, Default)]
pub struct Report {
    pub loaded: usize,
    pub harness_cases: usize,
    pub agreed: usize,
    pub known_gaps: usize,
    pub sign_agree: usize,
    pub sign_disagree: usize,
    /// Human-readable failures; the test passes only when this is empty.
    pub failures: Vec<String>,
}

impl Report {
    pub fn summary(&self) -> String {
        format!(
            "golden: {} cases loaded ({} from ggb-harness), {} agreed, {} known gaps, \
             line signs: {} agree / {} disagree",
            self.loaded,
            self.harness_cases,
            self.agreed,
            self.known_gaps,
            self.sign_agree,
            self.sign_disagree
        )
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
                if actual == gap.mismatches {
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
    fn stale_known_gap_fails() {
        let r = evaluate(&case(SCRIPT, 1.0), &listed("c1", "0:M"));
        assert!(r.failures.iter().any(|f| f.contains("stale")));
        let unknown = listed("nope", "0:M");
        assert!(!evaluate(&case(SCRIPT, 1.0), &unknown).failures.is_empty());
    }

    #[test]
    fn unparseable_script_line_is_a_disagreement() {
        let r = evaluate(&case(r#""M = Frobnicate(1)""#, 1.0), &gaps::Gaps::new());
        assert_eq!(r.failures.len(), 1);
        assert!(r.failures[0].contains("script line 1"));
    }
}
