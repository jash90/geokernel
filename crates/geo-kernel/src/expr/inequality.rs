//! Boolean combinations of polynomial borders in two coordinates.

use super::equation::Builder;
use super::{entry, Bin, IntersectionWork, Node, Ty, Un};
use crate::algo::plane_polynomial::{self, PlanePolynomial, MAX_PLANE_DEGREE};
use crate::tol::STANDARD_PRECISION;
use crate::{Construction, Def, Id};
use std::sync::Arc;

const MAX_BORDERS: usize = 32;
pub(crate) const MAX_VERTICES: usize = 512;
const MAX_EXPANSION_WORK: u64 = 100_000;

pub(crate) fn is_inequality(c: &Construction, id: Id) -> bool {
    let Def::Expr(def) = c.def(id) else {
        return false;
    };
    let e = entry(c, def);
    matches!(e.nodes[e.root as usize], Node::SystemFunction(2, root)
        if e.tys[root as usize] == Ty::Bool)
}

pub(crate) fn vertices(c: &Construction, source: Id) -> Arc<[(f64, f64)]> {
    c.function_roots
        .inequality(source, || compute(c, source).unwrap_or_default().into())
}

fn compute(c: &Construction, source: Id) -> Option<Vec<(f64, f64)>> {
    let Def::Expr(def) = c.def(source) else {
        return None;
    };
    let e = entry(c, def);
    let Node::SystemFunction(2, root) = e.nodes[e.root as usize] else {
        return None;
    };
    let work = IntersectionWork::new();
    let mut builder = Builder::function(e);
    builder.limit_expansion(MAX_EXPANSION_WORK);
    for node in &mut builder.nodes {
        *node = match *node {
            Node::Local(0, _) => Node::Var,
            Node::Local(1, _) => Node::VarY,
            other => other,
        };
    }
    let mut borders = Vec::new();
    let tree = Tree::build(c, &mut builder, root, &mut borders, &work)?;
    let mut vertices = Vec::new();
    for (i, a) in borders.iter().enumerate() {
        for b in &borders[i + 1..] {
            if !work.charge(1) {
                return None;
            }
            for (x, y) in plane_polynomial::intersections(a, b, &work)? {
                if tree.truth(&borders, x, y, &work)? == 3 {
                    if vertices.len() >= MAX_VERTICES {
                        return None;
                    }
                    vertices.push((x, y));
                }
            }
        }
    }
    Some(vertices)
}

enum Tree {
    Border(usize),
    Constant(bool),
    Not(Box<Tree>),
    Pair(Bin, Box<Tree>, Box<Tree>),
}

impl Tree {
    fn build(
        c: &Construction,
        builder: &mut Builder,
        root: u32,
        borders: &mut Vec<PlanePolynomial>,
        work: &IntersectionWork,
    ) -> Option<Self> {
        if !work.charge(1) {
            return None;
        }
        let tree = match builder.node(root) {
            Node::Bin(op, a, b) if op.is_logic() => Self::Pair(
                op,
                Box::new(Self::build(c, builder, a, borders, work)?),
                Box::new(Self::build(c, builder, b, borders, work)?),
            ),
            Node::Un(Un::Not, a) => Self::Not(Box::new(Self::build(c, builder, a, borders, work)?)),
            Node::Bin(
                op @ (Bin::Greater | Bin::GreaterEqual | Bin::Less | Bin::LessEqual),
                a,
                b,
            ) => {
                let (a, b) = if matches!(op, Bin::Less | Bin::LessEqual) {
                    (b, a)
                } else {
                    (a, b)
                };
                let root = builder.push(Node::Bin(Bin::Sub, a, b));
                let polynomial = builder.expand(c, root).ok()?;
                if polynomial.degree() as usize > MAX_PLANE_DEGREE || borders.len() == MAX_BORDERS {
                    return None;
                }
                let terms = polynomial.terms();
                let coefficients = builder.number_values(c, terms.iter().map(|term| term.coeff));
                let terms = terms
                    .iter()
                    .zip(coefficients)
                    .map(|(term, value)| {
                        Some((
                            term.x as usize,
                            term.y as usize,
                            value.filter(|c| c.is_finite())?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?;
                if polynomial.degree() == 0 {
                    Self::Constant(terms.first().is_some_and(|&(_, _, v)| match op {
                        Bin::GreaterEqual | Bin::LessEqual => v >= 0.0,
                        _ => v > 0.0,
                    }))
                } else {
                    let index = borders.len();
                    borders.push(plane_polynomial::from_terms(terms));
                    Self::Border(index)
                }
            }
            Node::Obj(id) => match c.value(id) {
                crate::Value::Bool(v) => Self::Constant(v),
                _ => return None,
            },
            Node::Bool(value) => Self::Constant(value),
            _ => return None,
        };
        Some(tree)
    }

    /// Bit 0 allows false, bit 1 allows true. At a border both are possible.
    fn truth(
        &self,
        borders: &[PlanePolynomial],
        x: f64,
        y: f64,
        work: &IntersectionWork,
    ) -> Option<u8> {
        if !work.charge(1) {
            return None;
        }
        let mask = match self {
            Self::Border(index) => {
                let v = plane_polynomial::value(&borders[*index], x, y);
                if !v.is_finite() {
                    return None;
                }
                if v.abs() < STANDARD_PRECISION {
                    3
                } else {
                    1 << u8::from(v > 0.0)
                }
            }
            Self::Constant(v) => 1 << u8::from(*v),
            Self::Not(a) => {
                let a = a.truth(borders, x, y, work)?;
                (a >> 1) | ((a & 1) << 1)
            }
            Self::Pair(op, a, b) => {
                let a = a.truth(borders, x, y, work)?;
                let b = b.truth(borders, x, y, work)?;
                let mut result = 0;
                for u in [false, true] {
                    for v in [false, true] {
                        if a & (1 << u8::from(u)) == 0 || b & (1 << u8::from(v)) == 0 {
                            continue;
                        }
                        let truth = match op {
                            Bin::And => u && v,
                            Bin::Or => u || v,
                            Bin::Xor => u ^ v,
                            Bin::Implies => !u || v,
                            _ => unreachable!("logical pair"),
                        };
                        result |= 1 << u8::from(truth);
                    }
                }
                result
            }
        };
        Some(mask)
    }
}
