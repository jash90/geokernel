//! The same live Rust constructions go to the importer and to the headless
//! reference reader. The Java test builds no goldens and checks every label.

use geo_app_core::workbench::{FileView, Workbench};
use geo_golden::compare::{compare_construction_object, numbers_agree};
use geo_golden::model::{expected_from_json, load_all, Step};
use geo_golden::repo_root;
use geo_kernel::algo::path::PathDef;
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
    ("export-geometry-001", 0),
    ("export-transforms-001", 0),
    ("export-conics-001", 0),
    ("export-paths-001", 3),
    ("export-curve-001", 0),
    // Free numbers outside the interval the reference gives a new slider.
    ("export-numbers-001", 0),
    ("export-sliders-001", 0),
    // Sliders set outside the interval the reference gives a shown number.
    ("export-sliders-002", 2),
    ("export-texts-001", 0),
    ("funcanalysis-curvatureupstream-002", 0),
    ("georestintersect-affine-002", 0),
    ("georestintersect-growth-001", 2),
    ("georestintersect-pathcurve-001", 0),
    ("georestsmall-dilatecurve-001", 2),
    ("georestsmall-matrixcurve-001", 0),
    ("implicit-equations-001", 0),
    ("implicit-equations-003", 2),
    ("lists-objects-001", 0),
    ("locus-slider-001", 0),
    ("paths-chain-001", 1),
    ("polygon-polyline-001", 0),
    ("polygon-regular-001", 0),
    // Pen strokes: cubic pieces, breaks and a lone point.
    ("stroke-breaks-001", 0),
    ("stroke-literals-002", 0),
    ("stroke-points-010", 0),
    ("stroke-points-004", 0),
    ("redefine-assignment-001", 2),
    ("redefine-assignment-015", 3),
    ("textobjects-dynamic-001", 1),
    ("vector-chain-001", 0),
];

const SAVED_CASES: &[&str] = &[
    "saved-controls-002.xml",
    "saved-curve-branch-001.xml",
    "saved-curve-intersections-001.xml",
    "saved-curve-intersections-002.xml",
    "saved-paths-002.xml",
    "saved-implicit-001.xml",
    "saved-images-001.ggb",
    "saved-images-002.ggb",
    "saved-object-scripts-001.xml",
    "saved-slider-bounds-001.xml",
    "saved-strokes-001.xml",
    "saved-texts-001.ggb",
    "saved-text-equations-001.xml",
    // Presentation properties per kind, set by the reference application.
    "saved-styles-001.xml",
    "saved-styles-002.xml",
    "saved-styles-003.xml",
    "saved-styles-004.xml",
    "saved-styles-005.xml",
    "saved-styles-006.xml",
    "saved-styles-007.xml",
    "saved-styles-008.ggb",
    // Styled objects that are undefined in the saved state.
    "saved-styles-009.xml",
    "saved-styles-010.xml",
    "saved-styles-011.xml",
    // Dynamic colour channels, a step and a speed over objects that are
    // undefined in the saved state.
    "saved-styles-012.xml",
    // Pen strokes with presentation properties.
    "saved-styles-013.xml",
];

const STYLED_FILE: &[u8] = br#"<geogebra format="5.0">
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
    <element type="point" label="B"><coords x="4" y="1" z="1"/></element>
    <command name="Line"><input a0="A" a1="B"/><output a0="g"/></command>
    <element type="line" label="g">
      <objColor r="123" g="45" b="67" alpha="0"/>
      <lineStyle thickness="7" type="10"/>
      <labelMode val="3"/>
      <caption val="A caption"/>
      <labelOffset x="9" y="-12"/>
    </element>
    <element type="numeric" label="a">
      <value val="2"/>
      <slider min="0" max="5" x="20" y="40" width="100" horizontal="true" absoluteScreenLocation="true"/>
      <animation step="0.125" speed="2.5" type="2" playing="false"/>
    </element>
  </construction>
</geogebra>"#;

const IMPLICIT_MATRIX_FILE: &[u8] = br#"<geogebra format="5.0">
  <construction>
    <element type="implicitpoly" label="c">
      <userinput show="false"/>
      <coefficients rep="array" data="[[-1,0,0,1],[0,0,0,0],[0,0,0,0],[1,0,0,0]]"/>
    </element>
  </construction>
</geogebra>"#;

fn labelled(c: &Construction) -> BTreeMap<&str, Id> {
    c.ids()
        .filter_map(|id| {
            let label = c.label(id);
            (!label.starts_with('\0')).then_some((label, id))
        })
        .collect()
}

/// Where a document comes from.
enum Origin {
    /// A script of the golden corpus, replayed.
    Script,
    /// A file the reference application reads without reporting anything.
    File(Vec<u8>),
    /// A hand-written file with tags the reference application does not
    /// write. It may make the reader report a problem and read on, or give
    /// up the whole file; `applied` is then the same file with the tag it
    /// gives up at cut down to what it had applied by then.
    Hand {
        source: Vec<u8>,
        applied: Option<Vec<u8>>,
    },
}

/// A document to export: its name, the live workbench, the view of a golden
/// case, and where it comes from.
type Case = (String, Workbench, Option<FileView>, Origin);

/// Hand-written files: `<name>.xml`, and `<name>.applied.xml` beside it.
fn hand_written() -> Vec<(String, Vec<u8>, Option<Vec<u8>>)> {
    let dir = repo_root().join("crates/geo-golden/tests/fixtures/export");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".xml") && !name.ends_with(".applied.xml"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|file| {
            let stem = file.strip_suffix(".xml").unwrap();
            let source = fs::read(dir.join(&file)).unwrap();
            let applied = fs::read(dir.join(format!("{stem}.applied.xml"))).ok();
            (format!("hand-{stem}"), source, applied)
        })
        .collect()
}

fn corpus() -> Vec<Case> {
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
                Step::Add { line } | Step::Redefine { line } => wb.run_line(line).unwrap(),
                Step::Reject { .. }
                | Step::Delete { .. }
                | Step::SetSliderBounds { .. }
                | Step::Append { .. }
                | Step::AnchorText { .. }
                | Step::Undo
                | Step::Redo
                | Step::Reload => {
                    panic!("{name}: unexpected step")
                }
                Step::Animate { frames } => {
                    for _ in 0..*frames {
                        wb.animate(geo_kernel::kernel::animation::MAX_ANIMATION_FRAME_RATE);
                    }
                }
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
        out.push((name.into(), wb, view, Origin::Script));
    }
    for &file in SAVED_CASES {
        let mut wb = Workbench::default();
        let bytes = fs::read(repo_root().join("golden/ggb/saved").join(file)).unwrap();
        wb.load_for_export(&bytes, file).unwrap();
        let name = file.split_once('.').unwrap().0;
        out.push((name.into(), wb, None, Origin::File(bytes)));
    }
    for (name, source, applied) in hand_written() {
        let mut wb = Workbench::default();
        wb.load_for_export(&source, &name).unwrap();
        out.push((name, wb, None, Origin::Hand { source, applied }));
    }
    let mut styled = Workbench::default();
    styled.load_for_export(STYLED_FILE, "styled").unwrap();
    out.push((
        "styled".into(),
        styled,
        None,
        Origin::File(STYLED_FILE.to_vec()),
    ));
    let mut matrix = Workbench::default();
    matrix
        .load_for_export(IMPLICIT_MATRIX_FILE, "implicit-matrix")
        .unwrap();
    assert_eq!(
        matrix
            .construction()
            .implicit_degree(matrix.construction().id("c").unwrap()),
        Some(3)
    );
    out.push((
        "implicit-matrix".into(),
        matrix,
        None,
        Origin::File(IMPLICIT_MATRIX_FILE.to_vec()),
    ));
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
    // What the document holds from its file is read back from its export.
    let (before, after) = (
        wb.export_presentation(view),
        again.export_presentation(view),
    );
    if !before.object_styles.is_empty() {
        for (label, style) in &before.object_styles {
            // By their text: a width read as NaN equals itself here.
            assert_eq!(
                format!("{:?}", Some(style)),
                format!("{:?}", after.object_styles.get(label)),
                "{name}: {label}"
            );
        }
        assert_eq!(
            before.object_styles.len(),
            after.object_styles.len(),
            "{name}"
        );
        assert_eq!(before.slider_animation, after.slider_animation, "{name}");
        assert_eq!(before.conditions, after.conditions, "{name}");
        assert_eq!(before.label_offsets, after.label_offsets, "{name}");
    }
    assert_eq!(
        again.file_view(),
        Some(view.or(wb.file_view()).unwrap_or_default()),
        "{name}"
    );
    (bytes, again)
}

