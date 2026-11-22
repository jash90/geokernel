// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/CmdCASCommand1Arg.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoCasBaseSingleArgument.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCasBase.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/CmdSimplify.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoConic.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! `Expand`, `Factor`, `IFactor` and `Simplify` of an object that is no
//! function. A curve is rewritten coordinate by coordinate into a curve
//! over the same interval (`AlgoCasBase` copies the input and applies the
//! command to it). `Simplify` also reads a line or a conic as the function
//! of `x` it is the graph of.

use super::functional::{transform, FunctionResult};
use super::{Op, Poly, Quotient, Rat, Work};
use crate::algo::measure::MeasureDef;
use crate::expr::{add, add_curve_coordinate, reparse, Lit, Node, NONE};
use crate::tol::STANDARD_PRECISION;
use crate::{Construction, Def, Id, Kind, Value};

/// The call `Command(object)` of one of the four commands when the object
/// is a curve; `None`, with nothing added, for every other input.
pub(crate) fn define_object(
    c: &mut Construction,
    label: &str,
    input: &str,
) -> Option<Result<Id, String>> {
    if !["Expand", "Factor", "IFactor", "Simplify"]
        .iter()
        .any(|name| input.starts_with(name))
    {
        return None;
    }
    let (name, args) = crate::script::split_call(input)?;
    let op = match name {
        "Expand" => Op::Expand,
        "Factor" => Op::Factor,
        "IFactor" => Op::IFactor,
        "Simplify" => Op::Simplify,
        _ => return None,
    };
    let &[argument] = args.as_slice() else {
        return None;
    };
    let argument = argument.trim();
    if let Some(source) = c.id(argument) {
        return match c.kind(source) {
            Kind::Curve => Some(curve(c, label, op, source)),
            // `AlgoSimplifyText.compute` reads the first character when it
            // is created, which fails for an empty or undefined text.
            Kind::Text if op == Op::Simplify && c.text(source).is_none_or(str::is_empty) => {
                Some(Err("`Simplify` needs a text that is not empty".into()))
            }
            _ => None,
        };
    }
    if op == Op::Simplify && argument == "\"\"" {
        return Some(Err("`Simplify` needs a text that is not empty".into()));
    }
    if op == Op::Simplify && crate::expr::equation::split_equation(argument).is_ok() {
        return Some(equation(c, label, argument));
    }
    None
}

/// `Simplify(equation)`: the line or conic of an inline equation, read as
/// a function.
fn equation(c: &mut Construction, label: &str, source: &str) -> Result<Id, String> {
    let start = c.checkpoint();
    let result = (|| {
        let hidden = format!("\0arg{}", c.len());
        let object = crate::expr::equation::define(c, &hidden, source)?;
        if !matches!(c.kind(object), Kind::Line | Kind::Conic | Kind::Circle) {
            // An equation of higher degree is an implicit curve, which the
            // reference rewrites into an implicit curve.
            return Err("`Simplify` needs a line or a conic".into());
        }
        let nodes = [Node::Obj(object), Node::Cas(Op::Simplify, 0, NONE)];
        let literals = vec![Lit::default(); nodes.len()];
        let parsed = reparse(c, Kind::Function, &nodes, &literals, 1)
            .ok_or("`Simplify` needs a line or a conic")?;
        add(c, label, parsed, Kind::Function)
    })();
    if result.is_err() {
        c.discard_since(start);
    }
    result
}

