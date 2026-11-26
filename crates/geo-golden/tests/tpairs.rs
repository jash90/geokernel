//! Runs every verified upstream t-pair through `geo-kernel` and checks the
//! result against `t-pairs-gaps.toml` (see `src/tpairs.rs`).

use geo_golden::{repo_root, tpairs};
use std::collections::BTreeMap;

/// The verified pairs the runner must find: raising the bar notices a lost
/// or truncated `verified.jsonl`.
const MIN_STEPS: usize = 1864;
const MIN_METHODS: usize = 580;

/// `GEOKERNEL_TPAIRS_COMMAND=Midpoint` narrows the run to one command, so a
/// lane can see its red pairs: only that command's verdicts are checked
/// (another command's new disagreement or stale entry does not fail this
/// run). The other commands still feed the counters, the per-command
/// summary and the regenerated gap list, so a blessed run stays complete.
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

    // Passes by kind: only the compared-value ones say the kernel computed
    // what GeoGebra computed.
    let mut passes: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut by_category: BTreeMap<&'static str, usize> = BTreeMap::new();
    // per command: pass, listed as a gap, total
    let mut by_command: BTreeMap<String, [usize; 3]> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();
    // The gap list as it should be now: id to (category, reason, labels).
    let mut current: BTreeMap<String, (tpairs::Category, String, BTreeMap<String, ()>)> =
        BTreeMap::new();
    let mut filtered_report = String::new();

    for method in &methods {
        for (pair, out) in method.steps.iter().zip(tpairs::replay_method(method)) {
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
                // Not in the report and not verdict-checked, but still part
                // of the counters and the regenerated gap list.
                if let Some(kind) = out.pass {
                    *passes.entry(kind.as_str()).or_default() += 1;
                    entry[0] += 1;
                }
                if let Some(category) = out.category {
                    *by_category.entry(category.as_str()).or_default() += 1;
                    current.insert(
                        out.id.clone(),
                        (category, preserved_reason(&gaps, &out), labels.clone()),
                    );
                }
                if listed.is_some() {
                    entry[1] += 1;
                }
                continue;
            }

            if filter.is_some() {
                let verdict = match (&out.pass, out.category) {
                    (Some(kind), _) => format!("PASS {}", kind.as_str()),
                    (None, Some(cat)) => format!("GAP {}", cat.as_str()),
                    (None, None) => "GAP".into(),
                };
                filtered_report
                    .push_str(&format!("\n    {} `{:?}` -> {verdict}", out.id, pair.input));
                for d in &out.disagreements {
                    filtered_report.push_str(&format!("\n      [{}] {}", d.label, d.why));
                }
            }

            match (out.pass, out.category, listed) {
                (Some(kind), None, None) => {
                    *passes.entry(kind.as_str()).or_default() += 1;
                    entry[0] += 1;
                }
                (Some(kind), None, Some((category, reason, _))) => {
                    // Still a pass (counted); the entry is stale and must go.
                    *passes.entry(kind.as_str()).or_default() += 1;
                    entry[0] += 1;
                    failures.push(format!(
                        "stale gap `{}` ({} {reason:?}): the step now passes as {}, remove \
                         the entry",
                        out.id,
                        category.as_str(),
                        kind.as_str()
                    ));
                }
                (None, Some(category), listed) => {
                    *by_category.entry(category.as_str()).or_default() += 1;
                    entry[1] += 1;
                    current.insert(
                        out.id.clone(),
                        (category, preserved_reason(&gaps, &out), labels.clone()),
                    );
                    // An entry matches only with the same category and the
                    // same mismatch labels: a cause that changed must be
                    // re-filed, not silently kept.
                    let matches_list =
                        listed.is_some_and(|(cat, _, ls)| *cat == category && *ls == labels);
                    if !matches_list {
                        if let Some((old, reason, ls)) = listed {
                            if *old != category {
                                failures.push(format!(
                                    "gap `{}` ({reason}) changed category: was {}, now {}; \
                                     update the entry",
                                    out.id,
                                    old.as_str(),
                                    category.as_str()
                                ));
                            } else {
                                let extra: Vec<&String> =
                                    labels.keys().filter(|l| !ls.contains_key(*l)).collect();
                                let gone: Vec<&String> =
                                    ls.keys().filter(|l| !labels.contains_key(*l)).collect();
                                failures.push(format!(
                                    "gap `{}` ({reason}) changed: new disagreements {extra:?}, \
                                     now agreeing {gone:?}; update its `mismatches` list",
                                    out.id
                                ));
                            }
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
                // A pass with a category or a gap without one cannot happen.
                (Some(_), Some(_), _) | (None, None, _) => unreachable!("inconsistent outcome"),
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

    let total_passes: usize = passes.values().sum();
    let headline = passes.get("compared-value").copied().unwrap_or(0);
    let other: Vec<String> = [
        "parse-only",
        "no-output",
        "error-class",
        "context-incomplete",
    ]
    .iter()
    .filter_map(|k| {
        passes
            .get(*k)
            .map(|n| format!("{n} {}", k.replace('-', " ")))
    })
    .collect();
    // Per-command summary: pass / listed gap / total, the reddest first.
    let mut summary = format!(
        "t-pairs: {steps} steps in {} methods: {headline} compared-value passes \
         ({total_passes} passes: {}, {} gaps, {} failures)",
        methods.len(),
        other.join(", "),
        by_category.values().sum::<usize>(),
        failures.len()
    );
    for (kind, n) in &passes {
        summary.push_str(&format!("\n  pass {kind}: {n}"));
    }
    for (category, n) in &by_category {
        summary.push_str(&format!("\n  gap {category}: {n}"));
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

    // Regenerates the gap list from the current state. A hand-written
    // reason survives a blessing when the entry itself did not change, and
    // the regeneration never runs in CI: CI checks the list, it must not
    // rewrite it.
    if std::env::var("GEOKERNEL_TPAIRS_BLESS").ok().as_deref() == Some("1") {
        assert_ne!(
            std::env::var("CI").ok().as_deref(),
            Some("true"),
            "GEOKERNEL_TPAIRS_BLESS rewrites the gap list; it must never run in CI"
        );
        std::fs::write(&gaps_path, tpairs::write_gaps(&current))
            .unwrap_or_else(|e| panic!("write {}: {e}", gaps_path.display()));
    }

    println!("{summary}");
    assert!(failures.is_empty(), "{}\n{}", summary, failures.join("\n"));
}

/// Phrases of the reasons the old runner wrote for its own refusals; such a
/// reason is never preserved, because the runner no longer refuses those
/// inputs that way and the kernel's own refusal text replaces it.
const STALE_RUNNER_REASONS: &[&str] = &[
    "the input assigns nothing and creates no object",
    "unlabelled output",
    "the object is missing only in its wake",
];

/// The reason a regenerated gap list holds for `out`: the file's own reason
/// while the entry (category and mismatch labels) is unchanged (and the
/// reason is not one of the old runner's), the first disagreement once it
/// changed.
fn preserved_reason(gaps: &geo_golden::tpairs::Gaps, out: &tpairs::StepOutcome) -> String {
    let labels: BTreeMap<String, ()> = out
        .disagreements
        .iter()
        .map(|d| (d.label.clone(), ()))
        .collect();
    match gaps.get(&out.id) {
        Some((category, reason, ls))
            if *category == out.category.unwrap()
                && *category != tpairs::Category::Cascade
                && *ls == labels
                && !STALE_RUNNER_REASONS.iter().any(|s| reason.contains(s))
                && !(*category == tpairs::Category::Redefine
                    && reason.ends_with("already exists")
                    && out.disagreements.first().is_some_and(|d| d.why != *reason)) =>
        {
            reason.clone()
        }
        _ => out
            .disagreements
            .first()
            .map(|d| {
                d.why
                    .replace('\n', " ")
                    .replace("GeoGebra", "the reference")
            })
            .unwrap_or_default(),
    }
}
