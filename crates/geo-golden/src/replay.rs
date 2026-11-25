//! Replays a golden case on `geo-kernel` and compares every snapshot.

use crate::compare::{
    compare_arc, compare_conic, compare_extra, compare_list, compare_locus, compare_sampled,
    compare_stroke, line_scale_agrees, numbers_agree, Sign,
};
use crate::model::{
    implicit_output, indexed_prefix, label_prefix, CasAction, CasViewStep, Case, SpreadsheetAction,
    SpreadsheetStep, Step,
};
use geo_app_core::workbench::Workbench;
use geo_kernel::{script, Construction, Def, Kind};

/// One disagreement between GeoGebra and the kernel.
#[derive(Debug, Clone, PartialEq)]
pub struct Mismatch {
    pub step: usize,
    pub label: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Default)]
pub struct CaseOutcome {
    pub mismatches: Vec<Mismatch>,
    pub sign_agree: usize,
    pub sign_disagree: usize,
    /// Defined lines whose raw coefficients equal GeoGebra's (see
    /// [`line_scale_agrees`]); counted, never a failure.
    pub scale_agree: usize,
    /// Labels of the defined lines whose raw coefficients differ from
    /// GeoGebra's, once per snapshot.
    pub scale_disagree: Vec<String>,
    /// Defined conics whose raw matrix agrees with GeoGebra's.
    pub conic_raw_agree: usize,
    /// Defined conics whose matrix agrees with GeoGebra's up to a factor.
    pub conic_scaled_agree: usize,
    /// Defined conics on both sides.
    pub conic_compared: usize,
}

fn failure(step: usize, label: &str, expected: &str, actual: String) -> Mismatch {
    Mismatch {
        step,
        label: label.into(),
        expected: expected.into(),
        actual,
    }
}

pub fn replay(case: &Case) -> CaseOutcome {
    if case.casview.is_some()
        || case.spreadsheet.is_some()
        || case.steps.iter().any(|step| {
            matches!(
                step,
                Step::Delete { .. }
                    | Step::ReloadGgb
                    | Step::SliderAnimation { .. }
                    | Step::Undo
                    | Step::Redo
                    | Step::Reload
            )
        })
    {
        return replay_document(case);
    }
    let mut out = CaseOutcome::default();
    let mut c = Construction::new();
    if let Some([xmin, xmax, ymin, ymax, xscale, yscale]) = case.view {
        c.set_locus_view(xmin, xmax, ymin, ymax, xscale, yscale);
    }
    let mut boundary_outputs = script::BoundaryOutputs::default();
    let mut former = std::collections::HashMap::new();
    for (i, line) in case.script.iter().enumerate() {
        let before = c.len();
        let step_before = c.step();
        match script::exec(&mut c, line) {
            Ok(Some(id)) if c.len() > before => {
                boundary_outputs.record(&c, line, id, c.len() - before)
            }
            Ok(_) => {}
            Err(e) => {
                out.mismatches.push(failure(
                    0,
                    &format!("script line {}", i + 1),
                    "the kernel runs the line",
                    format!("`{line}`: {e}"),
                ));
                return out;
            }
        }
        if c.step() != step_before {
            boundary_outputs.grow_after_step(&mut c, |_, _, _| {});
        }
    }
    let implicit_outputs: Vec<_> = case
        .script
        .iter()
        .filter_map(|line| implicit_output(line, &case.snapshots[0]).map(|(label, _)| label))
        .collect();
    for (i, (step, snapshot)) in case.steps.iter().zip(&case.snapshots).enumerate() {
        if let Step::Append { line } = step {
            let before = c.len();
            match script::exec(&mut c, line) {
                Ok(Some(id)) if c.len() > before => {
                    boundary_outputs.record(&c, line, id, c.len() - before);
                }
                result => {
                    out.mismatches.push(failure(
                        i,
                        "step",
                        "the kernel appends the definition",
                        format!("{result:?}"),
                    ));
                    return out;
                }
            }
        }
        match apply(&mut c, step) {
            Ok(Some(id)) => {
                boundary_outputs.grow(&mut c, id, |_, _, _| {});
            }
            Ok(None) => {}
            Err(e) => {
                out.mismatches
                    .push(failure(i, "step", "the kernel applies the step", e));
                return out;
            }
        }
        if !implicit_outputs.is_empty() {
            for label in c.ids().map(|id| c.label(id)) {
                if !snapshot.contains_key(label)
                    && implicit_outputs.iter().any(|base| {
                        label == *base || indexed_prefix(label) == Some(label_prefix(base))
                    })
                {
                    out.mismatches.push(failure(
                        i,
                        label,
                        "no output",
                        "unexpected command output".into(),
                    ));
                }
            }
        }
        // The harness reads an output whose label its command freed under
        // the label it had (`Session.recordGrowingPoints`), also after the
        // command labels it again.
        for id in c.ids() {
            let label = c.label(id);
            if !label.starts_with('\0') && former.get(label) != Some(&id) {
                former.insert(label.to_owned(), id);
            }
        }
        compare_snapshot(&mut out, &c, i, snapshot, &former);
    }
    if let Some(row) = &case.cascell {
        let result = c.evaluate_cas_row(&row.input);
        for (label, expected, actual) in [
            ("output", &row.output, result.text),
            ("latex", &row.latex, result.latex),
        ] {
            if expected != &actual {
                out.mismatches.push(failure(0, label, expected, actual));
            }
        }
    }
    out
}