/// The function of `x` whose graph the object is, simplified: a line that
/// is not vertical, or a conic `y = a x^2 + b x + c`
/// (`GeoConic.isRealValuedFunction`). A vertical or undefined line is the
/// function that is nowhere defined; any other conic is undefined.
pub(super) fn as_function(c: &Construction, id: Id, work: &mut Work<'_>) -> Option<FunctionResult> {
    let nowhere = Some(FunctionResult::Number(f64::NAN));
    if !c.value(id).is_defined() && matches!(c.kind(id), Kind::Line) {
        return nowhere;
    }
    let side = |left| crate::expr::equation::side_entry(c, id, left);
    if let (Some(left), Some(right)) = (side(true), side(false)) {
        // An equation as typed is read as typed (`Equation.asFunction`),
        // so `y = x / 3` keeps its third.
        let vars = &super::input::variables();
        let lhs = super::input::read(c, left, left.root, vars, work)?;
        let rhs = super::input::read(c, right, right.root, vars, work)?;
        let equation = lhs.sub(rhs, work)?;
        let divisor = equation.denominator.scalar()?;
        let mut slope = None;
        let mut rest = Poly(Default::default());
        for (&power, &coefficient) in &equation.numerator.0 {
            work.spend(1)?;
            match power {
                [_, 0, 0] => {
                    rest.0.insert(power, coefficient.div(divisor)?);
                }
                [0, 1, 0] => slope = Some(coefficient.div(divisor)?),
                // `x y`, `y^2` or a third variable: no function of `x`.
                _ => return None,
            }
        }
        let Some(slope) = slope else {
            // A vertical line is the function that is nowhere defined; a
            // conic without a `y` term is no function.
            return if matches!(c.kind(id), Kind::Line) {
                nowhere
            } else {
                None
            };
        };
        let factor = Rat::ONE.neg()?.div(slope)?;
        for coefficient in rest.0.values_mut() {
            *coefficient = coefficient.mul(factor)?;
        }
        let function = Quotient::polynomial(rest);
        return transform(Op::Simplify, function, None, work).map(FunctionResult::Exact);
    }
    // `y = (c2 x^2 + c1 x + c0) / divisor`
    let (coefficients, divisor) = match c.kind(id) {
        Kind::Line | Kind::Segment | Kind::Ray => {
            let (a, b, constant) = match c.value(id) {
                Value::Line { a, b, c } | Value::Ray { a, b, c, .. } => (a, b, c),
                // The line through the end points (`GeoVec3D.lineThroughPoints`).
                Value::Segment { x1, y1, x2, y2 } => (y1 - y2, x2 - x1, x1 * y2 - x2 * y1),
                _ => return nowhere,
            };
            if b == 0.0 {
                return nowhere;
            }
            ([-constant, -a, 0.0], b)
        }
        Kind::Conic | Kind::Circle => {
            let [xx, yy, constant, xy, x, y] = c.conic(id)?.matrix;
            let zero = |v: f64| v.abs() < STANDARD_PRECISION;
            if !zero(xy) || !zero(yy) || zero(y) {
                return None;
            }
            ([constant, 2.0 * x, xx], -2.0 * y)
        }
        _ => return None,
    };
    // The reference prints the function with 15 significant digits, so
    // each ratio is taken in doubles: the exact quotient of two doubles can
    // leave the range of the exact arithmetic where the reference is
    // defined.
    let coefficients = coefficients
        .iter()
        .map(|&v| significant(v / divisor))
        .collect::<Option<Vec<_>>>()?;
    let function = Quotient::polynomial(Poly::dense(&coefficients));
    transform(Op::Simplify, function, None, work).map(FunctionResult::Exact)
}

/// A double as the decimal of its 15 significant digits.
fn significant(value: f64) -> Option<Rat> {
    if value == 0.0 {
        return Some(Rat::ZERO);
    }
    let text = format!("{value:.14e}");
    let (mantissa, exponent) = text.split_once('e')?;
    let digits = mantissa.replace('.', "").parse().ok()?;
    Rat::decimal(digits, exponent.parse::<i64>().ok()? - 14)
}

/// The curve whose coordinates are the command applied to the coordinates
/// of `source`. It shares the bounds, so it is undefined with its source.
fn curve(c: &mut Construction, label: &str, op: Op, source: Id) -> Result<Id, String> {
    let Def::Measure(MeasureDef::CartesianCurve {
        coordinates,
        bounds,
    }) = c.def(source)
    else {
        return Err(format!(
            "`{}` needs a curve given by two coordinate functions",
            op.name()
        ));
    };
    let start = c.checkpoint();
    let result = (|| {
        let mut rewritten = [0; 2];
        for (slot, coordinate) in rewritten.iter_mut().zip(coordinates) {
            let coordinate = Id(coordinate as usize);
            let nodes = [Node::FunctionObject(coordinate), Node::Cas(op, 0, NONE)];
            let literals = vec![Lit::default(); nodes.len()];
            let parsed = reparse(c, Kind::Function, &nodes, &literals, 1)
                .ok_or("cannot rewrite the coordinates of the curve")?;
            let hidden = format!("\0arg{}", c.len());
            let id = add_curve_coordinate(c, &hidden, parsed, coordinate)?;
            *slot = u32::try_from(id.0).map_err(|_| "too many objects".to_owned())?;
        }
        c.add(
            label,
            Def::Measure(MeasureDef::CartesianCurve {
                coordinates: rewritten,
                bounds,
            }),
        )
        .map_err(|e| e.to_string())
    })();
    if result.is_err() {
        c.discard_since(start);
    }
    result
}
