//! Single-argument rewrites from GeoGebra's EUPL-licensed
//! AlgoCasBaseSingleArgument, Ggb2giac and AlgebraicExpression implementations.

use super::*;
use crate::expr::cas::FunctionResult;
use std::sync::Arc;

// Ggb2giac, as instantiated for the first evaluation in a session.
// Giac restarts on every call here, so the temporary suffix can stay fixed.
const SIMPLIFY: &str = r#"[[[ggbsimparg01:=%0],[ggbsimpans:=0/0],[ggbsimpans:=normal(simplify(regroup(texpand(ggbsimparg01))))],[ggbsimpans2:=factor(exp2pow(lin(pow2exp(ggbsimparg01))))]],when(length(""+ggbsimpans)<length(""+ggbsimpans2)||contains(lname(ggbsimpans2),lname(0/0)[0]),ggbsimpans,ggbsimpans2)][1]"#;
const EXPAND: &str = r#"[[ggbexpandarg:=%0],when(contains(ggbexpandarg,i),normal(real(ggbexpandarg))+normal(i*im(ggbexpandarg)),when(inString(string(simplify(ggbexpandarg)), "sqrt"),normal(expand(ggbfactor(ggbexpandarg,x,0,1))),normal(lnexpand(ggbexpandarg))))][1]"#;
const FACTOR: &str = r#"[[ggbfacans:=%0],[if type(ggbfacans)==DOM_INT then ggbfacans:=unquote(ifactor(ggbfacans)); elif inString(string(simplify(ggbfacans)), "sqrt") then ggbfacans:=regroup(ggbfactor(ggbfacans,x,0,1)); else ggbfacans:=ggbfactor(lncollect(ggbfacans),x, 0, 1); fi],ggbfacans][2]"#;
const IFACTOR: &str = "[[ggbfacans:=%0],[if type(ggbfacans)==DOM_INT then ggbfacans:=ifactor(ggbfacans); else L:=lvar(cZeros(ggbfacans)); ggbfacans:=when(size(L)>0, regroup(factor(ggbfacans,L)), ggbfactor(ggbfacans,x,1,1)); fi],ggbfacans][2]";

impl Op {
    pub(crate) fn is_giac_rewrite(self) -> bool {
        matches!(
            self,
            Self::Simplify | Self::Expand | Self::Factor | Self::IFactor
        )
    }
}

fn input(op: Op, argument: &str) -> Option<String> {
    let template = match op {
        Op::Simplify => SIMPLIFY,
        Op::Expand => EXPAND,
        Op::Factor => FACTOR,
        Op::IFactor => IFACTOR,
        _ => return None,
    };
    Some(fill(template, &[argument], &[]))
}

/// CmdSimplify converts a line, segment or conic to a function. An
/// equation with y alone on the left preserves its input expression.
fn graph_argument(printer: &Printer<'_>, id: crate::Id, out: &mut String) -> Option<()> {
    if let (Some(left), Some(right)) = (
        crate::expr::equation::side_entry(printer.c, id, true),
        crate::expr::equation::side_entry(printer.c, id, false),
    ) {
        if left.nodes[left.root as usize] == Node::VarY && !right.nodes.contains(&Node::VarY) {
            return printer.expr(right, right.root, out);
        }
    }

    let c = printer.c;
    let carrier = c.raw_carrier(id);
    let coefficients = if let Some((a, b, c)) = carrier {
        [0.0, a, c, b]
    } else if let Some(conic) = c.conic(id) {
        let [xx, yy, constant, xy, x, y] = conic.matrix;
        if xy.abs() >= crate::tol::STANDARD_PRECISION || yy.abs() >= crate::tol::STANDARD_PRECISION
        {
            return None;
        }
        [xx, 2.0 * x, constant, 2.0 * y]
    } else {
        return None;
    };
    let [quadratic, linear, constant, divisor] = coefficients;
    if divisor == 0.0 || !c.value(id).is_defined() {
        return None;
    }
    let mut terms = Vec::new();
    for (coefficient, variable) in [
        (-quadratic / divisor, "(x)^(2)"),
        (-linear / divisor, "x"),
        (-constant / divisor, ""),
    ] {
        if coefficient == 0.0 {
            continue;
        }
        let sign = if terms.is_empty() || coefficient > 0.0 {
            '+'
        } else {
            '-'
        };
        let mut number = String::new();
        printer.number(
            if terms.is_empty() {
                coefficient
            } else {
                coefficient.abs()
            },
            &mut number,
        );
        let term = match variable {
            "" => number,
            _ if coefficient == 1.0 => variable.into(),
            _ if carrier.is_none() => format!("({variable})*({number})"),
            _ => format!("({number})*({variable})"),
        };
        terms.push((sign, term));
    }
    let mut terms = terms.into_iter();
    let mut result = terms
        .next()
        .map_or_else(|| "0".to_owned(), |(_, term)| term);
    for (sign, term) in terms {
        result = format!("({result}){sign}({term})");
    }
    out.push_str(&result);
    Some(())
}