fn replay_document(case: &Case) -> CaseOutcome {
    let mut out = CaseOutcome::default();
    let mut document = Workbench::default();
    if let Err(error) = document.load_script(&case.script.join("\n")) {
        out.mismatches
            .push(failure(0, "script", "the document loads", error));
        return out;
    }
    for (i, (step, snapshot)) in case.steps.iter().zip(&case.snapshots).enumerate() {
        let id = |label: &str| document.construction().id(label);
        let result = match step {
            Step::Initial => Ok(()),
            Step::ReloadGgb => document
                .export_ggb(None)
                .map_err(|error| error.to_string())
                .and_then(|bytes| document.load_for_export(&bytes, "roundtrip.ggb")),
            Step::SliderAnimation { label, options } => {
                check_slider_animation(&document, label, options)
            }
            Step::Add { line } | Step::Redefine { line } | Step::Append { line } => {
                document.run_line(line)
            }
            Step::AnchorText { label, point } => document.run_lines(&[format!(
                "$textProperties({label}, -, -, -, -, -, point, {point})"
            )]),
            Step::Undo => document
                .undo()
                .then_some(())
                .ok_or_else(|| "cannot undo".into()),
            Step::Redo => document
                .redo()
                .then_some(())
                .ok_or_else(|| "cannot redo".into()),
            Step::Reload => {
                let snapshot = document.snapshot_text();
                document.restore_text(&snapshot).and_then(|rejected| {
                    if rejected.is_empty() {
                        Ok(())
                    } else {
                        Err(format!("snapshot rejected lines: {rejected:?}"))
                    }
                })
            }
            Step::Reject { line, error } => match document.run_line(line) {
                Ok(()) => Err("the document accepted the line".into()),
                Err(e) => match error {
                    Some(want) if *want != e => Err(format!(
                        "expected the rejection `{want}` of `{line}`, got `{e}`"
                    )),
                    _ => Ok(()),
                },
            },
            Step::Delete { label } => id(label)
                .is_some_and(|id| document.delete(id))
                .then_some(())
                .ok_or_else(|| format!("cannot delete `{label}`")),
            Step::Move { label, x, y } => id(label)
                .is_some_and(|id| document.move_point(id, *x, *y))
                .then_some(())
                .ok_or_else(|| format!("cannot move `{label}`")),
            Step::SetNumber { label, value } => id(label)
                .is_some_and(|id| {
                    // The workbench reports whether an edit changed the document.
                    // A same-value set of a free number can also succeed.
                    document.set_number(id, *value)
                        || matches!(document.construction().def(id), Def::Number(old)
                            if old.to_bits() == value.to_bits())
                })
                .then_some(())
                .ok_or_else(|| format!("cannot set `{label}`")),
            Step::SetSliderBounds { .. } => {
                Err("unexpected slider bounds edit in a deletion case".into())
            }
            Step::Animate { .. } => Err("unexpected animation in a deletion case".into()),
        };
        if let Err(error) = result {
            out.mismatches
                .push(failure(i, "step", "the document applies the step", error));
            break;
        }
        let c = document.construction();
        for label in case.snapshots[..i]
            .iter()
            .flat_map(|previous| previous.keys())
        {
            if !snapshot.contains_key(label) && c.id(label).is_some() {
                out.mismatches
                    .push(failure(i, label, "no object", "object survives".into()));
            }
        }
        compare_snapshot(&mut out, c, i, snapshot, &Default::default());
    }
    if let Some(actions) = &case.casview {
        replay_cas_view(&mut document, &mut out, actions);
    }
    if let Some(actions) = &case.spreadsheet {
        replay_spreadsheet(&mut document, &mut out, actions);
    }
    out
}

