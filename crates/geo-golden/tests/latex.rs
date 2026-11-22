//! Run the renderer on formula strings emitted by the Java harness.

use std::collections::BTreeSet;
use std::{fs, path::Path};

use serde_json::Value;

#[test]
fn all_java_formula_strings_parse_without_fallback() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden");
    let mut seen = BTreeSet::new();
    walk(&root, &mut seen);
    for line in fs::read_to_string(root.join("t-pairs/verified.jsonl"))
        .unwrap()
        .lines()
    {
        let record: Value = serde_json::from_str(line).unwrap();
        if record
            .get("input")
            .and_then(Value::as_str)
            .is_some_and(latex_command)
        {
            if let Some(outputs) = record.get("outputs").and_then(Value::as_array) {
                for output in outputs {
                    if let Some(text) = output.get("object").and_then(text_value) {
                        seen.insert(text.to_owned());
                    }
                }
            }
        }
        collect(&record, &mut seen);
    }
    assert!(seen.len() >= 150, "{} distinct formulas", seen.len());
    let mut unsupported = Vec::new();
    for source in &seen {
        let result = geo_latex::layout(source, 20.0, geo_latex::Style::Display, Default::default());
        if result.has_fallback {
            unsupported.push(source);
        }
    }
    assert!(unsupported.is_empty(), "{unsupported:#?}");
    println!("{} distinct Java formula strings", seen.len());
}

fn walk(dir: &Path, seen: &mut BTreeSet<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(&path, seen);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "json" || ext == "snapshot")
        {
            collect(
                &serde_json::from_slice(&fs::read(&path).unwrap()).unwrap(),
                seen,
            );
        }
    }
}

fn collect(value: &Value, seen: &mut BTreeSet<String>) {
    match value {
        Value::Object(fields) => {
            // Old harness records combine both types of text under the same
            // tag. Include markup from any text and literal results from
            // commands whose output is marked as LaTeX.
            if let Some(text) = text_value(value).filter(|text| text.contains('\\')) {
                seen.insert(text.to_owned());
            }
            if let Some(script) = fields.get("script").and_then(Value::as_array) {
                for (label, _) in script
                    .iter()
                    .filter_map(Value::as_str)
                    .filter_map(|line| line.split_once('='))
                    .filter(|(_, expression)| latex_command(expression))
                {
                    let snapshots = fields
                        .get("snapshots")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .chain(fields.get("objects"));
                    for snapshot in snapshots {
                        if let Some(text) = snapshot.get(label.trim()).and_then(text_value) {
                            seen.insert(text.to_owned());
                        }
                    }
                }
            }
            for field in fields.values() {
                collect(field, seen);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect(value, seen);
            }
        }
        _ => {}
    }
}

fn text_value(value: &Value) -> Option<&str> {
    (value.get("type")?.as_str()? == "text").then_some(())?;
    value.get("text")?.as_str()
}

fn latex_command(source: &str) -> bool {
    let Some((command, _)) = source.trim().split_once(['(', '[']) else {
        return false;
    };
    matches!(
        command.trim(),
        "LaTeX"
            | "FormulaText"
            | "RotateText"
            | "VerticalText"
            | "TableText"
            | "SurdText"
            | "FractionText"
            | "ScientificText"
            | "ContinuedFraction"
            | "FrequencyTable"
            | "StemPlot"
    )
}