fn is_graph(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Line | Kind::Segment | Kind::Ray | Kind::Conic | Kind::Circle
    )
}

pub(in crate::expr) fn function(
    c: &Construction,
    e: &Entry,
    op: Op,
    a: u32,
) -> Option<Option<FunctionResult>> {
    if !op.is_giac_rewrite() || geo_giac::backend() == geo_giac::Backend::KernelOnly {
        return None;
    }
    let node = e.nodes[a as usize];
    let source = match node {
        Node::Obj(id) | Node::FunctionObject(id) => Some(id),
        _ => None,
    };
    let parameter = source
        .and_then(|id| match c.def(id) {
            Def::Expr(d) => crate::expr::entry(c, d).curve_parameter.as_deref(),
            _ => None,
        })
        .or(e.curve_parameter.as_deref())
        .unwrap_or("x");
    let mut printer = Printer::new(c, vec!["x".into(), "y".into(), "z".into()], false);
    printer.parameter = Some(parameter);
    let mut argument = String::new();
    if let Some(id) = source.filter(|&id| op == Op::Simplify && is_graph(c.kind(id))) {
        if graph_argument(&printer, id, &mut argument).is_none() {
            return Some(None);
        }
    } else {
        printer.top(e, a, &mut argument)?;
    }
    let command = input(op, &argument)?;
    let Some(raw) = evaluate_raw(&command) else {
        return Some(None);
    };
    // AlgoCasBaseSingleArgument falls back to the input object when the
    // algebra view cannot read the returned expression.
    Some(read_function(c, &raw, parameter).or_else(|| read_function(c, &argument, parameter)))
}

fn read_function(c: &Construction, text: &str, parameter: &str) -> Option<FunctionResult> {
    let mut expression = parse(text)?;
    if parameter != "x" {
        let printed = if matches!(parameter, "y" | "z") {
            parameter.to_owned()
        } else {
            format!("ggbtmpvar{parameter}")
        };
        rename(&mut expression, &printed, "x");
    }
    if !known(&expression) {
        return None;
    }
    let mut builder = crate::expr::equation::Builder::empty();
    let root = build(&mut builder, &expression)?;
    let tree = crate::expr::function_entry(c, builder.finish(c, root).ok()?);
    Some(FunctionResult::Giac {
        tree: Arc::new(tree),
        result: Arc::new(Scalar {
            expression,
            text: text.to_owned(),
        }),
    })
}

fn rename(e: &mut Expr, from: &str, to: &str) {
    match e {
        Expr::Name(name) if name == from => *name = to.to_owned(),
        Expr::Neg(a) => rename(a, from, to),
        Expr::Bin(_, a, b) | Expr::Eq(a, b) => {
            rename(a, from, to);
            rename(b, from, to);
        }
        Expr::Call(_, args) | Expr::List(args) | Expr::Tuple(args) => {
            for a in args {
                rename(a, from, to);
            }
        }
        _ => {}
    }
}

fn build(b: &mut crate::expr::equation::Builder, expression: &Expr) -> Option<u32> {
    let node = match expression {
        Expr::Num(n) => Node::Num(*n),
        Expr::Name(name) => match name.as_str() {
            "x" => Node::Var,
            "y" => Node::VarY,
            "pi" => Node::Num(std::f64::consts::PI),
            "e" => Node::Num(std::f64::consts::E),
            _ => return None,
        },
        Expr::Neg(a) => {
            let constant = b.push(Node::MinusOne);
            Node::Bin(Bin::Mul, constant, build(b, a)?)
        }
        Expr::Bin(op, a, rhs) => Node::Bin(
            match op {
                b'+' => Bin::Add,
                b'-' => Bin::Sub,
                b'*' => Bin::Mul,
                b'/' => Bin::Div,
                b'^' => Bin::Pow,
                _ => return None,
            },
            build(b, a)?,
            build(b, rhs)?,
        ),
        Expr::Call(name, args) if args.len() == 1 => Node::Un(
            match name.as_str() {
                "sqrt" => Un::Sqrt,
                "sin" => Un::Sin,
                "cos" => Un::Cos,
                "tan" => Un::Tan,
                "exp" => Un::Exp,
                "ln" | "log" => Un::Ln,
                "abs" => Un::Abs,
                _ => return None,
            },
            build(b, &args[0])?,
        ),
        _ => return None,
    };
    Some(b.push(node))
}

