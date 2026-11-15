// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/javacc/org/geogebra/common/kernel/parser/Parser.jj
//   source/shared/common/src/main/java/org/geogebra/common/kernel/parser/FunctionParser.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/parser/function/ParserFunctionsFactory.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNode.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Recursive-descent parser with the precedence of GeoGebra's grammar:
//! sums, then products (`*` and implicit), then quotients (`/` binds tighter
//! than a product, so `a/2a` is `(a/2)·a`), unary signs, powers (right
//! associative, `-2^2 = -4`), factorials, then elements.

use super::lex::{tokens, Tok, Token};
use super::math::PI_180;
use super::names::{binary_builtin, builtin_name, unary_builtin};
use super::types::check;
use super::{Bin, Node, Ty, Un};
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

/// A parsed and type-checked expression.
pub(crate) struct Parsed {
    pub(crate) nodes: Vec<Node>,
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
    let mut p = Parser {
        c,
        src,
        toks: tokens(src)?,
        pos: 0,
        nodes: Vec::new(),
        nesting: 0,
        parents: Vec::new(),
        inputs: Vec::new(),
        uses_var: false,
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
    if depth[r] > MAX_DEPTH || cost[r] > MAX_COST {
        return Err("the expression is too large".into());
    }
    Ok(Parsed {
        root,
        depth: depth[r],
        cost: cost[r],
        vars: vars[r],
        nodes: p.nodes,
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
    nesting: u32,
    parents: Vec<Id>,
    inputs: Vec<Id>,
    uses_var: bool,
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
        (self.nodes.len() - 1) as u32
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

    /// sum = product (('+' | '-') product)*
    fn expression(&mut self) -> Result<u32, String> {
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
            Node::Bin(Bin::Mul, a, b) if depth < MAX_NESTING => {
                let special = self.multiply_special(b, right, depth + 1)?;
                Some(self.push(Node::Bin(Bin::Mul, a, special)))
            }
            _ => None,
        }
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
            Node::Num(v) => self.push(Node::Num(-v)),
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
                Some(&Tok::Square(n)) => {
                    self.pos += 1;
                    let f = self.push(Node::Num(f64::from(n)));
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
            let e = self.element()?;
            self.push(Node::Un(Un::Sqrt, e))
        } else {
            self.element()?
        };
        Ok(if self.eat(&Tok::Bang) {
            self.push(Node::Un(Un::Factorial, e))
        } else {
            e
        })
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
                let n = self.push(Node::Num(v));
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
                    None => first,
                })
            }
            Tok::Bar => {
                let e = self.expression()?;
                self.expect(&Tok::Bar, "|")?;
                Ok(self.push(Node::Un(Un::Abs, e)))
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
        let free = |s: &str| self.c.id(s).is_none();
        let constant = match text {
            "pi" | "Pi" | "\u{3c0}" => Some(Node::Num(PI)),
            "\u{212f}" => Some(Node::Num(E)),
            "e" if free("e") => Some(Node::Num(E)),
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
            // `a(b)` with a number `a` is a product: the bracket follows.
            return Ok(self.push(Node::Obj(id)));
        }
        let coordinate = match text {
            "x" => Some(Un::X),
            "y" => Some(Un::Y),
            _ => None,
        };
        if coordinate.is_some() || builtin_name(text) {
            if !call {
                return match unary_builtin(text) {
                    Some(op) => Ok(self.push(Node::FnName(op))),
                    None => Err(format!("`{text}` needs arguments")),
                };
            }
            let args = self.arguments()?;
            let node = match (coordinate.or_else(|| unary_builtin(text)), &args[..]) {
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
        if !matches!(kind, Kind::Number | Kind::Point | Kind::Vector) {
            return Err(format!("`{text}` is a {kind}, not a number or a point"));
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
        Ok(self.push(Node::Cmd(def)))
    }
}
