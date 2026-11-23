// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoQuadricND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec2D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/EquationSolver.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! General conics: the state GeoGebra's `GeoConicND` keeps, its
//! classification from the matrix and the transformations that work on the
//! matrix.
//!
//! The matrix is the flat symmetric matrix of
//! `m0*x² + m1*y² + m2 + 2*m3*x*y + 2*m4*x + 2*m5*y = 0`, raw as GeoGebra
//! holds it: [`ConicData::classify`] scales it by a power of two when its
//! quadratic coefficients are huge or tiny (`GeoConicND.checkDefined`),
//! and that scaled matrix is what later rules read.
//!
//! A conic's classification depends on its previous state: the order of
//! the two lines of a degenerate conic follows the old lines, and fields
//! that a type does not set (the half-axes of a pair of lines, for example)
//! keep their old values. [`ConicData`] therefore carries the whole state,
//! and the kernel keeps one record per conic object and computes the next
//! record from the previous one.
//!
//! The kernel is not continuous (`Kernel.isContinuous` is false by
//! default), so eigenvectors are set without looking at the old ones.

use crate::tol::{MAX_PRECISION, MIN_PRECISION, STANDARD_PRECISION};

/// `Kernel.INV_MAX_DOUBLE_PRECISION`.
const INV_MAX_DOUBLE_PRECISION: f64 = 1e15;

/// GeoGebra's conic types (`GeoConicNDConstants`), with the same numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ConicType {
    SinglePoint = 1,
    IntersectingLines = 2,
    Ellipse = 3,
    Circle = 4,
    Hyperbola = 5,
    Empty = 6,
    DoubleLine = 7,
    ParallelLines = 8,
    Parabola = 9,
    /// A conic that was never classified (`GeoQuadricND` starts so).
    NotClassified = 32,
}

impl ConicType {
    /// `GeoConicND.isDegenerate`: anything but a circle, an ellipse, a
    /// hyperbola or a parabola.
    pub fn is_degenerate(self) -> bool {
        !matches!(
            self,
            ConicType::Circle | ConicType::Ellipse | ConicType::Hyperbola | ConicType::Parabola
        )
    }

    /// The name used in golden files (`golden/SCHEMA.md`).
    pub fn name(self) -> &'static str {
        match self {
            ConicType::SinglePoint => "single_point",
            ConicType::IntersectingLines => "intersecting_lines",
            ConicType::Ellipse => "ellipse",
            ConicType::Circle => "circle",
            ConicType::Hyperbola => "hyperbola",
            ConicType::Empty => "empty",
            ConicType::DoubleLine => "double_line",
            ConicType::ParallelLines => "parallel_lines",
            ConicType::Parabola => "parabola",
            ConicType::NotClassified => "not_classified",
        }
    }

    /// Whether the conic is one of the two-line types, which keep `lines`.
    pub fn has_lines(self) -> bool {
        matches!(
            self,
            ConicType::IntersectingLines | ConicType::DoubleLine | ConicType::ParallelLines
        )
    }
}

/// The state of one conic, field by field as `GeoConicND` keeps it.
///
/// `scratch_*` fields are the working variables GeoGebra keeps in fields of
/// the object (`eigenvecX`, `eigenvecY`, `lambda`, `c`, `eigenval`); they
/// are read before they are set only in rare paths, and are kept so that
/// those paths agree too.
#[derive(Clone, Copy, Debug)]
pub struct ConicData {
    /// The flat matrix `[A00, A11, A22, A01, A02, A12]`.
    pub matrix: [f64; 6],
    pub ty: ConicType,
    /// `GeoConicND.isDefined`.
    pub defined: bool,
    pub half_axes: [f64; 2],
    /// The midpoint `b` (the vertex of a parabola, the point of a single
    /// point).
    pub midpoint: [f64; 2],
    /// The two eigenvectors, the axes' directions.
    pub eigenvec: [[f64; 2]; 2],
    pub linear_eccentricity: f64,
    pub eccentricity: f64,
    /// The parameter of a parabola.
    pub p: f64,
    pub mu: [f64; 2],
    /// The two lines `[x, y, z]` of a degenerate conic.
    pub lines: [[f64; 3]; 2],
    /// `errDetS`: the tolerance of the test `det S = 0`.
    pub err_det_s: f64,
    scratch_eigenvec: [f64; 2],
    scratch_lambda: f64,
    scratch_c: [f64; 2],
    scratch_eigenval: [f64; 3],
}

impl Default for ConicData {
    fn default() -> Self {
        ConicData::new()
    }
}

/// `DoubleUtil.isZero`.
fn is_zero(v: f64) -> bool {
    -STANDARD_PRECISION < v && v < STANDARD_PRECISION
}

/// `DoubleUtil.isZero(v, eps)`.
fn is_zero_eps(v: f64, eps: f64) -> bool {
    -eps < v && v < eps
}

/// `DoubleUtil.isEqual(a, b)`.
fn is_equal(a: f64, b: f64) -> bool {
    a == b || (a - STANDARD_PRECISION <= b && b <= a + STANDARD_PRECISION)
}

/// `DoubleUtil.isEqual(x, y, eps)`.
fn is_equal_eps(x: f64, y: f64, eps: f64) -> bool {
    x == y || (x - eps < y && y < x + eps)
}

/// `MyMath.length`: no overflow, no underflow.
pub(crate) fn my_length(x: f64, y: f64) -> f64 {
    let (ax, ay) = (x.abs(), y.abs());
    if ax == 0.0 {
        ay
    } else if ay == 0.0 {
        ax
    } else if ax > ay {
        let t = ay / ax;
        ax * (1.0 + t * t).sqrt()
    } else {
        let t = ax / ay;
        ay * (1.0 + t * t).sqrt()
    }
}