#[derive(PartialEq)]
pub(in crate::expr) struct Scalar {
    expression: Expr,
    text: String,
}

impl Scalar {
    pub(in crate::expr) fn value(&self, x: f64, y: f64) -> f64 {
        value(&self.expression, x, y).unwrap_or(f64::NAN)
    }

    pub(in crate::expr) fn text(&self) -> &str {
        &self.text
    }
}

/// The construction-object forms of CmdSimplify and CmdCASCommand1Arg.
/// The curve keeps the input's bound objects and rewrites each coordinate
/// through the same function path as GeoCurveCartesianND.
pub(crate) fn geometry(
    c: &mut Construction,
    label: &str,
    text: &str,
) -> Option<Result<crate::Id, String>> {
    use crate::algo::measure::MeasureDef;
    if !crate::expr::cas::giac_admits() {
        return None;
    }
    let (name, args) = crate::script::split_call(text)?;
    let op = Op::named(name)?;
    if !op.is_giac_rewrite() {
        return None;
    }
    let [argument] = args[..] else {
        return None;
    };
    let named = c.id(argument);
    if let Some(source) = named {
        let kind = c.kind(source);
        if !(matches!(kind, Kind::Curve | Kind::ImplicitCurve)
            || op == Op::Simplify && is_graph(kind))
        {
            return None;
        }
    } else if op != Op::Simplify
        || argument.starts_with('"')
        || crate::expr::equation::split_equation(argument).is_err()
    {
        return None;
    }
    let checkpoint = c.checkpoint();
    let result = (|| {
        let (source, inline) = match named {
            Some(id) => (id, false),
            None => {
                let hidden = format!("\0arg{}", c.len());
                match crate::expr::equation::define(c, &hidden, argument) {
                    Ok(id) => (id, true),
                    Err(e) => return Some(Err(e)),
                }
            }
        };
        let result = match c.kind(source) {
            kind if op == Op::Simplify && is_graph(kind) => coordinate(c, label, source, op),
            Kind::Function if inline => coordinate(c, label, source, op),
            Kind::ImplicitCurve => {
                crate::expr::equation::implicit::add_giac_rewrite(c, label, source, op)
                    .map_err(|e| e.to_string())
            }
            Kind::Curve => {
                let Def::Measure(MeasureDef::CartesianCurve {
                    coordinates,
                    bounds,
                }) = c.def(source)
                else {
                    return None;
                };
                (|| {
                    let mut rewritten = [0; 2];
                    for (target, coordinate_id) in rewritten.iter_mut().zip(coordinates) {
                        let hidden = format!("\0arg{}", c.len());
                        let changed =
                            coordinate(c, &hidden, crate::Id(coordinate_id as usize), op)?;
                        *target = u32::try_from(changed.0).map_err(|_| "too many objects")?;
                    }
                    c.add(
                        label,
                        Def::Measure(MeasureDef::CartesianCurve {
                            coordinates: rewritten,
                            bounds,
                        }),
                    )
                    .map_err(|e| e.to_string())
                })()
            }
            _ => return None,
        };
        Some(result)
    })();
    if let Some(Ok(id)) = result {
        c.remember_construction_source(id, text);
    } else {
        c.discard_since(checkpoint);
    }
    result
}

fn coordinate(
    c: &mut Construction,
    label: &str,
    source: crate::Id,
    op: Op,
) -> Result<crate::Id, String> {
    let mut b = crate::expr::equation::Builder::empty();
    let operand = b.push(if c.kind(source) == Kind::Function {
        Node::FunctionObject(source)
    } else {
        Node::Obj(source)
    });
    let root = b.push(Node::Cas(op, operand, crate::expr::NONE));
    let mut entry = crate::expr::function_entry(c, b.finish(c, root)?);
    if let Def::Expr(d) = c.def(source) {
        entry.curve_parameter = crate::expr::entry(c, d).curve_parameter.clone();
    }
    crate::expr::add_entry(
        c,
        label,
        entry,
        Kind::Function,
        crate::expr::AngleStyle::Unbounded,
        false,
    )
    .map_err(|e| e.to_string())
}

