// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/Fractions.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNode.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! `Fractions.getFraction` in the mode used by `AlgoNumeratorDenominatorFun`.
//! Split a function's products, quotients and powers once, then evaluate
//! the numerator and denominator separately at each sample.

use super::eval::{Ctx, Env, Lists};
use super::{entry, types, Bin, Entry, Lit, Node, Ty};
use crate::kernel::{Construction, Def, Id, Value};
use std::borrow::Cow;

pub(crate) struct FunctionFraction<'a> {
    construction: &'a Construction,
    tree: Cow<'a, Entry>,
    numerator: u32,
    denominator: Option<u32>,
    budget: Lists,
}

impl<'a> FunctionFraction<'a> {
    pub(crate) fn new(construction: &'a Construction, id: Id) -> Option<Self> {
        let (Def::Expr(def), Value::Function) = (construction.def(id), construction.value(id))
        else {
            return None;
        };
        let source = entry(construction, def);
        let mut tree: Cow<'_, Entry> = Cow::Borrowed(source);
        let (numerator, denominator) = match source.nodes[source.root as usize] {
            Node::Bin(Bin::Mul | Bin::Div | Bin::Pow, ..) => {
                split(construction, tree.to_mut(), source.root)
            }
            _ => (source.root, None),
        };
        Some(Self {
            construction,
            tree,
            numerator,
            denominator,
            budget: Lists::new(Vec::new()),
        })
    }

    /// The numerator and denominator at the given point.
    pub(crate) fn evaluate(&self, x: f64, y: f64) -> (f64, f64) {
        let env = if self.tree.function_of_y {
            Env::Value(y)
        } else {
            Env::Pair(x, y)
        };
        let ctx = Ctx::<true> {
            meter: Some(&self.budget),
            ..Ctx::new(self.construction, &self.tree, &env)
        };
        let denominator = self.denominator.map_or(1.0, |i| ctx.fast(i));
        let numerator = ctx.fast(self.numerator);
        (numerator, denominator)
    }
}

fn split(c: &Construction, tree: &mut Entry, node: u32) -> (u32, Option<u32>) {
    match tree.nodes[node as usize] {
        Node::Bin(Bin::Mul, a, b) => {
            let (an, ad) = split(c, tree, a);
            let (bn, bd) = split(c, tree, b);
            (
                multiply(tree, an, bn),
                match (ad, bd) {
                    (Some(a), Some(b)) => Some(multiply(tree, a, b)),
                    _ => ad.or(bd),
                },
            )
        }
        Node::Bin(Bin::Div, a, b) => {
            let (an, ad) = split(c, tree, a);
            let (bn, bd) = split(c, tree, b);
            (
                bd.map_or(an, |d| multiply(tree, an, d)),
                Some(ad.map_or(bn, |d| multiply(tree, bn, d))),
            )
        }
        Node::Bin(Bin::Pow, base, exponent) => {
            let (num, den) = split(c, tree, base);
            // A deep copy gives each FunctionVariable its initial value, 0.
            let value = Ctx::<false>::new(c, tree, &Env::Pair(0.0, 0.0)).fast(exponent);
            if value < 0.0 {
                let exponent = negate(tree, exponent);
                let numerator = match den {
                    Some(d) => power(tree, d, exponent),
                    None => constant(tree, 1.0),
                };
                (numerator, Some(power(tree, num, exponent)))
            } else {
                (
                    power(tree, num, exponent),
                    den.map(|d| power(tree, d, exponent)),
                )
            }
        }
        _ => (node, None),
    }
}

fn literal(tree: &Entry, i: u32) -> Option<f64> {
    match tree.nodes[i as usize] {
        Node::Num(v) => Some(v),
        Node::MinusOne => Some(-1.0),
        Node::Degree => Some(super::math::PI_180),
        _ => None,
    }
}

fn multiply(tree: &mut Entry, a: u32, b: u32) -> u32 {
    match (literal(tree, a), literal(tree, b)) {
        (_, Some(0.0)) | (Some(1.0), _) => b,
        (_, Some(1.0)) | (Some(0.0), _) => a,
        // ExpressionNode.multiply puts its argument first.
        (_, Some(-1.0)) => {
            let minus_one = push(tree, Node::MinusOne, Ty::Num(0), Lit::default());
            binary(tree, Bin::Mul, minus_one, a)
        }
        _ => binary(tree, Bin::Mul, b, a),
    }
}

fn power(tree: &mut Entry, base: u32, exponent: u32) -> u32 {
    match literal(tree, exponent) {
        Some(0.0) => constant(tree, 1.0),
        Some(1.0) => base,
        _ => binary(tree, Bin::Pow, base, exponent),
    }
}

fn negate(tree: &mut Entry, i: u32) -> u32 {
    match tree.nodes[i as usize] {
        Node::Num(v) => {
            let lit = &tree.lits[i as usize];
            let negated = Lit {
                special: lit.special,
                dec: lit.dec.as_ref().map(super::decimal::Dec::negated),
            };
            push(tree, Node::Num(-v), tree.tys[i as usize], negated)
        }
        Node::MinusOne => constant(tree, 1.0),
        Node::Bin(Bin::Mul, a, b) if matches!(tree.nodes[b as usize], Node::Degree) => {
            let negative = negate(tree, a);
            binary(tree, Bin::Mul, negative, b)
        }
        _ => {
            let minus_one = push(tree, Node::MinusOne, Ty::Num(0), Lit::default());
            binary(tree, Bin::Mul, minus_one, i)
        }
    }
}

fn constant(tree: &mut Entry, value: f64) -> u32 {
    push(tree, Node::Num(value), Ty::Num(0), Lit::default())
}

fn binary(tree: &mut Entry, op: Bin, a: u32, b: u32) -> u32 {
    let ty = types::scalar_binary_type(op, tree.tys[a as usize], tree.tys[b as usize])
        .expect("a function fraction combines scalar or vector operands");
    push(tree, Node::Bin(op, a, b), ty, Lit::default())
}

fn push(tree: &mut Entry, node: Node, ty: Ty, lit: Lit) -> u32 {
    let i = tree.nodes.len() as u32;
    tree.nodes.push(node);
    tree.lits.push(lit);
    tree.tys.push(ty);
    i
}
