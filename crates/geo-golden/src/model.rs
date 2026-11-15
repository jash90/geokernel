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
    Point {
        x: f64,
        y: f64,
    },
    Line {
        a: f64,
        b: f64,
        c: f64,
    },
    Segment {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
    },
    Number(f64),
    Vector {
        x: f64,
        y: f64,
    },
    /// The value of a boolean.
    Bool(bool),
    /// Start point `(x, y)` and raw line coefficients.
    Ray {
        x: f64,
        y: f64,
        a: f64,
        b: f64,
        c: f64,
    },
    /// Vertices in order, area (never negative) and signed area (positive
    /// counter-clockwise).
    Polygon {
        vertices: Vec<(f64, f64)>,
        area: f64,
        signed_area: f64,
    },
    /// Vertices in order, `None` for an undefined point, and length.
    PolyLine {
        vertices: Vec<Option<(f64, f64)>>,
        length: f64,
    },
}

#[derive(Deserialize)]
struct RawObject {
    #[serde(rename = "type")]
    ty: String,
    defined: bool,
    #[serde(flatten)]
    fields: BTreeMap<String, serde_json::Value>,
}

/// A number field.
fn number(name: &str, v: &serde_json::Value) -> Result<f64, String> {
    v.as_f64()
        .ok_or_else(|| format!("field `{name}` must be a number, got {v}"))
}

/// A vertex `[x, y]`; `None` for `null` when `holes` allows it.
fn vertex(v: &serde_json::Value, holes: bool) -> Result<Option<(f64, f64)>, String> {
    if v.is_null() && holes {
        return Ok(None);
    }
    match v.as_array().map(Vec::as_slice) {
        Some([x, y]) => Ok(Some((number("x", x)?, number("y", y)?))),
        _ => Err(format!("a vertex must be [x, y], got {v}")),
    }
}