fn check_slider_animation(
    document: &Workbench,
    label: &str,
    options: &crate::model::AnimationOptions,
) -> Result<(), String> {
    let bytes = document
        .export_ggb(None)
        .map_err(|error| error.to_string())?;
    let imported = geo_import::import(&bytes).map_err(|error| error.to_string())?;
    let actual = imported
        .slider_animation
        .get(label)
        .ok_or_else(|| format!("no saved animation for `{label}`"))?;
    let number = |source: &str| document.construction().numeric_value(source);
    for (name, actual, expected) in [
        ("step", actual.step.as_deref(), options.step.as_deref()),
        ("speed", actual.speed.as_deref(), options.speed.as_deref()),
    ] {
        if actual != expected {
            let equal = actual
                .and_then(number)
                .zip(expected.and_then(number))
                .is_some_and(|(a, b)| crate::compare::numbers_agree(a, b));
            if !equal {
                return Err(format!(
                    "animation {name} for `{label}`: expected {expected:?}, got {actual:?}"
                ));
            }
        }
    }
    Ok(())
}

fn replay_spreadsheet(
    document: &mut Workbench,
    out: &mut CaseOutcome,
    actions: &[SpreadsheetStep],
) {
    use geo_kernel::spreadsheet::Address;
    use std::collections::BTreeMap;
    for (i, step) in actions.iter().enumerate() {
        let address = |cell: Option<&str>| {
            Address::parse(cell.expect("validated address")).expect("validated address")
        };
        let applied = match step.action {
            SpreadsheetAction::Command => {
                let line = step.input.as_deref().unwrap();
                if let Some(("SetValue", args)) = script::split_call(line) {
                    match args.as_slice() {
                        [label, value] => document.construction().id(label)
                            .zip(document.construction().numeric_value(value))
                            .filter(|&(id, value)| document.set_number(id, value))
                            .map(|_| ())
                            .ok_or_else(|| "cannot set the cell value".into()),
                        _ => Err("SetValue needs a cell and a number".into()),
                    }
                } else {
                    document.run_line(line)
                }
            }
            SpreadsheetAction::Edit => document.edit_spreadsheet_cell(
                address(step.cell.as_deref()),
                step.input.as_deref().unwrap(),
            ),
            SpreadsheetAction::Drag => geo_kernel::spreadsheet::drag_line(
                document.construction(),
                address(step.cell.as_deref()),
                address(step.target.as_deref()),
            )
            .and_then(|line| document.run_line(&line.ok_or("empty drag source")?)),
            SpreadsheetAction::Reload => document
                .export_ggb(None)
                .map_err(|e| e.to_string())
                .and_then(|bytes| document.load_bytes(&bytes, "Spreadsheet round trip")),
            SpreadsheetAction::DegreeMode => document.load_bytes(
                br#"<geogebra><kernel><angleUnit val="degree"/></kernel><construction/></geogebra>"#,
                "Spreadsheet degree mode",
            ),
            SpreadsheetAction::Load => {
                document.load_bytes(step.input.as_deref().unwrap().as_bytes(), "Spreadsheet file")
            }
            SpreadsheetAction::Undo => document
                .undo()
                .then_some(())
                .ok_or_else(|| "cannot undo".into()),
            SpreadsheetAction::Redo => document
                .redo()
                .then_some(())
                .ok_or_else(|| "cannot redo".into()),
        };
        if let Err(error) = applied {
            out.mismatches
                .push(failure(i, "spreadsheet", "the action applies", error));
            return;
        }
        if let Some(view) = &step.view {
            let actual = document.spreadsheet_view().map(|sheet| sheet.grid());
            let expected = (view.show_grid, view.first_column_width);
            let actual = actual.map(|grid| (grid.show_lines, f64::from(grid.columns.edge(1))));
            if actual != Some(expected) {
                out.mismatches.push(failure(
                    i,
                    "view",
                    &format!("{expected:?}"),
                    format!("{actual:?}"),
                ));
            }
        }
        let c = document.construction();
        let cells: BTreeMap<_, _> = c
            .spreadsheet_cells()
            .map(|(address, _)| (address.label(), document.spreadsheet_display(address)))
            .collect();
        for (label, expected) in &step.cells {
            if cells.get(label) != Some(expected) {
                out.mismatches.push(failure(
                    i,
                    label,
                    expected,
                    cells
                        .get(label)
                        .cloned()
                        .unwrap_or_else(|| "no cell".into()),
                ));
            }
        }
        for (label, value) in &cells {
            if !step.cells.contains_key(label) {
                out.mismatches
                    .push(failure(i, label, "no cell", value.clone()));
            }
        }
        for (label, expected) in &step.outputs {
            let result = c
                .id(label)
                .ok_or_else(|| "no object".into())
                .and_then(|id| crate::compare::compare_construction_object(expected, c, id));
            if let Err(error) = result {
                out.mismatches
                    .push(failure(i, label, &format!("{expected:?}"), error));
            }
        }
    }
}

