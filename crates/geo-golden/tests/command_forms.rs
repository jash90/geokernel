//! The forms columns of the command matrix are backed by data a test checks:
//!
//! * `tools/command-matrix/forms.csv` lists the 2D forms of exactly the
//!   in-scope commands of `docs/command-matrix.csv`;
//! * every row of `tools/command-matrix/form-evidence.toml` names golden
//!   cases that exist, are not known gaps and call the command in that form,
//!   a row of `DIFFERENCES.md` that names the form (`` `Mod(function,
//!   function)` `` in its first cell) and gives it a disposition, or both;
//! * no case of `known-gaps.toml` may call a form that has an evidence row,
//!   so a form counts as done only when every recorded case calling it
//!   agrees;
//! * `forms_total`, `forms_done` and the `done` status of the matrix agree
//!   with those two files (CI has no upstream checkout, so this is what keeps
//!   the matrix from going stale there).
//!
//! `cargo test -p geo-golden --test command_forms -- --ignored --nocapture`
//! prints, for every form, golden cases that show it: the candidates for
//! new evidence rows.

use geo_golden::forms::{self, CommandForms, Evidence};
use geo_golden::model::Case;
use geo_golden::model::Step;
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

/// A form can occur in the script or in later assignments and rejections.
/// Refusal evidence uses only rejections. Solving evidence needs an
/// assignment because a refusal shows no solving variable.
#[derive(Clone, Copy, PartialEq)]
enum Lines {
    All,
    Script,
    Rejected,
}

fn lines_of(e: &Evidence) -> Lines {
    if e.refusal {
        Lines::Rejected
    } else if matches!(
        e.command.as_str(),
        "Solve" | "Solutions" | "NSolve" | "NSolutions"
    ) {
        Lines::Script
    } else {
        Lines::All
    }
}

fn case_lines(case: &Case, which: Lines) -> impl Iterator<Item = &String> {
    case.script
        .iter()
        .filter(move |_| which != Lines::Rejected)
        .chain(case.steps.iter().filter_map(move |step| match step {
            Step::Reject { line, .. } if which != Lines::Script => Some(line),
            Step::Add { line } | Step::Redefine { line } | Step::Append { line }
                if which != Lines::Rejected =>
            {
                Some(line)
            }
            _ => None,
        }))
}