/// `EquationSolver.solveQuadraticS` for `eqn = [c, b, a]`, writing the
/// roots over `eqn` like GeoGebra (which passes the same array twice).
/// Returns the number of roots, `-1` when every number is a root.
fn solve_quadratic_s(eqn: &mut [f64; 3], eps: f64) -> i32 {
    let a = eqn[2];
    let b = eqn[1];
    let c = eqn[0];
    if a.abs() < eps {
        if b.abs() < eps {
            return -1;
        }
        eqn[0] = -c / b;
        1
    } else if b.abs() < eps * a.abs() {
        let x2 = -c / a;
        if is_zero_eps(x2, eps) {
            eqn[0] = 0.0;
            1
        } else if x2 < 0.0 {
            0
        } else {
            eqn[0] = x2.sqrt();
            eqn[1] = -x2.sqrt();
            2
        }
    } else {
        let d = b * b - 4.0 * a * c;
        if d.abs() < eps * b * b {
            eqn[0] = -b / (2.0 * a);
            1
        } else {
            if d < 0.0 {
                return 0;
            }
            let mut d = d.sqrt();
            if b < 0.0 {
                d = -d;
            }
            let q = (b + d) / -2.0;
            eqn[0] = q / a;
            eqn[1] = c / q;
            2
        }
    }
}

/// `MAX_COEFFICIENT_SIZE` and `MIN_COEFFICIENT_SIZE` of `GeoConicND`.
const MAX_COEFFICIENT_SIZE: f64 = 100000.0;
const MIN_COEFFICIENT_SIZE: f64 = 1.0;

impl ConicData {
    /// A new `GeoConic`: zero matrix, half-axes and midpoint, the unit
    /// eigenvectors, never classified, defined.
    pub const fn new() -> Self {
        ConicData {
            matrix: [0.0; 6],
            ty: ConicType::NotClassified,
            defined: true,
            half_axes: [0.0; 2],
            midpoint: [0.0; 2],
            eigenvec: [[1.0, 0.0], [0.0, 1.0]],
            linear_eccentricity: 0.0,
            eccentricity: 0.0,
            p: 0.0,
            mu: [0.0; 2],
            // `new GeoLine(cons)` has NaN coordinates
            lines: [[f64::NAN; 3]; 2],
            err_det_s: STANDARD_PRECISION,
            scratch_eigenvec: [0.0; 2],
            scratch_lambda: 0.0,
            scratch_c: [0.0; 2],
            scratch_eigenval: [0.0; 3],
        }
    }

    /// The record of a circle made by GeoGebra's circle commands with
    /// centre `(cx, cy)` and radius `r` (`GeoQuadricND.setSphereND`), when
    /// it has been a circle since it was created: its radius is already 0
    /// below the standard precision, as the kernel keeps it.
    pub fn circle(cx: f64, cy: f64, r: f64) -> Self {
        let mut d = ConicData::new();
        d.set_sphere(cx, cy, r);
        d
    }

    /// `GeoQuadricND.setSphereND(M, r)` on this record.
    pub fn set_sphere(&mut self, cx: f64, cy: f64, r: f64) {
        let r = if is_zero(r) { 0.0 } else { r };
        if r < 0.0 {
            self.defined = false;
        }
        if !self.defined {
            return;
        }
        // `setSphereNDMatrix`
        self.midpoint = [cx, cy];
        self.half_axes = [r, r];
        let m2 = -r * r + cx * cx + cy * cy;
        self.matrix = [1.0, 1.0, m2, 0.0, -cx, -cy];
        if r >= STANDARD_PRECISION {
            if self.ty != ConicType::Circle {
                self.ty = ConicType::Circle;
                self.linear_eccentricity = 0.0;
                self.eccentricity = 0.0;
                self.scratch_eigenvec = [1.0, 0.0];
                self.find_eigenvectors();
            }
        } else if is_zero(r) {
            self.single_point();
        } else {
            self.ty = ConicType::Empty;
        }
    }

    /// `GeoQuadricND.setUndefined` clears the type and matrix.
    pub(crate) fn set_undefined(&mut self) {
        self.defined = false;
        self.ty = ConicType::Empty;
        self.matrix.fill(f64::NAN);
    }

    /// The record GeoGebra makes from a matrix (`GeoConicND.setMatrix`
    /// followed by the classification), from this one's state.
    pub fn set_matrix(&mut self, m: [f64; 6]) {
        self.matrix = m;
        self.classify(false);
    }

    /// `GeoConicND.classifyConic(degenerate)`.
    pub fn classify(&mut self, degenerate: bool) {
        self.defined = degenerate || self.check_defined();
        if !self.defined {
            return;
        }
        let m = self.matrix;
        if is_equal_eps(m[0] * m[1], m[3] * m[3], self.err_det_s) {
            self.classify_parabolic(degenerate);
        } else {
            let det_s = m[0] * m[1] - m[3] * m[3];
            self.classify_midpoint(degenerate, det_s);
        }
    }

