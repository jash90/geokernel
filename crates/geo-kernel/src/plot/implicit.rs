//! Screen-space marching squares for the zero contour of a polynomial.

use super::{Frame, Path};

pub fn evaluate(coefficients: &[Vec<f64>], x: f64, y: f64) -> f64 {
    coefficients.iter().rev().fold(0.0_f64, |value, row| {
        value.mul_add(
            x,
            row.iter()
                .rev()
                .fold(0.0_f64, |value, &c| value.mul_add(y, c)),
        )
    })
}

/// Returns the piecewise linear approximation to the visible contour.
/// Adjacent cells reuse their shared edge endpoints.
pub fn plot(coefficients: &[Vec<f64>], frame: &Frame) -> Vec<Path> {
    if !frame.resolves() || coefficients.iter().flatten().all(|&c| c == 0.0) {
        return Vec::new();
    }
    const MAX_CELLS_PER_AXIS: usize = 112;
    let cells = |pixels: f64| ((pixels / 7.0).ceil() as usize).clamp(1, MAX_CELLS_PER_AXIS);
    let (mut nx, mut ny) = (cells(frame.width()), cells(frame.height()));
    const MAX_COEFFICIENT_EVALUATIONS: usize = 750_000;
    let cost = coefficients.iter().map(Vec::len).sum::<usize>().max(1);
    // A cell can evaluate its centre in addition to the shared grid.
    let samples = MAX_COEFFICIENT_EVALUATIONS / cost / 2;
    let ratio = (samples as f64 / ((nx + 1) * (ny + 1)) as f64).sqrt();
    if ratio < 1.0 {
        nx = ((nx as f64 * ratio) as usize).max(1);
        ny = ((ny as f64 * ratio) as usize).max(1);
    }
    let point = |i, j| {
        (
            frame.width() * i as f64 / nx as f64,
            frame.height() * j as f64 / ny as f64,
        )
    };
    let value = |(sx, sy)| {
        let (x, y) = frame.screen_to_world(sx, sy);
        evaluate(coefficients, x, y)
    };
    let stride = nx + 1;
    let mut values = Vec::with_capacity(stride * (ny + 1));
    for j in 0..=ny {
        values.extend((0..=nx).map(|i| value(point(i, j))));
    }

    let vertices = values.len();
    let mut edges = vec![None; 2 * vertices];
    let mut links = vec![[None; 2]; edges.len()];
    for j in 0..ny {
        for i in 0..nx {
            let k = j * stride + i;
            let corners = [k, k + 1, k + stride + 1, k + stride];
            let v = corners.map(|index| values[index]);
            if v.iter().any(|n| !n.is_finite()) {
                continue;
            }
            // Horizontal edges use even slots, vertical edges odd slots.
            let slots = [2 * k, 2 * (k + 1) + 1, 2 * (k + stride), 2 * k + 1];
            let mut crossings = [0; 4];
            let mut len = 0;
            for e in 0..4 {
                let next = (e + 1) % 4;
                if (v[e] < 0.0) == (v[next] < 0.0) {
                    continue;
                }
                let slot = slots[e];
                crossings[len] = slot;
                len += 1;
                edges[slot].get_or_insert_with(|| {
                    let at = |n| point(n % stride, n / stride);
                    let (a, b) = (at(corners[e]), at(corners[next]));
                    let (va, vb) = (v[e].abs(), v[next].abs());
                    let scale = va.max(vb);
                    let t = (va / scale) / (va / scale + vb / scale);
                    (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))
                });
            }
            let mut join = |a: usize, b: usize| {
                let (a, b) = (crossings[a], crossings[b]);
                for (from, to) in [(a, b), (b, a)] {
                    if let Some(slot) = links[from].iter_mut().find(|slot| slot.is_none()) {
                        *slot = Some(to);
                    }
                }
            };
            match len {
                2 => join(0, 1),
                4 => {
                    let a = point(i, j);
                    let b = point(i + 1, j + 1);
                    if (value(((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5)) < 0.0) == (v[0] < 0.0) {
                        join(0, 1);
                        join(2, 3);
                    } else {
                        join(0, 3);
                        join(1, 2);
                    }
                }
                _ => {}
            }
        }
    }

    let mut paths = Vec::new();
    let mut seen = vec![false; edges.len()];
    // Start open paths at their endpoints, then walk closed paths.
    for closed in [false, true] {
        for start in 0..edges.len() {
            if seen[start] || edges[start].is_none() || closed != links[start][1].is_some() {
                continue;
            }
            let mut path = Vec::new();
            let mut previous = None;
            let mut current = start;
            for _ in 0..edges.len() {
                let Some(vertex) = edges[current] else { break };
                path.push(vertex);
                if seen[current] {
                    break;
                }
                seen[current] = true;
                let Some(next) = links[current]
                    .into_iter()
                    .flatten()
                    .find(|&n| Some(n) != previous)
                else {
                    break;
                };
                previous = Some(current);
                current = next;
            }
            if path.len() > 1 {
                paths.push(path);
            }
        }
    }
    paths
}

/// Tests for a zero crossing in a small world-coordinate rectangle.
pub fn meets_box(coefficients: &[Vec<f64>], lo: (f64, f64), hi: (f64, f64)) -> bool {
    let mut positive = false;
    let mut negative = false;
    for j in 0..=4 {
        for i in 0..=4 {
            let v = evaluate(
                coefficients,
                lo.0 + (hi.0 - lo.0) * f64::from(i) / 4.0,
                lo.1 + (hi.1 - lo.1) * f64::from(j) / 4.0,
            );
            positive |= v.is_finite() && v > 0.0;
            negative |= v.is_finite() && v < 0.0;
        }
    }
    positive && negative
}