fn replay_cas_view(document: &mut Workbench, out: &mut CaseOutcome, actions: &[CasViewStep]) {
    for (i, step) in actions.iter().enumerate() {
        let row = step.row - 1;
        let applied = match step.action {
            CasAction::Evaluate | CasAction::Edit | CasAction::StaticEdit => {
                document.submit_cas(row, step.input.as_deref().expect("validated CAS input"))
            }
            CasAction::Delete => document.delete_cas(row),
            CasAction::Undo => document.undo(),
            CasAction::Redo => document.redo(),
            CasAction::Enter => {
                let mut app = geo_app_core::app::App::new(std::mem::take(document), None);
                app.edit_cas_row(row);
                app.submit_cas_input();
                *document = app.wb;
                document.cas_rows().len() > row
            }
            CasAction::Load => {
                let cells = step
                    .xml
                    .as_deref()
                    .unwrap()
                    .repeat(step.repeat.unwrap_or(1));
                let xml = format!("<geogebra><construction>{cells}</construction></geogebra>");
                document.load_bytes(xml.as_bytes(), "CAS recording").is_ok()
            }
            CasAction::Visibility => {
                let tag = step.view_xml.as_deref().unwrap();
                let xml = format!(
                    "<geogebra><gui><perspectives><perspective id=\"tmp\"><views>\
                    {tag}</views></perspective></perspectives></gui><construction/></geogebra>"
                );
                let mut saved = Workbench::default();
                saved.load_bytes(xml.as_bytes(), "CAS perspective").unwrap();
                let app = geo_app_core::app::App::new(saved, None);
                if app.show_cas != step.visible.unwrap() {
                    out.mismatches.push(failure(
                        i,
                        "visible",
                        &step.visible.unwrap().to_string(),
                        app.show_cas.to_string(),
                    ));
                }
                true
            }
        };
        if !applied {
            out.mismatches.push(failure(
                i,
                "action",
                "CAS action applied",
                format!("{:?} was refused", step.action),
            ));
            return;
        }
        let actual = document.cas_rows();
        if actual.len() != step.rows.len() {
            out.mismatches.push(failure(
                i,
                "rows",
                &step.rows.len().to_string(),
                actual.len().to_string(),
            ));
        }
        for (n, (expected, actual)) in step.rows.iter().zip(actual).enumerate() {
            for (field, expected, actual) in [
                ("input", &expected.input, &actual.input),
                ("output", &expected.output, &actual.output.text),
                ("latex", &expected.latex, &actual.output.latex),
            ] {
                if expected != actual {
                    out.mismatches.push(failure(
                        i,
                        &format!("{}.{field}", n + 1),
                        expected,
                        actual.clone(),
                    ));
                }
            }
            if let Some(expected) = expected.number {
                let actual = if actual.unevaluated {
                    cas_number(&actual.output.text)
                } else {
                    actual.output.exact.as_deref().and_then(cas_number)
                };
                if !actual.is_some_and(|actual| numbers_agree(expected, actual)) {
                    out.mismatches.push(failure(
                        i,
                        &format!("{}.number", n + 1),
                        &expected.to_string(),
                        format!("{actual:?}"),
                    ));
                }
            }
        }
    }
}

