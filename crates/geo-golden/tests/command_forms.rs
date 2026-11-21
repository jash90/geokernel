//! The forms columns of the command matrix are backed by data a test checks:
//!
//! * `tools/command-matrix/forms.csv` lists the 2D forms of exactly the
//!   in-scope commands of `docs/command-matrix.csv`;
//! * every row of `tools/command-matrix/form-evidence.toml` names golden
//!   cases that exist, are not known gaps and call the command in that form,
//!   or a row of `DIFFERENCES.md` that names the form (`` `Mod(function,
//!   function)` `` in its first cell) and gives it a disposition;
//! * `forms_total`, `forms_done` and the `done` status of the matrix agree
//!   with those two files (CI has no upstream checkout, so this is what keeps
//!   the matrix from going stale there).
//!
//! `cargo test -p geo-golden --test command_forms -- --ignored --nocapture`
//! prints, for every form, golden cases that show it: the candidates for
//! new evidence rows.

use geo_golden::forms::{self, CommandForms, Evidence};
use geo_golden::model::Case;
use geo_golden::table::Table;
use geo_golden::{gaps, model, repo_root};
use geo_kernel::Kind;
use std::collections::{BTreeMap, BTreeSet};

const MATRIX_COLUMNS: [&str; 8] = [
    "command",
    "alias_of",
    "group",
    "priority",
    "status",
    "forms_total",
    "forms_done",
    "notes",
];

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn forms() -> BTreeMap<String, CommandForms> {
    forms::parse_forms(&read("tools/command-matrix/forms.csv")).unwrap_or_else(|e| panic!("{e}"))
}

fn evidence() -> Vec<Evidence> {
    forms::parse_evidence(&read("tools/command-matrix/form-evidence.toml"))
        .unwrap_or_else(|e| panic!("{e}"))
}

fn matrix() -> Table {
    let t = Table::parse(&read("docs/command-matrix.csv"), "docs/command-matrix.csv")
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(t.header, MATRIX_COLUMNS, "docs/command-matrix.csv header");
    t
}

/// Every name of the matrix (commands and aliases) to the command it means.
fn names(m: &Table) -> BTreeMap<String, String> {
    m.rows
        .iter()
        .map(|r| {
            let target = if r[1].is_empty() { &r[0] } else { &r[1] };
            (r[0].clone(), target.clone())
        })
        .collect()
}

fn harness_cases() -> BTreeMap<String, Case> {
    model::load_all(&repo_root())
        .unwrap_or_else(|e| panic!("golden data:\n{e}"))
        .into_iter()
        .filter(|c| c.generator == "ggb-harness")
        .map(|c| (c.id.clone(), c))
        .collect()
}

fn kinds_of(case: &Case) -> BTreeMap<String, Kind> {
    case.snapshots[0]
        .iter()
        .map(|(l, e)| (l.clone(), e.kind))
        .collect()
}

#[test]
fn forms_csv_lists_the_in_scope_commands_of_the_matrix() {
    let f = forms();
    let m = matrix();
    let in_scope: Vec<&String> = m
        .rows
        .iter()
        .filter(|r| r[1].is_empty() && r[4] != "out_of_scope")
        .map(|r| &r[0])
        .collect();
    let listed: Vec<&String> = f.keys().collect();
    let mut sorted = in_scope.clone();
    sorted.sort();
    assert_eq!(
        sorted, listed,
        "forms.csv and the in-scope rows of the matrix"
    );
    for r in &m.rows {
        if let Some(cf) = f.get(&r[0]) {
            assert_eq!(cf.group, r[2], "group of `{}`", r[0]);
        }
    }
}

/// Required source kinds for each wildcard transformation signature.
fn transformation_kinds(command: &str, form: &str) -> Option<&'static [Kind]> {
    use Kind::{Arc, Circle, Conic, Line, Point, PolyLine, Polygon, Ray, Sector, Segment, Vector};

    const INVERSION: &[Kind] = &[Point, Line, Segment, Circle, Arc, Sector];
    const AFFINE: &[Kind] = &[
        Point, Line, Segment, Circle, Ray, Vector, Polygon, PolyLine, Arc, Sector, Conic,
    ];
    const AXIS: &[Kind] = &[
        Point, Line, Segment, Circle, Ray, Vector, Polygon, Arc, Sector, Conic,
    ];
    match (command, form) {
        ("Mirror", "any,circle") => Some(INVERSION),
        ("Mirror", "any,point" | "any,line")
        | ("Rotate", "any,number" | "any,number,point")
        | ("Translate", "any,vector") => Some(AFFINE),
        ("Shear", "any,line,number" | "any,vector,number")
        | ("Stretch", "any,vector" | "any,line,number") => Some(AXIS),
        _ => None,
    }
}

fn transformed_kinds<'a>(
    case: &'a Case,
    names: &'a BTreeMap<String, String>,
    command: &'a str,
    suffix: &'a str,
) -> impl Iterator<Item = Kind> + 'a {
    let labels = kinds_of(case);
    case.script.iter().filter_map(move |line| {
        let (label, rhs) = line.split_once('=')?;
        let (call, args) = rhs.trim().split_once('(')?;
        if names.get(call)? != command
            || !case
                .snapshots
                .iter()
                .any(|objects| objects.get(label.trim()).is_some_and(|o| o.data.is_some()))
        {
            return None;
        }
        let args = forms::split_args(args.trim_end().strip_suffix(')')?);
        let (src, rest) = args.split_first()?;
        let rest: Vec<_> = rest
            .iter()
            .map(|arg| forms::arg_kind(arg, &labels))
            .collect();
        forms::form_matches(suffix, &rest)
            .then(|| forms::arg_kind(src, &labels))
            .flatten()
    })
}

