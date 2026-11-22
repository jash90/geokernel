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
    pub view: Option<[f64; 6]>,
    pub steps: Vec<Step>,
    pub snapshots: Vec<BTreeMap<String, Expected>>,
    /// Generator of the file this case came from. Filled by the loader.
    #[serde(skip)]
    pub generator: String,
    #[serde(skip)]
    schema: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Initial,
    Move { label: String, x: f64, y: f64 },
    SetNumber { label: String, value: f64 },
    Delete { label: String },
    SetSliderBounds { label: String, min: f64, max: f64 },
    Reject { line: String },
    Redefine { line: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStep {
    action: String,
    label: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
    value: Option<f64>,
    infinity: Option<i8>,
    min: Option<f64>,
    max: Option<f64>,
    line: Option<String>,
    reference_parse_error: Option<bool>,
}

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let raw = RawStep::deserialize(d)?;
        let need = |name: &str| D::Error::custom(format!("step `{}` needs `{name}`", raw.action));
        if !matches!(raw.action.as_str(), "reject" | "redefine") && raw.line.is_some() {
            return Err(D::Error::custom("only `reject` and `redefine` take `line`"));
        }
        if raw.action != "set_number" && raw.infinity.is_some() {
            return Err(D::Error::custom("only `set_number` takes `infinity`"));
        }
        if raw.action != "set_slider_bounds" && (raw.min.is_some() || raw.max.is_some()) {
            return Err(D::Error::custom(
                "only step `set_slider_bounds` takes `min` and `max`",
            ));
        }
        if raw.action != "move" && raw.reference_parse_error.is_some() {
            return Err(D::Error::custom(
                "only step `move` records a reference parse error",
            ));
        }
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
                let value = match (raw.value, raw.infinity) {
                    (Some(value), None) => value,
                    (None, Some(1)) => f64::INFINITY,
                    (None, Some(-1)) => f64::NEG_INFINITY,
                    _ => return Err(D::Error::custom("expected a value or a signed infinity")),
                };
                Ok(Step::SetNumber {
                    label: raw.label.clone().ok_or_else(|| need("label"))?,
                    value,
                })
            }
            "delete" => {
                if raw.x.is_some() || raw.y.is_some() || raw.value.is_some() {
                    return Err(D::Error::custom("step `delete` takes only `label`"));
                }
                Ok(Step::Delete {
                    label: raw.label.ok_or_else(|| need("label"))?,
                })
            }
            "set_slider_bounds" => {
                if raw.x.is_some() || raw.y.is_some() || raw.value.is_some() {
                    return Err(D::Error::custom(
                        "step `set_slider_bounds` takes only `label`, `min` and `max`",
                    ));
                }
                Ok(Step::SetSliderBounds {
                    label: raw.label.ok_or_else(|| need("label"))?,
                    min: raw.min.ok_or_else(|| need("min"))?,
                    max: raw.max.ok_or_else(|| need("max"))?,
                })
            }
            "reject" | "redefine" => {
                if raw.label.is_some() || raw.x.is_some() || raw.y.is_some() || raw.value.is_some()
                {
                    return Err(D::Error::custom(format!(
                        "step `{}` takes only `line`",
                        raw.action
                    )));
                }
                let line = raw.line.ok_or_else(|| need("line"))?;
                if raw.action == "reject" {
                    Ok(Step::Reject { line })
                } else {
                    Ok(Step::Redefine { line })
                }
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
    fn set_encoding(&mut self, encoding: Encoding) -> Result<(), String> {
        if !encoding.signed_samples && self.records_signed_samples() {
            return Err("signed infinite samples require schema 4".into());
        }
        self.legacy_angle_number = encoding.angles_as_numbers && self.kind == Kind::Number;
        if let Some(ExpectedData::List(elements)) = &mut self.data {
            for element in elements {
                element.set_encoding(encoding)?;
            }
        }
        Ok(())
    }

    /// Whether a sample, pie fraction or curve or locus coordinate carries
    /// the schema 4 encoding: a signed infinity, or a null curve coordinate.
    fn records_signed_samples(&self) -> bool {
        let infinite = |v: &Option<f64>| v.is_some_and(f64::is_infinite);
        let nonfinite =
            |p: &Option<(f64, f64)>| p.is_some_and(|(x, y)| !x.is_finite() || !y.is_finite());
        match &self.data {
            Some(ExpectedData::Function { samples }) => samples.iter().any(infinite),
            Some(ExpectedData::FunctionNVar(samples)) => samples.iter().any(|s| infinite(&s.2)),
            Some(ExpectedData::Curve { samples, .. }) => samples.iter().any(|s| nonfinite(&s.1)),
            Some(ExpectedData::Pie { fractions, .. }) => fractions.iter().any(infinite),
            Some(ExpectedData::Locus(points)) => points.iter().any(|p| nonfinite(&p.0)),
            Some(ExpectedData::List(elements)) => elements.iter().any(Self::records_signed_samples),
            _ => false,
        }
    }
}