    /// `GeoConicND.checkDefined`: no NaN or infinity and not all zero;
    /// scales the matrix by a power of two to keep the quadratic
    /// coefficients between 1 and 100000.
    fn check_defined(&mut self) -> bool {
        let mut all_zero = true;
        let mut max_coeff_abs = 0.0_f64;
        for (i, &v) in self.matrix.iter().enumerate() {
            if v.is_nan() || v.is_infinite() {
                return false;
            }
            let abs = v.abs();
            if abs > STANDARD_PRECISION {
                all_zero = false;
            }
            if (i == 0 || i == 1 || i == 3) && max_coeff_abs < abs {
                max_coeff_abs = abs;
            }
        }
        if all_zero {
            return false;
        }
        if max_coeff_abs == 0.0 {
            return true;
        }
        let mut factor = 1.0;
        if max_coeff_abs < MIN_COEFFICIENT_SIZE {
            factor = 2.0;
            while max_coeff_abs * factor < MIN_COEFFICIENT_SIZE {
                factor *= 2.0;
            }
        } else if max_coeff_abs > MAX_COEFFICIENT_SIZE {
            factor = 0.5;
            while max_coeff_abs * factor > MAX_COEFFICIENT_SIZE {
                factor *= 0.5;
            }
        }
        if factor != 1.0 && !factor.is_infinite() && !factor.is_nan() {
            for v in &mut self.matrix {
                *v *= factor;
            }
        }
        true
    }

    fn classify_midpoint(&mut self, degenerate: bool, det_s: f64) {
        let m = self.matrix;
        if is_zero(m[3]) {
            self.scratch_eigenval[0] = m[0];
            self.scratch_eigenval[1] = m[1];
            self.scratch_eigenvec = [1.0, 0.0];
        } else {
            self.scratch_eigenval[0] = det_s;
            self.scratch_eigenval[1] = -(m[0] + m[1]);
            self.scratch_eigenval[2] = 1.0;
            let n = solve_quadratic_s(&mut self.scratch_eigenval, STANDARD_PRECISION);
            if n == 1 {
                self.scratch_eigenval[1] = self.scratch_eigenval[0];
            }
            self.scratch_eigenvec = [-m[3], -self.scratch_eigenval[0] + m[0]];
        }
        self.midpoint = [
            (m[3] * m[5] - m[1] * m[4]) / det_s,
            (m[3] * m[4] - m[0] * m[5]) / det_s,
        ];
        let beta = m[4] * self.midpoint[0] + m[5] * self.midpoint[1] + m[2];
        let ev = self.scratch_eigenval;
        if degenerate || is_zero(beta) {
            self.find_eigenvectors();
            self.mu[0] = ev[0] / ev[1];
            if is_zero(self.mu[0]) {
                self.mu[0] = 0.0;
                self.intersecting_lines();
            } else if self.mu[0] < 0.0 {
                self.mu[0] = (-self.mu[0]).sqrt();
                self.intersecting_lines();
            } else {
                self.single_point();
            }
        } else {
            self.mu[0] = -ev[0] / beta;
            self.mu[1] = -ev[1] / beta;
            if det_s < 0.0 {
                self.hyperbola();
            } else if self.mu[0] > 0.0 && self.mu[1] > 0.0 {
                self.ellipse();
            } else {
                self.ty = ConicType::Empty;
            }
        }
    }

    /// `findEigenvectors` of a kernel that is not continuous.
    fn find_eigenvectors(&mut self) {
        let [mut x, mut y] = self.scratch_eigenvec;
        let length = my_length(x, y);
        if length != 1.0 {
            x /= length;
            y /= length;
        }
        self.scratch_eigenvec = [x, y];
        self.eigenvec = [[x, y], [-y, x]];
    }

    /// `setParabolicEigenvectors` of a kernel that is not continuous.
    fn set_parabolic_eigenvectors(&mut self) {
        // the same as `find_eigenvectors` without continuity
        self.find_eigenvectors();
    }

    fn single_point(&mut self) {
        self.ty = ConicType::SinglePoint;
        self.half_axes = [0.0, 0.0];
    }

    fn intersecting_lines(&mut self) {
        self.ty = ConicType::IntersectingLines;
        let [e0, e1] = self.eigenvec;
        let [bx, by] = self.midpoint;
        let temp1 = e0[0] * self.mu[0];
        let temp2 = e0[1] * self.mu[0];
        let nx = e1[0] - temp1;
        let ny = e1[1] - temp2;
        let l = self.lines;
        let index = if (nx * l[0][0] + ny * l[0][1]).abs() < (nx * l[1][0] + ny * l[1][1]).abs() {
            1
        } else {
            0
        };
        self.lines[index] = [nx, ny, -(nx * bx + ny * by)];
        let nx = e1[0] + temp1;
        let ny = e1[1] + temp2;
        self.lines[1 - index] = [nx, ny, -(nx * bx + ny * by)];
    }

    fn ellipse(&mut self) {
        let mut mu = self.mu;
        if is_equal(mu[0] / mu[1], 1.0) {
            self.scratch_eigenvec = [1.0, 0.0];
            self.find_eigenvectors();
            self.ty = ConicType::Circle;
            self.half_axes[0] = (1.0 / mu[0]).sqrt();
            self.half_axes[1] = self.half_axes[0];
            self.linear_eccentricity = 0.0;
            self.eccentricity = 0.0;
        } else {
            if mu[0] > mu[1] {
                mu.swap(0, 1);
                let [x, y] = self.scratch_eigenvec;
                self.scratch_eigenvec = [-y, x];
            }
            self.find_eigenvectors();
            self.ty = ConicType::Ellipse;
            mu[0] = 1.0 / mu[0];
            mu[1] = 1.0 / mu[1];
            self.half_axes[0] = mu[0].sqrt();
            self.half_axes[1] = mu[1].sqrt();
            self.linear_eccentricity = (mu[0] - mu[1]).sqrt();
            self.eccentricity = self.linear_eccentricity / mu[0].sqrt();
        }
        self.mu = mu;
    }

