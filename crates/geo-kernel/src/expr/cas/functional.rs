use super::input::{read, variables};
use super::{Op, Poly, Quotient, Rat, Slot, Work};
use crate::expr::{eval, fraction, Entry, Ty, NONE};
use crate::Construction;
use std::fmt;
use std::sync::Arc;

type Product = (Rat, Vec<(Poly, u32)>);

#[derive(Clone, Debug, PartialEq)]
enum Form {
    Expanded,
    /// The expanded form evaluated as the reference evaluates its printed
    /// expression: term by term from the highest power, each term the
    /// coefficient times the power (`Math.pow` in `MyDouble.pow`, here
    /// `f64::powf`), summed left to right (the printed
    /// `t^2 - 100000000t - 99999999` at `t = 10^8 + 1` is 1, Horner's scheme
    /// gives 2).
    Printed,
    Product {
        numerator: Product,
        denominator: Product,
        real_display: Option<String>,
    },
    Square {
        a: Rat,
        shift: Rat,
        offset: Rat,
        power: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::expr) struct Function {
    pub(in crate::expr) expanded: Quotient,
    form: Form,
}

impl Function {
    fn new(expanded: Quotient) -> Self {
        Self {
            expanded,
            form: Form::Expanded,
        }
    }

    pub(in crate::expr) fn value(&self, x: f64, y: f64) -> f64 {
        match &self.form {
            Form::Expanded => self.expanded.value(x, y),
            Form::Printed => {
                let sum = |p: &Poly| {
                    let mut terms: Vec<_> = p.0.iter().collect();
                    terms.sort_by(|u, v| v.0.cmp(u.0));
                    terms.iter().fold(0.0, |acc, (power, c)| {
                        acc + c.to_f64() * x.powf(f64::from(power[0])) * y.powf(f64::from(power[1]))
                    })
                };
                sum(&self.expanded.numerator) / sum(&self.expanded.denominator)
            }
            Form::Product {
                numerator,
                denominator,
                ..
            } => {
                let value = |(scalar, factors): &Product| {
                    factors
                        .iter()
                        .fold(scalar.to_f64(), |value, (factor, multiplicity)| {
                            value * factor.value([x, y, f64::NAN]).powi(*multiplicity as i32)
                        })
                };
                value(numerator) / value(denominator)
            }
            Form::Square {
                a,
                shift,
                offset,
                power,
            } => {
                let term = x.powi(*power as i32) + shift.to_f64();
                a.to_f64() * (term * term) + offset.to_f64()
            }
        }
    }
}

#[derive(Clone)]
pub(in crate::expr) enum FunctionResult {
    Exact(Function),
    Tree(Arc<Entry>),
    Number(f64),
}

impl PartialEq for FunctionResult {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Exact(a), Self::Exact(b)) => a == b,
            (Self::Tree(a), Self::Tree(b)) => Arc::ptr_eq(a, b),
            (Self::Number(a), Self::Number(b)) => a == b,
            _ => false,
        }
    }
}

impl FunctionResult {
    pub(in crate::expr) fn value(&self, c: &Construction, x: f64, y: f64) -> f64 {
        match self {
            Self::Exact(result) => result.value(x, y),
            Self::Tree(entry) => eval::sample(c, entry, x),
            Self::Number(value) => *value,
        }
    }

    pub(in crate::expr) fn quotient(&self) -> Option<&Quotient> {
        match self {
            Self::Exact(result) => Some(&result.expanded),
            _ => None,
        }
    }

    pub(super) fn displayed(&self) -> Option<String> {
        match self {
            Self::Exact(result) => Some(result.to_string()),
            _ => None,
        }
    }
}

pub(in crate::expr) fn function(
    c: &Construction,
    e: &Entry,
    op: Op,
    a: u32,
    b: u32,
) -> Option<FunctionResult> {
    evaluate(&eval::Ctx::<true>::new(c, e, &eval::Env::None), op, a, b)
}

