use super::{Poly, Quotient, Rat, Work};
use crate::cas::factor;
use crate::expr::{entry, free_decimal, Bin, Entry, Node, StoredExtra};
use crate::{Construction, Def, Id, Value};

pub(super) fn variables() -> [Quotient; 3] {
    std::array::from_fn(|i| Quotient::polynomial(Poly::variable(i)))
}

fn from_decimal(d: &crate::expr::Dec) -> Option<Rat> {
    let (coefficient, exp) = d.signed_coefficient()?;
    Rat::decimal(coefficient, exp)
}

pub(super) fn number(value: Rat) -> Quotient {
    Quotient::polynomial(Poly::constant(value))
}

fn object(c: &Construction, id: Id) -> Option<Rat> {
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
    read_at(c, e, i, vars, work, 0)
}

fn read_at(
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
            if let super::FunctionResult::Tree(tree) = form.as_ref() {
                return read_at(c, tree, tree.root, vars, work, depth + 1);
            }
        }
        if let Some(value) = cached_function(e) {
            return substitute(value, vars, work);
        }
    }
    let args = |index, work: &mut Work<'_>| read_at(c, e, index, vars, work, depth + 1);
    let function = |id, arguments: &[Quotient; 3], work: &mut Work<'_>| {
        let value = c.value(id);
        if !matches!(value, Value::Function | Value::FunctionNVar) {
            return None;
        }
        let Def::Expr(def) = c.def(id) else {
            return None;
        };
        let input = entry(c, def);
        read_at(c, input, input.root, arguments, work, depth + 1)
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
        Node::Obj(id) => Some(number(object(c, id)?)),
        Node::FunctionObject(id) => function(id, vars, work),
        Node::Apply(id, arg) => {
            let input = [args(arg, work)?, vars[1].clone(), vars[2].clone()];
            function(id, &input, work)
        }
        Node::Apply2(id, a, b) => {
            let input = [args(a, work)?, args(b, work)?, vars[2].clone()];
            function(id, &input, work)
        }
        Node::Bin(op @ (Bin::Add | Bin::Sub | Bin::Mul | Bin::Div | Bin::Pow), a, b) => {
            let lhs = args(a, work)?;
            let rhs = args(b, work)?;
            match op {
                Bin::Add => lhs.add(rhs, work),
                Bin::Sub => lhs.sub(rhs, work),
                Bin::Mul => lhs.mul(rhs, work),
                Bin::Div => lhs.div(rhs, work),
                Bin::Pow => {
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
            Node::Bin(Bin::Pow, base, exponent) => {
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

/// The largest coefficient magnitude of any intermediate result of the
/// expression as typed, read as a polynomial in one variable with every
/// coefficient taken by its absolute value, so that nothing cancels: each
/// literal, each value read from a number object, each product, power and
/// sum before cancellation. `None` where the expression is no polynomial
/// (a division by a non-constant, a fractional power) or holds a node this
/// walk does not know.
pub(super) fn typed_bound(c: &Construction, e: &Entry, i: u32, work: &mut Work<'_>) -> Option<f64> {
    let mut largest = 0.0_f64;
    let poly = bound_at(c, e, i, &[0.0, 1.0], &mut largest, work, 0)?;
    Some(poly.iter().fold(largest, |m, v| m.max(*v)))
}

/// The sum of two typed bounds' polynomials is not tracked here; the
/// polynomial of `lhs - rhs` is bounded by the larger side plus the other.
pub(super) fn typed_bound_sides(
    c: &Construction,
    sides: [&Entry; 2],
    work: &mut Work<'_>,
) -> Option<f64> {
    let mut largest = 0.0_f64;
    let left = bound_at(
        c,
        sides[0],
        sides[0].root,
        &[0.0, 1.0],
        &mut largest,
        work,
        0,
    )?;
    let right = bound_at(
        c,
        sides[1],
        sides[1].root,
        &[0.0, 1.0],
        &mut largest,
        work,
        0,
    )?;
    let sum = add_abs(&left, &right);
    Some(sum.iter().fold(largest, |m, v| m.max(*v)))
}

const BOUND_DEGREE: usize = 32;

fn add_abs(a: &[f64], b: &[f64]) -> Vec<f64> {
    (0..a.len().max(b.len()))
        .map(|k| a.get(k).copied().unwrap_or(0.0) + b.get(k).copied().unwrap_or(0.0))
        .collect()
}

fn mul_abs(a: &[f64], b: &[f64]) -> Option<Vec<f64>> {
    if a.is_empty() || b.is_empty() {
        return Some(Vec::new());
    }
    let len = a.len() + b.len() - 1;
    if len > BOUND_DEGREE + 1 {
        return None;
    }
    let mut out = vec![0.0; len];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            out[i + j] += x * y;
        }
    }
    Some(out)
}

/// Whether a sum or difference of two constants with a fractional operand
/// cancels to below a millionth of the larger one.
fn cancels(
    c: &Construction,
    e: &Entry,
    a: u32,
    b: u32,
    op: Bin,
    work: &mut Work<'_>,
) -> Option<bool> {
    let vars = &variables();
    let x = read(c, e, a, vars, work)?.scalar()?;
    let y = read(c, e, b, vars, work)?.scalar()?;
    if x.den == 1 && y.den == 1 {
        return Some(false);
    }
    let result = if op == Bin::Add { x.add(y)? } else { x.sub(y)? };
    let largest = x.to_f64().abs().max(y.to_f64().abs());
    Some(result.to_f64().abs() < 1e-6 * largest)
}

fn bound_at(
    c: &Construction,
    e: &Entry,
    i: u32,
    variable: &[f64],
    largest: &mut f64,
    work: &mut Work<'_>,
    depth: u8,
) -> Option<Vec<f64>> {
    if depth > 128 {
        return None;
    }
    work.spend(BOUND_DEGREE)?;
    let body = |id: Id, variable: &[f64], largest: &mut f64, work: &mut Work<'_>| {
        let Def::Expr(def) = c.def(id) else {
            return None;
        };
        let input = entry(c, def);
        bound_at(c, input, input.root, variable, largest, work, depth + 1)
    };
    let constant = |v: f64| Rat::printed(v).map(|r| vec![r.to_f64().abs()]);
    let result = match e.nodes[i as usize] {
        // A non-integer literal is evaluated in doubles by the reference's
        // numeric solver, whose rounding (`0.1000000001 x - 0.1 x`) the
        // kernel does not model.
        Node::Num(v) if v.fract() != 0.0 => return None,
        Node::Num(v) => constant(v)?,
        Node::MinusOne => vec![1.0],
        Node::Var | Node::VarY | Node::CasVariable(_) | Node::Local(..) => variable.to_vec(),
        Node::Obj(id) => match c.value(id) {
            Value::Number(v) => constant(v)?,
            _ => return None,
        },
        Node::FunctionObject(id) => body(id, variable, largest, work)?,
        Node::Apply(id, arg) => {
            let inner = bound_at(c, e, arg, variable, largest, work, depth + 1)?;
            body(id, &inner, largest, work)?
        }
        Node::Bin(op @ (Bin::Add | Bin::Sub | Bin::Mul | Bin::Div | Bin::Pow), a, b) => {
            let left = bound_at(c, e, a, variable, largest, work, depth + 1)?;
            let right = bound_at(c, e, b, variable, largest, work, depth + 1)?;
            match op {
                Bin::Add | Bin::Sub => {
                    if left.len() == 1 && right.len() == 1 && cancels(c, e, a, b, op, work)? {
                        // The reference evaluates the typed subtraction in
                        // doubles, which loses what cancels.
                        return None;
                    }
                    add_abs(&left, &right)
                }
                Bin::Mul => mul_abs(&left, &right)?,
                Bin::Div => {
                    // Only a division by a number keeps a polynomial.
                    let [d] = right[..] else {
                        return None;
                    };
                    if d == 0.0 {
                        return None;
                    }
                    left.iter().map(|v| v / d).collect()
                }
                _ => {
                    if right.len() != 1 {
                        return None;
                    }
                    // The exponent with its sign.
                    let exponent = read(c, e, b, &variables(), work)?.scalar()?.to_f64();
                    if exponent.fract() != 0.0 || !(0.0..=BOUND_DEGREE as f64).contains(&exponent) {
                        if left.len() == 1 {
                            // A constant to any power is a constant.
                            vec![left[0].powf(exponent)]
                        } else {
                            return None;
                        }
                    } else {
                        let mut out = vec![1.0];
                        for _ in 0..exponent as usize {
                            out = mul_abs(&out, &left)?;
                        }
                        out
                    }
                }
            }
        }
        _ => return None,
    };
    for v in &result {
        *largest = largest.max(*v);
    }
    Some(result)
}
