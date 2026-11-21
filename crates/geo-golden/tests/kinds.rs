//! `docs/kinds.csv`: one row per object kind, kernel kinds and the 2D kinds
//! of upstream alike, with what the project does with each.
//!
//! The rows: every `Kind` variant of `geo-kernel` (read from its source, as
//! the enum is `#[non_exhaustive]`) has exactly one row, a row names only
//! kernel kinds that exist, and every 2D object class of upstream has a row.
//!
//! The cells are `yes`, `partial` or `no`. Checked against the code, where
//! `yes` and `partial` both mean "the code handles some object of the kind":
//!
//! * `kernel_golden`: a golden case recorded by the harness has an object of
//!   the kind (the golden test then compares it with the kernel);
//! * `svg`: adding the object's line changes the image of the SVG writer
//!   (`geo_kernel::svg::render`), for a line that creates only that object;
//! * `app_draw`: the window's drawing code (`crates/geo-app/src/draw.rs`,
//!   `crates/geo-app/src/curves/mod.rs`) names the value variant (a source
//!   scan; the window crate is too heavy to link here);
//! * `pointer_hit`: `geo_app_core::hit::hit` hits the object somewhere on a
//!   400 x 300 view;
//! * `algebra_text`: `geo_app_core::describe::is_known` knows the value
//!   (otherwise the row says the kind is not shown yet);
//! * `import`: a file of `golden/ggb` imports to an object of the kind.
//!
//! Declared, not checked: `tool`, and `export` (there is no file writer, so
//! the column must be `no` everywhere). Whether a cell is `yes` or `partial`
//! is declared too. A row without a kernel kind is `no` in every column.

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

/// Kinds whose SVG cell cannot be checked: no golden line creates an object
/// of the kind alone, so its effect on the image cannot be separated (every
/// `Polygon` line creates the sides as well). The test fails once a line
/// exists, so the entry does not go stale.
const SVG_UNCHECKED: [&str; 1] = ["Polygon"];

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

/// Objects of each kind, found by running the golden scripts line by line.
#[derive(Default)]
struct Samples {
    /// Kinds recorded in harness snapshots.
    recorded: BTreeSet<String>,
    /// Per kind: a script and the label of a defined object of the kind.
    any: BTreeMap<String, (Vec<String>, String)>,
    /// Per kind: the script up to a line that creates only one object, of
    /// the kind and defined.
    single: BTreeMap<String, Vec<String>>,
}

fn samples(wanted: &BTreeSet<String>) -> Samples {
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
        if wanted.iter().all(|k| s.single.contains_key(k)) {
            break;
        }
        let mut c = Construction::new();
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
                if !s.any.contains_key(&k) {
                    let label = c.label(id).to_string();
                    s.any.insert(k.clone(), (case.script[..=i].to_vec(), label));
                }
                if new.len() == 1 && !s.single.contains_key(&k) {
                    s.single.insert(k, case.script[..=i].to_vec());
                }
            }
        }
    }
    s
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

/// The construction of a script and the id of `label` in it.
fn object(lines: &[String], label: &str) -> (Construction, Id) {
    let c = build(lines);
    let id = c.id(label).unwrap_or_else(|| panic!("no object `{label}`"));
    (c, id)
}

/// Whether a pointer anywhere on a 400 x 300 view hits the object.
fn pointer_hits(c: &Construction, id: Id) -> bool {
    let tr = ViewTransform::new(View::fit(c), 400.0, 300.0);
    let reach = Reach::square(3.0);
    (0..200).any(|i| {
        (0..150).any(|j| {
            let p = (2.0 * i as f64, 2.0 * j as f64);
            hit::hit(c, id, &tr, p, 3.0, &reach).is_some()
        })
    })
}

/// The text the window's drawing code must contain for a kind it draws.
fn draw_needle(kind: &str) -> String {
    match kind {
        // lines are drawn through `Value::unit_line`
        "Line" => "unit_line".into(),
        "Boolean" => "Value::Bool".into(),
        k => format!("Value::{k}"),
    }
}

fn xml_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let p = e.unwrap().path();
        if p.is_dir() {
            xml_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "xml") {
            out.push(p);
        }
    }
}

/// Kinds of the objects the importer makes from the files of `golden/ggb`.
fn imported_kinds() -> BTreeSet<String> {
    let mut files = Vec::new();
    xml_files(&repo_root().join("golden/ggb"), &mut files);
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
fn checked_cells_agree_with_the_code() {
    let rows: Vec<_> = table()
        .into_iter()
        .filter(|r| !r["kernel_kind"].is_empty())
        .collect();
    let wanted: BTreeSet<String> = rows.iter().map(|r| r["kernel_kind"].clone()).collect();
    let s = samples(&wanted);
    let imported = imported_kinds();
    let draw = read("crates/geo-app/src/draw.rs") + &read("crates/geo-app/src/curves/mod.rs");
    let mut errors = Vec::new();
    for r in &rows {
        let k = &r["kernel_kind"];
        let svg_unchecked = SVG_UNCHECKED.contains(&k.as_str());
        if svg_unchecked && s.single.contains_key(k) {
            errors.push(format!(
                "{k}: a golden line now creates it alone; drop it from SVG_UNCHECKED"
            ));
        }
        let mut check = |col: &str, actual: Option<bool>| {
            let claimed = r[col] != "no";
            match actual {
                Some(a) if a != claimed => errors.push(format!(
                    "{k}: {col} is `{}`, the code says {}",
                    r[col],
                    if a { "yes" } else { "no" }
                )),
                None if claimed => errors.push(format!(
                    "{k}: {col} is `{}`, but golden data has no object of the kind to check it with",
                    r[col]
                )),
                _ => {}
            }
        };
        check("kernel_golden", Some(s.recorded.contains(k)));
        if !svg_unchecked {
            check("svg", s.single.get(k).map(|l| svg_draws(l)));
        }
        check("app_draw", Some(draw.contains(&draw_needle(k))));
        let sample = s.any.get(k).map(|(lines, label)| object(lines, label));
        check(
            "pointer_hit",
            sample.as_ref().map(|(c, id)| pointer_hits(c, *id)),
        );
        check(
            "algebra_text",
            sample
                .as_ref()
                .map(|(c, id)| describe::is_known(&c.value(*id))),
        );
        check("import", Some(imported.contains(k)));
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