pub(in crate::expr) fn evaluate<const M: bool>(
    ctx: &eval::Ctx<'_, M>,
    op: Op,
    a: u32,
    b: u32,
) -> Option<FunctionResult> {
    let allowance = eval::Lists::new(Vec::new());
    let charge = |n| ctx.meter.unwrap_or(&allowance).charge(n as u64);
    let work = &mut Work::new(&charge);
    let vars = &variables();
    if matches!(op, Op::Numerator | Op::Denominator) {
        let denominator = op == Op::Denominator;
        if matches!(ctx.e.tys[a as usize], Ty::Num(_)) && !uses_variable(ctx.e, a, work)? {
            let (n, d) = fraction::number_parts(ctx, a)?;
            return Some(FunctionResult::Number(if denominator { d } else { n }));
        }
        let (entry, root) = super::argument_tree(ctx.c, ctx.e, a)?;
        if matches!(entry.tys[root as usize], Ty::Num(_))
            && !matches!(entry.nodes[root as usize], super::Node::Cas(..))
            && !uses_other_variable(entry, root, work)?
        {
            let tree = fraction::project_function(ctx.c, entry, root, denominator)?;
            return Some(FunctionResult::Tree(Arc::new(tree)));
        }
    }
    if op == Op::Polynomial {
        let (entry, root) = super::argument_tree(ctx.c, ctx.e, a)?;
        if matches!(entry.tys[root as usize], Ty::Num(_)) {
            if let Some(coefficients) = super::structural_coefficients(ctx.c, entry, root) {
                let mut builder = crate::expr::equation::Builder::empty();
                let parsed = crate::algo::calculus::poly_from_coefficients(
                    ctx.c,
                    &mut builder,
                    &coefficients,
                )?;
                let tree = crate::expr::function_entry(ctx.c, parsed);
                return Some(FunctionResult::Tree(Arc::new(tree)));
            }
        }
    }

    if let (Op::Simplify, super::Node::Obj(id)) = (op, ctx.node(a)) {
        if !matches!(ctx.c.kind(id), crate::Kind::Number | crate::Kind::Angle) {
            return super::objects::as_function(ctx.c, id, work);
        }
    }
    let exact = (|| {
        let source = read(ctx.c, ctx.e, a, vars, work)?;
        let other = if b == NONE {
            None
        } else {
            Some(read(ctx.c, ctx.e, b, vars, work)?)
        };
        transform(op, source, other, work)
    })();
    // `Simplify.1` answers the shorter of the normal form and the factored
    // form (`Ggb2giac`), which evaluate differently far from 0 (`(t - 10^8)^2`
    // against its expansion). A curve coordinate is answered only where both
    // forms are the same expression: a polynomial that is a number times one
    // irreducible factor at most, of multiplicity one.
    let exact = match exact {
        Some(f) if op == Op::Simplify && ctx.e.curve_parameter.is_some() => {
            // A rational coordinate is kept as a quotient by the reference
            // and evaluated in that form, which the kernel does not keep.
            let polynomial = f.expanded.denominator.scalar().is_some();
            (polynomial && single_factors(&f.expanded, work)?).then_some(f)
        }
        other => other,
    };
    // A rewritten curve coordinate is evaluated like the expression the
    // reference prints for it.
    let exact = exact.map(|mut f| {
        if ctx.e.curve_parameter.is_some()
            && matches!(op, Op::Expand | Op::Simplify)
            && f.form == Form::Expanded
        {
            f.form = Form::Printed;
        }
        f
    });
    // A curve coordinate that is no rational function is beyond this
    // kernel: `AlgoCasBaseSingleArgument.applyCasCommand` sends it to the
    // CAS, which may rewrite it into something that evaluates differently
    // (`exp(t) exp(-t)` becomes 1), so the coordinate is undefined.
    exact.map(FunctionResult::Exact)
}

/// Whether the numerator and the denominator each have at most one
/// irreducible factor, of multiplicity one.
fn single_factors(q: &Quotient, work: &mut Work<'_>) -> Option<bool> {
    for p in [&q.numerator, &q.denominator] {
        let (_, factors) = product(p, work)?;
        if factors.len() > 1 || factors.iter().any(|(_, m)| *m > 1) {
            return Some(false);
        }
    }
    Some(true)
}

pub(super) fn transform(
    op: Op,
    polynomial: Quotient,
    other: Option<Quotient>,
    work: &mut Work<'_>,
) -> Option<Function> {
    let result = match op {
        Op::Numerator | Op::Denominator => {
            let projected = if op == Op::Numerator {
                polynomial.numerator
            } else {
                polynomial.denominator
            };
            Function::new(Quotient::polynomial(projected))
        }
        Op::Expand | Op::Simplify => Function::new(polynomial.normalized(work)?),
        Op::Polynomial => Function::new(Quotient::polynomial(Poly::dense(
            &polynomial.coefficients(work)?,
        ))),
        Op::Factor | Op::IFactor => {
            let expanded = polynomial.normalized(work)?;
            let numerator = product(&expanded.numerator, work)?;
            let denominator = product(&expanded.denominator, work)?;
            let real_display = if op == Op::IFactor {
                let n = print_real_product(&numerator, work);
                if expanded.denominator.scalar() == Some(Rat::ONE) {
                    Some(n)
                } else {
                    Some(format!(
                        "({n})/({})",
                        print_real_product(&denominator, work)
                    ))
                }
            } else {
                None
            };
            Function {
                expanded,
                form: Form::Product {
                    numerator,
                    denominator,
                    real_display,
                },
            }
        }
        Op::CompleteSquare => {
            let coefficients = polynomial.clone().coefficients(work)?;
            let degree = coefficients.len().checked_sub(1)?;
            let power = degree / 2;
            if degree == 0
                || degree % 2 != 0
                || coefficients
                    .iter()
                    .enumerate()
                    .any(|(i, &c)| i != 0 && i != power && i != degree && c != Rat::ZERO)
            {
                return None;
            }
            let a = coefficients[degree];
            let b = coefficients[power];
            let c = coefficients[0];
            let shift = b.div(a.mul(Rat::new(2, 1)?)?)?;
            let offset = c.sub(a.mul(shift.mul(shift)?)?)?;
            Function {
                expanded: polynomial.normalized(work)?,
                form: Form::Square {
                    a,
                    shift,
                    offset,
                    power,
                },
            }
        }
        Op::CommonDenominator => {
            let a = polynomial
                .normalized(work)?
                .integral_parts(work)?
                .denominator;
            let b = other?.normalized(work)?.integral_parts(work)?.denominator;
            Function::new(Quotient::polynomial(a.common_multiple(&b, work)?))
        }
        _ => return None,
    };
    Some(result)
}

