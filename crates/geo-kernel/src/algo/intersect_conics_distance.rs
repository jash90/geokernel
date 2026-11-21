//! Point-to-conic distances used to validate decimal snapping.

// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/EquationSolver.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::roots::{cubic, quadratic};
use crate::algo::conic_nd::{my_length, ConicData, ConicType};
use crate::tol::STANDARD_PRECISION;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

pub(super) fn distance(conic: &ConicData, point: (f64, f64)) -> f64 {
    let (x, y) = point;
    let [mx, my] = conic.midpoint;
    let (qx, qy) = match conic.ty {
        ConicType::SinglePoint => (mx, my),
        ConicType::IntersectingLines | ConicType::ParallelLines => {
            let [first, second] = conic.lines;
            let offset = |[a, b, c]: [f64; 3]| ((a * x + b * y + c) / my_length(a, b)).abs();
            let line = if offset(first) <= offset(second) {
                first
            } else {
                second
            };
            project_line(line, point)
        }
        ConicType::DoubleLine => project_line(conic.lines[0], point),
        ConicType::Ellipse | ConicType::Circle | ConicType::Hyperbola | ConicType::Parabola => {
            let (px, py) = (x - mx, y - my);
            let [[ex, ey], [fx, fy]] = conic.eigenvec;
            let (px, py) = (px * ex + py * ey, px * fx + py * fy);
            let (qx, qy) = project_curved(conic, px, py);
            (qx * ex + qy * fx + mx, qx * ey + qy * fy + my)
        }
        _ => return f64::NAN,
    };
    my_length(x - qx, y - qy)
}

fn project_line([a, b, c]: [f64; 3], (x, y): (f64, f64)) -> (f64, f64) {
    let t = -(c + a * x + b * y) / (a * a + b * b);
    (x + t * a, y + t * b)
}

fn project_curved(conic: &ConicData, x: f64, y: f64) -> (f64, f64) {
    let [a, b] = conic.half_axes;
    let (u, v) = (x.abs(), y.abs());
    let eps = STANDARD_PRECISION;
    match conic.ty {
        ConicType::Circle => {
            let t = (a * y).atan2(b * x);
            (a * t.cos(), b * t.sin())
        }
        ConicType::Ellipse => {
            let c2 = a * a - b * b;
            let mut t = if u < eps {
                if b < a || v >= eps && b * v >= c2 {
                    FRAC_PI_2
                } else if v < eps {
                    0.0
                } else {
                    (b * v / c2).asin()
                }
            } else if v < eps {
                if a < b || a * u >= c2 {
                    0.0
                } else {
                    (a * u / c2).acos()
                }
            } else {
                perpendicular(a, b, u, v, false).asin()
            };
            if x < 0.0 {
                t = PI - t;
            }
            if y < 0.0 {
                t = -t;
            }
            (a * t.cos(), b * t.sin())
        }
        ConicType::Hyperbola => {
            let c2 = a * a + b * b;
            let mut t = if v < eps {
                (a * u / c2).max(1.0).acosh()
            } else if u < eps {
                (b * v / c2).asinh()
            } else {
                perpendicular(a, b, u, v, true).asinh()
            };
            if y < 0.0 {
                t = -t;
            }
            let qx = a * t.cosh();
            (if x < 0.0 { -qx } else { qx }, b * t.sinh())
        }
        _ => {
            let p = conic.p;
            let t = if v < eps {
                (2.0 * (x - p) / p).max(0.0).sqrt()
            } else {
                let (roots, _) = cubic([v, -p + x, 0.0, -p / 2.0], eps);
                let t = first_positive(roots);
                if y < 0.0 {
                    -t
                } else {
                    t
                }
            };
            (p * t * t / 2.0, p * t)
        }
    }
}

fn first_positive<const N: usize>(roots: [f64; N]) -> f64 {
    roots.into_iter().find(|v| *v > 0.0).unwrap_or(roots[N - 1])
}

fn perpendicular(a: f64, b: f64, x: f64, y: f64, hyperbola: bool) -> f64 {
    let by = b * y;
    let (c2, sign) = if hyperbola {
        (a * a + b * b, 1.0)
    } else {
        (a * a - b * b, -1.0)
    };
    first_positive(normal_quartic([
        by * by,
        -sign * 2.0 * by * c2,
        sign * by * by + c2 * c2 - a * a * x * x,
        -2.0 * by * c2,
        sign * c2 * c2,
    ]))
}

