//! Input resolution for function-analysis commands with live construction outputs.

use super::{add_named_outputs, add_outputs, define_cartesian_curve, index_label, split_call};
use crate::algo::analysis::{AnalysisDef, CurvatureKind, Mode, SplineInput};
use crate::expr;
use crate::kernel::locus::NumericLocus;
use crate::{Construction, Def, Id, Kind, LocusDef};

pub(super) fn exec(c: &mut Construction, label: &str, rhs: &str) -> Option<Result<Id, String>> {
    let (command, args) = split_call(rhs)?;
    match command {
        "SlopeField" if args.len() != 1 => return Some(add_ode(c, label, command, &args)),
        "SolveODE" | "NSolveODE" => return Some(add_ode(c, label, command, &args)),
        "Curve" | "CurveCartesian" => return Some(cartesian(c, label, &args)),
        "Curvature" | "CurvatureVector" | "OsculatingCircle" => {
            return Some(curvature(c, label, command, &args));
        }
        "ParametricDerivative" | "Spline" | "BezierCurve" => {
            return Some(derived_curve(c, label, command, &args));
        }
        "Asymptote"
            if matches!(&args[..], [a] if
            c.id(a).is_none_or(|id| c.kind(id) == Kind::Function)) =>
        {
            return Some((|| {
                let f = function(c, args[0])?;
                let source = format!("Asymptote({})", c.label(f));
                expr::define(c, label, &source, false)
            })());
        }
        "Function" if args.len() >= 3 => return Some(restrict(c, label, &args, false)),
        "$functionInterval" => return Some(restrict(c, label, &args, true)),
        "Root" if args.len() == 2 && args.iter().all(|arg| is_function(c, arg)) => {
            return Some(
                super::define_resolved(c, label, &format!("Intersect({})", args.join(", ")))
                    .and_then(|id| id.ok_or_else(|| "no intersection output".into())),
            );
        }
        _ => {}
    }
    Some(resolve_outputs(c, command, &args)?.and_then(|defs| add_analysis_outputs(c, label, defs)))
}

fn add_analysis_outputs(c: &mut Construction, label: &str, defs: Vec<Def>) -> Result<Id, String> {
    if defs.len() < 2 {
        return add_outputs(c, label, defs);
    }
    if c.id(label).is_some() {
        return Err(format!("object `{label}` already exists"));
    }
    // Conditional polynomial commands label slots before clearing those
    // outside the condition. ODE systems also have one slot per equation.
    let prefix = label
        .split('_')
        .next()
        .filter(|p| !p.is_empty())
        .unwrap_or("a");
    let start = c.checkpoint();
    let mut first = None;
    for def in defs {
        match c.add(&index_label(c, prefix), def) {
            Ok(id) => {
                first.get_or_insert(id);
            }
            Err(err) => {
                c.discard_since(start);
                return Err(err.to_string());
            }
        }
    }
    first.ok_or_else(|| "missing analysis output".into())
}

pub(super) fn exec_slots(
    c: &mut Construction,
    slots: &[&str],
    rhs: &str,
) -> Option<Result<Option<Id>, String>> {
    let (command, args) = split_call(rhs)?;
    let start = c.checkpoint();
    let result = match (command, args.as_slice()) {
        ("Root", [a, b]) if is_function(c, a) && is_function(c, b) => {
            return Some(super::exec_slots(c, slots, &format!("Intersect({a}, {b})")));
        }
        ("NSolveODE", _) => resolve_system(c, &args),
        _ => resolve_outputs(c, command, &args)?,
    }
    .and_then(|defs| add_named_outputs(c, slots, defs));
    if result.is_err() {
        c.discard_since(start);
    }
    Some(result)
}

fn is_analysis(command: &str) -> bool {
    matches!(
        command,
        "Root" | "Roots" | "Extremum" | "TurningPoint" | "ComplexRoot" | "RemovableDiscontinuity"
    )
}

