//! Classical RK4 and the Dormand–Prince 5(4) embedded pair.
//!
//! The integrators use the published Runge–Kutta formulas and are checked
//! against reference traces, including the integration times and endpoints.

use super::{LocusPoint, MAX_ODE_POINTS, MAX_SYSTEM_SIZE};
use crate::expr::IntersectionWork;

pub(super) fn classical<const N: usize>(
    start: f64,
    end: f64,
    step: f64,
    mut state: [f64; N],
    rhs: impl Fn(f64, [f64; N]) -> [f64; N],
    point: impl Fn(f64, [f64; N], bool) -> LocusPoint,
    work: &IntersectionWork,
) -> Option<Vec<LocusPoint>> {
    if !step.is_finite() || step.abs() < crate::tol::STANDARD_PRECISION {
        return None;
    }
    let h = step.abs().copysign(end - start);
    let mut t = start;
    let mut points = vec![point(t, state, false)];
    for _ in 1..MAX_ODE_POINTS {
        if t == end {
            return Some(points);
        }
        if !work.charge(4 * N as u64) {
            return None;
        }
        let next = if h > 0.0 {
            (t + h).min(end)
        } else {
            (t + h).max(end)
        };
        if next == t {
            return None;
        }
        let h = if next == end { next - t } else { h };
        let k1 = rhs(t, state);
        let k2 = rhs(
            t + h * 0.5,
            std::array::from_fn(|i| state[i] + h * (0.5 * k1[i])),
        );
        let k3 = rhs(
            t + h * 0.5,
            std::array::from_fn(|i| state[i] + h * (0.5 * k2[i])),
        );
        let k4 = rhs(t + h, std::array::from_fn(|i| state[i] + h * k3[i]));
        for i in 0..N {
            state[i] += h * (k1[i] / 6.0 + k2[i] / 3.0 + k3[i] / 3.0 + k4[i] / 6.0);
        }
        t = next;
        points.push(point(t, state, true));
    }
    None
}

const MIN_STEP: f64 = 0.001;
const MAX_STEP: f64 = 0.01;
const ABSOLUTE_ERROR: f64 = 1e-6;
const RELATIVE_ERROR: f64 = 1e-4;
const SAFETY: f64 = 0.9;

const A: [&[f64]; 6] = [
    &[1.0 / 5.0],
    &[3.0 / 40.0, 9.0 / 40.0],
    &[44.0 / 45.0, -56.0 / 15.0, 32.0 / 9.0],
    &[
        19372.0 / 6561.0,
        -25360.0 / 2187.0,
        64448.0 / 6561.0,
        -212.0 / 729.0,
    ],
    &[
        9017.0 / 3168.0,
        -355.0 / 33.0,
        46732.0 / 5247.0,
        49.0 / 176.0,
        -5103.0 / 18656.0,
    ],
    &[
        35.0 / 384.0,
        0.0,
        500.0 / 1113.0,
        125.0 / 192.0,
        -2187.0 / 6784.0,
        11.0 / 84.0,
    ],
];
const C: [f64; 6] = [1.0 / 5.0, 3.0 / 10.0, 4.0 / 5.0, 8.0 / 9.0, 1.0, 1.0];
const E: [f64; 7] = [
    71.0 / 57600.0,
    0.0,
    -71.0 / 16695.0,
    71.0 / 1920.0,
    -17253.0 / 339200.0,
    22.0 / 525.0,
    -1.0 / 40.0,
];

