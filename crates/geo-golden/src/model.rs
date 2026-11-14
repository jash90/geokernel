//! Golden file model, parsing and validation.

use geo_kernel::Kind;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    pub schema: u32,
    pub generator: String,
    pub upstream_commit: Option<String>,
    pub cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub group: String,
    pub script: Vec<String>,
    pub steps: Vec<Step>,
    pub snapshots: Vec<BTreeMap<String, Expected>>,
    /// Generator of the file this case came from. Filled by the loader.
    #[serde(skip)]
    pub generator: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Initial,
    Move { label: String, x: f64, y: f64 },
    SetNumber { label: String, value: f64 },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStep {
    action: String,
    label: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
    value: Option<f64>,
}

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let raw = RawStep::deserialize(d)?;
        let need = |name: &str| D::Error::custom(format!("step `{}` needs `{name}`", raw.action));
        match raw.action.as_str() {
            "initial" => {
                if raw.label.is_some() || raw.x.is_some() || raw.y.is_some() || raw.value.is_some()
                {
                    return Err(D::Error::custom("step `initial` takes no fields"));
                }
                Ok(Step::Initial)
            }
            "move" => {
                if raw.value.is_some() {
                    return Err(D::Error::custom("step `move` takes no `value`"));
                }
                Ok(Step::Move {
                    label: raw.label.clone().ok_or_else(|| need("label"))?,
                    x: raw.x.ok_or_else(|| need("x"))?,
                    y: raw.y.ok_or_else(|| need("y"))?,
                })
            }
            "set_number" => {
                if raw.x.is_some() || raw.y.is_some() {
                    return Err(D::Error::custom("step `set_number` takes no `x`/`y`"));
                }
                Ok(Step::SetNumber {
                    label: raw.label.clone().ok_or_else(|| need("label"))?,
                    value: raw.value.ok_or_else(|| need("value"))?,
                })
            }
            other => Err(D::Error::custom(format!("unknown action `{other}`"))),
        }
    }
}

/// An object as recorded in a snapshot.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "RawObject")]
pub struct Expected {
    pub kind: Kind,
    /// `None` when the object is undefined.
    pub data: Option<ExpectedData>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpectedData {
    Point { x: f64, y: f64 },
    Line { a: f64, b: f64, c: f64 },
    Segment { x1: f64, y1: f64, x2: f64, y2: f64 },
    Circle { cx: f64, cy: f64, r: f64 },
    Number(f64),
}

#[derive(Deserialize)]
struct RawObject {
    #[serde(rename = "type")]
    ty: String,
    defined: bool,
    #[serde(flatten)]
    fields: BTreeMap<String, f64>,
}

impl TryFrom<RawObject> for Expected {
    type Error = String;