/// With the same forms the harness records, read the exact scalar without
/// rounding it to the current algebra display precision.
fn cas_number(exact: &str) -> Option<f64> {
    if let Some((num, den)) = exact.split_once('/') {
        Some(num.trim().parse::<f64>().ok()? / den.trim().parse::<f64>().ok()?)
    } else {
        exact.parse().ok()
    }
}

fn compare_snapshot(
    out: &mut CaseOutcome,
    c: &Construction,
    i: usize,
    snapshot: &std::collections::BTreeMap<String, crate::model::Expected>,
    former: &std::collections::HashMap<String, geo_kernel::Id>,
) {
    for (label, expected) in snapshot {
        let Some(id) = c.id(label).or_else(|| former.get(label).copied()) else {
            out.mismatches.push(failure(
                i,
                label,
                &format!("{expected:?}"),
                "no such object".into(),
            ));
            continue;
        };
        let actual = c.value(id);
        if matches!(c.kind(id), geo_kernel::Kind::Arc | geo_kernel::Kind::Sector) {
            let data = c.arc(id);
            if let Err(why) = compare_arc(expected, c.kind(id), &actual, data.as_ref()) {
                out.mismatches.push(failure(
                    i,
                    label,
                    &format!("{expected:?}"),
                    format!("{actual:?} ({why})"),
                ));
            }
            continue;
        }
        if c.kind(id) == Kind::Locus {
            if let Err(why) = compare_locus(expected, Kind::Locus, &actual, c.locus(id)) {
                out.mismatches.push(failure(
                    i,
                    label,
                    &format!("{expected:?}"),
                    format!("{actual:?} ({why})"),
                ));
            }
            continue;
        }
        if c.kind(id) == Kind::PenStroke {
            let data = c.stroke(id);
            if let Err(why) = compare_stroke(expected, Kind::PenStroke, &actual, data) {
                out.mismatches.push(failure(
                    i,
                    label,
                    &format!("{expected:?}"),
                    format!("{actual:?} ({why})"),
                ));
            }
            continue;
        }
        if c.kind(id) == geo_kernel::Kind::Conic {
            let data = c.conic(id);
            let (result, agreement) = compare_conic(expected, c.kind(id), &actual, data.as_ref());
            if let Some(a) = agreement {
                out.conic_compared += 1;
                out.conic_raw_agree += usize::from(a.raw);
                out.conic_scaled_agree += usize::from(a.up_to_scale);
            }
            if let Err(why) = result {
                out.mismatches.push(failure(
                    i,
                    label,
                    &format!("{expected:?}"),
                    format!("{actual:?} ({why})"),
                ));
            }
            continue;
        }
        match line_scale_agrees(expected, &actual) {
            Some(true) => out.scale_agree += 1,
            Some(false) => out.scale_disagree.push(label.clone()),
            None => {}
        }
        let vertices: Vec<_> = c.vertices(id).collect();
        let sample = |x| c.function_value(id, x);
        let outcome = if c.kind(id) == Kind::List {
            compare_list(c, expected, Kind::List, c.elements(id))
        } else if c.kind(id) == Kind::PieChart {
            crate::compare::compare_pie(expected, c.chart(id).and_then(|d| d.pie.as_ref()))
        } else if matches!(
            c.kind(id),
            Kind::Text | Kind::FunctionNVar | Kind::Curve | Kind::ImplicitCurve
        ) {
            compare_extra(c, id, expected)
        } else {
            compare_sampled(expected, c.kind(id), &actual, &vertices, Some(&sample))
        };
        match outcome {
            Ok(Sign::Agrees) => out.sign_agree += 1,
            Ok(Sign::Flipped) => out.sign_disagree += 1,
            Ok(Sign::NotApplicable) => {}
            Err(why) => out.mismatches.push(failure(
                i,
                label,
                &format!("{expected:?}"),
                format!("{actual:?} ({why})"),
            )),
        }
    }
}

