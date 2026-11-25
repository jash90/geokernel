use super::{Poly, Quotient, Rat, Work};
use crate::cas::factor;
use crate::expr::{entry, free_decimal, Bin, Entry, Node, StoredExtra};
use crate::{Construction, Def, Id, Value};

pub(super) fn variables() -> [Quotient; 3] {
    std::array::from_fn(|i| Quotient::polynomial(Poly::variable(i)))
}

pub(super) fn number(value: Rat) -> Quotient {
    Quotient::polynomial(Poly::constant(value))
}

pub(super) fn from_decimal(d: &crate::expr::Dec) -> Option<Rat> {
    let (coefficient, exp) = d.signed_coefficient()?;
    Rat::decimal(coefficient, exp)
}

pub(super) fn object(c: &Construction, id: Id) -> Option<Rat> {
    let Value::Number(value) = c.value(id) else {
        return None;
    };
    if let Some(decimal) = free_decimal(c, id) {
        return from_decimal(decimal);
    }
    if let Def::Expr(def) = c.def(id) {
        if let StoredExtra::Decimal(d) = &entry(c, def).extra {
            return from_decimal(d);
        }
    }
    Rat::from_value(value)
}

fn cached_function(e: &Entry) -> Option<&Quotient> {
    match &e.extra {
        StoredExtra::Cas(form) => form.quotient(),
        _ => None,
    }
}

pub(super) fn read(
    c: &Construction,
    e: &Entry,
    i: u32,
    vars: &[Quotient; 3],
    work: &mut Work<'_>,
) -> Option<Quotient> {
    read_at::<false>(c, e, i, vars, work, 0)
}

pub(super) fn number_definition(c: &Construction, id: Id) -> Option<std::borrow::Cow<'_, Entry>> {
    match c.def(id) {
        Def::Expr(def) => Some(std::borrow::Cow::Borrowed(entry(c, def))),
        Def::Number(_) => {
            let source = c.construction_source(id)?;
            if source.len() > super::row::MAX_INPUT_BYTES {
                return None;
            }
            let parsed = crate::expr::parse::parse(c, source).ok()?;
            if !parsed.parents.is_empty() {
                return None;
            }
            Some(std::borrow::Cow::Owned(crate::expr::function_entry(
                c, parsed,
            )))
        }
        _ => None,
    }
}

pub(super) fn row_number(c: &Construction, id: Id, work: &mut Work<'_>) -> Option<Rat> {
    if let Some(definition) = number_definition(c, id) {
        if let Some(value) = read_at::<true>(c, &definition, definition.root, &variables(), work, 0)
            .and_then(|value| value.scalar())
        {
            return Some(value);
        }
    }
    object(c, id)
}

fn read_at<const DEFINITIONS: bool>(
    c: &Construction,
    e: &Entry,
    i: u32,
    vars: &[Quotient; 3],
    work: &mut Work<'_>,
    depth: u8,
) -> Option<Quotient> {
    if depth > 128 {
        return None;
    }
    work.spend(1)?;
    if i == e.root {
        if let StoredExtra::Cas(form) = &e.extra {
            if let Some(tree) = form.tree() {
                return read_at::<DEFINITIONS>(c, tree, tree.root, vars, work, depth + 1);
            }
        }
        if let Some(value) = cached_function(e) {
            return substitute(value, vars, work);
        }
    }
    if let Some(result) = e.graph_rewrite(i) {
        let result = result.as_ref()?;
        if let Some(tree) = result.tree() {
            return read_at::<DEFINITIONS>(c, tree, tree.root, vars, work, depth + 1);
        }
        return substitute(result.quotient()?, vars, work);
    }
    let args =
        |index, work: &mut Work<'_>| read_at::<DEFINITIONS>(c, e, index, vars, work, depth + 1);
    let function = |id, arguments: &[Quotient; 3], work: &mut Work<'_>| {
        let value = c.value(id);
        if !matches!(value, Value::Function | Value::FunctionNVar) {
            return None;
        }
        let Def::Expr(def) = c.def(id) else {
            return None;
        };
        let input = entry(c, def);
        read_at::<DEFINITIONS>(c, input, input.root, arguments, work, depth + 1)
    };

    match e.nodes[i as usize] {
        Node::Num(value) => {
            let rational = match e.lits[i as usize].decimal() {
                Some(d) if value != std::f64::consts::PI && value != std::f64::consts::E => {
                    from_decimal(d)?
                }
                Some(_) => return None,
                None => Rat::from_value(value)?,
            };
            Some(number(rational))
        }
        Node::MinusOne => Some(number(Rat::ONE.neg()?)),
        Node::Var => Some(vars[0].clone()),
        Node::VarY => Some(vars[1].clone()),
        Node::CasVariable(index) => vars.get(usize::from(index)).cloned(),
        Node::Local(index, _) => vars.get(index as usize).cloned(),
        Node::SystemFunction(arity, root) if arity <= 3 => args(root, work),
        Node::Obj(id) => {
            if DEFINITIONS && matches!(c.value(id), Value::Number(_)) {
                if let Def::Expr(def) = c.def(id) {
                    let definition = entry(c, def);
                    if let Some(value) = read_at::<DEFINITIONS>(
                        c,
                        definition,
                        definition.root,
                        vars,
                        work,
                        depth + 1,
                    ) {
                        return Some(value);
                    }
                }
            }
            Some(number(object(c, id)?))
        }
        Node::FunctionObject(id) => function(id, vars, work),
        Node::Apply(id, arg) => {
            let input = [args(arg, work)?, vars[1].clone(), vars[2].clone()];
            function(id, &input, work)
        }
        Node::Apply2(id, a, b) => {
            let input = [args(a, work)?, args(b, work)?, vars[2].clone()];
            function(id, &input, work)
        }
        Node::Bin(
            op @ (Bin::Add | Bin::Sub | Bin::Mul | Bin::Div | Bin::Pow | Bin::RootPow),
            a,
            b,
        ) => {
            let lhs = args(a, work)?;
            let rhs = args(b, work)?;
            match op {
                Bin::Add => lhs.add(rhs, work),
                Bin::Sub => lhs.sub(rhs, work),
                Bin::Mul => lhs.mul(rhs, work),
                Bin::Div => lhs.div(rhs, work),
                Bin::Pow | Bin::RootPow => {
                    let exponent = rhs.scalar()?;
                    if exponent.den != 1 {
                        return None;
                    }
                    lhs.pow(exponent.num, work)
                }
                _ => unreachable!(),
            }
        }
        Node::Cas(op, a, b) if op.is_function() => {
            let source = args(a, work)?;
            let other = if b == crate::expr::NONE {
                None
            } else {
                Some(args(b, work)?)
            };
            Some(super::functional::transform(op, source, other, work)?.expanded)
        }
        Node::PolynomialPart(..) if i == e.root => {
            let coefficients = e
                .polynomial()?
                .iter()
                .map(|&v| Rat::from_value(v))
                .collect::<Option<Vec<_>>>()?;
            substitute(
                &Quotient::polynomial(Poly::dense(&coefficients)),
                vars,
                work,
            )
        }
        _ => None,
    }
}

