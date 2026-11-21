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
    /// A number recorded when the file format also encoded angles as numbers.
    pub(crate) legacy_angle_number: bool,
}

impl Expected {
    fn set_angle_encoding(&mut self, legacy: bool) {
        self.legacy_angle_number = legacy && self.kind == Kind::Number;
        if let Some(ExpectedData::List(elements)) = &mut self.data {
            for element in elements {
                element.set_angle_encoding(legacy);
            }
        }
    }
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
    /// The values at [`SAMPLE_XS`], in that order; `None` where GeoGebra's
    /// value is not finite.
    Function {
        samples: Vec<Option<f64>>,
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
    /// The elements in order, each recorded like an object.
    List(Vec<Expected>),
    /// The points of a locus in order.
    Locus(Vec<LocusPoint>),
    /// A general conic: its type name (`ConicType::name`), its raw matrix
    /// `[A00, A11, A22, A01, A02, A12]` and the parts its type records.
    Conic {
        conic_type: String,
        matrix: [f64; 6],
        parts: ConicParts,
    },
    /// An arc or sector: its value (arc length or area), the type name of
    /// its conic, its orientation (`true` positive) where recorded, and the
    /// numbers of a part of a circle or ellipse.
    Arc {
        value: f64,
        conic_type: String,
        orientation: Option<bool>,
        curve: Option<ArcCurve>,
    },
}

/// A point of a locus: `(x, y)`, `None` for an undefined point, and whether
/// it is joined to the point before it.
pub type LocusPoint = (Option<(f64, f64)>, bool);

/// What an arc or sector of a circle or ellipse records besides its value
/// (`golden/SCHEMA.md`).
#[derive(Debug, Clone, PartialEq)]
pub struct ArcCurve {
    pub start: f64,
    pub end: f64,
    pub arc_length: f64,
    pub area: f64,
    pub midpoint: (f64, f64),
    pub half_axes: (f64, f64),
    pub eigenvectors: [(f64, f64); 2],
}

/// The parts of a general conic that its type records (`golden/SCHEMA.md`).
#[derive(Debug, Clone, PartialEq)]
pub enum ConicParts {
    /// A circle, an ellipse or a hyperbola.
    Central {
        midpoint: (f64, f64),
        half_axes: (f64, f64),
        eigenvectors: [(f64, f64); 2],
    },
    Parabola {
        vertex: (f64, f64),
        p: f64,
        eigenvectors: [(f64, f64); 2],
    },
    SinglePoint {
        point: (f64, f64),
    },
    /// Intersecting, parallel or double lines, raw `[x, y, z]` in GeoGebra's order.
    Lines {
        lines: [[f64; 3]; 2],
    },
    /// An empty conic: the matrix only.
    Empty,
}

/// A list of exactly `N` numbers.
fn numbers<const N: usize>(name: &str, v: &serde_json::Value) -> Result<[f64; N], String> {
    let list = v
        .as_array()
        .filter(|l| l.len() == N)
        .ok_or_else(|| format!("`{name}` must be a list of {N} numbers, got {v}"))?;
    let mut out = [0.0; N];
    for (slot, x) in out.iter_mut().zip(list) {
        *slot = number(name, x)?;
    }
    Ok(out)
}

fn pair(name: &str, v: &serde_json::Value) -> Result<(f64, f64), String> {
    let [x, y] = numbers::<2>(name, v)?;
    Ok((x, y))
}

fn pairs(name: &str, v: &serde_json::Value) -> Result<[(f64, f64); 2], String> {
    match v.as_array().map(Vec::as_slice) {
        Some([a, b]) => Ok([pair(name, a)?, pair(name, b)?]),
        _ => Err(format!("`{name}` must be two pairs, got {v}")),
    }
}

/// The data of a defined conic: the fields its type needs, exactly.
fn conic_data(raw: &RawObject) -> Result<ExpectedData, String> {
    let conic_type = raw
        .fields
        .get("conic_type")
        .and_then(|v| v.as_str())
        .ok_or("a defined conic needs a `conic_type` name")?
        .to_string();
    let parts: &[&str] = match conic_type.as_str() {
        "circle" | "ellipse" | "hyperbola" => &["midpoint", "half_axes", "eigenvectors"],
        "parabola" => &["vertex", "p", "eigenvectors"],
        "single_point" => &["point"],
        "intersecting_lines" | "parallel_lines" | "double_line" => &["lines"],
        "empty" => &[],
        other => return Err(format!("unknown conic type `{other}`")),
    };
    let mut expected: BTreeSet<&str> = parts.iter().copied().collect();
    expected.insert("conic_type");
    expected.insert("matrix");
    let got: BTreeSet<&str> = raw.fields.keys().map(String::as_str).collect();
    if expected != got {
        return Err(format!(
            "defined {conic_type} conic must carry exactly {expected:?}, got {got:?}"
        ));
    }
    let f = |n: &str| &raw.fields[n];
    let parts = match conic_type.as_str() {
        "circle" | "ellipse" | "hyperbola" => ConicParts::Central {
            midpoint: pair("midpoint", f("midpoint"))?,
            half_axes: pair("half_axes", f("half_axes"))?,
            eigenvectors: pairs("eigenvectors", f("eigenvectors"))?,
        },
        "parabola" => ConicParts::Parabola {
            vertex: pair("vertex", f("vertex"))?,
            p: number("p", f("p"))?,
            eigenvectors: pairs("eigenvectors", f("eigenvectors"))?,
        },
        "single_point" => ConicParts::SinglePoint {
            point: pair("point", f("point"))?,
        },
        "empty" => ConicParts::Empty,
        _ => match f("lines").as_array().map(Vec::as_slice) {
            Some([a, b]) => ConicParts::Lines {
                lines: [numbers::<3>("lines", a)?, numbers::<3>("lines", b)?],
            },
            _ => return Err(format!("`lines` must be two triples, got {}", f("lines"))),
        },
    };
    Ok(ExpectedData::Conic {
        conic_type,
        matrix: numbers::<6>("matrix", f("matrix"))?,
        parts,
    })
}

/// The fields of a defined arc or sector, which depend on its conic type.
fn arc_data(raw: &RawObject) -> Result<ExpectedData, String> {
    let conic_type = raw
        .fields
        .get("conic_type")
        .and_then(|v| v.as_str())
        .ok_or("a defined arc or sector needs a `conic_type` name")?
        .to_string();
    let names: &[&str] = match conic_type.as_str() {
        "circle" | "ellipse" => &[
            "value",
            "conic_type",
            "orientation",
            "start",
            "end",
            "arc_length",
            "area",
            "midpoint",
            "half_axes",
            "eigenvectors",
        ],
        "parallel_lines" => &["value", "conic_type", "orientation"],
        "single_point" => &["value", "conic_type"],
        other => return Err(format!("unknown conic type of a part `{other}`")),
    };
    let expected: BTreeSet<&str> = names.iter().copied().collect();
    let got: BTreeSet<&str> = raw.fields.keys().map(String::as_str).collect();
    if expected != got {
        return Err(format!(
            "defined {} of a {conic_type} must carry exactly {expected:?}, got {got:?}",
            raw.ty
        ));
    }
    let f = |n: &str| &raw.fields[n];
    let orientation = match raw.fields.get("orientation") {
        None => None,
        Some(v) => match v.as_str() {
            Some("positive") => Some(true),
            Some("negative") => Some(false),
            _ => {
                return Err(format!(
                    "`orientation` must be positive or negative, got {v}"
                ))
            }
        },
    };
    let curve = if names.contains(&"start") {
        Some(ArcCurve {
            start: number("start", f("start"))?,
            end: number("end", f("end"))?,
            arc_length: number("arc_length", f("arc_length"))?,
            area: number("area", f("area"))?,
            midpoint: pair("midpoint", f("midpoint"))?,
            half_axes: pair("half_axes", f("half_axes"))?,
            eigenvectors: pairs("eigenvectors", f("eigenvectors"))?,
        })
    } else {
        None
    };
    Ok(ExpectedData::Arc {
        value: number("value", f("value"))?,
        conic_type,
        orientation,
        curve,
    })
}

/// Where a function is sampled (`golden/SCHEMA.md`).
pub const SAMPLE_XS: [f64; 11] = [-8.0, -4.0, -2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0, 4.0, 8.0];

#[derive(Deserialize)]
struct RawObject {
    #[serde(rename = "type")]
    ty: String,
    defined: bool,
    /// Only for a function: `[x, y]` pairs, `y` null where not finite.
    samples: Option<Vec<(f64, Option<f64>)>>,
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

/// The points of a locus: `[x, y, lineTo]` each, `x` and `y` both null for
/// an undefined point.
fn locus_points(v: &serde_json::Value) -> Result<Vec<LocusPoint>, String> {
    v.as_array()
        .ok_or_else(|| format!("`points` must be a list, got {v}"))?
        .iter()
        .map(|p| match p.as_array().map(Vec::as_slice) {
            Some([x, y, line_to]) => {
                let line_to = line_to
                    .as_bool()
                    .ok_or_else(|| format!("a locus point needs a boolean lineTo, got {p}"))?;
                let at = match (x.is_null(), y.is_null()) {
                    (true, true) => None,
                    (false, false) => Some((number("x", x)?, number("y", y)?)),
                    _ => return Err(format!("a locus point has one null coordinate: {p}")),
                };
                Ok((at, line_to))
            }
            _ => Err(format!("a locus point must be [x, y, lineTo], got {p}")),
        })
        .collect()
}

/// The samples of a defined function: exactly one per [`SAMPLE_XS`], in order.
fn function_samples(raw: &RawObject) -> Result<ExpectedData, String> {
    if !raw.fields.is_empty() {
        return Err("a function carries no numeric fields".into());
    }
    let pairs = raw
        .samples
        .as_ref()
        .ok_or("defined function needs `samples`")?;
    let xs: Vec<f64> = pairs.iter().map(|p| p.0).collect();
    if xs != SAMPLE_XS {
        return Err(format!(
            "function samples must be at {SAMPLE_XS:?}, got {xs:?}"
        ));
    }
    if pairs.iter().any(|p| p.1.is_some_and(|y| !y.is_finite())) {
        return Err("function samples must be finite or null".into());
    }
    Ok(ExpectedData::Function {
        samples: pairs.iter().map(|p| p.1).collect(),
    })
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
            "angle" => (Kind::Angle, &["value"]),
            "vector" => (Kind::Vector, &["x", "y"]),
            "ray" => (Kind::Ray, &["x", "y", "a", "b", "c"]),
            "boolean" => (Kind::Boolean, &["value"]),
            "function" => (Kind::Function, &[]),
            "polygon" => (Kind::Polygon, &["vertices", "area", "signed_area"]),
            "polyline" => (Kind::PolyLine, &["vertices", "length"]),
            "list" => (Kind::List, &["elements"]),
            "conic" => (Kind::Conic, &[]),
            "arc" => (Kind::Arc, &[]),
            "sector" => (Kind::Sector, &[]),
            "locus" => (Kind::Locus, &["points"]),
            other => return Err(format!("unknown object type `{other}`")),
        };
        if raw.samples.is_some() && (kind != Kind::Function || !raw.defined) {
            return Err(format!(
                "only a defined function carries samples, not this {}",
                raw.ty
            ));
        }
        if !raw.defined {
            if !raw.fields.is_empty() {
                return Err(format!("undefined {} carries numeric fields", raw.ty));
            }
            return Ok(Expected {
                kind,
                data: None,
                legacy_angle_number: false,
            });
        }
        if kind == Kind::Function {
            return Ok(Expected {
                legacy_angle_number: false,
                kind,
                data: Some(function_samples(&raw)?),
            });
        }
        if kind == Kind::Conic {
            return Ok(Expected {
                legacy_angle_number: false,
                kind,
                data: Some(conic_data(&raw)?),
            });
        }
        if matches!(kind, Kind::Arc | Kind::Sector) {
            return Ok(Expected {
                legacy_angle_number: false,
                kind,
                data: Some(arc_data(&raw)?),
            });
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
                    legacy_angle_number: false,
                    kind,
                    data: Some(ExpectedData::Bool(b)),
                }),
                None => Err("a boolean's `value` must be true or false".to_string()),
            };
        }
        if kind == Kind::Polygon {
            let vertices = vertices(&raw.fields["vertices"], false)?;
            return Ok(Expected {
                legacy_angle_number: false,
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
                legacy_angle_number: false,
                kind,
                data: Some(ExpectedData::PolyLine {
                    vertices: vertices(&raw.fields["vertices"], true)?,
                    length: number("length", &raw.fields["length"])?,
                }),
            });
        }
        if kind == Kind::Locus {
            return Ok(Expected {
                legacy_angle_number: false,
                kind,
                data: Some(ExpectedData::Locus(locus_points(&raw.fields["points"])?)),
            });
        }
        if kind == Kind::List {
            let elements = raw.fields["elements"]
                .as_array()
                .ok_or("a list's `elements` must be an array")?
                .iter()
                .map(|e| Expected::deserialize(e).map_err(|err| format!("list element: {err}")))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(Expected {
                legacy_angle_number: false,
                kind,
                data: Some(ExpectedData::List(elements)),
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
            Kind::Number | Kind::Angle => ExpectedData::Number(f("value")),
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
            legacy_angle_number: false,
            kind,
            data: Some(data),
        })
    }
}