#[test]
fn exported_constructions_import_with_all_labelled_kinds_and_properties() {
    for (name, wb, view, _) in corpus() {
        round_trip(&name, &wb, view);
    }
}

#[test]
fn archive_preserves_view_kernel_settings_and_application_edits() {
    let mut wb = Workbench::default();
    wb.load_for_export(STYLED_FILE, "settings").unwrap();
    let id = wb.construction().id("A").unwrap();
    assert!(!wb.snapshot_text().contains("$drawingStyle(g,"));
    assert!(wb.move_point(id, -1.5, 2.25));
    let (exported, again) = round_trip("settings", &wb, None);
    let imported = geo_import::import(&exported).unwrap();
    assert_eq!(
        imported.settings,
        geo_import::import(STYLED_FILE).unwrap().settings
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
///
/// It also derives the proof of every presentation property and compares it
/// with `tests/fixtures/export-proof.json`, which the always-run test of the
/// kind table reads. `GEOKERNEL_EXPORT_BLESS=1` rewrites that file.
#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn geogebra_reloads_exports_and_matches_the_kernel() {
    // Exports and files the reference application must read without a report.
    let exports = Scratch::new("exports");
    let sources = Scratch::new("sources");
    // The documents of the files without their saved presentation, as a
    // construction made in the application exports.
    let plain = Scratch::new("plain");
    // Hand-written files and their exports, read with the reports recorded.
    let hand = Scratch::new("hand");
    let zipped = |bytes: &[u8]| {
        if geo_import::archive::is_zip(bytes) {
            bytes.to_vec()
        } else {
            archive(bytes, None)
        }
    };
    let constructions: Vec<_> = corpus()
        .into_iter()
        .map(|(name, wb, view, origin)| {
            let (bytes, again) = round_trip(&name, &wb, view);
            let write = |scratch: &Scratch, file: &str, bytes: &[u8]| {
                fs::write(scratch.0.join(format!("{file}.ggb")), bytes).unwrap();
            };
            match &origin {
                Origin::Script => write(&exports, &name, &bytes),
                Origin::File(source) => {
                    write(&exports, &name, &bytes);
                    write(&sources, &name, &zipped(source));
                    let mut stripped = Workbench::default();
                    let rejected = stripped.restore_text(&wb.snapshot_text()).unwrap();
                    assert!(rejected.is_empty(), "{name}: {rejected:?}");
                    let bytes = stripped
                        .export_ggb(view.or(wb.file_view()))
                        .unwrap_or_else(|e| panic!("{name}: {e}"));
                    write(&plain, &name, &bytes);
                }
                Origin::Hand { source, applied } => {
                    write(&hand, &name, &zipped(source));
                    write(&hand, &format!("{name}.export"), &bytes);
                    if let Some(applied) = applied {
                        write(&sources, &name, &zipped(applied));
                    }
                }
            }
            (name, wb, again, view, origin, bytes)
        })
        .collect();
    for (scratch, lenient) in [
        (&exports, false),
        (&sources, false),
        (&plain, false),
        (&hand, true),
    ] {
        let output = if lenient {
            lenient_java_snapshots(&scratch.0)
        } else {
            java_snapshots(&scratch.0)
        };
        assert!(
            output.status.success(),
            "Java export oracle failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let read = |scratch: &Scratch, file: &str| -> Value {
        let path = scratch.0.join(format!("{file}.ggb.snapshot"));
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    };

    let mut count = 0;
    let mut gave_up = Vec::new();
    // Per fixture: the styles of the source and of the export without the
    // saved presentation, for the witnesses.
    let mut saved_styles: BTreeMap<&str, (Value, Value)> = BTreeMap::new();
    for (name, live, imported, view, origin, bytes) in &constructions {
        let snap = match origin {
            Origin::Hand { .. } => read(&hand, &format!("{name}.export")),
            _ => read(&exports, name),
        };
        let snapshot = snap["objects"].as_object().unwrap();
        check_styles(name, live, *view, &snap);
        match origin {
            Origin::Script => {}
            Origin::File(_) => {
                // The reference application reads the source file and the
                // export of its import to the same presentation, property
                // by property, for every object.
                let source = read(&sources, name);
                compare_presentation(name, live, &source, &snap, bytes);
                assert_json(&source["settings"], &snap["settings"], name);
                let stripped = read(&plain, name);
                // A witness needs the same object in the stripped export.
                assert!(
                    labels_of(&source).eq(labels_of(&stripped)),
                    "{name}: the export without saved styles has other labels"
                );
                if name == "saved-styles-012" {
                    undefined_inputs_as_the_reference_reads_them(&source["styles"]);
                }
                saved_styles.insert(name, (source["styles"].clone(), stripped["styles"].clone()));
            }
            Origin::Hand { applied, .. } => {
                let source = read(&hand, name);
                if source.get("failed").is_some() {
                    // The reference gives up the file at the tag. The export
                    // loads without a report and holds what the tag had
                    // applied by then, which the twin spells out.
                    assert!(
                        applied.is_some(),
                        "{name}: the reference gives up the file ({}); add the twin",
                        source["failed"]
                    );
                    assert_eq!(snap["reported"], json!([]), "{name}");
                    compare_presentation(name, live, &read(&sources, name), &snap, bytes);
                    gave_up.push(name.as_str());
                } else {
                    assert!(applied.is_none(), "{name}: the reference reads the file");
                    // A stroke is what the reference reads from the file:
                    // older vertices smoothed in the file's view, Bezier
                    // points as written.
                    let c = live.construction();
                    for (label, id) in labelled(c) {
                        if c.kind(id) == Kind::PenStroke {
                            let expected = expected_from_json(&source["objects"][label]).unwrap();
                            compare_construction_object(&expected, c, id)
                                .unwrap_or_else(|why| panic!("{name}: source {label}: {why}"));
                        }
                    }
                    // The export makes the reader report nothing that the
                    // source did not: a tag that set nothing is not written,
                    // a tag read halfway is written as far as it was read.
                    let mut reports = source["reported"].as_array().unwrap().clone();
                    for report in snap["reported"].as_array().unwrap() {
                        let at = reports
                            .iter()
                            .position(|known| known == report)
                            .unwrap_or_else(|| panic!("{name}: the export reports {report}"));
                        reports.remove(at);
                    }
                    compare_presentation(name, live, &source, &snap, bytes);
                }
            }
        }
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
        }
    }
    // The files of `HAND_GIVEN_UP`, and no other, stop the reference reader.
    assert_eq!(gave_up, HAND_GIVEN_UP);

    // The proof: for every kind and property an object of a named fixture
    // whose source value differs from what the reader gives the same object
    // without the saved tag.
    let mut proof: BTreeMap<&str, BTreeMap<&str, Value>> = BTreeMap::new();
    for &(kind, objects) in WITNESSES {
        for property in kind_properties(kind) {
            let witness = objects.iter().find_map(|&(fixture, label)| {
                let (source, stripped) = saved_styles
                    .get(fixture)
                    .unwrap_or_else(|| panic!("{fixture} is no file of the corpus"));
                let live = &constructions
                    .iter()
                    .find(|case| case.0 == fixture)
                    .unwrap()
                    .1;
                let c = live.construction();
                let id = c
                    .id(label)
                    .unwrap_or_else(|| panic!("{fixture} has no {label}"));
                assert_eq!(format!("{:?}", c.kind(id)), kind, "{fixture}: {label}");
                let (mut saved, mut unsaved) = (BTreeMap::new(), BTreeMap::new());
                flatten("", &source[label], &mut saved);
                flatten("", &stripped[label], &mut unsaved);
                let value = saved.get(property)?;
                // What the reader gives the object without its saved tag:
                // the stripped export, or, for a property the document keeps
                // in its own state, the value of a new object.
                let without =
                    new_object(kind, property).or_else(|| unsaved.get(property).cloned())?;
                (without != *value).then(|| {
                    json!({
                        "fixture": fixture,
                        "label": label,
                        "saved": value,
                        "without": without,
                    })
                })
            });
            if let Some(witness) = witness {
                proof.entry(kind).or_default().insert(property, witness);
            }
        }
    }
    let proof = serde_json::to_string_pretty(&proof).unwrap() + "\n";
    let path = proof_path();
    if std::env::var_os("GEOKERNEL_EXPORT_BLESS").is_some() {
        fs::write(&path, &proof).unwrap();
    }
    assert!(
        fs::read_to_string(&path).unwrap_or_default() == proof,
        "tests/fixtures/export-proof.json is stale; rerun with GEOKERNEL_EXPORT_BLESS=1"
    );
    let pairs = serde_json::from_str::<BTreeMap<String, BTreeMap<String, Value>>>(&proof)
        .unwrap()
        .values()
        .map(BTreeMap::len)
        .sum::<usize>();
    println!(
        "{} exported files, {count} objects, {} kinds, {pairs} proven kind and property pairs, \
         {} files the reference gives up",
        constructions.len(),
        WITNESSES.len(),
        gave_up.len()
    );
}

fn proof_path() -> PathBuf {
    repo_root().join("crates/geo-golden/tests/fixtures/export-proof.json")
}

fn labels_of(snapshot: &Value) -> impl Iterator<Item = &String> {
    snapshot["styles"].as_object().unwrap().keys()
}

/// Hand-written files at which the reference reader gives up the whole file.
const HAND_GIVEN_UP: &[&str] = &[
    "hand-fill-alpha-not-a-number",
    "hand-fill-angle-not-a-number",
    "hand-fill-distance-missing",
    "hand-fill-pattern-negative",
];

/// Properties that differ between a source and its export on purpose:
/// fixture, label, property and the reason.
const KNOWN_LOSSES: &[(&str, &str, &str, &str)] = &[(
    "hand-animation-speed",
    "A",
    "animation.speed",
    "the kernel has no limit command, so the speed is dropped with an import note",
)];

/// Every property of every object of `before` (the reference application's
/// reading of a source) against `after` (its reading of the export).
fn compare_presentation(name: &str, wb: &Workbench, before: &Value, after: &Value, export: &[u8]) {
    assert!(
        labels_of(before).eq(labels_of(after)),
        "{name}: labels differ from the source"
    );
    let c = wb.construction();
    let xml = geo_import::archive::extract_xml(export).unwrap();
    // Whether the export defines the object by an expression or a command.
    let defined = |label: &str| {
        xml.contains(&format!("<expression label=\"{label}\""))
            || xml
                .lines()
                .any(|line| line.starts_with("<output") && line.contains(&format!("=\"{label}\"")))
    };
    for (label, style) in before["styles"].as_object().unwrap() {
        let (mut source, mut exported) = (BTreeMap::new(), BTreeMap::new());
        flatten("", style, &mut source);
        flatten("", &after["styles"][label], &mut exported);
        assert!(
            source.keys().eq(exported.keys()),
            "{name}: {label}: other properties"
        );
        let [min, max] = c.slider_bounds(c.id(label).unwrap());
        for (property, value) in &source {
            // A file that gives a visible slider one endpoint only: the
            // reference fills in the other when it reads its own file. The
            // export keeps the interval of the saved state, which
            // `check_styles` compares with the kernel.
            let open_end = match property.as_str() {
                "slider.min" => min.is_none() && max.is_some(),
                "slider.max" => max.is_none() && min.is_some(),
                _ => false,
            };
            let known = KNOWN_LOSSES
                .iter()
                .any(|&(file, object, lost, _)| (file, object, lost) == (name, label, property));
            if known {
                assert_ne!(value, &exported[property], "{name}: {label}.{property}");
                continue;
            }
            if open_end && *value != exported[property] {
                assert_eq!(
                    exported[property],
                    Value::Null,
                    "{name}: {label}.{property}"
                );
                continue;
            }
            // The reference shows the algebra label again when it names an
            // object that only its element tag creates. The writer saves a
            // free object that way, whatever its file did.
            if property == "algebra_label" && !defined(label) && *value != exported[property] {
                assert_eq!(exported[property], json!(true), "{name}: {label}");
                continue;
            }
            assert_json(
                value,
                &exported[property],
                &format!("{name}: {label}.{property}"),
            );
        }
    }
}

/// What the reference reader makes of style expressions whose inputs are
/// undefined in the saved file (`saved-styles-012`): it keeps the channels
/// of the dynamic colour as written, an undefined channel gives 0, and an
/// undefined step and speed stay with the slider, the speed reported as 0.
/// The comparison with the export shows that the export keeps all of them.
fn undefined_inputs_as_the_reference_reads_them(styles: &Value) {
    let point = &styles["P"];
    assert_eq!(
        point["dynamic_color"],
        json!(["x(Q) / 10", "sqrt(a)", "b"]),
        "{point}"
    );
    assert_eq!(point["color"], json!([0, 0, 191]), "{point}");
    let animation = &styles["n"]["animation"];
    assert_eq!(animation["step"], Value::Null, "{animation}");
    assert_eq!(animation["speed"], json!(0.0), "{animation}");
}

/// The objects whose saved presentation proves the properties of a kind:
/// fixture and label. Only these count, so that a property is never proven by
/// an object that happens to differ for another reason.
const WITNESSES: &[(&str, &[(&str, &str)])] = &[
    (
        "Point",
        &[
            ("saved-styles-001", "A"),
            ("saved-styles-001", "P"),
            ("saved-styles-001", "B"),
            ("saved-paths-002", "S"),
        ],
    ),
    (
        "Line",
        &[("saved-styles-001", "l"), ("saved-styles-001", "l2")],
    ),
    (
        "Segment",
        &[("saved-styles-001", "s"), ("saved-styles-001", "s2")],
    ),
    (
        "Ray",
        &[("saved-styles-001", "h"), ("saved-styles-001", "h2")],
    ),
    (
        "Vector",
        &[("saved-styles-001", "v"), ("saved-styles-001", "v2")],
    ),
    (
        "Circle",
        &[("saved-styles-002", "k"), ("saved-styles-002", "k2")],
    ),
    (
        "Conic",
        &[("saved-styles-002", "e"), ("saved-styles-002", "e2")],
    ),
    (
        "Arc",
        &[("saved-styles-002", "a"), ("saved-styles-002", "a2")],
    ),
    (
        "Sector",
        &[("saved-styles-002", "d"), ("saved-styles-002", "d2")],
    ),
    (
        "Number",
        &[
            ("saved-styles-003", "a"),
            ("saved-styles-003", "a2"),
            ("saved-styles-003", "n"),
        ],
    ),
    (
        "Angle",
        &[("saved-styles-003", "w"), ("saved-styles-003", "w2")],
    ),
    (
        "Boolean",
        &[
            ("saved-styles-003", "check"),
            ("saved-styles-003", "flag"),
            ("saved-styles-003", "lock"),
        ],
    ),
    (
        "Function",
        &[("saved-styles-004", "f"), ("saved-styles-004", "f2")],
    ),
    (
        "Polygon",
        &[("saved-styles-005", "tri"), ("saved-styles-005", "sq")],
    ),
    (
        "PolyLine",
        &[("saved-styles-005", "open"), ("saved-styles-005", "open2")],
    ),
    (
        "List",
        &[("saved-styles-005", "pts"), ("saved-styles-005", "nums")],
    ),
    (
        "Text",
        &[
            ("saved-styles-006", "title"),
            ("saved-styles-006", "value"),
            ("saved-styles-006", "boxed"),
            ("saved-styles-006", "tally"),
            ("saved-styles-006", "total"),
            ("saved-styles-006", "hiddenText"),
        ],
    ),
    (
        "Locus",
        &[("saved-styles-004", "path"), ("saved-styles-004", "path2")],
    ),
    (
        "PenStroke",
        &[("saved-styles-013", "s"), ("saved-styles-013", "s2")],
    ),
    (
        "Curve",
        &[("saved-styles-004", "c"), ("saved-styles-004", "c2")],
    ),
    (
        "ImplicitCurve",
        &[("saved-styles-010", "p"), ("saved-styles-010", "q")],
    ),
    (
        "FunctionNVar",
        &[("saved-styles-004", "j"), ("saved-styles-004", "j2")],
    ),
    (
        "Image",
        &[("saved-styles-008", "art"), ("saved-styles-008", "pinned")],
    ),
    (
        "PieChart",
        &[
            ("saved-styles-005", "chart"),
            ("saved-styles-005", "chart2"),
        ],
    ),
    (
        "InputBox",
        &[("saved-styles-007", "field"), ("saved-styles-007", "empty")],
    ),
    (
        "Button",
        &[("saved-styles-007", "run"), ("saved-styles-007", "run2")],
    ),
];

/// Properties the document keeps in its own state instead of the saved
/// style, with the value an object made in the application exports. They
/// survive the stripped export, so they are proven where the source differs
/// from that value.
fn new_object(kind: &str, property: &str) -> Option<Value> {
    Some(match (kind, property) {
        (_, "shown") => json!(true),
        (_, "layer") => json!(0),
        (_, "condition") => Value::Null,
        (_, "label_offset") => json!([0, 0]),
        // The kernel keeps the caption of a control and the opacity of an image.
        ("Boolean" | "Button" | "InputBox", "caption") => json!(""),
        ("Image", "alpha") => json!(1.0),
        // A chart draws with its saved label visibility and thickness.
        ("PieChart", "label_shown") => json!(true),
        ("PieChart", "line.thickness") => json!(3),
        ("Angle", "angle_style") => json!(0),
        ("Text", "latex") => json!(false),
        ("Text", "font.size") => json!(1.0),
        ("Text", "font.serif") => json!(false),
        ("Text", "font.style") => json!(0),
        ("Text", "screen_location") => json!(false),
        ("Text", "start_point") => Value::Null,
        _ => return None,
    })
}

/// The presentation properties an object of a kernel kind keeps in the
/// reference application, by the names of the style snapshot. A property is
/// left out where the reference application itself does not keep it; the
/// comments say which.
fn kind_properties(kind: &str) -> Vec<&'static str> {
    const SHARED: &[&str] = &[
        "shown",
        "views",
        "layer",
        "ordering",
        "color",
        "condition",
        "dynamic_color",
        "color_space",
        "selection_allowed",
        "tooltip_mode",
        "dynamic_caption",
        "breakpoint",
        "algebra_label",
        "fixed_size",
    ];
    const FLAGS: &[&str] = &["fixed", "auxiliary"];
    const LABEL: &[&str] = &["label_shown", "label_mode", "caption", "label_offset"];
    const LINE: &[&str] = &[
        "line.type",
        "line.thickness",
        "line.opacity",
        "line.hidden_type",
    ];
    const FILL: &[&str] = &[
        "alpha",
        "fill.type",
        "fill.angle",
        "fill.distance",
        "fill.symbol",
        "fill.inverse",
    ];
    let groups: &[&[&str]] = match kind {
        "Point" => &[
            SHARED,
            FLAGS,
            LABEL,
            &[
                "point_size",
                "point_style",
                "trace",
                "show_trimmed",
                "animation.playing",
            ],
        ],
        "Line" | "Ray" | "Function" | "Curve" | "ImplicitCurve" => {
            &[SHARED, FLAGS, LABEL, LINE, &["trace"]]
        }
        "PolyLine" => &[
            SHARED,
            FLAGS,
            LABEL,
            LINE,
            &["trace", "start_style", "end_style"],
        ],
        "Vector" => &[SHARED, FLAGS, LABEL, LINE, &["trace", "head_style"]],
        // A pen stroke is a locus in the reference application.
        "Locus" | "PenStroke" => &[SHARED, &["fixed"], LABEL, LINE, &["trace", "draw_arrows"]],
        "Segment" => &[
            SHARED,
            FLAGS,
            LABEL,
            LINE,
            &["trace", "decoration", "start_style", "end_style"],
        ],
        "Circle" | "Conic" | "Arc" | "Sector" | "Polygon" => {
            &[SHARED, FLAGS, LABEL, LINE, FILL, &["trace"]]
        }
        "List" => &[
            SHARED,
            FLAGS,
            LABEL,
            LINE,
            FILL,
            &["point_size", "point_style", "trace"],
        ],
        "FunctionNVar" => &[SHARED, FLAGS, LABEL, FILL, &["line.type", "line.thickness"]],
        "PieChart" => &[SHARED, &["auxiliary"], LABEL, LINE, FILL],
        "Number" => &[
            SHARED,
            FLAGS,
            LABEL,
            &[
                "line.thickness",
                "line.opacity",
                "slider.position",
                "slider.width",
                "slider.knob_size",
                "slider.horizontal",
                "slider.fixed",
                "slider.absolute",
                "slider.show_algebra",
                "animation.step",
                "animation.speed",
                "animation.mode",
                "animation.playing",
            ],
        ],
        "Angle" => &[
            SHARED,
            FLAGS,
            LABEL,
            LINE,
            FILL,
            &[
                "angle_style",
                "arc_size",
                "emphasize_right_angle",
                "decoration",
            ],
        ],
        "Boolean" => &[SHARED, FLAGS, LABEL, &["checkbox_fixed"]],
        "Text" => &[
            SHARED,
            &[
                "fixed",
                "background",
                "font.size",
                "font.serif",
                "font.style",
                "latex",
                "decimals",
                "figures",
                "screen_location",
                "start_point",
                "label_offset",
            ],
        ],
        "InputBox" => &[SHARED, &["fixed", "caption", "background"]],
        // The reference shows the algebra label again when it names an
        // object that only its element tag creates, as a button and an image
        // are, so it does not keep that flag for them.
        "Button" => &[&[
            "shown",
            "views",
            "layer",
            "ordering",
            "color",
            "condition",
            "dynamic_color",
            "color_space",
            "selection_allowed",
            "tooltip_mode",
            "dynamic_caption",
            "breakpoint",
            "fixed_size",
            "fixed",
            "caption",
            "background",
        ]],
        "Image" => &[
            &[
                "shown",
                "views",
                "layer",
                "ordering",
                "condition",
                "selection_allowed",
                "tooltip_mode",
                "dynamic_caption",
                "breakpoint",
                "fixed_size",
                "alpha",
            ],
            FLAGS,
        ],
        other => panic!("no property list for {other}"),
    };
    groups
        .iter()
        .flat_map(|group| group.iter().copied())
        .collect()
}