pub(super) fn existing_outputs(
    c: &Construction,
    command: &str,
    args: &[&str],
) -> Option<Result<Vec<Def>, String>> {
    if command == "NSolveODE" {
        return Some((|| {
            let ids = args
                .iter()
                .map(|arg| super::object(c, arg).and_then(packed))
                .collect::<Result<Vec<_>, _>>()?;
            system_defs(
                c,
                ids.try_into().map_err(|_| "invalid NSolveODE arguments")?,
            )
        })());
    }
    if let ("Root", [a, b]) = (command, args) {
        if is_function(c, a) && is_function(c, b) {
            return crate::algo::intersect::parse_multi(c, "Intersect", args).map(Ok);
        }
    }
    if !is_analysis(command) {
        return None;
    }
    Some(outputs(c, command, args, |arg| c.id(arg)))
}

fn resolve_outputs(
    c: &mut Construction,
    command: &str,
    args: &[&str],
) -> Option<Result<Vec<Def>, String>> {
    if !is_analysis(command) {
        return None;
    }
    Some((|| {
        let [source, parameters @ ..] = args else {
            return Err(format!("{command} needs a function"));
        };
        let function = function(c, source)?;
        let params: Vec<_> = parameters
            .iter()
            .map(|param| argument(c, param, Kind::Number))
            .collect::<Result<_, _>>()?;
        let ids = std::iter::once(function).chain(params).collect::<Vec<_>>();
        let mut next = ids.into_iter();
        outputs(c, command, args, |_| next.next())
    })())
}

fn outputs(
    c: &Construction,
    command: &str,
    args: &[&str],
    mut id: impl FnMut(&str) -> Option<Id>,
) -> Result<Vec<Def>, String> {
    let [source, rest @ ..] = args else {
        return Err(format!("{command} needs a function"));
    };
    let function = id(source)
        .filter(|&id| c.kind(id) == Kind::Function)
        .ok_or_else(|| format!("{command} needs a function"))?;
    let mut number = |arg| {
        let id = id(arg)
            .filter(|&id| c.kind(id).is_number())
            .ok_or_else(|| format!("{command} needs numeric bounds or a seed"))?;
        packed(id)
    };
    let mode = match (command, rest) {
        ("Root", []) => Mode::Polynomial,
        ("Root", [seed]) => Mode::Seed(number(seed)?),
        ("Root", [a, b]) => Mode::SingleInterval(number(a)?, number(b)?),
        ("Roots", []) => Mode::Roots(None),
        ("Roots", [a, b]) => Mode::Roots(Some((number(a)?, number(b)?))),
        ("Extremum", []) => Mode::Extremum(None),
        ("Extremum", [a, b]) => Mode::Extremum(Some((number(a)?, number(b)?))),
        ("TurningPoint", []) => Mode::TurningPoint,
        ("ComplexRoot", []) => Mode::Complex,
        ("RemovableDiscontinuity", []) => Mode::Discontinuity,
        _ => return Err(format!("unsupported {command} arguments")),
    };
    Ok(AnalysisDef::outputs(c, function, mode))
}

fn is_function(c: &Construction, text: &str) -> bool {
    c.id(text).is_some_and(|id| c.kind(id) == Kind::Function)
}

pub(super) fn function(c: &mut Construction, text: &str) -> Result<Id, String> {
    if let Some(id) = c.id(text).filter(|&id| c.kind(id) == Kind::Function) {
        return Ok(id);
    }
    let label = format!("\0arg{}", c.len());
    expr::define_function(c, &label, text, false)
}

pub(super) fn argument(c: &mut Construction, text: &str, kind: Kind) -> Result<Id, String> {
    if let Some(id) = c
        .id(text)
        .filter(|&id| c.kind(id) == kind || kind == Kind::Number && c.kind(id).is_number())
    {
        return Ok(id);
    }
    let label = format!("\0arg{}", c.len());
    if expr::argument_object(c, text, &label, kind) {
        return c
            .id(&label)
            .ok_or_else(|| format!("missing argument `{text}`"));
    }
    Err(format!("expected a {kind}, got `{text}`"))
}