fn product(input: &Poly, work: &mut Work<'_>) -> Option<Product> {
    let (mut scalar, factors) = crate::cas::factor_poly(input, work)?;
    let mut parts = Vec::with_capacity(factors.len());
    for (factor, exponent) in factors {
        let (content, primitive) = factor.primitive(work)?;
        for _ in 0..exponent {
            scalar = scalar.mul(content)?;
        }
        parts.push((primitive, exponent));
    }
    // This does not rely on the modular factor ordering.
    parts.sort_by(|(a, _), (b, _)| {
        let degree = |p: &Poly| p.0.last_key_value().map_or(0, |(power, _)| power[0]);
        degree(a)
            .cmp(&degree(b))
            .then_with(|| a.value([0.0; 3]).total_cmp(&b.value([0.0; 3])))
    });
    Some((scalar, parts))
}

pub(super) fn factor_list<const M: bool>(
    ctx: &eval::Ctx<'_, M>,
    input: u32,
    work: &mut Work<'_>,
) -> Option<(u32, u32)> {
    let polynomial = read(ctx.c, ctx.e, input, &variables(), work)?;
    let numeric = !matches!(ctx.e.tys[input as usize], crate::expr::Ty::Function)
        && !uses_variable(ctx.e, input, work)?;
    let pairs = if let Some(n) = polynomial.scalar().filter(|_| numeric) {
        if n.den != 1 || !(1..=1 << 53).contains(&n.num) {
            return None;
        }
        let charge = |n| work.spend(n as usize).is_none();
        crate::expr::commands::integer_factors(n.num as u64, charge)?
            .into_iter()
            .map(|(v, exponent)| (Slot::number(v as f64), exponent))
            .collect::<Vec<_>>()
    } else {
        let expanded = polynomial.normalized(work)?;
        if expanded.denominator.scalar() != Some(Rat::ONE) {
            return None;
        }
        let (scalar, mut parts) = product(&expanded.numerator, work)?;
        if scalar != Rat::ONE || parts.is_empty() {
            parts.insert(0, (Poly::constant(scalar), 1));
        }
        parts
            .into_iter()
            .map(|(factor, exponent)| (Slot::polynomial(factor), exponent))
            .collect()
    };
    let lists = ctx.lists?;
    let len = pairs.len().try_into().ok()?;
    let start = lists.reserve(len)?;
    for (i, (factor, exponent)) in pairs.into_iter().enumerate() {
        let row = ctx.cas_slots([factor, Slot::number(f64::from(exponent))].into_iter())?;
        lists.set(start + i as u32, Slot::list(row.0, row.1));
    }
    Some((start, len))
}

fn uses_other_variable(e: &Entry, input: u32, work: &mut Work<'_>) -> Option<bool> {
    uses_node(e, input, work, |node| {
        matches!(
            node,
            super::Node::VarY | super::Node::SystemFunction(..) | super::Node::Local(..)
        )
    })
}

fn uses_variable(e: &Entry, input: u32, work: &mut Work<'_>) -> Option<bool> {
    uses_node(e, input, work, |node| {
        matches!(
            node,
            super::Node::Var | super::Node::VarY | super::Node::Local(..)
        )
    })
}

fn uses_node(
    e: &Entry,
    input: u32,
    work: &mut Work<'_>,
    pred: impl Fn(super::Node) -> bool,
) -> Option<bool> {
    let mut pending = vec![input];
    while let Some(i) = pending.pop() {
        work.spend(1)?;
        let node = e.nodes[i as usize];
        if pred(node) {
            return Some(true);
        }
        pending.extend(crate::expr::children(&node));
    }
    Some(false)
}