/// What the reference application saves for a kind and the importer does not
/// read. A kind with an entry stays `partial`.
const NOT_KEPT: &[(&str, &str)] = &[
    ("Point", "polar and complex coordinate style and the vertical increment are not kept"),
    ("Vector", "the start point and the coordinate style are not kept"),
    ("Segment", "the outlying-intersection and keep-type flags are not kept"),
    ("Ray", "the outlying-intersection and keep-type flags are not kept"),
    ("Arc", "the outlying-intersection and keep-type flags are not kept"),
    ("Sector", "the outlying-intersection and keep-type flags are not kept"),
    ("Number", "the symbolic mode and the slope triangle size are not kept"),
    ("Boolean", "the position of the check box is not kept"),
    ("Function", "the show-on-axis and simplify flags and the table column are not kept"),
    ("Polygon", "the mask flag is not kept"),
    ("List", "the drop-down display with its selected index and the font and rounding of a list are not kept"),
    ("Text", "the symbolic mode is not kept"),
    ("FunctionNVar", "the level of detail is not kept"),
    ("PieChart", "the colour and filling of single slices are not kept"),
    ("InputBox", "the font and size and position and the length and symbolic mode and alignment and typed text are not kept"),
    ("Button", "the font and size and position and the image are not kept"),
];

