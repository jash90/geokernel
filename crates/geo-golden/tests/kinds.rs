//! `docs/kinds.csv`: one row per object kind, kernel kinds and the 2D kinds
//! of upstream alike, with what the project does with each.
//!
//! The rows: every `Kind` variant of `geo-kernel` (read from its source, as
//! the enum is `#[non_exhaustive]`) has exactly one row, a row names only
//! kernel kinds that exist, and every 2D object class of upstream has a row.
//!
//! The cells are `yes`, `partial` or `no`. Checked against the code, for
//! many objects of the kind (up to [`PER_KIND`] golden cases each, so the
//! shapes a kind takes in golden data are covered):
//!
//! * `kernel_golden`, exact: `yes` when a golden case recorded by the
//!   harness has an object of the kind (the golden test then compares it
//!   with the kernel), otherwise `no`;
//! * `svg`, exact: adding a line that creates only the object changes the
//!   image of the SVG writer (`geo_kernel::svg::render`): `yes` for every
//!   sample, `partial` for some, `no` for none. A function whose graph has
//!   no piece inside the view (golden cases of extreme numbers have such
//!   functions) is not a sample, as there is nothing to draw;
//! * `pointer_hit`, exact: `geo_app_core::hit::hit` hits the object somewhere
//!   on a 400 x 300 view, graded over the samples as for `svg`;
//! * `algebra_text`, exact: per sample, the text is proper when
//!   `geo_app_core::describe::is_known` knows the value and
//!   `describe::object_text` is not just its debug form, a fallback when it is
//!   the debug form, missing when the value is not known; `yes` when every
//!   sample is proper, `no` when none is known, otherwise `partial`;
//! * `app_draw`, `no` versus the rest: the window's drawing code
//!   (`crates/geo-app/src/draw.rs`, `crates/geo-app/src/curves/mod.rs`, with
//!   comments and string literals removed) names the value variant. A source
//!   scan, as the window crate is too heavy to link here; whether the cell is
//!   `yes` or `partial` is declared;
//! * `import`, `no` versus the rest: a file of `golden/ggb` imports to an
//!   object of the kind; `yes` or `partial` is declared.
//!
//! Declared, not checked: `tool`; `export` (there is no file writer, so the
//! column must be `no` everywhere).
//! A row without a kernel kind is `no` in every column.