    fn hyperbola(&mut self) {
        self.ty = ConicType::Hyperbola;
        let mut mu = self.mu;
        if mu[0] < 0.0 {
            mu.swap(0, 1);
            let [x, y] = self.scratch_eigenvec;
            self.scratch_eigenvec = [-y, x];
        }
        self.find_eigenvectors();
        mu[0] = 1.0 / mu[0];
        mu[1] = -1.0 / mu[1];
        self.half_axes[0] = mu[0].sqrt();
        self.half_axes[1] = mu[1].sqrt();
        self.linear_eccentricity = (mu[0] + mu[1]).sqrt();
        self.eccentricity = self.linear_eccentricity / mu[0].sqrt();
        self.mu = mu;
    }

    fn classify_parabolic(&mut self, degenerate: bool) {
        let m = self.matrix;
        if is_zero(m[3]) {
            if is_zero(m[0]) {
                if is_zero(m[1]) {
                    self.handle_s_zero();
                    return;
                }
                self.scratch_lambda = m[1];
                self.scratch_eigenvec = [1.0, 0.0];
                self.scratch_c = [m[4], m[5]];
            } else {
                self.scratch_lambda = m[0];
                self.scratch_eigenvec = [0.0, 1.0];
                self.scratch_c = [m[5], -m[4]];
            }
        } else {
            self.scratch_lambda = m[0] + m[1];
            let length = my_length(m[3], m[0]);
            let (ex, ey) = (m[3] / length, -m[0] / length);
            self.scratch_eigenvec = [ex, ey];
            self.scratch_c = [m[4] * ex + m[5] * ey, m[5] * ex - m[4] * ey];
        }
        if degenerate || is_zero(self.scratch_c[0]) {
            self.find_eigenvectors();
            let [ex, ey] = self.scratch_eigenvec;
            let temp = self.scratch_c[1] / self.scratch_lambda;
            self.midpoint = [temp * ey, -temp * ex];
            self.mu[0] = -temp * temp + m[2] / self.scratch_lambda;
            if is_zero(self.mu[0]) {
                self.double_line();
            } else if self.mu[0] < 0.0 {
                self.mu[0] = (-self.mu[0]).sqrt();
                self.parallel_lines();
            } else {
                self.ty = ConicType::Empty;
            }
        } else {
            self.parabola();
        }
    }

    fn double_line(&mut self) {
        self.ty = ConicType::DoubleLine;
        let nx = -self.eigenvec[0][1];
        let ny = self.eigenvec[0][0];
        let z = -(self.midpoint[0] * nx + self.midpoint[1] * ny);
        self.lines = [[nx, ny, z], [nx, ny, z]];
    }

    fn handle_s_zero(&mut self) {
        let m = self.matrix;
        if is_zero(m[4]) {
            if is_zero(m[5]) {
                self.ty = ConicType::Empty;
                return;
            }
            self.midpoint = [0.0, -m[2] / (2.0 * m[5])];
        } else {
            self.midpoint = [-m[2] / (2.0 * m[4]), 0.0];
        }
        self.scratch_eigenvec = [m[5], -m[4]];
        self.find_eigenvectors();
        self.double_line();
    }

    fn parallel_lines(&mut self) {
        self.ty = ConicType::ParallelLines;
        let nx = -self.eigenvec[0][1];
        let ny = self.eigenvec[0][0];
        let temp1 = self.midpoint[0] * nx + self.midpoint[1] * ny;
        self.lines[0][0] = nx;
        self.lines[0][1] = ny;
        self.lines[1][0] = nx;
        self.lines[1][1] = ny;
        let temp2 = self.mu[0] - temp1;
        if (self.lines[0][2] - temp2).abs() < (self.lines[1][2] - temp2).abs() {
            self.lines[0][2] = temp2;
            self.lines[1][2] = -temp1 - self.mu[0];
        } else {
            self.lines[0][2] = -temp1 - self.mu[0];
            self.lines[1][2] = temp2;
        }
    }

    fn parabola(&mut self) {
        self.ty = ConicType::Parabola;
        let m = self.matrix;
        let [cx, cy] = self.scratch_c;
        let lambda = self.scratch_lambda;
        let [ex, ey] = self.scratch_eigenvec;
        let temp2 = cy / lambda;
        let temp1 = (cy * temp2 - m[2]) / (2.0 * cx);
        self.midpoint = [ey * temp2 + ex * temp1, ey * temp1 - ex * temp2];
        self.set_parabolic_eigenvectors();
        self.p = -cx / lambda;
        if self.p < 0.0 {
            self.eigenvec[0] = [-self.eigenvec[0][0], -self.eigenvec[0][1]];
            self.p = -self.p;
        }
        self.linear_eccentricity = self.p / 2.0;
        self.eccentricity = 1.0;
    }