fn substitute(source: &Quotient, vars: &[Quotient; 3], work: &mut Work<'_>) -> Option<Quotient> {
    if *vars == variables() {
        work.spend(source.numerator.0.len() + source.denominator.0.len())?;
        return Some(source.clone());
    }
    let replace = |poly: &Poly, work: &mut Work<'_>| {
        let mut result = number(Rat::ZERO);
        for (powers, &coefficient) in &poly.0 {
            let mut part = number(coefficient);
            for (&exp, variable) in powers.iter().zip(vars) {
                if exp != 0 {
                    part = part.mul(variable.clone().pow(i128::from(exp), work)?, work)?;
                }
            }
            result = result.add(part, work)?;
        }
        Some(result)
    };
    replace(&source.numerator, work)?.div(replace(&source.denominator, work)?, work)
}

pub(super) fn is_factored(
    c: &Construction,
    e: &Entry,
    i: u32,
    work: &mut Work<'_>,
) -> Option<bool> {
    let vars = &variables();
    if read(c, e, i, vars, work)?.coefficients(work)?.len() > 6 {
        return None;
    }
    let mut parts = vec![i];
    let mut scalars = 0;
    while let Some(k) = parts.pop() {
        work.spend(1)?;
        match e.nodes[k as usize] {
            Node::Bin(Bin::Mul, a, b) => {
                parts.extend([a, b]);
                continue;
            }
            Node::Bin(Bin::Pow | Bin::RootPow, base, exponent) => {
                let exponent = read(c, e, exponent, vars, work)?.scalar()?;
                if exponent.den != 1 || exponent.num <= 0 {
                    return Some(false);
                }
                parts.push(base);
                continue;
            }
            Node::FunctionObject(id) => {
                let Def::Expr(def) = c.def(id) else {
                    return None;
                };
                let source = entry(c, def);
                if let Some(form) = cached_function(source) {
                    if matches!(
                        source.nodes[source.root as usize],
                        Node::Cas(super::Op::Factor | super::Op::IFactor, ..)
                    ) {
                        return Some(true);
                    }
                    let coeffs = form.clone().coefficients(work)?;
                    return irreducible(&coeffs, work);
                }
                return is_factored(c, source, source.root, work);
            }
            _ => {}
        }
        let coefficients = read(c, e, k, vars, work)?.coefficients(work)?;
        if coefficients.len() > 6 {
            return None;
        }
        if coefficients.len() <= 1 {
            scalars += 1;
            if scalars > 1 || matches!(e.nodes[k as usize], Node::Bin(Bin::Add | Bin::Sub, ..)) {
                return Some(false);
            }
        } else if !expanded_terms(c, e, k, vars, work)? || !irreducible(&coefficients, work)? {
            return Some(false);
        }
    }
    Some(true)
}

fn irreducible(coeffs: &[Rat], work: &mut Work<'_>) -> Option<bool> {
    if coeffs.len() <= 1 {
        return Some(true);
    }
    let content = coeffs.iter().fold(0, |gcd, c| {
        crate::cas::integer_gcd(gcd, c.num.unsigned_abs())
    });
    if content != 1 || coeffs.iter().any(|c| c.den != 1) || coeffs.iter().all(|c| c.num <= 0) {
        return Some(false);
    }
    let (_, parts) = factor(coeffs, work)?;
    Some(parts.len() == 1 && parts[0].1 == 1)
}

fn expanded_terms(
    c: &Construction,
    e: &Entry,
    i: u32,
    vars: &[Quotient; 3],
    work: &mut Work<'_>,
) -> Option<bool> {
    let mut seen = 0u128;
    let mut parts = vec![i];
    while let Some(k) = parts.pop() {
        work.spend(1)?;
        if let Node::Bin(Bin::Add | Bin::Sub, a, b) = e.nodes[k as usize] {
            parts.extend([a, b]);
            continue;
        }
        let terms = read(c, e, k, vars, work)?.coefficients(work)?;
        let term = 1u128 << terms.len();
        if terms.iter().filter(|c| **c != Rat::ZERO).count() != 1 || seen & term != 0 {
            return Some(false);
        }
        seen |= term;
    }
    Some(true)
}