/// The leaves of a style snapshot by their path (`line.type`). A list is one leaf.
fn flatten(path: &str, value: &Value, out: &mut BTreeMap<String, Value>) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                let path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                flatten(&path, value, out);
            }
        }
        leaf => {
            out.insert(path.to_owned(), leaf.clone());
        }
    }
}

/// Whether the export of every object of a kind is written from its own
/// stored data: the archive is the same when the journal line that made the
/// object is replaced by a form no writer knows. `None` for a kind without
/// an object in the corpus.
fn journal_independence() -> BTreeMap<String, bool> {
    let mut independent: BTreeMap<String, bool> = BTreeMap::new();
    for (name, wb, view, _) in corpus() {
        let whole = wb
            .export_ggb(view)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let c = wb.construction();
        for (label, id) in labelled(c) {
            if !c.value(id).is_defined() {
                continue;
            }
            let same = wb
                .export_ggb_without_definition(view, label)
                .is_ok_and(|bytes| bytes == whole);
            *independent
                .entry(format!("{:?}", c.kind(id)))
                .or_insert(true) &= same;
        }
    }
    independent
}

/// The `export` column of `docs/kinds.csv`, derived in a normal test run.
///
/// `yes` needs all of: every object of the kind in the corpus is exported
/// from its own stored data, whatever its journal line says (a construction
/// kind also depends on a saved form of its definition and on the command
/// history); the recorded proof of the reload oracle has a witness for every
/// property of the kind; and nothing the reference application saves for the
/// kind is known not to be kept. Otherwise the cell is `partial` and the
/// note says why. A kind without a defined example in the corpus is `no`.
#[test]
fn the_export_column_follows_the_journal_rule_and_the_recorded_proof() {
    let proof: BTreeMap<String, BTreeMap<String, Value>> =
        serde_json::from_str(&fs::read_to_string(proof_path()).unwrap()).unwrap();
    let independent = journal_independence();
    let table = fs::read_to_string(repo_root().join("docs/kinds.csv")).unwrap();
    let matrix = geo_golden::table::Table::parse(&table, "docs/kinds.csv").unwrap();
    let mut errors = Vec::new();
    for row in matrix.records() {
        let kind = row["kernel_kind"];
        let Some(&own_data) = independent.get(kind) else {
            if row["export"] != "no" {
                errors.push(format!("{kind}: no defined example is exported"));
            }
            continue;
        };
        let proven = proof.get(kind);
        let mut reasons = Vec::new();
        if !own_data {
            reasons.push("its definition needs a saved form".to_owned());
        }
        let missing: Vec<_> = kind_properties(kind)
            .into_iter()
            .filter(|property| proven.is_none_or(|proven| !proven.contains_key(*property)))
            .collect();
        if !missing.is_empty() {
            reasons.push(format!("no witness for {}", missing.join(" and ")));
        }
        reasons.extend(
            NOT_KEPT
                .iter()
                .filter(|(lossy, _)| *lossy == kind)
                .map(|(_, loss)| (*loss).to_owned()),
        );
        let (cell, note) = if reasons.is_empty() {
            ("yes", String::new())
        } else {
            ("partial", format!("Export: {}.", reasons.join("; ")))
        };
        println!(
            "{kind}: {cell}, {} of {} properties proven. {note}",
            kind_properties(kind).len() - missing.len(),
            kind_properties(kind).len()
        );
        if row["export"] != cell {
            errors.push(format!(
                "{kind}: export is `{}`, derived `{cell}`. {note}",
                row["export"]
            ));
        } else if !row["notes"].ends_with(&note)
            || (note.is_empty() && row["notes"].contains("Export:"))
        {
            errors.push(format!("{kind}: the note must end with `{note}`"));
        }
    }
    assert!(
        errors.is_empty(),
        "docs/kinds.csv export column:\n{}",
        errors.join("\n")
    );
}