/// A list of vertices.
fn vertices(v: &serde_json::Value, holes: bool) -> Result<Vec<Option<(f64, f64)>>, String> {
    v.as_array()
        .ok_or_else(|| format!("`vertices` must be a list, got {v}"))?
        .iter()
        .map(|p| vertex(p, holes))
        .collect()
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
            "vector" => (Kind::Vector, &["x", "y"]),
            "ray" => (Kind::Ray, &["x", "y", "a", "b", "c"]),
            "boolean" => (Kind::Boolean, &["value"]),
            "polygon" => (Kind::Polygon, &["vertices", "area", "signed_area"]),
            "polyline" => (Kind::PolyLine, &["vertices", "length"]),
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
        if kind == Kind::Boolean {
            return match raw.fields["value"].as_bool() {
                Some(b) => Ok(Expected {
                    kind,
                    data: Some(ExpectedData::Bool(b)),
                }),
                None => Err("a boolean's `value` must be true or false".to_string()),
            };
        }
        if kind == Kind::Polygon {
            let vertices = vertices(&raw.fields["vertices"], false)?;
            return Ok(Expected {
                kind,
                data: Some(ExpectedData::Polygon {
                    vertices: vertices.into_iter().flatten().collect(),
                    area: number("area", &raw.fields["area"])?,
                    signed_area: number("signed_area", &raw.fields["signed_area"])?,
                }),
            });
        }
        if kind == Kind::PolyLine {
            return Ok(Expected {
                kind,
                data: Some(ExpectedData::PolyLine {
                    vertices: vertices(&raw.fields["vertices"], true)?,
                    length: number("length", &raw.fields["length"])?,
                }),
            });
        }
        let mut nums = BTreeMap::new();
        for (name, v) in &raw.fields {
            nums.insert(name.as_str(), number(name, v)?);
        }
        let f = |n: &str| nums[n];
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
            Kind::Vector => ExpectedData::Vector {
                x: f("x"),
                y: f("y"),
            },
            Kind::Ray => ExpectedData::Ray {
                x: f("x"),
                y: f("y"),
                a: f("a"),
                b: f("b"),
                c: f("c"),
            },
            Kind::Boolean => unreachable!("handled above"),
            // `Kind` is non-exhaustive: a kind without a golden type yet.
            other => return Err(format!("no golden data format for {other:?}")),
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
    let mut first: Option<BTreeSet<String>> = None;
    for (i, snap) in case.snapshots.iter().enumerate() {
        let keys: BTreeSet<String> = snap.keys().cloned().collect();
        match &first {
            None => {
                // The sides and new vertices of a polygon get labels of
                // their own (see `golden/SCHEMA.md`).
                let names = PolygonNames::of(&case.script, |label| {
                    snap.get(label).map(|object| object.kind)
                });
                let created = |key: &str| match snap[key].kind {
                    Kind::Point => names.point(key),
                    Kind::Segment => names.segment(key),
                    _ => false,
                };
                check_keys(&keys, &labels, created).map_err(|e| format!("snapshot {i}: {e}"))?;
                first = Some(keys);
            }
            Some(f) if *f != keys => {
                return Err(format!("snapshot {i} labels differ from snapshot 0"));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// The part of a label before its first `_`, which GeoGebra numbers when a
/// command creates several objects (`t` -> `t_{1}`, `t_{2}`).
fn label_prefix(label: &str) -> &str {
    label.split('_').next().unwrap_or(label)
}

/// `Some(prefix)` when `key` has the form `prefix_{n}` with `n` a positive
/// integer written without leading zeros.
fn indexed_prefix(key: &str) -> Option<&str> {
    let (prefix, rest) = key.split_once("_{")?;
    let n = rest.strip_suffix('}')?;
    let digits = !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit());
    (digits && !n.starts_with('0') && !prefix.is_empty() && !prefix.contains('_')).then_some(prefix)
}

/// `LabelType.pointLabels`: the default labels of new points.
const POINT_LABELS: &[&str] = &[
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "Z",
];

/// `LabelType.lineLabels`: the default labels of new segments.
const LINE_LABELS: &[&str] = &[
    "f", "g", "h", "i", "j", "k", "l", "m", "n", "p", "q", "r", "s", "t", "a", "b", "c", "d", "e",
];

/// The labels the `Polygon(...)` lines of a script can give to the objects
/// they create on their own:
///
/// - `Polygon(A, B, n)` with `n` a number (a literal, or a label recorded
///   as a number): new vertices from the default point labels and sides
///   from the default segment labels, each optionally indexed;
/// - any other `Polygon(...)`: sides named after a vertex, the vertex label
///   in lower case, indexed when it is one character, else the default
///   segment labels.
#[derive(Default)]
struct PolygonNames {
    points: BTreeSet<String>,
    segments: BTreeSet<String>,
    /// Bases that also take `_<digits>` or `_{<digits>}`.
    indexed_points: BTreeSet<String>,
    indexed_segments: BTreeSet<String>,
}

impl PolygonNames {
    fn of(script: &[String], kind: impl Fn(&str) -> Option<Kind>) -> PolygonNames {
        let mut names = PolygonNames::default();
        for line in script {
            let Some(args) = polygon_args(line) else {
                continue;
            };
            let number = |arg: &str| match kind(arg) {
                Some(k) => k == Kind::Number,
                None => arg.parse::<f64>().is_ok(),
            };
            let point = |arg: &str| kind(arg) == Some(Kind::Point);
            if args.len() == 3 && point(args[0]) && point(args[1]) && number(args[2]) {
                names
                    .indexed_points
                    .extend(POINT_LABELS.iter().map(|l| l.to_string()));
                names.default_segments();
                continue;
            }
            if args.len() < 2 {
                continue;
            }
            for vertex in &args {
                let lower = vertex.to_lowercase();
                if lower.encode_utf16().count() == 1 {
                    names.indexed_segments.insert(lower);
                } else {
                    names.segments.insert(lower);
                    names.default_segments();
                }
            }
        }
        names
    }

    fn default_segments(&mut self) {
        self.indexed_segments
            .extend(LINE_LABELS.iter().map(|l| l.to_string()));
    }

    fn point(&self, key: &str) -> bool {
        self.points.contains(key) || matches_indexed(key, &self.indexed_points)
    }

    fn segment(&self, key: &str) -> bool {
        self.segments.contains(key) || matches_indexed(key, &self.indexed_segments)
    }
}

/// Whether `key` is one of `bases`, or one of them followed by `_<n>` or
/// `_{<n>}` with `n` a positive integer without leading zeros.
fn matches_indexed(key: &str, bases: &BTreeSet<String>) -> bool {
    if bases.contains(key) {
        return true;
    }
    let Some((base, index)) = key.split_once('_') else {
        return false;
    };
    let n = index
        .strip_prefix('{')
        .and_then(|i| i.strip_suffix('}'))
        .unwrap_or(index);
    let digits = !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && !n.starts_with('0');
    digits && bases.contains(base)
}

/// The arguments of a script line `name = Polygon(a, b, ...)`.
fn polygon_args(line: &str) -> Option<Vec<&str>> {
    let (_, rhs) = line.split('#').next()?.split_once('=')?;
    let inner = rhs.trim().strip_prefix("Polygon(")?.strip_suffix(')')?;
    Some(inner.split(',').map(str::trim).collect())
}

/// Snapshot keys must match the script labels: each label is either a key
/// itself, or (for a command with several outputs) is replaced by one or
/// more keys `<prefix>_{n}`. Any other key must be an object created by a
/// line on its own (`created`).
fn check_keys(
    keys: &BTreeSet<String>,
    labels: &BTreeSet<String>,
    created: impl Fn(&str) -> bool,
) -> Result<(), String> {
    let replaced: BTreeSet<&str> = labels
        .iter()
        .filter(|l| !keys.contains(*l))
        .map(|l| label_prefix(l))
        .collect();
    let mut used = BTreeSet::new();
    for key in keys.iter().filter(|k| !labels.contains(*k)) {
        match indexed_prefix(key) {
            Some(p) if replaced.contains(p) => {
                used.insert(p);
            }
            _ if created(key) => {}
            _ => return Err(format!("unexpected label `{key}`")),
        }
    }
    if let Some(l) = labels
        .iter()
        .find(|l| !keys.contains(*l) && !used.contains(label_prefix(l)))
    {
        return Err(format!("no label for script object `{l}`"));
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

    #[test]
    fn booleans_carry_one_truth_value() {
        let parse = |json: &str| serde_json::from_str::<Expected>(json);
        let t = parse(r#"{"type":"boolean","defined":true,"value":true}"#).unwrap();
        assert_eq!(t.kind, Kind::Boolean);
        assert_eq!(t.data, Some(ExpectedData::Bool(true)));
        let f = parse(r#"{"type":"boolean","defined":true,"value":false}"#).unwrap();
        assert_eq!(f.data, Some(ExpectedData::Bool(false)));
        let u = parse(r#"{"type":"boolean","defined":false}"#).unwrap();
        assert_eq!((u.kind, u.data), (Kind::Boolean, None));
        for bad in [
            // a number instead of a truth value
            r#"{"type":"boolean","defined":true,"value":1.0}"#,
            // an extra field
            r#"{"type":"boolean","defined":true,"value":true,"x":1.0}"#,
            // an undefined boolean carries no value
            r#"{"type":"boolean","defined":false,"value":true}"#,
            // the value is missing
            r#"{"type":"boolean","defined":true}"#,
            // a truth value in a numeric field
            r#"{"type":"number","defined":true,"value":true}"#,
        ] {
            assert!(parse(bad).is_err(), "accepted: {bad}");
        }
    }

    #[test]
    fn vectors_and_rays_carry_their_fields() {
        let parse = |json: &str| serde_json::from_str::<Expected>(json);
        let v = parse(r#"{"type":"vector","defined":true,"x":1.0,"y":-2.0}"#).unwrap();
        assert_eq!(v.kind, Kind::Vector);
        assert_eq!(v.data, Some(ExpectedData::Vector { x: 1.0, y: -2.0 }));
        let r = parse(r#"{"type":"ray","defined":true,"x":1.0,"y":2.0,"a":-4.0,"b":3.0,"c":-2.0}"#)
            .unwrap();
        assert_eq!(
            r.data,
            Some(ExpectedData::Ray {
                x: 1.0,
                y: 2.0,
                a: -4.0,
                b: 3.0,
                c: -2.0
            })
        );
        assert_eq!(
            parse(r#"{"type":"ray","defined":false}"#).unwrap().kind,
            Kind::Ray
        );
        // A ray needs its start point, a vector exactly x and y.
        assert!(parse(r#"{"type":"ray","defined":true,"a":-4.0,"b":3.0,"c":-2.0}"#).is_err());
        assert!(parse(r#"{"type":"vector","defined":true,"x":1.0,"y":2.0,"z":0.0}"#).is_err());
    }

    fn check_keys_(keys: &BTreeSet<String>, labels: &BTreeSet<String>) -> Result<(), String> {
        check_keys(keys, labels, |_| false)
    }

    #[test]
    fn polygons_and_polylines_carry_lists_of_vertices() {
        let parse = |json: &str| serde_json::from_str::<Expected>(json);
        let p = parse(
            r#"{"type":"polygon","defined":true,"vertices":[[0.0,0.0],[4.0,0.0],[0.0,3.0]],
                "area":6.0,"signed_area":6.0}"#,
        )
        .unwrap();
        assert_eq!(
            p.data,
            Some(ExpectedData::Polygon {
                vertices: vec![(0.0, 0.0), (4.0, 0.0), (0.0, 3.0)],
                area: 6.0,
                signed_area: 6.0
            })
        );
        let l = parse(
            r#"{"type":"polyline","defined":true,"vertices":[[0.0,0.0],null,[3.0,4.0]],
                "length":0.0}"#,
        )
        .unwrap();
        assert_eq!(
            l.data,
            Some(ExpectedData::PolyLine {
                vertices: vec![Some((0.0, 0.0)), None, Some((3.0, 4.0))],
                length: 0.0
            })
        );
        assert_eq!(
            parse(r#"{"type":"polygon","defined":false}"#).unwrap().kind,
            Kind::Polygon
        );
        for bad in [
            // a polygon has no holes
            r#"{"type":"polygon","defined":true,"vertices":[null],"area":0.0,"signed_area":0.0}"#,
            r#"{"type":"polygon","defined":true,"vertices":[[1.0]],"area":0.0,"signed_area":0.0}"#,
            r#"{"type":"polygon","defined":true,"vertices":[],"area":0.0}"#,
            r#"{"type":"polygon","defined":true,"vertices":0.0,"area":0.0,"signed_area":0.0}"#,
            r#"{"type":"polyline","defined":true,"vertices":[[0.0,"x"]],"length":0.0}"#,
            r#"{"type":"point","defined":true,"x":[0.0],"y":0.0}"#,
        ] {
            assert!(parse(bad).is_err(), "accepted: {bad}");
        }
    }

    #[test]
    fn polygon_lines_allow_the_labels_of_sides_and_vertices() {
        let file = |script: &str, extra: &str| {
            format!(
                r#"{{"schema":1,"generator":"g","upstream_commit":null,
                  "cases":[{{"id":"x","group":"g","script":["A = (0, 0)",{script}],
                  "steps":[{{"action":"initial"}}],
                  "snapshots":[{{"A":{{"type":"point","defined":true,"x":0.0,"y":0.0}},
                    "p":{{"type":"polygon","defined":false}},{extra}}}]}}]}}"#
            )
        };
        let side = r#""a_1":{"type":"segment","defined":false}"#;
        let point = r#""C":{"type":"point","defined":false}"#;
        let number = r#""n":{"type":"number","defined":false}"#;
        let odd = r#""a_x":{"type":"segment","defined":false}"#;
        let polygon = r#""p = Polygon(A, A)""#;
        let regular = r#""p = Polygon(A, A, 4)""#;
        let other = r#""p = PolyLine(A, A)""#;
        assert!(parse_file(&file(polygon, side), "t").is_ok());
        // a polygon from points creates no point, a regular one may
        assert!(parse_file(&file(polygon, point), "t").is_err());
        assert!(parse_file(&file(regular, point), "t").is_ok());
        // `x_1` is not a default segment label, and no vertex is `X`
        let x_side = r#""x_1":{"type":"segment","defined":false}"#;
        assert!(parse_file(&file(regular, side), "t").is_ok());
        assert!(parse_file(&file(regular, x_side), "t").is_err());
        assert!(parse_file(&file(polygon, x_side), "t").is_err());
        assert!(parse_file(&file(polygon, number), "t").is_err());
        assert!(parse_file(&file(polygon, odd), "t").is_err());
        assert!(parse_file(&file(other, side), "t").is_err());
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn plain_labels_are_accepted() {
        assert!(check_keys_(&set(&["A", "c", "t"]), &set(&["A", "c", "t"])).is_ok());
    }

    #[test]
    fn indexed_outputs_replace_their_label() {
        let labels = set(&["P", "c", "t"]);
        assert!(check_keys_(&set(&["P", "c", "t_{1}", "t_{2}"]), &labels).is_ok());
        assert!(check_keys_(&set(&["P", "c", "t_{1}"]), &labels).is_ok());
        let labels = set(&["P", "c", "b_1", "b"]);
        assert!(check_keys_(&set(&["P", "c", "b_1", "b_{2}", "b_{3}"]), &labels).is_ok());
        // A label that is itself a key keeps it; `t_{1}` written in the
        // script is a plain label.
        assert!(check_keys_(&set(&["t_{1}"]), &set(&["t_{1}"])).is_ok());
    }

    #[test]
    fn unexpected_keys_are_rejected() {
        let labels = set(&["P", "c", "t"]);
        for keys in [
            &["P", "c", "t", "u"][..],
            &["P", "c", "t", "t_{1}"],
            &["P", "c"],
            &["P", "c", "t_{0}"],
            &["P", "c", "t_{01}"],
            &["P", "c", "t_{}"],
            &["P", "c", "t_1"],
            &["P", "c", "t_{x}"],
        ] {
            assert!(
                check_keys_(&set(keys), &labels).is_err(),
                "accepted {keys:?}"
            );
        }
    }

    #[test]
    fn polygon_lines_allow_only_the_labels_they_can_give() {
        let script = |lines: &[&str]| lines.iter().map(|l| l.to_string()).collect::<Vec<_>>();
        let kinds = |label: &str| match label {
            "A" | "B" | "C" | "Pa" | "inf" => Some(Kind::Point),
            "n" => Some(Kind::Number),
            _ => None,
        };
        // a polygon from points: sides named after its vertices
        let names = PolygonNames::of(&script(&["p = Polygon(A, B, C)"]), kinds);
        for key in ["a", "b", "c", "a_1", "c_{12}"] {
            assert!(names.segment(key), "{key}");
        }
        for key in ["f", "d", "a_0", "a_{01}", "ab", "a_x"] {
            assert!(!names.segment(key), "{key}");
        }
        assert!(!names.point("D"));
        // a longer vertex label: itself in lower case, or a default label
        let names = PolygonNames::of(&script(&["p = Polygon(Pa, B)"]), kinds);
        for key in ["pa", "b_1", "f", "e_2"] {
            assert!(names.segment(key), "{key}");
        }
        assert!(!names.segment("pa_1") && !names.segment("u"));
        // a regular polygon: default point and segment labels
        for line in [
            "p = Polygon(A, B, 4)",
            "p = Polygon(A, B, n)",
            "p = Polygon(A, B, -2.5)",
        ] {
            let names = PolygonNames::of(&script(&[line]), kinds);
            for key in ["C", "Z", "D_1", "W_{10}"] {
                assert!(names.point(key), "{line}: {key}");
            }
            assert!(!names.point("X") && !names.point("Y"), "{line}");
            assert!(names.segment("f") && names.segment("e_{3}"), "{line}");
            assert!(!names.segment("o") && !names.segment("u"), "{line}");
        }
        // a recorded label wins over a number word
        let names = PolygonNames::of(&script(&["p = Polygon(A, B, inf)"]), kinds);
        assert!(!names.point("C") && names.segment("inf") && names.segment("f"));
        assert!(!names.segment("u_1") && !names.segment("in"));
        // `Polygon(A)` and other commands create nothing
        let names = PolygonNames::of(&script(&["p = Polygon(A)", "s = Segment(A, B)"]), kinds);
        assert!(!names.segment("a") && !names.segment("f"));
    }

    #[test]
    fn indexed_keys_need_a_matching_script_label() {
        assert!(check_keys_(&set(&["P", "c", "s_{1}"]), &set(&["P", "c", "t"])).is_err());
        // Both labels are present, so nothing was replaced.
        assert!(check_keys_(&set(&["P", "c", "c_{1}"]), &set(&["P", "c"])).is_err());
    }

    #[test]
    fn every_snapshot_has_the_same_labels() {
        let file = r#"{"schema":1,"generator":"g","upstream_commit":null,
          "cases":[{"id":"x","group":"g","script":["A = (0, 0)","t = A"],
          "steps":[{"action":"initial"},{"action":"move","label":"A","x":1.0,"y":0.0}],
          "snapshots":[
            {"A":{"type":"point","defined":true,"x":0.0,"y":0.0},
             "t_{1}":{"type":"point","defined":true,"x":0.0,"y":0.0},
             "t_{2}":{"type":"point","defined":true,"x":0.0,"y":0.0}},
            {"A":{"type":"point","defined":true,"x":1.0,"y":0.0},
             "t_{1}":{"type":"point","defined":true,"x":1.0,"y":0.0}}]}]}"#;
        assert!(parse_file(file, "t").is_err());
        let fixed = file.replace(
            r#""t_{1}":{"type":"point","defined":true,"x":1.0,"y":0.0}}"#,
            r#""t_{1}":{"type":"point","defined":true,"x":1.0,"y":0.0},
               "t_{2}":{"type":"point","defined":true,"x":1.0,"y":0.0}}"#,
        );
        assert!(parse_file(&fixed, "t").is_ok());
    }
}