pub(super) fn adaptive(
    start: f64,
    end: f64,
    initial: &[f64],
    rhs: impl Fn(f64, &[f64], &mut [f64]),
    slot: usize,
    work: &IntersectionWork,
) -> Option<Vec<LocusPoint>> {
    let n = initial.len();
    if start == end || initial.iter().any(|y| !y.is_finite()) {
        return None;
    }
    let mut t = start;
    let mut state = [0.0; MAX_SYSTEM_SIZE];
    state[..n].copy_from_slice(initial);
    let mut stages = [[0.0; MAX_SYSTEM_SIZE]; 7];
    rhs(t, &state[..n], &mut stages[0][..n]);
    let direction = (end - start).signum();
    let mut trial = [0.0; MAX_SYSTEM_SIZE];
    let mut h = initial_step(t, direction, &state[..n], &stages[0][..n], &rhs);
    let mut points = vec![LocusPoint {
        x: t,
        y: state[slot],
        line_to: false,
    }];
    for _ in 1..MAX_ODE_POINTS {
        if !work.charge((7 * n) as u64) {
            return None;
        }
        let remaining = end - t;
        let last = h.abs() >= remaining.abs();
        if last {
            h = remaining;
        }
        let next = if last { end } else { t + h };
        if next == t {
            return None;
        }
        for (k, weights) in A.iter().enumerate() {
            for i in 0..n {
                let sum: f64 = weights
                    .iter()
                    .enumerate()
                    .map(|(j, a)| a * stages[j][i])
                    .sum();
                trial[i] = state[i] + h * sum;
            }
            rhs(t + C[k] * h, &trial[..n], &mut stages[k + 1][..n]);
        }
        let err = ((0..n)
            .map(|i| {
                let weighted: f64 = E.iter().enumerate().map(|(j, a)| a * stages[j][i]).sum();
                let tolerance =
                    ABSOLUTE_ERROR + RELATIVE_ERROR * state[i].abs().max(trial[i].abs());
                (h * weighted / tolerance).powi(2)
            })
            .sum::<f64>()
            / n as f64)
            .sqrt();
        if !err.is_finite() {
            return None;
        }
        let proposed = h.abs() * (SAFETY * err.powf(-0.2)).clamp(0.2, 10.0);
        if err < 1.0 {
            state[..n].copy_from_slice(&trial[..n]);
            stages[0] = stages[6];
            t = next;
            points.push(LocusPoint {
                x: t,
                y: state[slot],
                line_to: true,
            });
            if last {
                return Some(points);
            }
        } else if proposed < MIN_STEP {
            return None;
        }
        h = direction * proposed.clamp(MIN_STEP, MAX_STEP);
    }
    None
}

fn initial_step(
    t: f64,
    direction: f64,
    y: &[f64],
    dy: &[f64],
    rhs: &impl Fn(f64, &[f64], &mut [f64]),
) -> f64 {
    let n = y.len();
    let scale = |i: usize| ABSOLUTE_ERROR + RELATIVE_ERROR * y[i].abs();
    let y_norm = (0..n).map(|i| (y[i] / scale(i)).powi(2)).sum::<f64>();
    let dy_norm = (0..n).map(|i| (dy[i] / scale(i)).powi(2)).sum::<f64>();
    let h = direction
        * if y_norm < 1e-10 || dy_norm < 1e-10 {
            1e-6
        } else {
            0.01 * (y_norm / dy_norm).sqrt()
        };
    let mut trial = [0.0; MAX_SYSTEM_SIZE];
    let mut derivative = [0.0; MAX_SYSTEM_SIZE];
    for i in 0..n {
        trial[i] = y[i] + h * dy[i];
    }
    rhs(t + h, &trial[..n], &mut derivative[..n]);
    let second = (0..n)
        .map(|i| ((derivative[i] - dy[i]) / scale(i)).powi(2))
        .sum::<f64>()
        .sqrt()
        / h.abs();
    let maximum = dy_norm.sqrt().max(second);
    let step = if maximum < 1e-15 {
        1e-6_f64.max(h.abs() * 0.001)
    } else {
        (0.01 / maximum).powf(0.2)
    };
    let step = step.min(100.0 * h.abs()).max(1e-12 * t.abs());
    direction * step.clamp(MIN_STEP, MAX_STEP)
}
