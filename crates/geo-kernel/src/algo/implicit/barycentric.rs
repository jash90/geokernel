// The cubic rules follow reference commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/barycentric/AlgoCubic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/barycentric/AlgoCubicSwitch.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/barycentric/CmdCubic.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use std::f64::consts::TAU;
use std::sync::OnceLock;

use crate::algo::conic::scaled_length;
use crate::{Construction, Id, Kind};

/// Each coordinate is stored as [constant, x coefficient, y coefficient].
pub(crate) fn coordinates(points: [(f64, f64); 3]) -> Option<[[f64; 3]; 3]> {
    let [(ax, ay), (bx, by), (cx, cy)] = points;
    let det = (cx - bx) * (ay - cy) + (ax - cx) * (by - cy);
    if !det.is_finite() || det.abs() < crate::tol::STANDARD_PRECISION {
        return None;
    }
    Some(std::array::from_fn(|i| {
        let (u, v) = points[(i + 1) % 3];
        let (s, t) = points[(i + 2) % 3];
        let dx = v - t;
        let dy = s - u;
        [-(dx * s + dy * t) / det, dx / det, dy / det]
    }))
}

pub(crate) fn argument(c: &mut Construction, text: &str, kind: Kind) -> Result<Id, String> {
    let matches = |actual| actual == kind || (kind == Kind::Number && actual == Kind::Angle);
    if let Some(id) = c.id(text).filter(|&id| matches(c.kind(id))) {
        return Ok(id);
    }
    let label = format!("\0implicit{}", c.len());
    if crate::expr::argument_object(c, text, &label, kind) {
        return Ok(c.id(&label).expect("resolved argument"));
    }
    Err(format!("expected {kind:?}, got `{text}`"))
}

pub(crate) struct Cubic {
    pub(crate) coordinates: [[f64; 3]; 3],
    /// For each pair of consecutive coordinates p and q, the weights
    /// of p³, p²q and q²p.
    pub(crate) cyclic: [[f64; 3]; 3],
    pub(crate) central: f64,
}

pub(crate) enum CubicError {
    Undefined,
    InvalidCatalogue,
}

pub(crate) fn cubic(c: &Construction, [a, b, d, n]: [Id; 4]) -> Result<Cubic, CubicError> {
    use CubicError::Undefined;

    let [Some(a), Some(b), Some(d)] = [a, b, d].map(|id| c.point(id)) else {
        return Err(Undefined);
    };
    let points = [a, b, d];
    let barycentric = coordinates(points).ok_or(Undefined)?;
    if barycentric.iter().flatten().any(|value| !value.is_finite()) {
        return Err(Undefined);
    }
    // Number and angle objects both expose a scalar. Angles use radians.
    let index = c.number(n).filter(|n| n.is_finite()).ok_or(Undefined)? as i32;
    let entry = CatalogueEntry::find(index).ok_or(Undefined)?;
    let distance = |i: usize, j: usize| {
        let (x, y) = points[i];
        let (u, v) = points[j];
        scaled_length(x - u, y - v)
    };
    let sides = [distance(1, 2), distance(2, 0), distance(0, 1)];
    let mut cyclic = [[0.0; 3]; 3];
    let mut central = 0.0;
    for (i, weights) in cyclic.iter_mut().enumerate() {
        let j = (i + 1) % 3;
        let k = (i + 2) % 3;
        let (a, b, c) = (sides[i], sides[j], sides[k]);
        let [vertex, mixed, product] = entry.coefficients([a, b, c]).ok_or(Undefined)?;
        let [_, swapped, _] = entry.coefficients([b, a, c]).ok_or(Undefined)?;
        *weights = [vertex, mixed, entry.symmetry * swapped];
        if i == 0 {
            central = product;
        }
    }
    let mut weights = cyclic.iter().flatten().chain([&central]);
    if weights.clone().any(|value| value.is_nan()) {
        return Err(CubicError::InvalidCatalogue);
    }
    if weights.any(|value| !value.is_finite()) {
        return Err(Undefined);
    }
    Ok(Cubic {
        coordinates: barycentric,
        cyclic,
        central,
    })
}