use geo_app_core::describe;
use geo_app_core::hit::{self, Reach};
use geo_app_core::interact::ViewTransform;
use geo_golden::table::Table;
use geo_golden::{gaps, model, repo_root};
use geo_kernel::svg::{self, View};
use geo_kernel::{script, Construction, Id, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const COLUMNS: [&str; 12] = [
    "kind",
    "upstream",
    "kernel_kind",
    "kernel_golden",
    "svg",
    "app_draw",
    "pointer_hit",
    "tool",
    "algebra_text",
    "import",
    "export",
    "notes",
];
const CELLS: [&str; 8] = [
    "kernel_golden",
    "svg",
    "app_draw",
    "pointer_hit",
    "tool",
    "algebra_text",
    "import",
    "export",
];

/// The 2D object classes of upstream (`GeoClass` at commit 112919f, without
/// 3D, CAS, media, scripting actions and whiteboard-only objects).
const UPSTREAM_2D: [&str; 27] = [
    "ANGLE",
    "AXIS",
    "BOOLEAN",
    "BUTTON",
    "TEXTFIELD",
    "CONIC",
    "CONICPART",
    "FUNCTION",
    "IMAGE",
    "LINE",
    "LIST",
    "LOCUS",
    "NUMERIC",
    "POINT",
    "POLYGON",
    "RAY",
    "SEGMENT",
    "TEXT",
    "FORMULA",
    "VECTOR",
    "CURVE_CARTESIAN",
    "IMPLICIT_POLY",
    "FUNCTION_NVAR",
    "POLYLINE",
    "PENSTROKE",
    "TURTLE",
    "PIECHART",
];

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The variants of `pub enum Kind` in the kernel's source.
fn kernel_kinds() -> Vec<String> {
    let src = read("crates/geo-kernel/src/kernel.rs");
    let start = src
        .find("pub enum Kind {")
        .expect("pub enum Kind in kernel.rs");
    let kinds: Vec<String> = src[start + "pub enum Kind {".len()..]
        .lines()
        .map(str::trim)
        .take_while(|l| *l != "}")
        .filter(|l| !l.starts_with("//") && !l.starts_with('#') && !l.is_empty())
        .map(|l| l.trim_end_matches(',').to_string())
        .collect();
    for k in &kinds {
        assert!(
            k.chars().all(char::is_alphanumeric),
            "enum Kind: cannot read `{k}` (only unit variants are expected)"
        );
    }
    kinds
}

fn table() -> Vec<BTreeMap<String, String>> {
    let t =
        Table::parse(&read("docs/kinds.csv"), "docs/kinds.csv").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(t.header, COLUMNS, "docs/kinds.csv header");
    t.records()
        .into_iter()
        .map(|r| {
            r.into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        })
        .collect()
}

fn kind_name(c: &Construction, id: Id) -> String {
    format!("{:?}", c.kind(id))
}

/// Golden cases sampled per kind and check.
const PER_KIND: usize = 30;

/// Objects of each kind, found by running the golden scripts line by line;
/// at most one sample of a kind per case.
#[derive(Default)]
struct Samples {
    /// Kinds recorded in harness snapshots.
    recorded: BTreeSet<String>,
    /// Per kind: scripts and the label of a defined object of the kind.
    any: BTreeMap<String, Vec<(Vec<String>, String)>>,
    /// Per kind: scripts up to a line that creates only one object, of the
    /// kind and defined.
    single: BTreeMap<String, Vec<Vec<String>>>,
}

fn samples() -> Samples {
    let cases = model::load_all(&repo_root()).unwrap_or_else(|e| panic!("golden data:\n{e}"));
    let gaps = gaps::load().unwrap_or_else(|e| panic!("{e}"));
    let mut s = Samples::default();
    let mut cases: Vec<_> = cases
        .into_iter()
        .filter(|c| c.generator == "ggb-harness" && !gaps.contains_key(&c.id))
        .collect();
    cases.sort_by(|a, b| a.id.cmp(&b.id));
    for case in &cases {
        for e in case.snapshots[0].values() {
            s.recorded.insert(format!("{:?}", e.kind));
        }
    }
    for case in &cases {
        let mut c = Construction::new();
        let (mut any_here, mut single_here) = (BTreeSet::new(), BTreeSet::new());
        for (i, line) in case.script.iter().enumerate() {
            let before: BTreeSet<Id> = c.ids().collect();
            if script::exec(&mut c, line).is_err() {
                break;
            }
            let new: Vec<Id> = c.ids().filter(|id| !before.contains(id)).collect();
            for &id in &new {
                if matches!(c.value(id), Value::Undefined) {
                    continue;
                }
                let k = kind_name(&c, id);
                let any = s.any.entry(k.clone()).or_default();
                if any.len() < PER_KIND && any_here.insert(k.clone()) {
                    any.push((case.script[..=i].to_vec(), c.label(id).to_string()));
                }
                let single = s.single.entry(k.clone()).or_default();
                if new.len() == 1 && single.len() < PER_KIND && single_here.insert(k) {
                    single.push(case.script[..=i].to_vec());
                }
            }
        }
    }
    let rasters = include_bytes!("../../../golden/ggb/saved/saved-images-001.ggb");
    let imported = geo_import::import(rasters).unwrap();
    let c = script::load(&imported.script.join("\n")).unwrap();
    s.any.insert(
        "Image".into(),
        c.images()
            .map(|(id, _)| (imported.script.clone(), c.label(id).into()))
            .collect(),
    );
    s.single.retain(|_, v| !v.is_empty());
    s
}

/// `yes` when every sample passes, `no` when none does, else `partial`.
fn grade(passed: impl IntoIterator<Item = bool>) -> Option<&'static str> {
    let (mut yes, mut n) = (0, 0);
    for p in passed {
        n += 1;
        yes += usize::from(p);
    }
    match (yes, n) {
        (_, 0) => None,
        (0, _) => Some("no"),
        (y, n) if y == n => Some("yes"),
        _ => Some("partial"),
    }
}