/// What a schema version leaves out of a recorded number.
#[derive(Debug, Clone, Copy)]
struct Encoding {
    /// Angles are recorded as numbers (schema 1).
    angles_as_numbers: bool,
    /// Samples, pie fractions and curve and locus coordinates record the
    /// sign of an infinity, and `null` there means NaN only (schema 4).
    /// Before, `null` stood for any value that is not finite.
    signed_samples: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpectedData {
    Implicit {
        coefficients: Vec<Vec<f64>>,
        equation: String,
        input_form: bool,
    },
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
    Image,
    Curve {
        start: f64,
        end: f64,
        samples: Vec<CurveSample>,
    },
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
    /// value is NaN (schema 4) or not finite (earlier schemas).
    Function {
        samples: Vec<Option<f64>>,
    },
    Text(String),
    Pie {
        center: (f64, f64),
        radius: f64,
        fractions: Vec<Option<f64>>,
    },
    Relation(Vec<String>),
    /// Two-coordinate values of an equation side.
    FunctionNVar(Vec<(f64, f64, Option<f64>)>),
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

/// A curve parameter and its coordinates. Before schema 4 the pair is `None`
/// when either coordinate is not finite; from schema 4 it is always present,
/// with NaN for a `null` coordinate and the recorded sign of an infinity.
pub type CurveSample = (f64, Option<(f64, f64)>);

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
    /// Only for a function: `[x, y]` pairs, `y` a sample value (see
    /// [`sample_value`]).
    samples: Option<Vec<(f64, serde_json::Value)>>,
    samples_xy: Option<Vec<(f64, f64, serde_json::Value)>>,
    samples_curve: Option<Vec<(f64, serde_json::Value)>>,
    #[serde(flatten)]
    fields: BTreeMap<String, serde_json::Value>,
}

/// A sampled value: a number, `null`, or the sign of an infinity written
/// `{"infinity": 1}` or `-1` (schema 4, checked by [`Expected::set_encoding`]).
fn sample_value(v: &serde_json::Value) -> Result<Option<f64>, String> {
    if v.is_null() {
        return Ok(None);
    }
    if let Some(n) = v.as_f64() {
        return Ok(Some(n));
    }
    match v.as_object() {
        Some(o) if o.len() == 1 => match o.get("infinity").and_then(serde_json::Value::as_i64) {
            Some(1) => Ok(Some(f64::INFINITY)),
            Some(-1) => Ok(Some(f64::NEG_INFINITY)),
            _ => Err(format!("a sample's `infinity` must be 1 or -1, got {v}")),
        },
        _ => Err(format!(
            "a sample must be a number, null or a signed infinity, got {v}"
        )),
    }
}

/// A curve sample: `null` (before schema 4, a coordinate that is not
/// finite), or `[x, y]` whose coordinates are sample values, NaN for `null`.
fn curve_sample(t: f64, v: &serde_json::Value) -> Result<CurveSample, String> {
    if v.is_null() {
        return Ok((t, None));
    }
    match v.as_array().map(Vec::as_slice) {
        Some([x, y]) => {
            let coordinate = |c| sample_value(c).map(|c| c.unwrap_or(f64::NAN));
            Ok((t, Some((coordinate(x)?, coordinate(y)?))))
        }
        _ => Err(format!("a curve sample must be null or [x, y], got {v}")),
    }
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
                let at = match (sample_value(x)?, sample_value(y)?) {
                    (None, None) => None,
                    (Some(x), Some(y)) => Some((x, y)),
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
    Ok(ExpectedData::Function {
        samples: pairs
            .iter()
            .map(|p| sample_value(&p.1))
            .collect::<Result<_, _>>()?,
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
            "function_nvar" => (Kind::FunctionNVar, &[]),
            "text" => (Kind::Text, &["text"]),
            "image" => (Kind::Image, &[]),
            "relation" => (Kind::Text, &["reports"]),
            "polygon" => (Kind::Polygon, &["vertices", "area", "signed_area"]),
            "piechart" => (Kind::PieChart, &["center", "radius", "fractions"]),
            "polyline" => (Kind::PolyLine, &["vertices", "length"]),
            "list" => (Kind::List, &["elements"]),
            "conic" => (Kind::Conic, &[]),
            "arc" => (Kind::Arc, &[]),
            "sector" => (Kind::Sector, &[]),
            "locus" => (Kind::Locus, &["points"]),
            "implicit" => (
                Kind::ImplicitCurve,
                &["coefficients", "equation", "input_form"],
            ),
            "curve" => (Kind::Curve, &["start", "end"]),
            other => return Err(format!("unknown object type `{other}`")),
        };
        if raw.samples.is_some() && (kind != Kind::Function || !raw.defined) {
            return Err(format!(
                "only a defined function carries samples, not this {}",
                raw.ty
            ));
        }
        if raw.samples_xy.is_some() && (kind != Kind::FunctionNVar || !raw.defined) {
            return Err("only a defined two-variable function carries samples_xy".into());
        }
        if raw.samples_curve.is_some() && (kind != Kind::Curve || !raw.defined) {
            return Err("only a defined curve carries samples_curve".into());
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
        if kind == Kind::FunctionNVar {
            let samples = raw
                .samples_xy
                .ok_or("defined two-variable function needs samples_xy")?
                .iter()
                .map(|(x, y, v)| Ok((*x, *y, sample_value(v)?)))
                .collect::<Result<Vec<_>, String>>()?;
            if !raw.fields.is_empty()
                || samples.len() != SAMPLE_XS.len()
                || samples
                    .iter()
                    .zip(SAMPLE_XS)
                    .any(|(&(x, y, _), want_x)| x != want_x || y != 1.0 - 2.0 * want_x)
            {
                return Err("unexpected two-variable function samples".into());
            }
            return Ok(Expected {
                kind,
                data: Some(ExpectedData::FunctionNVar(samples)),
                legacy_angle_number: false,
            });
        }
        if kind == Kind::ImplicitCurve {
            let field = |name| {
                raw.fields
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("missing {name}"))
            };
            let coefficients =
                serde_json::from_value(field("coefficients")?).map_err(|e| e.to_string())?;
            let equation = serde_json::from_value(field("equation")?).map_err(|e| e.to_string())?;
            let input_form =
                serde_json::from_value(field("input_form")?).map_err(|e| e.to_string())?;
            return Ok(Expected {
                legacy_angle_number: false,
                kind,
                data: Some(ExpectedData::Implicit {
                    coefficients,
                    equation,
                    input_form,
                }),
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
        let got: BTreeSet<&str> = raw.fields.keys().map(String::as_str).collect();
        // Schema 3: a number that is infinite in GeoGebra is defined and
        // carries the sign of its infinity in place of a value.
        if kind == Kind::Number && got.len() == 1 && got.contains("infinity") {
            let value = match raw.fields["infinity"].as_i64() {
                Some(1) => f64::INFINITY,
                Some(-1) => f64::NEG_INFINITY,
                _ => return Err("a number's `infinity` must be 1 or -1".into()),
            };
            return Ok(Expected {
                kind,
                data: Some(ExpectedData::Number(value)),
                legacy_angle_number: false,
            });
        }
        let expected: BTreeSet<&str> = names.iter().copied().collect();
        if expected != got {
            return Err(format!(
                "defined {} must carry exactly {names:?}, got {got:?}",
                raw.ty
            ));
        }
        if raw.ty == "relation" {
            let reports = serde_json::from_value(raw.fields["reports"].clone())
                .map_err(|e| format!("invalid relation reports: {e}"))?;
            return Ok(Expected {
                kind,
                data: Some(ExpectedData::Relation(reports)),
                legacy_angle_number: false,
            });
        }
        if kind == Kind::Text {
            let text = raw.fields["text"]
                .as_str()
                .ok_or("a text's content must be a string")?;
            return Ok(Expected {
                kind,
                data: Some(ExpectedData::Text(text.to_owned())),
                legacy_angle_number: false,
            });
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
        if kind == Kind::PieChart {
            let center: [f64; 2] =
                serde_json::from_value(raw.fields["center"].clone()).map_err(|e| e.to_string())?;
            let radius = number("radius", &raw.fields["radius"])?;
            let fractions = raw.fields["fractions"]
                .as_array()
                .ok_or("a pie chart's `fractions` must be an array")?
                .iter()
                .map(sample_value)
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(Expected {
                legacy_angle_number: false,
                kind,
                data: Some(ExpectedData::Pie {
                    center: (center[0], center[1]),
                    radius,
                    fractions,
                }),
            });
        }
        let mut nums = BTreeMap::new();
        for (name, v) in &raw.fields {
            nums.insert(name.as_str(), number(name, v)?);
        }
        let f = |n: &str| nums[n];
        let curve_samples = || -> Result<Vec<CurveSample>, String> {
            raw.samples_curve
                .iter()
                .flatten()
                .map(|(t, v)| curve_sample(*t, v))
                .collect()
        };
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
            Kind::Image => ExpectedData::Image,
            Kind::Curve => ExpectedData::Curve {
                start: f("start"),
                end: f("end"),
                samples: curve_samples()?,
            },
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

/// The scripting commands a script line may run without assigning a label
/// (the harness's `SCRIPTING` pattern).
const SCRIPTING_COMMANDS: &[&str] = &[
    "SetConstructionStep(",
    "ZoomIn(",
    "ZoomOut(",
    "SetActiveView(",
    "$imageAsset(",
];

/// How many lines of the script run a scripting command of their own.
fn scripting_lines(script: &[String]) -> usize {
    script
        .iter()
        .filter(|l| SCRIPTING_COMMANDS.iter().any(|c| l.starts_with(c)))
        .count()
}

/// Labels created by a script: the text before the first `=` of each line,
/// without the argument of a function.
pub fn script_labels(script: &[String]) -> BTreeSet<String> {
    script
        .iter()
        .filter_map(|l| {
            let line = l.split('#').next()?;
            // the label stands before the `:` of an equation line
            // (`a: 2x + 3y = 6`, whose `=` belongs to the equation) or the
            // `=` of any other line
            let sep = match (line.find(':'), line.find('=')) {
                (Some(c), Some(e)) if c < e => c,
                (_, Some(e)) => e,
                (Some(c), None) => c,
                (None, None) => return None,
            };
            Some(line_label(&line[..sep]))
        })
        .collect()
}

/// The label of a script line's left-hand side, such as `f` for `f(x)`.
pub fn line_label(lhs: &str) -> String {
    let lhs = lhs.trim();
    lhs.split_once('(')
        .filter(|(label, args)| {
            !label.is_empty()
                && args.strip_suffix(')').is_some_and(|args| {
                    args.split(',').all(|arg| {
                        let mut chars = arg.trim().chars();
                        chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
                            && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '\'')
                    })
                })
        })
        .map_or(lhs, |(label, _)| label)
        .trim_end()
        .to_string()
}

fn encoding(schema: u32) -> Result<Encoding, String> {
    match schema {
        1..=4 => Ok(Encoding {
            angles_as_numbers: schema == 1,
            signed_samples: schema >= 4,
        }),
        _ => Err(format!("unsupported schema {schema}")),
    }
}

/// Parses one snapshot object from JSON, in the format of `golden/SCHEMA.md`
/// (the `object` field of a verified t-pair output). The pairs were recorded
/// in the schema 2 encoding: angles carry the angle type, an infinite number
/// is written as undefined and a sample that is not finite as `null`.
pub fn expected_from_json(v: &serde_json::Value) -> Result<Expected, String> {
    let raw: RawObject = serde_json::from_value(v.clone()).map_err(|e| format!("object: {e}"))?;
    let mut expected = Expected::try_from(raw)?;
    expected.set_encoding(encoding(2)?)?;
    Ok(expected)
}

/// Apply the file's angle encoding to a snapshot read outside a replay case,
/// such as a snapshot paired with an exported construction.
pub fn set_snapshot_schema(
    schema: u32,
    snapshot: &mut BTreeMap<String, Expected>,
) -> Result<(), String> {
    let encoding = encoding(schema)?;
    for (label, expected) in snapshot.iter_mut() {
        expected
            .set_encoding(encoding)
            .map_err(|e| format!("`{label}`: {e}"))?;
    }
    Ok(())
}

/// Parses and validates one file. `source` is only used in messages.
pub fn parse_file(text: &str, source: &str) -> Result<File, String> {
    let mut file: File =
        serde_json::from_str(text).map_err(|e| format!("{source}: malformed JSON: {e}"))?;
    encoding(file.schema).map_err(|e| format!("{source}: {e}"))?;
    for case in &mut file.cases {
        case.generator = file.generator.clone();
        case.schema = file.schema;
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
    if let Some(view) = case.view {
        let [xmin, xmax, ymin, ymax, xscale, yscale] = view;
        if !view.iter().all(|v| v.is_finite())
            || xmin > xmax
            || ymin >= ymax
            || xscale <= 0.0
            || yscale <= 0.0
        {
            return Err("invalid sampling view".into());
        }
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
    // A scripting command assigns nothing (`SetConstructionStep(3)`, as
    // the harness's SCRIPTING lines); every other line must.
    let labels = script_labels(&case.script);
    if labels.len() + scripting_lines(&case.script) != case.script.len() {
        return Err("script lines must each define a distinct `label = ...`".into());
    }
    let mut previous_keys: Option<BTreeSet<String>> = None;
    let mut boundary_prefixes = BTreeSet::new();
    let mut tangent_prefixes = BTreeSet::new();
    let mut has_point_list = false;
    let mut has_root_interval = false;
    for (i, snap) in case.snapshots.iter().enumerate() {
        let keys: BTreeSet<String> = snap.keys().cloned().collect();
        match &previous_keys {
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
                    if let Some((
                        label,
                        "Difference" | "IntersectPath" | "IntersectRegion" | "IntersectionPaths",
                        _,
                    )) = command_args(line)
                    {
                        clip_prefixes.insert(label_prefix(label));
                    }
                    if let Some((label, kind)) = implicit_output(line, snap) {
                        let prefix = label_prefix(label);
                        if kind == Kind::Line {
                            tangent_prefixes.insert(prefix);
                        } else {
                            boundary_prefixes.insert(prefix);
                        }
                        if !keys.contains(label)
                            && !keys.iter().any(|key| indexed_prefix(key) == Some(prefix))
                        {
                            outputs.remove(label);
                        }
                    }
                    if let Some((label, "Intersect", args)) = command_args(line) {
                        let kind = |name: &str| snap.get(name).map(|o| o.kind);
                        match args.as_slice() {
                            [a, b] => {
                                let boundary =
                                    |k| matches!(k, Some(Kind::Polygon | Kind::PolyLine));
                                let carrier =
                                    |k| matches!(k, Some(Kind::Line | Kind::Segment | Kind::Ray));
                                let (a, b) = (kind(a), kind(b));
                                if boundary(a) && (boundary(b) || carrier(b))
                                    || boundary(b) && carrier(a)
                                    || a == Some(Kind::Curve) && carrier(b)
                                    || b == Some(Kind::Curve) && carrier(a)
                                    || (a, b) == (Some(Kind::Curve), Some(Kind::Curve))
                                {
                                    boundary_prefixes.insert(label_prefix(label));
                                }
                            }
                            [a, b, _, _] => {
                                has_root_interval |= (kind(a), kind(b))
                                    == (Some(Kind::Function), Some(Kind::Function));
                            }
                            _ => {}
                        }
                    }
                    if let Some((label, "Vertex" | "Corner", args)) = command_args(line) {
                        if matches!(args.as_slice(), [arg] if arg.contains(['<', '>'])
                            || snap.get(*arg).is_some_and(|o| o.kind == Kind::FunctionNVar))
                        {
                            boundary_prefixes.insert(label_prefix(label));
                        }
                    }
                    if let Some((_, "Point", args)) = command_args(line) {
                        has_point_list |= matches!(args.as_slice(), [arg]
                            if arg.starts_with('{')
                                || snap.get(*arg).is_some_and(|o| o.kind == Kind::List));
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
                previous_keys = Some(keys);
            }
            Some(previous) if matches!(&case.steps[i], Step::Delete { .. }) => {
                let Step::Delete { label } = &case.steps[i] else {
                    unreachable!()
                };
                if !previous.contains(label) || keys.contains(label) || !keys.is_subset(previous) {
                    return Err(format!("snapshot {i} has invalid labels after deletion"));
                }
                previous_keys = Some(keys);
            }
            Some(previous) if *previous != keys => {
                // These algorithms retain their earlier outputs and can
                // publish new point labels on a move.
                if !previous.is_subset(&keys)
                    || keys.difference(previous).any(|key| match snap[key].kind {
                        Kind::Point => {
                            !(indexed_prefix(key).is_some_and(|p| boundary_prefixes.contains(p))
                                || (has_point_list || has_root_interval)
                                    && POINT_LABELS.contains(&label_prefix(key)))
                        }
                        Kind::Line => {
                            !indexed_prefix(key).is_some_and(|p| tangent_prefixes.contains(p))
                        }
                        _ => true,
                    })
                {
                    return Err(format!("snapshot {i} labels differ from snapshot 0"));
                }
                previous_keys = Some(keys);
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// The part of a label before its first `_`, which GeoGebra numbers when a
/// command creates several objects (`t` -> `t_{1}`, `t_{2}`).
pub(crate) fn label_prefix(label: &str) -> &str {
    label.split('_').next().unwrap_or(label)
}

/// `Some(prefix)` when `key` has the form `prefix_{n}` with `n` a positive
/// integer written without leading zeros.
pub(crate) fn indexed_prefix(key: &str) -> Option<&str> {
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
        // A fixed-vertex polygon image is made from the labelled images of
        // its points. Track possible labels through every step in the chain.
        // None represents an indexed label, whose image takes a default label.
        let mut polygons: BTreeMap<&str, BTreeSet<Option<String>>> = BTreeMap::new();
        for line in script {
            let Some((label, command, args)) = command_args(line) else {
                continue;
            };
            if command == "Polygon"
                && args.len() >= 3
                && args
                    .iter()
                    .all(|arg| kind(arg) == Some(Kind::Point) || names.point(arg))
            {
                polygons.insert(label, args.iter().map(|v| Some(v.to_string())).collect());
                continue;
            }
            let Some(vertices) = transformed_arg(line).and_then(|src| polygons.get(src)) else {
                continue;
            };
            let mut image = BTreeSet::new();
            for vertex in vertices {
                if let Some(vertex) = vertex
                    .as_ref()
                    .filter(|v| !v.contains('_') && !v.ends_with("'''"))
                {
                    let prime = format!("{vertex}'");
                    if names.points.contains(&prime) || kind(&prime) == Some(Kind::Point) {
                        names.indexed_points.insert(prime.clone());
                        names.indexed_segments.insert(prime.to_lowercase());
                        image.insert(None);
                    }
                    names.points.insert(prime.clone());
                    names.segments.insert(prime.to_lowercase());
                    image.insert(Some(prime));
                } else {
                    names.default_points();
                    for base in POINT_LABELS {
                        names.indexed_segments.insert(base.to_lowercase());
                        image.insert(Some(base.to_string()));
                    }
                    image.insert(None);
                }
            }
            names.default_segments();
            polygons.insert(label, image);
        }
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
                || matches!(
                    command,
                    "IntersectPath" | "IntersectRegion" | "IntersectionPaths"
                ) && args
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

pub(crate) fn implicit_output<'a>(
    line: &'a str,
    snapshot: &BTreeMap<String, Expected>,
) -> Option<(&'a str, Kind)> {
    let (label, command, args) = command_args(line)?;
    let [a, b] = args.as_slice() else { return None };
    let (a, b) = (snapshot.get(*a)?.kind, snapshot.get(*b)?.kind);
    if command == "Tangent" {
        return matches!(
            (a, b),
            (Kind::Point | Kind::Line, Kind::ImplicitCurve) | (Kind::ImplicitCurve, Kind::Point)
        )
        .then_some((label, Kind::Line));
    }
    if command != "Intersect" {
        return None;
    }
    let partner = |k| {
        matches!(
            k,
            Kind::Line
                | Kind::Segment
                | Kind::Ray
                | Kind::PolyLine
                | Kind::Function
                | Kind::Conic
                | Kind::Circle
                | Kind::Arc
                | Kind::Sector
                | Kind::ImplicitCurve
        )
    };
    (a == Kind::ImplicitCurve && partner(b) || b == Kind::ImplicitCurve && partner(a))
        .then_some((label, Kind::Point))
}

/// The transformed object of a script line `name = Mirror(p, ...)` and the
/// other transformations, when the line creates the sides of a polygon's
/// image as well (`Transform.transformPoly`).
fn transformed_arg(line: &str) -> Option<&str> {
    let (_, rhs) = line.split('#').next()?.split_once('=')?;
    let rhs = rhs.trim();
    let (func, rest) = rhs.split_once('(')?;
    let inner = rest.strip_suffix(')')?;
    if func == "ApplyMatrix" {
        return inner.rsplit(',').next().map(str::trim);
    }
    if !matches!(
        func,
        "Mirror" | "Reflect" | "Rotate" | "Dilate" | "Shear" | "Stretch" | "Translate"
    ) {
        return None;
    }
    inner.split(',').map(str::trim).next()
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
        restore_legacy_infinities(&mut cases)?;
        Ok(cases)
    } else {
        Err(errors.join("\n"))
    }
}

/// One legacy `null` sample checked against the same position of a signed
/// recording: an infinity takes the recorded sign, NaN stays `None`. A
/// finite value on one side only is an error. Returns the positions verified.
fn restore_sample(old: &mut Option<f64>, new: Option<f64>) -> Result<usize, String> {
    match (*old, new) {
        (None, None) => Ok(1),
        (None, Some(v)) if v.is_infinite() => {
            *old = Some(v);
            Ok(1)
        }
        (Some(p), Some(q)) if p.is_finite() && q.is_finite() => Ok(0),
        _ => Err(format!("sample {old:?} is recorded as {new:?}")),
    }
}

/// Signs and NaNs of legacy `null` samples, pie fractions and curve
/// coordinates from the same object of a signed recording.
fn restore_samples(old: &mut ExpectedData, new: &ExpectedData) -> Result<usize, String> {
    fn each<T, U>(
        old: &mut [T],
        new: &[U],
        mut restore: impl FnMut(&mut T, &U) -> Result<usize, String>,
    ) -> Result<usize, String> {
        if old.len() != new.len() {
            return Err("sample counts differ".into());
        }
        old.iter_mut().zip(new).map(|(o, n)| restore(o, n)).sum()
    }
    match (old, new) {
        (ExpectedData::Function { samples: a }, ExpectedData::Function { samples: b })
        | (ExpectedData::Pie { fractions: a, .. }, ExpectedData::Pie { fractions: b, .. }) => {
            each(a, b, |o, n| restore_sample(o, *n))
        }
        (ExpectedData::FunctionNVar(a), ExpectedData::FunctionNVar(b)) => {
            each(a, b, |o, n| restore_sample(&mut o.2, n.2))
        }
        (ExpectedData::Curve { samples: a, .. }, ExpectedData::Curve { samples: b, .. }) => {
            each(a, b, |o, n| match (o.1, n.1) {
                (None, Some((x, y))) if !x.is_finite() || !y.is_finite() => {
                    o.1 = n.1;
                    Ok(1)
                }
                (Some((x, y)), Some((a, b)))
                    if x.is_finite() && y.is_finite() && a.is_finite() && b.is_finite() =>
                {
                    Ok(0)
                }
                // a recording that still writes `null` for the pair adds nothing
                (None, None) => Ok(0),
                (old, new) => Err(format!("curve sample {old:?} is recorded as {new:?}")),
            })
        }
        _ => Ok(0),
    }
}

/// A legacy undefined number can recover its sign only at the matching
/// position of a separately recorded case of schema 3 or later. A `null`
/// sample of a file before schema 4 recovers the sign of an infinity or is
/// verified as NaN at the matching position of a schema 4 recording.
fn restore_legacy_infinities(cases: &mut [Case]) -> Result<(), String> {
    const PAIRS: &str = include_str!("../../../tools/ggb-harness/cases/legacy-infinity.txt");
    fn restore(old: &mut Expected, new: &Expected, samples: bool) -> Result<usize, String> {
        match (&mut old.data, &new.data) {
            (None, Some(ExpectedData::Number(value)))
                if old.kind == Kind::Number && value.is_infinite() =>
            {
                old.data = Some(ExpectedData::Number(*value));
                Ok(1)
            }
            (Some(ExpectedData::List(a)), Some(ExpectedData::List(b))) if a.len() == b.len() => a
                .iter_mut()
                .zip(b)
                .map(|(old, new)| restore(old, new, samples))
                .sum(),
            (Some(a), Some(b)) if samples && old.kind == new.kind => restore_samples(a, b),
            _ => Ok(0),
        }
    }
    let positions: BTreeMap<_, _> = cases
        .iter()
        .enumerate()
        .map(|(i, case)| (case.id.clone(), i))
        .collect();
    for pair in PAIRS.lines() {
        let (old, new) = pair.split_once(' ').expect("two case IDs per line");
        let (&old_pos, &new_pos) = positions
            .get(old)
            .zip(positions.get(new))
            .ok_or_else(|| format!("missing case in legacy infinity pair {pair}"))?;
        let recording = &cases[new_pos];
        let legacy = &cases[old_pos];
        if recording.schema < 3 || recording.generator != "ggb-harness" {
            return Err(format!("unexpected formats in legacy infinity pair {pair}"));
        }
        if legacy.script != recording.script
            || legacy.view != recording.view
            || legacy.steps != recording.steps
        {
            return Err(format!("inputs differ in legacy infinity pair {pair}"));
        }
        // A legacy file re-recorded in schema 4 carries its own signs.
        if legacy.schema >= 4 {
            continue;
        }
        if !matches!(legacy.schema, 1 | 2) {
            return Err(format!("unexpected formats in legacy infinity pair {pair}"));
        }
        let samples = recording.schema >= 4;
        let snapshots = recording.snapshots.clone();
        let mut count = 0;
        for (old, new) in cases[old_pos].snapshots.iter_mut().zip(&snapshots) {
            for (label, expected) in old.iter_mut() {
                if let Some(recorded) = new.get(label) {
                    count += restore(expected, recorded, samples)
                        .map_err(|e| format!("legacy infinity pair {pair}, `{label}`: {e}"))?;
                }
            }
        }
        if count == 0 {
            return Err(format!("no legacy infinity positions verified by {pair}"));
        }
    }
    Ok(())
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
            &GOOD.replace("\"schema\":1", "\"schema\":5"),
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
    fn an_infinite_number_is_recorded_only_from_schema_3() {
        let parse = |json: &str| serde_json::from_str::<Expected>(json);
        let plus = parse(r#"{"type":"number","defined":true,"infinity":1}"#).unwrap();
        assert_eq!(plus.data, Some(ExpectedData::Number(f64::INFINITY)));
        let minus = parse(r#"{"type":"number","defined":true,"infinity":-1}"#).unwrap();
        assert_eq!(minus.data, Some(ExpectedData::Number(f64::NEG_INFINITY)));
        for bad in [
            r#"{"type":"number","defined":true,"infinity":0}"#,
            r#"{"type":"number","defined":true,"infinity":1,"value":1.0}"#,
            r#"{"type":"number","defined":false,"infinity":1}"#,
            r#"{"type":"angle","defined":true,"infinity":1}"#,
            r#"{"type":"vector","defined":true,"infinity":1}"#,
        ] {
            assert!(parse(bad).is_err(), "accepted: {bad}");
        }
    }

    /// A file of `schema` with one snapshot holding `object` as `f`.
    fn file_with(schema: u32, object: &str) -> String {
        format!(
            r#"{{"schema":{schema},"generator":"format-sample","upstream_commit":null,
              "cases":[{{"id":"x","group":"g","script":["f(x) = 1 / x"],
              "steps":[{{"action":"initial"}}],"snapshots":[{{"f":{object}}}]}}]}}"#
        )
    }

    fn function_with(value: &str) -> String {
        let samples: Vec<String> = SAMPLE_XS
            .iter()
            .map(|x| format!("[{x:?}, {}]", if *x == 0.0 { value } else { "1.0" }))
            .collect();
        format!(
            r#"{{"type":"function","defined":true,"samples":[{}]}}"#,
            samples.join(",")
        )
    }

    #[test]
    fn infinite_samples_carry_their_sign_from_schema_4() {
        let at_zero = |text: &str| {
            let file = parse_file(text, "t").unwrap();
            match &file.cases[0].snapshots[0]["f"].data {
                Some(ExpectedData::Function { samples }) => samples[5],
                other => panic!("not a function: {other:?}"),
            }
        };
        let plus = function_with(r#"{"infinity":1}"#);
        let minus = function_with(r#"{"infinity":-1}"#);
        assert_eq!(at_zero(&file_with(4, &plus)), Some(f64::INFINITY));
        assert_eq!(at_zero(&file_with(4, &minus)), Some(f64::NEG_INFINITY));
        assert_eq!(at_zero(&file_with(4, &function_with("null"))), None);
        assert_eq!(at_zero(&file_with(3, &function_with("null"))), None);
        // older schemas only know `null` for a value that is not finite
        for schema in 1..=3 {
            assert!(parse_file(&file_with(schema, &plus), "t").is_err());
        }
        for bad in [
            r#"{"infinity":0}"#,
            r#"{"infinity":1,"x":1}"#,
            r#"{}"#,
            "true",
        ] {
            assert!(
                parse_file(&file_with(4, &function_with(bad)), "t").is_err(),
                "{bad}"
            );
        }
        // the verified t-pairs keep the schema 2 encoding
        let value: serde_json::Value = serde_json::from_str(&plus).unwrap();
        assert!(expected_from_json(&value).is_err());
    }

    #[test]
    fn curve_pie_and_locus_coordinates_carry_their_sign_from_schema_4() {
        let curve = r#"{"type":"curve","defined":true,"start":0.0,"end":1.0,
            "samples_curve":[[0.0,[{"infinity":-1},null]],[1.0,[1.0,2.0]]]}"#;
        let file = parse_file(&file_with(4, curve), "t").unwrap();
        let Some(ExpectedData::Curve { samples, .. }) = &file.cases[0].snapshots[0]["f"].data
        else {
            panic!("not a curve");
        };
        let (x, y) = samples[0].1.unwrap();
        assert!(x == f64::NEG_INFINITY && y.is_nan());
        assert_eq!(samples[1].1, Some((1.0, 2.0)));
        assert!(parse_file(&file_with(3, curve), "t").is_err());
        let legacy = r#"{"type":"curve","defined":true,"start":0.0,"end":1.0,
            "samples_curve":[[0.0,null]]}"#;
        assert!(parse_file(&file_with(3, legacy), "t").is_ok());

        let pie = r#"{"type":"piechart","defined":true,"center":[0.0,0.0],"radius":1.0,
            "fractions":[null,{"infinity":1}]}"#;
        let file = parse_file(&file_with(4, pie), "t").unwrap();
        let Some(ExpectedData::Pie { fractions, .. }) = &file.cases[0].snapshots[0]["f"].data
        else {
            panic!("not a pie chart");
        };
        assert_eq!(fractions, &[None, Some(f64::INFINITY)]);
        assert!(parse_file(&file_with(3, pie), "t").is_err());

        let locus = r#"{"type":"locus","defined":true,
            "points":[[{"infinity":1},2.0,false],[null,null,true]]}"#;
        let file = parse_file(&file_with(4, locus), "t").unwrap();
        let Some(ExpectedData::Locus(points)) = &file.cases[0].snapshots[0]["f"].data else {
            panic!("not a locus");
        };
        assert_eq!(points[0], (Some((f64::INFINITY, 2.0)), false));
        assert_eq!(points[1], (None, true));
        assert!(parse_file(&file_with(3, locus), "t").is_err());
    }

    #[test]
    fn legacy_null_samples_take_the_signed_recording() {
        let mut old = vec![None, None, Some(1.0)];
        let new = [Some(f64::NEG_INFINITY), None, Some(1.0 + 1e-15)];
        let mut legacy = ExpectedData::Function {
            samples: old.clone(),
        };
        let recorded = ExpectedData::Function {
            samples: new.to_vec(),
        };
        assert_eq!(restore_samples(&mut legacy, &recorded), Ok(2));
        old[0] = Some(f64::NEG_INFINITY);
        assert_eq!(legacy, ExpectedData::Function { samples: old });
        // a value on one side only contradicts the legacy file
        for (old, new) in [
            (None, Some(2.0)),
            (Some(2.0), None),
            (Some(2.0), Some(f64::INFINITY)),
        ] {
            let mut sample = old;
            assert!(restore_sample(&mut sample, new).is_err(), "{old:?} {new:?}");
        }
        let mut curve = ExpectedData::Curve {
            start: 0.0,
            end: 1.0,
            samples: vec![(0.0, None), (1.0, Some((1.0, 1.0)))],
        };
        let recorded = ExpectedData::Curve {
            start: 0.0,
            end: 1.0,
            samples: vec![(0.0, Some((f64::INFINITY, 0.0))), (1.0, Some((1.0, 1.0)))],
        };
        assert_eq!(restore_samples(&mut curve, &recorded), Ok(1));
        assert_eq!(curve, recorded);
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

    #[test]
    fn transformed_polygon_sides_are_created_labels() {
        let script = vec![
            "A = (4.7, -3.93)".to_string(),
            "B = (-7.04, -4.69)".to_string(),
            "E = (4.14, 4.51)".to_string(),
            "p = Polygon(A, B, E)".to_string(),
            "Q = (0.71, -2.15)".to_string(),
            "m = Mirror(p, Q)".to_string(),
        ];
        let names = PolygonNames::of(&script, |l| {
            (l == "A" || l == "B" || l == "E" || l == "Q").then_some(Kind::Point)
        });
        assert!(names.segment("a'"));
        assert!(names.segment("b'"));
        assert!(names.segment("e'"));
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