#[derive(Clone, Copy)]
struct CatalogueEntry {
    index: i32,
    symmetry: f64,
    coefficients: [&'static str; 3],
}

impl CatalogueEntry {
    fn find(index: i32) -> Option<Self> {
        // Three angle trisections, in order of their phase.
        if (29..=31).contains(&index) {
            return Some(Self {
                index,
                symmetry: -1.0,
                coefficients: ["0"; 3],
            });
        }
        static CATALOGUE: OnceLock<Vec<CatalogueEntry>> = OnceLock::new();
        let table = CATALOGUE.get_or_init(|| {
            include_str!("cubic_catalogue.tsv")
                .lines()
                .take(700)
                .filter(|line| !line.starts_with('#'))
                .map(|line| {
                    let mut cells = line.split('\t');
                    let index = cells.next().unwrap().parse().unwrap();
                    let symmetry = if cells.next() == Some("-") { -1.0 } else { 1.0 };
                    let coefficients = std::array::from_fn(|_| cells.next().unwrap());
                    CatalogueEntry {
                        index,
                        symmetry,
                        coefficients,
                    }
                })
                .collect()
        });
        Some(
            table[table
                .binary_search_by_key(&index, |entry| entry.index)
                .ok()?],
        )
    }

    fn coefficients(&self, [a, b, c]: [f64; 3]) -> Option<[f64; 3]> {
        if let Some(phase) = match self.index {
            29 => Some(0.0),
            30 => Some(TAU),
            31 => Some(-TAU),
            _ => None,
        } {
            let cosine = |opposite: f64, u: f64, v: f64| {
                let angle = ((u * u + v * v - opposite * opposite) / (2.0 * u * v)).acos();
                ((angle + phase) / 3.0).cos()
            };
            let mixed = 2.0 * b * (c * c) * cosine(c, a, b) * cosine(a, b, c)
                + b * (c * c) * cosine(b, a, c);
            return Some([0.0, mixed, 0.0]);
        }
        let sides = [side_powers(a), side_powers(b), side_powers(c)];
        let [aa, bb, cc] = sides.map(|powers| powers[2]);
        let angle = |opposite, other, adjacent, u, v| {
            let quotient: f64 = (other + adjacent - opposite) / 2.0 / u / v;
            (quotient.acos() / 3.0).cos()
        };
        let third_angles = [
            angle(aa, bb, cc, b, c),
            angle(bb, aa, cc, a, c),
            angle(cc, bb, aa, a, b),
        ];
        let area = ((a + b - c) * (a - b + c) * (-a + b + c) * (a + b + c)).sqrt();
        let sqrt_three = 3.0_f64.sqrt();
        let factor = |symbol: &str| match symbol {
            "S" => Some(area),
            "r3" => Some(sqrt_three),
            "cA3" => Some(third_angles[0]),
            "cB3" => Some(third_angles[1]),
            "cC3" => Some(third_angles[2]),
            _ => {
                let (name, exponent) = symbol.split_at_checked(1)?;
                let side = match name {
                    "a" => 0,
                    "b" => 1,
                    "c" => 2,
                    _ => return symbol.parse().ok(),
                };
                let power = if exponent.is_empty() {
                    1
                } else {
                    exponent.parse().ok()?
                };
                sides[side].get(power).copied()
            }
        };
        let mut result = [0.0; 3];
        for (sum, formula) in result.iter_mut().zip(self.coefficients) {
            let mut terms = formula.split_ascii_whitespace();
            for term in terms.by_ref().take(160) {
                let sign = if term.starts_with('-') { -1.0 } else { 1.0 };
                let mut factors = term.trim_start_matches(['+', '-']).split('*');
                let mut value = sign;
                for symbol in factors.by_ref().take(9) {
                    value *= factor(symbol)?;
                }
                if factors.next().is_some() {
                    return None;
                }
                *sum += value;
            }
            if terms.next().is_some() {
                return None;
            }
        }
        Some(result)
    }
}

fn side_powers(side: f64) -> [f64; 21] {
    let mut powers = [1.0; 21];
    powers[1] = side;
    for (power, a, b) in [
        (2, 1, 1),
        (3, 1, 2),
        (4, 2, 2),
        (5, 2, 3),
        (6, 3, 3),
        (7, 3, 4),
        (8, 4, 4),
        (9, 4, 5),
        (10, 6, 4),
        (12, 6, 6),
        (14, 6, 8),
        (16, 8, 8),
        (18, 10, 8),
        (20, 10, 10),
    ] {
        powers[power] = powers[a] * powers[b];
    }
    powers
}
