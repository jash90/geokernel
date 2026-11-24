// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/LabelManager.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Construction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/AlgebraProcessor.java
//   source/shared/common/src/main/java/org/geogebra/common/util/StringUtil.java
//   source/shared/editor-base/src/main/java/org/geogebra/editor/share/input/Character.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! A small input-bar style language, one definition per line:
//!
//! ```text
//! A = (0, 0)              # free point
//! v = (1, 2)              # free vector (the label starts with a lower-case letter)
//! r = 3                   # free number
//! l = Line(A, B)
//! m = Line(P, l)          # GeoGebra form: parallel to l (a line or segment) through P
//! n = PerpendicularLine(P, l)  # GeoGebra name for `Perpendicular`
//! k = Circle(A, B)        # centre A, through B
//! m = Circle(A, r)        # centre A, radius r
//! P = Intersect(l, k, 2)  # second intersection (1 is the default)
//! t = Tangent(P, k)       # two tangents t_{1} and t_{2} when both are defined at creation
//! c = a + 2*b             # arithmetic (see `crate::expr`) where no form above applies
//! f(x) = x^2 - a          # a function of x
//! a: 2x + 3y = 6          # a line, a conic or a function as an equation in x and y
//! g: y = m x + b          # the input bar's equation form; see `crate::expr::equation`
//! {t_{1}, t_{2}} = Tangent(P, k)   # the same, with the output labels stated
//! {, g} = AngularBisector(l, m)    # only the second output is created, as `g`
//! t_{1} = (1, 2)         # a label may have the indexed form `prefix_{letters, digits}`
//! a = $point(1, 2)        # a free point whose label starts with a lower-case letter
//! B = $vector(1, 2)       # a free vector whose label starts with an upper-case letter
//! ```
//!
//! `{label, label, ...} = Command(...)` names every output of a command that
//! creates several objects, in GeoGebra's output order; an empty slot is an
//! output that is not created. Which outputs are created follows the labels,
//! not which outputs are defined when the line runs. It is what a saved
//! construction needs, because GeoGebra writes the label of every output it
//! created. One limit: when both tangents from a point are the same line at
//! creation (the point is on the circle) the kernel makes only the first, so a
//! label for the second one is an error, and a missing or empty slot after the
//! last output the kernel makes is accepted.
//!
//! `$vector(x, y)` is GeoGebra's own parser form and gives a free vector for
//! any label. `$point(x, y)` gives a free point for any label; GeoGebra's
//! parser also has a `$point` but it follows the label rule, so this is a
//! deliberate difference (see `DIFFERENCES.md`).
//! Both forms can also enclose a coordinate expression to preserve the
//! kind of a renamed point or vector when loading a construction.

use crate::algo::arc::ArcDef;
use crate::algo::conic::ConicDef;
use crate::algo::conic_parts::ConicPartDef;
use crate::algo::general_conic::GeneralConicDef;
use crate::algo::intersect;
use crate::algo::lines::LineDef;
use crate::algo::measure::MeasureDef;
use crate::algo::optimize::OptimizeDef;
use crate::algo::point::PointDef;
use crate::algo::polygon::PolygonDef;
use crate::algo::predicate::PredicateDef;
use crate::algo::transform::TransformDef;
use crate::algo::vector::VectorDef;
use crate::algo::Group;
use crate::kernel::angle::{self, AngleDef, AngleSource, AngleStyle};
use crate::kernel::locus::LocusDef;
use crate::kernel::{Construction, Def, Id, Kind, Value};
use std::fmt;

