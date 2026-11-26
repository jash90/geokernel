//! The oracle test: every construction file that GeoGebra wrote for a golden
//! case (`golden/ggb/<group>/<case id>.xml`, plus a few zipped `.ggb`) is
//! imported, the script it gives runs on the kernel, and every imported object
//! is compared with the reference reload or `snapshots[0]` of the same golden
//! case when loading keeps the original value, under the rules of
//! `golden/SCHEMA.md`, with the comparison of `geo-golden`.

use geo_golden::compare::{
    compare_arc, compare_conic, compare_construction_object, compare_object, numbers_agree,
};
use geo_golden::model::ExpectedData;
use geo_golden::model::{self, Case};
use geo_golden::{gaps, repo_root};
use geo_import::import;
use geo_kernel::script::exec;
use geo_kernel::Construction;
use geo_kernel::Kind;
use geo_kernel::Value;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// At the time of writing (see the assertions below for the exact counts).
const MIN_FILES: usize = 1636;
const MIN_ZIPS: usize = 37;
const MIN_COMPARED: usize = 13091;
/// Every labelled output in this corpus is importable.
const MAX_SKIPPED: usize = 0;

/// An object whose imported value is the golden one, but whose saved conic
/// matrix, classified on a fresh record, is a different object: the live
/// object kept the type of the conic it was made from (a dilated degenerate
/// conic is not classified again), the saved matrix alone does not.
#[derive(Deserialize)]
struct Reclassified {
    id: String,
    label: String,
    /// The saved value exactly as the cross-check reports it.
    saved: String,
    reason: String,
}

#[derive(Deserialize)]
struct ImportGapFile {
    #[serde(default)]
    reclassified: Vec<Reclassified>,
}

/// Whether two kernel values are the same within the tolerance of the golden files.
fn same_value(a: &Value, b: &Value) -> bool {
    let n = numbers_agree;
    match (*a, *b) {
        (Value::Point { x, y }, Value::Point { x: p, y: q })
        | (Value::Vector { x, y }, Value::Vector { x: p, y: q }) => n(x, p) && n(y, q),
        (Value::Line { a, b, c }, Value::Line { a: d, b: e, c: f }) => {
            n(a, d) && n(b, e) && n(c, f)
        }
        (
            Value::Segment { x1, y1, x2, y2 },
            Value::Segment {
                x1: p1,
                y1: q1,
                x2: p2,
                y2: q2,
            },
        ) => n(x1, p1) && n(y1, q1) && n(x2, p2) && n(y2, q2),
        (Value::Circle { cx, cy, r }, Value::Circle { cx: p, cy: q, r: s }) => {
            n(cx, p) && n(cy, q) && n(r, s)
        }
        (Value::Number(x), Value::Number(y)) => n(x, y),
        (a, b) => a == b,
    }
}

/// Outputs without a label: GeoGebra labels an object only once it is defined.
const MAX_UNLABELLED: usize = 5;

fn files(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(&path, extension, out);
        } else if path.extension().is_some_and(|e| e == extension) {
            out.push(path);
        }
    }
}

/// A reason without the names and numbers that differ from file to file.
fn reason_key(reason: &str) -> String {
    if reason.starts_with("depends on") {
        return "depends on a skipped object".into();
    }
    let mut key = String::new();
    let mut quoted = false;
    for ch in reason.chars() {
        match ch {
            '`' => {
                quoted = !quoted;
                key.push('`');
            }
            _ if quoted => {}
            _ => key.push(ch),
        }
    }
    key
}

#[derive(Default)]
struct Totals {
    files: usize,
    zips: usize,
    objects_compared: usize,
    objects_skipped: usize,
    gap_objects: usize,
    skipped_by_reason: BTreeMap<String, usize>,
    /// The first case that was skipped for each reason.
    example: BTreeMap<String, String>,
    /// Outputs without a label in the file: GeoGebra labels an object only
    /// once it is defined, so the snapshot must say undefined.
    unlabelled: usize,
    unlabelled_objects: usize,
    skipped_by_group: BTreeMap<String, usize>,
    hidden: usize,
    /// `case:label` of every skipped object.
    skipped_labels: BTreeSet<String>,
    /// `case:label` of every object whose computed value differs from the
    /// value saved in its file.
    mismatched: BTreeMap<String, (String, String)>,
}

