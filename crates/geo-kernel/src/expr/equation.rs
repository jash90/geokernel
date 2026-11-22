// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/Equation.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/Polynomial.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/Term.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNode.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/AlgebraProcessor.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoConic.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Equations in `x` and `y` that make a line or a conic, as GeoGebra's
//! input bar reads them: `a: 2x + 3y = 6`, `y = m x + b`,
//! `c: x^2 + y^2 = r^2`.
//!
//! Both sides are parsed as ordinary expressions whose `x` and `y` are
//! GeoGebra's function variables ([`parse::parse_equation`]), then expanded
//! into one polynomial, the normal form `lhs - rhs`, exactly as
//! `Equation.initEquation` builds it with `Polynomial.fromNode`. The terms
//! keep GeoGebra's order, and their coefficients are trees of the merged
//! arena that read the objects the equation names: a term may hold the `m`
//! of `y = m x + b`. The call of a function whose argument reads `x` or `y`
//! is expanded into the body of the function with the argument substituted
//! (`makePolyTreeFromFunction`), so `h(x) = y` with `h(x) = 2x` is the line
//! `y = 2x`.
//!
//! What the equation makes (`AlgebraProcessor.processEquation`). First
//! comes function dependence: applying a function to an argument that
//! reads `x` marks the equation (`isFunctionDependent`, set by
//! `makePolyTreeFromFunction` while the normal form is built), and
//! `y = <rhs in x>` then makes a function — as an `f(x) = ...` line does —
//! however polynomial the body of the function expands to: `y = h(x)`
//! with `h(x) = x^2` is a function, not a parabola. A call with a
//! constant argument leaves a plain equation (`y = h(1) x` is a line),
//! and so does every other function-dependent equation, whose implicit
//! curve of degree 1 or 2 GeoGebra still shows as a line or a conic
//! (`h(x) = y` with `h(x) = 2x` is the line `y = 2x`).
//!
//! The degree of the normal form makes of the rest:
//!
//! - degree 0 or 1: a line with the raw coefficients `(coeff(x), coeff(y),
//!   constant)` of the normal form (`GeoLine(a, b, c)`), however small they
//!   are: `0x + 0y = 1` is the undefined line `(0, 0, -1)`;
//! - degree 2: a conic with the coefficients `a x^2 + b xy + c y^2 + dx +
//!   ey + f = 0` (`GeoConic(coeffs)`);
//! - anything else, or an equation that is no polynomial (`sin(x) = 0`):
//!   `y = <rhs in x>` makes a function, as an `f(x) = ...` line does;
//!   everything else would be an implicit curve or, when an exponent reads
//!   an object (`x^n`), an implicit polynomial, which this kernel does not
//!   have and refuses.
//!
//! The line or conic is dependent when a coefficient reads an object, as
//! `AlgoDependentLine` and `AlgoDependentConic` are: the construction
//! keeps one expression entry per coefficient (three for a line, six for a
//! conic) in its equation table (see [`kernel::EquationDef`]), recomputes
//! them when an object changes and classifies the conic again.

use super::parse;
use super::{children, eval, math, shift_children, types, Bin, Entry, Lit, Node, Raw, Ty, NONE};
use crate::algo;
use crate::algo::conic_nd::ConicData;
use crate::kernel::{Construction, Def, EquationDef, EquationRecord, Error, Id, Kind, Value};
use crate::tol::STANDARD_PRECISION;

/// `Polynomial.MAX_ALLOWED_DEGREE`: the largest exponent of a polynomial
/// equation; a larger one, like a non-integer or a negative exponent,
/// makes the equation no polynomial.
const MAX_ALLOWED_DEGREE: f64 = 300.0;

/// Upper bound on the terms one expansion may hold, so that a long product
/// cannot grow without end.
const MAX_TERMS: usize = 4096;

/// Upper bound on the nodes the bodies of the function calls of one
/// equation may add to the arena, so that nested calls cannot grow without
/// end.
const MAX_FUNCTION_NODES: usize = 4096;

/// Defines `name: lhs = rhs` as the line, conic or function the equation
/// makes, as GeoGebra's algebra processor does. The equation must have one
/// top-level `=`; inequalities are refused.
pub(crate) fn define(c: &mut Construction, name: &str, text: &str) -> Result<Id, String> {
    let (lhs_text, rhs_text) = split_equation(text)?;
    let lhs = parse::parse_equation_left(c, lhs_text)?;
    let rhs = parse::parse_equation(c, rhs_text)?;
    for (side, parsed) in [("left", &lhs), ("right", &rhs)] {
        if !matches!(parsed.tys[parsed.root as usize], Ty::Num(_)) {
            return Err(format!("the {side} side of the equation must be a number"));
        }
    }
    // one arena for both sides: the right side's nodes follow the left's
    let sides = [lhs.clone(), rhs.clone()];
    let offset = lhs.nodes.len() as u32;
    let mut nodes = lhs.nodes;
    for mut node in rhs.nodes {
        shift_children(&mut node, offset);
        nodes.push(node);
    }
    let mut lits = lhs.lits;
    lits.extend(rhs.lits);
    let rhs_root = rhs.root + offset;
    let mut builder = Builder {
        nodes,
        lits,
        function_nodes: MAX_FUNCTION_NODES,
        function_dependent: false,
    };

    let poly = match build_normal(c, &mut builder, lhs.root, rhs_root) {
        Ok(poly) => poly,
        Err(why) => {
            let what = match why {
                NotPolynomial::VariableDegree => {
                    "a variable exponent, like `x^n`, would make an implicit \
                     polynomial, which is not supported: write the exponent \
                     as a number"
                }
                NotPolynomial::ImplicitCurve => {
                    "implicit curves are not supported: write a line or a conic in x and y"
                }
            };
            return function_or_refuse(c, name, lhs_text, rhs_text, rhs.uses_y, what);
        }
    };
    // `processEquation` checks `isFunctionDependent` before the degree: a
    // right side that applies a function to an argument reading `x` keeps
    // `y = <rhs>` a function, however polynomial the body of the function
    // expands to (`y = h(x)` with `h(x) = x^2`). Every other
    // function-dependent equation would be an implicit curve, of which
    // GeoGebra shows the degree 1 and 2 ones as the line and the conic the
    // degree makes below.
    if builder.function_dependent && is_function_form(lhs_text, rhs.uses_y) {
        return super::define_function(c, name, rhs_text.trim(), false);
    }
    match poly.degree() {
        0 | 1 => {
            let coeffs = coefficients(c, &mut builder, &poly, &[(1, 0), (0, 1), (0, 0)])?;
            add(c, name, false, coeffs, sides).map_err(|e| e.to_string())
        }
        2 => {
            let coeffs = coefficients(
                c,
                &mut builder,
                &poly,
                &[(2, 0), (1, 1), (0, 2), (1, 0), (0, 1), (0, 0)],
            )?;
            add(c, name, true, coeffs, sides).map_err(|e| e.to_string())
        }
        _ => function_or_refuse(
            c,
            name,
            lhs_text,
            rhs_text,
            rhs.uses_y,
            "implicit curves are not supported: write a line or a conic in x and y",
        ),
    }
}

/// The normal form `lhs - rhs` of both sides, expanded, or why either side
/// is no polynomial in `x` and `y`. GeoGebra negates the right side and
/// adds the left one to it, which fixes the order of the terms.
fn build_normal(
    c: &Construction,
    b: &mut Builder,
    lhs: u32,
    rhs: u32,
) -> Result<Poly, NotPolynomial> {
    let left = b.expand(c, lhs)?;
    let right = b.expand(c, rhs)?;
    let mut normal = right.negated(b)?;
    normal.add(&left, b)?;
    Ok(normal)
}