pub use intersect::growth::{
    boundary_slot_owner, first_slot_owner, free_label_for, growth_owner, restore_boundary_labels,
    BoundaryOutputs,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptError {
    /// 1-based line number.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ScriptError {}

/// Builds a construction from a whole script.
pub fn load(src: &str) -> Result<Construction, ScriptError> {
    load_with_assets(src, &[])
}

/// Replay imported definitions with the already decoded sources.
pub fn load_with_assets(
    src: &str,
    assets: &[saved::Property],
) -> Result<Construction, ScriptError> {
    let mut c = Construction::new();
    for asset in assets {
        asset
            .apply(&mut c)
            .map_err(|message| ScriptError { line: 1, message })?;
    }
    let mut groups = BoundaryOutputs::for_script(src);
    for (i, raw) in src.lines().enumerate() {
        let slider = slider_assignment_target(&c, raw);
        let before = c.len();
        let step_before = c.step();
        let first = exec(&mut c, raw).map_err(|message| ScriptError {
            line: i + 1,
            message,
        })?;
        if let Some(first) = first.filter(|_| c.len() > before) {
            groups.record(&c, raw, first, c.len() - before);
        }
        if c.step() != step_before {
            groups.grow_after_step(&mut c, |_, _, _| {});
        }
        if let Some(id) = slider {
            groups.grow(&mut c, id, |_, _, _| {});
        }
    }
    c.prune_image_assets();
    Ok(c)
}

mod analysis;
pub mod equation;
pub mod image;
pub mod saved;
pub mod text;

/// Runs one line. An assignment returns its first output and a bare label
/// returns the existing object. Directives with no output return `Ok(None)`.
/// Internal objects for inline arguments precede the outputs.
pub fn exec(c: &mut Construction, raw: &str) -> Result<Option<Id>, String> {
    if let Some(asset) = saved::load_image_asset(c, raw) {
        return asset.map(|_| None);
    }
    let (line, comment) = split_comment(raw);
    let line = line.trim();
    let lowered = comment
        .and_then(saved::inline_argument_positions)
        .map(|positions| saved::lower_marked_arguments(line, positions))
        .transpose()?;
    let line = lowered.as_deref().unwrap_or(line);
    if line.is_empty() {
        return Ok(None);
    }
    if let Some(id) = c.id(line) {
        return Ok(Some(id));
    }
    if let Some(property) = saved::parse(line) {
        return property?.apply(c).map(|()| None);
    }
    if let Some(result) = text::apply(c, line) {
        return result.map(|_| None);
    }
    if let Some(result) = equation::coefficients(c, line) {
        return result.map(Some);
    }
    if let Some(style) = equation::parse_style(line) {
        return style?.apply(c).map(|_| None);
    }
    // `SetConstructionStep(n)` is a scripting command: it changes the
    // construction's own state and returns its argument, so a script line
    // of it creates no object (`CmdSetConstructionStep`).
    if let Some(arg) = construction_step_argument(line) {
        return set_construction_step(c, arg).map(|()| None);
    }
    if line.starts_with('{') {
        return exec_labelled(c, line);
    }
    if let Some(args) = line
        .strip_prefix("$angleStyle(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let (label, style) = args
            .split_once(',')
            .ok_or("expected an angle and a style")?;
        let style = style.trim().parse().map_err(|_| "invalid angle style")?;
        c.set_angle_style(object(c, label.trim())?, AngleStyle::from_xml(style))
            .map_err(|e| e.to_string())?;
        return Ok(None);
    }
    let (name, rhs, colon) = assignment(line).ok_or("expected `name = definition`")?;
    if let Some((id, value)) = slider_assignment_value(c, name, rhs) {
        c.assign_slider_number(id, value)
            .map_err(|e| e.to_string())?;
        return Ok(None);
    }
    if let Some(label) = definition_label(name) {
        if let Some(id) = c.resolve_label(label) {
            let original = c.label(id);
            if label != original {
                let name = format!("{original}{}", &name[label.len()..]);
                return c.redefine(id, |c| define_assignment(c, &name, rhs, colon));
            }
            return c.redefine(id, |c| define_assignment(c, name, rhs, colon));
        }
    }
    define_assignment(c, name, rhs, colon)
}

/// The visible label assigned by a single-output definition.
pub fn assignment_label(line: &str) -> Option<&str> {
    assignment(split_comment(line).0).and_then(|(name, _, _)| definition_label(name))
}

/// The existing object a single assignment would replace, including numeric
/// subscript aliases.
pub fn assignment_target(c: &Construction, line: &str) -> Option<Id> {
    c.resolve_label(assignment_label(line)?)
}

fn definition_label(name: &str) -> Option<&str> {
    let label = split_call(name).map_or(name, |(label, _)| label);
    (is_output_label(label) && !label.starts_with('\0')).then_some(label)
}

/// Finds the assignment delimiter before the definition.
fn assignment(line: &str) -> Option<(&str, &str, bool)> {
    let i = line.find([':', '='])?;
    let rhs = &line[i + 1..];
    let colon = line.as_bytes()[i] == b':';
    let (rhs, colon) = if let (true, Some(rhs)) = (colon, rhs.strip_prefix('=')) {
        (rhs, false)
    } else {
        (rhs, colon)
    };
    Some((line[..i].trim(), rhs.trim(), colon))
}

fn define_assignment(
    c: &mut Construction,
    name: &str,
    rhs: &str,
    colon: bool,
) -> Result<Option<Id>, String> {
    if colon {
        if !is_output_label(name) {
            return Err(format!("`{name}` is not a valid object name"));
        }
        check_label(name)?;
        if crate::expr::equation::has_equation_separator(rhs) {
            return crate::expr::equation::define(c, name, rhs).map(Some);
        }
        return define_after_label(c, name, rhs);
    }
    // Named functions use `x`, and also `y` for inequalities.
    let function = name
        .strip_suffix("(x)")
        .map(|f| (f, false))
        .or_else(|| name.strip_suffix("(y)").map(|f| (f, true)));
    if let Some((f, of_y)) = function {
        let f = f.trim_end();
        if !is_output_label(f) {
            return Err(format!("`{f}` is not a valid object name"));
        }
        check_label(f)?;
        // `g(x) = SlowPlot(f)` is a function of its own making
        if !of_y {
            if let Some(("SlowPlot", args)) = split_call(rhs.trim()) {
                return slow_plot(c, f, &args);
            }
        }
        return crate::expr::define_function(c, f, rhs.trim(), of_y).map(Some);
    }
    if let Some((label, arguments)) = split_call(name) {
        if (2..=17).contains(&arguments.len())
            && is_output_label(label)
            && arguments.iter().all(|arg| is_identifier(arg))
        {
            check_label(label)?;
            let distinct = arguments
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>();
            if distinct.len() != arguments.len() {
                return Err("duplicate function parameter".into());
            }
            return crate::expr::define_system_function(c, label, rhs.trim(), &arguments).map(Some);
        }
    }
    if !is_output_label(name) {
        return Err(format!("`{name}` is not a valid object name"));
    }
    check_label(name)?;
    define_after_label(c, name, rhs.trim())
}

/// `SlowPlot(f[, repeat])`: the drawn-up-to-here view of the function `f`.
/// The slider starts at zero and controls how far across the view to plot.
fn slow_plot(c: &mut Construction, name: &str, args: &[&str]) -> Result<Option<Id>, String> {
    if args.is_empty() || args.len() > 2 {
        return Err(format!(
            "`SlowPlot` does not take {} argument(s)",
            args.len()
        ));
    }
    let start = c.checkpoint();
    let result = (|| {
        // the function itself may be written inline, as GeoGebra's argument
        // resolver wraps it
        let function = match c.id(args[0]).filter(|&id| match c.kind(id) {
            Kind::Line => true,
            Kind::Function => {
                !crate::expr::function_signature(c, id).is_some_and(|sig| sig.boolean)
            }
            _ => false,
        }) {
            Some(id) => id,
            None => {
                let label = format!("\u{0}spf{}", c.len());
                let resolved = resolve_geometry_arguments(c, args[0]);
                let line = parse_def(c, resolved.as_deref().unwrap_or(args[0]))
                    .ok()
                    .filter(|def| def.kind() == Kind::Line);
                if let Some(def) = line {
                    c.add(&label, def).map_err(|e| e.to_string())?
                } else {
                    crate::expr::argument_function(c, args[0], &label)
                        .ok_or("SlowPlot needs a real-valued function")?
                }
            }
        };
        // The repeat flag is read as a double and cut to an `int`: `true`
        // and 1 repeat, anything else plays once (`CmdSlowPlot`).
        let mut repeat = 1.0;
        if let Some(arg) = args.get(1) {
            repeat = match *arg {
                "true" => 1.0,
                "false" => 0.0,
                _ => match c.id(arg) {
                    Some(id) if c.kind(id).is_number() => match c.value(id) {
                        Value::Number(v) => v,
                        _ => f64::NAN,
                    },
                    Some(id) if c.kind(id) == Kind::Boolean => match c.value(id) {
                        Value::Bool(b) => f64::from(u8::from(b)),
                        _ => f64::NAN,
                    },
                    Some(_) => return Err(format!("`{arg}` is not a number")),
                    None => crate::expr::argument_number(c, arg)
                        .ok_or_else(|| format!("`{arg}` is not a number"))?,
                },
            };
        }
        // Java's `(int)` cast: NaN is 0, the rest truncates and saturates
        let repeats = repeat as i32 == 1;
        let label = (0..=c.len() / 23 + 1)
            .flat_map(|index| (b'a'..=b'w').map(move |letter| (index, char::from(letter))))
            .map(|(index, letter)| {
                if index == 0 {
                    letter.to_string()
                } else {
                    format!("{letter}_{{{index}}}")
                }
            })
            .find(|label| label != name && c.id(label).is_none() && check_label(label).is_ok())
            .ok_or("no label available for the SlowPlot slider")?;
        let slider = c.add_slider(&label, 0.0, 1.0).map_err(|e| e.to_string())?;
        let mut corner = |index| {
            let label = format!("\0sp{}", c.len());
            let def = crate::algo::point::parse_view_corner(c, &[index])
                .expect("corner of the view")
                .map_err(|e| e.to_string())?;
            c.add(&label, Def::Point(def)).map_err(|e| e.to_string())
        };
        let corners = [corner("1")?, corner("2")?];
        crate::expr::define_slow_plot(c, name, function, slider, corners)?;
        // the slider plays through its interval with the automatic
        // increment of a new number (0.001 for this interval while it plays;
        // 0.01 is only the stored step) and the source graph is hidden
        let mode = if repeats {
            crate::kernel::animation::AnimationMode::Increasing
        } else {
            crate::kernel::animation::AnimationMode::IncreasingOnce
        };
        c.start_animation(
            slider,
            crate::kernel::animation::Animation::playing(0.01, mode),
        );
        c.request_hidden(function);
        Ok(Some(slider))
    })();
    if result.is_err() {
        c.discard_since(start);
    }
    result
}

fn construction_step_argument(input: &str) -> Option<&str> {
    input
        .strip_prefix("SetConstructionStep(")?
        .strip_suffix(')')
        .map(str::trim)
}

/// Whether an accepted script line changes the construction protocol step
/// instead of defining an object. The app journals these lines for replay.
pub fn is_construction_step_command(line: &str) -> bool {
    let line = split_comment(line).0.trim();
    construction_step_argument(line).is_some()
        || line
            .split_once(['=', ':'])
            .is_some_and(|(_, rhs)| construction_step_argument(rhs.trim()).is_some())
}

fn set_construction_step(c: &mut Construction, arg: &str) -> Result<(), String> {
    let value =
        crate::expr::argument_number(c, arg).ok_or_else(|| format!("`{arg}` is not a number"))?;
    c.set_construction_step(value);
    Ok(())
}

/// The source argument of `ToComplex`/`ToPolar`/`ToPoint`: an existing
/// object, or an inline point or arithmetic expression as a hidden object.
fn convert_source(c: &mut Construction, arg: &str) -> Result<Id, String> {
    if let Some(id) = c.id(arg) {
        return Ok(id);
    }
    let label = format!("\u{0}conv{}", c.len());
    for kind in [Kind::Point, Kind::Number] {
        if crate::expr::argument_object(c, arg, &label, kind) {
            return Ok(c.id(&label).unwrap());
        }
    }
    Err(format!("cannot convert `{arg}`"))
}

/// The definition of a labelled line after its name has been read and
/// checked: the forms of the module documentation other than arithmetic.
fn define_after_label(c: &mut Construction, name: &str, rhs: &str) -> Result<Option<Id>, String> {
    if let Some((command, _)) = split_call(rhs) {
        if ConicPartDef::reader_name(command).is_some() && c.id(command).is_some() {
            return crate::expr::define(c, name, rhs, is_vector_label(name)).map(Some);
        }
    }
    let inline = inline_arguments(c, rhs);
    let rhs = inline.as_deref().unwrap_or(rhs);
    if let Some(result) = with_saved_arguments(c, rhs, |c, call| define_after_label(c, name, call))
    {
        return result;
    }
    // `name = SetConstructionStep(n)` runs the script too: a scripting
    // command's returned argument is not labelable, so even the labelled
    // form creates no object (`CmdSetConstructionStep` via the algebra
    // processor).
    if let Some(arg) = construction_step_argument(rhs) {
        return set_construction_step(c, arg).map(|()| None);
    }
    // `name = ToComplex(g)`, `ToPolar(g)` and `ToPoint(g)`: a copy of the
    // value that only shows in another coordinate style; the argument may
    // be an inline point literal or arithmetic, which becomes a hidden
    // object (`AlgoToComplexPolar` accepts point, number, vector, list).
    if let Some((func @ ("ToComplex" | "ToPolar" | "ToPoint"), args)) = split_call(rhs) {
        if args.len() != 1 {
            return Err(format!("`{func}` does not take {} argument(s)", args.len()));
        }
        let mode = match func {
            "ToComplex" => crate::algo::point::CoordMode::Complex,
            "ToPolar" => crate::algo::point::CoordMode::Polar,
            _ => crate::algo::point::CoordMode::Cartesian,
        };
        let start = c.checkpoint();
        let result = convert_source(c, args[0]);
        let result = result.and_then(|source| match c.kind(source) {
            Kind::Vector => c
                .add(
                    name,
                    Def::Vector(crate::algo::vector::VectorDef::Convert { source, mode }),
                )
                .map(Some)
                .map_err(|e| e.to_string()),
            _ => c
                .add(
                    name,
                    Def::Point(crate::algo::point::PointDef::Convert { source, mode }),
                )
                .map(Some)
                .map_err(|e| e.to_string()),
        });
        if result.is_err() {
            c.discard_since(start);
        }
        return result;
    }
    // `name = DynamicCoordinates(P, x, y)`: the point `(x, y)`, an input of
    // the movable point `P` so that dragging it recomputes
    // (`AlgoDynamicCoordinates`); the 4-argument form is 3D.
    if let Some(("DynamicCoordinates", args)) = split_call(rhs) {
        if args.len() == 4 {
            return Err("`DynamicCoordinates` with four arguments needs 3D".into());
        }
        if args.len() != 3 {
            return Err(format!(
                "`DynamicCoordinates` does not take {} argument(s)",
                args.len()
            ));
        }
        let start = c.checkpoint();
        let result = (|| {
            let point = match c.id(args[0]).filter(|&id| c.kind(id) == Kind::Point) {
                Some(id) => id,
                None => {
                    let label = format!("\0dyn{}", c.len());
                    if !crate::expr::argument_object(c, args[0], &label, Kind::Point) {
                        return Err(format!("`{}` is not a point", args[0]));
                    }
                    c.id(&label).unwrap()
                }
            };
            let mut number = |arg: &str, slot: usize| -> Result<Id, String> {
                match c.id(arg).filter(|&id| c.kind(id).is_number()) {
                    Some(id) => Ok(id),
                    None => {
                        let label = format!("\0dyn{}_{slot}", c.len());
                        if crate::expr::argument_object(c, arg, &label, Kind::Number) {
                            Ok(c.id(&label).unwrap())
                        } else {
                            Err(format!("`{arg}` is not a number"))
                        }
                    }
                }
            };
            let def = crate::algo::point::PointDef::Dynamic {
                point,
                x: number(args[1], 1)?,
                y: number(args[2], 2)?,
            };
            c.add(name, Def::Point(def))
                .map(Some)
                .map_err(|e| e.to_string())
        })();
        if result.is_err() {
            c.discard_since(start);
        }
        return result;
    }
    // `name = SlowPlot(f[, repeat])` without a function label is the same
    // function under a plain name
    if let Some(("SlowPlot", args)) = split_call(rhs) {
        return slow_plot(c, name, &args);
    }
    // `name = PenStroke(...)`: literal points frozen into a smoothed stroke
    // (`CmdPenStroke` reads its arguments without resolving them)
    if let Some(("PenStroke", args)) = split_call(rhs) {
        return c.add_pen_stroke(name, &args).map(Some);
    }
    // `name = Maximize(expr, independent)` and `Minimize`: the dependent value
    // may be an inline arithmetic expression, which becomes a hidden
    // expression object, as GeoGebra's argument resolver makes a temporary
    // number of it (`CmdMaximize`/`CmdMinimize`).
    if let Some((func @ ("Maximize" | "Minimize"), args)) = split_call(rhs) {
        if args.len() != 2 {
            return Err(format!("`{func}` does not take {} argument(s)", args.len()));
        }
        let start = c.checkpoint();
        let dependent = match c
            .id(args[0])
            .filter(|&id| matches!(c.kind(id), Kind::Number | Kind::Angle))
        {
            Some(id) => id,
            None => {
                let label = format!("\0dep{}", c.len());
                match crate::expr::define(c, &label, args[0], false) {
                    Ok(id) => id,
                    Err(e) => {
                        c.discard_since(start);
                        return Err(e.to_string());
                    }
                }
            }
        };
        let result = (|| {
            let independent = object(c, args[1])
                .or_else(|_| analysis::argument(c, args[1], Kind::Point))
                .or_else(|_| analysis::argument(c, args[1], Kind::Number))?;
            let def = OptimizeDef::new(c, dependent, independent, func == "Maximize");
            c.add(name, Def::Optimize(def))
                .map(Some)
                .map_err(|e| e.to_string())
        })();
        if result.is_err() {
            c.discard_since(start);
        }
        return result;
    }
    let start = c.checkpoint();
    if let Some(result) = define_inline_region(c, name, rhs) {
        if result.is_err() {
            c.discard_since(start);
        }
        return result;
    }
    let call = match resolve_text_argument(c, rhs) {
        Ok(call) => call.or_else(|| resolve_geometry_arguments(c, rhs)),
        Err(error) => {
            c.discard_since(start);
            return Err(error);
        }
    };
    let resolved = call.as_deref().unwrap_or(rhs);
    let result = match define_resolved(c, name, resolved) {
        Err(error) => {
            c.discard_since(start);
            // The expression evaluator can use scalar and coordinate results
            // directly. Other command outputs need a real hidden object.
            let retry = c.checkpoint();
            if let Some(call) = resolve_defined_arguments(c, rhs) {
                if let Ok(output) = define_resolved(c, name, &call) {
                    if let Some(id) = output {
                        c.remember_construction_source(id, rhs);
                    }
                    return Ok(output);
                }
            }
            c.discard_since(retry);
            return Err(error);
        }
        result => result,
    };
    if let Ok(Some(id)) = result.as_ref() {
        let numeric_expression = c.kind(*id).is_number()
            && crate::expr::plain_number(rhs).is_none()
            && !rhs.ends_with('°')
            && split_call(rhs).is_none();
        let coordinate_expression = c.kind(*id) == Kind::Point
            && rhs
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .is_some_and(|parts| {
                    parts
                        .split(',')
                        .any(|arg| crate::expr::plain_number(arg.trim()).is_none())
                });
        if c.def(*id).is_free() && (numeric_expression || coordinate_expression) {
            c.remember_construction_source(*id, rhs);
        }
    }
    if matches!(result, Ok(None)) && implicit_multi_output(c, resolved) {
        c.discard_since(start);
    }
    result
}

/// The number that a slider assignment such as `b = 10` sets: a growing
/// command that depends on it can reveal more outputs.
pub fn slider_assignment_target(c: &Construction, line: &str) -> Option<Id> {
    let (name, rhs, _) = assignment(split_comment(line).0)?;
    slider_assignment_value(c, name, rhs).map(|(id, _)| id)
}

/// Whether the line assigns a literal value to an existing slider.
pub fn is_slider_assignment(c: &Construction, line: &str) -> bool {
    slider_assignment_target(c, line).is_some()
}

fn slider_assignment_value(c: &Construction, name: &str, rhs: &str) -> Option<(Id, f64)> {
    let id = c.id(name)?;
    c.slider_interval(id)?;
    Some((id, crate::expr::plain_number(rhs)?))
}

fn implicit_multi_output(c: &Construction, rhs: &str) -> bool {
    split_call(rhs).is_some_and(|(command, args)| {
        matches!(command, "Intersect" | "Tangent")
            && args.len() == 2
            && args.iter().any(|arg| {
                c.id(arg)
                    .is_some_and(|id| c.kind(id) == Kind::ImplicitCurve)
            })
    })
}

/// Give a command used as the input of Defined its own hidden object when
/// the expression evaluator does not support its result type.
fn resolve_defined_arguments(c: &mut Construction, rhs: &str) -> Option<String> {
    use crate::expr::lex::{tokens, Tok};
    let tokens = tokens(rhs).ok()?;
    let mut closes = vec![0; tokens.len()];
    let mut opens = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.tok {
            Tok::LParen => opens.push(index),
            Tok::RParen => closes[opens.pop()?] = index,
            _ => (),
        }
    }
    let mut result = String::new();
    let mut end = 0;
    for (index, token) in tokens.iter().enumerate() {
        let Tok::Name { text, call: true } = &token.tok else {
            continue;
        };
        if token.start < end
            || !matches!(text.as_str(), "Defined" | "IsDefined")
            || c.id(text).is_some()
        {
            continue;
        }
        let close = *closes.get(index + 1)?;
        if close <= index + 1 {
            continue;
        }
        let start = tokens[index + 1].end;
        let stop = tokens[close].start;
        let arg = ungroup(rhs[start..stop].trim());
        if split_call(arg).is_none() {
            continue;
        }
        let hidden = saved_argument(c, arg, 0).ok()?;
        result.push_str(&rhs[end..start]);
        result.push_str(&hidden);
        end = stop;
    }
    if end == 0 {
        return None;
    }
    result.push_str(&rhs[end..]);
    Some(result)
}

/// Marks inline geometry for the same lowering used by saved definitions.
fn inline_arguments(c: &Construction, rhs: &str) -> Option<String> {
    let (command, args) = split_call(rhs)?;
    let conic_reader = ConicPartDef::reader_name(command);
    if conic_reader.is_some() && c.id(command).is_some() {
        return None;
    }
    let equations = conic_reader.is_some()
        || matches!(
            command,
            "Intersect"
                | "IntersectPath"
                | "IntersectionPaths"
                | "AngularBisector"
                | "AngleBisector"
                | "Arc"
                | "Sector"
                | "Area"
                | "AreConcurrent"
                | "AreParallel"
                | "ArePerpendicular"
                | "Center"
                | "Circumference"
                | "ConjugateDiameter"
                | "Diameter"
                | "Direction"
                | "Defined"
                | "IsDefined"
                | "IsTangent"
                | "Point"
                | "Radius"
                | "Slope"
                | "OrthogonalVector"
                | "PerpendicularVector"
                | "UnitOrthogonalVector"
                | "UnitPerpendicularVector"
                | "UnitVector"
                | "Tangent"
        );
    let point_arguments = matches!(
        command,
        "Circle"
            | "Polygon"
            | "PolyLine"
            | "Arc"
            | "Sector"
            | "CircleArc"
            | "CircularArc"
            | "CircleSector"
            | "CircularSector"
    ) || (command == "ClosestPoint"
        && args.len() == 2
        && args.iter().any(|arg| c.id(arg).is_some()));
    let mut rewritten = false;
    let args = args
        .iter()
        .map(|&arg| {
            let inline = c.id(arg).is_none()
                && (equations && crate::expr::equation::split_equation(arg).is_ok()
                    || point_arguments && arg.starts_with('(')
                    || conic_reader.is_some() && split_call(arg).is_some()
                    || matches!(
                        (command, split_call(arg).map(|(name, _)| name)),
                        ("PathParameter", Some("Point")) | ("Centroid", Some("Polygon"))
                    ));
            if inline {
                rewritten = true;
                format!("$arg({arg})")
            } else {
                arg.to_owned()
            }
        })
        .collect::<Vec<_>>();
    rewritten.then(|| format!("{command}({})", args.join(", ")))
}

/// Lowers an imported argument within its owning definition. Each temporary
/// object is anonymous and shares the lifetime of that definition's replay.
fn with_saved_arguments(
    c: &mut Construction,
    rhs: &str,
    run: impl FnOnce(&mut Construction, &str) -> Result<Option<Id>, String>,
) -> Option<Result<Option<Id>, String>> {
    if !rhs.contains("$arg(") && !rhs.contains("Maximize") && !rhs.contains("Minimize") {
        return None;
    }
    let (command, args) = split_call(rhs)?;
    let sources: Vec<_> = args
        .iter()
        .map(|&arg| {
            arg.strip_prefix("$arg(")
                .and_then(|s| s.strip_suffix(')'))
                .map(str::trim)
                .or_else(|| {
                    arg.split_once('(')
                        .is_some_and(|(name, _)| {
                            matches!(name.trim(), "Maximize" | "Minimize")
                                && call_closes_at_end(arg, name.len())
                        })
                        .then_some(arg)
                })
        })
        .collect();
    if sources.iter().all(Option::is_none) {
        return None;
    }
    let before = c.len();
    let start = c.checkpoint();
    let mut implicit = false;
    let result = (|| {
        let args = args
            .iter()
            .zip(sources)
            .map(|(&arg, source)| match source {
                Some(source) => saved_argument(c, source, 0),
                None => Ok(arg.to_owned()),
            })
            .collect::<Result<Vec<_>, String>>()?;
        let call = format!("{command}({})", args.join(", "));
        implicit = implicit_multi_output(c, &call);
        run(c, &call)
    })();
    if result.is_err() || (matches!(result, Ok(None)) && implicit) {
        c.discard_since(start);
    } else {
        let source = saved::command_with_inline_arguments(rhs).map(|(source, _)| source);
        for index in before..c.len() {
            let id = Id(index);
            if !c.label(id).starts_with('\0') {
                c.remember_construction_source(id, source.as_deref().unwrap_or(rhs));
            }
        }
    }
    Some(result)
}

fn saved_argument(c: &mut Construction, source: &str, depth: usize) -> Result<String, String> {
    if depth >= 32 {
        return Err("nested command inputs are too deep".into());
    }
    if let Some(id) = c.id(source) {
        return Ok(c.label(id).to_owned());
    }
    let start = c.checkpoint();
    let label = format!("\0arg{}", c.len());
    let call = split_call(source);
    if let Some(("Polygon", args)) = &call {
        return inline_polygon(c, &label, args).map(|_| label);
    }
    if let Some(("Vector", args)) = &call {
        if let [arg] = args.as_slice() {
            if c.id(arg).is_some_and(|id| c.kind(id) == Kind::Vector) {
                return Ok((*arg).to_owned());
            }
        }
    }
    if call.is_some() {
        if let Ok(def) = parse_def(c, source) {
            if c.add(&label, def).is_ok() {
                return Ok(label);
            }
        }
    }
    let optimize = matches!(call.as_ref(), Some(("Maximize" | "Minimize", _)));
    let vector = matches!(call.as_ref(), Some(("Vector", _)));
    if !optimize && !vector && crate::expr::define_saved_argument(c, &label, source).is_ok() {
        return Ok(label);
    }
    let Some((command, args)) = call else {
        if crate::expr::equation::split_equation(source).is_ok() {
            return crate::expr::equation::define(c, &label, source).map(|_| label);
        }
        return Err(format!("cannot evaluate command input `{source}`"));
    };
    let args = args
        .into_iter()
        .map(|arg| {
            if !optimize && crate::expr::plain_number(arg).is_some() {
                Ok(arg.to_owned())
            } else {
                saved_argument(c, arg, depth + 1)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let ("Vector", [arg]) = (command, args.as_slice()) {
        if c.id(arg).is_some_and(|id| c.kind(id) == Kind::Vector) {
            return Ok(arg.clone());
        }
    }
    let first_output = c.len();
    let label = format!("\0arg{first_output}");
    let id = define_after_label(c, &label, &format!("{command}({})", args.join(", ")))
        .map_err(|error| format!("nested `{command}`: {error}"))?;
    if id.is_some_and(|id| c.label(id) == label) && c.len() == first_output + 1 {
        Ok(label)
    } else {
        c.discard_since(start);
        Err(format!("nested `{command}` must produce a single object"))
    }
}

fn define_resolved(c: &mut Construction, name: &str, rhs: &str) -> Result<Option<Id>, String> {
    if rhs.starts_with('$') {
        if let Some((form @ ("$checkBox" | "$button" | "$inputBox"), args)) = split_call(rhs) {
            return crate::kernel::control::define(c, name, form, &args).map(Some);
        }
    }
    if rhs.starts_with("$image(") {
        return image::define(c, name, rhs).map(Some);
    }
    if let Some(result) = define_saved(c, name, rhs) {
        return result.map(Some);
    }
    if let Some(result) = analysis::exec(c, name, rhs) {
        return result.map(Some);
    }
    if let Some(def) = intersect::growth::parse_slot(c, rhs) {
        let (def, owner) = def?;
        if let Some(id) = intersect::growth::existing_slot(c, owner, def) {
            return c
                .relabel_output(id, name)
                .map(|()| Some(id))
                .map_err(|e| e.to_string());
        }
        return c
            .add_output(name, def, owner)
            .map(Some)
            .map_err(|e| e.to_string());
    }
    if let Some((form @ ("$point" | "$vector"), args)) = split_call(rhs) {
        match forced_coordinates(form, &args) {
            Ok(def) => return c.add(name, def).map(Some).map_err(|e| e.to_string()),
            Err(error) if args.len() > 2 => return Err(error),
            Err(_) => {}
        }
        let open = rhs.find('(').expect("forced forms have parentheses");
        return crate::expr::define_coordinates(c, name, &rhs[open..], form == "$vector").map(Some);
    }
    if rhs.starts_with("Vertex") || rhs.starts_with("Corner") {
        if let Some((func @ ("Vertex" | "Corner"), args)) = split_call(rhs) {
            if let Some(source) = PointDef::inequality_source(c, func, &args) {
                if crate::expr::inequality::fails(c, source) {
                    return Err(INEQUALITY_FAILURE.into());
                }
                let count = crate::expr::inequality::preview_vertices(c, source)
                    .len()
                    .max(1);
                let defs = PointDef::inequality_slots(c, source, count);
                return add_outputs(c, name, defs).map(Some);
            }
            // A boolean expression becomes an unlabeled helper, which has
            // no checkbox drawable.
            let helper;
            let mut args = args;
            if let [source, _] = args[..] {
                if c.id(source).is_none() {
                    let label = format!("\0arg{}", c.len());
                    let checkpoint = c.checkpoint();
                    if crate::expr::argument_object(c, source, &label, Kind::Boolean) {
                        helper = label;
                        args[0] = &helper;
                    } else {
                        c.discard_since(checkpoint);
                    }
                }
            }
            let def = PointDef::parse_image_vertex(c, &args)
                .or_else(|| PointDef::parse_segment_vertex(c, func, &args))
                .or_else(|| PointDef::parse_drawable_corner(c, &args))
                .or_else(|| crate::algo::point::parse_view_corner(c, &args));
            if let Some(def) = def {
                return c
                    .add(name, Def::Point(def?))
                    .map(Some)
                    .map_err(|e| e.to_string());
            }
        }
    }
    if let Some(result) = crate::expr::define_relation(c, name, rhs) {
        return result.map(Some);
    }
    if let Some(result) = crate::algo::prover::define(c, name, rhs) {
        return result.map(Some);
    }
    if let Some(result) = crate::expr::define_geometry_if(c, name, rhs) {
        return result.map(Some);
    }
    if let Some(result) = crate::expr::equation::implicit::asymptote_command(c, name, rhs) {
        return result.map(Some);
    }
    if let Some(result) = crate::expr::equation::implicit::command(c, name, rhs) {
        return result.map(Some);
    }
    if let Some(result) = crate::expr::equation::define_side(c, name, rhs) {
        return result.map(Some);
    }
    if let Some(result) = crate::expr::division_list(c, name, rhs) {
        return result.map(Some);
    }
    if let Some(error) = crate::expr::refuse_empty_simplify(c, rhs) {
        return Err(error);
    }
    if let Some(result) = crate::algo::discrete::exec(c, name, rhs) {
        return result.map(Some);
    }
    if let Some(result) = exec_polyline_flag(c, name, rhs) {
        return result;
    }
    if let Some(result) = crate::algo::charts::exec(c, name, rhs) {
        return result.map(Some);
    }
    if let Some(result) = crate::algo::vector::matrix::exec(c, name, rhs) {
        return result;
    }
    if let Some(result) = crate::algo::polygon::exec(c, name, rhs) {
        return result;
    }
    if let Some(result) = crate::algo::general_conic::exec(c, name, rhs) {
        return result;
    }
    if let Some(result) = ConicDef::exec_interval(c, name, rhs) {
        return result;
    }
    if let Some((command @ "Tangent", args)) = split_call(rhs) {
        if let Some(defs) = crate::algo::implicit::tangent::Tangent::multi(c, command, &args) {
            return if defs.is_empty() {
                Ok(None)
            } else {
                add_outputs(c, name, defs).map(Some)
            };
        }
    }
    if let Some(result) = intersect::exec_selected(c, name, rhs) {
        return result;
    }
    if let Some(result) = define_vector_call(c, name, rhs) {
        return result;
    }
    if let Some(result) = crate::algo::calculus::exec(c, name, rhs) {
        return result;
    }
    // GeoGebra returns an already labelled angle unchanged.
    if rhs.starts_with("Angle") {
        if let Some(("Angle", args)) = split_call(rhs) {
            if let [arg] = args.as_slice() {
                if c.id(arg).is_some_and(|id| c.kind(id) == Kind::Angle) {
                    return Ok(None);
                }
            }
        }
    }
    define_expression_or_command(c, name, rhs).map(Some)
}

fn define_saved(c: &mut Construction, name: &str, rhs: &str) -> Option<Result<Id, String>> {
    if !rhs.starts_with("$saved(")
        && !rhs.starts_with("$pointOnPath(")
        && !rhs.starts_with("$homogeneousPoint(")
        && !rhs.starts_with("$slider(")
        && !rhs.starts_with("$penStroke(")
    {
        return None;
    }
    let mut parse = || {
        let (form, args) = split_call(rhs).ok_or("invalid saved definition")?;
        match (form, args.as_slice()) {
            ("$slider", [value, min, max]) => c
                .add_saved_slider(
                    name,
                    parse_number(value)?,
                    parse_number(min)?,
                    parse_number(max)?,
                )
                .map_err(|e| e.to_string()),
            ("$saved", [literal, _definition]) if literal.starts_with("$penStroke(") => {
                match split_call(literal) {
                    Some(("$penStroke", coords)) => c.add_saved_stroke(name, &coords),
                    _ => Err("invalid saved stroke".into()),
                }
            }
            ("$saved", [literal, _definition]) => {
                let def = if let Some(def) = parse_forced(literal) {
                    def?
                } else if let Some(radians) = literal.strip_suffix(" rad") {
                    Def::Angle(parse_number(radians)?)
                } else {
                    Def::Number(parse_number(literal)?)
                };
                c.add(name, def).map_err(|e| e.to_string())
            }
            ("$homogeneousPoint", [x, y, z]) => c
                .add_saved_homogeneous_point(
                    name,
                    [parse_number(x)?, parse_number(y)?, parse_number(z)?],
                )
                .map_err(|e| e.to_string()),
            ("$penStroke", coords) => c.add_saved_stroke(name, coords),
            ("$pointOnPath", [curve, parameter]) => c
                .add_saved_curve_point(name, object(c, curve)?, parse_number(parameter)?)
                .map_err(|e| e.to_string()),
            ("$pointOnPath", [path, x, y, bounds @ ..]) => {
                let path = object(c, path)?;
                let (x, y) = (parse_number(x)?, parse_number(y)?);
                let bounds = match bounds {
                    [] => None,
                    [min, max] => Some([parse_number(min)?, parse_number(max)?]),
                    _ => return Err("invalid function path bounds".into()),
                };
                c.add_saved_path_point(name, path, x, y, bounds)
                    .map_err(|e| e.to_string())
            }
            _ => Err("invalid saved definition".into()),
        }
    };
    Some(parse())
}

/// The journal line that keeps the points of the pen stroke `id` made by
/// `line`: `label = $saved($penStroke(...), definition)`, so that replaying
/// it restores the frozen points whatever the view, and the definition
/// stays as typed. `None` for any other object or a line already saved.
pub fn frozen_stroke_line(c: &Construction, id: Id, line: &str) -> Option<String> {
    if c.kind(id) != Kind::PenStroke || split_comment(line).1.is_some() {
        return None;
    }
    let separator = line.find([':', '='])?;
    let (lhs, rhs) = (line[..separator].trim(), line[separator + 1..].trim());
    if rhs.starts_with('$') {
        return None;
    }
    // an undefined stroke has no points that a view could change
    let stroke = c.stroke(id).filter(|stroke| stroke.defined)?;
    let saved = crate::algo::stroke::saved_definition(stroke);
    Some(format!("{lhs} = $saved({saved}, {rhs})"))
}

/// The definition text saved with an independent object's exact value.
pub fn saved_definition(rhs: &str) -> Option<&str> {
    if rhs.starts_with("$saved(") {
        let ("$saved", args) = split_call(rhs)? else {
            return None;
        };
        if let [_, definition] = args.as_slice() {
            return Some(definition);
        }
    }
    None
}

fn define_cartesian_curve(c: &mut Construction, name: &str, args: &[&str]) -> Result<Id, String> {
    let [x, y, parameter, start, end] = *args else {
        return Err("`Curve` needs two coordinates, a parameter and two bounds".into());
    };
    if !is_identifier(parameter) {
        return Err("`Curve` needs a parameter name".into());
    }
    let checkpoint = c.checkpoint();
    let result = (|| {
        let packed = |id: Id| u32::try_from(id.0).map_err(|_| "too many objects".to_owned());
        let mut coordinate = |text| {
            let label = format!("\0arg{}", c.len());
            packed(crate::expr::define_parametric_coordinate(
                c, &label, text, parameter,
            )?)
        };
        let coordinates = [coordinate(x)?, coordinate(y)?];
        let mut bound = |text| {
            if let Some(id) = c.id(text) {
                return packed(id);
            }
            let label = format!("\0arg{}", c.len());
            if crate::expr::argument_object(c, text, &label, Kind::Number) {
                packed(object(c, &label)?)
            } else {
                Err(format!("`Curve` needs a numeric bound, got `{text}`"))
            }
        };
        let bounds = [bound(start)?, bound(end)?];
        c.add(
            name,
            Def::Measure(MeasureDef::CartesianCurve {
                coordinates,
                bounds,
            }),
        )
        .map_err(|e| e.to_string())
    })();
    if result.is_err() {
        c.discard_since(checkpoint);
    }
    result
}

fn define_expression_or_command(c: &mut Construction, name: &str, rhs: &str) -> Result<Id, String> {
    match define(c, name, rhs) {
        Ok(id) => {
            if matches!(c.def(id), Def::Number(_)) {
                // a decimal literal keeps its exact decimal
                crate::expr::record_literal(c, id, rhs);
            }
            Ok(id)
        }
        // A call of a command keeps the command's own error.
        Err(e) if is_single_call(c, rhs) => Err(e),
        Err(_) => {
            let vector = !sum_fold_point(c, rhs)
                && (is_vector_label(name)
                    || rhs.split_once('(').is_some_and(|(name, _)| {
                        VectorDef::vector_result(name.trim()) && call_closes_at_end(rhs, name.len())
                    }));
            crate::expr::define(c, name, rhs, vector)
        }
    }
}

/// `Sum` of three or more point arguments folds to a point, independently
/// of the output label.
fn sum_fold_point(c: &Construction, rhs: &str) -> bool {
    let Some(("Sum", args)) = split_call(rhs) else {
        return false;
    };
    args.len() >= 3
        && ({
            let first = args[0].trim();
            matches!(parse_def(c, first), Ok(Def::FreePoint { .. }))
                || c.id(first).is_some_and(|id| c.kind(id) == Kind::Point)
        })
}

/// `CmdVector` keeps a labelled vector and labels an anonymous one.
fn define_vector_call(
    c: &mut Construction,
    name: &str,
    rhs: &str,
) -> Option<Result<Option<Id>, String>> {
    let (command, rest) = rhs.split_once('(')?;
    if command.trim() != "Vector" || !call_closes_at_end(rhs, command.len()) {
        return None;
    }
    let arg = rest.strip_suffix(')')?.trim();
    if !arg.starts_with('\0') && c.id(arg).is_some_and(|id| c.kind(id) == Kind::Vector) {
        return Some(Ok(None));
    }
    match parse_forced(arg)? {
        Ok(def @ Def::Vector(_)) => Some(c.add(name, def).map(Some).map_err(|e| e.to_string())),
        _ => None,
    }
}

/// The forms of the module documentation other than arithmetic.
fn define(c: &mut Construction, name: &str, rhs: &str) -> Result<Id, String> {
    if let Some(("Slider", args)) = split_call(rhs) {
        return slider(c, name, &args);
    }
    if let Some(defs) = parse_multi(c, rhs) {
        return add_outputs(c, name, defs?);
    }
    let def = match parse_def(c, rhs)? {
        Def::FreePoint { x, y } if is_vector_label(name) => Def::Vector(VectorDef::Free { x, y }),
        def => def,
    };
    c.add(name, def).map_err(|e| e.to_string())
}

/// Numeric `Slider` overloads with optional animation increment and speed.
fn slider(c: &mut Construction, name: &str, args: &[&str]) -> Result<Id, String> {
    let number = |c: &Construction, arg| {
        if matches!(ungroup(arg), "?" | "NaN") {
            return Some(f64::NAN);
        }
        crate::expr::constant_number(c, arg)
            .filter(|&(_, angle)| !angle)
            .map(|(value, _)| value)
    };
    let [min, max, options @ ..] = args else {
        return Err(format!("`Slider` does not take {} argument(s)", args.len()));
    };
    if options.len() > 2 {
        return Err(format!("`Slider` does not take {} argument(s)", args.len()));
    }
    for &arg in options {
        if c.numeric_value(arg).is_none() {
            return Err(format!("expected a number, got `{arg}`"));
        }
    }
    let id = if let (Some(min), Some(max)) = (number(c, min), number(c, max)) {
        c.add_slider(name, min, max).map_err(|e| e.to_string())?
    } else {
        c.add_expression_slider(name, [min, max])?
    };
    c.set_slider_animation_options(id, options);
    Ok(id)
}

/// `CmdPolyLine` reads its trailing flag at creation. A false flag leaves
/// the ordinary point-based polyline, including its live point dependencies.
fn exec_polyline_flag(
    c: &mut Construction,
    name: &str,
    rhs: &str,
) -> Option<Result<Option<Id>, String>> {
    if !rhs.starts_with("PolyLine") {
        return None;
    }
    let ("PolyLine", args) = split_call(rhs)? else {
        return None;
    };
    let [first, _, .., flag] = args.as_slice() else {
        return None;
    };
    if !c.id(first).is_some_and(|id| c.kind(id) == Kind::Point) {
        return None;
    }
    let flag = match c.id(flag) {
        Some(id) if c.kind(id) == Kind::Boolean => match c.value(id) {
            Value::Bool(flag) => flag,
            _ => return Some(Err("`PolyLine` needs a defined boolean flag".into())),
        },
        Some(_) => return None,
        None => match *flag {
            "false" => false,
            "true" => true,
            _ => return None,
        },
    };
    if flag {
        return Some(Err(
            "the pen stroke form of `PolyLine` is unsupported".into()
        ));
    }
    let (points, _) = rhs.rsplit_once(',')?;
    crate::algo::polygon::exec(c, name, &format!("{points})"))
}

/// Whether `rhs` is exactly one call `Name(...)` of a command: the bracket
/// after the name closes at the end, and the name is neither a built-in
/// function nor an object.
fn is_single_call(c: &Construction, rhs: &str) -> bool {
    let Some(open) = rhs.find('(') else {
        return false;
    };
    let name = rhs[..open].trim();
    // `sin²(a)` is a power of a function, not a call of `sin²`
    if !is_identifier(name)
        || !name.chars().all(crate::expr::is_name_char)
        || crate::expr::is_function_name(name)
        || crate::expr::is_expression_command(name)
        || c.id(name).is_some()
    {
        return false;
    }
    call_closes_at_end(rhs, open)
}

/// Split off a script comment, including its leading `#`. A matched string
/// can contain a hash. An unterminated string leaves its first hash available
/// as the comment marker, as in scripts that predate string literals.
pub fn split_comment(line: &str) -> (&str, Option<&str>) {
    let Some(first_hash) = line.find('#') else {
        return (line, None);
    };
    if !line[..first_hash].contains('"') {
        return (&line[..first_hash], Some(&line[first_hash..]));
    }
    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
        } else {
            match ch {
                '\\' if quote.is_some() => escaped = true,
                '"' => quote = if quote.is_some() { None } else { Some(index) },
                '#' if quote.is_none() => return (&line[..index], Some(&line[index..])),
                _ => {}
            }
        }
    }
    if let Some(start) = quote {
        if let Some(hash) = line[start..].find('#') {
            let index = start + hash;
            return (&line[..index], Some(&line[index..]));
        }
    }
    (line, None)
}

/// The byte position of the first matching character outside a string literal.
pub fn find_unquoted(text: &str, ch: char) -> Option<usize> {
    unquoted(text).find_map(|(i, c)| (c == ch).then_some(i))
}

fn unquoted(text: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    let mut quoted = false;
    let mut escaped = false;
    text.char_indices().filter(move |&(_, c)| {
        if escaped {
            escaped = false;
        } else if quoted && c == '\\' {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        } else {
            return !quoted;
        }
        false
    })
}

fn call_closes_at_end(rhs: &str, open: usize) -> bool {
    let mut depth = 0usize;
    for (i, ch) in unquoted(rhs).skip_while(|&(i, _)| i < open) {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1 == rhs.len();
                }
            }
            _ => {}
        }
    }
    false
}