fn compare_case(
    case: &Case,
    bytes: &[u8],
    gaps: &gaps::Gaps,
    reload: Option<&BTreeMap<String, f64>>,
    totals: &mut Totals,
    failures: &mut Vec<String>,
) -> Vec<String> {
    let imported = match import(bytes) {
        Ok(i) => i,
        Err(e) => {
            failures.push(format!("{}: import failed: {e}", case.id));
            return vec![];
        }
    };
    let mut c = Construction::new();
    for line in &imported.script {
        if let Err(e) = exec(&mut c, line) {
            failures.push(format!(
                "{}: the imported line `{line}` fails: {e}",
                case.id
            ));
            return imported.script;
        }
    }
    if let Some(steps) = &case.spreadsheet {
        let last = steps.last().expect("spreadsheet steps");
        let actual: BTreeMap<_, _> = c
            .spreadsheet_cells()
            .map(|(address, _)| (address.label(), c.spreadsheet_display(address)))
            .collect();
        if actual != last.cells || !imported.skipped.is_empty() || !imported.mismatched.is_empty() {
            failures.push(format!(
                "{}: imported spreadsheet differs: {actual:?}; skipped {:?}; saved mismatches {:?}",
                case.id, imported.skipped, imported.mismatched
            ));
        }
        for (label, expected) in &last.outputs {
            let matched = c
                .id(label)
                .ok_or_else(|| "missing output".into())
                .and_then(|id| compare_construction_object(expected, &c, id).map(|_| ()));
            if let Err(error) = matched {
                failures.push(format!("{}: {label}: {error}", case.id));
            }
        }
        totals.objects_compared += last.cells.len() + last.outputs.len();
        totals.hidden += imported.hidden.len();
        totals.objects_skipped += imported.skipped.len();
        return imported.script;
    }
    totals.hidden += imported.hidden.len();
    let skipped: HashMap<&str, &str> = imported
        .skipped
        .iter()
        .map(|s| (s.label.as_str(), s.reason.as_str()))
        .collect();
    for m in &imported.mismatched {
        totals.mismatched.insert(
            format!("{}:{}", case.id, m.label),
            (m.saved.clone(), m.computed.clone()),
        );
    }
    // What the kernel gives for the golden script itself.
    let direct = geo_kernel::script::load(&case.script.join("\n")).ok();
    // The command names of the outputs the file left without a label.
    let mut unlabelled_kinds: Vec<String> = Vec::new();
    for s in &imported.skipped {
        if !s.label.is_empty() {
            totals
                .skipped_labels
                .insert(format!("{}:{}", case.id, s.label));
        }
        if s.label.is_empty() {
            unlabelled_kinds.push(s.kind.clone());
            totals.unlabelled += 1;
            continue;
        }
        let key = format!("{}: {}", s.kind, reason_key(&s.reason));
        totals.example.entry(key.clone()).or_insert_with(|| {
            format!(
                "{} `{}`: {}\n    script: {:?}",
                case.id, s.label, s.reason, case.script
            )
        });
        *totals.skipped_by_reason.entry(key).or_default() += 1;
    }
    let gap = gaps.get(&case.id);
    for (label, expected) in &case.snapshots[0] {
        let reload_expected = reload.and_then(|values| values.get(label)).map(|&value| {
            assert_eq!(expected.kind, Kind::Number);
            assert!(
                matches!(expected.data, Some(ExpectedData::Number(original)) if !numbers_agree(original, value)),
                "{}: {label} no longer changes on reference reload", case.id
            );
            let mut reloaded = expected.clone();
            reloaded.data = Some(ExpectedData::Number(value));
            reloaded
        });
        let expected = reload_expected.as_ref().unwrap_or(expected);
        if skipped.contains_key(label.as_str()) {
            totals.objects_skipped += 1;
            *totals
                .skipped_by_group
                .entry(case.group.clone())
                .or_default() += 1;
            continue;
        }
        let Some(id) = c.id(label) else {
            // The file has no label for it, so it must be undefined.
            // It must match by command: the script line that defines the
            // object (or, for a generated label such as `t_{1}`, the line of
            // its prefix) names the command of an output-less entry.
            let prefix = label.split("_{").next().unwrap_or(label);
            let command = case.script.iter().find_map(|l| {
                let (lhs, rhs) = l.split_once('=')?;
                let lhs = lhs.trim();
                (lhs == label || lhs == prefix).then(|| rhs.split('(').next().unwrap_or("").trim())
            });
            let slot = command.and_then(|name| unlabelled_kinds.iter().position(|k| k == name));
            if let (true, Some(slot)) = (expected.data.is_none(), slot) {
                unlabelled_kinds.remove(slot);
                totals.unlabelled_objects += 1;
            } else {
                failures.push(format!(
                    "{}: `{label}` was neither imported nor skipped",
                    case.id
                ));
            }
            continue;
        };
        let vertices: Vec<_> = c.vertices(id).collect();
        // Parts of conics and general conics are compared through their
        // records, as `geo_golden` does when it replays the case.
        let compared = match c.kind(id) {
            Kind::Arc | Kind::Sector => {
                compare_arc(expected, c.kind(id), &c.value(id), c.arc(id).as_ref())
            }
            Kind::Conic => {
                compare_conic(expected, c.kind(id), &c.value(id), c.conic(id).as_ref()).0
            }
            Kind::PieChart | Kind::List => compare_construction_object(expected, &c, id),
            _ => compare_object(expected, c.kind(id), &c.value(id), &vertices),
        };
        match compared {
            Ok(_) => totals.objects_compared += 1,
            Err(why) => {
                let key = format!("0:{label}");
                if gap.is_some_and(|g| g.mismatches.contains(&key)) {
                    // A kernel gap: the importer must add nothing to it, so
                    // the imported value is what the kernel gives for the
                    // golden script itself.
                    let same = direct
                        .as_ref()
                        .and_then(|d| d.id(label).map(|i| same_value(&d.value(i), &c.value(id))));
                    if same == Some(true) {
                        totals.gap_objects += 1;
                    } else {
                        failures.push(format!(
                            "{}: `{label}` is a known kernel gap, but the import gives {:?}, \
                             the golden script {:?}",
                            case.id,
                            c.value(id),
                            direct
                                .as_ref()
                                .and_then(|d| d.id(label).map(|i| d.value(i)))
                        ));
                    }
                } else {
                    failures.push(format!(
                        "{}: `{label}` disagrees: {why}; expected {expected:?}, got {:?}",
                        case.id,
                        c.value(id)
                    ));
                }
            }
        }
    }
    // Objects the import made that the snapshot does not have: only the
    // sides and new vertices of a polygon and the like, which the kernel
    // labels itself, and the image points of transformed segments, and only
    // defined ones (GeoGebra labels no other).
    for id in c.ids() {
        let label = c.label(id);
        if case.snapshots[0].contains_key(label) {
            continue;
        }
        let polygon_family = case.script.iter().any(|l| {
            ["Polygon(", "Vertex(", "InteriorAngles(", "Angle("]
                .iter()
                .any(|f| l.contains(f))
        });
        // the image points of a transformed segment or ray (`A'`, `B'`, or default
        // labels such as `C`)
        let image = matches!(case.group.as_str(), "transform" | "vector");
        if !(polygon_family || image) || !c.value(id).is_defined() {
            failures.push(format!(
                "{}: the import made `{label}`, which the snapshot does not have",
                case.id
            ));
        }
    }
    imported.script
}