/// `functionOrImplicitPoly`: whether the equation is the implicit function
/// form `y = <rhs>` whose right side reads no `y`, the form that makes a
/// function.
fn is_function_form(lhs_text: &str, rhs_uses_y: bool) -> bool {
    lhs_text.trim() == "y" && !rhs_uses_y
}

/// `AlgebraProcessor.functionOrImplicitPoly` for an equation this kernel
/// has no implicit curve or implicit polynomial for: `y = <rhs in x>` makes
/// a function, exactly as an `f(x) = ...` line does; everything else is
/// refused with `what`.
fn function_or_refuse(
    c: &mut Construction,
    name: &str,
    lhs_text: &str,
    rhs_text: &str,
    rhs_uses_y: bool,
    what: &str,
) -> Result<Id, String> {
    if is_function_form(lhs_text, rhs_uses_y) {
        return super::define_function(c, name, rhs_text.trim(), false);
    }
    Err(what.into())
}

/// The coefficient trees of the monomials `vars` of the polynomial
/// (`Polynomial.getCoefficient` sums the terms of one monomial), one
/// expression per coefficient.
fn coefficients(
    c: &Construction,
    builder: &mut Builder,
    poly: &Poly,
    vars: &[(u32, u32)],
) -> Result<Vec<parse::Parsed>, String> {
    vars.iter()
        .map(|&v| poly.coefficient(c, builder, v))
        .collect()
}

/// Splits `lhs = rhs` at the one `=` between them: the top-level `=` that
/// is not part of `==`, `!=`, `<=` or `>=` and sits outside every bracket.
pub(crate) fn split_equation(text: &str) -> Result<(&str, &str), String> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut split = None;
    let mut ordered = false;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'=' if depth == 0 => {
                let before = i.checked_sub(1).and_then(|j| bytes.get(j));
                let part_of_comparison = matches!(before, Some(b'=' | b'<' | b'>' | b'!'))
                    || matches!(bytes.get(i + 1), Some(b'='));
                if part_of_comparison {
                    continue;
                }
                if split.is_some() {
                    return Err("an equation may have only one `=`".into());
                }
                split = Some(i);
            }
            b'<' | b'>' if depth == 0 => ordered = true,
            _ => {}
        }
    }
    match split {
        Some(i) => Ok((&text[..i], &text[i + 1..])),
        None if ordered => Err("inequalities are not supported".into()),
        None => Err("expected an equation `lhs = rhs`".into()),
    }
}

// The polynomial expansion.

/// Rewrites every child index of `node` with `map` ([`shift_children`]'s
/// shape, for copying a tree into the builder's arena).
fn map_children(
    node: &mut Node,
    mut map: impl FnMut(u32) -> Result<u32, NotPolynomial>,
) -> Result<(), NotPolynomial> {
    let mut visit = |c: &mut u32| -> Result<(), NotPolynomial> {
        if *c != NONE {
            *c = map(*c)?;
        }
        Ok(())
    };
    match node {
        Node::Tuple(a, b) | Node::Polar(a, b) | Node::Bin(_, a, b) | Node::ListOp(_, a, b) => {
            visit(a)?;
            visit(b)?;
        }
        Node::Un(_, a) | Node::Apply(_, a) | Node::Item(a) | Node::Vector(a) => visit(a)?,
        Node::If(_, a, b, c) | Node::Range(a, b, c) => {
            visit(a)?;
            visit(b)?;
            visit(c)?;
        }
        Node::Seq(a, b, c, d, _) => {
            visit(a)?;
            visit(b)?;
            visit(c)?;
            visit(d)?;
        }
        Node::List(first, _) => visit(first)?,
        _ => {}
    }
    Ok(())
}

/// One monomial: the degrees of `x` and `y` and the root of its
/// coefficient tree, an index into the builder's arena.
#[derive(Clone, Copy)]
struct Term {
    x: u32,
    y: u32,
    coeff: u32,
}

/// A polynomial in `x` and `y`: GeoGebra's `Polynomial`, as its list of
/// terms in the order the operations produced them. Always simplified:
/// every operation ends in [`Poly::simplify`], which leaves no two terms
/// of one monomial and no term of a constant zero coefficient.
#[derive(Clone)]
struct Poly(Vec<Term>);

/// Why an expression is no polynomial in `x` and `y`.
enum NotPolynomial {
    /// The equation would be an implicit curve.
    ImplicitCurve,
    /// An exponent reads an object (`x^n`): GeoGebra makes an implicit
    /// polynomial of its value, which this kernel does not have.
    VariableDegree,
}

/// The merged arena of the two sides of an equation, which the coefficient
/// trees of its terms are built in.
struct Builder {
    nodes: Vec<Node>,
    lits: Vec<Lit>,
    /// How many more nodes the bodies of function calls may add (see
    /// [`MAX_FUNCTION_NODES`]).
    function_nodes: usize,
    /// Whether the expansion applied a function to an argument that reads
    /// `x` or `y` (`Equation.isFunctionDependent`, set by
    /// `makePolyTreeFromFunction`): such an equation is no plain line or
    /// conic, however polynomial its normal form is.
    function_dependent: bool,
}

impl Builder {
    fn push(&mut self, node: Node) -> u32 {
        self.nodes.push(node);
        self.lits.push(Lit::default());
        (self.nodes.len() - 1) as u32
    }

    fn node(&self, i: u32) -> Node {
        self.nodes[i as usize]
    }

    /// `ExpressionNode.makePolynomialTree` of the subtree at `i`, with the
    /// operations of `Polynomial.apply` and `Term` on the coefficients.
    fn expand(&mut self, c: &Construction, i: u32) -> Result<Poly, NotPolynomial> {
        match self.node(i) {
            Node::Var => Ok(Poly(vec![Term {
                x: 1,
                y: 0,
                coeff: self.push(Node::Num(1.0)),
            }])),
            Node::VarY => Ok(Poly(vec![Term {
                x: 0,
                y: 1,
                coeff: self.push(Node::Num(1.0)),
            }])),
            Node::Bin(Bin::Add, a, b) => {
                let mut p = self.expand(c, a)?;
                let q = self.expand(c, b)?;
                p.add(&q, self)?;
                Ok(p)
            }
            Node::Bin(Bin::Sub, a, b) => {
                let mut p = self.expand(c, a)?;
                let q = self.expand(c, b)?;
                p.sub(&q, self)?;
                Ok(p)
            }
            Node::Bin(Bin::Mul, a, b) => {
                let mut p = self.expand(c, a)?;
                let q = self.expand(c, b)?;
                p.mul(&q, self)?;
                Ok(p)
            }
            Node::Bin(Bin::Div, a, b) => {
                let mut p = self.expand(c, a)?;
                let q = self.expand(c, b)?;
                if q.degree() != 0 {
                    // `Polynomial.apply`: dividing by a variable makes the
                    // equation no polynomial
                    return Err(NotPolynomial::ImplicitCurve);
                }
                let divisor = q.constant();
                p.apply_coefficients(self, |b, coeff| b.divide(coeff, divisor))?;
                Ok(p)
            }
            Node::Bin(Bin::Pow, base, exponent) => self.power(c, base, exponent),
            Node::Apply(f, arg) => {
                // `makePolyTreeFromFunction`: a call whose argument reads
                // `x` or `y` is the body of the function, its variable
                // replaced by the argument, and the equation becomes
                // function dependent (`setFunctionDependent`); any other
                // call is one constant coefficient, like the operations
                // below
                if self.contains_var(arg) {
                    self.function_dependent = true;
                    let body = self.inline_function(c, f, arg)?;
                    self.expand(c, body)
                } else {
                    Ok(Poly(vec![Term {
                        x: 0,
                        y: 0,
                        coeff: i,
                    }]))
                }
            }
            // anything else is one constant coefficient, as
            // `makePolynomialTree` wraps a non-polynomial operation in one
            // term; a variable inside it makes the equation no polynomial
            _ if self.contains_var(i) => Err(NotPolynomial::ImplicitCurve),
            _ => Ok(Poly(vec![Term {
                x: 0,
                y: 0,
                coeff: i,
            }])),
        }
    }