/// GeoImplicitCurve applies the scalar command to the two-dimensional
/// polynomial expression and reads the result with its right side zero.
pub(crate) fn implicit(c: &Construction, id: crate::Id, op: Op) -> Option<(Vec<Vec<f64>>, String)> {
    let printer = Printer::new(c, vec![], false);
    let mut expression = String::new();
    if let (Some(left), Some(right)) = (
        crate::expr::equation::side_entry(c, id, true),
        crate::expr::equation::side_entry(c, id, false),
    ) {
        printer.operand(left, left.root, &mut expression)?;
        expression.push('-');
        printer.operand(right, right.root, &mut expression)?;
    } else {
        let text = c.formatted_equation(id)?;
        let (left, right) = text.split_once('=')?;
        expression.push('(');
        printer.computed_expression(left, &mut expression)?;
        expression.push_str(")-(");
        printer.computed_expression(right, &mut expression)?;
        expression.push(')');
    }
    let raw = evaluate_raw(&input(op, &expression)?)?;
    let coeffs = equation_coefficients(&parse(&raw)?, &Expr::Num(0.0))?;
    Some((coeffs, format!("{raw} = 0")))
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn simplify_uses_the_recorded_template() {
        // cas-simplify-001.
        let argument = "(((x)^(2))-(1))/((x)-(1))";
        let expected = r#"[[[ggbsimparg01:=(((x)^(2))-(1))/((x)-(1))],[ggbsimpans:=0/0],[ggbsimpans:=normal(simplify(regroup(texpand(ggbsimparg01))))],[ggbsimpans2:=factor(exp2pow(lin(pow2exp(ggbsimparg01))))]],when(length(""+ggbsimpans)<length(""+ggbsimpans2)||contains(lname(ggbsimpans2),lname(0/0)[0]),ggbsimpans,ggbsimpans2)][1]"#;
        assert_eq!(
            super::input(Op::Simplify, argument).as_deref(),
            Some(expected)
        );

        let c = Construction::new();
        let result = super::read_function(&c, "x+1", "x").unwrap();
        assert_eq!(result.value(&c, 1.0, 0.0), 2.0);
    }

    #[test]
    fn implicit_factor_uses_the_recorded_input_and_reads_the_polynomial() {
        // cas-implicitrewrite-006.
        let argument = "(((x)^(3))-(x))-(0)";
        let expected = r#"[[ggbfacans:=(((x)^(3))-(x))-(0)],[if type(ggbfacans)==DOM_INT then ggbfacans:=unquote(ifactor(ggbfacans)); elif inString(string(simplify(ggbfacans)), "sqrt") then ggbfacans:=regroup(ggbfactor(ggbfacans,x,0,1)); else ggbfacans:=ggbfactor(lncollect(ggbfacans),x, 0, 1); fi],ggbfacans][2]"#;
        assert_eq!(
            super::input(Op::Factor, argument).as_deref(),
            Some(expected)
        );
        let result = parse("x*(x+1)*(x-1)").unwrap();
        assert_eq!(
            equation_coefficients(&result, &Expr::Num(0.0)),
            Some(vec![vec![0.0], vec![-1.0], vec![0.0], vec![1.0]])
        );
    }

    #[test]
    fn curve_coordinates_use_the_recorded_expand_and_ifactor_templates() {
        // cas-curve-008, first coordinate.
        let operand = "((ggbtmpvart)^(2))-(2)";
        for (op, expected) in [
            (
                Op::Expand,
                r#"[[ggbexpandarg:=((ggbtmpvart)^(2))-(2)],when(contains(ggbexpandarg,i),normal(real(ggbexpandarg))+normal(i*im(ggbexpandarg)),when(inString(string(simplify(ggbexpandarg)), "sqrt"),normal(expand(ggbfactor(ggbexpandarg,x,0,1))),normal(lnexpand(ggbexpandarg))))][1]"#,
            ),
            (
                Op::IFactor,
                r#"[[ggbfacans:=((ggbtmpvart)^(2))-(2)],[if type(ggbfacans)==DOM_INT then ggbfacans:=ifactor(ggbfacans); else L:=lvar(cZeros(ggbfacans)); ggbfacans:=when(size(L)>0, regroup(factor(ggbfacans,L)), ggbfactor(ggbfacans,x,1,1)); fi],ggbfacans][2]"#,
            ),
        ] {
            assert_eq!(super::input(op, operand).as_deref(), Some(expected));
        }
    }

    #[test]
    fn recorded_curve_results_keep_the_printed_tree() {
        let c = Construction::new();
        // cas-curve-008, third command, first coordinate.
        let result = super::read_function(&c, "(ggbtmpvart-√2)*(ggbtmpvart+√2)", "t").unwrap();
        let expected = (2.0 - 2.0_f64.sqrt()) * (2.0 + 2.0_f64.sqrt());
        assert_eq!(result.value(&c, 2.0, 0.0), expected);
    }
}