    /// `GeoConicND.setEllipseHyperbola(B, C, a)`: the ellipse or hyperbola
    /// with foci `b` and `c` and semi-major axis `a`. Undefined unless the
    /// result is an ellipse, a circle, a hyperbola or a double line.
    pub fn set_ellipse_hyperbola(&mut self, b: (f64, f64), c: (f64, f64), a: f64) {
        if a < -STANDARD_PRECISION {
            self.defined = false;
            return;
        }
        self.defined = true;
        let (b1, b2) = b;
        let (c1, c2) = c;
        let diff1 = b1 - c1;
        let diff2 = b2 - c2;
        let sqsumb = b1 * b1 + b2 * b2;
        let sqsumc = c1 * c1 + c2 * c2;
        let sqsumdiff = sqsumb - sqsumc;
        let a2 = 2.0 * a;
        let asq4 = a2 * a2;
        let asq = a * a;
        let afo = asq * asq;
        self.matrix = [
            4.0 * (a2 - diff1) * (a2 + diff1),
            4.0 * (a2 - diff2) * (a2 + diff2),
            -16.0 * afo - sqsumdiff * sqsumdiff + 8.0 * asq * (sqsumb + sqsumc),
            -4.0 * diff1 * diff2,
            -2.0 * (asq4 * (b1 + c1) - diff1 * sqsumdiff),
            -2.0 * (asq4 * (b2 + c2) - diff2 * sqsumdiff),
        ];
        let e0 = [c1 - b1, c2 - b2];
        self.eigenvec = [e0, [-e0[1], e0[0]]];
        self.classify(false);
        if !matches!(
            self.ty,
            ConicType::Hyperbola | ConicType::Ellipse | ConicType::Circle | ConicType::DoubleLine
        ) {
            self.defined = false;
        }
    }

    /// `GeoConicND.setParabola(F, g)`: the parabola with focus `f` and
    /// directrix `g = (x, y, z)` (raw line coefficients).
    pub fn set_parabola(&mut self, f: (f64, f64), g: [f64; 3]) {
        let (fx, fy) = f;
        let [gx, gy, gz] = g;
        let m0 = gy * gy;
        let m1 = gx * gx;
        let lsq = m0 + m1;
        self.matrix = [
            m0,
            m1,
            lsq * (fx * fx + fy * fy) - gz * gz,
            -gx * gy,
            -(lsq * fx + gx * gz),
            -(lsq * fy + gy * gz),
        ];
        self.normalize_matrix();
        self.classify(false);
        // avoid flip
        let [ex, ey] = self.scratch_eigenvec;
        if self.defined && ex * gx + ey * gy < 0.0 {
            let (ex, ey) = (-ex, -ey);
            self.scratch_eigenvec = [ex, ey];
            let m = self.matrix;
            self.scratch_c = [m[4] * ex + m[5] * ey, m[5] * ex - m[4] * ey];
            self.parabola();
        }
    }

    /// `GeoConicND.normalizeMatrix`.
    fn normalize_matrix(&mut self) {
        let mut norm = 1.0_f64;
        for &v in &self.matrix {
            let entry = v.abs();
            if norm.abs() < entry && 1000.0 < entry {
                norm = entry;
            }
            if norm.abs() > entry && 0.001 > entry && !is_zero(entry) {
                norm = entry;
            }
        }
        if norm != 1.0 {
            for v in &mut self.matrix {
                *v /= norm;
            }
        }
    }

    /// `hasPositiveEigenvectorOrientation`.
    fn positive_orientation(&self) -> bool {
        let [e0, e1] = self.eigenvec;
        e0[0] * e1[1] > e0[1] * e1[0]
    }

    /// `setPositiveEigenvectorOrientation`.
    fn set_positive_orientation(&mut self, flag: bool) {
        if flag != self.positive_orientation() {
            self.eigenvec[1] = [-self.eigenvec[1][0], -self.eigenvec[1][1]];
        }
    }

    /// `GeoConicND.doTranslate(vx, vy)`.
    fn do_translate(&mut self, vx: f64, vy: f64) {
        let m = &mut self.matrix;
        m[2] =
            m[2] + vx * (m[0] * vx - 2.0 * m[4]) + vy * (m[1] * vy - 2.0 * m[5] + 2.0 * m[3] * vx);
        m[4] = m[4] - m[0] * vx - m[3] * vy;
        m[5] = m[5] - m[3] * vx - m[1] * vy;
        self.midpoint = [vx + self.midpoint[0], vy + self.midpoint[1]];
    }

    /// `GeoConicND.updateDegenerates()`: the parts of a degenerate conic
    /// from its eigenvectors, midpoint and `mu`.
    fn update_degenerates(&mut self) {
        match self.ty {
            ConicType::SinglePoint => self.single_point(),
            ConicType::IntersectingLines => self.intersecting_lines(),
            ConicType::DoubleLine => self.double_line(),
            ConicType::ParallelLines => self.parallel_lines(),
            _ => {}
        }
    }

    /// `GeoConicND.translate(v)`.
    pub fn translate(&mut self, vx: f64, vy: f64) {
        self.do_translate(vx, vy);
        self.update_degenerates();
    }

    /// `GeoConicND.rotate(phi)` about the origin, or about `q`.
    pub fn rotate(&mut self, phi: f64, q: Option<(f64, f64)>) {
        if let Some((qx, qy)) = q {
            self.do_translate(-qx, -qy);
            self.rotate_origin(phi);
            self.do_translate(qx, qy);
        } else {
            self.rotate_origin(phi);
        }
        self.update_degenerates();
    }

    /// The private `GeoConicND.rotate(phi)`.
    fn rotate_origin(&mut self, phi: f64) {
        let m = &mut self.matrix;
        let sum = m[0] + m[1];
        let diff = m[0] - m[1];
        let cos = phi.cos();
        let sin = phi.sin();
        let cos2 = (cos + sin) * (cos - sin);
        let sin2 = 2.0 * cos * sin;
        let temp = diff * cos2 - 2.0 * m[3] * sin2;
        let a0 = (sum + temp) / 2.0;
        let a1 = (sum - temp) / 2.0;
        let a3 = m[3] * cos2 + diff * cos * sin;
        let a4 = m[4] * cos - m[5] * sin;
        m[5] = m[5] * cos + m[4] * sin;
        m[0] = a0;
        m[1] = a1;
        m[3] = a3;
        m[4] = a4;
        let rot = |[x, y]: [f64; 2]| [x * cos - y * sin, x * sin + y * cos];
        self.eigenvec = [rot(self.eigenvec[0]), rot(self.eigenvec[1])];
        self.midpoint = rot(self.midpoint);
    }

