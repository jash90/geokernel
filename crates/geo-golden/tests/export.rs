//! The same live Rust constructions go to the importer and to the headless
//! reference reader. The Java test builds no goldens and checks every label.

use geo_app_core::workbench::{FileView, Workbench};
use geo_golden::compare::{compare_construction_object, numbers_agree};
use geo_golden::model::{expected_from_json, load_all, Step};
use geo_golden::repo_root;
use geo_kernel::kernel::image::{ImageCorner, ImagePlacement};
use geo_kernel::kernel::text::TextPosition;
use geo_kernel::{Construction, Def, Id, Kind};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

// Existing Java goldens, with an explicit step where the reloaded state matters.
const GOLDEN_CASES: &[(&str, usize)] = &[
    ("algebralogic-logic-019", 0),
    ("algebralogic-logic-024", 0),
    ("algebralogic-sides-001", 0),
    ("angle-conic-001", 0),
    ("angle-lines-012", 0),
    ("angle-numeric-001", 0),
    ("arcs-points-001", 1),
    ("charts-barchart-001", 0),
    ("charts-piechart-001", 0),
    ("expr-apply-001", 1),
    ("funcanalysis-curvatureupstream-002", 0),
    ("lists-objects-001", 0),
    ("locus-slider-001", 0),
    ("paths-chain-001", 1),
    ("polygon-polyline-001", 0),
    ("polygon-regular-001", 0),
    ("textobjects-dynamic-001", 1),
    ("vector-chain-001", 0),
];

const SAVED_CASES: &[&str] = &[
    "saved-controls-002.xml",
    "saved-images-001.ggb",
    "saved-images-002.ggb",
    "saved-object-scripts-001.xml",
    "saved-slider-bounds-001.xml",
    "saved-texts-001.ggb",
    "saved-text-equations-001.xml",
];

fn labelled(c: &Construction) -> BTreeMap<&str, Id> {
    c.ids()
        .filter_map(|id| {
            let label = c.label(id);
            (!label.starts_with('\0')).then_some((label, id))
        })
        .collect()
}

fn corpus() -> Vec<(String, Workbench, Option<FileView>)> {
    let mut cases: BTreeMap<_, _> = load_all(&repo_root())
        .unwrap()
        .into_iter()
        .map(|case| (case.id.clone(), case))
        .collect();
    let mut out = Vec::new();
    for &(name, step) in GOLDEN_CASES {
        let case = cases
            .remove(name)
            .unwrap_or_else(|| panic!("missing {name}"));
        let mut wb = Workbench::from_script(&case.script.join("\n"))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        for action in &case.steps[..=step] {
            match action {
                Step::Initial => {}
                Step::Move { label, x, y } => {
                    let id = wb.construction().id(label).unwrap();
                    wb.move_point(id, *x, *y);
                }
                Step::SetNumber { label, value } => {
                    let id = wb.construction().id(label).unwrap();
                    wb.set_number(id, *value);
                }
                Step::Reject { .. } => panic!("{name}: unexpected rejection step"),
            }
        }
        let c = wb.construction();
        for (label, expected) in &case.snapshots[step] {
            compare_construction_object(expected, c, c.id(label).unwrap())
                .unwrap_or_else(|why| panic!("{name} #{step} {label}: {why}"));
        }
        let view = case.view.map(|[xmin, xmax, ymin, ymax, sx, sy]| FileView {
            size: [
                ((xmax - xmin) * sx).round() as u32,
                ((ymax - ymin) * sy).round() as u32,
            ],
            origin: [-xmin * sx, ymax * sy],
            scale: [sx, sy],
            ..FileView::default()
        });
        out.push((name.into(), wb, view));
    }
    for &file in SAVED_CASES {
        let mut wb = Workbench::default();
        let bytes = fs::read(repo_root().join("golden/ggb/saved").join(file)).unwrap();
        wb.load_for_export(&bytes, file).unwrap();
        let name = file.split_once('.').unwrap().0;
        out.push((name.into(), wb, None));
    }
    out
}