/// The analytic quartic used for perpendicular feet. The reference keeps
/// its analytic output order because the closest-point method takes the
/// first positive root.
fn normal_quartic([d, c, b, a, leading]: [f64; 5]) -> [f64; 4] {
    let (a, b, c, d) = (a / leading, b / leading, c / leading, d / leading);
    if b == 0.0 && c == 0.0 {
        if d == 0.0 {
            return if a > 0.0 {
                [-a, 0.0, 0.0, 0.0]
            } else {
                [0.0, 0.0, 0.0, -a]
            };
        }
        if a == 0.0 {
            return if d > 0.0 {
                [0.0; 4]
            } else {
                let root = (-d).sqrt().sqrt();
                [root, -root, 0.0, 0.0]
            };
        }
    }
    if c == 0.0 && d == 0.0 {
        let (roots, count) = quadratic([b, a, 1.0], STANDARD_PRECISION);
        let [first, second, _] = roots;
        return [
            0.0,
            0.0,
            if count == 0 { b } else { first },
            if count < 2 { a } else { second },
        ];
    }

    let aa = a * a;
    let p = b - 3.0 / 8.0 * aa;
    let q = c - 0.5 * a * (b - 0.25 * aa);
    let r = d - 0.25 * (a * c - 0.25 * aa * (b - 3.0 / 16.0 * aa));
    let (rc, sc, tc) = (0.5 * p, 0.25 * (0.25 * p * p - r), -(0.125 * q * 0.125 * q));
    let qcub = rc * rc - 3.0 * sc;
    let rcub = 2.0 * rc * rc * rc - 9.0 * rc * sc + 27.0 * tc;
    let (qc, rr) = (qcub / 9.0, rcub / 54.0);
    let cr = 729.0 * rcub * rcub;
    let cq = 2916.0 * qcub * qcub * qcub;
    let disc = (cr - cq) / 2125764.0;

    let mut u = if qc == 0.0 && rr == 0.0 {
        [-rc / 3.0; 3]
    } else if cr == cq {
        let sqrt = qc.sqrt();
        let shift = rc / 3.0;
        if rr > 0.0 {
            [-2.0 * sqrt - shift, sqrt - shift, sqrt - shift]
        } else {
            [-sqrt - shift, -sqrt - shift, 2.0 * sqrt - shift]
        }
    } else if cr < cq {
        let sqrt = qc.sqrt();
        let t = rr / (sqrt * sqrt * sqrt);
        let angle = if t >= 1.0 { 0.0 } else { t.acos() };
        [angle, angle + TAU, angle - TAU].map(|angle| -2.0 * sqrt * (angle / 3.0).cos() - rc / 3.0)
    } else {
        let sqrt_disc = (rr * rr - qc * qc * qc).sqrt();
        let p = -(rr.abs() + sqrt_disc).powf(1.0 / 3.0).copysign(rr);
        let q = qc / p;
        [
            p + q - rc / 3.0,
            -0.5 * (p + q) - rc / 3.0,
            -(3.0_f64.sqrt() / 2.0) * (p - q).abs(),
        ]
    };
    let (w1r, w1i, w2r, w2i) = if disc <= 0.0 {
        if disc == 0.0 {
            u[2] = u[1];
        }
        let [v0, v1, v2] = u.map(f64::abs);
        let largest = v0.max(v1).max(v2);
        let (first, second_value) = if largest == v0 {
            (0, v1.max(v2))
        } else if largest == v1 {
            (1, v0.max(v2))
        } else {
            (2, v0.max(v1))
        };
        let second = if second_value == v0 {
            0
        } else if second_value == v1 {
            1
        } else {
            2
        };
        if u[first] < 0.0 || u[second] < 0.0 {
            return [0.0; 4];
        }
        (u[first].sqrt(), 0.0, u[second].sqrt(), 0.0)
    } else {
        let norm = (u[1] * u[1] + u[2] * u[2]).sqrt().sqrt();
        let half_angle = u[2].atan2(u[1]) / 2.0;
        let (re, im) = (norm * half_angle.cos(), norm * half_angle.sin());
        (re, im, re, -im)
    };
    let w3 = q / 8.0 * (w1i * w2i - w1r * w2r) / (w1i * w1i + w1r * w1r) / (w2i * w2i + w2r * w2r);
    let offset = a / 4.0;
    let mut roots = [w1r + w2r + w3 - offset, -w1r - w2r + w3 - offset, 0.0, 0.0];
    if disc <= 0.0 {
        roots[2] = -w1r + w2r - w3 - offset;
        roots[3] = w1r - w2r - w3 - offset;
    }
    roots
}