pub(super) fn print_value(value: Rat) -> String {
    if value.den == 1 {
        value.num.to_string()
    } else {
        format!("{}/{}", value.num, value.den)
    }
}

pub(in crate::expr) fn print_poly(p: &Poly) -> String {
    if p.0.is_empty() {
        return "0".into();
    }
    let mut text = String::new();
    for (powers, &coefficient) in p.0.iter().rev() {
        let magnitude = Rat {
            num: coefficient.num.abs(),
            den: coefficient.den,
        };
        if text.is_empty() {
            if coefficient.num < 0 {
                text.push('-');
            }
        } else {
            text.push_str(if coefficient.num < 0 { " - " } else { " + " });
        }
        let has_variables = powers.iter().any(|&power| power != 0);
        if !has_variables || magnitude != Rat::ONE {
            text.push_str(&print_value(magnitude));
            if has_variables {
                text.push('*');
            }
        }
        let mut separator = false;
        for (variable, &power) in ["x", "y", "z"].iter().zip(powers) {
            if power == 0 {
                continue;
            }
            if separator {
                text.push('*');
            }
            text.push_str(variable);
            if power > 1 {
                text.push('^');
                text.push_str(&power.to_string());
            }
            separator = true;
        }
    }
    text
}

fn print_product((scalar, parts): &Product) -> String {
    let mut factors = Vec::new();
    if *scalar != Rat::ONE || parts.is_empty() {
        factors.push(print_value(*scalar));
    }
    for (factor, exponent) in parts {
        let factor = print_poly(factor);
        factors.push(if *exponent == 1 {
            format!("({factor})")
        } else {
            format!("({factor})^{exponent}")
        });
    }
    factors.join("*")
}

fn print_real_product((scalar, parts): &Product, work: &mut Work<'_>) -> String {
    let mut factors = Vec::new();
    if *scalar != Rat::ONE || parts.is_empty() {
        factors.push(print_value(*scalar));
    }
    for (factor, exponent) in parts {
        let factor = radical_factors(factor, work).unwrap_or_else(|| print_poly(factor));
        factors.push(if *exponent == 1 {
            format!("({factor})")
        } else {
            format!("({factor})^{exponent}")
        });
    }
    factors.join("*")
}

fn radical_factors(factor: &Poly, work: &mut Work<'_>) -> Option<String> {
    let coefficients = factor.coefficients()?;
    let degree = coefficients.len().checked_sub(1)?;
    let lead = coefficients[degree];
    if degree == 2 {
        let crate::cas::Roots::Finite(roots) = crate::cas::real_roots(&coefficients, work)? else {
            return None;
        };
        if roots.len() != 2 {
            return None;
        }
        let a = super::solutions::exact(roots[0])?;
        let b = super::solutions::exact(roots[1])?;
        return Some(format!("{}*(x - ({a}))*(x - ({b}))", print_value(lead)));
    }
    if degree > 2
        && degree.is_power_of_two()
        && coefficients[0].num < 0
        && coefficients[1..degree].iter().all(|&c| c == Rat::ZERO)
    {
        let constant = print_value(coefficients[0].neg()?.div(lead)?);
        let mut power = degree;
        let mut factors = vec![print_value(lead)];
        while power > 1 {
            let exponent = degree / (power / 2);
            let root = format!("({constant})^(1/{exponent})");
            power /= 2;
            if power == 1 {
                factors.push(format!("(x + {root})"));
                factors.push(format!("(x - {root})"));
            } else {
                factors.push(format!("(x^{power} + {root})"));
            }
        }
        return Some(factors.join("*"));
    }
    None
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let quotient = |f: &mut fmt::Formatter<'_>, numerator, denominator, is_one| {
            if is_one {
                write!(f, "{numerator}")
            } else {
                write!(f, "({numerator})/({denominator})")
            }
        };
        match &self.form {
            Form::Expanded | Form::Printed => quotient(
                f,
                print_poly(&self.expanded.numerator),
                print_poly(&self.expanded.denominator),
                self.expanded.denominator.scalar() == Some(Rat::ONE),
            ),
            Form::Product {
                real_display: Some(text),
                ..
            } => f.write_str(text),
            Form::Product {
                numerator,
                denominator,
                ..
            } => quotient(
                f,
                print_product(numerator),
                print_product(denominator),
                self.expanded.denominator.scalar() == Some(Rat::ONE),
            ),
            Form::Square {
                a,
                shift,
                offset,
                power,
            } => {
                let variable = if *power == 1 {
                    "x".into()
                } else {
                    format!("x^{power}")
                };
                write!(
                    f,
                    "{}*({variable} + {})^2 + {}",
                    print_value(*a),
                    print_value(*shift),
                    print_value(*offset)
                )
            }
        }
    }
}