#[test]
fn a_line_distance_cache_that_matches_neither_order_is_reported() {
    let source = include_str!("../../../golden/ggb/conic/conic-diameter-009.xml");
    let original = r#"<value val="0.8944271909999159"/>"#;
    assert!(source.contains(original));
    let corrupt = source.replace(original, r#"<value val="99"/>"#);
    let imported = import(corrupt.as_bytes()).unwrap();
    assert!(imported.skipped.is_empty());
    assert_eq!(imported.mismatched.len(), 1);
    assert_eq!(imported.mismatched[0].label, "d");
    assert_eq!(imported.mismatched[0].saved, "value 99");
}

#[test]
fn imported_constructions_agree_with_the_golden_snapshots() {
    let root = repo_root();
    let cases: HashMap<String, Case> = model::load_all(&root)
        .unwrap()
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();
    let gaps = gaps::load().unwrap();
    let gap_file: ImportGapFile =
        toml::from_str(include_str!("../known-import-gaps.toml")).unwrap();
    let mut reloads: BTreeMap<String, BTreeMap<String, f64>> =
        serde_json::from_str(include_str!("../../../golden/ggb/distance-reload.snapshot")).unwrap();
    let reload_count: usize = reloads.values().map(BTreeMap::len).sum();
    assert_eq!(reload_count, 5);
    let reclassified: BTreeMap<String, &Reclassified> = gap_file
        .reclassified
        .iter()
        .map(|r| (format!("{}:{}", r.id, r.label), r))
        .collect();
    assert_eq!(
        reclassified.len(),
        gap_file.reclassified.len(),
        "a reclassified object is listed twice"
    );
    let mut xml = Vec::new();
    files(&root.join("golden/ggb"), "xml", &mut xml);
    // `saved/` holds files saved after moves, with their own snapshots
    // (tests/saved.rs); they have no golden case.
    xml.retain(|p| {
        p.parent()
            .and_then(|d| d.file_name())
            .is_none_or(|d| d != "saved")
    });
    xml.sort();
    let mut totals = Totals::default();
    let mut failures = Vec::new();
    for path in &xml {
        let id = path.file_stem().unwrap().to_str().unwrap();
        let case = cases
            .get(id)
            .unwrap_or_else(|| panic!("no golden case for {}", path.display()));
        let reload = reloads.remove(id);
        if let Some(reload) = &reload {
            assert!(reload
                .keys()
                .all(|label| case.snapshots[0].contains_key(label)));
        }
        let script = compare_case(
            case,
            &fs::read(path).unwrap(),
            &gaps,
            reload.as_ref(),
            &mut totals,
            &mut failures,
        );
        totals.files += 1;
        // The zipped file of the same case must give the same script.
        let zip = path.with_extension("ggb");
        if zip.exists() {
            totals.zips += 1;
            let zipped = import(&fs::read(&zip).unwrap()).unwrap();
            assert_eq!(zipped.script, script, "{}: zip and xml differ", case.id);
        }
    }
    assert!(reloads.is_empty(), "unmatched reference reload recordings");
    println!(
        "files imported: {} (of which with a zipped twin: {})\nobjects compared: {}\n\
         objects in known gaps (disagree as listed): {}\n\
         objects compared against reference reload: {}\nobjects skipped: {}\n\
         hidden objects reported: {}\nskipped by group: {:?}",
        totals.files,
        totals.zips,
        totals.objects_compared,
        totals.gap_objects,
        reload_count,
        totals.objects_skipped,
        totals.hidden,
        totals.skipped_by_group
    );
    println!(
        "outputs without a label in the file: {} (snapshot objects undefined as required: {})",
        totals.unlabelled, totals.unlabelled_objects
    );
    for (reason, n) in &totals.skipped_by_reason {
        println!(
            "  skipped {n:5}  {reason}\n    e.g. {}",
            totals.example[reason]
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(totals.objects_skipped, MAX_SKIPPED);
    assert!(totals.skipped_by_group.is_empty());
    assert!(
        totals.unlabelled <= MAX_UNLABELLED,
        "{} unlabelled",
        totals.unlabelled
    );
    assert_eq!(totals.unlabelled, totals.unlabelled_objects);
    assert!(
        totals.skipped_labels.is_empty(),
        "{:?}",
        totals.skipped_labels
    );
    // Every disagreement with the saved value is documented: a kernel gap at
    // step 0 (whose golden value is the saved one), or a
    // pinned saved matrix that classifies afresh as another object (the imported
    // value agreed with the golden one above, or the test already failed).
    let mut undocumented = Vec::new();
    for key in totals.mismatched.keys() {
        let (id, label) = key.split_once(':').unwrap();
        let in_kernel_gap = gaps
            .get(id)
            .is_some_and(|g| g.mismatches.contains(&format!("0:{label}")));
        let reclassified_as_pinned = reclassified
            .get(key)
            .is_some_and(|r| r.saved == totals.mismatched[key].0 && !r.reason.is_empty());
        if !(in_kernel_gap || reclassified_as_pinned) {
            undocumented.push(format!("{key}: {:?}", totals.mismatched[key]));
        }
    }
    assert!(
        undocumented.is_empty(),
        "{} objects differ from the value saved in their file and are not documented:\n{}",
        undocumented.len(),
        undocumented.join("\n")
    );
    for key in reclassified.keys() {
        assert!(
            totals.mismatched.contains_key(key),
            "{key} no longer mismatches"
        );
    }
    println!(
        "objects that differ from the saved value: {}",
        totals.mismatched.len()
    );
    assert!(totals.files >= MIN_FILES, "only {} files", totals.files);
    assert!(totals.zips >= MIN_ZIPS, "only {} zip files", totals.zips);
    assert!(
        totals.objects_compared >= MIN_COMPARED,
        "only {} objects compared",
        totals.objects_compared
    );
}