fn java_snapshots(dir: &std::path::Path) -> std::process::Output {
    Command::new(repo_root().join("tools/ggb-harness/harness.sh"))
        .arg("export-snapshot")
        .arg(dir)
        .output()
        .expect("run Java harness")
}

/// Snapshots that record what the reader reports instead of stopping at it.
fn lenient_java_snapshots(dir: &std::path::Path) -> std::process::Output {
    Command::new(repo_root().join("tools/ggb-harness/harness.sh"))
        .args(["export-snapshot", "--lenient"])
        .arg(dir)
        .output()
        .expect("run Java harness")
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn java_oracle_rejects_an_unknown_element_tag() {
    let dir = Scratch::new("unknown-tag");
    let xml = br#"<geogebra format="5.0"><construction>
      <element type="point" label="A">
        <coords x="1" y="2" z="1"/>
        <unknownExportTag val="1"/>
      </element>
    </construction></geogebra>"#;
    fs::write(dir.0.join("bad.ggb"), archive(xml, None)).unwrap();
    let result = java_snapshots(&dir.0);
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "{error}");
    assert!(
        error.contains("unknown tag in <element>: unknownExportTag"),
        "{error}"
    );
}

fn check_styles(name: &str, wb: &Workbench, view: Option<FileView>, snap: &Value) {
    let presentation = wb.export_presentation(view);
    let view = presentation.view;
    let settings = presentation.settings;
    assert_json(
        &json!({
            "origin": view.origin,
            "scale": view.scale,
            "axes": view.axes,
            "grid": view.grid,
            "angle": settings.angle_unit.as_deref().unwrap_or("degree"),
            "rounding": if settings.significant_figures.is_some() { "figures" } else { "decimals" },
            "digits": settings.significant_figures.or(settings.decimals).unwrap_or(2),
        }),
        &snap["settings"],
        name,
    );

    let c = wb.construction();
    let mut positions = BTreeSet::new();
    for (label, id) in labelled(c) {
        let actual = &snap["styles"][label];
        let where_ = format!("{name}: {label}");
        let drawing = c.drawing_style(id);
        if matches!(c.def(id), Def::Path(PathDef::On { path } | PathDef::OnParam { path, .. }) if c.kind(path) == Kind::Curve)
        {
            assert_json(
                &json!(c.path_state(id).unwrap().parameter()),
                &actual["curve_parameter"],
                &where_,
            );
        }
        if matches!(c.def(id), Def::Number(_)) {
            let [min, max] = c.slider_bounds(id);
            if min.is_some() || max.is_some() {
                // The bounds are the kernel's: the reference gives a visible
                // slider a default for an endpoint its own file leaves out.
                let slider = &actual["slider"];
                assert_json(&json!(min), &slider["min"], &where_);
                assert_json(&json!(max), &slider["max"], &where_);
                let moving = &actual["animation"];
                let animation = presentation.slider_animation.get(label);
                // A slider made by its command keeps the step of the command.
                let command_step = wb
                    .definition(label)
                    .and_then(geo_kernel::script::split_call)
                    .and_then(|(name, args)| match (name, args.as_slice()) {
                        ("Slider", [_, _, step]) => Some(*step),
                        _ => None,
                    });
                let from_file = presentation.object_styles.contains_key(label);
                let step = match animation {
                    Some(animation) => number_or_reference(c, animation.step.as_deref()),
                    // A file slider without the tag steps automatically.
                    None if from_file => Value::Null,
                    None => number_or_reference(c, command_step.or(Some("0.1"))),
                };
                let speed = animation.and_then(|a| a.speed.as_deref()).or(Some("1"));
                assert_json(&step, &moving["step"], &format!("{where_}.step"));
                // The reference reports the speed of an undefined
                // expression as 0 (`GeoElement.getAnimationSpeed`).
                let speed = match number_or_reference(c, speed) {
                    Value::Null => json!(0.0),
                    speed => speed,
                };
                assert_json(&speed, &moving["speed"], &where_);
                assert_json(
                    &json!(animation.map_or(0, |a| a.mode)),
                    &moving["mode"],
                    &where_,
                );
                assert_json(
                    &json!(animation.is_some_and(|a| a.playing)),
                    &moving["playing"],
                    &where_,
                );
                // A slider made in the application gets a place of its own.
                assert!(
                    presentation.object_styles.contains_key(label)
                        || positions.insert(slider["position"].to_string()),
                    "{where_}: overlapping sliders"
                );
            }
        }
        // An object from a file is compared with its source, tag by tag.
        if presentation.object_styles.contains_key(label) {
            continue;
        }
        let caption = c.control(id).map_or("", |control| control.caption.as_str());
        let offset = c.text_presentation(id).map_or_else(
            || {
                presentation
                    .label_offsets
                    .get(label)
                    .map_or([0, 0], |&(x, y)| [x, y])
            },
            |text| match text.position {
                TextPosition::Screen { x, y } => [x as i32, y as i32],
                _ => text.offset,
            },
        );
        // A free number without bounds is left to the reader, which keeps
        // it out of the view: shown, it would become a slider.
        let plain_number =
            matches!(c.def(id), Def::Number(_)) && c.slider_bounds(id) == [None, None];
        let mut expected = json!({
            "shown": !presentation.hidden.contains(label) && !plain_number,
            "label_shown": drawing.show_label,
            "layer": drawing.layer,
            "label_mode": 0,
            "label_offset": offset,
            "caption": caption,
            "color": presentation.colors[label],
            "dynamic_color": null,
            "background": null,
            "selection_allowed": true,
            "tooltip_mode": 0,
            "decoration": 0,
        });
        // The canvas tints polygons and angle sectors; an image has its own opacity.
        if c.image(id).is_none() {
            // The reference reads `alpha` as a 32-bit float
            // (`Float.parseFloat`), whatever decimal the file spells.
            expected["alpha"] = json!(geo_app_core::style::fill_opacity(c, id) as f32);
        }
        for (key, value) in expected.as_object().unwrap() {
            assert_json(value, &actual[key], &format!("{where_}.{key}"));
        }
        let kind = c.kind(id);
        if !matches!(
            kind,
            Kind::Point | Kind::Image | Kind::Text | Kind::Boolean | Kind::Button | Kind::InputBox
        ) {
            assert_json(&json!(0), &actual["line"]["type"], &where_);
            assert_json(
                &json!(drawing.line_thickness),
                &actual["line"]["thickness"],
                &where_,
            );
        }
        if let Some(text) = c.text_presentation(id) {
            assert_json(&json!(text.latex), &actual["latex"], &where_);
        }
    }
}

