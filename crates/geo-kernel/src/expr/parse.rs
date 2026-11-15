// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/javacc/org/geogebra/common/kernel/parser/Parser.jj
//   source/shared/common/src/main/java/org/geogebra/common/kernel/parser/FunctionParser.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/parser/function/ParserFunctionsFactory.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNode.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MySpecialDouble.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdIf.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdSequence.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Recursive-descent parser with the precedence of GeoGebra's grammar,
//! loosest first: implications (`->`, left associative), `||`, `⊕`, `&&`,
//! comparisons (all at one level; a chain `1 < a < 3` is the conjunction
//! of its links, which share the middle operand), sums, then products
//! (`*` and implicit), then quotients (`/` binds tighter than a product,
//! so `a/2a` is `(a/2)·a`), unary signs, powers (right associative,
//! `-2^2 = -4`), factorials, square roots, negations (`!a^2` is
//! `(!a)^2`), then elements.

use super::decimal::Dec;
use super::eval;
use super::lex::{split_bars, tokens, Tok, Token};
use super::math::{self, PI_180};
use super::names::{binary_builtin, builtin_name, unary_builtin};
use super::types::check;
use super::{children, Bin, IfMode, ListOp, Lit, Node, Ty, Un, NONE};
use crate::algo::{intersect, vector};
use crate::kernel::{Construction, Id, Kind};
use std::f64::consts::{E, PI};

/// Brackets, bars and calls nested deeper than this are rejected, so that
/// parsing never overflows the stack.
const MAX_NESTING: u32 = 64;
/// Expression trees deeper than this are rejected (evaluation recurses).
pub(crate) const MAX_DEPTH: u32 = 256;
/// Upper bound on the nodes one evaluation visits, functions expanded.
pub(crate) const MAX_COST: u64 = 1 << 20;

/// `MySpecialDouble.getConstantDecimal` of `π` and `ℯ`.
const PI_DECIMAL: &str = "3.1415926535897932385";
const E_DECIMAL: &str = "2.7182818284590452353";

/// A parsed and type-checked expression.
pub(crate) struct Parsed {
    pub(crate) nodes: Vec<Node>,
    /// What GeoGebra's number object of each constant is (see [`Lit`]).
    pub(crate) lits: Vec<Lit>,
    pub(crate) tys: Vec<Ty>,
    pub(crate) root: u32,
    /// Objects the expression reads, directly or through a command.
    pub(crate) parents: Vec<Id>,
    /// Objects named in the expression.
    pub(crate) inputs: Vec<Id>,
    /// Whether the variable `x` occurs.
    pub(crate) uses_var: bool,
    pub(crate) depth: u32,
    pub(crate) cost: u64,
    /// How many times one evaluation reads `x`.
    pub(crate) vars: u64,
}

pub(crate) fn parse(c: &Construction, src: &str) -> Result<Parsed, String> {
    let preprocessed = split_bars(src);
    let src: &str = &preprocessed;
    let mut p = Parser {
        c,
        src,
        toks: tokens(src)?,
        pos: 0,
        nodes: Vec::new(),
        lits: Vec::new(),
        bare: Vec::new(),
        brackets: Vec::new(),
        nesting: 0,
        parents: Vec::new(),
        inputs: Vec::new(),
        uses_var: false,
        locals: Vec::new(),
    };
    if p.toks.is_empty() {
        return Err("empty expression".into());
    }
    let root = p.expression()?;
    if let Some(t) = p.toks.get(p.pos) {
        return Err(format!("unexpected `{}`", &src[t.start..t.end]));
    }
    let (tys, depth, cost, vars) = check(c, &p.nodes, root)?;
    let r = root as usize;
    if depth[r] > MAX_DEPTH {
        return Err("the expression is nested too deeply".into());
    }
    if cost[r] > MAX_COST {
        return Err("the expression is too large to evaluate".into());
    }
    Ok(Parsed {
        root,
        depth: depth[r],
        cost: cost[r],
        vars: vars[r],
        nodes: p.nodes,
        lits: p.lits,
        tys,
        parents: p.parents,
        inputs: p.inputs,
        uses_var: p.uses_var,
    })
}

