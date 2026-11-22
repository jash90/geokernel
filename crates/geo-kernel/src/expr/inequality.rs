//! Boolean combinations of polynomial borders in two coordinates.

// Border classification and dispatch follow Inequality.java and AlgoVertexIneq.java
// at upstream revision 112919f2d450ec7430a5cd3b65dba226fdc43b74.
// Licensed under the EUPL 1.2.

use super::equation::Builder;
use super::{entry, Bin, IntersectionWork, Node, Ty, Un};
use crate::algo::conic_nd::ConicData;
use crate::algo::intersect::boundary::line_crossing;
use crate::algo::intersect::helper_line_conic_points;
use crate::algo::plane_polynomial::{self, PlanePolynomial};
use crate::tol::STANDARD_PRECISION;
use crate::{Construction, Def, Id};
use std::sync::Arc;

const MAX_BORDERS: usize = 32;
pub(crate) const MAX_VERTICES: usize = 512;
const MAX_EXPANSION_WORK: u64 = 100_000;
// Bound dense coefficient storage even when expansion returns a single high power.
const MAX_BORDER_DEGREE: u32 = 300;

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
            for (x, y) in a.intersections(b, &work)? {
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BorderType {
    ParametricX,
    ParametricY,
    Line,
    Conic,
    OneVariableX,
    OneVariableY,
    Implicit,
}

struct Border {
    kind: BorderType,
    polynomial: PlanePolynomial,
    evaluation_cost: u64,
}

impl Border {
    fn new(terms: Vec<(usize, usize, f64)>) -> Self {
        use BorderType::*;
        let mut degree = 0;
        let mut degrees = [0, 0];
        let mut mixed = false;
        for &(x, y, _) in &terms {
            degree = degree.max(x + y);
            degrees[0] = degrees[0].max(x);
            degrees[1] = degrees[1].max(y);
            mixed |= x > 0 && y > 0;
        }
        let kind = match degrees {
            [_, 0] => OneVariableX,
            [0, _] => OneVariableY,
            _ if degree == 1 => Line,
            [1, _] if !mixed => ParametricX,
            [_, 1] if !mixed => ParametricY,
            _ if degree == 2 => Conic,
            _ => Implicit,
        };
        let polynomial = plane_polynomial::from_terms(terms);
        let evaluation_cost = plane_polynomial::evaluation_cost(&polynomial);
        Self {
            kind,
            polynomial,
            evaluation_cost,
        }
    }

    fn coefficient(&self, x: usize, y: usize) -> f64 {
        self.polynomial
            .get(y)
            .and_then(|row| row.get(x))
            .copied()
            .unwrap_or_default()
    }

    fn intersections(&self, other: &Self, work: &IntersectionWork) -> Option<Vec<(f64, f64)>> {
        use BorderType::*;
        let (a, b) = if self.kind <= other.kind {
            (self, other)
        } else {
            (other, self)
        };
        match (a.kind, b.kind) {
            (_, Implicit) | (OneVariableY, OneVariableY) => Some(Vec::new()),
            (Line, Line) => {
                // The line-border helper works in homogeneous coordinates and
                // divides by any nonzero weight before testing the Boolean tree.
                let line = |p: &Self| {
                    (
                        p.coefficient(1, 0),
                        p.coefficient(0, 1),
                        p.coefficient(0, 0),
                    )
                };
                Some(line_crossing(line(a), line(b)).into_iter().collect())
            }
            (Line, Conic) => {
                let mut conic = ConicData::new();
                conic.set_matrix([
                    b.coefficient(2, 0),
                    b.coefficient(0, 2),
                    b.coefficient(0, 0),
                    b.coefficient(1, 1) * 0.5,
                    b.coefficient(1, 0) * 0.5,
                    b.coefficient(0, 1) * 0.5,
                ]);
                let line = [
                    a.coefficient(1, 0),
                    a.coefficient(0, 1),
                    a.coefficient(0, 0),
                ];
                Some(
                    helper_line_conic_points(line, &conic)
                        .into_iter()
                        .flatten()
                        .collect(),
                )
            }
            (ParametricX, ParametricX | Line | Conic | OneVariableX | OneVariableY) => {
                // Use y as the independent coordinate of x = f(y).
                if !work.charge(a.evaluation_cost + b.evaluation_cost) {
                    return None;
                }
                let a = plane_polynomial::transpose(&a.polynomial);
                let b = plane_polynomial::transpose(&b.polynomial);
                Some(
                    plane_polynomial::intersections(&a, &b, work)?
                        .into_iter()
                        .map(|(y, x)| (x, y))
                        .collect(),
                )
            }
            _ => plane_polynomial::intersections(&a.polynomial, &b.polynomial, work),
        }
    }
}

fn rewrite_abs(builder: &mut Builder, a: u32, b: u32) -> (u32, u32) {
    let square = |builder: &mut Builder, x| builder.push(Node::Bin(Bin::Mul, x, x));
    let signed_square = |builder: &mut Builder, x| {
        let abs = builder.push(Node::Un(Un::Abs, x));
        builder.push(Node::Bin(Bin::Mul, x, abs))
    };
    match (builder.node(a), builder.node(b)) {
        (Node::Un(Un::Abs, x), Node::Un(Un::Abs, y)) => (square(builder, x), square(builder, y)),
        (Node::Un(Un::Abs, x), _) if !builder.contains_var(b) => {
            (square(builder, x), signed_square(builder, b))
        }
        (_, Node::Un(Un::Abs, y)) if !builder.contains_var(a) => {
            (signed_square(builder, a), square(builder, y))
        }
        _ => (a, b),
    }
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
        borders: &mut Vec<Border>,
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
                let (a, b) = rewrite_abs(builder, a, b);
                let root = builder.push(Node::Bin(Bin::Sub, a, b));
                let polynomial = builder.expand(c, root).ok()?;
                if polynomial.degree() > MAX_BORDER_DEGREE {
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
                if terms.iter().all(|&(x, y, v)| (x, y) == (0, 0) || v == 0.0) {
                    let constant = terms
                        .iter()
                        .find(|&&(x, y, _)| (x, y) == (0, 0))
                        .map_or(0.0, |&(_, _, v)| v);
                    Self::Constant(match op {
                        Bin::GreaterEqual | Bin::LessEqual => constant >= 0.0,
                        _ => constant > 0.0,
                    })
                } else {
                    if borders.len() == MAX_BORDERS || !work.charge(terms.len() as u64) {
                        return None;
                    }
                    let index = borders.len();
                    borders.push(Border::new(
                        terms.into_iter().filter(|&(_, _, v)| v != 0.0).collect(),
                    ));
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
    fn truth(&self, borders: &[Border], x: f64, y: f64, work: &IntersectionWork) -> Option<u8> {
        if !work.charge(1) {
            return None;
        }
        let mask = match self {
            Self::Border(index) => {
                let border = &borders[*index];
                if !work.charge(border.evaluation_cost) {
                    return None;
                }
                let v = plane_polynomial::value(&border.polynomial, x, y);
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

#[cfg(test)]
mod tests {
    use crate::{script, Value};

    #[test]
    fn inequality_vertices_recover_after_an_exponent_exceeds_the_storage_budget() {
        let mut c = script::load("n = 2\nq(x,y) = y > x^n && x < 1\nP = Vertex(q)").unwrap();
        let p = c.id("P_{1}").or_else(|| c.id("P")).unwrap();
        let n = c.id("n").unwrap();
        let vertex = Value::Point { x: 1.0, y: 1.0 };
        assert_eq!(c.value(p), vertex);
        c.set_number(n, 1_000_000_000.0).unwrap();
        assert_eq!(c.value(p), Value::Undefined);
        c.set_number(n, 2.0).unwrap();
        assert_eq!(c.value(p), vertex);
    }
}