fn apply(c: &mut Construction, step: &Step) -> Result<Option<geo_kernel::Id>, String> {
    match step {
        Step::Initial | Step::Append { .. } => Ok(None),
        Step::AnchorText { label, point } => script::exec(
            c,
            &format!("$textProperties({label}, -, -, -, -, -, point, {point})"),
        ),
        Step::Delete { .. }
        | Step::ReloadGgb
        | Step::SliderAnimation { .. }
        | Step::Undo
        | Step::Redo
        | Step::Reload => {
            unreachable!("document step")
        }
        Step::Move { label, x, y } => {
            let id = c.id(label).ok_or(format!("unknown object `{label}`"))?;
            c.move_point(id, *x, *y)
                .map(|recomputed| (recomputed > 0).then_some(id))
                .map_err(|e| format!("move `{label}`: {e}"))
        }
        Step::SetNumber { label, value } => {
            let id = c.id(label).ok_or(format!("unknown object `{label}`"))?;
            c.set_number(id, *value)
                .map(|recomputed| (recomputed > 0).then_some(id))
                .map_err(|e| format!("set `{label}`: {e}"))
        }
        Step::Add { line } => {
            if script::assignment_target(c, line).is_some() {
                return Err(format!("expected an assignment to a new label: {line}"));
            }
            script::exec(c, line)?;
            Ok(None)
        }
        Step::Redefine { line } => {
            let id = script::assignment_target(c, line)
                .ok_or_else(|| format!("expected an assignment to an existing label: {line}"))?;
            match script::exec(c, line)? {
                Some(replaced) if replaced == id => Ok(Some(id)),
                _ => Err(format!("expected one replacement: {line}")),
            }
        }
        Step::SetSliderBounds { label, min, max } => {
            script::saved::Property::SliderBounds {
                label: label.clone(),
                bounds: [Some(min.to_string()), Some(max.to_string())],
            }
            .apply(c)?;
            Ok(None)
        }
        Step::Animate { frames } => {
            for _ in 0..*frames {
                c.animate(geo_kernel::kernel::animation::MAX_ANIMATION_FRAME_RATE);
            }
            Ok(None)
        }
        Step::Reject { line, error } => {
            let len = c.len();
            match script::exec(c, line) {
                Err(e) if c.len() == len => match error {
                    Some(want) if *want != e => Err(format!(
                        "expected the rejection `{want}` of `{line}`, got `{e}`"
                    )),
                    _ => Ok(None),
                },
                _ => Err(format!("expected rejection without new objects: {line}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_file;

    #[test]
    fn a_rejection_with_a_stated_error_checks_it() {
        let fixture = |error: &str| {
            format!(
                r#"{{"schema":1,"generator":"ggb-harness","upstream_commit":"c",
                "cases":[{{"id":"r","group":"g","script":["a = 1"],
                "steps":[{{"action":"initial"}},{{"action":"reject","line":"b = Foo(1)","error":"{error}"}}],
                "snapshots":[{{"a":{{"type":"number","defined":true,"value":1.0}}}},
                {{"a":{{"type":"number","defined":true,"value":1.0}}}}]}}]}}"#
            )
        };
        let wrong = parse_file(&fixture("no such error"), "t").unwrap();
        let outcome = replay(&wrong.cases[0]);
        assert_eq!(outcome.mismatches.len(), 1);
        let actual = outcome.mismatches[0].actual.clone();
        let error = actual
            .split(", got `")
            .nth(1)
            .and_then(|rest| rest.strip_suffix('`'))
            .unwrap()
            .replace('`', "\\u0060");
        let right = parse_file(&fixture(&error), "t").unwrap();
        assert!(replay(&right.cases[0]).mismatches.is_empty());
    }

    #[test]
    fn only_legacy_files_accept_an_angle_as_a_number() {
        let fixture = r#"{"schema":1,"generator":"format-sample","upstream_commit":null,
          "cases":[{"id":"sample","group":"expr","script":["a = 0°"],
          "steps":[{"action":"initial"}],
          "snapshots":[{"a":{"type":"number","defined":true,"value":0}}]}]}"#;
        let legacy = parse_file(fixture, "legacy example").unwrap();
        assert!(replay(&legacy.cases[0]).mismatches.is_empty());

        let current = fixture.replace("\"schema\":1", "\"schema\":2");
        let current = parse_file(&current, "current example").unwrap();
        let outcome = replay(&current.cases[0]);
        assert_eq!(outcome.mismatches.len(), 1);
        assert_eq!(outcome.mismatches[0].label, "a");
        assert!(outcome.mismatches[0]
            .actual
            .contains("type mismatch: expected number, got angle"));

        let typed = fixture
            .replace("\"schema\":1", "\"schema\":2")
            .replace("\"type\":\"number\"", "\"type\":\"angle\"");
        let typed = parse_file(&typed, "angle example").unwrap();
        assert!(replay(&typed.cases[0]).mismatches.is_empty());
    }
}