fn case_uses_form(
    case: &Case,
    which: Lines,
    names: &BTreeMap<String, String>,
    command: &str,
    form: &str,
) -> bool {
    let kinds = kinds_of(case);
    case_lines(case, which).any(|line| {
        forms::script_uses_form(std::slice::from_ref(line), &kinds, names, command, form)
    })
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

/// These signatures use a wildcard for a family of compatible operands.
fn numeric_wildcard(command: &str, form: &str) -> bool {
    matches!(
        (command, form),
        ("Dot" | "Relation", "any,any")
            | ("Defined", "any")
            | ("Name" | "ConstructionStep", "any")
            | ("IsInRegion", "point,region")
            | ("If", "function,any,...")
            | ("CountIf", "any,any,list")
            | ("KeepIf", "boolean,any,list")
    )
}

#[test]
fn every_evidence_row_is_backed() {
    let f = forms();
    let m = matrix();
    let names = names(&m);
    let cases = harness_cases();
    let plain_gaps = gaps::load().unwrap_or_else(|e| panic!("{e}"));
    let giac_gaps = gaps::load_giac().unwrap_or_else(|e| panic!("{e}"));
    let differences = forms::difference_rows(&read("DIFFERENCES.md"));
    let mut errors = Vec::new();
    for e in evidence() {
        let what = format!("`{}` `{}`", e.command, e.form);
        let gaps = if e.giac { &giac_gaps } else { &plain_gaps };
        if e.giac {
            // Done with Giac only when every recorded case calling the form
            // agrees with Giac.
            for case in cases.values() {
                if gaps.contains_key(&case.id)
                    && case_uses_form(case, lines_of(&e), &names, &e.command, &e.form)
                {
                    errors.push(format!(
                        "{what}: `{}` calls it and is a known gap with Giac",
                        case.id
                    ));
                }
            }
        }
        let Some(cf) = f.get(&e.command) else {
            errors.push(format!("{what}: no such in-scope command in forms.csv"));
            continue;
        };
        if !cf.forms.contains(&e.form) {
            errors.push(format!("{what}: forms.csv has no such form"));
            continue;
        }
        if e.refusal && e.cases.is_empty() {
            errors.push(format!(
                "{what}: a refused form needs a case that refuses it"
            ));
            continue;
        }
        if let Some(d) = &e.difference {
            if let Err(msg) = forms::check_difference(d, &e.command, &e.form, &differences) {
                errors.push(format!("{what}: DIFFERENCES.md: {msg}"));
            }
            if e.cases.is_empty() {
                continue;
            }
        }
        let required = transformation_kinds(&e.command, &e.form);
        if forms::is_wildcard(&e.form)
            && required.is_none()
            && e.difference.is_none()
            && !numeric_wildcard(&e.command, &e.form)
        {
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
            } else if !case_lines(case, lines_of(&e))
                .any(|line| forms::script_calls(std::slice::from_ref(line), &names, &e.command))
            {
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
            } else if !case_uses_form(case, lines_of(&e), &names, &e.command, &e.form) {
                errors.push(format!("{what}: `{id}` does not call it in this form"));
            }
        }
        if let Some(required) = required.filter(|_| e.difference.is_none()) {
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

/// `cargo test -p geo-golden --test command_forms print_giac_form_status
/// -- --ignored --nocapture` prints, for every form, how many recorded cases
/// call it and how many of those are known gaps with and without Giac.
#[test]
#[ignore]
fn print_giac_form_status() {
    let f = forms();
    let m = matrix();
    let names = names(&m);
    let cases = harness_cases();
    let plain = gaps::load().unwrap_or_else(|e| panic!("{e}"));
    let giac = gaps::load_giac().unwrap_or_else(|e| panic!("{e}"));
    let done: BTreeSet<(String, String)> = evidence()
        .into_iter()
        .map(|e| (e.command, e.form))
        .collect();
    for (command, cf) in &f {
        for form in &cf.forms {
            let which = if matches!(
                command.as_str(),
                "Solve" | "Solutions" | "NSolve" | "NSolutions"
            ) {
                Lines::Script
            } else {
                Lines::All
            };
            let using: Vec<&Case> = cases
                .values()
                .filter(|c| case_uses_form(c, which, &names, command, form))
                .collect();
            let gaps_plain = using.iter().filter(|c| plain.contains_key(&c.id)).count();
            let gaps_giac: Vec<&String> = using
                .iter()
                .filter(|c| giac.contains_key(&c.id))
                .map(|c| &c.id)
                .collect();
            if !using.is_empty()
                && gaps_giac.is_empty()
                && !done.contains(&(command.clone(), form.clone()))
            {
                let ids: Vec<&String> = using.iter().map(|c| &c.id).take(3).collect();
                println!("GIAC-DONE {command}|{form}|{ids:?}");
            }
            if gaps_plain != gaps_giac.len() || !done.contains(&(command.clone(), form.clone())) {
                println!(
                    "{command}({form}): {} cases, {gaps_plain} gaps, {} with Giac {:?}{}",
                    using.len(),
                    gaps_giac.len(),
                    gaps_giac.iter().take(4).collect::<Vec<_>>(),
                    if done.contains(&(command.clone(), form.clone())) {
                        " [done]"
                    } else {
                        ""
                    }
                );
            }
        }
    }
}

/// Prints evidence candidates; set GEOKERNEL_ALL_FORM_CANDIDATES to list
/// every matching case instead of the first three.
#[test]
#[ignore]
fn print_form_candidates() {
    let f = forms();
    let m = matrix();
    let names = names(&m);
    let cases = harness_cases();
    let gaps = gaps::load().unwrap_or_else(|e| panic!("{e}"));
    let limit = if std::env::var_os("GEOKERNEL_ALL_FORM_CANDIDATES").is_some() {
        usize::MAX
    } else {
        3
    };
    for (command, cf) in &f {
        for form in cf.forms.iter().filter(|f| !forms::is_wildcard(f)) {
            let ids: Vec<&String> = cases
                .values()
                .filter(|c| !gaps.contains_key(&c.id))
                .filter(|c| case_uses_form(c, Lines::All, &names, command, form))
                .map(|c| &c.id)
                .take(limit)
                .collect();
            if !ids.is_empty() {
                println!("\n[[form]]\ncommand = \"{command}\"\nform = \"{form}\"");
                println!("cases = {ids:?}");
            }
        }
    }
}

/// A form counts as done only when every recorded case that may call it
/// agrees: no case of `known-gaps.toml` may call a form of
/// `form-evidence.toml`. A call reaches a form as
/// [`forms::script_may_use_form`] reads it, so a circle reaches a `conic`
/// form and a nested call any form its arity fits, unless the call shows
/// another form of the command exactly.
/// A row marked `giac` is checked against the gaps that remain with the
/// Giac backend (`known-gaps-giac.toml` applied).
#[test]
fn no_done_form_is_called_by_a_known_gap() {
    let f = forms();
    let m = matrix();
    let names = names(&m);
    let cases = harness_cases();
    let plain_gaps = gaps::load().unwrap_or_else(|e| panic!("{e}"));
    let giac_gaps = gaps::load_giac().unwrap_or_else(|e| panic!("{e}"));
    let mut errors = Vec::new();
    for e in evidence() {
        let Some(cf) = f.get(&e.command) else {
            continue; // reported by every_evidence_row_is_backed
        };
        // A form that is done only with Giac is held to the gaps with Giac.
        let gaps = if e.giac { &giac_gaps } else { &plain_gaps };
        let which = lines_of(&e);
        let calling: Vec<&str> = gaps
            .keys()
            .filter(|id| {
                cases.get(*id).is_some_and(|case| {
                    let kinds = kinds_of(case);
                    case_lines(case, which).any(|line| {
                        forms::script_may_use_form(
                            std::slice::from_ref(line),
                            &kinds,
                            &names,
                            &e.command,
                            &cf.forms,
                            &e.form,
                        )
                    })
                })
            })
            .map(String::as_str)
            .collect();
        if !calling.is_empty() {
            errors.push(format!(
                "`{}` `{}` is done, but known gaps call it: {}",
                e.command,
                e.form,
                calling.join(", ")
            ));
        }
    }
    assert!(
        errors.is_empty(),
        "{} done forms are called by known gaps; remove their rows from \
         tools/command-matrix/form-evidence.toml:\n{}",
        errors.len(),
        errors.join("\n")
    );
}

#[test]
fn redefinitions_and_rejections_count_as_calls() {
    let text = r#"{"schema":4,"generator":"format-sample","upstream_commit":null,
      "cases":[{"id":"x","group":"g",
      "script":["A = (0, 0)", "k = Circle(A, 1)", "d = 1"],
      "steps":[{"action":"initial"},
               {"action":"redefine","line":"d = Radius(k)"},
               {"action":"reject","line":"e = Transpose(A)"}],
      "snapshots":[
        {"A":{"type":"point","defined":true,"x":0.0,"y":0.0},
         "k":{"type":"circle","defined":true,"cx":0.0,"cy":0.0,"r":1.0},
         "d":{"type":"number","defined":true,"value":1.0}},
        {"A":{"type":"point","defined":true,"x":0.0,"y":0.0},
         "k":{"type":"circle","defined":true,"cx":0.0,"cy":0.0,"r":1.0},
         "d":{"type":"number","defined":true,"value":1.0}},
        {"A":{"type":"point","defined":true,"x":0.0,"y":0.0},
         "k":{"type":"circle","defined":true,"cx":0.0,"cy":0.0,"r":1.0},
         "d":{"type":"number","defined":true,"value":1.0}}]}]}"#;
    let file = model::parse_file(text, "sample").unwrap_or_else(|e| panic!("{e}"));
    let case = &file.cases[0];
    let names = names(&matrix());
    let lines = |which| case_lines(case, which).cloned().collect::<Vec<_>>();
    assert!(lines(Lines::All).contains(&"d = Radius(k)".to_string()));
    assert!(lines(Lines::Script).contains(&"d = Radius(k)".to_string()));
    assert!(!lines(Lines::Rejected).contains(&"d = Radius(k)".to_string()));
    assert!(lines(Lines::Rejected).contains(&"e = Transpose(A)".to_string()));
    let kinds = kinds_of(case);
    let reaches = |which, command: &str, form: &str| {
        case_lines(case, which).any(|line| {
            forms::script_may_use_form(
                std::slice::from_ref(line),
                &kinds,
                &names,
                command,
                &forms()[command].forms,
                form,
            )
        })
    };
    assert!(reaches(Lines::All, "Radius", "conic"));
    assert!(reaches(Lines::Script, "Radius", "conic"));
    assert!(!reaches(Lines::Rejected, "Radius", "conic"));
}
