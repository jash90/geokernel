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
    ("export-texts-001", 0),
    ("funcanalysis-curvatureupstream-002", 0),
    ("georestintersect-affine-002", 0),
    ("georestintersect-growth-001", 2),
    ("georestintersect-pathcurve-001", 0),
    ("georestsmall-dilatecurve-001", 2),
    ("georestsmall-matrixcurve-001", 0),
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
    "saved-curve-branch-001.xml",
    "saved-curve-intersections-001.xml",
    "saved-curve-intersections-002.xml",
    "saved-paths-002.xml",
    "saved-images-001.ggb",
    "saved-images-002.ggb",
    "saved-object-scripts-001.xml",
    "saved-slider-bounds-001.xml",
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

fn labelled(c: &Construction) -> BTreeMap<&str, Id> {
    c.ids()
        .filter_map(|id| {
            let label = c.label(id);
            (!label.starts_with('\0')).then_some((label, id))
        })
        .collect()
}

/// A document to export: its name, the live workbench, the view of a golden
/// case, and the bytes of the file it was opened from.
type Case = (String, Workbench, Option<FileView>, Option<Vec<u8>>);

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
        out.push((name.into(), wb, view, None));
    }
    for &file in SAVED_CASES {
        let mut wb = Workbench::default();
        let bytes = fs::read(repo_root().join("golden/ggb/saved").join(file)).unwrap();
        wb.load_for_export(&bytes, file).unwrap();
        let name = file.split_once('.').unwrap().0;
        out.push((name.into(), wb, None, Some(bytes)));
    }
    let mut styled = Workbench::default();
    styled.load_for_export(STYLED_FILE, "styled").unwrap();
    out.push(("styled".into(), styled, None, Some(STYLED_FILE.to_vec())));
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
            assert_eq!(
                Some(style),
                after.object_styles.get(label),
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
#[test]
#[ignore = "requires the GeoGebra Java harness"]
fn geogebra_reloads_exports_and_matches_the_kernel() {
    let dir = Scratch::new("valid");
    // The files the documents were opened from, read by the same reader.
    let sources = Scratch::new("sources");
    // The same documents without their saved presentation, as a construction
    // made in the application exports. A property is proven where the source
    // differs from this and agrees with the export.
    let plain = Scratch::new("plain");
    let constructions: Vec<_> = corpus()
        .into_iter()
        .map(|(name, wb, view, source)| {
            let (bytes, again) = round_trip(&name, &wb, view);
            fs::write(dir.0.join(format!("{name}.ggb")), bytes).unwrap();
            if let Some(source) = &source {
                let zipped = if geo_import::archive::is_zip(source) {
                    source.clone()
                } else {
                    archive(source, None)
                };
                fs::write(sources.0.join(format!("{name}.ggb")), zipped).unwrap();
                let mut stripped = Workbench::default();
                let rejected = stripped.restore_text(&wb.snapshot_text()).unwrap();
                assert!(rejected.is_empty(), "{name}: {rejected:?}");
                let bytes = stripped
                    .export_ggb(view.or(wb.file_view()))
                    .unwrap_or_else(|e| panic!("{name}: {e}"));
                fs::write(plain.0.join(format!("{name}.ggb")), bytes).unwrap();
            }
            (name, wb, again, view, source.is_some())
        })
        .collect();
    for scratch in [&dir, &sources, &plain] {
        let output = java_snapshots(&scratch.0);
        assert!(
            output.status.success(),
            "Java export oracle failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let mut covered = BTreeSet::new();
    let mut count = 0;
    let mut properties: BTreeSet<(String, String)> = BTreeSet::new();
    for (name, live, imported, view, from_file) in &constructions {
        let read = |scratch: &Scratch| -> Value {
            let path = scratch.0.join(format!("{name}.ggb.snapshot"));
            serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
        };
        let snap = read(&dir);
        let snapshot = snap["objects"].as_object().unwrap();
        check_styles(name, live, *view, &snap);
        if *from_file {
            // The reference application reads the source file and the
            // export of its import to the same presentation, property by
            // property, for every object.
            let source = read(&sources);
            let stripped = read(&plain);
            let (before, after) = (&source["styles"], &snap["styles"]);
            assert!(
                before
                    .as_object()
                    .unwrap()
                    .keys()
                    .eq(after.as_object().unwrap().keys()),
                "{name}: labels differ from the source"
            );
            for (label, style) in before.as_object().unwrap() {
                let (mut style, mut exported) = (style.clone(), after[label].clone());
                // `check_styles` compares the bounds with the kernel.
                for style in [&mut style, &mut exported] {
                    if let Some(slider) = style.get_mut("slider").and_then(Value::as_object_mut) {
                        slider.remove("min");
                        slider.remove("max");
                    }
                }
                assert_json(&style, &exported, &format!("{name}: {label}"));
                let c = live.construction();
                let kind = format!("{:?}", c.kind(c.id(label).unwrap()));
                let (mut saved, mut unsaved) = (BTreeMap::new(), BTreeMap::new());
                flatten("", &style, &mut saved);
                flatten("", &stripped["styles"][label], &mut unsaved);
                for (property, value) in saved {
                    if unsaved.get(&property) != Some(&value)
                        || new_object(&kind, &property).is_some_and(|new| new != value)
                    {
                        properties.insert((kind.clone(), property));
                    }
                }
            }
            assert_json(&source["settings"], &snap["settings"], name);
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
    let mut proven: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (kind, property) in &properties {
        proven.entry(kind).or_default().insert(property);
    }
    // `yes` in the kind matrix means: every property of the kind is proven.
    let mut disagreements = Vec::new();
    for row in matrix.records() {
        let kind = row["kernel_kind"];
        if row["export"] == "no" {
            continue;
        }
        let have = proven.get(kind).cloned().unwrap_or_default();
        let missing: Vec<_> = kind_properties(kind)
            .into_iter()
            .filter(|property| !have.contains(property))
            .collect();
        println!(
            "{kind}: {} of {} properties{}",
            kind_properties(kind).len() - missing.len(),
            kind_properties(kind).len(),
            if missing.is_empty() {
                String::new()
            } else {
                format!(", not proven: {}", missing.join(", "))
            }
        );
        if (row["export"] == "yes") != missing.is_empty() {
            disagreements.push(format!(
                "{kind} is `{}`, not proven: {missing:?}",
                row["export"]
            ));
        }
    }
    assert!(
        disagreements.is_empty(),
        "docs/kinds.csv export column: {disagreements:#?}"
    );
    println!(
        "{} exported files, {count} objects, {} kinds, {} kind and property pairs",
        constructions.len(),
        covered.len(),
        properties.len()
    );
}

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
/// left out where the reference application itself does not keep it: it
/// refuses to fix a chart, reads no dash pattern for a number, saves no line
/// opacity for a function of two variables, and does not save the auxiliary
/// flag of a text, a locus, a button or an input box.
///
/// `chart.slices` stands for the colour and filling of single slices, which
/// the importer does not read: no snapshot has it, so a pie chart stays
/// `partial`.
fn kind_properties(kind: &str) -> Vec<&'static str> {
    const SHARED: &[&str] = &[
        "shown",
        "layer",
        "color",
        "condition",
        "dynamic_color",
        "color_space",
        "selection_allowed",
        "tooltip_mode",
    ];
    const FLAGS: &[&str] = &["fixed", "auxiliary"];
    const LABEL: &[&str] = &["label_shown", "label_mode", "caption", "label_offset"];
    const LINE: &[&str] = &["line.type", "line.thickness", "line.opacity"];
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
            &["point_size", "point_style", "trace"],
        ],
        "Line" | "Ray" | "Vector" | "PolyLine" | "Function" | "Curve" => {
            &[SHARED, FLAGS, LABEL, LINE, &["trace"]]
        }
        "Locus" => &[SHARED, &["fixed"], LABEL, LINE, &["trace"]],
        "Segment" => &[SHARED, FLAGS, LABEL, LINE, &["trace", "decoration"]],
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
        "PieChart" => &[SHARED, &["auxiliary"], LABEL, LINE, FILL, &["chart.slices"]],
        "Number" => &[
            SHARED,
            FLAGS,
            LABEL,
            &[
                "line.thickness",
                "line.opacity",
                "slider.position",
                "slider.width",
                "slider.horizontal",
                "slider.fixed",
                "slider.absolute",
                "slider.show_algebra",
                "animation.step",
                "animation.speed",
                "animation.mode",
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
        "Button" | "InputBox" => &[SHARED, &["fixed", "caption", "background"]],
        "Image" => &[
            &[
                "shown",
                "layer",
                "condition",
                "selection_allowed",
                "tooltip_mode",
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

fn java_snapshots(dir: &std::path::Path) -> std::process::Output {
    Command::new(repo_root().join("tools/ggb-harness/harness.sh"))
        .arg("export-snapshot")
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
                let step = match (name, label) {
                    ("export-sliders-001", "a") => json!(0.25),
                    ("export-sliders-001", "b") => json!(0.5),
                    _ => {
                        animation.map_or(json!(0.1), |a| number_or_reference(c, a.step.as_deref()))
                    }
                };
                let speed = animation.and_then(|a| a.speed.as_deref()).or(Some("1"));
                assert_json(&step, &moving["step"], &format!("{where_}.step"));
                assert_json(&number_or_reference(c, speed), &moving["speed"], &where_);
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
            expected["alpha"] = json!(geo_app_core::style::fill_opacity(c, id));
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
    source.map_or(Value::Null, |source| {
        let value = source.parse::<f64>().unwrap_or_else(|_| {
            let geo_kernel::Value::Number(value) = c.value(c.id(source).unwrap()) else {
                panic!("expected a numeric reference for {source}")
            };
            value
        });
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
