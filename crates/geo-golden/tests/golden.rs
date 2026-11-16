//! Loads every golden file, replays each case on `geo-kernel` and compares.

use geo_golden::{gaps, model, repo_root, report};

/// Number of cases recorded by `ggb-harness` so far. Raise it when adding cases,
/// so that losing a file is noticed.
const MIN_GOLDEN_CASES: usize = 2650;

#[test]
fn kernel_agrees_with_golden_data() {
    let cases = model::load_all(&repo_root()).unwrap_or_else(|e| panic!("golden data:\n{e}"));
    let gaps = gaps::load().unwrap_or_else(|e| panic!("{e}"));
    let r = report::evaluate(&cases, &gaps);
    println!("{}", r.summary());
    assert!(
        r.harness_cases.cmp(&MIN_GOLDEN_CASES).is_ge(),
        "only {} ggb-harness cases, expected at least {MIN_GOLDEN_CASES}",
        r.harness_cases
    );
    assert!(
        r.failures.is_empty(),
        "{}\n{}",
        r.summary(),
        r.failures.join("\n")
    );
}