    /// The body of the function `f`, copied into this arena with its
    /// variable replaced by `arg`: `h(x) = 2x` in `f: h(x) = y` gives the
    /// line `y = 2x`.
    fn inline_function(&mut self, c: &Construction, f: Id, arg: u32) -> Result<u32, NotPolynomial> {
        let body = match (c.def(f), c.value(f)) {
            (Def::Expr(d), Value::Function) => super::entry(c, d),
            _ => return Err(NotPolynomial::ImplicitCurve),
        };
        let root = body.root;
        self.copy_body(body, root, arg, 0)
    }

    /// One node of that body at `i`, its children copied the same way.
    fn copy_body(
        &mut self,
        body: &Entry,
        i: u32,
        arg: u32,
        depth: u32,
    ) -> Result<u32, NotPolynomial> {
        if depth > parse::MAX_NESTING || self.function_nodes == 0 {
            return Err(NotPolynomial::ImplicitCurve);
        }
        self.function_nodes -= 1;
        if matches!(body.nodes[i as usize], Node::Var) {
            return Ok(arg);
        }
        let mut node = body.nodes[i as usize];
        let deeper = depth + 1;
        map_children(&mut node, |child| self.copy_body(body, child, arg, deeper))?;
        let copy = self.push(node);
        self.lits[copy as usize] = body.lits[i as usize].clone();
        Ok(copy)
    }

    /// `Polynomial.apply(POWER, ...)`: the exponent must be a constant
    /// integer in `[0, MAX_ALLOWED_DEGREE]`, or the base must be a constant
    /// whose one coefficient is raised as a tree. An exponent that reads an
    /// object (`x^n`) has a variable degree, which makes an implicit
    /// polynomial upstream.
    fn power(&mut self, c: &Construction, base: u32, exponent: u32) -> Result<Poly, NotPolynomial> {
        let mut p = self.expand(c, base)?;
        let e = self.expand(c, exponent)?;
        if e.degree() != 0 {
            return Err(NotPolynomial::ImplicitCurve);
        }
        let exp_coeff = e.constant();
        if p.degree() == 0 {
            // `apply` raises the one coefficient to that power as a tree
            let coeff = p.0[0].coeff;
            p.0[0].coeff = self.push(Node::Bin(Bin::Pow, coeff, exp_coeff));
            return Ok(p);
        }
        if self.contains_object(exp_coeff) {
            return Err(NotPolynomial::VariableDegree);
        }
        let Some(power) = self.constant_value(exp_coeff) else {
            return Err(NotPolynomial::ImplicitCurve);
        };
        if !math::is_integer(power) || !(0.0..=MAX_ALLOWED_DEGREE).contains(&power) {
            return Err(NotPolynomial::ImplicitCurve);
        }
        p.pow(power as u32, self)?;
        Ok(p)
    }

    /// Whether the subtree at `i` reads the variable `x` or `y`
    /// (`containsFunctionVariable`).
    fn contains_var(&self, i: u32) -> bool {
        match self.node(i) {
            Node::Var | Node::VarY => true,
            node => children(&node).any(|child| self.contains_var(child)),
        }
    }

    /// Whether the subtree at `i` reads an object (`isDynamicGeoElement`):
    /// the coefficient of such a term is recomputed when the object
    /// changes, and it survives `simplify` even at the value 0.
    fn contains_object(&self, i: u32) -> bool {
        match self.node(i) {
            Node::Obj(_) | Node::Apply(..) | Node::Cmd(..) | Node::If(..) => true,
            node => children(&node).any(|child| self.contains_object(child)),
        }
    }

    /// The value of a subtree that reads no object (`isConstant` with
    /// `evaluateDouble`); `None` when it does read one, or is nested too
    /// deeply to look at.
    fn constant_value(&self, i: u32) -> Option<f64> {
        self.constant_at(i, 0)
    }

    fn constant_at(&self, i: u32, depth: u32) -> Option<f64> {
        if depth > parse::MAX_NESTING {
            return None;
        }
        let value = |j: u32| self.constant_at(j, depth + 1);
        Some(match self.node(i) {
            Node::Num(v) => v,
            Node::MinusOne => -1.0,
            Node::Degree => math::PI_180,
            Node::Un(op, a) => eval::scalar(op, value(a)?, false),
            Node::Bin(op, a, b) => {
                let (x, y) = (value(a)?, value(b)?);
                match op {
                    Bin::Add => x + y,
                    Bin::Sub => x - y,
                    Bin::Mul => x * y,
                    Bin::Div => x / y,
                    Bin::Pow => math::my_pow(x, y),
                    _ => return None,
                }
            }
            _ => return None,
        })
    }

    /// `Term.add` of two coefficients: constants fold into one number,
    /// and the tree is otherwise the operation.
    fn add(&mut self, a: u32, b: u32) -> u32 {
        match (self.constant_value(a), self.constant_value(b)) {
            (Some(x), Some(y)) => self.push(Node::Num(x + y)),
            _ => self.push(Node::Bin(Bin::Add, a, b)),
        }
    }

    /// `Term.multiply` of two coefficients, with its constant reductions.
    fn multiply(&mut self, a: u32, b: u32) -> u32 {
        match (self.constant_value(a), self.constant_value(b)) {
            (Some(x), Some(y)) => self.push(Node::Num(x * y)),
            (Some(0.0), _) => self.push(Node::Num(0.0)),
            (Some(1.0), _) => b,
            _ => self.push(Node::Bin(Bin::Mul, a, b)),
        }
    }

    /// `Term.divide` of two coefficients, with its constant reductions.
    fn divide(&mut self, a: u32, b: u32) -> u32 {
        match (self.constant_value(a), self.constant_value(b)) {
            (Some(x), Some(y)) => self.push(Node::Num(x / y)),
            (Some(0.0), _) => self.push(Node::Num(0.0)),
            (_, Some(1.0)) => a,
            _ => self.push(Node::Bin(Bin::Div, a, b)),
        }
    }

    fn collect_reads(&self, c: &Construction, i: u32, parents: &mut Vec<Id>, inputs: &mut Vec<Id>) {
        let mut read = |id: Id, input: bool| {
            if !parents.contains(&id) {
                parents.push(id);
            }
            if input && !inputs.contains(&id) {
                inputs.push(id);
            }
        };
        match self.node(i) {
            Node::Obj(id) => read(id, true),
            Node::Apply(f, a) => {
                read(f, true);
                self.collect_reads(c, a, parents, inputs);
            }
            Node::Cmd(def, _) => {
                for &p in def.parents().as_slice() {
                    read(p, false);
                }
                for p in crate::algo::intersect::hidden_parents(c, &def) {
                    read(p, false);
                }
                for p in crate::algo::vector::hidden_parents(c, &def) {
                    read(p, false);
                }
                let node = self.node(i);
                for child in children(&node) {
                    self.collect_reads(c, child, parents, inputs);
                }
            }
            node => {
                for child in children(&node) {
                    self.collect_reads(c, child, parents, inputs);
                }
            }
        }
    }
}

