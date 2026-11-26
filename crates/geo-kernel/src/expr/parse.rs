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

mod cas_input;

use super::commands;
use super::decimal::Dec;
use super::eval;
use super::lex::{split_bars, tokens, Tok, Token};
use super::math::{self, PI_180};
use super::names::{binary_builtin, builtin_name, unary_builtin};
use super::types::check;
use super::{Bin, Elem, ListOp, Lit, Node, Ty, Un, NONE};
use crate::algo::{intersect, vector};
use crate::kernel::{Construction, Id, Kind};
use std::f64::consts::{E, PI};
use std::sync::Arc;

/// Brackets, bars and calls nested deeper than this are rejected, so that
/// parsing never overflows the stack.
pub(crate) const MAX_NESTING: u32 = 64;
/// Expression trees deeper than this are rejected (evaluation recurses).
pub(crate) const MAX_DEPTH: u32 = 256;
/// Upper bound on the nodes one evaluation visits, functions expanded.
pub(crate) const MAX_COST: u64 = 1 << 20;

/// `MySpecialDouble.getConstantDecimal` of `π` and `ℯ`.
const PI_DECIMAL: &str = "3.1415926535897932385";
const E_DECIMAL: &str = "2.7182818284590452353";

/// Speculative command forms share an overall work limit even when their
/// provisional range reads are discarded.
struct CellBudget {
    remaining: Option<usize>,
    visits: Option<usize>,
}

const MAX_CELL_VISITS: usize = crate::spreadsheet::MAX_CELLS_PER_OPERATION * 16;

impl Default for CellBudget {
    fn default() -> Self {
        Self {
            remaining: Some(crate::spreadsheet::MAX_CELLS_PER_OPERATION),
            visits: Some(MAX_CELL_VISITS),
        }
    }
}

impl CellBudget {
    fn charge(&mut self, count: usize) -> Result<(), String> {
        self.remaining = self.remaining.and_then(|budget| budget.checked_sub(count));
        self.charge_visits(count)?;
        if self.exhausted() {
            return Err(crate::spreadsheet::LIMIT.into());
        }
        Ok(())
    }

    fn charge_visits(&mut self, count: usize) -> Result<(), String> {
        self.visits = self.visits.and_then(|budget| budget.checked_sub(count));
        self.visits
            .map(|_| ())
            .ok_or_else(|| crate::spreadsheet::LIMIT.into())
    }

    fn exhausted(&self) -> bool {
        self.remaining.is_none() || self.visits.is_none()
    }

    fn reset(&mut self, remaining: Option<usize>) {
        // Do not refund work, or mask an error, when abandoning a form.
        self.remaining = self.remaining.and(remaining);
    }
}

/// A parsed and type-checked expression.
#[derive(Clone)]
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
    /// Whether the variable `y` of an equation occurs (see [`equation`]).
    pub(crate) uses_y: bool,
    pub(crate) depth: u32,
    pub(crate) cost: u64,
    /// How many times one evaluation reads `x`.
    pub(crate) vars: u64,
}

pub(crate) fn parse(c: &Construction, src: &str) -> Result<Parsed, String> {
    parse_in(c, src, Mode::Expression)
}

/// Parses the expression of an inline geometry command argument.
pub(super) fn parse_argument(c: &Construction, src: &str) -> Result<Parsed, String> {
    parse_in(c, src, Mode::Argument)
}

pub(super) fn parse_parameter(c: &Construction, src: &str, name: &str) -> Result<Parsed, String> {
    parse_in(c, src, Mode::Parameter(name))
}

pub(super) fn parse_system(
    c: &Construction,
    src: &str,
    parameters: &[&str],
) -> Result<Parsed, String> {
    parse_in(c, src, Mode::System(parameters))
}

/// [`parse`] with the variables an equation may read: `x` and `y` are
/// GeoGebra's function variables, not objects (`Equation.initEquation`
/// replaces the `x`, `y` and `z` nodes).
pub(crate) fn parse_equation(c: &Construction, src: &str) -> Result<Parsed, String> {
    parse_in(c, src, Mode::Equation)
}

/// [`parse_equation`] for the left side of an equation: exactly as
/// `Equation.initEquation` resolves a lone variable on the left as a whole,
/// so `xy = 1` is no equation in `x·y` while `2xy = 1` is one.
pub(crate) fn parse_equation_left(c: &Construction, src: &str) -> Result<Parsed, String> {
    parse_in(c, src, Mode::EquationLeft)
}

pub(super) fn parse_scoped_equation(
    c: &Construction,
    src: &str,
    parameters: &[&str],
) -> Result<Parsed, String> {
    parse_in(c, src, Mode::ScopedEquation(parameters))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode<'a> {
    Expression,
    Argument,
    Equation,
    EquationLeft,
    Parameter(&'a str),
    System(&'a [&'a str]),
    ScopedEquation(&'a [&'a str]),
}

fn parse_in(c: &Construction, src: &str, mode: Mode<'_>) -> Result<Parsed, String> {
    parse_in_with_work(c, src, mode, &mut Some(MAX_CELL_VISITS))
}

pub(super) fn parse_spreadsheet_coordinate(
    c: &Construction,
    src: &str,
    budget: &mut usize,
) -> Result<Option<Parsed>, String> {
    let mut work = Some(*budget);
    let parsed = parse_in_with_work(c, src, Mode::Argument, &mut work);
    *budget = work.ok_or(crate::spreadsheet::LIMIT)?;
    match parsed {
        Err(error) if error == crate::spreadsheet::LIMIT => Err(error),
        result => Ok(result.ok()),
    }
}