/// Labels created by a script: the text before the first `=` of each line,
/// without the `(x)` of a function.
pub fn script_labels(script: &[String]) -> BTreeSet<String> {
    script
        .iter()
        .filter_map(|l| {
            l.split('#')
                .next()?
                .split_once('=')
                .map(|(n, _)| line_label(n))
        })
        .collect()
}

/// The label of a script line's left-hand side: `f` for `f(x)`.
pub fn line_label(lhs: &str) -> String {
    let lhs = lhs.trim();
    lhs.strip_suffix("(x)")
        .unwrap_or(lhs)
        .trim_end()
        .to_string()
}

fn legacy_angle_numbers(schema: u32) -> Result<bool, String> {
    match schema {
        1 => Ok(true),
        2 => Ok(false),
        _ => Err(format!("unsupported schema {schema}")),
    }
}

/// Apply the file's angle encoding to a snapshot read outside a replay case,
/// such as a snapshot paired with an exported construction.
pub fn set_snapshot_schema(
    schema: u32,
    snapshot: &mut BTreeMap<String, Expected>,
) -> Result<(), String> {
    let legacy = legacy_angle_numbers(schema)?;
    for expected in snapshot.values_mut() {
        expected.set_angle_encoding(legacy);
    }
    Ok(())
}

/// Parses and validates one file. `source` is only used in messages.
pub fn parse_file(text: &str, source: &str) -> Result<File, String> {
    let mut file: File =
        serde_json::from_str(text).map_err(|e| format!("{source}: malformed JSON: {e}"))?;
    legacy_angle_numbers(file.schema).map_err(|e| format!("{source}: {e}"))?;
    for case in &mut file.cases {
        case.generator = file.generator.clone();
        for snapshot in &mut case.snapshots {
            set_snapshot_schema(file.schema, snapshot).map_err(|e| format!("{source}: {e}"))?;
        }
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
                let directrices = directrix_labels(&case.script);
                let mut outputs = labels.clone();
                let mut clip_prefixes = BTreeSet::new();
                let mut rotated_prefixes = BTreeSet::new();
                for line in &case.script {
                    if let Some((_, rhs)) = line.split('#').next().and_then(|s| s.split_once('=')) {
                        for name in rhs.split(|ch: char| !ch.is_alphanumeric() && ch != '_') {
                            if matches!(name, "xAxis" | "yAxis") {
                                outputs.insert(name.to_string());
                            }
                        }
                    }
                    if let Some((label, "Difference" | "IntersectPath", _)) = command_args(line) {
                        clip_prefixes.insert(label_prefix(label));
                    }
                    if let Some((label, "Angle", args)) = command_args(line) {
                        match args.as_slice() {
                            [arg] if snap.get(*arg).is_some_and(|o| o.kind == Kind::Angle) => {
                                outputs.remove(label);
                            }
                            [from, center, by]
                                if [from, center].iter().all(|&&p| {
                                    snap.get(p).is_some_and(|o| o.kind == Kind::Point)
                                }) && snap.get(*by).is_none_or(|o| o.kind.is_number()) =>
                            {
                                rotated_prefixes.insert(format!("{from}'"));
                            }
                            _ => {}
                        }
                    }
                    if let Some((label, "Segment", args)) = command_args(line) {
                        if let [a, b, polygon] = args.as_slice() {
                            if [a, b]
                                .iter()
                                .all(|&&p| snap.get(p).is_some_and(|o| o.kind == Kind::Point))
                                && snap.get(*polygon).is_some_and(|o| o.kind == Kind::Polygon)
                            {
                                outputs.remove(label);
                            }
                        }
                    }
                }
                let created = |key: &str| {
                    if indexed_prefix(key).is_some_and(|p| clip_prefixes.contains(p))
                        && matches!(snap[key].kind, Kind::Segment | Kind::Polygon)
                    {
                        return true;
                    }
                    match snap[key].kind {
                        Kind::Point => {
                            names.point(key) || rotated_prefixes.contains(label_prefix(key))
                        }
                        Kind::Segment => names.segment(key),
                        // `Directrix` gives both lines the script label, so the
                        // second one gets an index of it.
                        Kind::Line => indexed_prefix(key).is_some_and(|p| directrices.contains(p)),
                        _ => false,
                    }
                };
                check_keys(&keys, &outputs, created).map_err(|e| format!("snapshot {i}: {e}"))?;
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
            let Some((_, command, args)) = command_args(line) else {
                continue;
            };
            let number = |arg: &str| match kind(arg) {
                Some(k) => k.is_number(),
                None => arg.parse::<f64>().is_ok(),
            };
            let point = |arg: &str| kind(arg) == Some(Kind::Point);
            if command == "Difference"
                || command == "IntersectPath"
                    && args
                        .iter()
                        .filter(|&&arg| kind(arg) == Some(Kind::Polygon))
                        .count()
                        == 2
            {
                names.default_points();
                names.default_segments();
                continue;
            }
            if command == "Angle"
                && args.len() == 3
                && point(args[0])
                && point(args[1])
                && kind(args[2]).is_none_or(Kind::is_number)
                && (args[0].contains('_') || args[0].ends_with("'''"))
            {
                names.default_points();
            }
            if command == "Segment" {
                if args.len() == 2 && point(args[0]) && number(args[1]) {
                    names.default_points();
                }
                continue;
            }
            if command == "Polygon"
                && args.len() == 3
                && point(args[0])
                && point(args[1])
                && number(args[2])
            {
                names.default_points();
                names.default_segments();
                continue;
            }
            if !matches!(command, "Polygon" | "RigidPolygon") {
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

    fn default_points(&mut self) {
        self.indexed_points
            .extend(POINT_LABELS.iter().map(|l| l.to_string()));
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

/// The labels of the script lines `name = Directrix(...)`.
fn directrix_labels(script: &[String]) -> BTreeSet<&str> {
    script
        .iter()
        .filter_map(|line| {
            let (label, rhs) = line.split('#').next()?.split_once('=')?;
            rhs.trim().starts_with("Directrix(").then_some(label.trim())
        })
        .collect()
}

/// The label, command and arguments of a script line.
fn command_args(line: &str) -> Option<(&str, &str, Vec<&str>)> {
    let (label, rhs) = line.split('#').next()?.split_once('=')?;
    let (name, inner) = rhs.trim().split_once('(')?;
    let args = inner.strip_suffix(')')?.split(',').map(str::trim).collect();
    Some((label.trim(), name.trim(), args))
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
            &GOOD.replace("\"schema\":1", "\"schema\":3"),
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

    #[test]
    fn functions_carry_their_samples() {
        let parse = |json: &str| serde_json::from_str::<Expected>(json);
        let pairs = |ys: &[&str]| -> String {
            let xs = SAMPLE_XS
                .iter()
                .zip(ys)
                .map(|(x, y)| format!("[{x:?}, {y}]"));
            format!(
                r#"{{"type":"function","defined":true,"samples":[{}]}}"#,
                xs.collect::<Vec<_>>().join(",")
            )
        };
        let mut ys = vec!["1.5"; 11];
        ys[3] = "null";
        let f = parse(&pairs(&ys)).unwrap();
        assert_eq!(f.kind, Kind::Function);
        let Some(ExpectedData::Function { samples }) = f.data else {
            panic!("no samples");
        };
        assert_eq!(samples.len(), 11);
        assert_eq!(samples[3], None);
        assert_eq!(samples[4], Some(1.5));
        let undefined = parse(r#"{"type":"function","defined":false}"#).unwrap();
        assert_eq!((undefined.kind, undefined.data), (Kind::Function, None));
        // exactly the sample points, in order
        assert!(parse(&pairs(&ys[..10])).is_err());
        let swapped = pairs(&ys).replacen("[-8.0", "[-7.0", 1);
        assert!(parse(&swapped).is_err());
        assert!(parse(r#"{"type":"function","defined":true}"#).is_err());
        assert!(parse(r#"{"type":"function","defined":false,"samples":[]}"#).is_err());
        assert!(parse(r#"{"type":"number","defined":true,"value":1.0,"samples":[]}"#).is_err());
        assert!(parse(r#"{"type":"function","defined":true,"value":1.0}"#).is_err());
        assert_eq!(
            script_labels(&["f(x) = x^2".to_string(), "v = f(2)".to_string()]),
            set(&["f", "v"])
        );
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

    #[test]
    fn conics_carry_the_fields_of_their_type() {
        let parse = |s: &str| serde_json::from_str::<Expected>(s);
        let e = parse(
            r#"{"type":"conic","defined":true,"conic_type":"single_point","matrix":[1,1,0,0,0,0],"point":[0,0]}"#,
        )
        .unwrap();
        assert_eq!(
            e.data,
            Some(ExpectedData::Conic {
                conic_type: "single_point".into(),
                matrix: [1.0, 1.0, 0.0, 0.0, 0.0, 0.0],
                parts: ConicParts::SinglePoint { point: (0.0, 0.0) },
            })
        );
        for bad in [
            // a field of another type
            r#"{"type":"conic","defined":true,"conic_type":"single_point","matrix":[1,1,0,0,0,0],"lines":[[1,0,0],[0,1,0]]}"#,
            // a short matrix
            r#"{"type":"conic","defined":true,"conic_type":"empty","matrix":[1,1,0,0,0]}"#,
            // an unknown type
            r#"{"type":"conic","defined":true,"conic_type":"line","matrix":[1,1,0,0,0,0]}"#,
            // no type
            r#"{"type":"conic","defined":true,"matrix":[1,1,0,0,0,0]}"#,
            // fields on an undefined conic
            r#"{"type":"conic","defined":false,"conic_type":"empty"}"#,
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        let lines = parse(
            r#"{"type":"conic","defined":true,"conic_type":"double_line","matrix":[0,1,-1,0,0,0],"lines":[[0,1,-1],[0,1,-1]]}"#,
        )
        .unwrap();
        assert!(matches!(
            lines.data,
            Some(ExpectedData::Conic {
                parts: ConicParts::Lines { .. },
                ..
            })
        ));
    }
}