    /// `GeoConic.mirror(Q)`: reflection in the point `q`.
    pub fn mirror_point(&mut self, (qx, qy): (f64, f64)) {
        let m = &mut self.matrix;
        // `4 * (...) + m2`, commutative: the same bits
        m[2] += 4.0 * (qy * qy * m[1] + qx * (qx * m[0] + 2.0 * qy * m[3] + m[4]) + qy * m[5]);
        m[4] = -2.0 * (qx * m[0] + qy * m[3]) - m[4];
        m[5] = -2.0 * (qx * m[3] + qy * m[1]) - m[5];
        self.eigenvec = [
            [-self.eigenvec[0][0], -self.eigenvec[0][1]],
            [-self.eigenvec[1][0], -self.eigenvec[1][1]],
        ];
        self.midpoint = [2.0 * qx - self.midpoint[0], 2.0 * qy - self.midpoint[1]];
        self.update_degenerates();
    }

    /// `GeoConic.mirror(g)`: reflection in the line `g = (x, y, z)`.
    pub fn mirror_line(&mut self, g: [f64; 3]) {
        let [gx, gy, gz] = g;
        let (qx, qy) = if gx.abs() > gy.abs() {
            (-gz / gx, 0.0)
        } else {
            (0.0, -gz / gy)
        };
        self.do_translate(-qx, -qy);
        self.mirror_angle(2.0 * (-gx).atan2(gy));
        self.do_translate(qx, qy);
        self.update_degenerates();
    }

    /// The private `GeoConic.mirror(phi)`.
    fn mirror_angle(&mut self, phi: f64) {
        let m = &mut self.matrix;
        let sum = m[0] + m[1];
        let diff = m[0] - m[1];
        let cos = phi.cos();
        let sin = phi.sin();
        let cos2 = (cos + sin) * (cos - sin);
        let sin2 = 2.0 * cos * sin;
        let tmp = diff * cos2 + 2.0 * m[3] * sin2;
        let a0 = (sum + tmp) / 2.0;
        let a1 = (sum - tmp) / 2.0;
        let a3 = -m[3] * cos2 + diff * cos * sin;
        let a4 = m[4] * cos + m[5] * sin;
        m[5] = -m[5] * cos + m[4] * sin;
        m[0] = a0;
        m[1] = a1;
        m[3] = a3;
        m[4] = a4;
        let mir = |[x, y]: [f64; 2]| [x * cos + y * sin, x * sin - y * cos];
        self.eigenvec = [mir(self.eigenvec[0]), mir(self.eigenvec[1])];
        self.midpoint = mir(self.midpoint);
    }

    /// `GeoConic.dilate(r, S)`: dilation by `r` from `s`.
    pub fn dilate(&mut self, r: f64, (sx, sy): (f64, f64)) {
        let old = self.positive_orientation();
        self.do_translate(-sx, -sy);
        self.do_dilate(r);
        self.do_translate(sx, sy);
        if self.ty.is_degenerate() && r != 0.0 && r.is_finite() {
            let scale = |[x, y]: [f64; 2]| [x * r, y * r];
            self.eigenvec = [scale(self.eigenvec[0]), scale(self.eigenvec[1])];
            self.update_degenerates();
        } else {
            self.classify(false);
        }
        self.set_positive_orientation(old);
    }

    /// `GeoConicND.matrixTransform(a00, a01, a10, a11)` (shear and stretch).
    pub fn matrix_transform(&mut self, a00: f64, a01: f64, a10: f64, a11: f64) {
        let det = a00 * a11 - a01 * a10;
        let det2 = det * det;
        let m = &mut self.matrix;
        let a0 = a11 * (a11 * m[0] - a10 * m[3]) - a10 * (a11 * m[3] - a10 * m[1]);
        let a3 = a00 * (a11 * m[3] - a10 * m[1]) - a01 * (a11 * m[0] - a10 * m[3]);
        let a1 = a00 * (a00 * m[1] - a01 * m[3]) - a01 * (a00 * m[3] - a01 * m[0]);
        let a4 = a11 * m[4] - a10 * m[5];
        m[5] = (a00 * m[5] - a01 * m[4]) / det;
        m[0] = a0 / det2;
        m[1] = a1 / det2;
        m[3] = a3 / det2;
        m[4] = a4 / det;
        self.classify(false);
    }

    /// `GeoConic.fromLine`: this conic becomes the line `g`, whose start
    /// point (the first point of a segment or ray, or `None` for the point
    /// of a plain line closest to the origin, `setStandardStartPoint`)
    /// becomes the midpoint. GeoGebra keeps a line as its private single-line
    /// type; the kernel holds it as a double line until the inversion below
    /// classifies it again.
    pub fn from_line(&mut self, g: [f64; 3], start: Option<(f64, f64)>) {
        self.lines[0] = g;
        self.lines[1] = g;
        self.defined = true;
        self.ty = ConicType::DoubleLine;
        self.eigenvec[0] = [-g[1], g[0]];
        self.midpoint = match start {
            Some((x, y)) => [x, y],
            None => {
                let [x, y, z] = g;
                if x != 0.0 || y != 0.0 {
                    [-z * x / (x * x + y * y), -z * y / (x * x + y * y)]
                } else {
                    [0.0, 0.0]
                }
            }
        };
    }