    fn try_from(raw: RawObject) -> Result<Self, String> {
        let (kind, names): (Kind, &[&str]) = match raw.ty.as_str() {
            "point" => (Kind::Point, &["x", "y"]),
            "line" => (Kind::Line, &["a", "b", "c"]),
            "segment" => (Kind::Segment, &["x1", "y1", "x2", "y2"]),
            "circle" => (Kind::Circle, &["cx", "cy", "r"]),
            "number" => (Kind::Number, &["value"]),
            other => return Err(format!("unknown object type `{other}`")),
        };
        if !raw.defined {
            if !raw.fields.is_empty() {
                return Err(format!("undefined {} carries numeric fields", raw.ty));
            }
            return Ok(Expected { kind, data: None });
        }
        let expected: BTreeSet<&str> = names.iter().copied().collect();
        let got: BTreeSet<&str> = raw.fields.keys().map(String::as_str).collect();
        if expected != got {
            return Err(format!(
                "defined {} must carry exactly {names:?}, got {got:?}",
                raw.ty
            ));
        }
        let f = |n: &str| raw.fields[n];
        let data = match kind {
            Kind::Point => ExpectedData::Point {
                x: f("x"),
                y: f("y"),
            },
            Kind::Line => ExpectedData::Line {
                a: f("a"),
                b: f("b"),
                c: f("c"),
            },
            Kind::Segment => ExpectedData::Segment {
                x1: f("x1"),
                y1: f("y1"),
                x2: f("x2"),
                y2: f("y2"),
            },
            Kind::Circle => ExpectedData::Circle {
                cx: f("cx"),
                cy: f("cy"),
                r: f("r"),
            },
            Kind::Number => ExpectedData::Number(f("value")),
        };
        Ok(Expected {
            kind,
            data: Some(data),
        })
    }
}

/// Labels created by a script: the text before the first `=` of each line.
pub fn script_labels(script: &[String]) -> BTreeSet<String> {
    script
        .iter()
        .filter_map(|l| {
            l.split('#')
                .next()?
                .split_once('=')
                .map(|(n, _)| n.trim().to_string())
        })
        .collect()
}

/// Parses and validates one file. `source` is only used in messages.
pub fn parse_file(text: &str, source: &str) -> Result<File, String> {
    let mut file: File =
        serde_json::from_str(text).map_err(|e| format!("{source}: malformed JSON: {e}"))?;
    if file.schema != 1 {
        return Err(format!("{source}: unsupported schema {}", file.schema));
    }
    for case in &mut file.cases {
        case.generator = file.generator.clone();
        validate_case(case).map_err(|e| format!("{source}: case `{}`: {e}", case.id))?;
    }
    Ok(file)
}

fn validate_case(case: &Case) -> Result<(), String> {
    if case.id.is_empty() {
        return Err("empty id".into());
    }
    if case.steps.first() != Some(&Step::Initial) {
        return Err("the first step must be `initial`".into());
    }
    if case.steps.iter().skip(1).any(|s| *s == Step::Initial) {
        return Err("`initial` may only be the first step".into());
    }
    if case.steps.len() != case.snapshots.len() {
        return Err(format!(
            "{} steps but {} snapshots",
            case.steps.len(),
            case.snapshots.len()
        ));
    }
    let labels = script_labels(&case.script);
    if labels.len() != case.script.len() {
        return Err("script lines must each define a distinct `label = ...`".into());
    }
    for (i, snap) in case.snapshots.iter().enumerate() {
        let keys: BTreeSet<String> = snap.keys().cloned().collect();
        if keys != labels {
            return Err(format!("snapshot {i} labels differ from the script labels"));
        }
    }
    Ok(())
}

/// Every golden file: all `*.json` under `<root>/golden/` plus the fixture.
pub fn collect_paths(root: &Path) -> Result<Vec<PathBuf>, String> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                walk(&path, out)?;
            } else if path.extension().is_some_and(|e| e == "json") {
                out.push(path);
            }
        }
        Ok(())
    }
    let mut paths = Vec::new();
    let golden = root.join("golden");
    if golden.is_dir() {
        walk(&golden, &mut paths)?;
    }
    paths.push(root.join("crates/geo-golden/tests/fixtures/format-sample.json"));
    paths.sort();
    Ok(paths)
}

/// Loads every golden file. Any malformed file or duplicate id is an error;
/// all problems are reported together.
pub fn load_all(root: &Path) -> Result<Vec<Case>, String> {
    let mut cases = Vec::new();
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    for path in collect_paths(root)? {
        let shown = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("{shown}: {e}"));
                continue;
            }
        };
        match parse_file(&text, &shown) {
            Ok(file) => {
                let dir = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str());
                for case in file.cases {
                    if !seen.insert(case.id.clone()) {
                        errors.push(format!("{shown}: duplicate case id `{}`", case.id));
                    }
                    if file.generator == "ggb-harness" && dir != Some(case.group.as_str()) {
                        errors.push(format!(
                            "{shown}: case `{}` has group `{}` but lives in `{}`",
                            case.id,
                            case.group,
                            dir.unwrap_or("?")
                        ));
                    }
                    cases.push(case);
                }
            }
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() {
        Ok(cases)
    } else {
        Err(errors.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{"schema":1,"generator":"format-sample","upstream_commit":null,
      "cases":[{"id":"x","group":"g","script":["A = (0, 0)"],
      "steps":[{"action":"initial"}],
      "snapshots":[{"A":{"type":"point","defined":true,"x":0.0,"y":0.0}}]}]}"#;

    #[test]
    fn accepts_a_valid_file() {
        assert_eq!(parse_file(GOOD, "t").unwrap().cases.len(), 1);
    }

    #[test]
    fn malformed_files_fail_loudly() {
        for bad in [
            "not json",
            &GOOD.replace("\"schema\":1", "\"schema\":2"),
            &GOOD.replace("\"y\":0.0", "\"z\":0.0"),
            &GOOD.replace("\"initial\"", "\"teleport\""),
            &GOOD.replace("\"A\":{", "\"B\":{"),
            &GOOD.replace("\"snapshots\":[", "\"snapshots\":[{},"),
            &GOOD.replace("\"cases\"", "\"extra\":1,\"cases\""),
            &GOOD.replace(
                "\"defined\":true,\"x\":0.0,\"y\":0.0",
                "\"defined\":false,\"x\":0.0",
            ),
        ] {
            assert!(parse_file(bad, "t").is_err(), "accepted: {bad}");
        }
    }
}