impl Poly {
    /// The degree of the polynomial: the largest sum of the variable
    /// degrees of its terms (`Polynomial.degree`).
    fn degree(&self) -> u32 {
        self.0
            .iter()
            .map(|t| t.x.saturating_add(t.y))
            .max()
            .unwrap_or(0)
    }

    /// The coefficient tree of the one term of a degree 0 polynomial.
    fn constant(&self) -> u32 {
        self.0[0].coeff
    }

    /// The coefficient tree of one monomial of the finished polynomial:
    /// the terms of that monomial summed left to right
    /// (`Polynomial.getCoefficient` starts from `0`, whose first addition
    /// gives the first coefficient unchanged).
    fn coefficient(
        &self,
        c: &Construction,
        b: &mut Builder,
        vars: (u32, u32),
    ) -> Result<parse::Parsed, String> {
        let terms = self.terms_of(vars);
        let mut acc: Option<u32> = None;
        for term in terms {
            acc = Some(match acc {
                None => term,
                Some(a) => b.add(a, term),
            });
        }
        let root = acc.unwrap_or_else(|| b.push(Node::Num(0.0)));
        let (mut parents, mut inputs) = (Vec::new(), Vec::new());
        b.collect_reads(c, root, &mut parents, &mut inputs);
        let (tys, depth, cost, vars_read) = types::check(c, &b.nodes, root)?;
        let r = root as usize;
        Ok(parse::Parsed {
            root,
            depth: depth[r],
            cost: cost[r],
            vars: vars_read[r],
            nodes: b.nodes.clone(),
            lits: b.lits.clone(),
            tys,
            parents,
            inputs,
            uses_var: false,
            uses_y: false,
        })
    }

    /// The coefficient roots of the monomial `vars`, in term order.
    fn terms_of(&self, vars: (u32, u32)) -> Vec<u32> {
        self.0
            .iter()
            .filter(|t| (t.x, t.y) == vars)
            .map(|t| t.coeff)
            .collect()
    }

    /// `Polynomial.sub`: `-1` times `e`, added to this polynomial.
    fn sub(&mut self, e: &Poly, b: &mut Builder) -> Result<(), NotPolynomial> {
        let negated = e.negated(b)?;
        self.add(&negated, b)
    }

    /// This polynomial times `-1`, on a copy (`Polynomial.sub` builds it
    /// on one).
    fn negated(&self, b: &mut Builder) -> Result<Poly, NotPolynomial> {
        let mut negated = self.clone();
        negated.apply_coefficients(b, |b, coeff| {
            let minus_one = b.push(Node::Num(-1.0));
            b.multiply(minus_one, coeff)
        })?;
        Ok(negated)
    }

    /// `Polynomial.add`: the terms of `e` appended to this one's, then
    /// [`Poly::simplify`].
    fn add(&mut self, e: &Poly, b: &mut Builder) -> Result<(), NotPolynomial> {
        self.0.extend(e.0.iter().copied());
        self.simplify(b)
    }

    /// `Polynomial.multiply`: every term of this polynomial times every
    /// term of `e`, the variables joined, then [`Poly::simplify`].
    fn mul(&mut self, e: &Poly, b: &mut Builder) -> Result<(), NotPolynomial> {
        let mut out = Vec::with_capacity(self.0.len() * e.0.len());
        for ti in &self.0 {
            for tj in &e.0 {
                if out.len() >= MAX_TERMS {
                    return Err(NotPolynomial::ImplicitCurve);
                }
                let x = ti.x.checked_add(tj.x).ok_or(NotPolynomial::ImplicitCurve)?;
                let y = ti.y.checked_add(tj.y).ok_or(NotPolynomial::ImplicitCurve)?;
                let coeff = b.multiply(ti.coeff, tj.coeff);
                out.push(Term { x, y, coeff });
            }
        }
        self.0 = out;
        self.simplify(b)
    }

    /// `Polynomial.power`: by repeated squaring, exactly upstream's
    /// recursion (`p == 0` clears to `1`, `p == 1` stays as it is).
    fn pow(&mut self, p: u32, b: &mut Builder) -> Result<(), NotPolynomial> {
        if p == 0 {
            let one = b.push(Node::Num(1.0));
            self.0 = vec![Term {
                x: 0,
                y: 0,
                coeff: one,
            }];
            return Ok(());
        }
        if p < 2 {
            return Ok(());
        }
        let exp = self.clone();
        self.mul(&exp, b)?;
        self.pow(p / 2, b)?;
        if p % 2 == 1 {
            self.mul(&exp, b)?;
        }
        Ok(())
    }

    /// `Polynomial.multiply(number)` and `divide(number)` on every term.
    fn apply_coefficients(
        &mut self,
        b: &mut Builder,
        op: impl Fn(&mut Builder, u32) -> u32,
    ) -> Result<(), NotPolynomial> {
        for term in &mut self.0 {
            term.coeff = op(b, term.coeff);
        }
        self.simplify(b)
    }

    /// `Polynomial.simplify`: the terms of one monomial are summed and the
    /// merged ones consumed (upstream nulls every `t[j]`, also when the sum
    /// is 0 and the first term is dropped), a term whose coefficient is the
    /// constant 0 is dropped, and a polynomial of no terms keeps a single
    /// constant 0.
    fn simplify(&mut self, b: &mut Builder) -> Result<(), NotPolynomial> {
        let mut kept: Vec<Term> = Vec::with_capacity(self.0.len());
        let mut consumed = vec![false; self.0.len()];
        for i in 0..self.0.len() {
            if consumed[i] {
                continue;
            }
            consumed[i] = true;
            let mut term = self.0[i];
            for (j, other) in self.0.iter().enumerate().skip(i + 1) {
                if (other.x, other.y) == (term.x, term.y) {
                    consumed[j] = true;
                    term.coeff = b.add(term.coeff, other.coeff);
                }
            }
            // `simplify` drops a term only of a constant coefficient 0; a
            // coefficient that reads an object stays, whatever its value
            let constant_zero = !b.contains_object(term.coeff)
                && b.constant_value(term.coeff).is_none_or(|v| v == 0.0);
            if !constant_zero {
                kept.push(term);
            }
        }
        if kept.is_empty() {
            let zero = b.push(Node::Num(0.0));
            kept.push(Term {
                x: 0,
                y: 0,
                coeff: zero,
            });
        }
        self.0 = kept;
        Ok(())
    }
}

// Adding the object.

/// Adds the equation object: one expression entry per coefficient, then
/// the object, whose definition names the record of this equation. A
/// rejected object takes the entries and the record with it.
fn add(
    c: &mut Construction,
    name: &str,
    conic: bool,
    coeffs: Vec<parse::Parsed>,
    sides: [parse::Parsed; 2],
) -> Result<Id, Error> {
    let first_entry = c.exprs.entries.len();
    let mut indices = Vec::with_capacity(coeffs.len());
    for parsed in coeffs {
        let entry = Entry::new(c, parsed, false);
        let index = c.exprs.entries.len() as u32;
        c.exprs.entries.push(entry);
        indices.push(index);
    }
    let sides = sides.map(|parsed| {
        let index = c.exprs.entries.len() as u32;
        let entry = Entry::new(c, parsed, true);
        c.exprs.entries.push(entry);
        index
    });
    let index = c.equations.len() as u32;
    c.equations.push((
        c.len(),
        EquationRecord {
            coeffs: indices,
            sides,
        },
    ));
    match c.add(name, Def::Equation(EquationDef { conic, index })) {
        Ok(id) => Ok(id),
        Err(e) => {
            c.equations.pop();
            c.exprs.entries.truncate(first_entry);
            Err(e)
        }
    }
}