    /// `GeoConic.mirror(conic)`: the inversion of this conic in the circle
    /// `c`. The image of a circle is a circle or a line; the image of a line
    /// (a record made by [`Self::from_line`], `from_a_line`) is a circle
    /// through the centre of the mirror, or the line itself when it passes
    /// through that centre (GeoGebra's single-line type, which the kernel
    /// reports as undefined).
    pub fn mirror_conic(&mut self, c: &ConicData, from_a_line: bool) {
        if c.ty == ConicType::SinglePoint {
            self.defined = false;
            return;
        }
        if c.ty == ConicType::Circle
            && (from_a_line || matches!(self.ty, ConicType::SinglePoint | ConicType::Circle))
        {
            let (x1, y1) = (c.midpoint[0], c.midpoint[1]);
            let r2 = self.half_axes[0];
            let (x2, y2) = (self.midpoint[0], self.midpoint[1]);
            let dist = ((x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2)).sqrt();
            if is_zero(r2) && is_zero(dist) {
                self.defined = false;
                return;
            }
            let r1 = c.half_axes[0];
            if from_a_line {
                // the image of a line
                let [lx, ly, lz] = self.lines[0];
                let (perp_x, perp_y) = if lx == 0.0 {
                    (x1, -lz / ly)
                } else {
                    let perp_y = -(lx * ly * x1 - lx * lx * y1 + ly * lz) / (lx * lx + ly * ly);
                    let perp_x = (-lz - ly * perp_y) / lx;
                    (perp_x, perp_y)
                };
                let dist2 = (perp_x - x1) * (perp_x - x1) + (perp_y - y1) * (perp_y - y1);
                if !is_zero(dist2) {
                    let sf = r1 * r1 / dist2;
                    let mx = x1 + sf * (perp_x - x1) / 2.0;
                    let my = y1 + sf * (perp_y - y1) / 2.0;
                    let radius = sf / 2.0 * dist2.sqrt();
                    self.defined = true;
                    self.set_sphere(mx, my, radius);
                } else {
                    // the line passes through the centre: GeoGebra keeps it
                    self.defined = false;
                }
                return;
            }
            if is_equal(dist, r2) {
                // the circle passes through the centre of the mirror: a line
                let (dx, dy) = (x2 - x1, y2 - y1);
                let k = r1 * r1 / 2.0 / r2 / r2;
                let (x3, y3) = (x1 + k * dx, y1 + k * dy);
                self.matrix[4] = dx * 0.5;
                self.matrix[5] = dy * 0.5;
                self.matrix[2] = -dx * x3 - dy * y3;
                self.matrix[0] = 0.0;
                self.matrix[1] = 0.0;
                self.matrix[3] = 0.0;
                // GeoGebra writes its single-line type over the double line
                // the classification finds; the kernel keeps the double line
                self.classify(true);
                return;
            }
            let x = r1 * r1 / (dist - r2);
            let y = r1 * r1 / (dist + r2);
            // radius of the image circle
            let r3 = (y - x).abs() / 2.0;
            let (cx, cy) = if is_zero(dist) {
                (x1, y1)
            } else {
                let t = (x.min(y) + r3) / dist;
                (x1 + (x2 - x1) * t, y1 + (y2 - y1) * t)
            };
            self.set_sphere(cx, cy, r3);
            return;
        }
        if c.ty == ConicType::ParallelLines {
            self.mirror_line(c.lines[0]);
        } else {
            self.defined = false;
        }
    }

    /// `GeoConicND.doDilate(factor)`.
    fn do_dilate(&mut self, factor: f64) {
        let r = 1.0 / factor;
        let r2 = r * r;
        let m = &mut self.matrix;
        m[0] *= r2;
        m[1] *= r2;
        m[3] *= r2;
        m[4] *= r;
        m[5] *= r;
        self.midpoint = [self.midpoint[0] * factor, self.midpoint[1] * factor];
    }