struct Parser<'a> {
    c: &'a Construction,
    src: &'a str,
    toks: Vec<Token>,
    pos: usize,
    nodes: Vec<Node>,
    lits: Vec<Lit>,
    /// Literals written as such, which `unaryMinus` negates in place
    /// (`-0.1`); a bracketed one (`-(0.1)`) or a letter constant (`-π`) is
    /// multiplied by -1 instead, which gives a plain number.
    bare: Vec<bool>,
    /// Whether each node was written in brackets (`hasBrackets`).
    brackets: Vec<bool>,
    nesting: u32,
    parents: Vec<Id>,
    inputs: Vec<Id>,
    uses_var: bool,
    /// The variables of the `Sequence`s being parsed, outermost first.
    locals: Vec<String>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.tok)
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == Some(tok) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, tok: &Tok, what: &str) -> Result<(), String> {
        if self.eat(tok) {
            Ok(())
        } else {
            Err(format!("expected `{what}`"))
        }
    }

    fn push(&mut self, node: Node) -> u32 {
        self.nodes.push(node);
        self.lits.push(Lit::default());
        self.bare.push(false);
        self.brackets.push(false);
        (self.nodes.len() - 1) as u32
    }

    /// A constant that is a `MySpecialDouble` in GeoGebra, with its decimal.
    fn push_special(&mut self, v: f64, dec: Option<Dec>) -> u32 {
        let i = self.push(Node::Num(v));
        self.lits[i as usize] = Lit {
            special: true,
            dec: dec.filter(|_| v.is_finite()),
        };
        i
    }

    fn node(&self, i: u32) -> Node {
        self.nodes[i as usize]
    }

    fn read(&mut self, id: Id, input: bool) {
        if !self.parents.contains(&id) {
            self.parents.push(id);
        }
        if input && !self.inputs.contains(&id) {
            self.inputs.push(id);
        }
    }

    /// `expressionnode`: implications of disjunctions, left associative.
    fn expression(&mut self) -> Result<u32, String> {
        let mut left = self.disjunction()?;
        while self.eat(&Tok::Implies) {
            let right = self.disjunction()?;
            left = self.push(Node::Bin(Bin::Implies, left, right));
        }
        Ok(left)
    }

    /// `ORterm`, `XORterm` and `ANDterm`: each a left-associative chain of
    /// the next.
    fn disjunction(&mut self) -> Result<u32, String> {
        self.chain(&Tok::Or, Bin::Or, Self::exclusive)
    }

    fn exclusive(&mut self) -> Result<u32, String> {
        self.chain(&Tok::Xor, Bin::Xor, Self::conjunction)
    }

    fn conjunction(&mut self) -> Result<u32, String> {
        self.chain(&Tok::And, Bin::And, Self::comparison)
    }

    fn chain(
        &mut self,
        tok: &Tok,
        op: Bin,
        next: fn(&mut Self) -> Result<u32, String>,
    ) -> Result<u32, String> {
        let mut left = next(self)?;
        while self.eat(tok) {
            let right = next(self)?;
            left = self.push(Node::Bin(op, left, right));
        }
        Ok(left)
    }

    /// `COMPAREterm`: sums joined by comparisons. A chain compares each
    /// sum with the next, and the comparisons are joined left to right by
    /// `AND_INTERVAL`, a conjunction.
    fn comparison(&mut self) -> Result<u32, String> {
        let mut left = self.sum()?;
        let mut joined: Option<u32> = None;
        loop {
            let op = match self.peek() {
                Some(Tok::Less) => Bin::Less,
                Some(Tok::Greater) => Bin::Greater,
                Some(Tok::LessEqual) => Bin::LessEqual,
                Some(Tok::GreaterEqual) => Bin::GreaterEqual,
                Some(Tok::Equal) => Bin::Equal,
                Some(Tok::NotEqual) => Bin::NotEqual,
                _ => break,
            };
            self.pos += 1;
            let right = self.sum()?;
            let link = self.push(Node::Bin(op, left, right));
            joined = Some(match joined {
                None => link,
                Some(j) => self.push(Node::Bin(Bin::And, j, link)),
            });
            left = right;
        }
        Ok(joined.unwrap_or(left))
    }

    /// sum = product (('+' | '-') product)*
    fn sum(&mut self) -> Result<u32, String> {
        let mut left = self.product()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Plus) => Bin::Add,
                Some(Tok::Minus) => Bin::Sub,
                _ => return Ok(left),
            };
            self.pos += 1;
            let right = self.product()?;
            left = self.push(Node::Bin(op, left, right));
        }
    }

    /// `multterm`: quotients joined by `*` or by juxtaposition.
    fn product(&mut self) -> Result<u32, String> {
        let mut left = self.quotient()?;
        loop {
            if self.eat(&Tok::Star) {
                let right = self.quotient()?;
                left = self.push(Node::Bin(Bin::Mul, left, right));
            } else if self.starts_factor() {
                let right = self.power_quotient()?;
                left = self.implicit_product(left, right);
            } else {
                return Ok(left);
            }
        }
    }

    /// Whether the next token can start an implicit factor; never `|`,
    /// which could also close an absolute value.
    fn starts_factor(&self) -> bool {
        matches!(
            self.peek(),
            Some(
                Tok::Num(_)
                    | Tok::Name { .. }
                    | Tok::LParen
                    | Tok::Degree
                    | Tok::Sqrt
                    | Tok::Infinity
                    | Tok::Undefined
                    | Tok::Not
                    | Tok::LBrace
            )
        )
    }

    /// `divterm`: signed powers joined by `/`.
    fn quotient(&mut self) -> Result<u32, String> {
        let mut left = self.unary()?;
        while self.eat(&Tok::Slash) {
            let right = self.unary()?;
            left = self.push(Node::Bin(Bin::Div, left, right));
        }
        Ok(left)
    }

    /// `powerdivterm`: like a quotient, but the first factor has no sign.
    fn power_quotient(&mut self) -> Result<u32, String> {
        let mut left = self.power()?;
        while self.eat(&Tok::Slash) {
            let right = self.unary()?;
            left = self.push(Node::Bin(Bin::Div, left, right));
        }
        Ok(left)
    }

    /// The parser's multiplication by juxtaposition: `sin a` applies `sin`,
    /// and a degree sign joins the right factor (`Parser.multterm`).
    fn implicit_product(&mut self, left: u32, right: u32) -> u32 {
        if let Some(special) = self.multiply_special(left, right, 0) {
            return special;
        }
        if self.is_degree(right) {
            if let Node::Bin(Bin::Mul, a, b) = self.node(left) {
                let inner = self.push(Node::Bin(Bin::Mul, b, right));
                return self.push(Node::Bin(Bin::Mul, a, inner));
            }
        }
        self.push(Node::Bin(Bin::Mul, left, right))
    }

    /// `FunctionParser.multiplySpecial`: a bare one-argument function name
    /// times a factor is that function applied to the factor; `a sin b`
    /// is `a sin(b)`.
    fn multiply_special(&mut self, left: u32, right: u32, depth: u32) -> Option<u32> {
        match self.node(left) {
            Node::FnName(op) => Some(self.push(Node::Un(op, right))),
            // `sin^2 x`, `sin²(x)`, `sin^-1 x`
            Node::Bin(Bin::Pow, f, exponent) => match self.node(f) {
                Node::FnName(op) => Some(self.inverse_or_power(op, right, exponent)),
                _ => None,
            },
            Node::Bin(Bin::Mul, a, b) if depth < MAX_NESTING => {
                let special = self.multiply_special(b, right, depth + 1)?;
                Some(self.push(Node::Bin(Bin::Mul, a, special)))
            }
            _ => None,
        }
    }

    /// `FunctionParser.inverseOrPower`: `op^e x` is `op(x)^e`, and with a
    /// constant exponent -1 the inverse function (`minusFirstPower`). In
    /// `sin^2(a)^3` the power belongs outside: `(sin(a)^2)^3`.
    fn inverse_or_power(&mut self, op: Un, arg: u32, exponent: u32) -> u32 {
        if let Node::Bin(Bin::Pow, base, outer) = self.node(arg) {
            if !self.brackets[arg as usize] && self.brackets[base as usize] {
                let inner = self.inverse_or_power(op, base, exponent);
                return self.push(Node::Bin(Bin::Pow, inner, outer));
            }
        }
        let minus_one = self
            .constant_value(exponent, 0)
            .is_some_and(|v| math::is_equal(v, -1.0));
        if minus_one {
            return self.minus_first_power(op, arg);
        }
        let f = self.push(Node::Un(op, arg));
        self.push(Node::Bin(Bin::Pow, f, exponent))
    }

    /// `FunctionParser.minusFirstPower`: `sin^-1 x` is the inverse
    /// function, in degrees for the trigonometric ones; `sec^-1 x` is
    /// `acos(1/x)`; any other function is raised to the power -1.
    fn minus_first_power(&mut self, op: Un, arg: u32) -> u32 {
        let inverse = match op {
            Un::Sin => Some(Un::AsinD),
            Un::Cos => Some(Un::AcosD),
            Un::Tan => Some(Un::AtanD),
            Un::Sinh => Some(Un::Asinh),
            Un::Cosh => Some(Un::Acosh),
            Un::Tanh => Some(Un::Atanh),
            _ => None,
        };
        if let Some(inverse) = inverse {
            return self.push(Node::Un(inverse, arg));
        }
        let of_reciprocal = match op {
            Un::Sec => Some(Un::Acos),
            Un::Csc => Some(Un::Asin),
            Un::Sech => Some(Un::Acosh),
            Un::Csch => Some(Un::Asinh),
            Un::Coth => Some(Un::Atanh),
            _ => None,
        };
        if let Some(inverse) = of_reciprocal {
            let one = self.push(Node::Num(1.0));
            let r = self.push(Node::Bin(Bin::Div, one, arg));
            return self.push(Node::Un(inverse, r));
        }
        if op == Un::Cot {
            // `π/2 - atan(x)`
            let pi = self.push_special(PI, Dec::parse(PI_DECIMAL));
            let two = self.push(Node::Num(2.0));
            let half = self.push(Node::Bin(Bin::Div, pi, two));
            let atan = self.push(Node::Un(Un::Atan, arg));
            return self.push(Node::Bin(Bin::Sub, half, atan));
        }
        let f = self.push(Node::Un(op, arg));
        let m = self.push(Node::Num(-1.0));
        self.push(Node::Bin(Bin::Pow, f, m))
    }

    /// The value of a subtree that reads no object and no `x`
    /// (`isConstant` and `evaluateDouble`); `None` otherwise, and for trees
    /// nested too deep to look at.
    fn constant_value(&self, i: u32, depth: u32) -> Option<f64> {
        if depth > MAX_NESTING {
            return None;
        }
        let value = |j: u32| self.constant_value(j, depth + 1);
        Some(match self.node(i) {
            Node::Num(v) => v,
            Node::MinusOne => -1.0,
            Node::Degree => PI_180,
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

    /// `ExpressionNode.isConstantDouble(f, Kernel.PI_180)`.
    fn is_degree(&self, i: u32) -> bool {
        match self.node(i) {
            Node::Degree => true,
            Node::Num(v) => v == PI_180,
            _ => false,
        }
    }

    /// `ExpressionNode.unaryMinus`: a constant is negated, `x°` becomes
    /// `(-x)°`, anything else is multiplied by -1.
    fn unary_minus(&mut self, e: u32, depth: u32) -> u32 {
        match self.node(e) {
            Node::Num(v) => {
                let lit = &self.lits[e as usize];
                if lit.special && self.bare[e as usize] {
                    let dec = lit.dec.as_ref().map(Dec::negated);
                    let n = self.push_special(-v, dec);
                    self.bare[n as usize] = true;
                    n
                } else {
                    self.push(Node::Num(-v))
                }
            }
            Node::Bin(Bin::Mul, a, b)
                if depth < MAX_NESTING && matches!(self.node(b), Node::Degree) =>
            {
                let a = self.unary_minus(a, depth + 1);
                self.push(Node::Bin(Bin::Mul, a, b))
            }
            _ => {
                let m = self.push(Node::MinusOne);
                self.push(Node::Bin(Bin::Mul, m, e))
            }
        }
    }

    /// `enunary`: `-e`, `--e`, `+e` or `e`.
    fn unary(&mut self) -> Result<u32, String> {
        let mut minus = 0u32;
        while self.eat(&Tok::Minus) {
            minus += 1;
        }
        if minus == 0 && self.eat(&Tok::Plus) {
            return self.power();
        }
        let e = self.power()?;
        Ok(if minus % 2 == 1 {
            self.unary_minus(e, 0)
        } else {
            e
        })
    }

    /// `enpower`: `a^b^c` is `a^(b^c)`; an exponent may carry one sign.
    fn power(&mut self) -> Result<u32, String> {
        let base = self.factorial()?;
        let mut exponents = Vec::new();
        loop {
            match self.peek() {
                Some(Tok::Caret) => {
                    self.pos += 1;
                    let minus = self.eat(&Tok::Minus);
                    if !minus {
                        self.eat(&Tok::Plus);
                    }
                    let f = self.factorial()?;
                    exponents.push(if minus { self.unary_minus(f, 0) } else { f });
                }
                Some(&Tok::Power(n)) => {
                    // `convertIndexToNumber`
                    self.pos += 1;
                    let f = self.push_special(n, Dec::from_f64(n));
                    exponents.push(f);
                }
                _ => break,
            }
        }
        let Some(mut exponent) = exponents.pop() else {
            return Ok(base);
        };
        while let Some(previous) = exponents.pop() {
            exponent = self.push(Node::Bin(Bin::Pow, previous, exponent));
        }
        Ok(self.push(Node::Bin(Bin::Pow, base, exponent)))
    }

    /// `enfactorial`: one optional `!`.
    fn factorial(&mut self) -> Result<u32, String> {
        let e = if self.eat(&Tok::Sqrt) {
            let e = self.negation()?;
            self.push(Node::Un(Un::Sqrt, e))
        } else {
            self.negation()?
        };
        Ok(if self.eat(&Tok::Bang) {
            self.push(Node::Un(Un::Factorial, e))
        } else {
            e
        })
    }

    /// `NOTterm`: `¬` or a `!` in front of an element negates it; a `!`
    /// after an operand is a factorial.
    fn negation(&mut self) -> Result<u32, String> {
        if self.eat(&Tok::Not) || self.eat(&Tok::Bang) {
            let e = self.element()?;
            return Ok(self.push(Node::Un(Un::Not, e)));
        }
        self.element()
    }

    fn element(&mut self) -> Result<u32, String> {
        self.nesting += 1;
        if self.nesting > MAX_NESTING {
            return Err("the expression is nested too deeply".into());
        }
        let e = self.element_inner();
        self.nesting -= 1;
        e
    }

    fn element_inner(&mut self) -> Result<u32, String> {
        let Some(token) = self.toks.get(self.pos).cloned() else {
            return Err("unexpected end of the expression".into());
        };
        self.pos += 1;
        match token.tok {
            Tok::Num(v) => {
                // `MySpecialDouble.toDecimal`: the literal as written
                let dec = Dec::parse(&self.src[token.start..token.end]);
                // `50%` is a literal (`Parser.parseDouble`)
                let (v, dec) = if self.eat(&Tok::Percent) {
                    let hundredth = Dec::parse("0.01");
                    let dec = dec.zip(hundredth).and_then(|(d, h)| d.mul(&h));
                    (v * 0.01, dec)
                } else {
                    (v, dec)
                };
                let n = self.push_special(v, dec);
                self.bare[n as usize] = true;
                if self.eat(&Tok::Degree) {
                    // `30°` is `30 * °` (`Parser.myDouble`)
                    let d = self.push(Node::Degree);
                    return Ok(self.push(Node::Bin(Bin::Mul, n, d)));
                }
                Ok(n)
            }
            Tok::Degree => Ok(self.push(Node::Degree)),
            Tok::Infinity => Ok(self.push(Node::Num(f64::INFINITY))),
            Tok::Undefined => Ok(self.push(Node::Num(f64::NAN))),
            Tok::LParen => {
                let first = self.expression()?;
                let node = if self.eat(&Tok::Comma) {
                    let second = self.expression()?;
                    Some(Node::Tuple(first, second))
                } else if self.eat(&Tok::Semicolon) {
                    let second = self.expression()?;
                    Some(Node::Polar(first, second))
                } else {
                    None
                };
                self.expect(&Tok::RParen, ")")?;
                Ok(match node {
                    Some(n) => self.push(n),
                    None => {
                        // a bracketed literal is no longer negated in place
                        self.bare[first as usize] = false;
                        self.brackets[first as usize] = true;
                        first
                    }
                })
            }
            Tok::Bar => {
                let e = self.expression()?;
                self.expect(&Tok::Bar, "|")?;
                Ok(self.push(Node::Un(Un::Abs, e)))
            }
            Tok::LBrace => {
                // `{a, b, ...}`: the items go one after the other
                let mut elements = Vec::new();
                if !self.eat(&Tok::RBrace) {
                    elements.push(self.expression()?);
                    while self.eat(&Tok::Comma) {
                        elements.push(self.expression()?);
                    }
                    self.expect(&Tok::RBrace, "}")?;
                }
                let first = self.nodes.len() as u32;
                for &e in &elements {
                    self.push(Node::Item(e));
                }
                Ok(self.push(Node::List(first, elements.len() as u32)))
            }
            Tok::Name { text, call } => self.name(&text, call),
            _ => Err(format!(
                "unexpected `{}`",
                &self.src[token.start..token.end]
            )),
        }
    }

    /// A name: a constant, the variable `x`, an object, a built-in function
    /// or a command (`Parser.functionOrCommand`, `FunctionParser`).
    fn name(&mut self, text: &str, call: bool) -> Result<u32, String> {
        // the variable of a `Sequence` hides an object of the same name
        if let Some(depth) = self.locals.iter().rposition(|l| l == text) {
            let local = self.push(Node::Local(depth as u32));
            if !call {
                return Ok(local);
            }
            let arg = self.element()?;
            return Ok(self.push(Node::Bin(Bin::Mul, local, arg)));
        }
        let free = |s: &str| self.c.id(s).is_none();
        let letter = match text {
            "pi" | "Pi" | "\u{3c0}" => Some((PI, PI_DECIMAL)),
            "\u{212f}" => Some((E, E_DECIMAL)),
            "e" if free("e") => Some((E, E_DECIMAL)),
            _ => None,
        };
        if let Some((v, dec)) = letter {
            return Ok(self.push_special(v, Dec::parse(dec)));
        }
        let constant = match text {
            // `TRUE` and `FALSE` tokens, never labels
            "true" | "True" => Some(Node::Bool(true)),
            "false" | "False" => Some(Node::Bool(false)),
            "deg" if free("deg") => Some(Node::Degree),
            "Infinity" | "infinity" | "inf" => Some(Node::Num(f64::INFINITY)),
            "Undefined" | "NaN" => Some(Node::Num(f64::NAN)),
            "x" if !call => {
                self.uses_var = true;
                Some(Node::Var)
            }
            _ => None,
        };
        if let Some(node) = constant {
            return Ok(self.push(node));
        }
        let coordinate = match text {
            "x" => Some(Un::X),
            "y" => Some(Un::Y),
            _ => None,
        };
        if let (Some(op), true) = (coordinate, call) {
            // `x(A)`: never an object, `x` and `y` are not labels
            return match self.arguments()?[..] {
                [a] => Ok(self.push(Node::Un(op, a))),
                _ => Err(format!("`{text}` takes one argument")),
            };
        }
        if let Some(id) = self.c.id(text) {
            self.read(id, true);
            if self.c.kind(id) == Kind::Function {
                if !call {
                    return Err(format!("the function `{text}` needs an argument"));
                }
                return match self.arguments()?[..] {
                    [a] => Ok(self.push(Node::Apply(id, a))),
                    _ => Err(format!("the function `{text}` takes one argument")),
                };
            }
            let obj = self.push(Node::Obj(id));
            if !call {
                return Ok(obj);
            }
            if self.c.kind(id) == Kind::List {
                // `l(n)`: an element (`ELEMENT_OF`)
                return match self.arguments()?[..] {
                    [n] => Ok(self.push(Node::ListOp(ListOp::Index, obj, n))),
                    _ => Err(format!("`{text}(...)` takes one index")),
                };
            }
            // `a(b)` with a number `a` is the product `a·b`, one element
            // (`MULTIPLY_OR_FUNCTION`): `a(b)/c` is `(a·b)/c`.
            let arg = self.element()?;
            return Ok(self.push(Node::Bin(Bin::Mul, obj, arg)));
        }
        if call && text == "If" {
            return self.if_command();
        }
        if call {
            if let Some(node) = self.list_command(text)? {
                return Ok(node);
            }
        }
        if coordinate.is_some() || builtin_name(text) {
            if !call {
                return match unary_builtin(text) {
                    Some(op) => Ok(self.push(Node::FnName(op))),
                    None => Err(format!("`{text}` needs arguments")),
                };
            }
            let args = self.arguments()?;
            let node = match (unary_builtin(text), &args[..]) {
                (Some(op), &[a]) => Node::Un(op, a),
                _ => match (binary_builtin(text), &args[..]) {
                    (Some(op), &[a, b]) => Node::Bin(op, a, b),
                    _ => return Err(format!("`{text}` does not take {} argument(s)", args.len())),
                },
            };
            return Ok(self.push(node));
        }
        if call {
            return self.command();
        }
        Err(format!("unknown object `{text}`"))
    }

    /// `If(c, a)`, `If(c, a, b)` and `If(c1, a1, c2, a2, ..., [else])`
    /// (`CmdIf`). With `x` in a condition it is the operation of a
    /// function (`IF`, `IF_ELSE`, `IF_LIST`); otherwise the command
    /// `AlgoIf`.
    fn if_command(&mut self) -> Result<u32, String> {
        let args = self.arguments()?;
        if args.len() < 2 {
            return Err("`If` needs a condition and a value".into());
        }
        let pairs = args.len() / 2;
        let conditions_read_x = (0..pairs).any(|k| self.reads(args[2 * k], true));
        let mode = match (conditions_read_x, args.len()) {
            (false, _) => IfMode::Algo,
            (true, 3) if !self.reads(args[0], false) => IfMode::Else,
            (true, _) => IfMode::List,
        };
        let mut rest = if args.len() % 2 == 1 {
            args[args.len() - 1]
        } else {
            NONE
        };
        for k in (0..pairs).rev() {
            rest = self.push(Node::If(mode, args[2 * k], args[2 * k + 1], rest));
        }
        Ok(rest)
    }

    /// Whether the subtree at `i` reads `x` (`variable`) or an object.
    fn reads(&self, i: u32, variable: bool) -> bool {
        let mut stack = vec![i];
        while let Some(j) = stack.pop() {
            let node = self.node(j);
            let found = if variable {
                matches!(node, Node::Var)
            } else {
                matches!(node, Node::Obj(_) | Node::Apply(..) | Node::Cmd(..))
            };
            if found {
                return true;
            }
            stack.extend(children(&node));
        }
        false
    }

    /// The list commands: `Sequence` and `Element`, and `Length` and `Sum`
    /// of a list (of anything else they are the commands of the script
    /// parser). `None` when `text` is none of these.
    fn list_command(&mut self, text: &str) -> Result<Option<u32>, String> {
        match text {
            "Sequence" => self.sequence().map(Some),
            "Element" => match self.arguments()?[..] {
                [l, n] => Ok(Some(self.push(Node::ListOp(ListOp::Element, l, n)))),
                _ => Err("`Element` takes a list and one index".into()),
            },
            "Length" | "Sum" => {
                let saved = (
                    self.pos,
                    self.parents.len(),
                    self.inputs.len(),
                    self.uses_var,
                );
                let op = if text == "Length" {
                    ListOp::Length
                } else {
                    ListOp::Sum
                };
                let node = match self.arguments() {
                    Ok(args) if self.is_list(args[0]) => match (op, &args[..]) {
                        (ListOp::Length, &[l]) => Some(Node::ListOp(op, l, NONE)),
                        (ListOp::Sum, &[l]) => Some(Node::ListOp(op, l, NONE)),
                        (ListOp::Sum, &[l, n]) => Some(Node::ListOp(op, l, n)),
                        _ => {
                            return Err(format!(
                                "`{text}` of a list does not take {} arguments",
                                args.len()
                            ))
                        }
                    },
                    _ => None,
                };
                match node {
                    Some(node) => Ok(Some(self.push(node))),
                    None => {
                        // not a list: the script parser's command
                        self.pos = saved.0;
                        self.parents.truncate(saved.1);
                        self.inputs.truncate(saved.2);
                        self.uses_var = saved.3;
                        Ok(None)
                    }
                }
            }
            _ => Ok(None),
        }
    }

    /// Whether the subtree at `i` is a list.
    fn is_list(&self, i: u32) -> bool {
        matches!(check(self.c, &self.nodes, i), Ok((tys, ..)) if matches!(tys[i as usize], Ty::List(_)))
    }

    /// `Sequence(n)`, `Sequence(a, b)`, `Sequence(a, b, step)` and
    /// `Sequence(expression, k, from, to[, step])` (`CmdSequence`): with
    /// four or five arguments the second is the variable, which the
    /// expression reads and which hides an object of the same name.
    fn sequence(&mut self) -> Result<u32, String> {
        let variable = self.sequence_variable()?;
        let Some(variable) = variable else {
            return match self.arguments()?[..] {
                [n] => Ok(self.push(Node::Range(NONE, n, NONE))),
                [a, b] => Ok(self.push(Node::Range(a, b, NONE))),
                [a, b, s] => Ok(self.push(Node::Range(a, b, s))),
                _ => Err("`Sequence` takes one to five arguments".into()),
            };
        };
        self.expect(&Tok::LParen, "(")?;
        let depth = self.locals.len() as u32;
        self.locals.push(variable);
        let expression = self.expression();
        self.locals.pop();
        let expression = expression?;
        self.expect(&Tok::Comma, ",")?;
        self.pos += 1;
        self.expect(&Tok::Comma, ",")?;
        let from = self.expression()?;
        self.expect(&Tok::Comma, ",")?;
        let to = self.expression()?;
        let step = if self.eat(&Tok::Comma) {
            self.expression()?
        } else {
            NONE
        };
        self.expect(&Tok::RParen, ")")?;
        Ok(self.push(Node::Seq(expression, from, to, step, depth)))
    }

    /// The variable of a `Sequence` with four or five arguments, found
    /// before its expression is parsed; `None` with fewer arguments.
    fn sequence_variable(&self) -> Result<Option<String>, String> {
        let mut depth = 0usize;
        let mut commas = Vec::new();
        for (j, t) in self.toks.iter().enumerate().skip(self.pos) {
            match t.tok {
                Tok::LParen | Tok::LBrace => depth += 1,
                Tok::RParen | Tok::RBrace => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                Tok::Comma if depth == 1 => commas.push(j),
                _ => {}
            }
        }
        if commas.len() < 3 {
            return Ok(None);
        }
        if commas.len() > 4 {
            return Err("`Sequence` takes one to five arguments".into());
        }
        match &self.toks[commas[0] + 1].tok {
            Tok::Name { text, call: false } if commas[1] == commas[0] + 2 => Ok(Some(text.clone())),
            _ => Err("the second argument of `Sequence` must be a variable".into()),
        }
    }

    /// `( expression, expression, ... )` after a function name.
    fn arguments(&mut self) -> Result<Vec<u32>, String> {
        self.expect(&Tok::LParen, "(")?;
        let mut args = vec![self.expression()?];
        while self.eat(&Tok::Comma) {
            args.push(self.expression()?);
        }
        self.expect(&Tok::RParen, ")")?;
        Ok(args)
    }

    /// A command call such as `Distance(A, B)`, read by the script parser.
    /// Its result is computed inside the expression: no helper object is
    /// created.
    fn command(&mut self) -> Result<u32, String> {
        let name = &self.toks[self.pos - 1];
        let start = name.start;
        let mut depth = 0usize;
        let mut end = None;
        for (j, t) in self.toks.iter().enumerate().skip(self.pos) {
            match t.tok {
                Tok::LParen => depth += 1,
                Tok::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(j);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = end.ok_or("missing closing `)`")?;
        let text = &self.src[start..self.toks[end].end];
        let nested = self.toks[self.pos..end]
            .iter()
            .any(|t| matches!(t.tok, Tok::Name { call: true, .. }));
        if nested {
            // its result would need a helper object
            return Err(format!(
                "`{text}`: a command inside a command is not supported"
            ));
        }
        self.pos = end + 1;
        let def = crate::script::parse_def(self.c, text)?;
        let kind = def.kind();
        if !matches!(
            kind,
            Kind::Number | Kind::Point | Kind::Vector | Kind::Boolean
        ) {
            return Err(format!(
                "`{text}` is a {kind}, not a number, a point or a boolean"
            ));
        }
        self.c.check(&def).map_err(|e| e.to_string())?;
        let def = intersect::prepare(self.c, def);
        for &p in def.parents().as_slice() {
            self.read(p, false);
        }
        for p in intersect::hidden_parents(self.c, &def) {
            self.read(p, false);
        }
        for p in vector::hidden_parents(self.c, &def) {
            self.read(p, false);
        }
        let hint = intersect::hint(self.c, &def);
        Ok(self.push(Node::Cmd(def, hint)))
    }
}
