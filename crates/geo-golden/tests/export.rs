//! The same live Rust constructions go to the importer and to the headless
//! reference reader. The Java test builds no goldens and checks every label.

use geo_app_core::workbench::{FileView, Workbench};
use geo_golden::compare::{compare_construction_object, numbers_agree};
use geo_golden::model::{expected_from_json, load_all, Expected, ExpectedData, Step};
use geo_golden::repo_root;
use geo_kernel::algo::path::PathDef;
use geo_kernel::kernel::image::{ImageCorner, ImagePlacement};
use geo_kernel::kernel::text::TextPosition;
use geo_kernel::kernel::DrawingStyle;
use geo_kernel::script::saved::Property;
use geo_kernel::{Construction, Def, Id, Kind};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
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
    ("optimizerreview-reload-001", 1),
    ("optimizerreview-roundtrip-001", 4),
    ("optimizerreview-redefine-001", 2),
    ("optimizerreview-redefine-002", 1),
    ("optimizerreview-forms-001", 1),
    ("optimizerreview-forms-002", 0),
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
    ("points-infinityedit-003", 2),
];

const SAVED_CASES: &[&str] = &[
    "saved-controls-002.xml",
    "saved-curve-branch-001.xml",
    "saved-curve-intersections-001.xml",
    "saved-curve-intersections-002.xml",
    "saved-paths-002.xml",
    "saved-slope-arrows-001.xml",
    "saved-export-fixed-segment-001.xml",
    "saved-implicit-001.xml",
    "saved-images-001.ggb",
    "saved-images-002.ggb",
    "saved-object-scripts-001.xml",
    "saved-slider-bounds-001.xml",
    "saved-strokes-001.xml",
    "saved-texts-001.ggb",
    "saved-text-equations-001.xml",
    "saved-projective-001.xml",
    "saved-projectiveoverflow-001.xml",
    // Presentation properties per kind, set by the reference application.
    "saved-styles-001.xml",
    "saved-styles-002.xml",
    "saved-styles-003.xml",
    "saved-styles-004.xml",
    "saved-styles-005.xml",
    "saved-styles-006.xml",
    "saved-styles-007.xml",
    "saved-input-box-001.xml",
    "saved-input-rounding-001.xml",
    "saved-canvas-sector-001.xml",
    "saved-styles-008.ggb",
    "saved-element-properties-001.xml",
    "saved-element-properties-002.xml",
    "saved-element-properties-003.xml",
    "saved-element-properties-004.xml",
    "saved-element-properties-005.xml",
    "saved-element-properties-extra-001.xml",
    "saved-element-raster-properties-001.ggb",
    // Styled objects that are undefined in the saved state.
    "saved-styles-009.xml",
    "saved-styles-010.xml",
    "saved-styles-011.xml",
    // Dynamic colour channels, a step and a speed over objects that are
    // undefined in the saved state.
    "saved-styles-012.xml",
    // Pen strokes with presentation properties.
    "saved-styles-013.xml",
    "saved-export-limits-001.xml",
    "saved-export-limits-002.xml",
    "saved-export-limits-003.ggb",
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
                Step::ReloadGgb => {
                    let bytes = wb.export_ggb(None).unwrap();
                    wb.load_for_export(&bytes, name).unwrap();
                }
                Step::Reject { .. }
                | Step::Delete { .. }
                | Step::SetSliderBounds { .. }
                | Step::SliderAnimation { .. }
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
    out.push((
        "live-fixed-segment".into(),
        Workbench::from_script("A = (1, 1)\ns = Segment(A, 4)").unwrap(),
        None,
        Origin::Script,
    ));
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
fn live_geometry_keeps_command_inputs_when_the_journal_definition_is_unavailable() {
    let wb = Workbench::from_script(
        "A = (1, 2)\nB = (4, 3)\nC = (3, 5)\nm = Midpoint(A, B)\n\
         s = Segment(A, m)\nr = Ray(B, m)\np = PolyLine(A, C, m)",
    )
    .unwrap();
    let exported = wb.export_ggb(None).unwrap();
    let xml = geo_import::archive::extract_xml(&exported).unwrap();
    assert!(
        xml.contains("<command name=\"Midpoint\">\n<input a0=\"A\" a1=\"B\"/>\n<output a0=\"m\"/>")
    );
    for label in ["m", "s", "r", "p"] {
        let independent = wb
            .export_ggb_without_definition(None, label)
            .unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(independent, exported, "{label}");
    }
}

#[test]
fn native_polyline_export_checks_the_argument_limit() {
    use geo_import::export::{ggb, Definition, Presentation};
    use geo_kernel::algo::polygon::PolygonDef;

    let mut c = Construction::new();
    let a = c.add("A", Def::FreePoint { x: 1.0, y: 2.0 }).unwrap();
    let b = c.add("B", Def::FreePoint { x: 4.0, y: 3.0 }).unwrap();
    let points: Vec<_> = [a, b].into_iter().cycle().take(10_001).collect();
    c.add_with_list("p", &points, |ids| Def::Polygon(PolygonDef::PolyLine(ids)))
        .unwrap();
    let definitions = [
        Definition {
            line: "A = (1, 2)",
            outputs: &["A".into()],
        },
        Definition {
            line: "B = (4, 3)",
            outputs: &["B".into()],
        },
        Definition {
            line: "p = $unlisted()",
            outputs: &["p".into()],
        },
    ];
    assert_eq!(
        ggb(&c, definitions, &Presentation::default())
            .unwrap_err()
            .to_string(),
        "command exceeds the import argument limit"
    );
}

#[test]
fn native_fixed_segment_checks_the_generated_circle_attribute_limit() {
    use geo_import::export::{ggb, Definition, Presentation};
    use geo_kernel::algo::polygon::Count;

    let mut c = Construction::new();
    let label = "A".repeat(geo_import::xml::MAX_ATTRIBUTE_BYTES);
    let start = c.add(&label, Def::FreePoint { x: 1.0, y: 1.0 }).unwrap();
    let endpoint = c
        .add(
            "B",
            Def::Path(PathDef::FixedEndpoint {
                start,
                length: Count::Literal(4.0),
            }),
        )
        .unwrap();
    c.add("s", Def::Segment(start, endpoint)).unwrap();
    let definitions = [
        Definition {
            line: "A = (1, 1)",
            outputs: &[label],
        },
        Definition {
            line: "s = $unlisted()",
            outputs: &["B".into(), "s".into()],
        },
    ];
    assert_eq!(
        ggb(&c, definitions, &Presentation::default())
            .unwrap_err()
            .to_string(),
        "fixed segment circle exceeds the import attribute size limit"
    );
}

#[test]
fn a_fixed_segment_saves_its_endpoint_as_a_point_on_an_inline_circle() {
    let wb = Workbench::from_script("A = (1, 1)\ns = Segment(A, 4)").unwrap();
    let exported = wb.export_ggb_without_definition(None, "s").unwrap();
    let xml = geo_import::archive::extract_xml(&exported).unwrap();
    assert!(
        xml.contains("<command name=\"Point\">\n<input a0=\"Circle[A, 4]\"/>\n<output a0=\"B\"/>"),
        "{xml}"
    );
    assert!(
        xml.contains("<command name=\"Segment\">\n<input a0=\"A\" a1=\"B\"/>\n<output a0=\"s\"/>"),
        "{xml}"
    );
    let mut again = Workbench::default();
    again.load_for_export(&exported, "fixed segment").unwrap();
    let c = wb.construction();
    let restored = again.construction();
    for label in ["B", "s"] {
        assert_eq!(
            restored.value(restored.id(label).unwrap()),
            c.value(c.id(label).unwrap())
        );
    }
}

#[test]
fn live_expression_definitions_survive_redefinition_and_undo() {
    let mut wb = Workbench::from_script("a = 2\nf(x) = a x\nL = {a, f(3)}").unwrap();
    let compare = |wb: &Workbench, expression| {
        let export = wb.export_ggb(None).unwrap();
        let xml = geo_import::archive::extract_xml(&export).unwrap();
        assert!(xml.contains(expression), "{xml}");
        for label in ["f", "L"] {
            assert_eq!(
                wb.export_ggb_without_definition(None, label).unwrap(),
                export
            );
        }
    };
    compare(&wb, "f(x) = a x");
    wb.run_line("f(x) = a + x").unwrap();
    compare(&wb, "f(x) = a + x");
    wb.undo();
    compare(&wb, "f(x) = a x");
}

#[test]
fn exported_constructions_import_with_all_labelled_kinds_and_properties() {
    for (name, wb, view, _) in corpus() {
        round_trip(&name, &wb, view);
    }
}

#[test]
fn a_session_slider_keeps_its_expression_bound_in_the_archive() {
    let wb = Workbench::from_script("m = 10\na = Slider(0, m - 1)\na = 8").unwrap();
    let mut restored = Workbench::default();
    restored
        .load_for_export(&wb.export_ggb(None).unwrap(), "live-slider")
        .unwrap();
    let m = restored.construction().id("m").unwrap();
    assert!(restored.set_number(m, 5.0));
    let c = restored.construction();
    assert_eq!(c.value(c.id("a").unwrap()), geo_kernel::Value::Number(4.0));
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
                    let plain_snapshot = without_saved_canvas_styles(&wb, view);
                    let rejected = stripped.restore_text(&plain_snapshot).unwrap();
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
    for (scratch, run) in [
        (&exports, tag_snapshots as fn(&std::path::Path) -> _),
        (&sources, tag_snapshots),
        (&plain, java_snapshots),
        (&hand, lenient_java_snapshots),
    ] {
        let output = run(&scratch.0);
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
    let mut tag_proof = BTreeMap::new();
    let mut limit_proof = BTreeMap::new();
    let mut limit_failures = Vec::new();
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
                record_tag_witnesses(&mut tag_proof, name, &source, &snap);
                record_export_limit_witnesses(
                    &mut limit_proof,
                    &mut limit_failures,
                    name,
                    &source,
                    &snap,
                );
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
                if name == "saved-styles-013" {
                    // The fixture asks GeoLocus to set arrows on the stroke.
                    // The reference only accepts that setting on slope fields.
                    assert_eq!(source["styles"]["s"]["draw_arrows"], json!(false));
                }
                if name == "saved-slope-arrows-001" {
                    assert_eq!(source["styles"]["field"]["draw_arrows"], json!(true));
                    assert_json(&source["objects"]["field"], &snapshot["field"], name);
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
            if name == "saved-slope-arrows-001" && label == "field" {
                let expected = slope_field_shafts(snap);
                for (kernel, id) in [(c, id), (reconstructed, restored)] {
                    compare_construction_object(&expected, kernel, id)
                        .unwrap_or_else(|why| panic!("{name}: {label}: {why}"));
                }
            } else if matches!(kind, Kind::Image | Kind::Button | Kind::InputBox) {
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
    assert!(limit_failures.is_empty(), "{}", limit_failures.join("\n"));

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
    proof
        .entry("CAS")
        .or_default()
        .insert("rows", cas_export_witness());
    proof
        .entry("Spreadsheet")
        .or_default()
        .insert("cells", spreadsheet_export_witness());
    for (kind, properties) in limit_proof {
        proof.entry(kind).or_default().extend(properties);
    }
    proof.insert("ElementTags", tag_proof);
    let serialized = serde_json::to_string_pretty(&proof).unwrap() + "\n";
    let path = proof_path();
    if std::env::var_os("GEOKERNEL_EXPORT_BLESS").is_some() {
        fs::write(&path, &serialized).unwrap();
    }
    let recorded = fs::read_to_string(&path).unwrap();
    if recorded != serialized {
        let recorded: BTreeMap<&str, BTreeMap<&str, Value>> =
            serde_json::from_str(&recorded).expect("invalid export-proof.json");
        let mut differences = Vec::new();
        for kind in recorded.keys().chain(proof.keys()).collect::<BTreeSet<_>>() {
            let empty = BTreeMap::new();
            let expected = recorded.get(kind).unwrap_or(&empty);
            let actual = proof.get(kind).unwrap_or(&empty);
            for property in expected
                .keys()
                .chain(actual.keys())
                .collect::<BTreeSet<_>>()
            {
                let expected = expected.get(property).map(Value::to_string);
                let actual = actual.get(property).map(Value::to_string);
                if expected != actual {
                    let expected = expected.as_deref().unwrap_or("<missing>");
                    let actual = actual.as_deref().unwrap_or("<missing>");
                    differences.push(format!(
                        "{kind}.{property}: recorded {expected}, computed {actual}"
                    ));
                }
            }
        }
        let details = if differences.is_empty() {
            "The JSON entries agree, but the file serialization differs.".into()
        } else {
            differences.join("\n")
        };
        panic!(
            "tests/fixtures/export-proof.json is stale; rerun with GEOKERNEL_EXPORT_BLESS=1\n{details}"
        );
    }
    let element_tags = proof["ElementTags"].len();
    let pairs = proof.values().map(BTreeMap::len).sum::<usize>() - element_tags;
    println!(
        "{} exported files, {count} objects, {} kinds, {pairs} proven kind and property pairs, \
         {element_tags} element tags, {} files the reference gives up",
        constructions.len(),
        WITNESSES.len(),
        gave_up.len()
    );
}

/// Build the witness control with the same explicit thickness and label
/// defaults as the original stripped archives. The source and full export
/// still carry their real presentation and are compared property by property.
fn without_saved_canvas_styles(wb: &Workbench, view: Option<FileView>) -> String {
    let c = wb.construction();
    let presentation = wb.export_presentation(view);
    let mut script = String::new();
    for line in wb.snapshot_text().lines().filter(|line| {
        !line.starts_with("$sliderStyle(")
            && !line.starts_with("$checkboxStyle(")
            && !line.starts_with("$inputBoxStyle(")
            && !line.starts_with("$drawingStyle(")
    }) {
        writeln!(script, "{line}").unwrap();
    }
    for (label, id) in labelled(c) {
        let mut style = c.drawing_style(id);
        let chart = c.chart(id).is_some();
        let slider = matches!(c.def(id), Def::Number(_))
            && presentation
                .object_styles
                .get(label)
                .is_some_and(|saved| saved.slider.is_some());
        if !chart && !slider {
            style.line_thickness = DrawingStyle::default().line_thickness;
            if !matches!(c.def(id), Def::Boolean(_) | Def::InputBox(_)) {
                style.show_label = DrawingStyle::default().show_label;
            }
        }
        let property = Property::DrawingStyle {
            label: label.into(),
            style,
        };
        writeln!(script, "{property}").unwrap();
    }
    script
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
            // The journal retains slider thickness. The free number's
            // thickness still differs from its stripped export.
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
        &[
            ("saved-styles-004", "path"),
            ("saved-styles-004", "path2"),
            ("saved-slope-arrows-001", "field"),
        ],
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
        &[
            ("saved-styles-007", "field"),
            ("saved-styles-007", "empty"),
            ("saved-input-box-001", "field"),
            ("saved-input-rounding-001", "fixedPlaces"),
            ("saved-input-rounding-001", "significantDigits"),
        ],
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
        ("InputBox", "label_offset") => json!([30, 30]),
        ("InputBox", "input_length") => json!(20),
        (_, "label_offset") => json!([0, 0]),
        // The kernel keeps the caption of a control and the opacity of an image.
        ("Boolean" | "Button" | "InputBox", "caption") => json!(""),
        ("Image", "alpha") => json!(1.0),
        // Drawable controls and charts keep label visibility in the journal.
        ("Number" | "Boolean" | "PieChart", "label_shown") => json!(true),
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
        "Locus" => &[SHARED, &["fixed"], LABEL, LINE, &["trace", "draw_arrows"]],
        // GeoLocus.drawAsArrows ignores pen strokes.
        "PenStroke" => &[SHARED, &["fixed"], LABEL, LINE, &["trace"]],
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
        "InputBox" => &[
            SHARED,
            &[
                "fixed",
                "caption",
                "background",
                "label_offset",
                "input_length",
                "decimals",
                "figures",
            ],
        ],
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

/// Additional limits specific to a construction kind.
const PROPERTY_LIMITS: &[(&str, &str)] = &[
    (
        "Point",
        "vertical increments support scalar arithmetic with named list lengths as operands and standalone sums of one named list that the kernel types as numeric or empty",
    ),
    ("Button", "SVG and built-in icons are omitted because the asset store accepts archived rasters"),
    ("InputBox", "SVG and built-in icons are omitted because the asset store accepts archived rasters"),
];

fn definition_limit(kind: &str) -> &'static str {
    match kind {
        "Function" | "FunctionNVar" => "function expressions need a saved journal definition",
        "Curve" => "parametric curves need a saved journal definition",
        "List" => "dependent lists need a saved journal definition",
        "Text" => "dependent text expressions need a saved journal definition",
        _ => "its definition needs a saved form",
    }
}

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
/// history); the reload oracle has a witness for each modeled property; and
/// none of the listed property limits apply. The note qualifies the forms
/// the writer supports. A kind with a missing witness or a listed loss is
/// `partial`. A kind without a defined example in the corpus is `no`.
#[test]
fn the_export_column_follows_the_journal_rule_and_the_recorded_proof() {
    let proof: BTreeMap<String, BTreeMap<String, Value>> =
        serde_json::from_str(&fs::read_to_string(proof_path()).unwrap()).unwrap();
    let independent = journal_independence();
    let table = fs::read_to_string(repo_root().join("docs/kinds.csv")).unwrap();
    let matrix = geo_golden::table::Table::parse(&table, "docs/kinds.csv").unwrap();
    let mut errors = Vec::new();
    for row in matrix.records() {
        if row["export"] == "yes"
            && row["notes"]
                .split_once("Export: ")
                .is_none_or(|(_, limits)| limits.trim().is_empty())
        {
            errors.push(format!(
                "{}: qualify the supported export forms",
                row["kind"]
            ));
        }
        let kind = match row["kind"] {
            // View settings round-trip in `archive_preserves_view_kernel_settings_and_application_edits`.
            "Axis" => continue,
            "Formula" => "Text",
            _ => row["kernel_kind"],
        };
        let Some(&own_data) = independent.get(kind) else {
            if row["export"] != "no" {
                errors.push(format!("{kind}: no defined example is exported"));
            }
            continue;
        };
        let proven = proof.get(kind);
        let mut reasons = Vec::new();
        if !own_data {
            reasons.push(definition_limit(kind).to_owned());
        }
        let mut properties = kind_properties(kind);
        properties.extend(
            EXPORT_LIMIT_WITNESSES
                .iter()
                .filter_map(|&(_, _, owner, property)| (owner == kind).then_some(property)),
        );
        let missing: Vec<_> = properties
            .iter()
            .copied()
            .filter(|property| proven.is_none_or(|proven| !proven.contains_key(*property)))
            .collect();
        if !missing.is_empty() {
            reasons.push(format!("no witness for {}", missing.join(" and ")));
        }
        reasons.extend(
            PROPERTY_LIMITS
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
            properties.len() - missing.len(),
            properties.len()
        );
        if row["export"] != cell {
            errors.push(format!(
                "{kind}: export is `{}`, derived `{cell}`. {note}",
                row["export"]
            ));
        } else if !note.is_empty() && !row["notes"].ends_with(&note) {
            errors.push(format!("{kind}: the note must end with `{note}`"));
        }
    }
    assert!(
        errors.is_empty(),
        "docs/kinds.csv export column:\n{}",
        errors.join("\n")
    );
}

const ELEMENT_TAGS: &[&str] = &[
    "show",
    "objColor",
    "lineStyle",
    "pointSize",
    "pointStyle",
    "labelMode",
    "labelOffset",
    "caption",
    "dynamicCaption",
    "tooltipMode",
    "fixed",
    "auxiliary",
    "layer",
    "animation",
    "slider",
    "condition",
    "decoration",
    "angleStyle",
    "allowReflexAngle",
    "forceReflexAngle",
    "emphasizeRightAngle",
    "arcSize",
    "eqnStyle",
    "coords",
    "coordStyle",
    "value",
    "matrix",
    "eigenvectors",
    "coefficients",
    "startPoint",
    "curveParam",
    "trace",
    "spreadsheetTrace",
    "selectionAllowed",
    "breakpoint",
    "font",
    "decimals",
    "significantfigures",
    "isLaTeX",
    "textAlign",
    "verticalAlign",
    "userinput",
    "tempUserInput",
    "content",
    "contentSerif",
    "contentSize",
    "dimensions",
    "fixedSize",
    "checkbox",
    "comboBox",
    "listType",
    "selectedIndex",
    "length",
    "incrementY",
    "absoluteScreenLocation",
    "inBackground",
    "centered",
    "interpolate",
    "isMask",
    "cropBox",
    "fading",
    "bgColor",
    "borderColor",
    "boundingBox",
    "file",
    "audio",
    "video",
    "embed",
    "embedSettings",
    "linkedGeo",
    "parent",
    "parentLabel",
    "ordering",
    "algebra",
    "tableview",
    "keepTypeOnTransform",
    "outlyingIntersections",
    "showTrimmed",
    "showOnAxis",
    "symbolic",
    "simplifyCoefficients",
    "slopeTriangleSize",
    "startStyle",
    "endStyle",
    "headStyle",
    "levelOfDetailQuality",
    "strokeCoords",
    "strokeBezierCoords",
    "variables",
    "autocolor",
    "tag",
    "tags",
    "casMap",
    "listener",
    "ggbscript",
    "javascript",
];

/// Inventory rows with limits documented under Construction export.
const UNWITNESSED_ELEMENT_TAGS: &[&str] = &[
    "allowReflexAngle",
    "audio",
    "autocolor",
    "borderColor",
    "boundingBox",
    "casMap",
    "content",
    "contentSize",
    "embed",
    "embedSettings",
    "fading",
    "forceReflexAngle",
    "linkedGeo",
    "listener",
    "parent",
    "spreadsheetTrace",
    "strokeCoords",
    "variables",
    "verticalAlign",
    "video",
];

#[test]
fn element_tag_proof_accounts_for_the_inventory() {
    let proof: Value = serde_json::from_str(&fs::read_to_string(proof_path()).unwrap()).unwrap();
    let witnesses = proof["ElementTags"].as_object().unwrap();
    let all: BTreeSet<_> = ELEMENT_TAGS.iter().copied().collect();
    let documented: BTreeSet<_> = UNWITNESSED_ELEMENT_TAGS.iter().copied().collect();
    let proven: BTreeSet<_> = witnesses.keys().map(String::as_str).collect();
    assert_eq!(all.len(), 96);
    assert!(proven.is_disjoint(&documented));
    assert_eq!(
        proven.union(&documented).copied().collect::<BTreeSet<_>>(),
        all
    );
    for witness in witnesses.values() {
        assert!(witness["fixture"].is_string() && witness["label"].is_string());
        for stage in ["source", "export", "reload"] {
            assert!(witness[stage].is_array(), "{stage}: {witness}");
        }
    }
    println!("{} of {} element tags witnessed", proven.len(), all.len());
}

/// Non-default values set by the saved-file generator. A tag already
/// witnessed on a different kind must still survive on these objects.
const PROPERTY_CASES: &[(&str, &str, &[&str])] = &[
    (
        "saved-element-properties-001",
        "field",
        &[
            "font",
            "symbolic",
            "contentSerif",
            "textAlign",
            "tempUserInput",
            "length",
            "decimals",
        ],
    ),
    (
        "saved-element-properties-001",
        "b",
        &["font", "dimensions", "labelOffset"],
    ),
    (
        "saved-element-properties-002",
        "L",
        &["comboBox", "selectedIndex", "font", "decimals"],
    ),
    ("saved-element-properties-002", "M", &["listType"]),
    (
        "saved-element-properties-003",
        "A",
        &["coordStyle", "incrementY"],
    ),
    (
        "saved-element-properties-003",
        "v",
        &["coordStyle", "startPoint"],
    ),
    (
        "saved-element-properties-003",
        "s",
        &["outlyingIntersections", "keepTypeOnTransform"],
    ),
    ("saved-element-properties-003", "p", &["isMask"]),
    ("saved-element-properties-003", "n", &["slopeTriangleSize"]),
    (
        "saved-element-properties-004",
        "f",
        &[
            "tableview",
            "showOnAxis",
            "simplifyCoefficients",
            "lineStyle",
        ],
    ),
    (
        "saved-element-properties-004",
        "t",
        &["symbolic", "font", "isLaTeX", "startPoint"],
    ),
    ("saved-element-properties-005", "pie", &["tags/tag"]),
    (
        "saved-element-properties-extra-001",
        "h",
        &["levelOfDetailQuality"],
    ),
    ("saved-element-properties-extra-001", "s", &["parentLabel"]),
    (
        "saved-element-raster-properties-001",
        "anchoredImage",
        &["file", "interpolate", "cropBox"],
    ),
];

fn record_tag_witnesses(
    proof: &mut BTreeMap<&str, Value>,
    fixture: &str,
    source: &Value,
    export: &Value,
) {
    let witness = |label: &str, name: &str| {
        let original = &source["elementTags"][label][name];
        let written = &export["elementTags"][label][name];
        let reference = &source["reloadedTags"][label][name];
        let reloaded = &export["reloadedTags"][label][name];
        // The archive writer allocates its own names for raster assets.
        let hash = &source["rasterHashes"][label];
        let raster = name == "file" && hash.is_string() && hash == &export["rasterHashes"][label];
        if original.is_null()
            || written.is_null()
            || reloaded.is_null()
            || !(reference == reloaded || raster)
        {
            return None;
        }
        let mut value = json!({
            "fixture": fixture, "label": label, "source": original,
            "export": written, "reload": reloaded,
        });
        if raster {
            value["sha256"] = hash.clone();
        }
        Some(value)
    };

    for &(case, label, tags) in PROPERTY_CASES
        .iter()
        .filter(|(case, _, _)| *case == fixture)
    {
        for &tag in tags {
            if tag == "tempUserInput" {
                // The reference clears this linked field's pending input on
                // reload of its own file. Preserve both saved attributes.
                let input = &source["elementTags"][label][tag];
                assert!(input[0]["eval"].is_string() && input[0]["display"].is_string());
                assert_eq!(export["elementTags"][label][tag], *input);
                assert!(source["reloadedTags"][label][tag].is_null());
                assert!(export["reloadedTags"][label][tag].is_null());
            } else {
                assert!(
                    witness(label, tag).is_some(),
                    "{case}: {label}.{tag} changed across the reference reload"
                );
            }
        }
    }

    for (label, tags) in source["elementTags"].as_object().unwrap() {
        for name in tags.as_object().unwrap().keys() {
            let tag = if name == "tags/tag" { "tag" } else { name };
            let Some(&tag) = ELEMENT_TAGS.iter().find(|&&key| key == tag) else {
                continue;
            };
            let std::collections::btree_map::Entry::Vacant(entry) = proof.entry(tag) else {
                continue;
            };
            if let Some(value) = witness(label, name) {
                entry.insert(value);
            }
        }
    }
}

const EXPORT_LIMIT_WITNESSES: &[(&str, &str, &str, &str)] = &[
    ("saved-export-limits-001", "P", "Point", "pointSize"),
    ("saved-export-limits-001", "P", "Point", "incrementY"),
    ("saved-export-limits-002", "v", "Vector", "startPoint"),
    ("saved-export-limits-003", "bars", "Number", "barImage"),
    ("saved-export-limits-003", "pie", "PieChart", "barImage"),
];

fn record_export_limit_witnesses(
    proof: &mut BTreeMap<&str, BTreeMap<&str, Value>>,
    failures: &mut Vec<String>,
    fixture: &str,
    source: &Value,
    export: &Value,
) {
    for &(_, label, kind, property) in EXPORT_LIMIT_WITNESSES
        .iter()
        .filter(|&&(case, ..)| case == fixture)
    {
        let mut hash = Value::Null;
        let chart_tag = |snapshot: &Value, stage: &str, key: &str| {
            snapshot[stage][label]["tags/tag"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|tag| tag["key"] == key && tag["barNumber"] == "2")
                .cloned()
                .unwrap_or(Value::Null)
        };
        let tag = |snapshot: &Value, stage: &str| {
            let tags = &snapshot[stage][label];
            if property == "barImage" {
                chart_tag(snapshot, stage, property)
            } else {
                tags[property].clone()
            }
        };
        let saved = tag(source, "elementTags");
        let written = tag(export, "elementTags");
        let reference = tag(source, "reloadedTags");
        let reloaded = tag(export, "reloadedTags");
        let evaluated = match property {
            "incrementY" => Some("pointIncrements"),
            "startPoint" => Some("vectorAnchors"),
            _ => None,
        };
        let agrees = if property == "barImage" {
            hash = source["chartRasterHashes"][label]["2"].clone();
            let fill = chart_tag(source, "reloadedTags", "barFillType");
            !fill.is_null()
                && fill == chart_tag(export, "reloadedTags", "barFillType")
                && hash.is_string()
                && hash == export["chartRasterHashes"][label]["2"]
                && reloaded["value"] == written["value"]
        } else {
            reference == reloaded
                && evaluated.is_none_or(|key| {
                    !source[key][label].is_null() && source[key][label] == export[key][label]
                })
        };
        assert!(
            !saved.is_null() && !reference.is_null(),
            "{fixture}: missing reference {label}.{property}"
        );
        if !agrees || written.is_null() || reloaded.is_null() {
            failures.push(format!(
                "{fixture}: {label}.{property} changed: {reference} -> {reloaded}"
            ));
            continue;
        }
        let mut witness = json!({
            "fixture": fixture, "label": label, "source": saved,
            "export": written, "reload": reloaded,
        });
        if !hash.is_null() {
            witness["sha256"] = hash;
        }
        if let Some(key) = evaluated {
            witness["value"] = export[key][label].clone();
        }
        proof.entry(kind).or_default().insert(property, witness);
    }
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn geogebra_reloads_point_increments_vector_anchors_and_chart_images() {
    let mut failures = Vec::new();
    let mut proof = BTreeMap::new();
    for file in SAVED_CASES
        .iter()
        .filter(|file| file.starts_with("saved-export-limits-"))
    {
        let input = saved_review_file(file);
        let wb = review_workbench(&input);
        let bytes = wb.export_ggb(wb.file_view()).unwrap();
        let (source, export) = review_oracle(&input, &bytes);
        record_export_limit_witnesses(
            &mut proof,
            &mut failures,
            file.split_once('.').unwrap().0,
            &source,
            &export,
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

const LIST_POINT_INCREMENTS: &str = "saved-export-point-increments-001.xml";

#[test]
fn point_list_increments_keep_the_reference_spelling() {
    let wb = review_workbench(&saved_review_file(LIST_POINT_INCREMENTS));
    let xml = exported_xml(&wb);
    assert_eq!(xml.matches("<incrementY ").count(), 3, "{xml}");
    for value in ["Length[L]", "Length[L] + 1", "Sum[L]"] {
        assert!(
            xml.contains(&format!(r#"<incrementY val="{value}"/>"#)),
            "{xml}"
        );
    }
}

#[test]
fn point_list_increment_length_survives_an_object_named_length() {
    let shadow = r#"<element type="numeric" label="Length"><value val="7"/></element>"#;
    for label in ["L", "L_{1}"] {
        let increment = format!(r#"<incrementY val="Length[{label}]"/>"#);
        let document = |extra: &str| {
            format!(
                r#"<geogebra><construction>
                  <expression label="{label}" exp="{{2, 4, 8}}"/>
                  <element type="list" label="{label}"/>
                  {extra}
                  <element type="point" label="P"><coords x="0" y="1" z="1"/>{increment}</element>
                </construction></geogebra>"#
            )
        };

        let loaded_with_shadow = review_workbench(document(shadow).as_bytes());
        assert!(loaded_with_shadow.construction().id("Length").is_some());
        assert!(
            exported_xml(&loaded_with_shadow).contains(&increment),
            "{label}"
        );

        let mut edited = review_workbench(document("").as_bytes());
        assert!(exported_xml(&edited).contains(&increment), "{label}");
        edited.run_line("Length = 7").unwrap();
        assert!(edited.construction().id("Length").is_some());
        assert!(exported_xml(&edited).contains(&increment), "{label}");
    }
}

#[test]
fn point_list_increment_length_alias_uses_canonical_spelling_when_it_fits() {
    let input = br#"<geogebra><construction>
      <expression label="L_{1}" exp="{2, 4, 8}"/>
      <element type="list" label="L_{1}"/>
      <element type="point" label="P"><coords x="0" y="1" z="1"/><incrementY val="Length[L_1]"/></element>
    </construction></geogebra>"#;
    let wb = review_workbench(input);
    assert!(exported_xml(&wb).contains(r#"<incrementY val="Length[L_{1}]"/>"#));
}

#[test]
fn point_list_increment_length_alias_round_trips_at_the_attribute_limit() {
    use geo_import::xml::MAX_ATTRIBUTE_BYTES;

    let base = "l".repeat(MAX_ATTRIBUTE_BYTES - "Length[_1]".len());
    let label = format!("{base}_{{1}}");
    let value = format!("Length[{base}_1]");
    assert_eq!(value.len(), MAX_ATTRIBUTE_BYTES);
    assert!(label.len() < MAX_ATTRIBUTE_BYTES);
    assert_eq!("Length[]".len() + label.len(), MAX_ATTRIBUTE_BYTES + 2);

    // The list definition and element both fit the XML attribute cap.
    // Its short alias is one identifier token and the increment fits too.
    let input = format!(
        r#"<geogebra><construction>
          <expression label="{label}" exp="{{2, 4, 8}}"/>
          <element type="list" label="{label}"/>
          <element type="point" label="P"><coords x="0" y="1" z="1"/><incrementY val="{value}"/></element>
        </construction></geogebra>"#
    );
    let wb = review_workbench(input.as_bytes());
    let xml = exported_xml(&wb);
    let increment = format!(r#"<incrementY val="{value}"/>"#);
    assert!(xml.contains(&increment));

    let reloaded = review_workbench(xml.as_bytes());
    let c = reloaded.construction();
    assert_eq!(c.kind(c.id(&label).unwrap()), Kind::List);
    assert!(exported_xml(&reloaded).contains(&increment));
}

#[test]
fn bracketed_segment_length_still_exports_in_point_arithmetic() {
    let wb = review_workbench(
        br#"<geogebra><construction>
      <element type="point" label="A"><coords x="0" y="0" z="1"/></element>
      <element type="point" label="B"><coords x="3" y="4" z="1"/></element>
      <command name="Segment"><input a0="A" a1="B"/><output a0="s"/></command>
      <element type="segment" label="s"/>
      <element type="point" label="P"><coords x="0" y="1" z="1"/>
        <incrementY val="Length[s] + 1"/>
      </element>
    </construction></geogebra>"#,
    );
    assert_eq!(
        wb.construction().kind(wb.construction().id("s").unwrap()),
        Kind::Segment
    );
    assert!(exported_xml(&wb).contains(r#"<incrementY val="Length(s) + 1"/>"#));
}

const SHADOWED_POINT_INCREMENTS: &str = "saved-export-point-increment-shadow-001.xml";
const SHADOWED_LIST_LENGTHS: [(&str, &str, f64); 4] = [
    ("P", "Length[L]", 3.0),
    ("Q", "Length[L] + 1", 4.0),
    ("R", "Length[L_{1}]", 3.0),
    ("S", "Length[L] + Length[L_{1}]", 6.0),
];

#[test]
fn point_list_length_arithmetic_survives_an_object_named_length() {
    let wb = review_workbench(&saved_review_file(SHADOWED_POINT_INCREMENTS));
    assert!(wb.construction().id("Length").is_some());
    let xml = exported_xml(&wb);
    for (label, value, _) in SHADOWED_LIST_LENGTHS {
        assert!(
            xml.contains(&format!(r#"<incrementY val="{value}"/>"#)),
            "{label}: {value} was dropped"
        );
    }
}

#[test]
fn point_list_length_arithmetic_survives_a_later_object_named_length() {
    let input = review_xml(
        SHADOWED_POINT_INCREMENTS,
        &[(r#"label="Length""#, r#"label="initial""#)],
    );
    let mut wb = review_workbench(&input);
    assert_eq!(exported_xml(&wb).matches("<incrementY ").count(), 4);
    wb.run_line("Length = 7").unwrap();
    assert_eq!(exported_xml(&wb).matches("<incrementY ").count(), 4);

    assert!(wb.delete(wb.construction().id("L_{1}").unwrap()));
    let xml = exported_xml(&wb);
    assert_eq!(xml.matches("<incrementY ").count(), 2);
    assert!(xml.contains(r#"<incrementY val="Length[L] + 1"/>"#));
    wb.run_line("L_{1} = (3, 4)").unwrap();
    assert_eq!(exported_xml(&wb).matches("<incrementY ").count(), 2);

    assert!(wb.undo());
    assert!(wb.undo());
    assert_eq!(exported_xml(&wb).matches("<incrementY ").count(), 4);
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn geogebra_reloads_shadowed_point_list_lengths() {
    let input = saved_review_file(SHADOWED_POINT_INCREMENTS);
    let wb = review_workbench(&input);
    let (source, exported) = review_oracle(&input, &wb.export_ggb(None).unwrap());
    for (label, value, expected) in SHADOWED_LIST_LENGTHS {
        for snapshot in [&source, &exported] {
            let actual = snapshot["pointIncrements"][label]
                .as_f64()
                .unwrap_or_else(|| panic!("{label}: missing reloaded increment"));
            assert!(numbers_agree(expected, actual), "{label}: {actual}");
            for tags in ["elementTags", "reloadedTags"] {
                assert_eq!(
                    snapshot[tags][label]["incrementY"][0]["val"], value,
                    "{label}"
                );
            }
        }
    }
}

const POINT_SUM_INCREMENT: &str = r#"<incrementY val="Sum[L]"/>"#;

#[test]
fn point_list_increment_sum_survives_an_object_named_sum() {
    let input = review_xml(
        LIST_POINT_INCREMENTS,
        &[(
            r#"<expression label="L" exp="{2, 4, 8}"/>"#,
            concat!(
                r#"<element type="numeric" label="Sum"><value val="7"/></element>"#,
                r#"<expression label="L" exp="{2, 4, 8}"/>"#,
            ),
        )],
    );
    let wb = review_workbench(&input);
    let c = wb.construction();
    assert_eq!(c.kind(c.id("Sum").unwrap()), Kind::Number);

    let xml = exported_xml(&wb);
    assert!(xml.contains(POINT_SUM_INCREMENT), "{xml}");
    let reloaded = review_workbench(xml.as_bytes());
    assert!(exported_xml(&reloaded).contains(POINT_SUM_INCREMENT));
}

#[test]
fn point_list_increment_sum_survives_a_later_object_named_sum() {
    let mut wb = review_workbench(&saved_review_file(LIST_POINT_INCREMENTS));
    assert!(exported_xml(&wb).contains(POINT_SUM_INCREMENT));

    wb.run_line("Sum = 7").unwrap();
    assert!(exported_xml(&wb).contains(POINT_SUM_INCREMENT));
    assert_eq!(wb.construction().arithmetic_value("Sum(2)"), Some(14.0));

    assert!(wb.delete(wb.construction().id("L").unwrap()));
    assert!(!exported_xml(&wb).contains(POINT_SUM_INCREMENT));
    wb.run_line("L = 2").unwrap();
    assert!(!exported_xml(&wb).contains(POINT_SUM_INCREMENT));

    assert!(wb.undo());
    assert!(wb.undo());
    assert!(exported_xml(&wb).contains(POINT_SUM_INCREMENT));
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn geogebra_reloads_shadowed_point_list_sum() {
    let input = saved_review_file("saved-export-point-increment-sum-shadow-001.xml");
    let wb = review_workbench(&input);
    let (source, exported) = review_oracle(&input, &wb.export_ggb(None).unwrap());
    for snapshot in [&source, &exported] {
        assert_eq!(snapshot["pointIncrements"]["P"].as_f64(), Some(14.0));
        for tags in ["elementTags", "reloadedTags"] {
            assert_eq!(snapshot[tags]["P"]["incrementY"][0]["val"], "Sum[L]");
        }
    }
}

#[test]
fn point_list_increments_require_the_list_at_export() {
    let mut wb = review_workbench(&saved_review_file(LIST_POINT_INCREMENTS));
    assert_eq!(exported_xml(&wb).matches("<incrementY ").count(), 3);

    let list = wb.construction().id("L").unwrap();
    assert!(wb.delete(list));
    for label in ["P", "Q", "R"] {
        assert!(wb.construction().id(label).is_some(), "{label}");
    }
    assert!(!exported_xml(&wb).contains("<incrementY "));

    wb.run_line("L = (3, 4)").unwrap();
    let replacement = wb.construction().id("L").unwrap();
    assert_eq!(wb.construction().kind(replacement), Kind::Point);
    let xml = exported_xml(&wb);
    assert!(!xml.contains("<incrementY "), "{xml}");

    assert!(wb.undo());
    assert!(wb.undo());
    assert_eq!(exported_xml(&wb).matches("<incrementY ").count(), 3);
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn geogebra_reloads_point_list_increments() {
    let input = saved_review_file(LIST_POINT_INCREMENTS);
    let wb = review_workbench(&input);
    let (source, exported) = review_oracle(&input, &wb.export_ggb(None).unwrap());
    for (label, expected) in [("P", 3.0), ("Q", 4.0), ("R", 14.0)] {
        for snapshot in [&source, &exported] {
            let value = snapshot["pointIncrements"][label]
                .as_f64()
                .unwrap_or_else(|| panic!("{label}: missing reloaded increment"));
            assert!(numbers_agree(expected, value), "{label}: {value}");
        }
        assert_eq!(
            source["elementTags"][label]["incrementY"],
            exported["elementTags"][label]["incrementY"],
            "{label}: saved XML"
        );
        assert_eq!(
            source["reloadedTags"][label]["incrementY"],
            exported["reloadedTags"][label]["incrementY"],
            "{label}"
        );
    }
}

fn saved_review_file(file: &str) -> Vec<u8> {
    fs::read(repo_root().join("golden/ggb/saved").join(file)).unwrap()
}

fn review_xml(file: &str, changes: &[(&str, &str)]) -> Vec<u8> {
    let mut xml = String::from_utf8(saved_review_file(file)).unwrap();
    for &(from, to) in changes {
        assert!(xml.contains(from), "{file}: missing {from}");
        xml = xml.replacen(from, to, 1);
    }
    xml.into_bytes()
}

fn review_workbench(input: &[u8]) -> Workbench {
    let mut wb = Workbench::default();
    wb.load_for_export(input, "property review").unwrap();
    wb
}

fn review_oracle(source: &[u8], exported: &[u8]) -> (Value, Value) {
    review_oracle_with(source, exported, &["--element-tags"])
}

fn review_oracle_with(source: &[u8], exported: &[u8], args: &[&str]) -> (Value, Value) {
    let scratch = Scratch::new("property-review");
    for (name, bytes) in [("source", source), ("export", exported)] {
        let bytes = if geo_import::archive::is_zip(bytes) {
            bytes.to_vec()
        } else {
            archive(bytes, None)
        };
        fs::write(scratch.0.join(format!("{name}.ggb")), bytes).unwrap();
    }
    let result = oracle_command(args, &scratch.0).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let read = |name| {
        serde_json::from_slice(&fs::read(scratch.0.join(format!("{name}.ggb.snapshot"))).unwrap())
            .unwrap()
    };
    (read("source"), read("export"))
}

fn compare_saved_regression(file: &str, script: &str, labels: &[&str]) {
    let live = Workbench::from_script(script).unwrap();
    let archive = live
        .export_ggb(None)
        .unwrap_or_else(|e| panic!("{file}: {e}"));
    let source = review_workbench(&saved_review_file(file));
    let restored = review_workbench(&archive);
    for label in labels {
        let value = |wb: &Workbench| {
            let c = wb.construction();
            c.value(c.id(label).unwrap())
        };
        assert_eq!(value(&source), value(&live), "{file}: {label}");
        assert_eq!(value(&restored), value(&source), "{file}: {label}");
    }
}

#[test]
fn a_spreadsheet_fill_can_be_saved_and_reopened() {
    compare_saved_regression("saved-export-fill-row-001.xml", "FillRow(1,{7})", &["A1"]);
}

const EDITED_FILL_FILE: &str = "saved-export-edited-fill-001.xml";
const EDITED_FILL_SCRIPT: &str = "FillRow(1,{7})\nSetValue(A1,9)";
const MOVED_FILL_FILE: &str = "saved-export-moved-fill-001.xml";
const MOVED_FILL_SCRIPT: &str = "FillRow(1,{(7,8)})\nSetCoords(A1,9,10)";
const RAY_AND_FUNCTION_FILE: &str = "saved-export-ray-and-function-001.xml";
const RAY_AND_FUNCTION_SCRIPT: &str = "A = (1,2)\nB = (3,4)\nr = Ray(A,B)\nf(x,y)=x+y";

#[test]
fn an_edited_spreadsheet_fill_can_be_saved_and_reopened() {
    compare_saved_regression(EDITED_FILL_FILE, EDITED_FILL_SCRIPT, &["A1"]);
}

#[test]
fn a_moved_spreadsheet_fill_can_be_saved_and_reopened() {
    compare_saved_regression(MOVED_FILL_FILE, MOVED_FILL_SCRIPT, &["A1"]);
}

#[test]
fn ray_and_multivariable_function_export_notes_follow_the_xml() {
    use geo_import::xml::{parse, Item};

    let live = Workbench::from_script(RAY_AND_FUNCTION_SCRIPT).unwrap();
    let output = live.export_ggb(None).unwrap();
    let source = saved_review_file(RAY_AND_FUNCTION_FILE);
    for bytes in [&source, &output] {
        let xml = geo_import::archive::extract_xml(bytes).unwrap();
        let document = parse(&xml).unwrap();
        assert!(document.items.iter().any(|item| matches!(
            item,
            Item::Command { name, inputs, outputs }
                if name == "Ray" && inputs == &["A", "B"] && outputs == &["r"]
        )));
        assert!(document.items.iter().any(|item| matches!(
            item,
            Item::Element(el)
                if el.label == "r" && el.kind == "ray" && el.coords.is_some() && !el.start_point
        )));
        assert!(document.items.iter().any(|item| matches!(
            item,
            Item::Element(el) if el.label == "f" && el.kind == "functionnvar"
        )));
        assert!(xml
            .lines()
            .any(|line| line.starts_with("<expression label=\"f\" ")
                && line.contains("type=\"function\"")));
    }
    let table = fs::read_to_string(repo_root().join("docs/kinds.csv")).unwrap();
    let matrix = geo_golden::table::Table::parse(&table, "docs/kinds.csv").unwrap();
    let mut missing = Vec::new();
    for (kind, clauses) in [
        (
            "Ray",
            [
                "command carries the start",
                "element carries the line coefficients",
            ],
        ),
        (
            "FunctionNVar",
            [
                "expression type is `function`",
                "element type is `functionnvar`",
            ],
        ),
    ] {
        let row = matrix
            .records()
            .into_iter()
            .find(|row| row["kind"] == kind)
            .unwrap();
        for clause in clauses {
            if !row["notes"].contains(clause) {
                missing.push(format!("{kind}: missing {clause}"));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn edited_fill_ray_and_multivariable_function_match_the_reference_reader() {
    for (file, script) in [
        (EDITED_FILL_FILE, EDITED_FILL_SCRIPT),
        (MOVED_FILL_FILE, MOVED_FILL_SCRIPT),
        (RAY_AND_FUNCTION_FILE, RAY_AND_FUNCTION_SCRIPT),
    ] {
        let live = Workbench::from_script(script).unwrap();
        let output = live.export_ggb(None).unwrap();
        let (source, exported) = review_oracle(&saved_review_file(file), &output);
        if file == EDITED_FILL_FILE {
            assert_eq!(source["objects"]["A1"]["value"], 9.0);
        }
        assert_json(&source["objects"], &exported["objects"], file);
    }
}

#[test]
fn an_inline_fixed_segment_can_be_saved_and_reopened() {
    compare_saved_regression(
        "saved-export-inline-fixed-segment-001.xml",
        "s = Segment((1,1),4)",
        &["A", "s"],
    );
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn fill_and_inline_fixed_segment_match_the_reference_reader() {
    for (file, script) in [
        ("saved-export-fill-row-001.xml", "FillRow(1,{7})"),
        (
            "saved-export-inline-fixed-segment-001.xml",
            "s = Segment((1,1),4)",
        ),
    ] {
        let live = Workbench::from_script(script).unwrap();
        let output = live.export_ggb(None).unwrap();
        let (source, exported) = review_oracle(&saved_review_file(file), &output);
        assert_json(&source["objects"], &exported["objects"], file);
    }
}

#[test]
fn stored_line_distances_import_at_the_reference_reload_values() {
    let root = repo_root();
    let recordings: BTreeMap<String, BTreeMap<String, f64>> = serde_json::from_str(
        &fs::read_to_string(root.join("golden/ggb/distance-reload.snapshot")).unwrap(),
    )
    .unwrap();
    for case in load_all(&root)
        .unwrap()
        .into_iter()
        .filter(|case| recordings.contains_key(&case.id))
    {
        let source = fs::read(
            root.join("golden/ggb")
                .join(&case.group)
                .join(format!("{}.xml", case.id)),
        )
        .unwrap();
        let wb = review_workbench(&source);
        let c = wb.construction();
        for (label, value) in &recordings[&case.id] {
            let geo_kernel::Value::Number(actual) = c.value(c.id(label).unwrap()) else {
                panic!("{}: {label} is not a distance", case.id);
            };
            assert!(numbers_agree(actual, *value), "{}: {label}", case.id);
        }
    }
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn line_distances_save_in_the_reference_argument_order() {
    let root = repo_root();
    let recordings: BTreeMap<String, BTreeMap<String, f64>> = serde_json::from_str(
        &fs::read_to_string(root.join("golden/ggb/distance-reload.snapshot")).unwrap(),
    )
    .unwrap();
    for case in load_all(&root)
        .unwrap()
        .into_iter()
        .filter(|case| recordings.contains_key(&case.id))
    {
        let live = Workbench::from_script(&case.script.join("\n")).unwrap();
        let exported = live.export_ggb(None).unwrap();
        let source = fs::read(
            root.join("golden/ggb")
                .join(&case.group)
                .join(format!("{}.xml", case.id)),
        )
        .unwrap();
        let (source, snapshot) = review_oracle(&source, &exported);
        let restored = review_workbench(&exported);
        for (label, value) in &recordings[&case.id] {
            let expected = expected_from_json(&source["objects"][label]).unwrap();
            assert!(numbers_agree(
                source["objects"][label]["value"].as_f64().unwrap(),
                *value
            ));
            assert_json(
                &source["objects"][label],
                &snapshot["objects"][label],
                &case.id,
            );
            let c = restored.construction();
            compare_construction_object(&expected, c, c.id(label).unwrap()).unwrap();
        }
    }
}

// GeoGebra appends three head vertices to each two-point shaft. The
// kernel stores the shaft; the archive preserves the separate arrow flag.
fn slope_field_shafts(reference: &Value) -> Expected {
    let mut expected = expected_from_json(reference).unwrap();
    let Some(ExpectedData::Locus(points)) = &mut expected.data else {
        panic!("the arrow witness must contain a defined slope field");
    };
    let (glyphs, remainder) = points.as_chunks::<5>();
    assert!(remainder.is_empty());
    let shafts = glyphs
        .iter()
        .flat_map(|glyph| {
            assert_eq!(glyph.map(|point| point.1), [false, true, false, true, true]);
            assert_eq!(
                glyph[1], glyph[3],
                "the head must meet the tip of the shaft"
            );
            [glyph[0], glyph[1]]
        })
        .collect();
    *points = shafts;
    expected
}

#[test]
fn slope_field_samples_match_the_recorded_arrows() {
    let witness = "saved-slope-arrows-001";
    let source: Value =
        serde_json::from_slice(&saved_review_file(&format!("{witness}.snapshot"))).unwrap();
    let live = review_workbench(&saved_review_file(&format!("{witness}.xml")));
    let saved = live.export_ggb(live.file_view()).unwrap();
    let restored = review_workbench(&saved);
    let expected = slope_field_shafts(&source["objects"]["field"]);
    for wb in [&live, &restored] {
        let c = wb.construction();
        compare_construction_object(&expected, c, c.id("field").unwrap())
            .unwrap_or_else(|why| panic!("{witness}: {why}"));
    }
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn ordered_angle_tags_survive_reference_reload() {
    for flag in ["allowReflexAngle", "forceReflexAngle"] {
        let before = r#"<angleStyle val="2"/>"#;
        let after = format!(r#"<{flag} val="false"/>{before}"#);
        let input = review_xml("saved-styles-003.xml", &[(before, &after)]);
        let wb = review_workbench(&input);
        let output = wb.export_ggb(None).unwrap();
        let (source, exported) = review_oracle(&input, &output);
        let expected = &source["objects"]["w"];
        assert_eq!(expected["value"], json!(7.0 * std::f64::consts::PI / 4.0));
        assert_json(expected, &exported["objects"]["w"], flag);
        assert_eq!(
            source["reloadedTags"]["w"]["angleStyle"], exported["reloadedTags"]["w"]["angleStyle"],
            "{flag}"
        );
        let xml = geo_import::archive::extract_xml(&output).unwrap();
        let angle = xml.split_once(r#"label="w">"#).unwrap().1;
        assert!(
            angle.find(&format!("<{flag} ")).unwrap() < angle.find("<angleStyle ").unwrap(),
            "{flag} must precede angleStyle"
        );
    }
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn final_legacy_flag_also_changes_the_live_angle() {
    let input = review_xml(
        "saved-styles-003.xml",
        &[(
            r#"<angleStyle val="2"/>"#,
            r#"<angleStyle val="2"/><allowReflexAngle val="false"/>"#,
        )],
    );
    let wb = review_workbench(&input);
    let (source, exported) = review_oracle(&input, &wb.export_ggb(None).unwrap());
    let angle = &source["objects"]["w"];
    assert_eq!(angle["value"], json!(std::f64::consts::FRAC_PI_4));
    assert_json(angle, &exported["objects"]["w"], "w");
    let c = wb.construction();
    compare_construction_object(&expected_from_json(angle).unwrap(), c, c.id("w").unwrap())
        .unwrap();
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn ordered_button_tags_survive_reference_reload() {
    let dimensions = r#"<dimensions width="120" height="42"/>"#;
    let input = review_xml(
        "saved-element-properties-001.xml",
        &[(
            dimensions,
            &format!(r#"{dimensions}<fixedSize val="false"/>"#),
        )],
    );
    let wb = review_workbench(&input);
    let output = wb.export_ggb(None).unwrap();
    let (source, exported) = review_oracle(&input, &output);
    assert!(source["reloadedTags"]["b"]["dimensions"].is_null());
    assert_eq!(
        source["reloadedTags"]["b"]["dimensions"],
        exported["reloadedTags"]["b"]["dimensions"]
    );
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn list_angle_presentation_survives_reference_reload() {
    let input = saved_review_file("saved-property-review-001.xml");
    let (source, exported) =
        review_oracle(&input, &review_workbench(&input).export_ggb(None).unwrap());
    assert_json(&source["objects"]["L"], &exported["objects"]["L"], "L");
    for tag in ["angleStyle", "emphasizeRightAngle"] {
        let expected = &source["reloadedTags"]["L"][tag];
        assert!(!expected.is_null());
        assert_eq!(*expected, exported["reloadedTags"]["L"][tag], "{tag}");
    }
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn bar_colors_survive_reference_reload() {
    let input = saved_review_file("saved-property-review-001.xml");
    let (source, exported) =
        review_oracle(&input, &review_workbench(&input).export_ggb(None).unwrap());
    let tags = &source["reloadedTags"]["chart"]["tags/tag"];
    assert_eq!(tags[0]["value"], "rgba(255,0,0,1.0)");
    assert_eq!(*tags, exported["reloadedTags"]["chart"]["tags/tag"]);
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn reused_point_label_does_not_reattach_vector() {
    let input = saved_review_file("saved-element-properties-003.xml");
    let mut wb = review_workbench(&input);
    assert!(wb.delete(wb.construction().id("A").unwrap()));
    wb.run_line("A = (30, 40)").unwrap();
    let reference = saved_review_file("saved-property-review-002.xml");
    let (source, exported) = review_oracle(&reference, &wb.export_ggb(None).unwrap());
    assert_json(&source["objects"]["v"], &exported["objects"]["v"], "v");
    assert!(source["reloadedTags"]["v"]["startPoint"].is_null());
    assert_eq!(
        source["reloadedTags"]["v"]["startPoint"],
        exported["reloadedTags"]["v"]["startPoint"]
    );
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn legacy_widget_font_multipliers_survive_reference_reload() {
    for (file, fonts, labels) in [
        (
            "saved-element-properties-001.xml",
            &[
                (r#"sizeM="1.5" size="6""#, r#"size="6""#),
                (r#"sizeM="1.25" size="3""#, r#"size="6""#),
            ][..],
            &["field", "b"][..],
        ),
        (
            "saved-element-properties-002.xml",
            &[(r#"sizeM="1.25" size="3""#, r#"size="6""#)][..],
            &["L"][..],
        ),
    ] {
        let mut changes = vec![(r#"<font size="12"/>"#, r#"<font size="24"/>"#)];
        changes.extend_from_slice(fonts);
        let input = review_xml(file, &changes);
        let (source, exported) =
            review_oracle(&input, &review_workbench(&input).export_ggb(None).unwrap());
        for label in labels {
            let expected = &source["reloadedTags"][label]["font"][0]["sizeM"];
            assert_eq!(*expected, "1.25", "{label}");
            assert_eq!(
                *expected, exported["reloadedTags"][label]["font"][0]["sizeM"],
                "{label}"
            );
        }
    }
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn archived_widget_icons_survive_reference_reload() {
    let input = saved_review_file("saved-property-review-003.ggb");
    let output = review_workbench(&input).export_ggb(None).unwrap();
    let (source, exported) = review_oracle(&input, &output);
    let entry = |bytes: &[u8], name: &str| {
        use std::io::Read;
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut data = Vec::new();
        zip.by_name(name).unwrap().read_to_end(&mut data).unwrap();
        data
    };
    for label in ["field", "b"] {
        assert_eq!(source["reloadedTags"][label]["file"][0]["name"], "tile.png");
        let name = exported["reloadedTags"][label]["file"][0]["name"]
            .as_str()
            .unwrap();
        assert_eq!(entry(&input, "tile.png"), entry(&output, name), "{label}");
    }
}

#[test]
fn widget_icons_obey_the_raster_entry_limit() {
    use std::io::{Cursor, Write};

    let source = saved_review_file("saved-property-review-003.ggb");
    let xml = geo_import::archive::extract_xml(&source).unwrap();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("geogebra.xml", options).unwrap();
    zip.write_all(xml.as_bytes()).unwrap();
    zip.start_file("tile.png", options).unwrap();
    let size = geo_kernel::kernel::image::MAX_ASSET_BYTES as usize;
    zip.write_all(&vec![0; size + 1]).unwrap();
    let oversized = zip.finish().unwrap().into_inner();
    let imported = geo_import::import_for_replay(&oversized).unwrap();
    for label in ["field", "b"] {
        assert!(imported.notes.iter().any(|note| {
            note.contains(&format!("icon of `{label}`")) && note.contains("too large")
        }));
    }
    assert!(!exported_xml(&review_workbench(&oversized)).contains("<file "));
}

#[test]
fn chart_images_obey_the_raster_entry_limit_and_report_missing_assets() {
    use std::io::{Cursor, Write};

    let source = saved_review_file("saved-export-limits-003.ggb");
    let xml = geo_import::archive::extract_xml(&source).unwrap();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("geogebra.xml", options).unwrap();
    zip.write_all(xml.as_bytes()).unwrap();
    zip.start_file("bar.png", options).unwrap();
    let size = geo_kernel::kernel::image::MAX_ASSET_BYTES as usize;
    zip.write_all(&vec![0; size + 1]).unwrap();
    let damaged = zip.finish().unwrap().into_inner();

    let imported = geo_import::import_for_replay(&damaged).unwrap();
    for (label, reason) in [("bars", "too large"), ("pie", "missing from the archive")] {
        assert!(imported.notes.iter().any(|note| {
            note.contains(&format!("chart image of `{label}`")) && note.contains(reason)
        }));
    }
    let wb = review_workbench(&damaged);
    let export = wb.export_ggb(wb.file_view()).unwrap();
    assert!(!geo_import::archive::extract_xml(&export)
        .unwrap()
        .contains(r#"key="barImage""#));
    assert_eq!(zip::ZipArchive::new(Cursor::new(export)).unwrap().len(), 1);
}

const RASTER_BUDGET_FILE: &str = "saved-export-raster-budget-001.ggb";

#[test]
fn save_rejects_images_that_together_would_evict_a_chart_texture_on_reload() {
    use geo_app_core::{app::App, editor::ImageRequest};
    use std::io::{Cursor, Read};

    let input = saved_review_file(RASTER_BUDGET_FILE);
    let mut source = zip::ZipArchive::new(Cursor::new(&input)).unwrap();
    let mut candidate = Vec::new();
    source
        .by_name("candidate.jpg")
        .unwrap()
        .read_to_end(&mut candidate)
        .unwrap();

    let mut app = App::new(review_workbench(&input), None);
    let before = app.wb.export_ggb(None).unwrap();
    let request = ImageRequest {
        anchor: None,
        at: (0.0, 0.0),
        scale: 50.0,
        xmax: 8.0,
        ymax: 6.0,
    };
    app.image_request = Some(request.clone());
    app.finish_image(candidate.clone()).unwrap();
    let err = app.wb.export_ggb(None).err().unwrap_or_else(|| {
        panic!("save accepted two 4096×4096 rasters that exceed the shared limit")
    });
    assert!(err.to_string().contains("memory limit"), "{err}");

    assert!(app.wb.undo());
    assert_eq!(app.wb.export_ggb(None).unwrap(), before);
    let mut image_only = App::new(Workbench::default(), None);
    image_only.image_request = Some(request);
    image_only.finish_image(candidate).unwrap();
    assert!(image_only.wb.export_ggb(None).is_ok());
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn geogebra_reloads_the_large_chart_texture() {
    let input = saved_review_file(RASTER_BUDGET_FILE);
    let output = review_workbench(&input).export_ggb(None).unwrap();
    let (source, exported) = review_oracle(&input, &output);
    let texture = &source["chartRasterHashes"]["bars"]["2"];
    assert!(texture.is_string());
    assert_eq!(exported["chartRasterHashes"]["bars"]["2"], *texture);
}

fn duplicate_chart_image_input() -> Vec<u8> {
    use std::io::{Cursor, Write};

    let input = saved_review_file("saved-export-limits-003.ggb");
    let xml = geo_import::archive::extract_xml(&input).unwrap();
    let first = r#"<tag key="barImage" barNumber="2" value="bar.png"/>"#;
    let last = r#"<tag key="barImage" barNumber="2" value="missing.png"/>"#;
    assert_eq!(xml.matches(first).count(), 1);
    let mut source = zip::ZipArchive::new(Cursor::new(&input)).unwrap();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..source.len() {
        let entry = source.by_index(index).unwrap();
        if entry.name().unwrap() == "geogebra.xml" {
            zip.start_file("geogebra.xml", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(
                xml.replacen(first, &format!("{first}\n{last}"), 1)
                    .as_bytes(),
            )
            .unwrap();
        } else {
            zip.raw_copy_file(entry).unwrap();
        }
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn an_unavailable_last_chart_image_does_not_restore_an_earlier_texture() {
    let input = duplicate_chart_image_input();
    let imported = geo_import::import_for_replay(&input).unwrap();
    assert!(imported
        .notes
        .iter()
        .any(|note| { note.contains("chart image of `bars`") && note.contains("missing.png") }));
    let xml = exported_xml(&review_workbench(&input));
    assert_eq!(
        xml.matches(r#"key="barImage""#).count(),
        1,
        "only the pie texture survives"
    );
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn geogebra_reloads_only_the_effective_chart_texture() {
    let input = duplicate_chart_image_input();
    let output = review_workbench(&input).export_ggb(None).unwrap();
    let (source, exported) = review_oracle_with(&input, &output, &["--lenient", "--element-tags"]);
    assert!(source["reported"].as_array().unwrap().iter().any(|note| {
        note.as_str()
            .is_some_and(|note| note.contains("missing raster missing.png"))
    }));
    assert!(source["reloadedTags"]["bars"]["tags/tag"]
        .as_array()
        .unwrap()
        .contains(&json!({"key": "barImage", "barNumber": "2", "value": "missing.png"})));
    assert!(source["chartRasterHashes"]["bars"]["2"].is_null());
    assert!(source["chartRasterHashes"]["pie"]["2"].is_string());
    assert_eq!(exported["chartRasterHashes"], source["chartRasterHashes"]);
    assert_eq!(exported["reported"], json!([]));
}

#[test]
fn deleting_an_input_detaches_a_dependent_vector_anchor() {
    let input = saved_review_file("saved-export-limits-002.xml");
    let mut wb = review_workbench(&input);
    assert!(exported_xml(&wb).contains("<startPoint exp="));
    let point = wb.construction().id("A").unwrap();
    assert!(wb.delete(point));
    assert!(!exported_xml(&wb).contains("<startPoint "));
    wb.run_line("A = (30, 40)").unwrap();
    assert!(!exported_xml(&wb).contains("<startPoint "));
}

fn explicit_plain_text_source() -> Vec<u8> {
    review_xml(
        "saved-text-equations-001.xml",
        &[(r#"<isLaTeX val="true"/>"#, r#"<isLaTeX val="false"/>"#)],
    )
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn latex_command_keeps_reference_mode_with_the_rust_override_tag() {
    let input = explicit_plain_text_source();
    let output = review_workbench(&input).export_ggb(None).unwrap();
    let (source, exported) = review_oracle(&input, &output);
    assert_eq!(
        exported["elementTags"]["pTex"]["isLaTeX"][0]["val"],
        "false"
    );
    let expected = &source["reloadedTags"]["pTex"]["isLaTeX"];
    assert_eq!(expected[0]["val"], "true");
    assert_eq!(*expected, exported["reloadedTags"]["pTex"]["isLaTeX"]);
}

#[test]
fn explicit_plain_text_override_survives_rust_reload() {
    let input = explicit_plain_text_source();
    let wb = review_workbench(&input);
    let c = wb.construction();
    assert!(!c.text_presentation(c.id("pTex").unwrap()).unwrap().latex);
    round_trip("plain text override", &wb, None);
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn malformed_chart_property_is_dropped_before_reference_reload() {
    let input = review_xml(
        "saved-property-review-001.xml",
        &[(
            r#"key="barFillType" barNumber="2" value="1""#,
            r#"key="barFillType" barNumber="2" value="invalid""#,
        )],
    );
    let scratch = Scratch::new("malformed-chart");
    let path = scratch.0.join("malformed.ggb");
    fs::write(&path, archive(&input, None)).unwrap();
    let result = lenient_java_snapshots(&scratch.0);
    assert!(result.status.success());
    let reference: Value =
        serde_json::from_slice(&fs::read(path.with_extension("ggb.snapshot")).unwrap()).unwrap();
    assert!(reference["failed"]
        .as_str()
        .unwrap()
        .contains("NumberFormatException"));
    let clean = saved_review_file("saved-property-review-001.xml");
    let (source, exported) =
        review_oracle(&clean, &review_workbench(&input).export_ggb(None).unwrap());
    let colors = |snapshot: &Value| {
        snapshot["reloadedTags"]["pie"]["tags/tag"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|tag| tag["key"] == "barColor")
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(colors(&source), colors(&exported));
    assert!(!exported["reloadedTags"]["pie"]["tags/tag"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tag| tag["key"] == "barFillType"));
}

fn recorded_legacy_xml(id: &str) -> String {
    let saved: Value =
        serde_json::from_str(include_str!("../../../tools/ggb-harness/saved/saved.json")).unwrap();
    saved["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .unwrap()["steps"][0]["xml"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn last_round_free_angle_value_survives_reference_reload() {
    let id = "saved-property-final-001";
    let input = recorded_legacy_xml(id);
    let wb = review_workbench(input.as_bytes());
    let output = wb.export_ggb(None).unwrap();
    let reference = saved_review_file(&format!("{id}.xml"));
    let (source, exported) = review_oracle(&reference, &output);
    let angle = &source["objects"]["w"];
    assert_json(angle, &exported["objects"]["w"], "free angle after save");
    let c = wb.construction();
    compare_construction_object(&expected_from_json(angle).unwrap(), c, c.id("w").unwrap())
        .unwrap();
    let reopened = review_workbench(&output);
    let c = reopened.construction();
    compare_construction_object(&expected_from_json(angle).unwrap(), c, c.id("w").unwrap())
        .unwrap();
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn last_round_free_angles_preserve_the_raw_sweep() {
    let id = "saved-property-final-006";
    let input = recorded_legacy_xml(id);
    let wb = review_workbench(input.as_bytes());
    let output = wb.export_ggb(None).unwrap();
    let reference = saved_review_file(&format!("{id}.xml"));
    let (source, exported) = review_oracle(&reference, &output);
    let reopened = review_workbench(&output);
    for label in ["r", "u", "w"] {
        let angle = &source["objects"][label];
        assert_json(angle, &exported["objects"][label], label);
        let expected = source["reloadedTags"][label]["value"][0]["val"]
            .as_str()
            .unwrap()
            .parse::<f64>()
            .unwrap();
        for stage in ["elementTags", "reloadedTags"] {
            let raw = exported[stage][label]["value"][0]["val"]
                .as_str()
                .unwrap()
                .parse::<f64>()
                .unwrap();
            assert_json(&json!(expected), &json!(raw), &format!("{label} {stage}"));
        }
        for c in [wb.construction(), reopened.construction()] {
            compare_construction_object(
                &expected_from_json(angle).unwrap(),
                c,
                c.id(label).unwrap(),
            )
            .unwrap();
        }
    }
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn last_round_undo_restores_vector_anchor_through_save() {
    let input = saved_review_file("saved-element-properties-003.xml");
    let mut wb = review_workbench(&input);
    assert!(wb.delete(wb.construction().id("A").unwrap()));
    assert!(wb.undo());
    let reference = saved_review_file("saved-property-final-002.xml");
    let (source, exported) = review_oracle(&reference, &wb.export_ggb(None).unwrap());
    assert_json(&source["objects"]["v"], &exported["objects"]["v"], "vector");
    let anchor = &source["reloadedTags"]["v"]["startPoint"];
    assert_eq!(anchor[0]["exp"], "A");
    assert_eq!(*anchor, exported["reloadedTags"]["v"]["startPoint"]);

    assert!(wb.redo());
    wb.run_line("A = (30, 40)").unwrap();
    let detached = saved_review_file("saved-property-review-002.xml");
    let (source, exported) = review_oracle(&detached, &wb.export_ggb(None).unwrap());
    assert!(source["reloadedTags"]["v"]["startPoint"].is_null());
    assert_eq!(
        source["reloadedTags"]["v"]["startPoint"],
        exported["reloadedTags"]["v"]["startPoint"]
    );
}

#[test]
fn last_round_generated_angle_style_is_charged_to_the_item_limit() {
    let at_limit = format!(
        r#"<geogebra><construction><element type="angle" label="w"><value val="1"/>{}</element></construction></geogebra>"#,
        r#"<coordStyle style="polar"/>"#.repeat(geo_import::xml::MAX_ITEMS - 1)
    );
    let wb = review_workbench(at_limit.as_bytes());
    match wb.export_ggb(None) {
        Err(error) => assert!(error.to_string().contains("import object limit"), "{error}"),
        Ok(output) => {
            geo_import::import(&output).expect("a successfully saved archive must reopen");
        }
    }
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn last_round_hex_bar_opacity_survives_reference_reload() {
    let file = "saved-property-final-003.xml";
    let input = review_xml(
        file,
        &[(
            r#"key="barAlpha" barNumber="2" value="0.5""#,
            r#"key="barAlpha" barNumber="2" value="0x1.0p-1""#,
        )],
    );
    let wb = review_workbench(&input);
    let reference = saved_review_file(file);
    let (source, exported) = review_oracle(&reference, &wb.export_ggb(None).unwrap());
    let tags = &source["reloadedTags"]["chart"]["tags/tag"];
    assert!(tags
        .as_array()
        .unwrap()
        .iter()
        .any(|tag| tag["key"] == "barAlpha" && tag["value"] == "0.5"));
    assert_eq!(*tags, exported["reloadedTags"]["chart"]["tags/tag"]);
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn last_round_legacy_positions_follow_live_control_moves() {
    let id = "saved-property-final-004";
    let input = recorded_legacy_xml(id);
    let mut wb = review_workbench(input.as_bytes());
    let reference = saved_review_file(&format!("{id}.xml"));
    let (source, exported) = review_oracle(&reference, &wb.export_ggb(None).unwrap());
    assert_eq!(source["styles"]["b"]["label_offset"], json!([90, 110]));
    assert_eq!(
        source["styles"]["n"]["slider"]["position"],
        json!([95.0, 115.0])
    );
    assert_eq!(
        source["styles"]["b"]["label_offset"],
        exported["styles"]["b"]["label_offset"]
    );
    assert_eq!(
        source["styles"]["n"]["slider"]["position"],
        exported["styles"]["n"]["slider"]["position"]
    );
    let check_box = wb.construction().id("b").unwrap();
    let slider = wb.construction().id("n").unwrap();
    assert_eq!(
        wb.construction().checkbox_position(check_box),
        Some((90.0, 110.0))
    );
    assert_eq!(
        wb.construction().slider_style(slider).position,
        Some((95.0, 115.0))
    );
    wb.move_checkbox(check_box, [130, 150]);
    wb.move_slider(slider, (135.0, 155.0));
    let (_, moved) = review_oracle(&reference, &wb.export_ggb(None).unwrap());
    assert_eq!(moved["styles"]["b"]["label_offset"], json!([130, 150]));
    assert_eq!(
        moved["styles"]["n"]["slider"]["position"],
        json!([135.0, 155.0])
    );
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn last_round_vertical_increment_uses_the_validated_expression() {
    let id = "saved-property-final-005";
    let input = recorded_legacy_xml(id);
    let wb = review_workbench(input.as_bytes());
    let output = wb.export_ggb(None).unwrap();
    let reference = saved_review_file(&format!("{id}.xml"));
    let (source, exported) = review_oracle(&reference, &output);
    assert_eq!(
        source["reloadedTags"]["P"]["incrementY"],
        exported["reloadedTags"]["P"]["incrementY"]
    );
    assert_eq!(
        exported["elementTags"]["P"]["incrementY"][0]["val"],
        "0.25 * 2"
    );
}

fn oracle_command(args: &[&str], dir: &std::path::Path) -> Command {
    let root = repo_root();
    let (script, path) = if std::env::var_os("GEOKERNEL_EXPORT_CONTAINER").is_some() {
        (
            "linux-x86_64.sh",
            PathBuf::from("/geokernel").join(dir.strip_prefix(&root).unwrap()),
        )
    } else {
        ("harness.sh", dir.to_owned())
    };
    let mut command = Command::new(root.join("tools/ggb-harness").join(script));
    command.arg("export-snapshot").args(args).arg(path);
    command
}

fn tag_snapshots(dir: &std::path::Path) -> std::process::Output {
    oracle_command(&["--element-tags"], dir)
        .output()
        .expect("run Java element tag oracle")
}

fn java_snapshots(dir: &std::path::Path) -> std::process::Output {
    oracle_command(&[], dir).output().expect("run Java harness")
}

/// Snapshots that record what the reader reports instead of stopping at it.
fn lenient_java_snapshots(dir: &std::path::Path) -> std::process::Output {
    oracle_command(&["--lenient"], dir)
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
                let command_options = wb
                    .definition(label)
                    .and_then(geo_kernel::script::split_call)
                    .filter(|(name, _)| *name == "Slider")
                    .map(|(_, args)| args);
                let option = |index| {
                    command_options
                        .as_ref()
                        .and_then(|args| args.get(index).copied())
                };
                let from_file = presentation.object_styles.contains_key(label);
                let step = match animation {
                    Some(animation) => {
                        number_or_reference(c, option(2).or(animation.step.as_deref()))
                    }
                    // A file slider without the tag steps automatically.
                    None if from_file && option(2).is_none() => Value::Null,
                    None => match c.slider_style(id).step.filter(|_| option(2).is_none()) {
                        Some(step) => json!(step),
                        None => number_or_reference(c, option(2).or(Some("0.1"))),
                    },
                };
                let speed = option(3)
                    .or_else(|| animation.and_then(|a| a.speed.as_deref()))
                    .or(Some("1"));
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
                    .map(|&(x, y)| [x, y])
                    .or_else(|| {
                        (!presentation.hidden.contains(label))
                            .then(|| c.checkbox_position(id).map(|(x, y)| [x as i32, y as i32]))
                            .flatten()
                    })
                    .unwrap_or([0, 0])
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
            "label_mode": i32::from(c.kind(id) == Kind::Number && c.slider_bounds(id) != [None, None]),
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
        let parent = if std::env::var_os("GEOKERNEL_EXPORT_CONTAINER").is_some() {
            repo_root().join("target")
        } else {
            std::env::temp_dir()
        };
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT_SCRATCH: AtomicUsize = AtomicUsize::new(0);
        let sequence = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            "geokernel-ggb-oracle-{case}-{}-{sequence}",
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

#[test]
fn normal_function_archive_keeps_the_default_cumulative_flag() {
    let wb = Workbench::from_script(
        "m = 2\ns = 3\nf = Normal(m,s,x)\ng = Normal(m,s,(( x )),false)\nh = Normal(m,s,(x))\na = f(-1)\nb = g(-1)\ni = h(-1)\nf2 = (Normal(m,s,x))\nf3 = 2*Normal(m,s,(x))\nu = f2(-1)\nv = f3(-1)\nf4 = If(x>0,Normal(0,1,(x)),0)\nw = f4(1)",
    )
    .unwrap();
    let xml = exported_xml(&wb);
    assert_eq!(
        xml.matches("a0=\"m\" a1=\"s\" a2=\"x\" a3=\"true\"")
            .count(),
        2
    );
    assert!(xml.contains("a0=\"m\" a1=\"s\" a2=\"x\" a3=\"false\""));
    assert!(xml.contains("(Normal(m,s,x, true))"));
    assert!(xml.contains("2*Normal(m,s,x, true)"));
    assert!(xml.contains("a1=\"Normal(0,1,x, true)\""));
    let (_, reopened) = round_trip("normal CDF and density", &wb, None);
    for label in ["a", "b", "i", "u", "v", "w"] {
        let value = |w: &Workbench| {
            let c = w.construction();
            c.value(c.id(label).unwrap())
        };
        assert_eq!(value(&reopened), value(&wb), "{label}");
    }
}

#[test]
fn reloaded_fit_lines_follow_point_edits() {
    let wb = Workbench::from_script(
        "A = (1, 2)\nB = (3, 4)\nC = (5, 1)\nL = {A, B, C}\ng = FitLine(L)\nh = FitLineX({A, B, C})",
    )
    .unwrap();
    let (_, mut reloaded) = round_trip("fit lines", &wb, None);
    let original = Workbench::from_script(
        "A = (1, 7)\nB = (3, 4)\nC = (5, 1)\nL = {A, B, C}\ng = FitLine(L)\nh = FitLineX({A, B, C})",
    )
    .unwrap();
    assert!(reloaded.move_point(reloaded.construction().id("A").unwrap(), 1.0, 7.0));
    for label in ["g", "h"] {
        let value = |w: &Workbench| {
            let c = w.construction();
            c.value(c.id(label).unwrap())
        };
        assert_ne!(value(&wb), value(&original));
        assert_eq!(value(&reloaded), value(&original), "{label}");
    }
}

#[test]
fn reloaded_line_calls_follow_point_edits() {
    let wb = Workbench::from_script(
        "A = (0, 1)\nB = (2, 5)\ng = Line(A, B)\na = g(3)\nf(x) = 1 + g(x)\nb = f(3)",
    )
    .unwrap();
    let (_, mut reloaded) = round_trip("a line applied as a function", &wb, None);
    let values = |w: &Workbench| {
        let c = w.construction();
        ["a", "b"].map(|label| c.value(c.id(label).unwrap()))
    };
    let point = reloaded.construction().id("B").unwrap();
    let number = geo_kernel::Value::Number;
    assert_eq!(values(&reloaded), [number(7.0), number(8.0)]);
    assert!(reloaded.move_point(point, 0.0, 5.0));
    assert!(values(&reloaded).iter().all(|v| !v.is_defined()));
    assert!(reloaded.move_point(point, 2.0, 3.0));
    assert_eq!(values(&reloaded), [number(4.0), number(5.0)]);
}

#[test]
fn reloaded_line_expansion_keeps_the_typed_equation() {
    let wb = Workbench::from_script("g: y = (x + 1)/3\nf(x) = g(x)\na = 1/(f(5)-2)").unwrap();
    let (_, reloaded) = round_trip("a line with a typed right-hand side", &wb, None);
    let construction = reloaded.construction();
    assert_eq!(
        construction.value(construction.id("a").unwrap()),
        geo_kernel::Value::Number(f64::INFINITY)
    );
}

#[test]
fn reloaded_typed_line_calls_keep_infinite_coefficients() {
    let wb = Workbench::from_script("n = 0\ng: y = 1/n\nh: y = x/n\na = g(0)\nb = h(1)").unwrap();
    let (_, mut reloaded) = round_trip("typed lines with infinite coefficients", &wb, None);
    let [n, a, b] = ["n", "a", "b"].map(|label| reloaded.construction().id(label).unwrap());
    let outputs = |w: &Workbench| [a, b].map(|id| w.construction().value(id));
    let number = geo_kernel::Value::Number;
    assert_eq!(outputs(&reloaded), [number(f64::INFINITY); 2]);
    assert!(reloaded.set_number(n, -2.0));
    assert_eq!(outputs(&reloaded), [number(-0.5); 2]);
    assert!(reloaded.set_number(n, 0.0));
    assert_eq!(outputs(&reloaded), [number(f64::INFINITY); 2]);
}

#[test]
fn fit_line_archive_checks_definedness_before_the_coefficient_tolerance() {
    for (x, defined, replacement) in [
        ("0.00007", false, "-1.02e-8"),
        ("0.0000715", true, "-9.8e-9"),
    ] {
        let wb = Workbench::from_script(&format!("g = FitLine({{(-{x},0),({x},0)}})")).unwrap();
        let (bytes, reloaded) = round_trip("fit near the definedness threshold", &wb, None);
        let construction = reloaded.construction();
        let value = construction.stored_value(construction.id("g").unwrap());
        assert_eq!(value.is_defined(), defined);
        let (geo_kernel::Value::Line { b, .. } | geo_kernel::Value::UndefinedLine { b, .. }) =
            value
        else {
            panic!("expected fit coefficients");
        };
        let xml = geo_import::archive::extract_xml(&bytes).unwrap();
        let coords = format!(r#"x="0" y="{b}" z="0""#);
        assert!(xml.contains(&coords));
        let changed = xml.replace(&coords, &format!(r#"x="0" y="{replacement}" z="0""#));
        let result = geo_import::import_for_replay(changed.as_bytes()).unwrap();
        assert_eq!(result.mismatched.len(), 1);
        assert_eq!(result.mismatched[0].label, "g");
    }
}

#[test]
fn reloaded_fit_lines_keep_their_infinite_intercepts() {
    let wb = Workbench::from_script(
        "t = 2^341\ng = FitLine({(0,4*t),(t,5*t)})\nh = FitLineX({(4*t,0),(5*t,t)})",
    )
    .unwrap();
    let (bytes, reloaded) = round_trip("fit lines with infinite intercepts", &wb, None);
    let c = reloaded.construction();
    for (label, a, b, equation) in [("g", 1.0, -1.0, "y = x + ∞"), ("h", -1.0, 1.0, "y = x - ∞")]
    {
        let id = c.id(label).unwrap();
        assert_eq!(geo_app_core::describe::object_text(c, id), equation);
        assert_eq!(
            c.value(id),
            geo_kernel::Value::Line {
                a,
                b,
                c: f64::INFINITY
            }
        );
    }
    let xml = geo_import::archive::extract_xml(&bytes).unwrap();
    let explicit = r#"<eqnStyle style="explicit"/>"#;
    assert_eq!(xml.matches(explicit).count(), 2);
    let implicit = r#"<eqnStyle style="implicit"/>"#;
    let restyled = xml.replacen(explicit, implicit, 1);
    let mut changed = Workbench::default();
    changed
        .load_for_export(restyled.as_bytes(), "restyled fit")
        .unwrap();
    let c = changed.construction();
    assert_eq!(
        c.equation_form(c.id("g").unwrap()),
        Some(&geo_kernel::kernel::EquationForm::Implicit)
    );
    let again = exported_xml(&changed);
    assert_eq!(again.matches(implicit).count(), 1);
    assert_eq!(again.matches(explicit).count(), 1);
    let rescaled = xml.replace(
        r#"x="1" y="-1" z="Infinity""#,
        r#"x="2" y="-2" z="Infinity""#,
    );
    assert!(geo_import::import_for_replay(rescaled.as_bytes())
        .unwrap()
        .mismatched
        .is_empty());
    let wrong_sign = xml.replace(r#"z="Infinity""#, r#"z="-Infinity""#);
    let imported = geo_import::import_for_replay(wrong_sign.as_bytes()).unwrap();
    let labels: Vec<_> = imported
        .mismatched
        .iter()
        .map(|m| m.label.as_str())
        .collect();
    assert_eq!(labels, ["g", "h"]);
    let wrong_normal = xml.replace(
        r#"x="1" y="-1" z="Infinity""#,
        r#"x="2" y="-1" z="Infinity""#,
    );
    let imported = geo_import::import_for_replay(wrong_normal.as_bytes()).unwrap();
    assert_eq!(imported.mismatched.len(), 1);
    assert_eq!(imported.mismatched[0].label, "g");
}

#[test]
fn archive_checks_infinite_components_of_fit_line_directions() {
    let wb = Workbench::from_script(
        "A = (1,-10^308)\nB = (2,10^308)\ng = FitLine({A,B})\nv = Direction(g)",
    )
    .unwrap();
    let (bytes, reloaded) = round_trip("infinite normal", &wb, None);
    let c = reloaded.construction();
    assert_eq!(
        c.value(c.id("v").unwrap()),
        geo_kernel::Value::Vector {
            x: -0.5,
            y: f64::NEG_INFINITY
        }
    );
    let xml = geo_import::archive::extract_xml(&bytes).unwrap();
    let wrong = xml.replace(
        r#"x="-0.5" y="-Infinity" z="0""#,
        r#"x="-0.5" y="Infinity" z="0""#,
    );
    let imported = geo_import::import_for_replay(wrong.as_bytes()).unwrap();
    assert_eq!(imported.mismatched.len(), 1);
    assert_eq!(imported.mismatched[0].label, "v");
}

#[test]
fn reloaded_undefined_fit_keeps_its_direction_through_edits() {
    let wb = Workbench::from_script(
        "n = 342\nt = 2^n\nL = {(t,t),(2*t,2*t)}\ng = FitLine(L)\nv = Direction(g)",
    )
    .unwrap();
    let (_, mut again) = round_trip("fit with undefined intercept", &wb, None);
    for n in [342, 343, 2] {
        let id = again.construction().id("n").unwrap();
        again.set_number(id, f64::from(n));
        let c = again.construction();
        assert_eq!(c.value(c.id("g").unwrap()).is_defined(), n == 2);
        let component = -2.0_f64.powi(2 * n - 1);
        assert_eq!(
            c.value(c.id("v").unwrap()),
            geo_kernel::Value::Vector {
                x: component,
                y: component
            }
        );
    }
}

#[test]
fn export_keeps_live_slider_endpoints_with_a_colon_label() {
    let mut wb = Workbench::default();
    wb.run_line("b = 36").unwrap();
    wb.run_line("a: Slider(0, If(b == 36, 6, 2))").unwrap();
    let xml = exported_xml(&wb);
    assert!(xml.contains("max=\"If(b == 36, 6, 2)\""), "{xml}");

    let mut reloaded = Workbench::default();
    reloaded.load_for_export(xml.as_bytes(), "slider").unwrap();
    let c = reloaded.construction();
    assert_eq!(c.slider_interval(c.id("a").unwrap()), Some((0.0, 6.0)));
}

#[test]
fn reloaded_slider_responds_to_an_edit_of_its_expression_endpoint() {
    let cases = load_all(&repo_root()).unwrap();
    let name = "optimizerreview-reload-001";
    let case = cases.iter().find(|case| case.id == name).unwrap();
    let mut wb = Workbench::from_script(&case.script.join("\n")).unwrap();
    let set = |wb: &mut Workbench, step: &Step| {
        let Step::SetNumber { label, value } = step else {
            panic!("expected a numeric edit");
        };
        assert!(wb.set_number(wb.construction().id(label).unwrap(), *value));
    };
    set(&mut wb, &case.steps[1]);
    let (_, mut reloaded) = round_trip(name, &wb, None);
    set(&mut reloaded, &case.steps[2]);
    let c = reloaded.construction();
    for (label, expected) in &case.snapshots[2] {
        compare_construction_object(expected, c, c.id(label).unwrap())
            .unwrap_or_else(|why| panic!("{label}: {why}"));
    }
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

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn geogebra_reloads_session_control_positions_and_values() {
    use geo_app_core::app::App;
    use geo_app_core::tools::Tool;

    let mut app = App::new(Workbench::default(), None);
    app.select_tool(Tool::Slider);
    app.tool.begin_control((124.0, 88.0));
    let slider = app.tool.finish_slider(-5.0, 5.0, 0.25, |_| false);
    app.apply_outcome(slider);
    let a = app.wb.construction().id("a").unwrap();
    app.wb.set_label_offset(a, (17.0, -8.0));
    app.select_tool(Tool::Checkbox);
    app.tool.begin_control((51.0, 73.0));
    let checkbox = app.tool.finish_checkbox("Show", |label| label == "a");
    app.apply_outcome(checkbox);
    let b = app.wb.construction().id("b").unwrap();
    assert!(app.wb.set_boolean(b, false));

    let dir = Scratch::new("session-controls");
    fs::write(dir.0.join("controls.ggb"), app.wb.export_ggb(None).unwrap()).unwrap();
    let output = java_snapshots(&dir.0);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snap: Value =
        serde_json::from_slice(&fs::read(dir.0.join("controls.ggb.snapshot")).unwrap()).unwrap();
    assert_eq!(
        snap["styles"]["a"]["slider"]["position"],
        json!([124.0, 88.0])
    );
    assert_eq!(snap["styles"]["a"]["slider"]["width"], json!(200.0));
    assert_eq!(snap["styles"]["a"]["label_mode"], json!(1));
    assert_eq!(snap["styles"]["a"]["label_offset"], json!([17, -8]));
    assert_eq!(snap["styles"]["a"]["animation"]["step"], json!(0.25));
    assert_eq!(snap["objects"]["a"]["value"], json!(1.0));
    assert_eq!(snap["styles"]["b"]["label_offset"], json!([51, 73]));
    assert_eq!(snap["styles"]["b"]["caption"], json!("Show"));
    assert_eq!(snap["objects"]["b"]["value"], json!(false));
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn geogebra_reloads_a_slider_with_an_undefined_endpoint() {
    let wb = Workbench::from_script("limit = 2\ns = Slider((?), limit, 0.5)").unwrap();
    let archive = wb.export_ggb(None).unwrap();
    let xml = geo_import::archive::extract_xml(&archive).unwrap();
    assert!(xml.contains(r#"min="NaN" max="limit""#));
    let dir = Scratch::new("undefined-slider");
    fs::write(dir.0.join("slider.ggb"), &archive).unwrap();
    let output = java_snapshots(&dir.0);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snap: Value =
        serde_json::from_slice(&fs::read(dir.0.join("slider.ggb.snapshot")).unwrap()).unwrap();
    let saved_slider = &snap["styles"]["s"]["slider"];
    assert_eq!(saved_slider["min"], Value::Null);
    assert_eq!(saved_slider["max"], json!(2.0));
    assert_eq!(saved_slider["position"], json!([30.0, 50.0]));
    assert_eq!(snap["styles"]["s"]["label_mode"], json!(1));
    assert_eq!(snap["styles"]["s"]["animation"]["step"], json!(0.5));
    assert_eq!(snap["objects"]["s"]["value"], json!(0.0));

    let mut reloaded = Workbench::default();
    reloaded.load_bytes(&archive, "slider.ggb").unwrap();
    let limit = reloaded.construction().id("limit").unwrap();
    let s = reloaded.construction().id("s").unwrap();
    assert!(reloaded.set_number(limit, 4.0));
    assert_eq!(reloaded.construction().slider_bounds(s), [None, Some(4.0)]);
}

const CAS_EXPORT_CASE: &str = "casview-rows-002";

fn cas_export_case() -> geo_golden::model::Case {
    load_all(&repo_root())
        .unwrap()
        .into_iter()
        .find(|case| case.id == CAS_EXPORT_CASE)
        .unwrap()
}

fn check_cas_export(wb: &Workbench, expected: &[geo_golden::model::CasViewRow]) {
    assert_eq!(wb.cas_rows().len(), expected.len());
    for (actual, expected) in wb.cas_rows().iter().zip(expected) {
        assert_eq!(actual.input, expected.input);
        assert_eq!(actual.output.text, expected.output);
        assert_eq!(actual.output.latex, expected.latex);
    }
}

#[test]
fn reopens_the_reference_cas_table_and_a_new_export() {
    let case = cas_export_case();
    let rows = &case.casview.as_ref().unwrap().last().unwrap().rows;
    let mut wb = Workbench::default();
    wb.load_for_export(
        include_bytes!("../../../golden/ggb/casview/casview-rows-002.xml"),
        CAS_EXPORT_CASE,
    )
    .unwrap();
    check_cas_export(&wb, rows);
    let export = wb.export_ggb(None).unwrap();
    wb.load_for_export(&export, CAS_EXPORT_CASE).unwrap();
    check_cas_export(&wb, rows);
}

fn cas_export_witness() -> Value {
    let case = cas_export_case();
    let actions = case.casview.as_ref().unwrap();
    let expected = &actions.last().unwrap().rows;
    let mut authored = Workbench::from_script(&case.script.join("\n")).unwrap();
    for step in actions {
        assert!(authored.submit_cas(step.row - 1, step.input.as_deref().unwrap()));
    }
    check_cas_export(&authored, expected);
    let mut imported = Workbench::default();
    imported
        .load_for_export(
            include_bytes!("../../../golden/ggb/casview/casview-rows-002.xml"),
            CAS_EXPORT_CASE,
        )
        .unwrap();
    let dir = Scratch::new("cas");
    for (name, wb, visible) in [("authored", authored, true), ("imported", imported, false)] {
        fs::write(
            dir.0.join(format!("{name}.ggb")),
            wb.export_ggb_with_cas_visibility(None, visible).unwrap(),
        )
        .unwrap();
    }
    let output = oracle_command(&[], &dir.0)
        .env("GGB_HARNESS_CAS", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "CAS export oracle failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let read = |name| -> Value {
        let snapshot: Value =
            serde_json::from_slice(&fs::read(dir.0.join(format!("{name}.ggb.snapshot"))).unwrap())
                .unwrap();
        snapshot["casRows"].clone()
    };
    let saved = read("authored");
    assert_eq!(saved, read("imported"));
    let actual = saved
        .as_array()
        .expect("reference did not reload the CAS table");
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual["input"], expected.input);
        assert_eq!(actual["output"], expected.output);
        assert_eq!(actual["latex"], expected.latex);
        if let Some(number) = expected.number {
            assert!(numbers_agree(actual["number"].as_f64().unwrap(), number));
        }
    }
    json!({ "fixture": CAS_EXPORT_CASE, "saved": saved })
}

#[test]
fn input_box_export_keeps_the_link_and_widget_geometry() {
    let source = include_bytes!("../../../golden/ggb/saved/saved-input-box-001.xml");
    let mut wb = Workbench::default();
    wb.load_for_export(source, "input-box-widget").unwrap();
    let (_, again) = round_trip("input-box-widget", &wb, None);
    let xml = exported_xml(&again);
    assert!(xml.contains(r#"<labelOffset x="124" y="88""#));
    assert!(xml.contains(r#"<length val="12""#));
}

#[test]
fn input_box_export_keeps_both_rounding_modes() {
    let source = include_bytes!("../../../golden/ggb/saved/saved-input-rounding-001.xml");
    let mut wb = Workbench::default();
    wb.load_for_export(source, "input-box-rounding").unwrap();
    let (_, again) = round_trip("input-box-rounding", &wb, None);
    let xml = exported_xml(&again);
    assert!(xml.contains(r#"<decimals val="4""#));
    assert!(xml.contains(r#"<significantfigures val="5""#));
}

const SPREADSHEET_EXPORT_CASE: &str = "spreadsheet-files-001";

fn spreadsheet_export_case() -> geo_golden::model::Case {
    load_all(&repo_root())
        .unwrap()
        .into_iter()
        .find(|case| case.id == SPREADSHEET_EXPORT_CASE)
        .unwrap()
}

fn spreadsheet_export_source() -> &'static [u8] {
    include_bytes!("../../../golden/ggb/spreadsheet/spreadsheet-files-001.xml")
}

fn spreadsheet_cells(wb: &Workbench) -> BTreeMap<String, String> {
    let c = wb.construction();
    c.spreadsheet_cells()
        .map(|(address, _)| (address.label(), c.spreadsheet_display(address)))
        .collect()
}

fn authored_spreadsheet(case: &geo_golden::model::Case) -> Workbench {
    let mut authored = Workbench::default();
    for step in case.spreadsheet.as_ref().unwrap() {
        let input = step.input.as_deref().unwrap();
        let line = match &step.cell {
            Some(cell) => geo_kernel::spreadsheet::edit_line(
                geo_kernel::spreadsheet::Address::parse(cell).unwrap(),
                input,
            ),
            None => input.into(),
        };
        authored.run_line(&line).unwrap();
    }
    authored
}

#[test]
fn reopens_reference_spreadsheet_settings_and_authored_cells() {
    use geo_kernel::spreadsheet::Address;
    let case = spreadsheet_export_case();
    let expected = &case.spreadsheet.as_ref().unwrap().last().unwrap().cells;
    let mut imported = Workbench::default();
    imported
        .load_for_export(spreadsheet_export_source(), SPREADSHEET_EXPORT_CASE)
        .unwrap();
    assert_eq!(&spreadsheet_cells(&imported), expected);
    assert!(imported.spreadsheet_view().is_some());
    let bytes = imported.export_ggb(None).unwrap();
    let again = geo_import::import(&bytes).unwrap();
    assert_eq!(again.spreadsheet_view.as_ref(), imported.spreadsheet_view());

    let authored = authored_spreadsheet(&case);
    let address = Address::parse("B1").unwrap();
    let export = authored
        .export_ggb_with_spreadsheet(None, false, true, Some(address))
        .unwrap();
    imported
        .load_for_export(&export, SPREADSHEET_EXPORT_CASE)
        .unwrap();
    assert_eq!(&spreadsheet_cells(&imported), expected);
    assert_eq!(
        imported.spreadsheet_view().unwrap().selected(),
        Some(address)
    );
    assert!(imported.spreadsheet_view().unwrap().visible.unwrap());
    let c = imported.construction();
    assert_eq!(
        c.spreadsheet_input(Address::parse("A1").unwrap())
            .as_deref(),
        Some("1.250")
    );
    assert_eq!(c.spreadsheet_input(address).as_deref(), Some("=$A$1 * 2"));
}

fn spreadsheet_export_witness() -> Value {
    let case = spreadsheet_export_case();
    let expected = &case.spreadsheet.as_ref().unwrap().last().unwrap().cells;
    let authored = authored_spreadsheet(&case);
    let mut imported = Workbench::default();
    imported
        .load_for_export(spreadsheet_export_source(), SPREADSHEET_EXPORT_CASE)
        .unwrap();
    let container = std::env::var_os("GEOKERNEL_EXPORT_SPREADSHEET_CONTAINER").is_some();
    let root = repo_root();
    let dir = if container {
        let dir = root
            .join("tools/ggb-harness/build")
            .join(format!("spreadsheet-export-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        Scratch(dir)
    } else {
        Scratch::new("spreadsheet")
    };
    for (name, wb) in [("authored", authored), ("imported", imported)] {
        assert_eq!(&spreadsheet_cells(&wb), expected);
        fs::write(
            dir.0.join(format!("{name}.ggb")),
            wb.export_ggb(None).unwrap(),
        )
        .unwrap();
    }
    let output = if container {
        Command::new(root.join("tools/ggb-harness/linux-x86_64.sh"))
            .arg("export-snapshot")
            .arg(dir.0.strip_prefix(&root).unwrap())
            .output()
            .unwrap()
    } else {
        java_snapshots(&dir.0)
    };
    assert!(
        output.status.success(),
        "spreadsheet export oracle failed:
{}
{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let read = |name| -> Value {
        let snapshot: Value =
            serde_json::from_slice(&fs::read(dir.0.join(format!("{name}.ggb.snapshot"))).unwrap())
                .unwrap();
        snapshot["spreadsheetCells"].clone()
    };
    let saved = read("authored");
    assert_eq!(saved, read("imported"));
    assert_eq!(saved, json!(expected));
    json!({ "fixture": SPREADSHEET_EXPORT_CASE, "saved": saved })
}

#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn reloads_spreadsheet_exports_in_geogebra() {
    let witness = spreadsheet_export_witness();
    let path = proof_path();
    let mut proof: BTreeMap<String, BTreeMap<String, Value>> =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    if std::env::var_os("GEOKERNEL_EXPORT_BLESS").is_some() {
        proof
            .entry("Spreadsheet".into())
            .or_default()
            .insert("cells".into(), witness);
        fs::write(path, serde_json::to_string_pretty(&proof).unwrap() + "\n").unwrap();
    } else {
        assert_eq!(proof["Spreadsheet"]["cells"], witness);
    }
}

#[test]
fn saved_widget_and_list_properties_round_trip() {
    let file = br#"<geogebra format="5.0"><construction>
      <element type="numeric" label="a"><value val="2"/></element>
      <command name="InputBox"><input a0="a"/><output a0="box"/></command>
      <element type="textfield" label="box">
        <font serif="true" sizeM="1.75" style="1"/>
        <contentSerif val="true"/>
        <symbolic val="true"/>
        <textAlign val="right"/>
        <tempUserInput eval="a&amp;b" display="a &lt; b"/>
      </element>
      <expression label="L" exp="{1, 2, 3}"/>
      <element type="list" label="L">
        <comboBox val="true"/>
        <selectedIndex val="2"/>
        <font serif="false" sizeM="1.25" style="2"/>
        <decimals val="4"/>
      </element>
    </construction></geogebra>"#;
    let load = |bytes| {
        let mut wb = Workbench::default();
        wb.load_for_export(bytes, "saved widgets").unwrap();
        wb
    };
    let original = export_widget_xml(&load(file));
    for tag in [
        r#"<font serif="true" sizeM="1.75" style="1"/>"#,
        r#"<contentSerif val="true"/>"#,
        r#"<symbolic val="true"/>"#,
        r#"<textAlign val="right"/>"#,
        r#"<tempUserInput eval="a&amp;b" display="a &lt; b"/>"#,
        r#"<comboBox val="true"/>"#,
        r#"<selectedIndex val="2"/>"#,
        r#"<font serif="false" sizeM="1.25" style="2"/>"#,
        r#"<decimals val="4"/>"#,
    ] {
        assert!(original.contains(tag), "{tag}: {original}");
    }
    assert_eq!(export_widget_xml(&load(original.as_bytes())), original);
}

fn export_widget_xml(wb: &Workbench) -> String {
    let bytes = wb.export_ggb(wb.file_view()).unwrap();
    geo_import::archive::extract_xml(&bytes)
        .unwrap()
        .into_owned()
}

#[test]
fn saved_geometry_and_display_flags_round_trip() {
    let file = br#"<geogebra format="5.0"><construction>
      <element type="point" label="P"><coords x="2" y="3" z="1"/><coordStyle style="polar"/><incrementY val="0.250"/></element>
      <element type="vector" label="v"><coords x="1" y="0" z="0"/><startPoint exp="P"/><coordStyle style="polar"/></element>
      <element type="numeric" label="n"><value val="2"/><slopeTriangleSize val="3"/><symbolic val="false"/></element>
      <expression label="f" exp="x^2"/>
      <element type="function" label="f"><tableview column="2" points="false"/><simplifyCoefficients val="false"/></element>
      <element type="button" label="b"><dimensions width="135.0" height="42"/><absoluteScreenLocation x="18" y="29"/></element>
      <expression label="t" exp="&quot;Formula&quot;"/>
      <element type="text" label="t"><isLaTeX val="true"/><symbolic val="true"/></element>
    </construction></geogebra>"#;
    let mut wb = Workbench::default();
    wb.load_for_export(file, "presentation flags").unwrap();
    let xml = export_widget_xml(&wb);
    for tag in [
        r#"<coordStyle style="polar"/>"#,
        r#"<incrementY val="0.250"/>"#,
        r#"<startPoint exp="P"/>"#,
        r#"<slopeTriangleSize val="3"/>"#,
        r#"<symbolic val="false"/>"#,
        r#"<tableview column="2" points="false"/>"#,
        r#"<simplifyCoefficients val="false"/>"#,
        r#"<dimensions width="135.0" height="42"/>"#,
        r#"<absoluteScreenLocation x="18" y="29"/>"#,
        r#"<isLaTeX val="true"/>"#,
        r#"<symbolic val="true"/>"#,
    ] {
        assert!(xml.contains(tag), "{tag}: {xml}");
    }
}

#[test]
fn saved_properties_obey_document_and_archive_limits() {
    let wrap = |tag: &str| {
        format!(
            "<geogebra><construction><element type=\"point\" label=\"P\"><coords x=\"0\" y=\"0\" z=\"1\"/>{tag}</element></construction></geogebra>"
        )
    };
    let oversized = wrap(&format!(
        "<coordStyle style=\"{}\"/>",
        "x".repeat(geo_import::xml::MAX_ATTRIBUTE_BYTES + 1)
    ));
    let many = wrap(&r#"<coordStyle style="polar"/>"#.repeat(geo_import::xml::MAX_ITEMS));
    for archived in [false, true] {
        let bytes = |xml: &str| {
            if archived {
                archive(xml.as_bytes(), None)
            } else {
                xml.as_bytes().to_vec()
            }
        };
        assert!(matches!(
            geo_import::import(&bytes(&oversized)),
            Err(geo_import::ImportError::AttributeTooLong)
        ));
        assert!(matches!(
            geo_import::import(&bytes(&many)),
            Err(geo_import::ImportError::TooManyItems)
        ));
    }
}

#[test]
fn saved_properties_do_not_reintroduce_unchecked_expressions_or_nested_tags() {
    let file = br#"<geogebra format="5.0"><construction>
      <element type="point" label="P">
        <coords x="1" y="0" z="1"/>
        <incrementY val="Execute({&quot;n = 7&quot;})"/>
        <unknown><coords x="20" y="30" z="1"/><coordStyle style="polar"/></unknown>
      </element>
      <element type="vector" label="v"><coords x="1" y="0" z="0"/><startPoint exp="Execute({})"/></element>
    </construction></geogebra>"#;
    let mut wb = Workbench::default();
    wb.load_for_export(file, "nested properties").unwrap();
    let xml = export_widget_xml(&wb);
    assert!(!xml.contains("Execute"), "{xml}");
    assert!(!xml.contains("<coordStyle"), "{xml}");
    let c = wb.construction();
    assert_eq!(
        c.value(c.id("P").unwrap()),
        geo_kernel::Value::Point { x: 1.0, y: 0.0 }
    );
}
