// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoNumeratorDenominator.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoNumeratorDenominatorFun.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/Fractions.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNode.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! `Fractions.getFraction` in the mode used by `AlgoNumeratorDenominatorFun`.
//! Split a function's products, quotients and powers once, then evaluate
//! the numerator and denominator separately at each sample.

use super::equation::Builder;
use super::eval::{Ctx, Env, Lists, Local, Val};
use super::{entry, types, Bin, Entry, Lit, Node, Ty};
use crate::kernel::{Construction, Def, Id, Value};
use std::borrow::Cow;

/// `AlgoNumeratorDenominator`: reduce integral parts of the definition, or
/// use the continued fraction of the value if the definition has no such parts.
pub(super) fn number_parts<const M: bool>(ctx: &Ctx<'_, M>, input: u32) -> Option<(f64, f64)> {
    let value = ctx.full_number(input);
    if value.is_nan() {
        return None;
    }
    let parts = |e: &Entry, root| {
        let mut b = Builder::function(e);
        let (n, d) = get_fraction(&mut b, root, true);
        let n = b.number_value(ctx.c, n?)?;
        let d = b.number_value(ctx.c, d?)?;
        if n.is_finite() && d.is_finite() && n == n.round() && d == d.round() {
            let fraction = crate::cas::Rat::new(n as i64 as i128, d as i64 as i128)?;
            Some((fraction.num as f64, fraction.den as f64))
        } else {
            None
        }
    };
    let exact = if let Node::Obj(id) = ctx.node(input) {
        if let Some(source) = ctx.c.construction_source(id) {
            super::parse::parse(ctx.c, source).ok().and_then(|parsed| {
                let entry = Entry::new(ctx.c, parsed, false);
                parts(&entry, entry.root)
            })
        } else if let Def::Expr(def) = ctx.c.def(id) {
            let entry = entry(ctx.c, def);
            parts(entry, entry.root)
        } else {
            None
        }
    } else {
        parts(ctx.e, input)
    };
    Some(exact.unwrap_or_else(|| {
        super::derivative::decimal_to_fraction(value, crate::tol::STANDARD_PRECISION)
    }))
}

pub(super) fn project_function(
    c: &Construction,
    e: &Entry,
    input: u32,
    denominator: bool,
) -> Option<Entry> {
    let mut b = Builder::function(e);
    let (n, d) = get_fraction(&mut b, input, false);
    let root = if denominator {
        d.unwrap_or_else(|| b.push(Node::Num(1.0)))
    } else {
        n?
    };
    let entry = super::function_entry(c, b.finish(c, root).ok()?);
    super::tree_defined(c, &entry).then_some(entry)
}

/// `Fractions.getFraction`: the numerator and denominator trees of the
/// expression at `i`, `None` standing for the implicit 1. With `expand`
/// the sums are put over a common denominator (`AlgoNumeratorDenominator`)
/// and without it they are left alone
/// (`AlgoNumeratorDenominatorFun`).
pub(super) fn get_fraction(b: &mut Builder, i: u32, expand: bool) -> (Option<u32>, Option<u32>) {
    let side = |b: &mut Builder, i: u32| -> (u32, Option<u32>) {
        // checkFraction recurses into a subtree that is itself a node,
        // and leaves the node itself otherwise
        match b.node(i) {
            Node::Num(_)
            | Node::Var
            | Node::MinusOne
            | Node::Degree
            | Node::Obj(_)
            | Node::Bool(_)
            | Node::VarY
            | Node::Local(..) => (i, None),
            _ => {
                let (num, den) = get_fraction(b, i, expand);
                (num.unwrap_or(i), den)
            }
        }
    };
    let multiply_check = |b: &mut Builder, a: Option<u32>, d: Option<u32>| -> Option<u32> {
        match (a, d) {
            (None, None) => None,
            (Some(a), None) | (None, Some(a)) => Some(a),
            (Some(a), Some(d)) => Some(b.push(Node::Bin(Bin::Mul, a, d))),
        }
    };
    let Node::Bin(op, left, right) = b.node(i) else {
        return (Some(i), None);
    };
    let (num_l, den_l) = side(b, left);
    let (num_r, den_r) = side(b, right);
    match op {
        Bin::Mul => {
            let num = b.push(Node::Bin(Bin::Mul, num_l, num_r));
            let den = multiply_check(b, den_r, den_l);
            (Some(num), den)
        }
        Bin::Div => {
            let num = multiply_check(b, den_r, Some(num_l));
            let den = multiply_check(b, den_l, Some(num_r));
            (num, den)
        }
        Bin::Pow | Bin::RootPow => {
            let exponent = b.constant_value(right);
            let power_check = |b: &mut Builder, base: Option<u32>, e: u32| -> Option<u32> {
                base.map(|base| b.push(Node::Bin(Bin::Pow, base, e)))
            };
            match exponent {
                Some(e) if e < 0.0 => {
                    // a negative exponent swaps the sides, negated
                    let neg = match b.node(right) {
                        Node::Num(v) => b.push(Node::Num(-v)),
                        _ => {
                            let m = b.push(Node::Num(-1.0));
                            b.push(Node::Bin(Bin::Mul, m, right))
                        }
                    };
                    let den = power_check(b, Some(num_l), neg);
                    let num = match den_l {
                        None => Some(b.push(Node::Num(1.0))),
                        Some(den_l) => power_check(b, Some(den_l), neg),
                    };
                    (num, den)
                }
                _ => {
                    let num = power_check(b, Some(num_l), right);
                    let den = power_check(b, den_l, right);
                    (num, den)
                }
            }
        }
        Bin::Add | Bin::Sub if expand => {
            let l = multiply_check(b, den_r, Some(num_l)).unwrap();
            let r = multiply_check(b, den_l, Some(num_r)).unwrap();
            let num = b.push(Node::Bin(op, l, r));
            let den = multiply_check(b, den_r, den_l);
            (Some(num), den)
        }
        _ => (Some(i), None),
    }
}