fn round_trip(name: &str, wb: &Workbench, view: Option<FileView>) -> (Vec<u8>, Workbench) {
    let bytes = wb
        .export_ggb(view)
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut again = Workbench::default();
    again.load_for_export(&bytes, name).unwrap();
    let original = wb.construction();
    let restored = again.construction();
    for (label, id) in labelled(original) {
        let new_id = restored
            .id(label)
            .unwrap_or_else(|| panic!("{name}: missing {label}"));
        assert_eq!(original.kind(id), restored.kind(new_id), "{name}: {label}");
        assert_eq!(
            original.drawing_style(id),
            restored.drawing_style(new_id),
            "{name}: {label}"
        );
        assert_eq!(
            original.equation_form(id),
            restored.equation_form(new_id),
            "{name}: {label}"
        );
        assert_eq!(
            original.angle_style(id),
            restored.angle_style(new_id),
            "{name}: {label}"
        );
        assert_eq!(
            original.slider_bounds(id),
            restored.slider_bounds(new_id),
            "{name}: {label}"
        );
        assert_eq!(
            original.object_scripts(label),
            restored.object_scripts(label),
            "{name}: {label}"
        );
        assert_eq!(
            original.control(id),
            restored.control(new_id),
            "{name}: {label}"
        );
        assert_eq!(original.text(id), restored.text(new_id), "{name}: {label}");
        if let Some(mut text) = original.text_presentation(id) {
            let resolve = |point| restored.id(original.label(point)).unwrap();
            text.position = match text.position {
                TextPosition::Point(point) => TextPosition::Point(resolve(point)),
                TextPosition::ScreenPoint(point) => TextPosition::ScreenPoint(resolve(point)),
                other => other,
            };
            assert_eq!(
                Some(text),
                restored.text_presentation(new_id),
                "{name}: {label}"
            );
        }
        assert_eq!(
            wb.is_visible(id),
            again.is_visible(new_id),
            "{name}: {label}"
        );
        assert_eq!(
            wb.label_offset(id),
            again.label_offset(new_id),
            "{name}: {label}"
        );
        if let Some(image) = original.image(id) {
            let reloaded = restored.image(new_id).unwrap();
            assert_eq!(
                image.asset.bytes(),
                reloaded.asset.bytes(),
                "{name}: {label}"
            );
        }
        match original.kind(id) {
            Kind::Image | Kind::Button | Kind::InputBox => {
                assert_eq!(
                    special(original, id),
                    special(restored, new_id),
                    "{name}: {label}"
                );
            }
            _ => {}
        }
    }
    assert_eq!(labelled(original).len(), labelled(restored).len(), "{name}");
    assert_eq!(
        again.file_view(),
        Some(view.or(wb.file_view()).unwrap_or_default()),
        "{name}"
    );
    (bytes, again)
}

#[test]
fn exported_constructions_import_with_all_labelled_kinds_and_properties() {
    for (name, wb, view) in corpus() {
        round_trip(&name, &wb, view);
    }
}

#[test]
fn archive_preserves_view_kernel_settings_and_application_edits() {
    let source = br#"<geogebra format="5.0">
      <euclidianView>
        <size width="720" height="640"/>
        <coordSystem xZero="377.5" yZero="296" scale="70" yscale="43.25"/>
        <evSettings axes="false" grid="true"/>
        <axis id="0" show="true"/>
      </euclidianView>
      <kernel><significantfigures val="6"/><angleUnit val="radiant"/></kernel>
      <construction>
        <element type="point" label="A">
          <coords x="2" y="3" z="1"/>
          <layer val="7"/>
          <labelOffset x="-5" y="6"/>
          <show object="false" label="false"/>
        </element>
      </construction>
    </geogebra>"#;
    let mut wb = Workbench::default();
    wb.load_for_export(source, "settings").unwrap();
    let id = wb.construction().id("A").unwrap();
    assert!(wb.move_point(id, -1.5, 2.25));
    let (exported, again) = round_trip("settings", &wb, None);
    let imported = geo_import::import(&exported).unwrap();
    assert_eq!(
        imported.settings,
        geo_import::import(source).unwrap().settings
    );
    let view = again.file_view().unwrap();
    assert_eq!(view.axes, [true, false]);
    assert!(view.grid);
    assert_eq!(view.scale, [70.0, 43.25]);
    assert_eq!(view.origin, [377.5, 296.0]);
    assert_eq!(
        again
            .construction()
            .value(again.construction().id("A").unwrap()),
        wb.construction().value(id)
    );
}