fn restrict(c: &mut Construction, label: &str, args: &[&str], legacy: bool) -> Result<Id, String> {
    let (source, variable, a, b) = match *args {
        [source, a, b] => (source, "x", a, b),
        [source, variable, a, b] if super::is_identifier(variable) => (source, variable, a, b),
        _ => {
            return Err("Function needs a function, optional local variable and two bounds".into())
        }
    };
    let value = if is_function(c, source) {
        format!("{source}({variable})")
    } else {
        source.to_owned()
    };
    let body = format!("If(({a}) <= {variable} <= ({b}), {value})");
    if legacy {
        let id = expr::define_function_interval(c, label, &body, variable)?;
        c.remember_construction_source(id, &format!("$functionInterval({})", args.join(", ")));
        Ok(id)
    } else {
        expr::define_parametric_coordinate(c, label, &body, variable)
    }
}

fn packed(id: Id) -> Result<u32, String> {
    u32::try_from(id.0).map_err(|_| "argument index is too large".to_owned())
}

fn resolve_system(c: &mut Construction, args: &[&str]) -> Result<Vec<Def>, String> {
    let [functions, start, initial, end] = *args else {
        return Err("NSolveODE needs a function list, a start, initial values and an end".into());
    };
    let args = [
        packed(argument(c, functions, Kind::List)?)?,
        packed(argument(c, start, Kind::Number)?)?,
        packed(argument(c, initial, Kind::List)?)?,
        packed(argument(c, end, Kind::Number)?)?,
    ];
    system_defs(c, args)
}

fn system_defs(c: &Construction, args: [u32; 4]) -> Result<Vec<Def>, String> {
    let count = NumericLocus::system_size(c, args)
        .ok_or("NSolveODE needs equally sized nonempty lists of functions and numbers")?;
    Ok((0..count)
        .map(|slot| {
            Def::Locus(LocusDef::Numeric(NumericLocus::System {
                args,
                slot: slot as u8,
            }))
        })
        .collect())
}

fn add_ode(c: &mut Construction, label: &str, command: &str, args: &[&str]) -> Result<Id, String> {
    let slope = |c: &mut Construction, text| {
        if let Some(id) = c
            .id(text)
            .filter(|&id| matches!(c.kind(id), Kind::Function | Kind::FunctionNVar))
        {
            packed(id)
        } else {
            let label = format!("\0arg{}", c.len());
            packed(expr::define_slope(c, &label, text)?)
        }
    };
    if command == "NSolveODE" {
        let defs = resolve_system(c, args)?;
        return add_analysis_outputs(c, label, defs);
    }
    let functions = match (command, args.len()) {
        ("SlopeField", 2 | 3 | 7) | ("SolveODE", 5) => 1,
        ("SolveODE", 6) => 2,
        ("SolveODE", 8) => 3,
        _ => return Err(format!("unsupported {command} arguments")),
    };
    let mut ids = [0; 8];
    for (i, &text) in args.iter().enumerate() {
        ids[i] = if i < functions {
            slope(c, text)?
        } else {
            packed(argument(c, text, Kind::Number)?)?
        };
    }
    let def = match command {
        "SlopeField" => NumericLocus::Slope {
            function: ids[0],
            options: ids[1..7].try_into().unwrap(),
            count: (args.len() - 1) as u8,
        },
        _ => match functions {
            1 => NumericLocus::First(ids[..5].try_into().unwrap()),
            2 => NumericLocus::Parametric(ids[..6].try_into().unwrap()),
            _ => NumericLocus::Second(ids),
        },
    };
    c.add(label, Def::Locus(LocusDef::Numeric(def)))
        .map_err(|e| e.to_string())
}

fn cartesian(c: &mut Construction, label: &str, args: &[&str]) -> Result<Id, String> {
    if let [coordinates, parameter, start, end] = *args {
        if let Some(("Vector", args)) = split_call(coordinates) {
            if let [point] = args.as_slice() {
                if !super::is_identifier(parameter) {
                    return Err("`Curve` needs a parameter name".into());
                }
                let x = format!("\0arg{}", c.len());
                expr::define_parametric_coordinate(c, &x, &format!("x({point})"), parameter)?;
                let y = format!("\0arg{}", c.len());
                expr::define_parametric_coordinate(c, &y, &format!("y({point})"), parameter)?;
                // A Vector command constructs a vector at the initial local
                // value. A point expression supplies live coordinates in t.
                return define_cartesian_curve(
                    c,
                    label,
                    &[
                        &format!("{x}({start})"),
                        &format!("{y}({start})"),
                        parameter,
                        start,
                        end,
                    ],
                );
            }
        }
        let x = format!("x({coordinates})");
        let y = format!("y({coordinates})");
        return define_cartesian_curve(c, label, &[&x, &y, parameter, start, end]);
    }
    define_cartesian_curve(c, label, args)
}