fn parse_in_with_work(
    c: &Construction,
    src: &str,
    mode: Mode<'_>,
    work: &mut Option<usize>,
) -> Result<Parsed, String> {
    let equation = matches!(
        mode,
        Mode::Equation | Mode::EquationLeft | Mode::ScopedEquation(_)
    );
    let preprocessed = split_bars(src);
    let src: &str = &preprocessed;
    let toks = tokens(src)?;
    // a name that is no object and no command reads as a product of
    // single-letter variables (`VariableReplacerAlgorithm`), never when the
    // name alone is the whole left side
    let whole_name = mode == Mode::EquationLeft
        && matches!(
            &toks[..],
            [Token {
                tok: Tok::Name { call: false, .. },
                ..
            }]
        );
    let mut p = Parser {
        c,
        src,
        toks,
        pos: 0,
        nodes: Vec::new(),
        lits: Vec::new(),
        bare: Vec::new(),
        brackets: Vec::new(),
        nesting: 0,
        parents: Vec::new(),
        inputs: Vec::new(),
        uses_var: false,
        uses_y: false,
        equation,
        inline_argument: mode == Mode::Argument,
        split_names: equation && !whole_name,
        parameter: if let Mode::Parameter(name) = mode {
            Some(name)
        } else {
            None
        },
        cas_variables: Vec::new(),
        watching_cells: false,
        cell_work: CellBudget {
            visits: *work,
            ..CellBudget::default()
        },
        locals: match mode {
            Mode::System(names) | Mode::ScopedEquation(names) => {
                names.iter().map(|n| (n.to_string(), Elem::Num)).collect()
            }
            _ => Vec::new(),
        },
    };
    if p.toks.is_empty() {
        return Err("empty expression".into());
    }
    let root = p.expression();
    *work = p.cell_work.visits;
    // Speculative command parsing may fall back after an error. A work
    // budget that was exhausted still rejects the whole operation.
    if p.cell_work.exhausted() {
        return Err(crate::spreadsheet::LIMIT.into());
    }
    let root = root?;
    let root = if let Mode::System(names) | Mode::ScopedEquation(names) = mode {
        p.push(Node::SystemFunction(names.len() as u8, root))
    } else {
        root
    };
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
        uses_y: p.uses_y,
    })
}

