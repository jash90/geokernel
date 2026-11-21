// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MyList.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The geometry group of the command matrix, as far as expressions compute
//! it: `Length` of a list (of anything else it is the command of the script
//! parser).

#[path = "../../algo/conic_coefficients.rs"]
mod conic_coefficients;
#[path = "../../algo/conic_interval.rs"]
mod conic_interval;

use crate::expr::eval::{Ctx, Env, Val};
use crate::expr::parse::{list_form, Parser};
use crate::expr::{self, math, Bin, Elem, ListOp, Node, Ty, Un};
use crate::kernel::{Construction, Def, Id, Value};

/// The commands of this group (see `commands::is_command`).
pub(super) const NAMES: &[&str] = &["Length"];

/// `Length(list)`.
pub(super) fn parse(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    if text != "Length" {
        return Ok(None);
    }
    p.list_command(|_, args, lists| list_form(text, ListOp::Length, false, args, lists))
}

/// The type of a list command of this group (see `commands::list_type`).
pub(super) fn list_type(
    op: ListOp,
    _elem: Elem,
    _numbers: bool,
    _nodes: &[Node],
    _tys: &[Ty],
    _l: u32,
    _n: u32,
) -> Result<Option<Ty>, String> {
    Ok(match op {
        ListOp::Length => Some(Ty::Num(0)),
        _ => None,
    })
}

/// The value of a list command of this group (see `commands::list_value`).
pub(super) fn list_value<const M: bool>(
    ctx: &Ctx<'_, M>,
    _i: u32,
    op: ListOp,
    l: u32,
    _n: u32,
) -> Option<Val> {
    Some(match op {
        ListOp::Length => Val::N(ctx.list(l).map_or(f64::NAN, |(_, len)| f64::from(len))),
        _ => return None,
    })
}

type Polynomial = [f64; 5];

impl Construction {
    /// Coefficients in ascending degree for a polynomial function of degree
    /// at most four. Coefficients can read other construction objects.
    pub(crate) fn function_polynomial(&self, id: Id) -> Option<Polynomial> {
        let ctx = function_context(self, id, &Env::None)?;
        let p = polynomial(&ctx, ctx.e.root)?;
        p.iter().all(|x| x.is_finite()).then_some(p)
    }

    /// The derivative at a seed for Newton iteration, when the expression
    /// tree consists of operations with scalar differentiation rules.
    pub(crate) fn function_derivative(&self, id: Id, x: f64) -> Option<f64> {
        let env = Env::Value(x);
        let ctx = function_context(self, id, &env)?;
        derivative(&ctx, ctx.e.root).filter(|d| d.is_finite())
    }
}

fn function_context<'a>(
    c: &'a Construction,
    id: Id,
    env: &'a Env<'a, false>,
) -> Option<Ctx<'a, false>> {
    let (Def::Expr(def), Value::Function) = (c.def(id), c.value(id)) else {
        return None;
    };
    Some(Ctx {
        c,
        e: expr::entry(c, def),
        env,
        lists: None,
        meter: None,
        locals: None,
    })
}

fn scalar_polynomial(x: f64) -> Polynomial {
    [x, 0.0, 0.0, 0.0, 0.0]
}

fn product(a: Polynomial, b: Polynomial) -> Option<Polynomial> {
    let mut out = [0.0; 5];
    for (i, x) in a.into_iter().enumerate().filter(|&(_, x)| x != 0.0) {
        for (j, y) in b.into_iter().enumerate().filter(|&(_, y)| y != 0.0) {
            *out.get_mut(i + j)? += x * y;
        }
    }
    Some(out)
}

fn has_variable(ctx: &Ctx<'_, false>, i: u32) -> bool {
    let node = ctx.node(i);
    matches!(node, Node::Var) || expr::children(&node).any(|child| has_variable(ctx, child))
}

fn polynomial(ctx: &Ctx<'_, false>, i: u32) -> Option<Polynomial> {
    if !has_variable(ctx, i) {
        return Some(scalar_polynomial(ctx.full_number(i)));
    }
    match ctx.node(i) {
        Node::Var => Some([0.0, 1.0, 0.0, 0.0, 0.0]),
        Node::Bin(op, a, b) => {
            let a = polynomial(ctx, a)?;
            let b = polynomial(ctx, b)?;
            let b_is_scalar = b[1..].iter().all(|&x| x == 0.0);
            match op {
                Bin::Add => Some(std::array::from_fn(|i| a[i] + b[i])),
                Bin::Sub => Some(std::array::from_fn(|i| a[i] - b[i])),
                Bin::Mul => product(a, b),
                Bin::Div if b_is_scalar => Some(a.map(|x| x / b[0])),
                Bin::Pow if b_is_scalar && (0.0..=4.0).contains(&b[0]) && b[0].fract() == 0.0 => (0
                    ..b[0] as usize)
                    .try_fold(scalar_polynomial(1.0), |power, _| product(power, a)),
                _ => None,
            }
        }
        Node::Apply(f, arg) => {
            let a = polynomial(ctx, arg)?;
            let p = ctx.c.function_polynomial(f)?;
            let degree = p.iter().rposition(|&x| x != 0.0).unwrap_or(0);
            let mut out = scalar_polynomial(p[degree]);
            for &constant in p[..degree].iter().rev() {
                out = product(out, a)?;
                out[0] += constant;
            }
            Some(out)
        }
        _ => None,
    }
}

fn derivative(ctx: &Ctx<'_, false>, i: u32) -> Option<f64> {
    if !has_variable(ctx, i) {
        return Some(0.0);
    }
    Some(match ctx.node(i) {
        Node::Var => 1.0,
        Node::Apply(f, arg) => {
            derivative(ctx, arg)? * ctx.c.function_derivative(f, ctx.full_number(arg))?
        }
        Node::Bin(op, a, b) => {
            let (da, db) = (derivative(ctx, a)?, derivative(ctx, b)?);
            match op {
                Bin::Add => da + db,
                Bin::Sub => da - db,
                _ => {
                    let (a, b) = (ctx.full_number(a), ctx.full_number(b));
                    match op {
                        Bin::Mul => da * b + a * db,
                        Bin::Div => (da * b - a * db) / (b * b),
                        Bin::Pow if db == 0.0 => b * a.powf(b - 1.0) * da,
                        Bin::Pow => a.powf(b) * (db * a.ln() + b * da / a),
                        _ => return None,
                    }
                }
            }
        }
        Node::Un(op, arg) => {
            let x = ctx.full_number(arg);
            let factor = match op {
                Un::Sin => math::cos(x),
                Un::Cos => -math::sin(x),
                Un::Tan => 1.0 / math::cos(x).powi(2),
                Un::Sinh => x.cosh(),
                Un::Cosh => x.sinh(),
                Un::Tanh => 1.0 / x.cosh().powi(2),
                Un::Asin => 1.0 / (1.0 - x * x).sqrt(),
                Un::Acos => -1.0 / (1.0 - x * x).sqrt(),
                Un::Atan => 1.0 / (1.0 + x * x),
                Un::Sqrt => 0.5 / x.sqrt(),
                Un::Cbrt => 1.0 / (3.0 * x.cbrt().powi(2)),
                Un::Abs => x / x.abs(),
                Un::Exp => x.exp(),
                Un::Ln => x.recip(),
                Un::Log2 => 1.0 / (x * std::f64::consts::LN_2),
                Un::Log10 => 1.0 / (x * std::f64::consts::LN_10),
                _ => return None,
            };
            factor * derivative(ctx, arg)?
        }
        _ => return None,
    })
}