/// The coefficient entries of `d`'s record.
fn entries(c: &Construction, d: EquationDef) -> &[u32] {
    c.equations
        .get(d.index as usize)
        .map_or(&[][..], |(_, r)| &r.coeffs)
}

/// `Construction::check` of an equation definition: its record is there
/// and has an entry per coefficient.
pub(crate) fn check(c: &Construction, d: EquationDef) -> Result<(), Error> {
    let Some((_, record)) = c.equations.get(d.index as usize) else {
        return Err(Error::UnknownObject);
    };
    if record.coeffs.len() != usize::from(d.conic) * 3 + 3 {
        return Err(Error::UnknownObject);
    }
    if record
        .coeffs
        .iter()
        .chain(&record.sides)
        .any(|&e| e as usize >= c.exprs.entries.len())
    {
        return Err(Error::UnknownObject);
    }
    Ok(())
}

/// The objects the equation reads: the parents of its coefficients.
pub(crate) fn parents(c: &Construction, d: EquationDef) -> Vec<Id> {
    let mut out = Vec::new();
    for &e in entries(c, d) {
        for &p in &c.exprs.entries[e as usize].parents {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// The original side of a user equation, with its free coordinate variables.
fn side_entry(c: &Construction, equation: Id, left: bool) -> Option<&Entry> {
    let Def::Equation(def) = c.def(equation) else {
        return None;
    };
    let (_, record) = c.equations.get(def.index as usize)?;
    c.exprs
        .entries
        .get(record.sides[usize::from(!left)] as usize)
}

pub(super) fn side_parents(c: &Construction, equation: Id, left: bool) -> &[Id] {
    side_entry(c, equation, left).map_or(&[], |entry| &entry.parents)
}

/// Evaluate a side object at a pair of coordinates.
pub(crate) fn side_value(c: &Construction, id: Id, x: f64, y: f64) -> Option<f64> {
    let meter = eval::Lists::new(Vec::new());
    sample_side(c, id, x, y, &meter)
}

pub(super) fn sample_side(
    c: &Construction,
    id: Id,
    x: f64,
    y: f64,
    meter: &eval::Lists,
) -> Option<f64> {
    let (Def::Expr(def), Value::FunctionNVar) = (c.def(id), c.value(id)) else {
        return None;
    };
    let e = super::entry(c, def);
    let Node::Side(equation, left) = e.nodes[e.root as usize] else {
        return None;
    };
    if let Some(entry) = side_entry(c, equation, left) {
        if !meter.charge(1) {
            return None;
        }
        let env = eval::Env::Pair(x, y);
        let ctx = eval::Ctx::<true> {
            meter: Some(meter),
            ..eval::Ctx::new(c, entry, &env)
        };
        return Some(ctx.fast(entry.root));
    }
    let (lhs, rhs) = match c.value(equation) {
        Value::Line { a: 0.0, b, c } => (y, -c / b),
        Value::Line { a, b: 0.0, c } => (x, -c / a),
        Value::Line { a, b, c } => {
            let g = crate::equation::equation_scale(&[a, b, c]);
            (a / g * x + b / g * y, -c / g)
        }
        Value::Circle { cx, cy, r } => ((x - cx).powi(2) + (y - cy).powi(2), r * r),
        Value::Conic { .. } => {
            let [a, b, constant, h, u, v] = c.conic(equation)?.matrix;
            let coefficients = [a, 2.0 * h, b, 2.0 * u, 2.0 * v, constant];
            let g = crate::equation::equation_scale(&coefficients);
            let lhs = coefficients[..5]
                .iter()
                .zip([x * x, x * y, y * y, x, y])
                .filter(|(coefficient, _)| coefficient.abs() >= STANDARD_PRECISION)
                .map(|(coefficient, term)| coefficient / g * term)
                .sum();
            (lhs, -constant / g)
        }
        _ => return None,
    };
    Some(if left { lhs } else { rhs })
}

pub(crate) fn define_side(
    c: &mut Construction,
    label: &str,
    input: &str,
) -> Option<Result<Id, String>> {
    if !input.starts_with("LeftSide") && !input.starts_with("RightSide") {
        return None;
    }
    let (command, args) = crate::script::split_call(input)?;
    if !matches!(command, "LeftSide" | "RightSide") {
        return None;
    }
    let &[arg] = args.as_slice() else {
        return Some(Err(format!("`{command}` takes one equation")));
    };
    let start = c.checkpoint();
    let result = (|| {
        let equation = if let Some(id) = c.id(arg) {
            id
        } else if split_equation(arg).is_ok() {
            let hidden = format!("\0equation{}", c.len());
            define(c, &hidden, arg)?
        } else {
            return Err(format!("expected a line or conic, got `{arg}`"));
        };
        if !matches!(c.kind(equation), Kind::Line | Kind::Circle | Kind::Conic) {
            return Err(format!("expected a line or conic, got `{arg}`"));
        }
        let node = Node::Side(equation, command == "LeftSide");
        let parsed = super::reparse(c, Kind::FunctionNVar, &[node], &[Lit::default()], 0)
            .ok_or("cannot build an equation side")?;
        super::add(c, label, parsed, Kind::FunctionNVar)
    })();
    if result.is_err() {
        c.discard_since(start);
    }
    Some(result)
}

/// Evaluates the coefficients and stores their results in their entries;
/// the values, and whether any changed, bit for bit.
fn evaluate_all(c: &mut Construction, d: EquationDef) -> (Vec<f64>, bool) {
    let coeffs = entries(c, d).to_vec();
    let mut values = Vec::with_capacity(coeffs.len());
    let mut changed = false;
    for index in coeffs {
        let v = evaluate_number(c, index);
        let raw = Raw::Num(v);
        let entry = &mut c.exprs.entries[index as usize];
        changed |= !entry.raw.same(raw);
        entry.raw = raw;
        entry.extra = super::StoredExtra::None;
        values.push(v);
    }
    (values, changed)
}

/// The value of coefficient entry `index` for the current values of its
/// parents.
fn evaluate_number(c: &Construction, index: u32) -> f64 {
    let entry = &c.exprs.entries[index as usize];
    match eval::root(c, entry, None).0 {
        Raw::Num(v) => v,
        _ => f64::NAN,
    }
}

/// `GeoLine.isDefined`: the coefficients are numbers and `(a, b)` is not
/// both zero at the standard precision.
fn line_value(v: &[f64]) -> Value {
    let [a, b, c] = v.try_into().unwrap_or([f64::NAN; 3]);
    let is_zero = |x: f64| -STANDARD_PRECISION < x && x < STANDARD_PRECISION;
    if a.is_finite() && b.is_finite() && c.is_finite() && !(is_zero(a) && is_zero(b)) {
        Value::Line { a, b, c }
    } else {
        Value::Undefined
    }
}

/// `GeoConic.setCoeffs`: the matrix of the coefficients of
/// `a x^2 + b xy + c y^2 + dx + ey + f = 0`, classified.
fn set_conic(record: &mut ConicData, v: &[f64]) {
    let [a, b, c, d, e, f] = v.try_into().unwrap_or([f64::NAN; 6]);
    record.set_matrix([a, c, f, b / 2.0, d / 2.0, e / 2.0]);
}

/// The value of a new equation object, and the record of a conic one
/// (`AlgoDependentLine.compute`, `AlgoDependentConic.compute`).
pub(crate) fn initialize(c: &mut Construction, d: EquationDef) -> (Value, Option<ConicData>) {
    let (values, _) = evaluate_all(c, d);
    if d.conic {
        let mut record = ConicData::new();
        set_conic(&mut record, &values);
        let value = algo::general_conic::value(Kind::Conic, &record);
        (value, Some(record))
    } else {
        (line_value(&values), None)
    }
}

/// The value of the equation object `d` for the current values of its
/// parents, without storing anything (a probe).
pub(crate) fn current(c: &Construction, d: EquationDef) -> Value {
    let values: Vec<f64> = entries(c, d)
        .iter()
        .map(|&index| evaluate_number(c, index))
        .collect();
    if d.conic {
        let mut record = ConicData::new();
        set_conic(&mut record, &values);
        algo::general_conic::value(Kind::Conic, &record)
    } else {
        line_value(&values)
    }
}

/// Recomputes the equation object `d` whose conic record was `prev`:
/// whether any coefficient changed, its new value and, for a conic, its
/// new record. Nothing changed means nothing downstream can change.
pub(crate) fn recompute(
    c: &mut Construction,
    d: EquationDef,
    prev: ConicData,
) -> (bool, Value, Option<ConicData>) {
    let (values, changed) = evaluate_all(c, d);
    if !changed {
        return (false, Value::Undefined, None);
    }
    if d.conic {
        let mut record = prev;
        set_conic(&mut record, &values);
        (
            true,
            algo::general_conic::value(Kind::Conic, &record),
            Some(record),
        )
    } else {
        (true, line_value(&values), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::conic_nd::ConicType;
    use crate::script::{exec, load};

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn line(c: &Construction, label: &str) -> (f64, f64, f64) {
        match c.value(c.id(label).unwrap()) {
            Value::Line { a, b, c } => (a, b, c),
            other => panic!("`{label}` is not a defined line: {other:?}"),
        }
    }

    fn conic(c: &Construction, label: &str) -> ConicData {
        c.conic(c.id(label).unwrap()).unwrap()
    }

    #[test]
    fn linear_equations_make_lines_with_raw_coefficients() {
        let c = load(
            "a: 2x + 3y = 6\n\
             b: y = 2x + 1\n\
             d: x = 3\n\
             g: 3x - y + 2 = 0\n\
             e: 2 = y\n",
        )
        .unwrap();
        // the coefficients of the normal form `lhs - rhs`
        assert_eq!(line(&c, "a"), (2.0, 3.0, -6.0));
        assert_eq!(line(&c, "b"), (-2.0, 1.0, -1.0));
        assert_eq!(line(&c, "d"), (1.0, 0.0, -3.0));
        assert_eq!(line(&c, "g"), (3.0, -1.0, 2.0));
        assert_eq!(line(&c, "e"), (0.0, -1.0, 2.0));
        assert_eq!(c.kind(c.id("a").unwrap()), Kind::Line);
    }

    #[test]
    fn quadratic_equations_make_conics() {
        let c = load(
            "c: x^2 + y^2 = 9\n\
             k: (x - 1)^2 + (y + 2)^2 = 4\n\
             e: x^2/4 + y^2/9 = 1\n\
             p: y = x^2 - 3\n\
             h: x y = 1\n",
        )
        .unwrap();
        let circle = conic(&c, "c");
        assert_eq!(circle.ty, ConicType::Circle);
        assert_eq!(circle.matrix, [1.0, 1.0, -9.0, 0.0, 0.0, 0.0]);
        let translated = conic(&c, "k");
        assert_eq!(translated.ty, ConicType::Circle);
        assert!(close(translated.midpoint[0], 1.0) && close(translated.midpoint[1], -2.0));
        assert!(close(translated.half_axes[0], 2.0));
        let ellipse = conic(&c, "e");
        assert_eq!(ellipse.ty, ConicType::Ellipse);
        // the coefficients [0.25, 1/9, -1] stay below the smallest size
        // GeoGebra keeps, so the classification scales the matrix by 4
        assert_eq!(ellipse.matrix, [1.0, 4.0 / 9.0, -4.0, 0.0, 0.0, 0.0]);
        assert!(close(ellipse.half_axes[0], 3.0) && close(ellipse.half_axes[1], 2.0));
        assert_eq!(conic(&c, "p").ty, ConicType::Parabola);
        let hyperbola = conic(&c, "h");
        assert_eq!(hyperbola.ty, ConicType::Hyperbola);
        // the xy coefficient 0.5 is below the smallest size, so the matrix
        // is scaled by 2
        assert_eq!(hyperbola.matrix, [0.0, 0.0, -2.0, 1.0, 0.0, 0.0]);
        assert_eq!(c.kind(c.id("c").unwrap()), Kind::Conic);
    }

    #[test]
    fn a_general_quadratic_and_degenerate_conics() {
        let c = load(
            "q: 3x^2 - 2x y + y^2 + x - 5 = 0\n\
             i: x^2 - y^2 = 0\n\
             s: (x + 1)^2 + y^2 = 0\n",
        )
        .unwrap();
        let q = conic(&c, "q");
        // a x² + b xy + c y² + dx + ey + f = 0 gives the matrix
        // [a, c, f, b/2, d/2, e/2]
        assert_eq!(q.matrix, [3.0, 1.0, -5.0, -1.0, 0.5, 0.0]);
        assert_eq!(q.ty, ConicType::Ellipse);
        assert_eq!(conic(&c, "i").ty, ConicType::IntersectingLines);
        let point = conic(&c, "s");
        assert_eq!(point.ty, ConicType::SinglePoint);
        assert!(close(point.midpoint[0], -1.0) && close(point.midpoint[1], 0.0));
    }

    #[test]
    fn a_degree_zero_equation_makes_an_undefined_line() {
        let c = load("a: 0x + 0y = 1\nb: 2 = 2\n").unwrap();
        for label in ["a", "b"] {
            assert_eq!(c.kind(c.id(label).unwrap()), Kind::Line);
            assert_eq!(c.value(c.id(label).unwrap()), Value::Undefined);
        }
    }

    #[test]
    fn coefficients_that_read_objects_update_with_them() {
        let mut c = load("m = 2\nb = 1\na: y = m x + b\nr = 3\nk: x^2 + y^2 = r^2\n").unwrap();
        // the normal form of `y = m x + b` is `-m x + y - b`
        assert_eq!(line(&c, "a"), (-2.0, 1.0, -1.0));
        assert!(matches!(c.def(c.id("a").unwrap()), Def::Equation(_)));
        assert_eq!(conic(&c, "k").ty, ConicType::Circle);
        assert!(close(conic(&c, "k").half_axes[0], 3.0));
        // the slider steps of the golden cases
        c.set_number(c.id("m").unwrap(), 3.0).unwrap();
        c.set_number(c.id("b").unwrap(), -2.0).unwrap();
        assert_eq!(line(&c, "a"), (-3.0, 1.0, 2.0));
        c.set_number(c.id("r").unwrap(), 5.0).unwrap();
        assert!(close(conic(&c, "k").half_axes[0], 5.0));
        // a coefficient that becomes zero: the line loses both its x and y
        c.set_number(c.id("r").unwrap(), 0.0).unwrap();
        assert_eq!(conic(&c, "k").ty, ConicType::SinglePoint);
        let mut c = load("m = 1\na: m x = 2\n").unwrap();
        assert_eq!(line(&c, "a"), (1.0, 0.0, -2.0));
        c.set_number(c.id("m").unwrap(), 0.0).unwrap();
        assert_eq!(c.value(c.id("a").unwrap()), Value::Undefined);
        c.set_number(c.id("m").unwrap(), 1.0).unwrap();
        assert_eq!(line(&c, "a"), (1.0, 0.0, -2.0));
    }

    #[test]
    fn equations_that_mention_an_axis() {
        // `xAxis` and `yAxis` stay reserved names inside equations: a
        // command that reads the axis line still gives its number
        // (`y = Slope(xAxis)` is the line y = 0, `y = Distance(A, yAxis)`
        // with A = (3, 0) the line y = 3, as in GeoGebra), while the axis
        // itself is no number, so an equation that reads it as one is
        // refused — GeoGebra refuses these lines too (``Illegal
        // multiplication -1 * yAxis``). So is a line that would relabel
        // an axis.
        let c = load("A = (3, 0)\na: y = Slope(xAxis)\nb: y = Distance(A, yAxis)\n").unwrap();
        assert_eq!(line(&c, "a"), (0.0, 1.0, 0.0));
        assert_eq!(line(&c, "b"), (0.0, 1.0, -3.0));
        for refused in [
            "c: xAxis = yAxis",
            "d: y = xAxis",
            "e: xAxis + y = 1",
            "f: 2xAxis = 3",
            "g: y = x(xAxis)",
            "xAxis: 2x = 4",
            "yAxis = 3",
        ] {
            let Err(why) = load(refused) else {
                panic!("`{refused}` should be refused");
            };
            assert!(
                why.message.contains("cannot be used in an expression")
                    || why.message.contains("already exists"),
                "`{refused}` refused with `{}`",
                why.message
            );
        }
    }

    #[test]
    fn an_equation_may_read_the_coordinates_of_a_point() {
        let mut c = load("A = (1, 5)\na: x(A) + y = 2\n").unwrap();
        // `x(A)` is no variable of the equation: it belongs to the
        // constant coefficient, as in GeoGebra, so the line is
        // y = 2 - x(A)
        assert_eq!(line(&c, "a"), (0.0, 1.0, -1.0));
        c.move_point(c.id("A").unwrap(), 3.0, 0.0).unwrap();
        assert_eq!(line(&c, "a"), (0.0, 1.0, 1.0));
    }

    #[test]
    fn y_is_the_variable_and_a_label_y_is_not_an_object() {
        // `y` inside the equation is the variable, never an object
        let c = load("a: y = 2x + 1\n").unwrap();
        assert!(c.id("y").is_none());
        assert!(c.id("x").is_none());
        assert!(matches!(c.def(c.id("a").unwrap()), Def::Equation(_)));
    }

    #[test]
    fn a_variable_exponent_is_refused_with_its_own_message() {
        // GeoGebra makes an implicit polynomial of `x^n`, which this kernel
        // does not have
        let mut c = Construction::new();
        exec(&mut c, "n = 2").unwrap();
        let err = exec(&mut c, "a: x^n = 1").unwrap_err();
        assert!(err.contains("variable exponent"), "{err}");
        assert!(!err.contains("implicit curves"), "{err}");
        let err = exec(&mut c, "a: x^n + y = 1").unwrap_err();
        assert!(err.contains("variable exponent"), "{err}");
        // `y = <rhs in x>` is still the function form, whatever the exponent
        exec(&mut c, "f: y = x^n").unwrap();
        assert_eq!(c.kind(c.id("f").unwrap()), Kind::Function);
        // an exponent that is no number at all makes an implicit curve
        for script in ["a: x^0.5 = 1\n", "a: x^-1 = 1\n", "a: x^x = 1\n"] {
            let mut c = Construction::new();
            let err = exec(&mut c, script).unwrap_err();
            assert!(err.contains("implicit curves"), "{script}: {err}");
        }
    }

    #[test]
    fn terms_merged_to_zero_disappear_together() {
        let c = load(
            "a: x = x\n\
             b: x + y = x + y\n\
             d: x = x + 1\n\
             e: 2x + y = 2x + 5\n\
             f: x^2 - x^2 + y = 1\n\
             g: x^2 + y^2 = 1 + x^2\n\
             h: (x + 1)(x - 1) = y\n\
             k: y = x^2 + y\n\
             l: x^2 + 2x y + y^2 = x^2\n\
             m: (x + y)(x - y) = 1\n",
        )
        .unwrap();
        // the cancelled monomials leave no term behind: `x = x` is the
        // undefined line (0, 0, 0) and `x = x + 1` the undefined (0, 0, -1)
        for label in ["a", "b", "d"] {
            assert_eq!(c.kind(c.id(label).unwrap()), Kind::Line, "{label}");
            assert_eq!(c.value(c.id(label).unwrap()), Value::Undefined, "{label}");
        }
        assert_eq!(line(&c, "e"), (0.0, 1.0, -5.0));
        assert_eq!(line(&c, "f"), (0.0, 1.0, -1.0));
        assert_eq!(conic(&c, "g").ty, ConicType::ParallelLines);
        assert_eq!(conic(&c, "g").matrix, [0.0, 1.0, -1.0, 0.0, 0.0, 0.0]);
        assert_eq!(conic(&c, "h").ty, ConicType::Parabola);
        assert_eq!(conic(&c, "h").matrix, [1.0, 0.0, -1.0, 0.0, 0.0, -0.5]);
        assert_eq!(conic(&c, "k").ty, ConicType::DoubleLine);
        assert_eq!(conic(&c, "l").ty, ConicType::IntersectingLines);
        assert_eq!(conic(&c, "m").ty, ConicType::Hyperbola);
        assert_eq!(conic(&c, "m").matrix, [1.0, -1.0, -1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_constant_folded_zero_coefficient_drops_the_merged_term() {
        let mut c = load("a = 1\nf: x + a = x + 2\n").unwrap();
        // the merged `x` terms are consumed although their sum folds to the
        // constant 0, so `a - 2` alone remains: the undefined line (0, 0, -1)
        assert_eq!(c.kind(c.id("f").unwrap()), Kind::Line);
        assert_eq!(c.value(c.id("f").unwrap()), Value::Undefined);
        c.set_number(c.id("a").unwrap(), 4.0).unwrap();
        assert_eq!(c.value(c.id("f").unwrap()), Value::Undefined);
        // a merged term that keeps a variable survives the fold
        let c = load("a = 1\nf: x + a x = x + 2\n").unwrap();
        assert_eq!(line(&c, "f"), (1.0, 0.0, -2.0));
    }

    #[test]
    fn digit_prefixed_names_and_coordinate_calls_read_as_products() {
        let c = load(
            "a: 2xy = 1\n\
             b: 2 xy = 3\n\
             d: x(y) = 1\n\
             e: x(y + 1) = 1\n\
             g: y = x(x + 1)\n\
             h: y(x) = 1\n\
             k: x(3) = 1\n",
        )
        .unwrap();
        // `2xy` is 2*x*y; `xy` alone on the left stays one unknown name
        assert_eq!(conic(&c, "a").ty, ConicType::Hyperbola);
        assert_eq!(conic(&c, "b").ty, ConicType::Hyperbola);
        assert_eq!(conic(&c, "d").ty, ConicType::Hyperbola);
        // x(y + 1) = x y + x
        assert_eq!(conic(&c, "e").ty, ConicType::Hyperbola);
        assert_eq!(conic(&c, "g").ty, ConicType::Parabola);
        assert_eq!(conic(&c, "h").ty, ConicType::Hyperbola);
        assert_eq!(line(&c, "k"), (3.0, 0.0, -1.0));
        let mut c = Construction::new();
        let err = exec(&mut c, "a: xy = 1").unwrap_err();
        assert!(err.contains("unknown object `xy`"), "{err}");
    }

    #[test]
    fn a_function_call_whose_argument_is_a_variable_is_expanded() {
        let c = load("h(x) = 2x\nf: h(x) = y\ng: y + 1 = h(x)\n").unwrap();
        assert_eq!(line(&c, "f"), (2.0, -1.0, 0.0));
        assert_eq!(line(&c, "g"), (-2.0, 1.0, 1.0));
        let c = load("h(x) = x^2\nf: x^2 + y = h(x)\n").unwrap();
        assert_eq!(line(&c, "f"), (0.0, 1.0, 0.0));
        // the expansion follows the objects the function reads
        let mut c = load("w = 2\nh(x) = w x\nf: h(x) = y\n").unwrap();
        assert_eq!(line(&c, "f"), (2.0, -1.0, 0.0));
        c.set_number(c.id("w").unwrap(), -3.0).unwrap();
        assert_eq!(line(&c, "f"), (-3.0, -1.0, 0.0));
        // a call of a constant argument is one constant coefficient
        let c = load("h(x) = 2x\nf: h(2) = 4\n").unwrap();
        assert_eq!(c.value(c.id("f").unwrap()), Value::Undefined);
        // a body that is no polynomial makes no line
        let mut c = Construction::new();
        exec(&mut c, "h(x) = sin(x)").unwrap();
        let err = exec(&mut c, "f: h(x) = y").unwrap_err();
        assert!(err.contains("implicit curves"), "{err}");
        // `y = <rhs in x>` stays the function form
        exec(&mut c, "f: y = h(x)").unwrap();
        assert_eq!(c.kind(c.id("f").unwrap()), Kind::Function);
    }

    #[test]
    fn a_function_applied_to_x_keeps_y_eq_rhs_a_function() {
        // `processEquation` checks `isFunctionDependent` before the degree:
        // a right side that applies a function to an argument reading `x`
        // makes `y = <rhs>` a function, however polynomial the body of the
        // function expands to, also through a second function and with an
        // object in the body
        let c = load(
            "h(x) = x^2\n\
             f: y = h(x)\n\
             g: y = h(x) + 1\n\
             k(x) = x + 1\n\
             q(x) = k(x)^2\n\
             p: y = q(x)\n\
             a = 2\n\
             w(x) = a x^2\n\
             r: y = w(x) + x\n\
             m = 2\n\
             n(x) = m x\n\
             s: y = n(x)",
        )
        .unwrap();
        for label in ["f", "g", "p", "r", "s"] {
            let id = c.id(label).unwrap();
            assert_eq!(c.kind(id), Kind::Function, "`{label}`");
            assert!(matches!(c.def(id), Def::Expr(_)), "`{label}`");
        }
        assert_eq!(c.function_value(c.id("f").unwrap(), 3.0), Some(9.0));
        assert_eq!(c.function_value(c.id("g").unwrap(), 3.0), Some(10.0));
        assert_eq!(c.function_value(c.id("p").unwrap(), 2.0), Some(9.0));
        assert_eq!(c.function_value(c.id("r").unwrap(), 2.0), Some(10.0));
        assert_eq!(c.function_value(c.id("s").unwrap(), 3.0), Some(6.0));
        // a step that empties the body keeps the function a function
        let mut c = load("a = 2\nh(x) = a x\nf: y = h(x)").unwrap();
        assert_eq!(c.kind(c.id("f").unwrap()), Kind::Function);
        assert_eq!(c.function_value(c.id("f").unwrap(), 5.0), Some(10.0));
        c.set_number(c.id("a").unwrap(), 0.0).unwrap();
        assert_eq!(c.kind(c.id("f").unwrap()), Kind::Function);
        assert_eq!(c.function_value(c.id("f").unwrap(), 5.0), Some(0.0));
    }

    #[test]
    fn a_call_of_a_constant_argument_keeps_a_line() {
        // `y = h(2)` and `y = h(1) x` apply the function to a constant, so
        // the equation is not function dependent and stays a line, like
        // every function-dependent equation that is not `y = <rhs>`
        let c = load("h(x) = 2x\nf: y = h(2)\ng: y = h(1) x\nk: h(x) = y").unwrap();
        assert_eq!(line(&c, "f"), (0.0, 1.0, -4.0));
        assert_eq!(line(&c, "g"), (-2.0, 1.0, 0.0));
        assert_eq!(line(&c, "k"), (2.0, -1.0, 0.0));
    }

    #[test]
    fn implicit_curves_and_inequalities_are_refused() {
        for line in [
            "a: y^2 = x^3",
            "a: sin(x) = 0",
            "a: x / y = 1",
            "a: sqrt(x) = 1",
            "a: x < 3",
            "a: x <= 3",
            "a: x = y = z",
        ] {
            let mut c = Construction::new();
            let err = exec(&mut c, line).unwrap_err();
            assert!(
                err.contains("implicit curves")
                    || err.contains("inequalities")
                    || err.contains("only one `=`"),
                "{line}: {err}"
            );
        }
    }

    #[test]
    fn y_of_a_function_form_makes_a_function() {
        let c = load("a: y = sin(x)\nb: y = x^3\n").unwrap();
        assert_eq!(c.kind(c.id("a").unwrap()), Kind::Function);
        assert_eq!(c.kind(c.id("b").unwrap()), Kind::Function);
        assert!(c.function_value(c.id("b").unwrap(), 2.0).is_some());
        // `y = 2` is a line (degree 1), not a function
        let c = load("a: y = 2\n").unwrap();
        assert_eq!(c.kind(c.id("a").unwrap()), Kind::Line);
    }

    #[test]
    fn a_failed_equation_changes_nothing() {
        let mut c = Construction::new();
        exec(&mut c, "A = (0, 0)").unwrap();
        let n = c.len();
        for line in [
            "a: q = 1",
            "a: A = (1, 2)",
            "a: x + A = 1",
            "a: 2x + 3y = 6",
        ] {
            // `q` and `A` are not numbers, a point is not a number;
            // the last one takes the label `a`
            if line == "a: 2x + 3y = 6" {
                exec(&mut c, line).unwrap();
                break;
            }
            assert!(exec(&mut c, line).is_err(), "{line}");
        }
        assert_eq!(c.len(), n + 1);
        assert!(matches!(c.def(c.id("a").unwrap()), Def::Equation(_)));
    }

    #[test]
    fn the_label_of_an_equation_follows_the_rules_of_labels() {
        let mut c = Construction::new();
        exec(&mut c, "a: 2x + 3y = 6").unwrap();
        // the label is taken twice
        assert!(exec(&mut c, "a: x = 1")
            .unwrap_err()
            .contains("already exists"));
        // reserved names stay reserved
        assert!(exec(&mut c, "x: y = 1").unwrap_err().contains("reserved"));
        assert!(exec(&mut c, "sin: y = 1").unwrap_err().contains("reserved"));
        // an indexed label
        exec(&mut c, "b_{1}: y = 2x").unwrap();
        assert!(c.id("b_{1}").is_some());
    }

    #[test]
    fn a_colon_also_names_an_ordinary_definition() {
        assert!(load("P: (1, 2)\ns: Segment(P, Q)\nQ: (3, 4)\n").is_err());
        let c = load("P: (1, 2)\nQ: (3, 4)\ns: Segment(P, Q)\n").unwrap();
        assert_eq!(c.value(c.id("P").unwrap()), Value::Point { x: 1.0, y: 2.0 });
        assert_eq!(c.kind(c.id("s").unwrap()), Kind::Segment);
    }
}