/// Rust source with comments, string literals and character literals
/// removed, so a scan for a name sees only code.
fn code_only(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < b.len() {
                if b[i] == '/' && b.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && b.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            out.push(' ');
        } else if c == 'r'
            && matches!(next, Some('"' | '#'))
            && !b[..i]
                .last()
                .is_some_and(|p| p.is_alphanumeric() || *p == '_')
        {
            // raw string r"...", r#"..."#
            let mut j = i + 1;
            let mut hashes = 0;
            while b.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if b.get(j) != Some(&'"') {
                out.push(c);
                i += 1;
                continue;
            }
            j += 1;
            loop {
                if j >= b.len() {
                    break;
                }
                if b[j] == '"' && (0..hashes).all(|h| b.get(j + 1 + h) == Some(&'#')) {
                    j += 1 + hashes;
                    break;
                }
                j += 1;
            }
            out.push_str("\"\"");
            i = j;
        } else if c == '"' {
            i += 1;
            while i < b.len() && b[i] != '"' {
                i += if b[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            out.push_str("\"\"");
        } else if c == '\'' {
            // a character literal ('x', '\n', '\'') or a lifetime ('a)
            if next == Some('\\') {
                i += 2;
                while i < b.len() && b[i] != '\'' {
                    i += 1;
                }
                i += 1;
                out.push_str("' '");
            } else if b.get(i + 2) == Some(&'\'') {
                i += 3;
                out.push_str("' '");
            } else {
                out.push(c);
                i += 1;
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn build(lines: &[String]) -> Construction {
    let mut c = Construction::new();
    for l in lines {
        script::exec(&mut c, l).unwrap_or_else(|e| panic!("`{l}`: {e}"));
    }
    c
}

/// Whether the SVG writer draws the last line's object.
fn svg_draws(lines: &[String]) -> bool {
    let with = build(lines);
    let without = build(&lines[..lines.len() - 1]);
    let view = View::fit(&with);
    svg::render(&with, view, 400, 300) != svg::render(&without, view, 400, 300)
}

/// Whether the last line of a script made something there is to draw: a
/// function counts only when its graph has a piece inside the view, two
/// neighbouring x of 401 probes with finite values in it (probed
/// independently of the plotter; an isolated defined point is not drawn).
fn has_something_to_draw(lines: &[String]) -> bool {
    let c = build(lines);
    let before: BTreeSet<Id> = build(&lines[..lines.len() - 1]).ids().collect();
    let view = View::fit(&c);
    let inside = |id: Id, i: u32| {
        let x = view.xmin + (view.xmax - view.xmin) * f64::from(i) / 400.0;
        c.function_value(id, x)
            .is_some_and(|y| y.is_finite() && (view.ymin..=view.ymax).contains(&y))
    };
    c.ids().filter(|id| !before.contains(id)).all(|id| {
        !matches!(c.value(id), Value::Function)
            || (0..400).any(|i| inside(id, i) && inside(id, i + 1))
    })
}

/// The construction of a script and the id of `label` in it.
fn object(lines: &[String], label: &str) -> (Construction, Id) {
    let c = build(lines);
    let id = c.id(label).unwrap_or_else(|| panic!("no object `{label}`"));
    (c, id)
}

/// Whether a pointer anywhere on a 400 x 300 view (probed every 3 pixels)
/// hits the object.
fn pointer_hits(c: &Construction, id: Id) -> bool {
    let tr = ViewTransform::new(View::fit(c), 400.0, 300.0);
    let reach = Reach::square(3.0);
    (0..134).any(|i| {
        (0..100).any(|j| {
            let p = (3.0 * i as f64, 3.0 * j as f64);
            hit::hit(c, id, &tr, p, 3.0, &reach).is_some()
        })
    })
}

/// The text the window's drawing code must contain for a kind it draws.
fn draw_needle(kind: &str) -> String {
    match kind {
        // lines are drawn through `Value::unit_line`
        "Line" => "unit_line".into(),
        "Angle" => "angle_geometry".into(),
        "Boolean" => "Value::Bool".into(),
        k => format!("Value::{k}"),
    }
}

fn construction_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let p = e.unwrap().path();
        if p.is_dir() {
            construction_files(&p, out);
        } else if p
            .extension()
            .is_some_and(|x| x == "xml" || x == "ggb" && !p.with_extension("xml").exists())
        {
            out.push(p);
        }
    }
}

/// Kinds of the objects the importer makes from the files of `golden/ggb`.
fn imported_kinds() -> BTreeSet<String> {
    let mut files = Vec::new();
    construction_files(&repo_root().join("golden/ggb"), &mut files);
    assert!(!files.is_empty(), "no files in golden/ggb");
    let mut kinds = BTreeSet::new();
    for f in files {
        let Ok(imported) = geo_import::import(&std::fs::read(&f).unwrap()) else {
            continue;
        };
        let mut c = Construction::new();
        for line in &imported.script {
            if script::exec(&mut c, line).is_err() {
                break;
            }
        }
        kinds.extend(c.ids().map(|id| kind_name(&c, id)));
    }
    kinds
}

#[test]
fn every_kernel_kind_and_upstream_kind_has_a_row() {
    let rows = table();
    let kinds = kernel_kinds();
    assert!(kinds.len() >= 15, "parsed only {kinds:?} from enum Kind");
    let mut errors = Vec::new();
    for k in &kinds {
        let n = rows.iter().filter(|r| &r["kernel_kind"] == k).count();
        if n != 1 {
            errors.push(format!("Kind::{k} has {n} rows"));
        }
    }
    let mut names = BTreeSet::new();
    for r in &rows {
        let kk = &r["kernel_kind"];
        if !kk.is_empty() && !kinds.contains(kk) {
            errors.push(format!(
                "row `{}` names Kind::{kk}, which does not exist",
                r["kind"]
            ));
        }
        if !names.insert(r["kind"].clone()) {
            errors.push(format!("row `{}` is listed twice", r["kind"]));
        }
        if !UPSTREAM_2D.contains(&r["upstream"].as_str()) {
            errors.push(format!(
                "row `{}`: `{}` is no 2D upstream class",
                r["kind"], r["upstream"]
            ));
        }
        for col in CELLS {
            if !["yes", "partial", "no"].contains(&r[col].as_str()) {
                errors.push(format!("row `{}`: {col} is `{}`", r["kind"], r[col]));
            }
            if kk.is_empty() && r[col] != "no" {
                errors.push(format!(
                    "row `{}` has no kernel kind but {col} is `{}`",
                    r["kind"], r[col]
                ));
            }
        }
        if r["export"] != "no" {
            errors.push(format!(
                "row `{}`: there is no file writer to export with",
                r["kind"]
            ));
        }
    }
    for u in UPSTREAM_2D {
        if !rows.iter().any(|r| r["upstream"] == u) {
            errors.push(format!("the upstream class {u} has no row"));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn rust_source_scans_skip_comments_and_strings() {
    let src = "let a = Value::Point; // Value::Circle\n/* Value::Ray /* nested */ Value::Ray */\nlet s = \"Value::Segment\"; let r = r#\"Value::Line\"#;\nlet q = '\"'; let t = Value::Vector; fn f<'a>(x: &'a str) {} let e = '\\'';";
    let code = code_only(src);
    assert!(code.contains("Value::Point") && code.contains("Value::Vector"));
    for gone in [
        "Value::Circle",
        "Value::Ray",
        "Value::Segment",
        "Value::Line",
        "nested",
    ] {
        assert!(!code.contains(gone), "{gone} in {code}");
    }
    assert!(code.contains("fn f<'a>(x: &'a str)"));
    assert_eq!(grade([true, true]), Some("yes"));
    assert_eq!(grade([true, false]), Some("partial"));
    assert_eq!(grade([false]), Some("no"));
    assert_eq!(grade([]), None);
}

/// Grades one sample's algebra text: 2 proper, 1 the debug form, 0 unknown.
fn text_grade(c: &Construction, id: Id) -> u8 {
    let v = c.value(id);
    if !describe::is_known(&v) {
        0
    } else if v == Value::PieChart && describe::object_text(c, id) == "PieChart" {
        2
    } else if describe::object_text(c, id) == format!("{v:?}") {
        1
    } else {
        2
    }
}

#[test]
fn checked_cells_agree_with_the_code() {
    let rows: Vec<_> = table()
        .into_iter()
        .filter(|r| !r["kernel_kind"].is_empty())
        .collect();
    let s = samples();
    let imported = imported_kinds();
    let draw = code_only(
        &(read("crates/geo-app/src/draw.rs")
            + "\n"
            + &read("crates/geo-app/src/curves/mod.rs")
            + "\n"
            + &read("crates/geo-app/src/draw/image.rs")
            + "\n"
            + &read("crates/geo-app/src/draw/charts.rs")),
    );
    let mut errors = Vec::new();
    for r in &rows {
        let k = &r["kernel_kind"];
        // An exact value, or only whether the code handles the kind at all.
        let mut exact = |col: &str, actual: Option<&str>| match actual {
            Some(a) if a != r[col] => {
                errors.push(format!("{k}: {col} is `{}`, the code says `{a}`", r[col]))
            }
            None if r[col] != "no" => errors.push(format!(
                "{k}: {col} is `{}`, but golden data has no object of the kind to check it with",
                r[col]
            )),
            _ => {}
        };
        exact(
            "kernel_golden",
            Some(if s.recorded.contains(k) { "yes" } else { "no" }),
        );
        exact(
            "svg",
            s.single.get(k).and_then(|v| {
                grade(
                    v.iter()
                        .filter(|l| has_something_to_draw(l))
                        .map(|l| svg_draws(l)),
                )
            }),
        );
        let objects: Vec<(Construction, Id)> = s
            .any
            .get(k)
            .map(|v| v.iter().map(|(l, label)| object(l, label)).collect())
            .unwrap_or_default();
        exact(
            "pointer_hit",
            grade(objects.iter().map(|(c, id)| pointer_hits(c, *id))),
        );
        let texts: Vec<u8> = objects.iter().map(|(c, id)| text_grade(c, *id)).collect();
        exact(
            "algebra_text",
            (!texts.is_empty()).then(|| {
                if texts.iter().all(|&t| t == 2) {
                    "yes"
                } else if texts.iter().all(|&t| t == 0) {
                    "no"
                } else {
                    "partial"
                }
            }),
        );
        let mut handles = |col: &str, actual: bool| {
            if actual != (r[col] != "no") {
                errors.push(format!(
                    "{k}: {col} is `{}`, the code says {}",
                    r[col],
                    if actual { "it handles the kind" } else { "no" }
                ));
            }
        };
        handles("app_draw", draw.contains(&draw_needle(k)));
        handles("import", imported.contains(k));
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