pub(crate) struct FunctionFraction<'a> {
    construction: &'a Construction,
    tree: Cow<'a, Entry>,
    numerator: u32,
    denominator: Option<u32>,
    budget: Lists,
    system: bool,
    symbolic: Option<&'a crate::cas::Quotient>,
}

impl<'a> FunctionFraction<'a> {
    pub(crate) fn new(construction: &'a Construction, id: Id) -> Option<Self> {
        let Def::Expr(def) = construction.def(id) else {
            return None;
        };
        let source = entry(construction, def);
        let (root, system) = match (construction.value(id), source.nodes[source.root as usize]) {
            (Value::Function, _) => (source.root, false),
            (Value::FunctionNVar, Node::SystemFunction(2, root)) => (root, true),
            (Value::FunctionNVar, Node::Cas(..)) => (source.root, false),
            _ => return None,
        };
        let symbolic = source.cas_quotient(root);
        let mut tree = Cow::Borrowed(source);
        let (numerator, denominator) = match source.nodes[root as usize] {
            Node::Bin(Bin::Mul | Bin::Div | Bin::Pow | Bin::RootPow, ..) => {
                split(construction, tree.to_mut(), root)
            }
            _ => (root, None),
        };
        Some(Self {
            construction,
            tree,
            numerator,
            denominator,
            budget: Lists::new(Vec::new()),
            system,
            symbolic,
        })
    }

    /// Whether the denominator depends on a coordinate.
    pub(crate) fn has_nonconstant_denominator(&self) -> bool {
        if let Some(rational) = self.symbolic {
            return rational.denominator.scalar().is_none();
        }
        let Some(denominator) = self.denominator else {
            return false;
        };
        let mut pending = vec![denominator];
        while let Some(i) = pending.pop() {
            if !self.budget.charge(1) {
                return true;
            }
            let node = self.tree.nodes[i as usize];
            if matches!(node, Node::Var | Node::VarY | Node::Local(..)) {
                return true;
            }
            pending.extend(super::children(&node));
        }
        false
    }

    pub(crate) fn exhausted(&self) -> bool {
        self.budget.over()
    }

    /// The numerator and denominator at the given point.
    pub(crate) fn evaluate(&self, x: f64, y: f64) -> (f64, f64) {
        if let Some(rational) = self.symbolic {
            let variables = [x, y, f64::NAN];
            if !self
                .budget
                .charge((rational.numerator.0.len() + rational.denominator.0.len()) as u64)
            {
                return (f64::NAN, f64::NAN);
            }
            return (
                rational.numerator.value(variables),
                rational.denominator.value(variables),
            );
        }
        let env = if self.tree.function_of_y {
            Env::Value(y)
        } else {
            Env::Pair(x, y)
        };
        let first = Local {
            depth: 0,
            value: Val::N(x),
            run: None,
            vector: false,
            text: None,
            up: None,
        };
        let second = Local {
            depth: 1,
            value: Val::N(y),
            run: None,
            vector: false,
            text: None,
            up: Some(&first),
        };
        let ctx = Ctx::<true> {
            meter: Some(&self.budget),
            locals: self.system.then_some(&second),
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
        Node::Bin(Bin::Pow | Bin::RootPow, base, exponent) => {
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
            let negated = match lit {
                Lit::Special(decimal, spelling) => Lit::Special(
                    decimal.as_ref().map(super::decimal::Dec::negated),
                    spelling.clone(),
                ),
                _ => Lit::Plain,
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