fn curvature(
    c: &mut Construction,
    label: &str,
    command: &str,
    args: &[&str],
) -> Result<Id, String> {
    let [point, source] = *args else {
        return Err(format!(
            "{command} needs a point and a function, curve or conic"
        ));
    };
    let point = argument(c, point, Kind::Point)?;
    let source = if let Some(id) = c.id(source) {
        id
    } else if split_call(source).is_some_and(|(cmd, _)| {
        matches!(
            cmd,
            "Curve"
                | "CurveCartesian"
                | "Circle"
                | "Ellipse"
                | "Hyperbola"
                | "Parabola"
                | "Conic"
                | "Spline"
                | "BezierCurve"
        )
    }) {
        let hidden = format!("\0curve{}", c.len());
        super::define_after_label(c, &hidden, source)?.ok_or("missing curve")?
    } else {
        function(c, source)?
    };
    let output = match command {
        "Curvature" => CurvatureKind::Scalar,
        "CurvatureVector" => CurvatureKind::Vector,
        _ => CurvatureKind::Circle,
    };
    c.add(
        label,
        Def::Analysis(AnalysisDef::Curvature {
            point,
            source,
            output,
        }),
    )
    .map_err(|e| e.to_string())
}

fn derived_curve(
    c: &mut Construction,
    label: &str,
    name: &str,
    args: &[&str],
) -> Result<Id, String> {
    let def = match (name, args) {
        ("ParametricDerivative", [curve]) => {
            let id = if let Some(id) = c.id(curve) {
                id
            } else {
                let hidden = format!("\0arg{}", c.len());
                super::define_after_label(c, &hidden, curve)?.ok_or("missing parametric curve")?
            };
            AnalysisDef::ParametricDerivative(id)
        }
        ("Spline", [list, tail @ ..]) if tail.len() <= 2 => {
            let points = argument(c, list, Kind::List)?;
            let degree = tail
                .first()
                .map(|s| argument(c, s, Kind::Number))
                .transpose()?;
            let weight = tail
                .get(1)
                .map(|s| {
                    if let Some(id) = c
                        .id(s)
                        .filter(|&id| matches!(c.kind(id), Kind::Function | Kind::FunctionNVar))
                    {
                        Ok(id)
                    } else {
                        let hidden = format!("\0arg{}", c.len());
                        expr::define_slope(c, &hidden, s)
                    }
                })
                .transpose()?;
            AnalysisDef::Spline(SplineInput {
                points,
                degree: degree.map(packed).transpose()?,
                weight: weight.map(packed).transpose()?,
            })
        }
        ("Spline", _) if args.len() >= 4 => {
            let list = format!("{{{}}}", args.join(", "));
            AnalysisDef::Spline(SplineInput {
                points: argument(c, &list, Kind::List)?,
                degree: None,
                weight: None,
            })
        }
        ("BezierCurve", [a, b, d, e]) => {
            let mut points = Vec::with_capacity(4);
            for p in [a, b, d, e] {
                let id = argument(c, p, Kind::Point)?;
                points.push(c.label(id).to_string());
            }
            let coordinate = |axis| {
                let [a, b, c, d] = std::array::from_fn(|i| format!("{axis}({})", points[i]));
                format!("{a}*t^3 + 3*{b}*t^2*(1-t) + 3*{c}*t*(1-t)^2 + {d}*(1-t)^3")
            };
            return define_cartesian_curve(
                c,
                label,
                &[&coordinate("x"), &coordinate("y"), "t", "0", "1"],
            );
        }
        _ => return Err(format!("unsupported {name} arguments")),
    };
    c.add(label, Def::Analysis(def)).map_err(|e| e.to_string())
}