pub(super) struct Parser<'a> {
    pub(super) c: &'a Construction,
    src: &'a str,
    pub(super) toks: Vec<Token>,
    pub(super) pos: usize,
    pub(super) nodes: Vec<Node>,
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
    pub(super) uses_var: bool,
    uses_y: bool,
    /// Whether `y` reads as the second variable of an equation, not as the
    /// coordinate function (see [`parse_equation`]).
    pub(super) equation: bool,
    inline_argument: bool,
    parameter: Option<&'a str>,
    /// Whether a name that resolves as nothing may read as a product of
    /// single-letter variables (see [`parse_in`]).
    split_names: bool,
    /// The variables of the `Sequence`s being parsed, outermost first.
    pub(super) locals: Vec<(String, Elem)>,
    cas_variables: Vec<String>,
    watching_cells: bool,
    cell_work: CellBudget,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.tok)
    }

    pub(super) fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == Some(tok) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    pub(super) fn expect(&mut self, tok: &Tok, what: &str) -> Result<(), String> {
        if self.eat(tok) {
            Ok(())
        } else {
            Err(format!("expected `{what}`"))
        }
    }

    pub(super) fn push(&mut self, node: Node) -> u32 {
        self.nodes.push(node);
        self.lits.push(Lit::default());
        self.bare.push(false);
        self.brackets.push(false);
        (self.nodes.len() - 1) as u32
    }

    /// Marks `left - right` as the equation `left = right` it was read from,
    /// for the Giac translation of the solving commands.
    #[cfg(feature = "giac")]
    pub(super) fn mark_equation(&mut self, i: u32) {
        self.lits[i as usize] = Lit::Text("=".into());
    }

    /// Keeps the text of a solving command's second argument on its node;
    /// the objects it names become parents, so the command follows them.
    #[cfg(feature = "giac")]
    pub(super) fn mark_hint(&mut self, i: u32, hint: String) {
        for name in hint.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
            if name.chars().next().is_some_and(char::is_alphabetic) {
                if let Some(id) = self.lookup(name) {
                    self.read(id, false);
                }
            }
        }
        self.lits[i as usize] = Lit::Text(hint.into());
    }

    /// The source text from token `from` to the closing bracket of the call.
    #[cfg(feature = "giac")]
    pub(super) fn hint_text(&self, from: usize) -> Option<String> {
        let start = self.toks.get(from)?.start;
        let mut depth = 0i32;
        for token in &self.toks[from..] {
            match token.tok {
                Tok::LParen | Tok::LBrace => depth += 1,
                Tok::RParen if depth == 0 => {
                    return Some(self.src[start..token.start].trim().to_string());
                }
                Tok::RParen | Tok::RBrace => depth -= 1,
                _ => {}
            }
        }
        None
    }

    /// A constant that is a `MySpecialDouble` in GeoGebra, with its decimal.
    fn push_special(&mut self, v: f64, dec: Option<Dec>, spelling: Option<Arc<str>>) -> u32 {
        let i = self.push(Node::Num(v));
        self.lits[i as usize] = Lit::Special(dec.filter(|_| v.is_finite()), spelling);
        i
    }

    pub(super) fn node(&self, i: u32) -> Node {
        self.nodes[i as usize]
    }

    /// The object a reference names when it is a function of one variable
    /// (a `Node::FunctionObject` of kind `Function`), for a fold that
    /// applies it.
    pub(super) fn applied_function(&self, i: u32) -> Option<Id> {
        match self.node(i) {
            Node::FunctionObject(id) if self.c.kind(id) == Kind::Function => Some(id),
            _ => None,
        }
    }

    /// The variable `x`, marking the expression as one that uses it.
    pub(super) fn push_var(&mut self) -> u32 {
        self.uses_var = true;
        self.push(Node::Var)
    }

    /// A reference to the object `id`, registered as a parent — what
    /// writing its label would parse to (`Name` and `Object` insert these
    /// for the object their argument resolved to).
    pub(super) fn object_reference(&mut self, id: Id) -> u32 {
        self.read(id, true);
        self.push(Node::Obj(id))
    }

    /// A string literal node with the text `text`.
    pub(super) fn text_literal(&mut self, text: &str) -> u32 {
        let id = self.push(Node::Text);
        self.lits[id as usize] = Lit::Text(text.into());
        id
    }

    /// The text of a string literal node.
    pub(super) fn literal_text(&self, i: u32) -> Option<&str> {
        match &self.lits[i as usize] {
            Lit::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The construction the expression parses in.
    pub(super) fn construction(&self) -> &Construction {
        self.c
    }

    /// The label of the object `id`.
    pub(super) fn label(&self, id: Id) -> &str {
        self.c.label(id)
    }

    /// The kind of the object `id`.
    pub(super) fn kind(&self, id: Id) -> Kind {
        self.c.kind(id)
    }

    /// The object named `label`, if the construction has it.
    pub(super) fn lookup(&self, label: &str) -> Option<Id> {
        if label.contains('$') {
            let address = crate::spreadsheet::Address::parse(label)?;
            return self.c.id(&address.label());
        }
        self.c.resolve_label(label)
    }

    pub(super) fn read(&mut self, id: Id, input: bool) {
        if !self.parents.contains(&id) {
            self.parents.push(id);
        }
        if input && !self.inputs.contains(&id) {
            self.inputs.push(id);
        }
    }

    /// `expressionnode`: implications of disjunctions, left associative.
    pub(super) fn expression(&mut self) -> Result<u32, String> {
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
        let mut left = self.range()?;
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
            let right = self.range()?;
            let link = self.push(Node::Bin(op, left, right));
            joined = Some(match joined {
                None => link,
                Some(j) => self.push(Node::Bin(Bin::And, j, link)),
            });
            left = right;
        }
        Ok(joined.unwrap_or(left))
    }

    /// The ellipsis creates a list from the two bounds.
    fn range(&mut self) -> Result<u32, String> {
        let mut left = self.sum()?;
        while self.eat(&Tok::Range) {
            let right = self.sum()?;
            left = self.push(Node::Range(left, right, NONE));
        }
        Ok(left)
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
            let pi = self.push_special(PI, Dec::parse(PI_DECIMAL), None);
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
        self.scalar_value(i, depth, false)
    }

    fn scalar_value(&self, i: u32, depth: u32, allow_objects: bool) -> Option<f64> {
        if depth > MAX_NESTING {
            return None;
        }
        let value = |j: u32| self.scalar_value(j, depth + 1, allow_objects);
        Some(match self.node(i) {
            Node::Obj(id) if allow_objects => match self.c.value(id) {
                crate::Value::Number(n) => n,
                _ => return None,
            },
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
                    Bin::Pow | Bin::RootPow => math::my_pow(x, y),
                    _ => return None,
                }
            }
            _ => return None,
        })
    }

    fn cell_coordinate(&mut self, root: u32) -> Result<Option<f64>, String> {
        if let Some(value) = self.scalar_value(root, 0, true) {
            return Ok(Some(value));
        }
        let nodes = &self.nodes[..=root as usize];
        self.cell_work.charge_visits(nodes.len())?;
        let Ok((tys, depth, cost, vars)) = check(self.c, nodes, root) else {
            return Ok(None);
        };
        let r = root as usize;
        if !matches!(tys[r], Ty::Num(_)) || depth[r] > MAX_DEPTH || vars[r] != 0 {
            return Ok(None);
        }
        self.cell_work.charge_visits(cost[r] as usize)?;
        let parsed = Parsed {
            root,
            depth: depth[r],
            cost: cost[r],
            vars: vars[r],
            nodes: nodes.to_vec(),
            lits: self.lits[..=r].to_vec(),
            tys,
            parents: Vec::new(),
            inputs: Vec::new(),
            uses_var: false,
            uses_y: false,
        };
        Ok(super::value_of_tree(self.c, &parsed))
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
            // GeoGebra preserves the multiplication in `-(?)`.
            Node::Num(v) if !(v.is_nan() && self.brackets[e as usize]) => {
                let lit = &self.lits[e as usize];
                if lit.special() && self.bare[e as usize] {
                    let Lit::Special(decimal, spelling) = &self.lits[e as usize] else {
                        unreachable!("a special literal")
                    };
                    let dec = decimal.as_ref().map(Dec::negated);
                    let n = self.push_special(-v, dec, spelling.clone());
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
                    let f = self.push_special(n, Dec::from_f64(n), Some(n.to_string().into()));
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
            Tok::Text(text) => {
                let id = self.push(Node::Text);
                self.lits[id as usize] = Lit::Text(text.into());
                Ok(id)
            }
            Tok::Num(v) => {
                // `MySpecialDouble.toDecimal`: the literal as written
                let text = &self.src[token.start..token.end];
                let dec = Dec::parse(text);
                // `50%` is a literal (`Parser.parseDouble`)
                let (v, dec, spelling) = if self.eat(&Tok::Percent) {
                    let hundredth = Dec::parse("0.01");
                    let dec = dec.zip(hundredth).and_then(|(d, h)| d.mul(&h));
                    (v * 0.01, dec, format!("{text}%").into())
                } else {
                    (v, dec, text.into())
                };
                let n = self.push_special(v, dec, Some(spelling));
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
                    let item = match self.node(e) {
                        Node::Obj(id) if super::list::object_kind(self.c.kind(id)) => {
                            for p in super::list::object_parents(self.c, id) {
                                self.read(p, false);
                            }
                            Node::ItemObject(id)
                        }
                        _ => Node::Item(e),
                    };
                    self.push(item);
                }
                Ok(self.push(Node::List(first, elements.len() as u32)))
            }
            Tok::Name { text, call: false } if self.eat(&Tok::Colon) => {
                let end = self.cell_address()?;
                self.cell_range(&text, &end, Elem::Empty)
            }
            Tok::Name { text, call } => self.name(&text, call),
            _ => Err(format!(
                "unexpected `{}`",
                &self.src[token.start..token.end]
            )),
        }
    }

    fn cell_address(&mut self) -> Result<String, String> {
        let Some(text) = self.name_at(self.pos).map(str::to_owned) else {
            return Err("expected a spreadsheet address".into());
        };
        self.pos += 1;
        Ok(text)
    }

    fn cell_range(&mut self, first: &str, last: &str, template: Elem) -> Result<u32, String> {
        let range = crate::spreadsheet::Range::parse(first, last)?;
        self.cell_work.charge(range.cell_count())?;
        let start = self.nodes.len() as u32;
        for address in range.column_major() {
            let label = address.label();
            let id = self
                .c
                .id(&label)
                .ok_or_else(|| format!("unknown object `{label}`"))?;
            self.read(id, true);
            self.push(Node::CellItem(id));
        }
        Ok(self.push(Node::CellRange(
            start,
            self.nodes.len() as u32 - start,
            template,
        )))
    }

    fn spreadsheet_command(&mut self, name: &str) -> Result<Option<u32>, String> {
        use crate::spreadsheet::Address;

        let result = match name {
            "CellRange" => {
                self.expect(&Tok::LParen, "(")?;
                let first = self.cell_address()?;
                self.expect(&Tok::Comma, ",")?;
                let last = self.cell_address()?;
                let mut template = Elem::Empty;
                if self.eat(&Tok::Comma) {
                    let arg = self.expression()?;
                    if matches!(self.node(arg), Node::Text) {
                        template = match self.literal_text(arg) {
                            Some("text") => Elem::Text,
                            Some("point" | "vector") => Elem::Vec,
                            Some("boolean") => Elem::Bool,
                            Some("list") => Elem::List,
                            Some("function") => Elem::Function,
                            _ => Elem::Empty,
                        };
                    }
                }
                self.expect(&Tok::RParen, ")")?;
                self.cell_range(&first, &last, template)?
            }
            "Row" | "Column" | "ColumnName" => {
                let [arg] = self.arguments()?[..] else {
                    return Err(format!("`{name}` takes one argument"));
                };
                let address = match self.node(arg) {
                    Node::Obj(id) | Node::FunctionObject(id) => Address::parse(self.label(id)),
                    _ => None,
                };
                match name {
                    "Row" => self.push(Node::Num(address.map_or(f64::NAN, |a| f64::from(a.row)))),
                    "Column" => {
                        self.push(Node::Num(address.map_or(f64::NAN, |a| f64::from(a.column))))
                    }
                    _ => match address {
                        Some(address) => self.text_literal(&Address::column_name(address.column)),
                        None => {
                            let condition = self.push(Node::Bool(false));
                            let text = self.text_literal("");
                            self.push(Node::If(super::IfMode::Algo, condition, text, NONE))
                        }
                    },
                }
            }
            "Cell" => {
                let [column, row] = self.arguments()?[..] else {
                    return Err("`Cell` takes two arguments".into());
                };
                let address = self
                    .cell_coordinate(column)?
                    .zip(self.cell_coordinate(row)?)
                    .and_then(|(column, row)| Address::from_numbers(column, row));
                let selected = address.and_then(|a| self.c.spreadsheet_cell(a));
                let kind = selected.map_or(Kind::Number, |id| self.c.kind(id));
                if self.constant_value(column, 0).is_some() && self.constant_value(row, 0).is_some()
                {
                    if let Some(id) = address.and_then(|a| self.c.id(&a.label())) {
                        self.cell_work.charge(1)?;
                        self.read(id, true);
                        self.push(Node::CellWatch(id));
                    }
                } else if !self.watching_cells {
                    self.watching_cells = true;
                    for (address, id) in self.c.spreadsheet_slots() {
                        self.cell_work.charge(1)?;
                        if self.c.id(&address.label()) == Some(id) {
                            self.read(id, true);
                            self.push(Node::CellWatch(id));
                        }
                    }
                }
                self.push(Node::CellLookup(column, row, kind))
            }
            _ => return Ok(None),
        };
        Ok(Some(result))
    }

    /// A name: a constant, the variable `x`, an object, a built-in function
    /// or a command (`Parser.functionOrCommand`, `FunctionParser`).
    fn name(&mut self, text: &str, call: bool) -> Result<u32, String> {
        if let Some(index) = self.cas_variables.iter().position(|name| name == text) {
            let variable = self.push(Node::CasVariable(index as u8));
            if call {
                let argument = self.element()?;
                return Ok(self.push(Node::Bin(Bin::Mul, variable, argument)));
            }
            return Ok(variable);
        }
        // the variable of a `Sequence` hides an object of the same name
        if let Some(depth) = self.locals.iter().rposition(|(name, _)| name == text) {
            let local = self.push(Node::Local(depth as u32, self.locals[depth].1));
            if !call {
                return Ok(local);
            }
            let arg = self.element()?;
            if matches!(text, "x" | "y") && self.is_point_or_vector(arg) {
                let coordinate = if text == "x" { Un::X } else { Un::Y };
                return Ok(self.push(Node::Un(coordinate, arg)));
            }
            return Ok(self.push(Node::Bin(Bin::Mul, local, arg)));
        }
        if self.parameter == Some(text) {
            self.uses_var = true;
            let variable = self.push(Node::Var);
            if !call {
                return Ok(variable);
            }
            let arg = self.element()?;
            if matches!(text, "x" | "y") && self.is_point_or_vector(arg) {
                let coordinate = if text == "x" { Un::X } else { Un::Y };
                return Ok(self.push(Node::Un(coordinate, arg)));
            }
            return Ok(self.push(Node::Bin(Bin::Mul, variable, arg)));
        }
        let free = |s: &str| self.c.id(s).is_none();
        let letter = match text {
            "pi" | "Pi" | "\u{3c0}" => Some((PI, PI_DECIMAL)),
            "\u{212f}" => Some((E, E_DECIMAL)),
            "e" if free("e") => Some((E, E_DECIMAL)),
            _ => None,
        };
        if let Some((v, dec)) = letter {
            return Ok(self.push_special(v, Dec::parse(dec), None));
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
            "y" if self.equation && !call && free("y") => {
                self.uses_y = true;
                Some(Node::VarY)
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
            let args = self.arguments()?;
            return match args[..] {
                [a] => {
                    // `replaceXYZnodes` of an equation: `x(y)` applied to
                    // something that is not a point or a vector is the
                    // product `x·y`, so `y = x(x+1)` is the parabola
                    // y = x² + x and `x(y) = 1` the hyperbola xy = 1
                    let multiplies =
                        self.equation && !self.is_point_or_vector(a) && (op == Un::X || free("y"));
                    if multiplies {
                        let var = match op {
                            Un::Y => {
                                self.uses_y = true;
                                self.push(Node::VarY)
                            }
                            _ => {
                                self.uses_var = true;
                                self.push(Node::Var)
                            }
                        };
                        Ok(self.push(Node::Bin(Bin::Mul, var, a)))
                    } else {
                        // `x(A)`: never an object, `x` and `y` are not labels
                        Ok(self.push(Node::Un(op, a)))
                    }
                }
                _ => Err(format!("`{text}` takes one argument")),
            };
        }
        if text.starts_with("log_{") && self.lookup(text).is_none() {
            if let Some((base, rhs)) = text.strip_prefix("log_{").and_then(|s| s.split_once('}')) {
                if let Some(base) = super::plain_number(base) {
                    let base = self.push(Node::Num(base));
                    let arg = if rhs.is_empty() {
                        self.element()?
                    } else if let Some(number) = super::plain_number(rhs) {
                        self.push(Node::Num(number))
                    } else {
                        return Err(format!("invalid logarithm argument `{rhs}`"));
                    };
                    return Ok(self.push(Node::Bin(Bin::LogB, base, arg)));
                }
            }
        }
        if let Some(id) = self.lookup(text) {
            if self.equation && matches!(self.c.def(id), crate::Def::XAxis | crate::Def::YAxis) {
                return Err("a line cannot be used in an expression".into());
            }
            self.read(id, true);
            if self.c.kind(id) == Kind::FunctionNVar {
                if !call {
                    return Ok(self.push(Node::FunctionObject(id)));
                }
                return match self.arguments()?[..] {
                    [a, b] => Ok(self.push(Node::Apply2(id, a, b))),
                    _ => Err(format!("the function `{text}` takes two arguments")),
                };
            }
            if self.c.kind(id) == Kind::Function {
                if !call {
                    return Ok(self.push(Node::FunctionObject(id)));
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
        if call {
            if let Some(node) = self.spreadsheet_command(text)? {
                return Ok(node);
            }
            // `If` and the commands of a list (see `commands`)
            if let Some(node) = commands::parse(self, text)? {
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
        if self.inline_argument && !call {
            // The fast parser (`VariableReplacerAlgorithm`) reads a name
            // that resolves as nothing by its longest known prefix, and a
            // prefix that names a constant keeps the rest as its own
            // token, so `e5` is `e*5` and `pi15` is `pi*15`.
            let prefix = ["e", "pi", "Pi", "\u{3c0}", "\u{212f}"]
                .iter()
                .copied()
                .filter(|p| *p != "e" || free("e"))
                .find(|p| text.starts_with(*p) && text.len() > p.len());
            if let Some(prefix) = prefix {
                let rest = &text[prefix.len()..];
                if rest.bytes().all(|b| b.is_ascii_digit()) {
                    let constant = self.name(prefix, false)?;
                    let number = self.push(Node::Num(rest.parse().unwrap_or(f64::NAN)));
                    return Ok(self.implicit_product(constant, number));
                }
            }
        }
        if call {
            return self.command();
        }
        if self.split_names && text.len() > 1 && text.bytes().all(|b| b.is_ascii_alphabetic()) {
            // `VariableReplacerAlgorithm`: inside an equation a name of
            // letters that resolves as nothing is a product of one-letter
            // variables (`2xy` is `2*x*y`), while `xy` alone on the left
            // stays one unknown name
            let mut product = self.name(&text[..1], false)?;
            for k in 1..text.len() {
                let right = self.name(&text[k..k + 1], false)?;
                product = self.implicit_product(product, right);
            }
            return Ok(product);
        }
        if text.len() <= MAX_NESTING as usize {
            let mut letters = text.chars();
            if let Some(first) = letters.next() {
                if !letters.as_str().is_empty()
                    && text.chars().all(|letter| {
                        let mut bytes = [0; 4];
                        (letter.is_ascii_lowercase() || letter == 'π')
                            && (matches!(letter, 'x' | 'e' | 'π')
                                || self.c.id(letter.encode_utf8(&mut bytes)).is_some())
                    })
                {
                    let prefix = text.trim_end_matches('x');
                    let power = text.len() - prefix.len();
                    if power > 1 {
                        let var = self.name("x", false)?;
                        let exponent = self.push(Node::Num(power as f64));
                        let right = self.push(Node::Bin(Bin::Pow, var, exponent));
                        if prefix.is_empty() {
                            return Ok(right);
                        }
                        let left = self.name(prefix, false)?;
                        return Ok(self.implicit_product(left, right));
                    }
                    let mut bytes = [0; 4];
                    let mut product = self.name(first.encode_utf8(&mut bytes), false)?;
                    for letter in letters {
                        let right = self.name(letter.encode_utf8(&mut bytes), false)?;
                        product = self.implicit_product(product, right);
                    }
                    return Ok(product);
                }
            }
        }
        Err(format!("unknown object `{text}`"))
    }

    /// Top-level comma positions in the upcoming call.
    pub(super) fn argument_commas(&self) -> Vec<usize> {
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
        commas
    }

    pub(super) fn name_at(&self, i: usize) -> Option<&str> {
        match &self.toks.get(i)?.tok {
            Tok::Name { text, call: false } => Some(text),
            _ => None,
        }
    }

    pub(super) fn object_at(&self, i: usize) -> Option<Id> {
        let name = self.name_at(i)?;
        if self.locals.iter().any(|(local, _)| local == name) {
            return None;
        }
        self.c.id(name)
    }

    pub(super) fn kind_at(&self, i: usize, kind: Kind) -> Option<Id> {
        self.object_at(i).filter(|&id| self.c.kind(id) == kind)
    }

    pub(super) fn list_element_type(&self, i: u32) -> Option<Elem> {
        match check(self.c, &self.nodes, i).ok()?.0[i as usize] {
            Ty::List(elem) => Some(elem),
            _ => None,
        }
    }

    pub(super) fn list_template_type(&self, i: u32) -> Option<Elem> {
        let ty = self.list_element_type(i)?;
        if ty != Elem::Any {
            return Some(ty);
        }
        match self.node(i) {
            Node::List(first, len) if len > 0 => {
                let (types, ..) = check(self.c, &self.nodes, first).ok()?;
                Some(super::types::elem_of(types[first as usize]))
            }
            Node::Obj(id) => {
                let element = self.c.elements(id)?.get(0)?;
                Some(match element {
                    super::Element::List(_) => Elem::List,
                    super::Element::Text(_) => Elem::Text,
                    super::Element::Value(kind, _) => match kind {
                        Kind::Point | Kind::Vector => Elem::Vec,
                        Kind::Boolean => Elem::Bool,
                        Kind::Number | Kind::Angle => Elem::Num,
                        _ => return None,
                    },
                })
            }
            _ => None,
        }
    }

    /// A command of a list (of anything else it is the command of the
    /// script parser of the same name): `form` makes its node from the
    /// arguments and whether each is a list, `None` when they are not what
    /// the command of a list takes. `None` then, with nothing read.
    pub(super) fn list_command(
        &mut self,
        form: impl FnOnce(&mut Self, &[u32], &[bool]) -> Result<Option<Node>, String>,
    ) -> Result<Option<u32>, String> {
        self.list_command_arguments(false, form)
    }

    /// Like a list command, but a bare function name in the first slot
    /// stands for the function of x.
    pub(super) fn function_command(
        &mut self,
        form: impl FnOnce(&mut Self, &[u32], &[bool]) -> Result<Option<Node>, String>,
    ) -> Result<Option<u32>, String> {
        self.list_command_arguments(true, form)
    }

    fn list_command_arguments(
        &mut self,
        takes_function: bool,
        form: impl FnOnce(&mut Self, &[u32], &[bool]) -> Result<Option<Node>, String>,
    ) -> Result<Option<u32>, String> {
        self.command_arguments(
            |p| p.parse_arguments(takes_function),
            |p, args| {
                let lists: Vec<bool> = args.iter().map(|&a| p.is_list(a)).collect();
                form(p, args, &lists)
            },
        )
    }

    /// Variadic list commands classify each argument within its own arena
    /// span to keep type probing independent of its siblings. The probe
    /// returns the root, its type, and whether it is a vector.
    pub(super) fn typed_list_command(
        &mut self,
        form: impl FnOnce(&mut Self, &[(u32, Option<Ty>, bool)]) -> Result<Option<Node>, String>,
    ) -> Result<Option<u32>, String> {
        let mut nodes = Vec::new();
        self.command_arguments(
            |p| {
                p.parse_arguments_as(false, |p, start, root| {
                    // An argument only refers to nodes created while reading
                    // it. Rebase the probe so check keeps its own body root
                    // and budget for nested rewrites.
                    nodes.clear();
                    nodes.extend_from_slice(&p.nodes[start as usize..]);
                    for node in &mut nodes {
                        super::map_children(node, |child| child - start);
                    }
                    let local_root = root - start;
                    let Ok((tys, ..)) = check(p.c, &nodes, local_root) else {
                        return (root, None, false);
                    };
                    let ty = tys[local_root as usize];
                    let vector = ty == Ty::Vec
                        && super::types::vec_kind(p.c, &nodes, &tys, local_root) == Kind::Vector;
                    (root, Some(ty), vector)
                })
            },
            form,
        )
    }

    fn command_arguments<A>(
        &mut self,
        read: impl FnOnce(&mut Self) -> Result<Vec<A>, String>,
        form: impl FnOnce(&mut Self, &[A]) -> Result<Option<Node>, String>,
    ) -> Result<Option<u32>, String> {
        let saved = self.saved();
        // On an error the script parser reports what is wrong.
        let args = read(self).unwrap_or_default();
        let node = if args.is_empty() {
            None
        } else {
            form(self, &args)?
        };
        match node {
            Some(node) => Ok(Some(self.push(node))),
            None => {
                // not a list: the script parser's command
                self.reset(saved);
                Ok(None)
            }
        }
    }

    /// The state a command that read its arguments leaves behind (see
    /// [`Parser::reset`]), so that a command that takes no part of them
    /// can hand them to the script parser untouched.
    pub(super) fn saved(&self) -> (usize, usize, usize, bool, bool, Option<usize>) {
        (
            self.pos,
            self.parents.len(),
            self.inputs.len(),
            self.uses_var,
            self.watching_cells,
            self.cell_work.remaining,
        )
    }

    /// Puts the parser back to a [`Parser::saved`] state.
    pub(super) fn reset(&mut self, saved: (usize, usize, usize, bool, bool, Option<usize>)) {
        self.pos = saved.0;
        self.parents.truncate(saved.1);
        self.inputs.truncate(saved.2);
        self.uses_var = saved.3;
        self.watching_cells = saved.4;
        self.cell_work.reset(saved.5);
    }

    /// Whether the subtree at `i` is a list.
    pub(super) fn is_list(&self, i: u32) -> bool {
        matches!(check(self.c, &self.nodes, i), Ok((tys, ..)) if matches!(tys[i as usize], Ty::List(_)))
    }

    pub(super) fn is_text(&self, i: u32) -> bool {
        matches!(check(self.c, &self.nodes, i), Ok((tys, ..)) if tys[i as usize] == Ty::Text)
    }

    pub(super) fn uses_function_variable(&self, i: u32) -> bool {
        matches!(self.node(i), Node::Apply(_, arg) if matches!(self.node(arg), Node::Var))
            || check(self.c, &self.nodes, i).is_ok_and(|(_, _, _, vars)| vars[i as usize] != 0)
    }

    pub(super) fn is_number(&self, i: u32) -> bool {
        matches!(check(self.c, &self.nodes, i), Ok((tys, ..)) if matches!(tys[i as usize], Ty::Num(_)))
    }

    /// AlgoDimension chooses its output kind from the list at creation.
    pub(super) fn is_matrix(&self, i: u32) -> bool {
        if let Node::Obj(id) = self.node(i) {
            return self.c.elements(id).is_some_and(|list| {
                vector::matrix::dimensions(list, vector::matrix::budget()).is_some()
            });
        }
        let Some(parsed) = super::reparse(self.c, Kind::List, &self.nodes, &self.lits, i) else {
            return false;
        };
        let entry = super::Entry::new(self.c, parsed, false);
        let mut items = Vec::new();
        super::eval::root_list(
            self.c,
            &entry,
            &super::eval::Lists::new(Vec::new()),
            &mut items,
        )
        .is_some_and(|len| {
            vector::matrix::dimensions(super::List::new(&items, len), vector::matrix::budget())
                .is_some()
        })
    }

    pub(super) fn wrap_list(&mut self, args: &[u32]) -> u32 {
        let start = self.nodes.len() as u32;
        for &arg in args {
            self.push(Node::Item(arg));
        }
        self.push(Node::List(start, args.len() as u32))
    }

    /// Whether the subtree at `i` is a point or a vector, the argument
    /// `x(...)` and `y(...)` read as a coordinate of.
    pub(super) fn is_point_or_vector(&self, i: u32) -> bool {
        matches!(check(self.c, &self.nodes, i), Ok((tys, ..)) if matches!(tys[i as usize], Ty::Vec))
    }

    pub(super) fn coordinate_kind(&self, i: u32) -> Option<Kind> {
        let (types, ..) = check(self.c, &self.nodes, i).ok()?;
        (types[i as usize] == Ty::Vec)
            .then(|| super::types::vec_kind(self.c, &self.nodes, &types, i))
    }

    pub(super) fn is_vector_argument(&self, i: u32) -> bool {
        check(self.c, &self.nodes, i)
            .is_ok_and(|(tys, ..)| super::argument_is_vector_at(self.c, &self.nodes, &tys, i))
    }

    pub(super) fn checked_type(&self, i: u32) -> Result<Ty, String> {
        let (types, ..) = check(self.c, &self.nodes, i)?;
        Ok(types[i as usize])
    }

    /// `( expression, expression, ... )` after a function name.
    pub(super) fn arguments(&mut self) -> Result<Vec<u32>, String> {
        self.parse_arguments(false)
    }

    pub(super) fn parse_arguments(&mut self, takes_function: bool) -> Result<Vec<u32>, String> {
        self.parse_arguments_as(takes_function, |_, _, root| root)
    }

    fn parse_arguments_as<A>(
        &mut self,
        takes_function: bool,
        mut classify: impl FnMut(&Self, u32, u32) -> A,
    ) -> Result<Vec<A>, String> {
        self.expect(&Tok::LParen, "(")?;
        let mut read = |p: &mut Self| {
            let start = p.nodes.len() as u32;
            let root = p.argument(takes_function)?;
            Ok::<_, String>(classify(p, start, root))
        };
        let mut args = vec![read(self)?];
        while self.eat(&Tok::Comma) {
            args.push(read(self)?);
        }
        self.expect(&Tok::RParen, ")")?;
        Ok(args)
    }

    pub(super) fn iteration_argument(&mut self, binary: bool) -> Result<u32, String> {
        if binary
            && self
                .toks
                .get(self.pos + 1)
                .is_some_and(|t| t.tok == Tok::Comma)
        {
            if let Some(f) = self.kind_at(self.pos, Kind::FunctionNVar) {
                self.pos += 1;
                self.read(f, true);
                let x = self.push(Node::Var);
                let y = self.push(Node::VarY);
                return Ok(self.push(Node::Apply2(f, x, y)));
            }
        }
        self.argument(true)
    }

    pub(super) fn argument(&mut self, takes_function: bool) -> Result<u32, String> {
        if takes_function
            && matches!(
                self.toks.get(self.pos + 1).map(|t| &t.tok),
                Some(Tok::Comma | Tok::RParen)
            )
        {
            if let Some(f) = self.kind_at(self.pos, Kind::Function) {
                self.pos += 1;
                self.read(f, true);
                self.uses_var = true;
                let x = self.push(Node::Var);
                return Ok(self.push(Node::Apply(f, x)));
            }
        }
        self.expression()
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
            Kind::Number | Kind::Angle | Kind::Point | Kind::Vector | Kind::Boolean
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

/// The forms of the list command `op` named `text`: the list alone, and
/// the list and a number when `count` (`Sum(list, n)`); a first argument
/// that is no list is the script parser's command (see
/// [`Parser::list_command`]).
pub(super) fn list_form(
    text: &str,
    op: ListOp,
    count: bool,
    args: &[u32],
    lists: &[bool],
) -> Result<Option<Node>, String> {
    match (args, lists) {
        (_, [false, ..]) => Ok(None),
        (&[l], _) => Ok(Some(Node::ListOp(op, l, NONE))),
        (&[l, n], _) if count => Ok(Some(Node::ListOp(op, l, n))),
        _ => Err(list_arity(text, args.len())),
    }
}

/// The error of a list command with `n` arguments of the wrong kind.
pub(super) fn list_arity(text: &str, n: usize) -> String {
    format!("`{text}` of a list does not take {n} arguments")
}

#[cfg(test)]
mod spreadsheet_tests {
    use super::*;

    #[test]
    fn repeated_dynamic_lookups_share_one_set_of_watchers() {
        let c = crate::script::load("FillCells(A1:A1000, 2)\nn = 1").unwrap();
        let formula = vec!["Cell(n,1)"; 100].join("+");
        let expr = parse(&c, &formula).unwrap();
        assert_eq!(
            expr.nodes
                .iter()
                .filter(|node| matches!(node, Node::CellWatch(_)))
                .count(),
            1000
        );
    }

    #[test]
    fn lookup_coordinate_probes_share_the_parse_work_budget() {
        let c = crate::script::load("A1 = 2").unwrap();
        let unit = "Cell(If(true,1,2),1)";
        let short = format!("{unit}+1");
        assert_eq!(c.numeric_value(&short), Some(3.0));
        let source = std::iter::repeat_n(unit, 100).collect::<Vec<_>>().join("+");
        assert!(matches!(parse(&c, &source), Err(error) if error == crate::spreadsheet::LIMIT));
    }

    #[test]
    fn abandoned_cell_parses_still_consume_the_work_budget() {
        let mut budget = CellBudget::default();
        let per_form = crate::spreadsheet::MAX_CELLS_PER_OPERATION;
        let start = budget.remaining;
        for _ in 0..MAX_CELL_VISITS / per_form {
            budget.charge(per_form).unwrap();
            budget.reset(start);
        }
        assert!(budget.charge(1).is_err());
        budget.reset(start);
        assert!(budget.exhausted());
    }
}