/// The CI oracle job supplies the pinned source checkout and built harness.
/// Run locally with the same environment and `cargo test --test export -- --ignored`.
#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn geogebra_reloads_exports_and_matches_the_kernel() {
    let dir = Scratch::new();
    let constructions: Vec<_> = corpus()
        .into_iter()
        .map(|(name, wb, view)| {
            let (bytes, again) = round_trip(&name, &wb, view);
            fs::write(dir.0.join(format!("{name}.ggb")), bytes).unwrap();
            (name, wb, again)
        })
        .collect();
    let output = Command::new(repo_root().join("tools/ggb-harness/harness.sh"))
        .arg("export-snapshot")
        .arg(&dir.0)
        .output()
        .expect("run Java harness");
    assert!(
        output.status.success() && output.stderr.is_empty(),
        "Java export oracle failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let mut covered = BTreeSet::new();
    let mut count = 0;
    for (name, live, imported) in &constructions {
        let path = dir.0.join(format!("{name}.ggb.snapshot"));
        let snapshot: BTreeMap<String, Value> =
            serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let c = live.construction();
        let labels = labelled(c);
        assert!(
            labels
                .keys()
                .copied()
                .eq(snapshot.keys().map(String::as_str)),
            "{name}: labels differ"
        );
        for (label, id) in labels {
            let kind = c.kind(id);
            let snap = &snapshot[label];
            let reconstructed = imported.construction();
            let restored = reconstructed.id(label).unwrap();
            if matches!(kind, Kind::Image | Kind::Button | Kind::InputBox) {
                assert_json(snap, &special(c, id), &format!("{name}: {label}"));
            } else {
                let expected = expected_from_json(snap).unwrap();
                for (kernel, id) in [(c, id), (reconstructed, restored)] {
                    compare_construction_object(&expected, kernel, id)
                        .unwrap_or_else(|why| panic!("{name}: {label}: {why}"));
                }
            }
            count += 1;
            if c.value(id).is_defined() {
                covered.insert(format!("{kind:?}"));
            }
        }
    }
    let table = fs::read_to_string(repo_root().join("docs/kinds.csv")).unwrap();
    let matrix = geo_golden::table::Table::parse(&table, "docs/kinds.csv").unwrap();
    let exported: BTreeSet<_> = matrix
        .records()
        .into_iter()
        .filter_map(|row| (row["export"] != "no").then(|| row["kernel_kind"].to_owned()))
        .collect();
    assert_eq!(
        covered, exported,
        "every claimed export kind needs a defined example"
    );
    println!(
        "{} exported files, {count} objects, {} kinds",
        constructions.len(),
        covered.len()
    );
}

fn special(c: &Construction, id: Id) -> Value {
    let defined = c.value(id).is_defined();
    if let Some(image) = c.image(id) {
        let mut result = json!({
            "type": "image", "defined": defined, "size": image.asset.size(),
            "background": image.style.background, "alpha": image.style.alpha,
            "interpolate": image.style.interpolate, "centered": false,
        });
        if let Some(crop) = image.style.crop {
            result["crop"] = json!(crop);
        }
        match image.placement {
            ImagePlacement::Screen(screen) => result["screen"] = json!(screen),
            ImagePlacement::World {
                corners,
                centered,
                screen_coords,
            } => {
                result["centered"] = json!(centered);
                result["screen_coords"] = json!(screen_coords);
                result["anchors"] = json!(corners.map(|corner| match corner {
                    Some(ImageCorner::Point(point)) => json!(c.label(point)),
                    Some(ImageCorner::Coordinates(xy)) => json!(xy),
                    None => Value::Null,
                }));
            }
        }
        result
    } else {
        let caption = c.control(id).map_or("", |control| control.caption.as_str());
        if let Def::InputBox(link) = c.def(id) {
            json!({ "type": "textfield", "defined": defined, "caption": caption, "link": link.map_or("", |id| c.label(id)) })
        } else {
            json!({ "type": "button", "defined": defined, "caption": caption })
        }
    }
}

fn assert_json(expected: &Value, actual: &Value, where_: &str) {
    match (expected, actual) {
        (Value::Number(a), Value::Number(b)) => assert!(
            numbers_agree(a.as_f64().unwrap(), b.as_f64().unwrap()),
            "{where_}: {a} vs {b}"
        ),
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (a, b) in a.iter().zip(b) {
                assert_json(a, b, where_);
            }
        }
        (Value::Object(a), Value::Object(b)) if a.keys().eq(b.keys()) => {
            for (key, value) in a {
                assert_json(value, &b[key], &format!("{where_}.{key}"));
            }
        }
        _ => assert_eq!(expected, actual, "{where_}"),
    }
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("geokernel-ggb-oracle-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