#[test]
fn every_evidence_row_is_backed() {
    let f = forms();
    let m = matrix();
    let names = names(&m);
    let cases = harness_cases();
    let gaps = gaps::load().unwrap_or_else(|e| panic!("{e}"));
    let differences = forms::difference_rows(&read("DIFFERENCES.md"));
    let mut errors = Vec::new();
    for e in evidence() {
        let what = format!("`{}` `{}`", e.command, e.form);
        let Some(cf) = f.get(&e.command) else {
            errors.push(format!("{what}: no such in-scope command in forms.csv"));
            continue;
        };
        if !cf.forms.contains(&e.form) {
            errors.push(format!("{what}: forms.csv has no such form"));
            continue;
        }
        if let Some(d) = &e.difference {
            if let Err(msg) = forms::check_difference(d, &e.command, &e.form, &differences) {
                errors.push(format!("{what}: DIFFERENCES.md: {msg}"));
            }
            continue;
        }
        let required = transformation_kinds(&e.command, &e.form);
        if forms::is_wildcard(&e.form) && required.is_none() {
            errors.push(format!(
                "{what}: golden cases do not cover this wildcard form"
            ));
            continue;
        }
        let mut covered = BTreeSet::new();
        for id in &e.cases {
            let Some(case) = cases.get(id) else {
                errors.push(format!("{what}: no golden case `{id}`"));
                continue;
            };
            if gaps.contains_key(id) {
                errors.push(format!("{what}: `{id}` is a known gap"));
            } else if !forms::script_calls(&case.script, &names, &e.command) {
                errors.push(format!("{what}: `{id}` does not use the command"));
            } else if let Some(required) = required {
                let suffix = e.form.strip_prefix("any,").unwrap();
                let kinds: Vec<_> = transformed_kinds(case, &names, &e.command, suffix)
                    .filter(|kind| required.contains(kind))
                    .map(|kind| kind.to_string())
                    .collect();
                if kinds.is_empty() {
                    errors.push(format!("{what}: `{id}` has no defined image in this form"));
                }
                covered.extend(kinds);
            } else if !forms::script_uses_form(
                &case.script,
                &kinds_of(case),
                &names,
                &e.command,
                &e.form,
            ) {
                errors.push(format!("{what}: `{id}` does not call it in this form"));
            }
        }
        if let Some(required) = required {
            for kind in required {
                if !covered.contains(&kind.to_string()) {
                    errors.push(format!("{what}: no case for source kind {kind}"));
                }
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn matrix_counts_agree_with_forms_and_evidence() {
    let f = forms();
    let m = matrix();
    let mut done: BTreeMap<&str, usize> = BTreeMap::new();
    let ev = evidence();
    for e in &ev {
        *done.entry(e.command.as_str()).or_default() += 1;
    }
    let mut errors = Vec::new();
    for r in &m.rows {
        let (command, status, total, got) = (&r[0], &r[4], &r[5], &r[6]);
        if !r[1].is_empty() || status == "out_of_scope" {
            if !total.is_empty() || !got.is_empty() {
                errors.push(format!(
                    "`{command}`: an alias or out-of-scope row has no counts"
                ));
            }
            continue;
        }
        let Some(cf) = f.get(command) else {
            continue; // reported by the test of forms.csv
        };
        let want_total = cf.forms.len();
        let want_done = done.get(command.as_str()).copied().unwrap_or(0);
        if *total != want_total.to_string() || *got != want_done.to_string() {
            errors.push(format!(
                "`{command}`: forms {got}/{total}, forms.csv and the evidence give {want_done}/{want_total}"
            ));
        }
        // The counts alone decide the status.
        let want_status = if want_total > 0 && want_done == want_total {
            "done"
        } else if want_done > 0 {
            "partial"
        } else {
            "todo"
        };
        if status != want_status {
            errors.push(format!(
                "`{command}`: status `{status}` with {want_done}/{want_total}, expected `{want_status}`"
            ));
        }
    }
    assert!(
        errors.is_empty(),
        "regenerate docs/command-matrix.csv (tools/command-matrix/README.md):\n{}",
        errors.join("\n")
    );
}

/// Prints evidence candidates; see the module documentation.
#[test]
#[ignore]
fn print_form_candidates() {
    let f = forms();
    let m = matrix();
    let names = names(&m);
    let cases = harness_cases();
    let gaps = gaps::load().unwrap_or_else(|e| panic!("{e}"));
    for (command, cf) in &f {
        for form in cf.forms.iter().filter(|f| !forms::is_wildcard(f)) {
            let ids: Vec<&String> = cases
                .values()
                .filter(|c| !gaps.contains_key(&c.id))
                .filter(|c| forms::script_uses_form(&c.script, &kinds_of(c), &names, command, form))
                .map(|c| &c.id)
                .take(3)
                .collect();
            if !ids.is_empty() {
                println!("\n[[form]]\ncommand = \"{command}\"\nform = \"{form}\"");
                println!("cases = {ids:?}");
            }
        }
    }
}