pub(crate) fn ungroup(source: &str) -> &str {
    if !source.starts_with('(') {
        return source;
    }
    let mut opens = Vec::new();
    let mut groups = Vec::new();
    for (i, ch) in unquoted(source) {
        match ch {
            '(' => opens.push(i),
            ')' => {
                let Some(open) = opens.pop() else {
                    return source;
                };
                groups.push((open, i));
            }
            _ => (),
        }
    }
    let mut start = 0;
    let mut end = source.len();
    for (open, close) in groups.into_iter().rev() {
        if start != open || close + 1 != end {
            break;
        }
        let inner = &source[open + 1..close];
        start = open + 1 + inner.len() - inner.trim_start().len();
        end = open + 1 + inner.trim_end().len();
        if start >= end {
            return "";
        }
    }
    &source[start..end]
}

/// `Name(args)` split into the name and the trimmed arguments; `None` for
/// anything else (a point literal, a number, a malformed line).
pub fn split_call(rhs: &str) -> Option<(&str, Vec<&str>)> {
    if rhs.starts_with('(') {
        return None;
    }
    let open = rhs.find('(')?;
    let inner = rhs[open + 1..].strip_suffix(')')?;
    let mut args = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    for (i, ch) in unquoted(inner) {
        match ch {
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => {
                args.push(inner[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    args.push(inner[start..].trim());
    Some((rhs[..open].trim(), args))
}

/// Give unlabelled points and regions the same dependencies as named ones.
fn define_inline_region(
    c: &mut Construction,
    label: &str,
    rhs: &str,
) -> Option<Result<Option<Id>, String>> {
    if !rhs.starts_with("IsInRegion") {
        return None;
    }
    let ("IsInRegion", args) = split_call(rhs)? else {
        return None;
    };
    let &[point, region] = args.as_slice() else {
        return None;
    };
    Some((|| {
        let a = relation_or_region_argument(c, point)?;
        let b = relation_or_region_argument(c, region)?;
        c.add(label, Def::Predicate(PredicateDef::InRegion(a, b)))
            .map(Some)
            .map_err(|e| e.to_string())
    })())
}

pub(crate) fn relation_or_region_argument(c: &mut Construction, arg: &str) -> Result<Id, String> {
    if let Some(id) = c.id(arg) {
        return Ok(id);
    }
    let call = match split_call(arg) {
        Some(("Circle", args)) if args.len() == 2 => {
            let a = relation_or_region_argument(c, args[0])?;
            let radius = if crate::expr::plain_number(args[1]).is_some() {
                args[1]
            } else {
                let b = relation_or_region_argument(c, args[1])?;
                c.label(b)
            };
            format!("Circle({}, {radius})", c.label(a))
        }
        _ => arg.to_owned(),
    };
    let label = format!("\0region{}", c.len());
    if crate::expr::equation::split_equation(&call).is_ok() {
        return crate::expr::equation::define(c, &label, &call);
    }
    define_expression_or_command(c, &label, &call)
}

/// A formatting command accepts a geometry command as its first argument.
fn resolve_text_argument(c: &mut Construction, rhs: &str) -> Result<Option<String>, String> {
    let Some((name @ ("Text" | "FormulaText" | "LaTeX"), args)) = split_call(rhs) else {
        return Ok(None);
    };
    let Some((first, rest)) = args.split_first() else {
        return Ok(None);
    };
    let Some((command, args)) = split_call(first) else {
        return Ok(None);
    };
    if crate::expr::is_expression_command(command) || crate::expr::accepts_text_argument(c, first) {
        return Ok(None);
    }
    let label = format!("\0textarg{}", c.len());
    let id = if command == "Polygon" && args.len() >= 2 {
        inline_polygon(c, &label, &args)?
    } else {
        define_after_label(c, &label, first)?.ok_or("the text argument has no value")?
    };
    c.remember_construction_source(id, first);
    Ok(Some(format!(
        "{name}({})",
        std::iter::once(label.as_str())
            .chain(rest.iter().copied())
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

/// Builds the polygon value of a nested command without publishing its sides.
fn inline_polygon(c: &mut Construction, label: &str, args: &[&str]) -> Result<Id, String> {
    let mut points = Vec::with_capacity(args.len());
    for token in args {
        if let Some(id) = c.id(token).filter(|&id| c.kind(id) == Kind::Point) {
            points.push(id);
        } else {
            let point = format!("\0polypoint{}", c.len());
            if crate::expr::argument_object(c, token, &point, Kind::Point) {
                points.push(c.id(&point).unwrap());
            } else {
                break;
            }
        }
    }
    let polygon = if points.len() == 2 && args.len() == 3 {
        let n = crate::algo::polygon::count_arg(c, args[2])?;
        c.add(
            label,
            Def::Polygon(crate::algo::polygon::PolygonDef::Regular {
                a: points[0],
                b: points[1],
                n,
            }),
        )
    } else if points.len() == args.len() && points.len() >= 3 {
        c.add_with_list(label, &points, |list| {
            Def::Polygon(crate::algo::polygon::PolygonDef::Polygon(list))
        })
    } else {
        return Err("expected points or two points and a vertex count".into());
    };
    polygon.map_err(|err| err.to_string())
}

/// Give inline numeric and list inputs a live dependency, as for the
/// named inputs of these commands. Their normal dispatch reads the labels.
fn resolve_geometry_arguments(c: &mut Construction, rhs: &str) -> Option<String> {
    let (name, _) = rhs.split_once('(')?;
    if matches!(name.trim(), "Corner" | "Vertex") {
        if let Some((func, args)) = split_call(rhs) {
            if let Some(id) = PointDef::inline_inequality_source(c, func, &args) {
                return Some(format!("{func}({})", c.label(id)));
            }
        }
    }
    if matches!(name.trim(), "Locus" | "SlopeField") {
        let (func, args) = split_call(rhs)?;
        let first = match (func, args.as_slice()) {
            ("Locus", [slope, point])
                if c.id(point).is_some_and(|id| c.kind(id) == Kind::Point) =>
            {
                *slope
            }
            ("SlopeField", [slope]) => *slope,
            _ => return None,
        };
        if c.id(first).is_none() {
            let label = format!("\0arg{}", c.len());
            crate::expr::define_slope(c, &label, first).ok()?;
            let suffix = if args.len() == 2 {
                format!(", {}", args[1])
            } else {
                String::new()
            };
            return Some(format!("{func}({label}{suffix})"));
        }
    }
    if !matches!(
        name.trim(),
        "Tangent"
            | "Length"
            | "Line"
            | "Vertex"
            | "Corner"
            | "Point"
            | "Barycenter"
            | "Polygon"
            | "PolyLine"
            | "Arc"
            | "Sector"
            | "Circle"
            | "LowerSum"
            | "UpperSum"
            | "LeftSum"
            | "TrapezoidalSum"
            | "RectangleSum"
            | "Integral"
            | "NIntegral"
            | "IntegralBetween"
            | "Dilate"
            | "Rotate"
            | "Derivative"
            | "NDerivative"
    ) {
        return None;
    }
    let (func, mut args) = split_call(rhs)?;
    let inline_source = match (func, args.first().and_then(|arg| split_call(arg))) {
        ("Derivative" | "NDerivative", Some(("Curve" | "CurveCartesian", coords))) => {
            let label = format!("\0curve{}", c.len());
            define_cartesian_curve(c, &label, &coords)
                .ok()
                .map(|_| label)
        }
        ("Point", Some(("Segment", _))) => {
            let label = format!("\0path{}", c.len());
            let def = parse_def(c, args[0]).ok()?;
            c.add(&label, def).ok().map(|_| label)
        }
        _ => None,
    };
    if let Some(source) = &inline_source {
        args[0] = source;
    }
    let is_kind = |arg, kind| c.id(arg).is_some_and(|id| c.kind(id) == kind);
    let inputs: &[(usize, Kind)] = match (func, args.as_slice()) {
        ("Line", [_, _]) => &[(0, Kind::Point), (1, Kind::Point)],
        ("Point", [_]) if inline_source.is_some() => &[],
        ("Tangent", [f, _]) if is_kind(f, Kind::ImplicitCurve) => &[(1, Kind::Point)],
        ("Tangent", [_, f]) if is_kind(f, Kind::ImplicitCurve) => &[(0, Kind::Point)],
        ("Tangent", [f, _]) if is_kind(f, Kind::Function) => &[(1, Kind::Point)],
        ("Tangent", [_, f]) if is_kind(f, Kind::Curve) => &[(0, Kind::Point)],
        ("Tangent", [_, f]) if is_kind(f, Kind::Function) => &[(0, Kind::Number)],
        ("Length", [f, _, _]) if is_kind(f, Kind::Function) || is_kind(f, Kind::Curve) => {
            &[(1, Kind::Number), (2, Kind::Number)]
        }
        ("Vertex" | "Corner", [s, _]) if is_kind(s, Kind::Segment) => &[(1, Kind::Number)],
        ("Vertex" | "Corner", [source, _])
            if [Kind::Image, Kind::Text, Kind::Boolean]
                .into_iter()
                .any(|kind| is_kind(source, kind)) =>
        {
            &[(1, Kind::Number)]
        }
        ("Point", [path, _])
            if c.id(path)
                .is_some_and(|id| crate::algo::path::PATH.contains(&c.kind(id))) =>
        {
            &[(1, Kind::Number)]
        }
        ("Arc" | "Sector", [path, _, _])
            if is_kind(path, Kind::Circle) || is_kind(path, Kind::Conic) =>
        {
            &[(1, Kind::Number), (2, Kind::Number)]
        }
        ("Point" | "Polygon" | "PolyLine", [_]) | ("PolyLine", [_, _]) => &[(0, Kind::List)],
        ("Barycenter", [_, _]) => &[(0, Kind::List), (1, Kind::List)],
        ("Circle", [_, radius]) if crate::expr::plain_number(radius).is_none() => {
            &[(0, Kind::Point), (1, Kind::Number), (1, Kind::Point)]
        }
        ("Circle", [_, _]) => &[(0, Kind::Point)],
        ("Rotate", [_, _] | [_, _, _]) => &[(1, Kind::Number)],
        ("Dilate", [src, _]) if is_kind(src, Kind::Curve) || is_kind(src, Kind::Image) => {
            &[(1, Kind::Number)]
        }
        ("Dilate", [src, _, _]) if is_kind(src, Kind::Curve) || is_kind(src, Kind::Image) => {
            &[(1, Kind::Number), (2, Kind::Point)]
        }
        // the calculus commands: their number and boolean positions, in
        // the order the commands check them
        ("LowerSum" | "UpperSum" | "LeftSum" | "TrapezoidalSum", [f, _, _, _])
            if is_kind(f, Kind::Function) =>
        {
            &[(1, Kind::Number), (2, Kind::Number), (3, Kind::Number)]
        }
        ("RectangleSum", [f, _, _, _, _]) if is_kind(f, Kind::Function) => &[
            (1, Kind::Number),
            (2, Kind::Number),
            (3, Kind::Number),
            (4, Kind::Number),
        ],
        ("Integral" | "NIntegral", [f, _, _]) if is_kind(f, Kind::Function) => {
            &[(1, Kind::Number), (2, Kind::Number)]
        }
        // the boolean fourth argument is tried as a number first, so an
        // inline `true` or `false` falls through to the boolean row
        ("Integral" | "NIntegral" | "IntegralBetween", [f, _, _, _])
            if is_kind(f, Kind::Function) =>
        {
            &[
                (1, Kind::Number),
                (2, Kind::Number),
                (3, Kind::Number),
                (3, Kind::Boolean),
            ]
        }
        ("Integral" | "NIntegral" | "IntegralBetween", [f, _, _, _, _])
            if is_kind(f, Kind::Function) =>
        {
            &[
                (1, Kind::Number),
                (2, Kind::Number),
                (3, Kind::Number),
                (3, Kind::Boolean),
                (4, Kind::Boolean),
            ]
        }
        ("IntegralBetween", [f, _, _]) if is_kind(f, Kind::Function) => {
            &[(1, Kind::Number), (2, Kind::Number)]
        }
        ("Derivative" | "NDerivative", [_]) => &[],
        ("Derivative" | "NDerivative", [_, _]) => &[(1, Kind::Number)],
        ("Derivative" | "NDerivative", [_, _, _]) => &[(2, Kind::Number)],
        _ => return None,
    };
    // the calculus commands keep their numbers as live objects; the
    // geometry commands parse a plain decimal into their own definition
    let live_numbers = matches!(
        func,
        "Dilate"
            | "Rotate"
            | "LowerSum"
            | "UpperSum"
            | "LeftSum"
            | "TrapezoidalSum"
            | "RectangleSum"
            | "Integral"
            | "NIntegral"
            | "IntegralBetween"
            | "Derivative"
            | "NDerivative"
    );
    let replacements: Vec<_> = inputs
        .iter()
        .filter_map(|&(index, kind)| {
            let arg = args[index];
            if c.id(arg).is_some()
                || (!(live_numbers || matches!(func, "Point" | "Arc" | "Sector" | "Circle"))
                    && crate::algo::polygon::decimal(arg).is_some())
            {
                return None;
            }
            let label = format!("\0input{}", c.len());
            let resolved = crate::expr::argument_object(c, arg, &label, kind)
                || (func == "Tangent"
                    && kind == Kind::Number
                    && crate::expr::argument_object(c, arg, &label, Kind::Point));
            resolved.then_some((index, label))
        })
        .collect();
    for (index, label) in &replacements {
        args[*index] = label;
    }
    (inline_source.is_some() || !replacements.is_empty())
        .then(|| format!("{func}({})", args.join(", ")))
}

/// Asks the groups for a command that creates several objects.
fn parse_multi(c: &Construction, rhs: &str) -> Option<Result<Vec<Def>, String>> {
    fn wrap<G: Group>(r: Result<Vec<G>, String>, f: fn(G) -> Def) -> Result<Vec<Def>, String> {
        r.map(|defs| defs.into_iter().map(f).collect())
    }
    let (func, args) = split_call(rhs)?;
    if let Some(defs) = analysis::existing_outputs(c, func, &args) {
        return Some(defs);
    }
    if let Some(defs) = crate::algo::implicit::tangent::Tangent::multi(c, func, &args) {
        return Some(Ok(defs));
    }
    if let Some(defs) = intersect::parse_multi(c, func, &args) {
        return Some(Ok(defs));
    }
    if let Some(source) = PointDef::inequality_source(c, func, &args) {
        if crate::expr::inequality::fails(c, source) {
            return Some(Err(INEQUALITY_FAILURE.into()));
        }
        let count = crate::expr::inequality::preview_vertices(c, source)
            .len()
            .max(1);
        return Some(Ok(PointDef::inequality_slots(c, source, count)));
    }
    if func == "Angle" {
        if let Some(defs) = angle::fixed(c, &args) {
            return Some(defs.map(|defs| defs.into_iter().map(Def::AngleObject).collect()));
        }
    }
    // asked first: `Tangent` of a conic is not a circle tangent
    if let Some(r) = ConicPartDef::parse_multi(c, func, &args) {
        return Some(wrap(r, Def::ConicPart));
    }
    if let Some(r) = LineDef::parse_multi(c, func, &args) {
        return Some(wrap(r, Def::Lines));
    }
    if let Some(r) = ConicDef::parse_multi(c, func, &args) {
        return Some(wrap(r, Def::Conic));
    }
    if let Some(r) = MeasureDef::parse_multi(c, func, &args) {
        return Some(wrap(r, Def::Measure));
    }
    if let Some(r) = PointDef::parse_multi(c, func, &args) {
        return Some(wrap(r, Def::Point));
    }
    if let Some(r) = TransformDef::parse_multi(c, func, &args) {
        return Some(wrap(r, Def::Transform));
    }
    None
}

/// Adds the outputs of one command `name = Command(...)`. The labels follow
/// what GeoGebra's `LabelManager.setLabels` and `Construction.getIndexLabel`
/// give at creation. The kernel's rule:
///
/// - two or more outputs defined at creation: every defined output is
///   created and gets the next free indexed label (`t_{1}`, `t_{2}`, ...);
///   `Vertex` of a hyperbola labels its two defined vertices only;
/// - exactly one defined: only that output is created, using an indexed
///   label for algorithms that assign slots through an output handler;
/// - none defined: only the first output is created, using the same rule.
/// - curve and line: create every indexed parameter slot, including those
///   whose parameters lie outside the curve.
///
/// The workbench can record additional curve or polygonal boundary outputs
/// after a move reveals more crossings.
/// Returns the id of the first object created.
pub(crate) fn add_outputs(c: &mut Construction, name: &str, defs: Vec<Def>) -> Result<Id, String> {
    if c.id(name).is_some() {
        return Err(format!("object `{name}` already exists"));
    }
    if let [first @ Def::AngleObject(AngleDef {
        source: AngleSource::Fixed {
            from, point: false, ..
        },
        ..
    }), second] = defs[..]
    {
        let point = rotated_point_label(c, from, name);
        // The rotated point is made even when the angle is undefined.
        probe(c, &first)?;
        probe(c, &second)?;
        let at = c.checkpoint();
        let result = c
            .add(name, first)
            .and_then(|id| c.add_output(&point, second, id).map(|_| id));
        if result.is_err() {
            c.discard_since(at);
        }
        return result.map_err(|e| e.to_string());
    }
    let mut defined = Vec::with_capacity(defs.len());
    for def in &defs {
        defined.push(probe(c, def)?);
    }
    if let [first @ Def::ConicPart(ConicPartDef::Directrix { .. }), second] = defs[..] {
        // `AlgoDirectrix` gives both directrices the same label: with both
        // defined the first keeps `name` and the second gets the next free
        // indexed label (observed with the harness).
        return match (defined[0], defined[1]) {
            (true, true) => {
                let id = c.add(name, first).map_err(|e| e.to_string())?;
                let prefix = name
                    .split('_')
                    .next()
                    .filter(|p| !p.is_empty())
                    .unwrap_or("a");
                let label = index_label(c, prefix);
                c.add_output(&label, second, id)
                    .map_err(|e| e.to_string())?;
                Ok(id)
            }
            (false, true) => c.add(name, second).map_err(|e| e.to_string()),
            _ => c.add(name, first).map_err(|e| e.to_string()),
        };
    }
    let count = defined.iter().filter(|&&d| d).count();
    let indexed = matches!(defs.first(), Some(Def::IntersectBoundary(d)) if d.labels_indexed())
        || matches!(
            defs.first(),
            Some(
                Def::Point(PointDef::InequalityVertex { .. })
                    | Def::Lines(LineDef::ImplicitTangent(_))
            )
        );
    let all_indexed =
        matches!(defs.first(), Some(Def::IntersectBoundary(d)) if d.labels_all_indexed(c));
    // Construction.getIndexLabel: the prefix is the name up to the first `_`.
    let prefix = match name.split('_').next() {
        Some(p) if !p.is_empty() => p,
        _ => "a",
    };
    if count < 2 && !all_indexed {
        let k = defined.iter().position(|&d| d).unwrap_or(0);
        return if indexed {
            c.add(&index_label(c, prefix), defs[k])
        } else {
            c.add(name, defs[k])
        }
        .map_err(|e| e.to_string());
    }
    let at = c.checkpoint();
    let mut first = None;
    for (def, _) in defs
        .into_iter()
        .zip(defined)
        .filter(|&(_, d)| all_indexed || d)
    {
        let label = index_label(c, prefix);
        // the indexed outputs of one intersection share the element of
        // the first (`AlgoIntersectConics` is one construction element)
        let added = match first {
            Some(owner) => c.add_output(&label, def, owner),
            None => c.add(&label, def),
        };
        match added {
            Ok(id) => {
                first.get_or_insert(id);
            }
            Err(e) => {
                c.discard_since(at);
                return Err(e.to_string());
            }
        }
    }
    first.ok_or_else(|| "the command creates no object".into())
}

/// The point made by the fixed-angle macro uses the rotation's usual
/// primed label, or a default point label for an indexed source.
fn rotated_point_label(c: &Construction, from: Id, angle: &str) -> String {
    let name = c.label(from);
    if !name.contains('_') && !name.ends_with("'''") {
        let candidate = format!("{name}'");
        if candidate != angle && c.id(&candidate).is_none() {
            return candidate;
        }
        if candidate != angle {
            return index_label(c, &candidate);
        }
    }
    for n in 0.. {
        for ch in 'A'..='Z' {
            let label = if n == 0 {
                ch.to_string()
            } else if n < 10 {
                format!("{ch}_{n}")
            } else {
                format!("{ch}_{{{n}}}")
            };
            if label != angle && c.id(&label).is_none() {
                return label;
            }
        }
    }
    unreachable!("the label sequence is infinite")
}

/// The error of a vertex command whose inequality cannot test its
/// candidates (see [`crate::expr::inequality::fails`]).
const INEQUALITY_FAILURE: &str = "an inequality border is neither polynomial nor a function";

/// `Construction.buildIndexedLabel`: the first `p_{n}` that is free, where
/// for `n < 10` the short form `p_n` must be free too.
pub(crate) fn index_label(c: &Construction, prefix: &str) -> String {
    // Indexes below the hint are known to be taken (see `label_hint`).
    let family = format!("I:{prefix}");
    let mut n = c.label_hint(&family).max(1);
    loop {
        let long = format!("{prefix}_{{{n}}}");
        if c.id(&long).is_none() && (n >= 10 || c.id(&format!("{prefix}_{n}")).is_none()) {
            c.set_label_hint(&family, n);
            return long;
        }
        n += 1;
    }
}

/// Checks a definition of a group and tells whether it is defined now.
fn probe(c: &Construction, def: &Def) -> Result<bool, String> {
    fn run<G: Group>(c: &Construction, d: &G) -> Result<bool, String> {
        d.check(c).map_err(|e| e.to_string())?;
        Ok(d.compute(c).is_some_and(|v| is_finite(&v)))
    }
    match def {
        Def::Lines(d) => run(c, d),
        Def::Conic(d) => run(c, d),
        Def::Measure(d) => run(c, d),
        Def::Point(d) => run(c, d),
        Def::AngleObject(d) => run(c, d),
        Def::Transform(d) => run(c, d),
        Def::ConicPart(d) => run(c, d),
        Def::Path(d) => run(c, d),
        Def::Arc(d) => run(c, d),
        Def::IntersectLineCircle {
            line,
            circle,
            which,
        } => {
            c.check(def).map_err(|e| e.to_string())?;
            Ok(intersect::probe_line_circle(c, *line, *circle, *which))
        }
        Def::IntersectConics(_) | Def::IntersectLineConic(_) | Def::IntersectBoundary(_) => c
            .evaluate(*def)
            .map(|v| v != Value::Undefined)
            .map_err(|e| e.to_string()),
        _ => Ok(true),
    }
}

fn is_finite(v: &Value) -> bool {
    match *v {
        Value::Point { x, y } => x.is_finite() && y.is_finite(),
        Value::Line { a, b, c } => a.is_finite() && b.is_finite() && c.is_finite(),
        Value::Segment { x1, y1, x2, y2 } => {
            x1.is_finite() && y1.is_finite() && x2.is_finite() && y2.is_finite()
        }
        Value::Circle { cx, cy, r } => cx.is_finite() && cy.is_finite() && r.is_finite(),
        Value::Number(v) => v.is_finite(),
        Value::Vector { x, y } => x.is_finite() && y.is_finite(),
        Value::Ray { x, y, a, b, c } => [x, y, a, b, c].iter().all(|v| v.is_finite()),
        Value::Bool(_) => true,
        Value::Curve { start, end } => start.is_finite() && end.is_finite(),
        Value::Function | Value::FunctionNVar | Value::Text | Value::List { .. } => true,
        Value::Polygon { area } => area.is_finite(),
        Value::PolyLine { length } => length.is_finite(),
        Value::Conic { .. } => true,
        Value::Arc { length } => length.is_finite(),
        Value::Sector { area } => area.is_finite(),
        Value::Locus { .. } | Value::PenStroke { .. } => true,
        Value::HomogeneousPoint { .. }
        | Value::NormalizedPointAtInfinity { .. }
        | Value::Button
        | Value::InputBox
        | Value::Image
        | Value::PieChart
        | Value::ImplicitCurve
        | Value::Undefined => false,
    }
}

/// GeoGebra's `AlgebraProcessor.isVectorLabel`: coordinates `(x, y)` make a
/// vector when the first UTF-16 code unit of the label is lower case by
/// `StringUtil.isLowerCase`, a point otherwise. A label that starts outside
/// the Basic Multilingual Plane starts with a surrogate, so it is a point.
pub fn is_vector_label(name: &str) -> bool {
    name.encode_utf16().next().is_some_and(is_lower_case)
}

/// `StringUtil.isLowerCase`: a letter by [`is_letter`] that Java's
/// `Character.toLowerCase(char)` leaves unchanged.
fn is_lower_case(unit: u16) -> bool {
    let Some(ch) = char::from_u32(u32::from(unit)) else {
        return false;
    };
    let mut lower = ch.to_lowercase();
    is_letter(unit) && lower.next() == Some(ch) && lower.next().is_none()
}

/// `Character.isLetter` of GeoGebra's editor (`org.geogebra.editor.share.input`),
/// a fixed list of ranges of UTF-16 code units.
fn is_letter(c: u16) -> bool {
    matches!(c,
        0x0041..=0x005a // upper case (A-Z)
        | 0x0061..=0x007a // lower case (a-z)
        | 0x00b7 // middle dot (for Catalan)
        | 0x00c0..=0x00d6 // accentuated letters
        | 0x00d8..=0x00f6
        | 0x00f8..=0x01bf
        | 0x01c4..=0x02a8
        | 0x038e..=0x03f5 // Greek
        | 0x0401..=0x0481 // Cyrillic
        | 0x0490..=0x04f9
        | 0x0531..=0x1ffc // Arabic, accentuated and others
        | 0x212f // Euler number
        | 0x3041..=0x3357 // Asian letters
        | 0x4e00..=0xd7a3
        | 0xf71d..=0xfa2d
        | 0xfb13..=0xfdfb // Armenian, Hebrew, Arabic
        | 0xfe80..=0xfefc // Arabic
        | 0xff66..=0xff9d // Katakana
        | 0xffa1..=0xffdc)
}

pub(crate) fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(ch) if ch.is_alphabetic() || ch == '_' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '\'')
}

pub(crate) fn parse_def(c: &Construction, rhs: &str) -> Result<Def, String> {
    // (x, y)
    if let Some(inner) = rhs.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() != 2 {
            return Err("a point needs two coordinates: (x, y)".into());
        }
        return Ok(Def::FreePoint {
            x: parse_number(parts[0])?,
            y: parse_number(parts[1])?,
        });
    }

    // 3.5
    if let Some(v) = crate::expr::plain_number(rhs) {
        return Ok(Def::Number(v));
    }

    // Name(arg, arg, ...)
    let open = rhs
        .find('(')
        .ok_or_else(|| format!("cannot understand `{rhs}`"))?;
    let inner = rhs[open + 1..]
        .strip_suffix(')')
        .ok_or("missing closing `)`")?;
    let func = rhs[..open].trim();
    let mut args: Vec<&str> = inner.split(',').map(str::trim).collect();
    if matches!(
        func,
        "Perpendicular" | "PerpendicularLine" | "OrthogonalLine"
    ) {
        if let [_, _, orientation] = args.as_slice() {
            let orientation = ungroup(orientation);
            if matches!(orientation, "xOyPlane" | "space") && c.id(orientation).is_none() {
                args.pop();
            }
        }
    }
    let obj = |s: &str| object(c, s);

    if let Some(def) = intersect::parse_selected(c, func, &args) {
        return def;
    }
    // Groups are asked first, so that a group can add a new form of a
    // built-in command (for example `Circle` with three points). General
    // conics come first: they claim the transformations of conics.
    if let Some(def) = GeneralConicDef::parse(c, func, &args) {
        return def.map(Def::GeneralConic);
    }
    if let Some(def) = ArcDef::parse(c, func, &args) {
        return def.map(Def::Arc);
    }
    if let Some(def) = ConicPartDef::parse(c, func, &args) {
        return def.map(Def::ConicPart);
    }
    if let Some(def) = VectorDef::parse(c, func, &args) {
        return def.map(Def::Vector);
    }
    if let Some(def) = LineDef::parse(c, func, &args) {
        return def.map(Def::Lines);
    }
    if let Some(def) = ConicDef::parse(c, func, &args) {
        return def.map(Def::Conic);
    }
    if !(func == "Angle" && args.len() == 3) {
        if let Some(def) = AngleDef::parse(c, func, &args) {
            return def.map(Def::AngleObject);
        }
    }
    if let ("Circumference", [polygon]) = (func, args.as_slice()) {
        if let Some(id) = c.id(polygon).filter(|&id| c.kind(id) == Kind::Polygon) {
            return Ok(Def::Polygon(PolygonDef::Perimeter(id)));
        }
    }
    if let Some(def) = MeasureDef::parse(c, func, &args) {
        return def.map(Def::Measure);
    }
    if let Some(def) = TransformDef::parse(c, func, &args) {
        return def.map(Def::Transform);
    }
    if let Some(def) = PredicateDef::parse(c, func, &args) {
        return def.map(Def::Predicate);
    }
    if let Some(def) = PointDef::parse(c, func, &args) {
        return def.map(Def::Point);
    }
    if let Some(def) = crate::algo::path::PathDef::parse(c, func, &args) {
        return def.map(Def::Path);
    }
    if let Some(def) = LocusDef::parse(c, func, &args) {
        return def.map(Def::Locus);
    }
    if let Some(def) = OptimizeDef::parse(c, func, &args) {
        return def.map(Def::Optimize);
    }

    match (func, args.len()) {
        ("Line", 2) => {
            let first = obj(args[0])?;
            let second = obj(args[1])?;
            // GeoGebra: `Line(P, l)` is the parallel to `l` through `P`.
            Ok(if matches!(c.kind(second), Kind::Line | Kind::Segment) {
                Def::Parallel {
                    through: first,
                    to: second,
                }
            } else {
                Def::Line(first, second)
            })
        }
        ("Segment", 2) => Ok(Def::Segment(obj(args[0])?, obj(args[1])?)),
        ("Midpoint" | "Center", 2) => Ok(Def::Midpoint(obj(args[0])?, obj(args[1])?)),
        ("Distance", 2) => Ok(Def::Distance(obj(args[0])?, obj(args[1])?)),
        ("Perpendicular" | "PerpendicularLine" | "OrthogonalLine", 2) => Ok(Def::Perpendicular {
            through: obj(args[0])?,
            to: obj(args[1])?,
        }),
        ("Parallel", 2) => Ok(Def::Parallel {
            through: obj(args[0])?,
            to: obj(args[1])?,
        }),
        ("Circle", 2) => {
            let center = obj(args[0])?;
            let second = obj(args[1])?;
            Ok(if c.kind(second).is_number() {
                Def::CircleRadius {
                    center,
                    radius: second,
                }
            } else {
                Def::CircleThrough {
                    center,
                    through: second,
                }
            })
        }
        ("Intersect", 2 | 3) => {
            let first = obj(args[0])?;
            let second = obj(args[1])?;
            if let Some(def) = intersect::parse_lines(c, first, second, args.get(2).copied()) {
                return def;
            }
            let which = match args.get(2) {
                None => 0,
                Some(&"1") => 0,
                Some(&"2") => 1,
                Some(other) => {
                    return Err(format!("intersection number must be 1 or 2, got `{other}`"))
                }
            };
            match (c.kind(first), c.kind(second)) {
                (Kind::Line, Kind::Circle) => Ok(Def::IntersectLineCircle {
                    line: first,
                    circle: second,
                    which,
                }),
                (Kind::Circle, Kind::Line) => Ok(Def::IntersectLineCircle {
                    line: second,
                    circle: first,
                    which,
                }),
                (Kind::Circle, Kind::Circle) => Ok(Def::IntersectCircles {
                    c1: first,
                    c2: second,
                    which,
                    special: None,
                }),
                (a, b) => Err(format!("cannot intersect a {a} with a {b}")),
            }
        }
        (
            "Line" | "Segment" | "Midpoint" | "Distance" | "Perpendicular" | "PerpendicularLine"
            | "OrthogonalLine" | "Parallel" | "Circle" | "Intersect",
            n,
        ) => Err(format!("`{func}` does not take {n} argument(s)")),
        _ => Err(format!("unknown command `{func}`")),
    }
}

/// Looks up an object by its label.
pub(crate) fn object(c: &Construction, label: &str) -> Result<Id, String> {
    c.id(label)
        .ok_or_else(|| format!("unknown object `{label}`"))
}

fn parse_number(s: &str) -> Result<f64, String> {
    crate::expr::plain_number(s).ok_or_else(|| format!("`{s}` is not a number"))
}

/// Names GeoGebra never gives to an object of these forms: `x = 5` and
/// `pi = 3` are equations there, and `sin = 3` is renamed `sin_{1}` (all
/// observed with the harness). Command names such as `Mod` and the letters
/// `e` and `i` are ordinary labels. GeoGebra also reads `inf = ...`,
/// `Infinity = ...`, `NaN = ...` and `Undefined = ...` as equations; here
/// they stay labels, which the polygon forms rely on (see DIFFERENCES.md),
/// while an expression still reads these words as `∞` and `?`.
fn check_label(name: &str) -> Result<(), String> {
    const RESERVED: &[&str] = &[
        "x",
        "y",
        "z",
        "pi",
        "Pi",
        "\u{3c0}",
        "\u{212f}",
        "euler_gamma",
        "deg",
        "rad",
        "true",
        "True",
        "false",
        "False",
    ];
    if RESERVED.contains(&name) || crate::expr::is_function_name(name) && !is_command_name(name) {
        return Err(format!("`{name}` is a reserved name"));
    }
    Ok(())
}

/// Commands among the built-in names (`Mod`, `Max`, ...): upper case.
fn is_command_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
}

/// `$point(x, y)` and `$vector(x, y)`: a free point or a free vector whatever
/// the label looks like. `None` for any other right-hand side.
fn parse_forced(rhs: &str) -> Option<Result<Def, String>> {
    let (func, args) = split_call(rhs)?;
    if func != "$point" && func != "$vector" {
        return None;
    }
    Some(forced_coordinates(func, &args))
}

fn forced_coordinates(func: &str, args: &[&str]) -> Result<Def, String> {
    let [x, y] = args else {
        return Err(format!("`{func}` needs two coordinates"));
    };
    let (x, y) = (parse_number(x)?, parse_number(y)?);
    Ok(if func == "$point" {
        Def::FreePoint { x, y }
    } else {
        Def::Vector(VectorDef::Free { x, y })
    })
}

/// Index of the `}` that closes the `{` at the start of `s`. Labels such as
/// `t_{1}` contain balanced braces of their own.
fn closing_brace(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, ch) in s.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Splits at the commas that are outside braces.
fn split_slots(s: &str) -> Vec<&str> {
    let mut slots = Vec::new();
    let (mut depth, mut start) = (0usize, 0);
    for (i, ch) in s.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                slots.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    slots.push(s[start..].trim());
    slots
}

/// A label for an output slot or a plain line: an identifier, or the indexed
/// form `prefix_{inner}` where `inner` is one or more letters, digits, `_` or
/// `'` (`t_{1}`, `v_{max}`).
fn is_output_label(s: &str) -> bool {
    match s.strip_suffix('}').and_then(|t| t.split_once("_{")) {
        Some((prefix, inner)) => {
            is_identifier(prefix)
                && !inner.is_empty()
                && inner
                    .chars()
                    .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '\'')
        }
        None => is_identifier(s),
    }
}

/// Whether `rhs` (`Command(args)`) is a command that creates several objects
/// and, if so, how many of them the kernel makes now (or why it cannot). `None`
/// for a command that creates one object, and for anything that is not a call.
/// A saved file uses it to choose between `name = ...` and `{a, b} = ...`.
pub fn multi_output_count(c: &Construction, rhs: &str) -> Option<Result<usize, String>> {
    if let Some(def) = conic_clip(c, rhs.trim()) {
        return Some(Ok(def.segments(c).len().max(1)));
    }
    if let Some(("Intersect", args)) = split_call(rhs.trim()) {
        if intersect::curves::CurveIntersection::seed_pair(c, &args).is_some() {
            return Some(Ok(1));
        }
    }
    parse_multi(c, rhs.trim()).map(|r| r.map(|defs| defs.len()))
}

fn conic_clip(c: &Construction, rhs: &str) -> Option<crate::algo::polygon_clip::ClipDef> {
    let (func @ ("IntersectPath" | "IntersectionPaths" | "IntersectRegion"), args) =
        split_call(rhs)?
    else {
        return None;
    };
    crate::algo::polygon_clip::ClipDef::parse(c, func, &args)
        .ok()
        .filter(|def| def.is_conic())
}

/// `{t_{1}, t_{2}} = Tangent(P, k)`: names the outputs of a command that
/// creates several objects. The slots follow GeoGebra's output order; an
/// empty slot is an output that is not created. Nothing is created when the
/// line fails. Returns the id of the first object created.
fn exec_labelled(c: &mut Construction, line: &str) -> Result<Option<Id>, String> {
    let close = closing_brace(line).ok_or("missing closing `}` after the output labels")?;
    let slots = split_slots(&line[1..close]);
    let rhs = line[close + 1..]
        .trim_start()
        .strip_prefix('=')
        .ok_or("expected `=` after the output labels")?
        .trim();
    let inline = inline_arguments(c, rhs);
    let rhs = inline.as_deref().unwrap_or(rhs);
    if let Some(result) = with_saved_arguments(c, rhs, |c, call| exec_slots(c, &slots, call)) {
        return result;
    }
    exec_slots(c, &slots, rhs)
}

fn exec_slots(c: &mut Construction, slots: &[&str], rhs: &str) -> Result<Option<Id>, String> {
    if let Some(result) = analysis::exec_slots(c, slots, rhs) {
        return result;
    }
    if let Some(clip) = conic_clip(c, rhs) {
        use crate::algo::polygon::PolygonDef;
        use crate::algo::polygon_clip::ClipOutput;
        if slots.len() > clip.output_capacity(c) {
            return Err("too many output labels for the clipped path".into());
        }
        let defs = (0..slots.len())
            .map(|i| Def::Polygon(PolygonDef::Clip(clip.slot(ClipOutput::Segment, i))))
            .collect();
        return add_named_outputs(c, slots, defs);
    }
    if let Some((func, args)) = split_call(rhs) {
        if let Some(source) = PointDef::inequality_source(c, func, &args) {
            if slots.len() > crate::expr::inequality::MAX_VERTICES {
                return Err("too many output labels for the inequality".into());
            }
            let defs = PointDef::inequality_slots(c, source, slots.len());
            return add_named_outputs(c, slots, defs);
        }
    }
    if let Some(("Point", args)) = split_call(rhs) {
        if let [arg] = args.as_slice() {
            if let Some(list) = c.id(arg).filter(|&id| c.kind(id) == Kind::List) {
                if slots.len() > crate::expr::MAX_ELEMENTS as usize {
                    return Err("too many output labels for the coordinate list".into());
                }
                let defs = (0..slots.len() as u32)
                    .map(|slot| Def::Point(PointDef::Coordinates { list, slot }))
                    .collect();
                return add_named_outputs(c, slots, defs);
            }
        }
    }
    if let Some((name @ "Intersect", args)) = split_call(rhs) {
        use intersect::curves::CurveIntersection;
        if let Some(curve) = CurveIntersection::unseeded_pair(c, name, &args) {
            if slots.len() > CurveIntersection::MAX_SAVED_SLOTS {
                return Err("too many output labels for the curve intersection".into());
            }
            return add_named_outputs(c, slots, curve.saved_slots(slots.len()));
        }
        let checkpoint = c.checkpoint();
        if let Some(def) = CurveIntersection::selected(c, &args) {
            let result = def.and_then(|def| add_named_outputs(c, slots, vec![def]));
            if result.is_err() {
                c.discard_since(checkpoint);
            }
            return result;
        }
    }
    // The command decides how many outputs it has; a longer list of labels is
    // refused before any label is looked at, so the work is bounded by the
    // command, not by the length of the line.
    let Some(defs) = parse_multi(c, rhs) else {
        // An unknown command or a bad argument is the better message.
        return Err(match parse_def(c, rhs) {
            Err(e) => e,
            Ok(_) => "the command does not create several objects, write `name = ...`".into(),
        });
    };
    add_named_outputs(c, slots, defs?)
}

pub(crate) fn add_named_outputs(
    c: &mut Construction,
    slots: &[&str],
    defs: Vec<Def>,
) -> Result<Option<Id>, String> {
    // A command can make fewer outputs than GeoGebra's algorithm has (the
    // second tangent from a point on a circle is undefined at creation and
    // never created); slots for outputs that are not created may stay empty.
    if defs.len() > slots.len() || slots[defs.len()..].iter().any(|s| !s.is_empty()) {
        return Err(format!(
            "the command creates {} outputs, but {} labels are given",
            defs.len(),
            slots.len()
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for slot in slots {
        if slot.is_empty() {
            continue;
        }
        if !is_output_label(slot) {
            return Err(format!("`{slot}` is not a valid object name"));
        }
        check_label(slot)?;
        if !seen.insert(*slot) {
            return Err(format!("the label `{slot}` is used twice"));
        }
        if c.id(slot).is_some() {
            return Err(format!("object `{slot}` already exists"));
        }
    }
    if seen.is_empty() {
        return Err("no output is given a label".into());
    }
    for (slot, def) in slots.iter().zip(&defs) {
        if !slot.is_empty() {
            probe(c, def)?;
        }
    }
    // All or nothing: a failure after the first object removes it again.
    let at = c.checkpoint();
    let mut first = None;
    for (slot, def) in slots.iter().zip(defs) {
        if !slot.is_empty() {
            let added = match first {
                Some(owner) => c.add_output(slot, def, owner),
                None => c.add(slot, def),
            };
            match added {
                Ok(id) => {
                    first.get_or_insert(id);
                }
                Err(e) => {
                    c.discard_since(at);
                    return Err(e.to_string());
                }
            }
        }
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_empty_implicit_tangent_discards_inline_arguments() {
        let mut c = load("f(x,y) = x^2 + y^2 - 1\nc = ImplicitCurve(f)").unwrap();
        let before = c.len();
        for line in ["t = Tangent((0, 0), c)", "t = Tangent($arg((0, 0)), c)"] {
            assert_eq!(exec(&mut c, line), Ok(None));
            assert_eq!(c.len(), before);
        }
    }

    #[test]
    fn saved_curve_intersections_reject_overlong_output_lists() {
        let mut c = super::load("q = Curve(t, t^2, t, -2, 2)\ng: y = 100").unwrap();
        let labels = (1..=513).map(|i| format!("P_{{{i}}}")).collect::<Vec<_>>();
        let line = format!("{{{}}} = Intersect(g, q)", labels.join(","));
        let before = c.len();
        let error = super::exec(&mut c, &line).unwrap_err();
        assert_eq!(error, "too many output labels for the curve intersection");
        assert_eq!(c.len(), before);
    }

    use super::*;
    use crate::kernel::Value;

    #[test]
    fn comments_after_an_unmatched_quote_end_at_the_first_hash() {
        let mut c = Construction::new();
        let without_comment = exec(&mut c, "a\"").unwrap_err();
        let with_comment = exec(&mut c, "a\" # this is not a definition = 2").unwrap_err();
        assert_eq!(without_comment, with_comment);
        let id = exec(&mut c, r#"n = Length("a#b") # unmatched " in the comment"#)
            .unwrap()
            .unwrap();
        assert_eq!(c.number(id), Some(3.0));
    }

    #[test]
    fn inline_conic_coefficients_stay_within_the_evaluation_budget() {
        let mut c = Construction::new();
        let before = crate::expr::work_done();
        let id = exec(&mut c, "k = Conic({1, 0.5, -1, 0, 0, 0})")
            .unwrap()
            .unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c.kind(id), Kind::Conic);
        assert!(c.conic(id).is_some());
        assert!(crate::expr::work_done() - before < crate::expr::MAX_EVALUATION_WORK);
    }

    #[test]
    fn a_point_on_an_arc_axis_uses_the_carrier_midpoint() {
        let c = load(
            "O = (1, 2)\nA = (2, 2)\nB = (1, 3)\nr = CircleArc(O, A, B)\n\
             g = FirstAxis(r)\nP = Point(g, 0.5)",
        )
        .unwrap();
        assert_eq!(c.point(c.id("P").unwrap()), Some((1.0, 2.0)));
    }

    #[test]
    fn defined_line_checks_do_not_walk_conditional_ancestors() {
        let mut c = Construction::new();
        exec(&mut c, "A = (0, 0)").unwrap();
        exec(&mut c, "B = (1, 0)").unwrap();
        exec(&mut c, "g0 = Line(A, B)").unwrap();
        for i in 1..=64 {
            let prior = i - 1;
            exec(&mut c, &format!("g{i} = If(true, g{prior}, g{prior})")).unwrap();
        }
        let before = crate::expr::work_done();
        let result = exec(&mut c, "b = Defined(g64)").unwrap().unwrap();
        assert_eq!(c.value(result), Value::Bool(true));
        assert!(crate::expr::work_done() - before < 8);
    }

    #[test]
    fn vector_labels_follow_string_util_is_lower_case() {
        assert!(is_vector_label("a_1"));
        assert!(is_vector_label("\u{3b1}")); // Greek small alpha
        assert!(is_vector_label("\u{142}k")); // Latin small l with stroke
        assert!(is_vector_label("\u{b7}")); // a letter for GeoGebra, unchanged by lower case
        assert!(!is_vector_label("A_1"));
        assert!(!is_vector_label("_p"));
        assert!(!is_vector_label("\u{141}")); // Latin capital L with stroke
        assert!(!is_vector_label("\u{1d44e}")); // mathematical italic small a, outside the BMP
        assert!(!is_vector_label("\u{2170}")); // small roman numeral one, a letter number
        assert!(!is_vector_label(""));
    }

    #[test]
    fn a_colon_after_the_equals_sign_is_part_of_the_definition() {
        let mut c = Construction::new();
        exec(&mut c, "O = (0, 0)").unwrap();
        exec(&mut c, "A = (3, 4)").unwrap();
        c.add(
            "\0radius:k",
            Def::Distance(c.id("O").unwrap(), c.id("A").unwrap()),
        )
        .unwrap();
        let k = exec(&mut c, "k = Circle(O, \0radius:k)").unwrap().unwrap();
        assert_eq!(c.label(k), "k");
        assert_eq!(c.kind(k), crate::Kind::Circle);
        // the label form still reads a colon before the equation
        let a = exec(&mut c, "a: 2x + 3y = 6").unwrap().unwrap();
        assert_eq!(c.label(a), "a");
    }

    #[test]
    fn builds_a_construction_from_text() {
        let c = load(
            "# two circles\n\
             A = (0, 0)\n\
             B = (8, 0)\n\
             r = 5\n\
             k1 = Circle(A, r)\n\
             k2 = Circle(B, r)\n\
             P = Intersect(k1, k2, 1)   # first point\n\
             Q = Intersect(k1, k2, 2)\n\
             d = Distance(P, Q)\n",
        )
        .unwrap();

        assert_eq!(c.len(), 8);
        assert_eq!(c.value(c.id("P").unwrap()), Value::Point { x: 4.0, y: 3.0 });
        assert_eq!(c.value(c.id("d").unwrap()), Value::Number(6.0));
    }

    #[test]
    fn geogebra_parallel_and_perpendicular_forms() {
        let c = load(
            "A = (0, 0)\nB = (4, 0)\nP = (1, 3)\n\
             l = Line(A, B)\ns = Segment(A, B)\n\
             p = Line(P, l)\nq = Line(P, s)\n\
             n = PerpendicularLine(P, l)\nm = Line(A, P)\n",
        )
        .unwrap();
        let v = |n: &str| c.value(c.id(n).unwrap());
        // GeoGebra's raw coefficients: those of l, (0, 4, 0), moved through P.
        assert_eq!(
            v("p"),
            Value::Line {
                a: 0.0,
                b: 4.0,
                c: -12.0
            }
        );
        assert_eq!(v("q"), v("p"));
        assert_eq!(c.kind(c.id("n").unwrap()), Kind::Line);
        let old =
            load("A = (0, 0)\nB = (4, 0)\nP = (1, 3)\nl = Line(A, B)\nn = Perpendicular(P, l)\n")
                .unwrap();
        assert_eq!(old.value(old.id("n").unwrap()), v("n"));
        // Two points still make a plain line.
        assert!(matches!(c.def(c.id("m").unwrap()), Def::Line(..)));
        assert!(matches!(c.def(c.id("p").unwrap()), Def::Parallel { .. }));
    }

    #[test]
    fn a_colon_assignment_updates_an_existing_slider() {
        let mut c = load("a = Slider(0, 4)\nb = 2 a").unwrap();
        let slider = c.id("a").unwrap();
        assert!(is_slider_assignment(&c, "a: 3"));
        assert_eq!(slider_assignment_target(&c, "a: 3"), Some(slider));
        assert_eq!(slider_assignment_target(&c, "a := 3"), Some(slider));
        assert_eq!(exec(&mut c, "a: 3"), Ok(None));
        assert_eq!(c.id("a"), Some(slider));
        assert_eq!(c.value(c.id("b").unwrap()), Value::Number(6.0));
    }

    #[test]
    fn set_construction_step_script_and_labelled_forms() {
        // One object exists, so every set is clamped to step 0; the labelled
        // form also creates no object, exactly like GeoGebra's processor.
        let mut c = load("cs = ConstructionStep()\n").unwrap();
        assert_eq!(c.number(c.id("cs").unwrap()), Some(1.0));
        assert!(exec(&mut c, "SetConstructionStep(2)").unwrap().is_none());
        assert_eq!(c.number(c.id("cs").unwrap()), Some(1.0));
        assert!(exec(&mut c, "a = SetConstructionStep(9)")
            .unwrap()
            .is_none());
        assert!(c.id("a").is_none());
        assert_eq!(c.number(c.id("cs").unwrap()), Some(1.0));

        let err = load("SetConstructionStep(x)\n").err().unwrap();
        assert!(
            err.message.contains("`x` is not a number"),
            "{}",
            err.message
        );
    }

    #[test]
    fn reports_the_failing_line() {
        let err = load("A = (0, 0)\nl = Line(A, Z)\n").err().unwrap();
        assert_eq!(err.line, 2);
        assert!(err.message.contains("unknown object `Z`"));

        let err = load("A = (0, 0)\nB = (1, 1)\nx = Intersect(A, B)\n")
            .err()
            .unwrap();
        assert_eq!(err.line, 3);

        let err = load("A = Foo(1)\n").err().unwrap();
        assert!(err.message.contains("unknown command"));

        let err = load("A = (0, 0)\nB = (1, 1)\nl = Line(A)\n").err().unwrap();
        assert!(err.message.contains("does not take 1"));
    }

    #[test]
    fn labels_several_outputs_like_geogebra() {
        let mut c = load(
            "A = (0, 0)\nr = 2\nk = Circle(A, r)\nP = (5, 1)\nB = (4, 0)\nC = (0, 1)\n\
             l = Line(A, B)\nm = Line(A, C)\n",
        )
        .unwrap();
        // two defined outputs: indexed labels, the plain name is not used
        let first = exec(&mut c, "t = Tangent(P, k)").unwrap().unwrap();
        assert_eq!(c.label(first), "t_{1}");
        assert!(c.id("t_{2}").is_some() && c.id("t").is_none());
        // the prefix is the name up to the first `_`
        exec(&mut c, "w_7 = AngularBisector(l, m)").unwrap();
        assert!(c.id("w_{1}").is_some() && c.id("w_{2}").is_some());
        // a short label `b_1` blocks the index 1
        exec(&mut c, "b_1 = (1, 1)").unwrap();
        exec(&mut c, "b = AngularBisector(l, m)").unwrap();
        assert!(c.id("b_{1}").is_none());
        assert!(c.id("b_{2}").is_some() && c.id("b_{3}").is_some());
        // an existing name is an error
        let e = exec(&mut c, "k = Tangent(P, k)").unwrap_err();
        assert!(e.contains("already exists"), "{e}");
    }

    #[test]
    fn a_single_defined_output_gets_the_plain_label() {
        let c = load(
            "A = (0, 0)\nr = 5\nk = Circle(A, r)\nP = (3, 4)\nt = Tangent(P, k)\n\
             B = (4, 0)\nC = (0, 2)\nD = (-4, 2)\nl = Line(A, B)\nm = Line(C, D)\n\
             b = AngularBisector(l, m)\n",
        )
        .unwrap();
        // P on the circle: GeoGebra labels only the first tangent
        assert!(c.id("t").is_some() && c.id("t_{1}").is_none() && c.id("t_{2}").is_none());
        // parallel lines with opposite directions: only the second bisector
        // is defined, and it gets the plain label
        let b = c.id("b").unwrap();
        assert!(matches!(
            c.def(b),
            Def::Lines(crate::algo::lines::LineDef::AngleBisectorLines { which: 1, .. })
        ));
        assert_eq!(
            c.value(b),
            Value::Line {
                a: 0.0,
                b: 1.0,
                c: -1.0
            }
        );
    }

    #[test]
    fn explicit_output_labels() {
        let mut c = load(
            "A = (0, 0)\nr = 2\nk = Circle(A, r)\nP = (5, 1)\nB = (4, 0)\nC = (0, 1)\n\
             l = Line(A, B)\nm = Line(A, C)\n",
        )
        .unwrap();
        let first = exec(&mut c, "{t_{1}, t_{2}} = Tangent(P, k)")
            .unwrap()
            .unwrap();
        assert_eq!(c.label(first), "t_{1}");
        assert!(c.id("t_{2}").is_some() && c.id("t").is_none());
        // any labels, an empty slot creates nothing
        exec(&mut c, "{u, } = Tangent(P, k)").unwrap();
        assert!(c.id("u").is_some() && c.len() == 11);
        exec(&mut c, "{, w} = Tangent(P, k)").unwrap();
        assert!(matches!(
            c.def(c.id("w").unwrap()),
            Def::Lines(crate::algo::lines::LineDef::TangentPoint { which: 1, .. })
        ));
        // the labels do not depend on how many outputs are defined
        exec(&mut c, "{b1, b2} = AngularBisector(l, m)").unwrap();
        assert!(c.id("b1").is_some() && c.id("b2").is_some());
        // the second tangent from a point on the circle is never created: its slot stays empty;
        // a label for it is an error (the limit stated at the top of this file)
        exec(&mut c, "T = (2, 0)").unwrap();
        exec(&mut c, "{o, } = Tangent(T, k)").unwrap();
        exec(&mut c, "T2 = (0, 2)").unwrap();
        exec(&mut c, "{o2, , } = Tangent(T2, k)").unwrap();
        let n = c.len();
        let e = exec(&mut c, "{o3, o4} = Tangent(T, k)").unwrap_err();
        assert!(e.contains("creates 1 outputs, but 2 labels"), "{e}");
        assert_eq!(c.len(), n);
        assert!(c.id("o").is_some() && c.id("o2").is_some());
        // an output that is undefined at creation is still created when it has a label
        exec(&mut c, "Q = (4, 1)").unwrap();
        exec(&mut c, "{s1, s2} = Tangent(A, k)").unwrap();
        assert!(c.id("s1").is_some() && c.id("s2").is_some());
    }

    #[test]
    fn explicit_output_labels_fail_cleanly() {
        let mut c = load("A = (0, 0)\nr = 2\nk = Circle(A, r)\nP = (5, 1)\n").unwrap();
        let n = c.len();
        for line in [
            "{t_{1}, t_{2} = Tangent(P, k)",
            "{t_{1}, t_{2}} Tangent(P, k)",
            "{t_{1}} = Tangent(P, k)",
            "{t_{1}, t_{2}, t_{3}} = Tangent(P, k)",
            "{, , t} = Tangent(P, k)",
            "{a, a} = Tangent(P, k)",
            "{A, b} = Tangent(P, k)",
            "{, } = Tangent(P, k)",
            "{a, 1b} = Tangent(P, k)",
            "{a, b} = Circle(A, r)",
            "{a, b} = (1, 2)",
            "{a, b} = Tangent(P, Z)",
            "{a, b} = Foo(P)",
            "{a, b} = Polygon(A, A, A)",
            "{}",
            "{",
            "}{",
        ] {
            assert!(exec(&mut c, line).is_err(), "{line}");
            assert_eq!(c.len(), n, "{line}");
        }
        let e = exec(&mut c, "{a, b} = Foo(P)").unwrap_err();
        assert!(e.contains("unknown command `Foo`"), "{e}");
        let e = exec(&mut c, "{a, b} = Circle(A, r)").unwrap_err();
        assert!(e.contains("does not create several"), "{e}");
    }

    #[test]
    fn forced_point_and_vector_forms() {
        let mut c = Construction::new();
        exec(&mut c, "a = $point(1, 2)").unwrap();
        exec(&mut c, "B = $vector(3, 4)").unwrap();
        exec(&mut c, "c = (5, 6)").unwrap();
        assert_eq!(c.value(c.id("a").unwrap()), Value::Point { x: 1.0, y: 2.0 });
        assert_eq!(
            c.value(c.id("B").unwrap()),
            Value::Vector { x: 3.0, y: 4.0 }
        );
        assert_eq!(c.kind(c.id("c").unwrap()), Kind::Vector);
        // a lower-case point can be moved like any free point
        c.move_point(c.id("a").unwrap(), 7.0, 8.0).unwrap();
        assert_eq!(c.value(c.id("a").unwrap()), Value::Point { x: 7.0, y: 8.0 });
        assert!(exec(&mut c, "d = $point(1)").is_err());
        assert!(exec(&mut c, "d = $point(1, x)").is_err());
        let a = c.id("a").unwrap();
        assert_eq!(exec(&mut c, "a = $vector(1, 2)").unwrap(), Some(a));
        assert_eq!(c.value(a), Value::Vector { x: 1.0, y: 2.0 });
    }

    #[test]
    fn labelled_common_tangents_of_coincident_conics_are_undefined() {
        let c = load(
            "k = Conic({1, 1, -1, 0, 0, 0})\nh = Conic({1, 1, -1, 0, 0, 0})\n\
             {t_{1}, t_{2}, t_{3}, t_{4}} = Tangent(k, h)",
        )
        .unwrap();
        for label in ["t_{1}", "t_{2}", "t_{3}", "t_{4}"] {
            let id = c.id(label).unwrap();
            assert_eq!(c.kind(id), Kind::Line);
            assert_eq!(c.value(id), Value::Undefined);
        }
    }
}
