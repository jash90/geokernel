// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectFunctionsNewton.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectFunctionLineNewton.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectPolynomialConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRootsPolynomial.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSimpleRootsPolynomial.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdIntersect.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::boundary::Carrier;
use super::conics::graph_roots;
use super::function_roots::{check_root, interval_roots, root_near};
use crate::algo::conic_nd::ConicType;
use crate::algo::{curve, Group, Parents, CARRIERS, CONIC_OBJECTS};
use crate::expr::Raw;
use crate::kernel::{Construction, Error, Id, Kind, Value};
use crate::tol::{MIN_PRECISION, STANDARD_PRECISION};

const FUNCTION: &[Kind] = &[Kind::Function];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FunctionIntersection {
    function: Id,
    other: Id,
    method: Method,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Method {
    Polynomial(u32),
    Indexed(u32),
    ConstantIndex(u32),
    Seed { point: u32, line_first: bool },
    Automatic,
    Conic(u32),
    Interval { left: u32, right: u32, slot: u32 },
}

impl FunctionIntersection {
    pub(super) fn labels_indexed(self) -> bool {
        matches!(self.method, Method::Conic(_))
    }

    fn pair(c: &Construction, first: &str, second: &str) -> Option<(Id, Id)> {
        let (a, b) = (c.id(first)?, c.id(second)?);
        let graph_partner = |kind| {
            kind == Kind::Function || CARRIERS.contains(&kind) || CONIC_OBJECTS.contains(&kind)
        };
        match (c.kind(a), c.kind(b)) {
            (Kind::Function, kind) if graph_partner(kind) => Some((a, b)),
            (kind, Kind::Function) if graph_partner(kind) => Some((b, a)),
            _ => None,
        }
    }

    fn with_slot(self, slot: u32) -> Self {
        let method = match self.method {
            Method::Conic(_) => Method::Conic(slot),
            Method::Interval { left, right, .. } => Method::Interval { left, right, slot },
            _ => Method::Polynomial(slot),
        };
        Self { method, ..self }
    }

    fn outputs(self, c: &Construction) -> Vec<Self> {
        if self.method == Method::Automatic {
            return vec![self];
        }
        let count = self.points(c).len().max(1) as u32;
        (0..count).map(|slot| self.with_slot(slot)).collect()
    }

    pub(super) fn multi(c: &Construction, name: &str, args: &[&str]) -> Option<Vec<Self>> {
        let ("Intersect", [first, second]) = (name, args) else {
            return None;
        };
        let (function, other) = Self::pair(c, first, second)?;
        let method = if CONIC_OBJECTS.contains(&c.kind(other)) {
            Method::Conic(0)
        } else if c.function_fraction(function).is_some()
            && (c.kind(other) != Kind::Function || c.function_fraction(other).is_some())
        {
            Method::Polynomial(0)
        } else {
            Method::Automatic
        };
        Some(
            Self {
                function,
                other,
                method,
            }
            .outputs(c),
        )
    }

    /// Selector expressions become ordinary dependencies of the point.
    pub(super) fn selected(
        c: &mut Construction,
        args: &[&str],
    ) -> Option<Result<Vec<Self>, String>> {
        let [first, second, selector, tail @ ..] = args else {
            return None;
        };
        let (function, other) = Self::pair(c, first, second)?;
        let method = match tail {
            [] => {
                if let Some(value) = crate::expr::plain_number(selector) {
                    Ok(Method::ConstantIndex(index(value)))
                } else {
                    c.store_intersection_argument(selector)
                        .and_then(|(id, kind)| match kind {
                            Kind::Point if !CONIC_OBJECTS.contains(&c.kind(other)) => {
                                Ok(Method::Seed {
                                    point: id,
                                    line_first: c.id(first) == Some(other),
                                })
                            }
                            kind if kind.is_number() => Ok(Method::Indexed(id)),
                            _ => Err(format!("invalid intersection selector `{selector}`")),
                        })
                }
            }
            [right] if c.kind(other) == Kind::Function => (|| {
                let (left, left_kind) = c.store_intersection_argument(selector)?;
                let (right, right_kind) = c.store_intersection_argument(right)?;
                if !(left_kind.is_number() && right_kind.is_number()) {
                    return Err("intersection interval bounds must be numbers".into());
                }
                Ok(Method::Interval {
                    left,
                    right,
                    slot: 0,
                })
            })(),
            _ => Err("invalid function intersection arguments".into()),
        };
        Some(method.and_then(|method| {
            let def = Self {
                function,
                other,
                method,
            };
            if matches!(method, Method::ConstantIndex(_) | Method::Indexed(_))
                && !CONIC_OBJECTS.contains(&c.kind(other))
                && (!polynomial(c, function)
                    || c.kind(other) == Kind::Function && !polynomial(c, other))
            {
                return Err("an intersection index requires polynomial functions".into());
            }
            Ok(if matches!(method, Method::Interval { .. }) {
                def.outputs(c)
            } else {
                vec![def]
            })
        }))
    }

    fn arguments(self) -> impl Iterator<Item = u32> {
        match self.method {
            Method::Seed { point, .. } | Method::Indexed(point) => [Some(point), None],
            Method::Interval { left, right, .. } => [Some(left), Some(right)],
            _ => [None, None],
        }
        .into_iter()
        .flatten()
    }

    pub(super) fn hidden_parents(self, c: &Construction) -> Vec<Id> {
        self.arguments()
            .filter_map(|arg| c.intersection_argument_parents(arg))
            .flatten()
            .copied()
            .collect()
    }

    fn value(self, c: &Construction, x: f64) -> f64 {
        c.function_value(self.function, x).unwrap_or(f64::NAN)
    }

    fn points(self, c: &Construction) -> Vec<(f64, f64)> {
        let other = c.kind(self.other);
        if CONIC_OBJECTS.contains(&other) {
            return self.conic_points(c);
        }
        let f = |x| self.value(c, x);
        if other == Kind::Function {
            let g = |x| c.function_value(self.other, x).unwrap_or(f64::NAN);
            let difference = |x| f(x) - g(x);
            match self.method {
                Method::Interval { left, right, .. } => {
                    let (Some(left), Some(right)) = (number(c, left), number(c, right)) else {
                        return Vec::new();
                    };
                    interval_roots(&difference, left, right)
                }
                Method::Seed { .. } | Method::Automatic => self
                    .seed(c)
                    .and_then(|seed| {
                        root_near(
                            &difference,
                            |x| {
                                c.function_derivative(self.function, x)
                                    .zip(c.function_derivative(self.other, x))
                                    .map_or_else(|| curve::slope(difference, x), |(a, b)| a - b)
                            },
                            seed,
                        )
                    })
                    .into_iter()
                    .collect(),
                _ => match (
                    c.function_fraction(self.function),
                    c.function_fraction(self.other),
                ) {
                    (Some((p, q)), Some((r, s))) => {
                        graph_roots(&subtract(product(&p, &s), product(&r, &q)))
                            .into_iter()
                            .filter(|&x| difference(x).abs() < MIN_PRECISION)
                            .collect()
                    }
                    _ => Vec::new(),
                },
            }
            .into_iter()
            .filter_map(|x| finite_point(x, f(x)))
            .collect()
        } else {
            self.line_points(c, &f)
        }
    }

    fn seed(self, c: &Construction) -> Option<f64> {
        match self.method {
            Method::Seed { point, .. } => match c.intersection_argument_value(point)? {
                Raw::Vec(x, y) if x.is_finite() && y.is_finite() => Some(x),
                _ => None,
            },
            _ => Some(0.0),
        }
    }

    fn line_points(self, c: &Construction, f: &impl Fn(f64) -> f64) -> Vec<(f64, f64)> {
        let Some(line) = Carrier::of(c, self.other) else {
            return Vec::new();
        };
        let (a, b, d) = line.line;
        let vertical = b.abs() < STANDARD_PRECISION;
        let numeric = matches!(self.method, Method::Seed { .. } | Method::Automatic);
        let line_value = |x: f64| -(a * x + d) / b;
        let difference = |x| f(x) - line_value(x);
        let roots = if numeric {
            let Some(seed) = self.seed(c) else {
                return Vec::new();
            };
            let root = if vertical {
                check_root(-d / a, f)
            } else {
                root_near(
                    &difference,
                    |x| {
                        c.function_derivative(self.function, x)
                            .map_or_else(|| curve::slope(difference, x), |v| v + a / b)
                    },
                    seed,
                )
                .and_then(|x| {
                    // CmdIntersect reaches function/function before the
                    // reversed line form for a line that is a function of x.
                    if matches!(
                        self.method,
                        Method::Seed {
                            line_first: true,
                            ..
                        }
                    ) {
                        Some(x)
                    } else {
                        check_root(x, &difference)
                    }
                })
            };
            root.into_iter().collect()
        } else if vertical {
            vec![-d / a]
        } else if let Some((p, q)) = c.function_fraction(self.function) {
            graph_roots(&subtract(p, product(&[-d / b, -a / b], &q)))
        } else {
            return Vec::new();
        };
        roots
            .into_iter()
            .filter_map(|x| {
                let y = f(x);
                if !line.incident((x, y), MIN_PRECISION) {
                    return None;
                }
                finite_point(
                    x,
                    if numeric && !vertical {
                        line_value(x)
                    } else {
                        y
                    },
                )
            })
            .collect()
    }

    fn conic_points(self, c: &Construction) -> Vec<(f64, f64)> {
        let part = c.arc(self.other);
        let Some(conic) = part.map(|p| p.conic).or_else(|| c.conic(self.other)) else {
            return Vec::new();
        };
        if !conic.defined || part.is_some_and(|p| !p.defined) {
            return Vec::new();
        }
        let [xx, yy, constant, xy, linear_x, linear_y] = conic.matrix;
        let f = |x| self.value(c, x);
        let roots = if let Some((p, q)) = c
            .function_fraction(self.function)
            .filter(|(_, q)| q.len() == 1)
        {
            let y: Vec<_> = p.into_iter().map(|a| a / q[0]).collect();
            let mut residual = vec![constant, 2.0 * linear_x, xx];
            add(&mut residual, product(&y, &[2.0 * linear_y, 2.0 * xy]));
            add(&mut residual, product(&y, &product(&y, &[yy])));
            let mut roots = graph_roots(&residual);
            roots.dedup_by(|a, b| (*a - *b).abs() <= MIN_PRECISION);
            roots
        } else {
            let (left, right) = if matches!(conic.ty, ConicType::Circle | ConicType::Ellipse) {
                let radius = 1.05 * conic.half_axes[0].max(conic.half_axes[1]);
                (conic.midpoint[0] - radius, conic.midpoint[0] + radius)
            } else {
                // GeoFunction.getMinParameter/getMaxParameter in the headless view.
                (-4.3 * 1.5, 11.700000000000001 * 1.5)
            };
            interval_roots(
                &|x| {
                    let y = f(x);
                    xx * x * x
                        + yy * y * y
                        + constant
                        + 2.0 * (xy * x * y + linear_x * x + linear_y * y)
                },
                left,
                right,
            )
        };
        // The solver uses the full conic even when the input is a conic part.
        let mut points = Vec::with_capacity(roots.len());
        for (x, y) in roots.into_iter().filter_map(|x| finite_point(x, f(x))) {
            if points
                .iter()
                .all(|&(a, b): &(f64, f64)| (x - a).powi(2) + (y - b).powi(2) >= STANDARD_PRECISION)
            {
                points.push((x, y));
            }
        }
        points
    }
}

impl Group for FunctionIntersection {
    fn kind(&self) -> Kind {
        Kind::Point
    }

    fn parents(&self) -> Parents {
        Parents::of(&[self.function, self.other])
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        c.expect(self.function, FUNCTION)?;
        for argument in self.arguments() {
            c.intersection_argument_parents(argument)
                .ok_or(Error::UnknownObject)?;
        }
        match c.kind(self.other) {
            Kind::Function => c.expect(self.other, FUNCTION),
            kind if CARRIERS.contains(&kind) => c.expect(self.other, CARRIERS),
            _ => c.expect(self.other, CONIC_OBJECTS),
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        let slot = match self.method {
            Method::Indexed(id) => index(number(c, id).unwrap_or(f64::NAN)),
            Method::ConstantIndex(slot)
            | Method::Interval { slot, .. }
            | Method::Polynomial(slot)
            | Method::Conic(slot) => slot,
            Method::Seed { .. } | Method::Automatic => 0,
        };
        let (x, y) = *self.points(c).get(slot as usize)?;
        Some(Value::Point { x, y })
    }

    fn parse(_: &Construction, _: &str, _: &[&str]) -> Option<Result<Self, String>> {
        None
    }
}

fn number(c: &Construction, selector: u32) -> Option<f64> {
    match c.intersection_argument_value(selector)? {
        Raw::Num(n) => Some(n),
        _ => None,
    }
}

fn index(n: f64) -> u32 {
    (n as i32).wrapping_sub(1).max(0) as u32
}

fn polynomial(c: &Construction, id: Id) -> bool {
    c.function_fraction(id).is_some_and(|(_, q)| q.len() == 1)
}

fn finite_point(x: f64, y: f64) -> Option<(f64, f64)> {
    (x.is_finite() && y.is_finite()).then_some((x + 0.0, y + 0.0))
}

fn product(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; a.len() + b.len() - 1];
    for (i, a) in a.iter().enumerate() {
        for (j, b) in b.iter().enumerate() {
            out[i + j] += a * b;
        }
    }
    out
}

fn add(a: &mut Vec<f64>, b: Vec<f64>) {
    a.resize(a.len().max(b.len()), 0.0);
    for (a, b) in a.iter_mut().zip(b) {
        *a += b;
    }
}

fn subtract(mut a: Vec<f64>, b: Vec<f64>) -> Vec<f64> {
    add(&mut a, b.into_iter().map(|b| -b).collect());
    a
}
