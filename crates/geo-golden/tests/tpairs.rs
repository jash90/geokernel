//! Runs every verified upstream t-pair through `geo-kernel` and checks the
//! result against `t-pairs-gaps.toml` (see `src/tpairs.rs`).

use geo_golden::{repo_root, tpairs};
use geo_kernel::Construction;
use std::collections::BTreeMap;

/// The verified pairs the runner must find: raising the bar notices a lost
/// or truncated `verified.jsonl`.
const MIN_STEPS: usize = 1864;
const MIN_METHODS: usize = 580;

/// `GEOKERNEL_TPAIRS_COMMAND=Midpoint` narrows the report to one command,
/// so a lane can see its red pairs. The verdicts of the other commands are
/// still checked; only the printout shrinks.
fn filter_command() -> Option<String> {
    let s = std::env::var("GEOKERNEL_TPAIRS_COMMAND").ok()?;
    assert!(
        !s.trim().is_empty(),
        "GEOKERNEL_TPAIRS_COMMAND must name a command"
    );
    Some(s)
}

#[test]
fn kernel_agrees_with_the_verified_t_pairs() {
    let text = std::fs::read_to_string(repo_root().join("golden/t-pairs/verified.jsonl"))
        .unwrap_or_else(|e| panic!("golden/t-pairs/verified.jsonl: {e}"));
    let methods = tpairs::load(&text).unwrap_or_else(|e| panic!("verified.jsonl: {e}"));
    let gaps_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("t-pairs-gaps.toml");
    let gaps_text = std::fs::read_to_string(&gaps_path)
        .unwrap_or_else(|e| panic!("{}: {e}", gaps_path.display()));
    let gaps = tpairs::parse_gaps(&gaps_text).unwrap_or_else(|e| panic!("{e}"));
    let filter = filter_command();

    let steps: usize = methods.iter().map(|m| m.steps.len()).sum();
    assert!(
        steps >= MIN_STEPS && methods.len() >= MIN_METHODS,
        "only {} steps in {} methods, expected at least {MIN_STEPS} in {MIN_METHODS}",
        steps,
        methods.len()
    );

    let mut passed = 0usize;
    let mut by_category: BTreeMap<&'static str, usize> = BTreeMap::new();
    // per command: pass, listed as a gap, total
    let mut by_command: BTreeMap<String, [usize; 3]> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();
    // The gap list as it should be now: id to (category, reason, labels).
    let mut current: BTreeMap<String, (tpairs::Category, String, BTreeMap<String, ()>)> =
        BTreeMap::new();
    let mut filtered_report = String::new();

    for method in &methods {
        let mut c = Construction::new();
        for pair in &method.steps {
            let out = tpairs::run_pair(&mut c, pair);
            let labels: BTreeMap<String, ()> = out
                .disagreements
                .iter()
                .map(|d| (d.label.clone(), ()))
                .collect();
            let listed = gaps.get(&out.id);
            let entry = by_command.entry(out.command.clone()).or_default();
            entry[2] += 1;

            if filter
                .as_ref()
                .is_some_and(|f| !out.command.contains(f.as_str()))
            {
                // Not in the report, but still part of the verdicts.
                if let Some(category) = out.category {
                    *by_category.entry(category.as_str()).or_default() += 1;
                    current.insert(
                        out.id.clone(),
                        (category, auto_reason(&out), labels.clone()),
                    );
                } else {
                    passed += 1;
                    entry[0] += 1;
                }
                if listed.is_some() {
                    entry[1] += 1;
                }
                continue;
            }

            if filter.is_some() {
                let verdict = match out.category {
                    None => "PASS".into(),
                    Some(cat) => format!("GAP {}", cat.as_str()),
                };
                filtered_report
                    .push_str(&format!("\n    {} `{:?}` -> {verdict}", out.id, pair.input));
                for d in &out.disagreements {
                    filtered_report.push_str(&format!("\n      [{}] {}", d.label, d.why));
                }
            }

            match (out.category, listed) {
                (None, None) => {
                    passed += 1;
                    entry[0] += 1;
                }
                (None, Some((category, reason, _))) => failures.push(format!(
                    "stale gap `{}` ({} {reason:?}): the step now passes, remove the entry",
                    out.id,
                    category.as_str()
                )),
                (Some(category), listed) => {
                    *by_category.entry(category.as_str()).or_default() += 1;
                    entry[1] += 1;
                    current.insert(
                        out.id.clone(),
                        (category, auto_reason(&out), labels.clone()),
                    );
                    let matches_list = listed.is_some_and(|(_, _, ls)| *ls == labels);
                    if !matches_list {
                        if let Some((_, reason, ls)) = listed {
                            let extra: Vec<&String> =
                                labels.keys().filter(|l| !ls.contains_key(*l)).collect();
                            let gone: Vec<&String> =
                                ls.keys().filter(|l| !labels.contains_key(*l)).collect();
                            failures.push(format!(
                                "gap `{}` ({reason}) changed: new disagreements {extra:?}, \
                                 now agreeing {gone:?}; update its `mismatches` list",
                                out.id
                            ));
                        } else {
                            for d in &out.disagreements {
                                failures.push(format!(
                                    "`{}` ({}): [{}] {}",
                                    out.id, out.command, d.label, d.why
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    for id in gaps.keys() {
        if !current.contains_key(id) {
            failures.push(format!(
                "known gap `{id}` does not match any step of verified.jsonl"
            ));
        }
    }

    // Per-command summary: pass / listed gap / total, the reddest first.
    let mut summary = format!(
        "t-pairs: {steps} steps in {} methods: {passed} pass, {} gaps, {} failures",
        methods.len(),
        by_category.values().sum::<usize>(),
        failures.len()
    );
    for (category, n) in &by_category {
        summary.push_str(&format!("\n  {category}: {n}"));
    }
    summary.push_str("\n  per command (pass/gap/total):");
    let mut commands: Vec<_> = by_command.iter().collect();
    commands.sort_by_key(|(_, n)| std::cmp::Reverse(n[2] - n[0]));
    for (command, n) in &commands {
        summary.push_str(&format!("\n    {command}: {} / {} / {}", n[0], n[1], n[2]));
    }
    if let Some(f) = &filter {
        summary.push_str(&format!("\n  filter: {f}{filtered_report}"));
    }

    // Regenerates the gap list from the current state.
    if std::env::var("GEOKERNEL_TPAIRS_BLESS").ok().as_deref() == Some("1") {
        std::fs::write(&gaps_path, tpairs::write_gaps(&current))
            .unwrap_or_else(|e| panic!("write {}: {e}", gaps_path.display()));
    }

    println!("{summary}");
    assert!(failures.is_empty(), "{}\n{}", summary, failures.join("\n"));
}

/// The reason written into a regenerated gap list: the first disagreement,
/// in one line.
fn auto_reason(out: &tpairs::StepOutcome) -> String {
    out.disagreements
        .first()
        .map(|d| d.why.replace('\n', " "))
        .unwrap_or_default()
}