    /// The conic through five points (`AlgoConicFivePoints.compute`)
    /// without its special case of five points on a parabola by
    /// construction, which needs the definitions of the points.
    pub fn set_five_points(&mut self, p: [(f64, f64); 5]) {
        // `GeoVec3D.cross` of two points with `z = 1`
        let cross = |(x0, y0): (f64, f64), (x1, y1): (f64, f64)| {
            let (x, y, z) = (y0 - y1, x1 - x0, x0 * y1 - y0 * x1);
            if (z.is_finite() && z.abs() > INV_MAX_DOUBLE_PRECISION)
                || (z != 0.0 && z.abs() < MAX_PRECISION)
            {
                [x / z, y / z, 1.0]
            } else {
                [x, y, z]
            }
        };
        let mut lines = [
            cross(p[0], p[1]),
            cross(p[2], p[3]),
            cross(p[0], p[2]),
            cross(p[1], p[3]),
        ];
        for l in &mut lines {
            normalize_line(l);
        }
        let a = outer(lines[0], lines[1]);
        let b = outer(lines[2], lines[3]);
        let mut e1 = eval_matrix(&b, p[4]);
        let mut e2 = -eval_matrix(&a, p[4]);
        if should_invert(e1) && should_invert(e2) {
            if huge_for_matrix(e1, &a) && huge_for_matrix(e2, &a) {
                e2 /= e1;
                e1 = 1.0;
            } else {
                let tmp = e1;
                e1 = 1.0 / e2;
                e2 = 1.0 / tmp;
            }
        }
        let mut c = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                c[i][j] = e1 * a[i][j] + e2 * b[i][j];
            }
        }
        self.err_det_s = MIN_PRECISION;
        self.set_matrix([
            c[0][0],
            c[1][1],
            c[2][2],
            (c[0][1] + c[1][0]) / 2.0,
            (c[0][2] + c[2][0]) / 2.0,
            (c[1][2] + c[2][1]) / 2.0,
        ]);
    }

    /// Whether `(x, y)` lies on the conic within `eps`
    /// (`GeoConicND.isOnFullConic` for a finite point).
    pub fn on_full_conic(&self, (px, py): (f64, f64), eps: f64) -> bool {
        match self.ty {
            ConicType::SinglePoint => {
                let (dx, dy) = (px - self.midpoint[0], py - self.midpoint[1]);
                (dx * dx + dy * dy + 0.0).sqrt() < eps
            }
            ConicType::IntersectingLines | ConicType::DoubleLine | ConicType::ParallelLines => {
                on_full_line(self.lines[0], (px, py), eps)
                    || on_full_line(self.lines[1], (px, py), eps)
            }
            ConicType::Empty => false,
            _ => {
                // translate by -b, then into the eigenvector system
                let (x, y) = (px - self.midpoint[0], py - self.midpoint[1]);
                let (x, y) = if self.ty == ConicType::Circle {
                    (x, y)
                } else {
                    let [e0, e1] = self.eigenvec;
                    (x * e0[0] + y * e0[1], x * e1[0] + y * e1[1])
                };
                let [a, b] = self.half_axes;
                match self.ty {
                    ConicType::Circle => {
                        let radius2 = a * a;
                        is_equal_eps(x * x / radius2 + y * y / radius2, 1.0, eps)
                    }
                    ConicType::Ellipse => is_equal_eps(x * x / (a * a) + y * y / (b * b), 1.0, eps),
                    ConicType::Hyperbola => {
                        is_equal_eps(x * x / (a * a), 1.0 + y * y / (b * b), eps)
                    }
                    ConicType::Parabola => is_equal_eps(y * y, 2.0 * self.p * x, eps),
                    _ => false,
                }
            }
        }
    }

    /// Whether two records are the same, bit for bit.
    pub(crate) fn same(&self, other: &ConicData) -> bool {
        fn bits<const N: usize>(a: &[f64; N], b: &[f64; N]) -> bool {
            a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
        }
        let flat = |d: &ConicData| {
            [
                d.half_axes[0],
                d.half_axes[1],
                d.midpoint[0],
                d.midpoint[1],
                d.eigenvec[0][0],
                d.eigenvec[0][1],
                d.eigenvec[1][0],
                d.eigenvec[1][1],
                d.linear_eccentricity,
                d.eccentricity,
                d.p,
                d.mu[0],
                d.mu[1],
                d.lines[0][0],
                d.lines[0][1],
                d.lines[0][2],
                d.lines[1][0],
                d.lines[1][1],
                d.lines[1][2],
                d.err_det_s,
                d.scratch_eigenvec[0],
                d.scratch_eigenvec[1],
                d.scratch_lambda,
                d.scratch_c[0],
                d.scratch_c[1],
                d.scratch_eigenval[0],
                d.scratch_eigenval[1],
                d.scratch_eigenval[2],
            ]
        };
        self.ty == other.ty
            && self.defined == other.defined
            && bits(&self.matrix, &other.matrix)
            && bits(&flat(self), &flat(other))
    }

    /// Whether every number a reader can see is finite.
    pub fn is_finite(&self) -> bool {
        self.matrix.iter().all(|v| v.is_finite())
    }
}

/// `AlgoConicFivePoints.normalizeLines` for one line: divide by the
/// largest magnitude only when it is below 1.
fn normalize_line(l: &mut [f64; 3]) {
    let norm = l[0].abs().max(l[1].abs()).max(l[2].abs());
    if norm < 1.0 {
        for v in l.iter_mut() {
            *v /= norm;
        }
    }
}

/// The matrix `a ⊗ b` (`A[i][j] = a[i] * b[j]`).
fn outer(a: [f64; 3], b: [f64; 3]) -> [[f64; 3]; 3] {
    let mut m = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = a[i] * b[j];
        }
    }
    m
}

/// `AlgoConicFivePoints.evalMatrix` at the point `(x, y, 1)`.
fn eval_matrix(m: &[[f64; 3]; 3], (x, y): (f64, f64)) -> f64 {
    let z = 1.0;
    m[0][0] * x * x
        + m[1][1] * y * y
        + m[2][2] * z * z
        + (m[0][1] + m[1][0]) * x * y
        + (m[0][2] + m[2][0]) * x * z
        + (m[1][2] + m[2][1]) * y * z
}

/// `AlgoConicFivePoints.shouldInvert`.
fn should_invert(d: f64) -> bool {
    (!is_zero(d) && d.abs() < 0.001) || d.abs() > 1000.0
}

/// `AlgoConicFivePoints.hugeForMatrix`.
fn huge_for_matrix(e: f64, m: &[[f64; 3]; 3]) -> bool {
    m.iter()
        .flatten()
        .any(|&v| !is_zero_eps(v, MIN_PRECISION) && e.abs() > 1e10 * v)
}

/// `GeoLine.isOnFullLine2D` of the raw line `l` for a finite point.
fn on_full_line(l: [f64; 3], (px, py): (f64, f64), eps: f64) -> bool {
    let [x, y, z] = l;
    let simplelength = x.abs() + y.abs();
    (x * px / 1.0 + y * py / 1.0 + z).abs() < eps * simplelength
}