fn number_or_reference(c: &Construction, source: Option<&str>) -> Value {
    // A step or a speed is a number or arithmetic over the construction. An
    // undefined value is null, as in the reference snapshot.
    source.map_or(Value::Null, |source| {
        let value = c
            .numeric_value(source)
            .unwrap_or_else(|| panic!("`{source}` is not a number of the construction"));
        json!(value)
    })
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
    fn new(case: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "geokernel-ggb-oracle-{case}-{}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn archive(xml: &[u8], library: Option<&[u8]>) -> Vec<u8> {
    use std::io::{Cursor, Write};

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("geogebra.xml", options).unwrap();
    zip.write_all(xml).unwrap();
    if let Some(library) = library {
        zip.start_file("geogebra_javascript.js", options).unwrap();
        zip.write_all(library).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// The `alpha` attribute of every `<objColor>` in `xml`, by element label.
fn alphas(xml: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for element in xml.split("<element ").skip(1) {
        let attribute = |text: &str, name: &str| {
            let start = text
                .find(&format!(" {name}=\""))
                .map(|at| at + name.len() + 3)?;
            Some(text[start..start + text[start..].find('"')?].to_owned())
        };
        let element = &element[..element.find("</element>").unwrap_or(element.len())];
        let label = attribute(&format!(" {element}"), "label").unwrap();
        if let Some(color) = element.find("<objColor") {
            if let Some(alpha) = attribute(&element[color..], "alpha") {
                out.insert(label, alpha);
            }
        }
    }
    out
}

#[test]
fn opacities_are_written_as_the_shortest_decimal_of_the_stored_value() {
    use std::io::{Cursor, Read, Write};

    // An image saved with an opacity of 0.3, which the kernel stores as a
    // 32-bit float.
    let source = fs::read(repo_root().join("golden/ggb/saved/saved-images-001.ggb")).unwrap();
    let mut fixture = zip::ZipArchive::new(Cursor::new(source.as_slice())).unwrap();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..fixture.len() {
        let mut file = fixture.by_index(i).unwrap();
        let name = file.name().unwrap().into_owned();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if name == "geogebra.xml" {
            let xml = String::from_utf8(data).unwrap();
            assert_eq!(xml.matches(r#"alpha="0.5""#).count(), 1);
            data = xml.replace(r#"alpha="0.5""#, r#"alpha="0.3""#).into_bytes();
        }
        out.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        out.write_all(&data).unwrap();
    }
    let source = out.finish().unwrap().into_inner();
    let mut wb = Workbench::default();
    wb.load_for_export(&source, "opacity.ggb").unwrap();
    let written = alphas(&exported_xml(&wb));
    assert_eq!(written["anchoredImage"], "0.3", "{written:?}");
    assert_eq!(written["fixedImage"], "1", "{written:?}");

    // Objects made in the application: the canvas tints a polygon.
    let mut wb = Workbench::default();
    wb.load_script("A = (0, 0)\nB = (4, 0)\nC = (1, 3)\ntri = Polygon(A, B, C)")
        .unwrap();
    let written = alphas(&exported_xml(&wb));
    assert_eq!(written["tri"], "0.1", "{written:?}");
    assert_eq!(written["A"], "0", "{written:?}");
    for alpha in written.values() {
        assert!(alpha.len() <= 3, "{written:?}");
    }
}

#[test]
fn imported_library_is_omitted_from_export() {
    let library = b"function ggbOnInit() { throw Error('unexpected execution'); }";
    let source = archive(b"<geogebra><construction/></geogebra>", Some(library));
    assert_eq!(geo_import::import(&source).unwrap().javascript, library);
    let mut wb = Workbench::default();
    wb.load_for_export(&source, "library.ggb").unwrap();
    let exported = wb.export_ggb(None).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(exported.as_slice())).unwrap();
    assert!(matches!(
        archive.by_name("geogebra_javascript.js"),
        Err(zip::result::ZipError::FileNotFound)
    ));
    let xml = geo_import::archive::extract_xml(&exported).unwrap();
    assert!(xml.contains("disabled=\"true\""));
}

/// Attributes the reference reader hands to its command processor, filled
/// with commands that would change the construction when the file is read.
const HOSTILE_FILE: &[u8] = br#"<geogebra format="5.0"><construction>
  <element type="point" label="B"><coords x="0" y="0" z="1"/></element>
  <element type="numeric" label="r"><value val="0.5"/></element>
  <element type="point" label="A"><coords x="1" y="2" z="1"/>
    <objColor r="10" g="20" b="30" alpha="0"
      dynamicr="Execute[{&quot;Delete[B]&quot;}]" dynamicg="r" dynamicb="r"/>
    <animation step="RunClickScript[A]" speed="Execute[{&quot;Delete[B]&quot;}]"
      type="1" playing="false"/>
  </element>
  <element type="point" label="C"><coords x="3" y="2" z="1"/>
    <objColor r="10" g="20" b="30" alpha="0" dynamicr="r" dynamicg="1 - r" dynamicb="r"/>
  </element>
  <element type="numeric" label="a"><value val="1"/>
    <slider min="0" max="5" x="20" y="40" width="100" horizontal="true"
      absoluteScreenLocation="true"/>
    <animation step="Execute[{&quot;Delete[B]&quot;}]" speed="SetValue[r, 3]"
      type="0" playing="false"/>
  </element>
</construction></geogebra>"#;

/// `SlowPlot` hides its source graph and plays its new slider from 0 to 1,
/// again and again with the repeat flag 1 and once otherwise. The export of
/// the same script says so as the file the reference application saved
/// does: the visibility of every object, and the bounds and animation of
/// every slider.
#[test]
fn slow_plot_hides_its_source_and_plays_its_slider_as_the_reference_saves_it() {
    let dir = repo_root().join("golden/ggb/saved");
    let saved = fs::read_to_string(dir.join("saved-slowplot-001.xml")).unwrap();
    let snapshot: Value =
        serde_json::from_slice(&fs::read(dir.join("saved-slowplot-001.snapshot")).unwrap())
            .unwrap();
    let script: Vec<&str> = snapshot["script"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap())
        .collect();
    let wb = Workbench::from_script(&script.join("\n")).unwrap();
    let elements = |text: &str| {
        geo_import::xml::parse(text)
            .unwrap()
            .items
            .into_iter()
            .filter_map(|item| match item {
                geo_import::xml::Item::Element(el) => Some((el.label.clone(), el)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>()
    };
    let (reference, ours) = (elements(&saved), elements(&exported_xml(&wb)));
    let mut sliders = 0;
    for (label, want) in &reference {
        let got = ours
            .get(label)
            .unwrap_or_else(|| panic!("the export has no `{label}`"));
        assert_eq!(
            got.shown.unwrap_or(true),
            want.shown.unwrap_or(true),
            "{label}: shown"
        );
        assert_eq!(got.slider, want.slider, "{label}: slider bounds");
        // every element of the reference has an animation tag; a slider's
        // is the one SlowPlot sets
        if want.slider.is_some() {
            assert_eq!(
                got.slider_animation, want.slider_animation,
                "{label}: animation"
            );
            sliders += 1;
        }
    }
    assert_eq!(sliders, 3);
    assert!(wb.is_animating());
}

/// A running animation is saved with `<startAnimation>` and the slider as
/// playing, which the reference reader starts again (`MyXMLHandler`); a
/// slider that has played once to its end is saved as stopped.
#[test]
fn a_running_slow_plot_animation_is_saved_and_restarts_on_load() {
    let mut wb = Workbench::from_script("f(x) = x\ng = SlowPlot(f)\nh = SlowPlot(f, 0)").unwrap();
    let xml = exported_xml(&wb);
    assert!(xml.contains("<startAnimation val=\"true\"/>"), "{xml}");
    // (the functions read view corners, which the importer skips)
    let imported = geo_import::import(&wb.export_ggb(None).unwrap()).unwrap();
    let c = geo_kernel::script::load(&imported.script.join("\n")).unwrap();
    assert!(c.is_animating());
    for (label, mode) in [("a", 1), ("b", 3)] {
        let a = c.animation(c.id(label).unwrap()).unwrap();
        assert!(a.playing && a.auto_step && a.speed == 1.0, "{label}");
        assert_eq!(a.mode.code(), mode, "{label}");
    }
    // the slider that plays once stops; the other keeps the timer running
    for _ in 0..400 {
        wb.animate(geo_kernel::kernel::animation::MAX_ANIMATION_FRAME_RATE);
    }
    let xml = exported_xml(&wb);
    assert!(xml.contains("<startAnimation val=\"true\"/>"));
    assert!(xml.contains("type=\"3\" playing=\"false\""), "{xml}");
    let mut once = Workbench::from_script("f(x) = x\ng = SlowPlot(f, 0)").unwrap();
    for _ in 0..400 {
        once.animate(geo_kernel::kernel::animation::MAX_ANIMATION_FRAME_RATE);
    }
    assert!(!once.is_animating());
    assert!(!exported_xml(&once).contains("startAnimation"));
}

fn exported_xml(wb: &Workbench) -> String {
    let bytes = wb.export_ggb(None).unwrap();
    geo_import::archive::extract_xml(&bytes)
        .unwrap()
        .into_owned()
}

fn hostile_export() -> (Workbench, String) {
    let mut wb = Workbench::default();
    wb.load_for_export(HOSTILE_FILE, "hostile").unwrap();
    let xml = exported_xml(&wb);
    (wb, xml)
}

#[test]
fn export_writes_no_expression_the_kernel_has_not_evaluated() {
    let (mut wb, xml) = hostile_export();
    for command in ["Execute", "RunClickScript", "SetValue", "Delete"] {
        assert!(!xml.contains(command), "{command} in\n{xml}");
    }
    // Arithmetic over the construction is kept, in the kernel's syntax.
    assert!(
        xml.contains(r#"dynamicr="r" dynamicg="1 - r" dynamicb="r""#),
        "{xml}"
    );
    assert_eq!(xml.matches("dynamicr").count(), 1, "{xml}");
    // The colour follows a number only while the number exists.
    let r = wb.construction().id("r").unwrap();
    assert!(wb.delete(r));
    let xml = exported_xml(&wb);
    assert!(!xml.contains("dynamic"), "{xml}");
    assert!(xml.contains(r#"<objColor r="10" g="20" b="30""#), "{xml}");
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn geogebra_runs_nothing_from_the_export_of_a_hostile_file() {
    let dir = Scratch::new("hostile");
    let (wb, _) = hostile_export();
    fs::write(dir.0.join("hostile.ggb"), wb.export_ggb(None).unwrap()).unwrap();
    let output = java_snapshots(&dir.0);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snap: Value =
        serde_json::from_slice(&fs::read(dir.0.join("hostile.ggb.snapshot")).unwrap()).unwrap();
    let styles = snap["styles"].as_object().unwrap();
    // Nothing was deleted and nothing was added.
    let labels: BTreeSet<&str> = styles.keys().map(String::as_str).collect();
    assert_eq!(labels, BTreeSet::from(["A", "B", "C", "a", "r"]));
    assert_eq!(styles["A"]["dynamic_color"], Value::Null);
    assert_eq!(styles["C"]["dynamic_color"], json!(["r", "1 - r", "r"]));
    assert_eq!(styles["a"]["animation"]["speed"], json!(1.0));
    assert_eq!(snap["objects"]["r"]["value"], json!(0.5));
}

#[test]
fn an_undefined_object_keeps_its_saved_style_and_recovers_after_export() {
    let source = fs::read(repo_root().join("golden/ggb/saved/saved-styles-009.xml")).unwrap();
    let mut wb = Workbench::default();
    wb.load_for_export(&source, "undefined").unwrap();
    let undefined = ["Q", "s", "M"];
    for label in undefined {
        let id = wb.construction().id(label).unwrap();
        assert!(!wb.construction().value(id).is_defined(), "{label}");
    }
    let (_, mut again) = round_trip("undefined", &wb, None);
    // The circle comes back onto the line in both documents.
    for document in [&mut wb, &mut again] {
        let center = document.construction().id("C").unwrap();
        assert!(document.move_point(center, 2.0, 0.5));
    }
    for label in undefined {
        let (before, after) = (wb.construction(), again.construction());
        let value = before.value(before.id(label).unwrap());
        assert!(value.is_defined(), "{label}");
        assert_eq!(value, after.value(after.id(label).unwrap()), "{label}");
    }
    // The style written for the undefined point is still written for the
    // defined one.
    let xml = exported_xml(&again);
    let element = &xml[xml.find(r#"label="Q""#).unwrap()..];
    let element = &element[..element.find("</element>").unwrap()];
    for tag in [
        r#"<pointSize val="9"/>"#,
        r#"<pointStyle val="9"/>"#,
        r#"<trace val="true"/>"#,
        r#"<fixed val="true"/>"#,
        r#"dynamicr="r""#,
        r#"<condition showObject="show"/>"#,
    ] {
        assert!(element.contains(tag), "{tag} in\n{element}");
    }
    assert!(!element.contains("NaN"), "{element}");
}

#[test]
fn a_style_expression_over_undefined_inputs_is_imported_and_written() {
    let source = fs::read(repo_root().join("golden/ggb/saved/saved-styles-012.xml")).unwrap();
    let notes = geo_import::import(&source).unwrap().notes;
    assert!(
        !notes
            .iter()
            .any(|note| note.contains("dynamic colour") || note.contains("animation")),
        "{notes:?}"
    );
    let mut wb = Workbench::default();
    wb.load_for_export(&source, "undefined-inputs").unwrap();
    let written = [
        r#"dynamicr="x(Q) / 10" dynamicg="sqrt(a)" dynamicb="b""#,
        r#"step="sqrt(a)""#,
        r#"speed="x(Q) + 1""#,
    ];
    let check = |wb: &Workbench, state: &str| {
        let xml = exported_xml(wb);
        for attribute in written {
            assert!(xml.contains(attribute), "{state}: {attribute} in\n{xml}");
        }
    };
    check(&wb, "as saved");
    // The inputs become defined, then undefined again: the expressions are
    // written in every state.
    let id = |wb: &Workbench, label: &str| wb.construction().id(label).unwrap();
    let (center, a, q) = (id(&wb, "C"), id(&wb, "a"), id(&wb, "Q"));
    assert!(wb.move_point(center, 2.0, 0.5));
    assert!(wb.set_number(a, 4.0));
    assert!(wb.construction().value(q).is_defined());
    check(&wb, "defined");
    assert!(wb.move_point(center, 2.0, 5.0));
    assert!(wb.set_number(a, -1.0));
    assert!(!wb.construction().value(q).is_defined());
    check(&wb, "undefined again");
}

/// Without the Java reader: a file with a tag the reference gives up at is
/// imported as its twin, which spells out what the tag had applied by then,
/// and the import says so.
#[test]
fn a_tag_the_reference_gives_up_at_keeps_what_it_had_applied() {
    let mut twins = 0;
    for (name, source, applied) in hand_written() {
        let Some(applied) = applied else { continue };
        twins += 1;
        let (given_up, twin) = (
            geo_import::import(&source).unwrap(),
            geo_import::import(&applied).unwrap(),
        );
        assert_eq!(
            format!("{:?}", given_up.object_styles),
            format!("{:?}", twin.object_styles),
            "{name}"
        );
        assert!(
            given_up
                .notes
                .iter()
                .any(|note| note.contains("gives up the file")),
            "{name}: {:?}",
            given_up.notes
        );
        assert!(twin.notes.is_empty(), "{name}: {:?}", twin.notes);
        // The export has the well-formed tag of the twin.
        let export = |bytes: &[u8]| {
            let mut wb = Workbench::default();
            wb.load_for_export(bytes, &name).unwrap();
            exported_xml(&wb)
        };
        assert_eq!(export(&source), export(&applied), "{name}");
    }
    assert_eq!(twins, HAND_GIVEN_UP.len());
}

#[test]
fn export_refuses_a_document_opened_with_skips() {
    let source = br#"<geogebra><construction>
      <element type="point" label="A"><coords x="1" y="2" z="1"/></element>
      <element type="unsupported" label="B"/>
    </construction></geogebra>"#;
    let mut wb = Workbench::default();
    wb.load_bytes(source, "partial.ggb").unwrap();
    assert!(wb.construction().id("A").is_some());
    assert!(wb
        .export_ggb(None)
        .unwrap_err()
        .to_string()
        .contains("1 skipped"));
    wb.load_script("A = (1, 2)").unwrap();
    assert!(wb.export_ggb(None).is_ok());
}

#[test]
fn export_refuses_a_curve_intersection_with_unlabelled_slots() {
    let wb =
        Workbench::from_script("c = Curve(t, t^2, t, -2, 2)\ng: y = 1\n{, P} = Intersect(g, c)")
            .unwrap();
    assert!(wb
        .export_ggb(None)
        .unwrap_err()
        .to_string()
        .contains("unlabelled output slot"));
}

#[test]
fn export_refuses_a_curve_intersection_with_noncontiguous_replay_slots() {
    let wb = Workbench::from_script(
        "c = Curve(t, t^2, t, -2, 2)\ng: y = 0\nP = Intersect(g, c)\nQ = $boundarySlot(P_{1}, 3)",
    )
    .unwrap();
    assert!(wb
        .export_ggb(None)
        .unwrap_err()
        .to_string()
        .contains("$boundarySlot"));
}
