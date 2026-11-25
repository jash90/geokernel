// Recognition decisions and stroke moments follow EuclidianPenFreehand.java,
// GeoGebra revision 112919f2d450ec7430a5cd3b65dba226fdc43b74.
// Copyright (c) GeoGebra GmbH. Licensed under the EUPL 1.2.

//! Screen-space stroke recognition and construction recipes.

use crate::interact::ViewTransform;
use crate::labels::{next_label, polygon_label};
use geo_kernel::{Construction, Kind};
use std::fmt::Write as _;

pub const MAX_STROKE_SAMPLES: usize = 512;
const MAX_FUNCTION_COLUMNS: usize = 2048;
type Point = (f64, f64);

enum Shape {
    Segment([Point; 2]),
    Polygon(Vec<Point>),
    Circle(Point, f64),
    Ellipse([Point; 3]),
    Hyperbola([Point; 3]),
}

fn distance(a: Point, b: Point) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

#[derive(Default)]
struct Moments {
    mass: f64,
    x: f64,
    y: f64,
    xx: f64,
    xy: f64,
    yy: f64,
}

impl Moments {
    fn of(stroke: &[Point]) -> Option<Self> {
        let origin = stroke[0];
        let mut moments = Self::default();
        for pair in stroke.windows(2) {
            let (x, y) = (pair[0].0 - origin.0, pair[0].1 - origin.1);
            let mass = distance(pair[0], pair[1]);
            moments.mass += mass;
            moments.x += mass * x;
            moments.y += mass * y;
            moments.xx += mass * x * x;
            moments.xy += mass * x * y;
            moments.yy += mass * y * y;
        }
        let mass = moments.mass;
        if mass <= 0.0 {
            return None;
        }
        let x = moments.x / mass;
        let y = moments.y / mass;
        Some(Self {
            mass,
            x: origin.0 + x,
            y: origin.1 + y,
            xx: moments.xx / mass - x * x,
            xy: moments.xy / mass - x * y,
            yy: moments.yy / mass - y * y,
        })
    }

    fn determinant(&self) -> f64 {
        let trace = self.xx + self.yy;
        if trace <= 0.0 {
            return 0.0;
        }
        4.0 * (self.xx * self.yy - self.xy * self.xy) / trace.powi(2)
    }

    fn segment(&self, stroke: &[Point]) -> [Point; 2] {
        let angle = 0.5 * (2.0 * self.xy).atan2(self.xx - self.yy);
        let (sin, cos) = angle.sin_cos();
        let (mut min, mut max) = (0.0_f64, 0.0_f64);
        for &(x, y) in stroke {
            let along = (x - self.x) * cos + (y - self.y) * sin;
            min = min.min(along);
            max = max.max(along);
        }
        let slant = 5.0_f64.to_radians();
        [min, max].map(|along| {
            (
                self.x
                    + if cos.abs() < slant.sin() {
                        0.0
                    } else {
                        along * cos
                    },
                self.y
                    + if angle.abs() < slant {
                        0.0
                    } else {
                        along * sin
                    },
            )
        })
    }
}

/// A closed triangle or quadrilateral whose sides each meet the line tolerance.
/// Split the piece with the largest normalized bend, up to four pieces.
fn polygon(stroke: &[Point], length: f64) -> Option<Vec<Point>> {
    let last = stroke.len() - 1;
    if last < 5 || distance(stroke[0], stroke[last]) > 0.10 * length {
        return None;
    }
    let mut corners = vec![0, last];
    while corners.len() <= 4 {
        let (piece, split, _) = corners
            .windows(2)
            .enumerate()
            .filter_map(|(i, edge)| {
                let (lo, hi) = (edge[0], edge[1]);
                let a = stroke[lo];
                let b = stroke[hi];
                let d = distance(a, b);
                let deviation = |p: Point| {
                    if d == 0.0 {
                        distance(a, p)
                    } else {
                        ((p.0 - a.0) * (b.1 - a.1) - (p.1 - a.1) * (b.0 - a.0)).abs() / d
                    }
                };
                (lo + 1..hi)
                    .map(|j| (i, j, deviation(stroke[j])))
                    .max_by(|a, b| a.2.total_cmp(&b.2))
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))?;
        corners.insert(piece + 1, split);
        if corners.len() < 4 {
            continue;
        }
        if corners.windows(2).all(|edge| {
            let (lo, hi) = (edge[0], edge[1]);
            distance(stroke[lo], stroke[hi]) >= length * 0.09
                && Moments::of(&stroke[lo..=hi]).is_some_and(|m| m.determinant() <= 0.015)
        }) {
            corners.pop();
            return Some(corners.into_iter().map(|i| stroke[i]).collect());
        }
    }
    None
}

/// Fit the ten reference samples to an ellipse. The stroke is normalized
/// before solving the least-squares normal equations with partial pivoting.
/// The fit with unit constant is sufficient when the origin is inside the ellipse.
fn conic(stroke: &[Point], center: Point, scale: f64) -> Option<Shape> {
    let count = 10;
    if stroke.len() < count || scale <= 0.0 {
        return None;
    }
    let normalized = |(x, y): Point| ((x - center.0) / scale, (y - center.1) / scale);
    let mut matrix = [[0.0; 6]; 5];
    for p in stroke.iter().step_by(stroke.len() / count).take(count) {
        let (x, y) = normalized(*p);
        let row = [x * x, x * y, y * y, x, y, 1.0];
        for i in 0..5 {
            for (cell, coefficient) in matrix[i].iter_mut().zip(row) {
                *cell += row[i] * coefficient;
            }
        }
    }
    for i in 0..5 {
        let pivot = (i..5).max_by(|&a, &b| matrix[a][i].abs().total_cmp(&matrix[b][i].abs()))?;
        matrix.swap(i, pivot);
        let d = matrix[i][i];
        if d.abs() < 1e-10 {
            return None;
        }
        for entry in &mut matrix[i][i..] {
            *entry /= d;
        }
        let row = matrix[i];
        for (other, cells) in matrix.iter_mut().enumerate() {
            if i != other {
                let factor = cells[i];
                for (cell, coefficient) in cells[i..].iter_mut().zip(&row[i..]) {
                    *cell -= factor * coefficient;
                }
            }
        }
    }
    let [a, b, c, d, e] = matrix.map(|row| row[5]);
    let determinant = a * c - 0.25 * b * b;
    if determinant.abs() <= 1e-10 {
        return None;
    }
    let x0 = (b * e * 0.5 - c * d) / (2.0 * determinant);
    let y0 = (b * d * 0.5 - a * e) / (2.0 * determinant);
    let rhs = 1.0 - 0.5 * (d * x0 + e * y0);
    let separation = (a - c).hypot(b);
    let angle = 0.5 * (-b).atan2(c - a);
    if determinant < 0.0 {
        let low = 0.5 * (a + c - separation);
        let high = 0.5 * (a + c + separation);
        let (along, across, angle) = if rhs > 0.0 {
            (high, low, angle + std::f64::consts::FRAC_PI_2)
        } else {
            (low, high, angle)
        };
        let transverse = (rhs / along).sqrt();
        let conjugate = (-rhs / across).sqrt();
        if !(transverse.is_finite() && transverse > 0.0 && conjugate.is_finite() && conjugate > 0.0)
        {
            return None;
        }
        let (sin, cos) = angle.sin_cos();
        let error = stroke
            .iter()
            .map(|&point| {
                let (x, y) = normalized(point);
                let (x, y) = (x - x0, y - y0);
                let a = (x * cos + y * sin) / transverse;
                let b = (y * cos - x * sin) / conjugate;
                (a * a - b * b - 1.0).abs()
            })
            .sum::<f64>()
            / stroke.len() as f64;
        if !(error.is_finite() && error <= 0.15) {
            return None;
        }
        let focus = transverse.hypot(conjugate);
        let points = [-focus, focus, transverse].map(|offset| {
            (
                center.0 + scale * (x0 + offset * cos),
                center.1 + scale * (y0 + offset * sin),
            )
        });
        return points
            .iter()
            .all(|&(x, y)| x.is_finite() && y.is_finite())
            .then_some(Shape::Hyperbola(points));
    }
    if a <= 0.0 {
        return None;
    }
    let minor = (rhs / (0.5 * (a + c + separation))).sqrt();
    let major = (rhs / (0.5 * (a + c - separation))).sqrt();
    if !(major.is_finite() && minor > 0.0) {
        return None;
    }
    let (sin, cos) = angle.sin_cos();
    let error = stroke
        .iter()
        .map(|&p| {
            let (x, y) = normalized(p);
            let (x, y) = (x - x0, y - y0);
            let along = (x * cos + y * sin) / major;
            let across = (y * cos - x * sin) / minor;
            (along.hypot(across) - 1.0).abs() * major
        })
        .sum::<f64>()
        / stroke.len() as f64;
    if error * 10.0 > minor {
        return None;
    }
    let focus = (major * major - minor * minor).sqrt();
    Some(Shape::Ellipse([-focus, focus, major].map(|offset| {
        (
            center.0 + scale * (x0 + offset * cos),
            center.1 + scale * (y0 + offset * sin),
        )
    })))
}

fn shape(stroke: &[Point], view: &ViewTransform) -> Option<Shape> {
    let m = Moments::of(stroke)?;
    let world = |(x, y)| view.screen_to_world(x, y);
    let det = m.determinant();
    if det < 0.015 {
        return Some(Shape::Segment(m.segment(stroke).map(world)));
    }
    if let Some(vertices) = polygon(stroke, m.mass) {
        return Some(Shape::Polygon(vertices.into_iter().map(world).collect()));
    }
    let center = (m.x, m.y);
    let radius = (m.xx + m.yy).sqrt();
    if det > 0.95 && radius > 0.0 {
        let error = stroke
            .windows(2)
            .map(|pair| distance(pair[0], pair[1]) * (distance(center, pair[0]) - radius).abs())
            .sum::<f64>()
            / (m.mass * radius);
        if error < 0.10 {
            let r = (m.xx / view.xscale().powi(2) + m.yy / view.yscale().powi(2)).sqrt();
            return Some(Shape::Circle(world(center), r));
        }
    }
    let world_stroke: Vec<_> = stroke.iter().copied().map(world).collect();
    let radius = radius / view.xscale().min(view.yscale());
    conic(&world_stroke, world(center), radius)
}

/// First y in each integer pixel column wins. Missing columns are interpolated
/// in screen order after the reference's 90 percent monotonicity test.
fn function_data(stroke: &[Point], view: &ViewTransform) -> Option<Vec<f64>> {
    let forward = stroke
        .windows(2)
        .filter(|pair| pair[0].0 <= pair[1].0)
        .count() as f64
        / (stroke.len() - 1) as f64;
    if (0.1..=0.9).contains(&forward) {
        return None;
    }
    let xs = stroke.iter().map(|p| p.0 as i64);
    let min = xs.clone().min()?;
    let max = xs.max()?;
    let width = usize::try_from(max.checked_sub(min)?).ok()?;
    if width >= MAX_FUNCTION_COLUMNS {
        return None;
    }
    let world_x = |x| crate::snap::check_decimal_fraction(view.screen_to_world(x, 0.0).0);
    let mut samples = vec![None; width + 1];
    for &(x, y) in stroke {
        samples[(x as i64 - min) as usize].get_or_insert(view.screen_to_world(x, y).1);
    }
    let mut result = vec![world_x(min as f64), world_x(max as f64)];
    let mut next = samples
        .iter()
        .enumerate()
        .filter_map(|(i, y)| y.map(|y| (i, y)));
    let mut previous = next.next()?;
    result.push(crate::snap::check_decimal_fraction(previous.1));
    for (i, y) in next {
        let (j, before) = previous;
        for k in j + 1..=i {
            let value = (before * (i - k) as f64 + y * (k - j) as f64) / (i - j) as f64;
            result.push(crate::snap::check_decimal_fraction(value));
        }
        previous = (i, y);
    }
    Some(result)
}

pub(crate) fn recognize(
    c: &Construction,
    world: &[Point],
    view: &ViewTransform,
    function_only: bool,
) -> Option<(Vec<String>, Vec<String>)> {
    if !(2..=MAX_STROKE_SAMPLES).contains(&world.len()) {
        return None;
    }
    let stroke: Vec<_> = world
        .iter()
        .map(|&(x, y)| {
            let (x, y) = view.world_to_screen(x, y);
            (x.round(), y.round())
        })
        .collect();
    if stroke
        .iter()
        .any(|&(x, y)| !x.is_finite() || !y.is_finite())
    {
        return None;
    }
    let detected = if function_only {
        None
    } else {
        shape(&stroke, view)
    };
    let Some(detected) = detected else {
        let data = function_data(&stroke, view)?;
        let name = next_label(Kind::Function, |name| c.id(name).is_some());
        let mut line = format!("{name} = Function({{");
        for (i, value) in data.iter().enumerate() {
            if i > 0 {
                line.push_str(", ");
            }
            write!(line, "{value}").unwrap();
        }
        line.push_str("})");
        return Some((vec![line], Vec::new()));
    };

    let mut lines = Vec::new();
    let mut points: Vec<String> = Vec::new();
    let mut add = |(x, y): Point| {
        let name = next_label(Kind::Point, |name| {
            c.id(name).is_some() || points.iter().any(|p| p == name)
        });
        lines.push(format!("{name} = ({x}, {y})"));
        points.push(name.clone());
        name
    };
    let mut hidden = Vec::new();
    let definition = match detected {
        Shape::Segment([a, b]) => {
            let (a, b) = (add(a), add(b));
            let name = next_label(Kind::Segment, |name| c.id(name).is_some());
            format!("{name} = Segment({a}, {b})")
        }
        Shape::Polygon(vertices) => {
            let name = polygon_label(vertices.len(), |name| c.id(name).is_some());
            let vertices: Vec<_> = vertices.into_iter().map(add).collect();
            format!("{name} = Polygon({})", vertices.join(", "))
        }
        Shape::Circle(center, radius) => {
            let center = add(center);
            let name = next_label(Kind::Circle, |name| c.id(name).is_some());
            hidden.push(center.clone());
            format!("{name} = Circle({center}, {radius})")
        }
        Shape::Ellipse([a, b, p]) | Shape::Hyperbola([a, b, p]) => {
            let command = if matches!(detected, Shape::Hyperbola(_)) {
                "Hyperbola"
            } else {
                "Ellipse"
            };
            let (a, b, p) = (add(a), add(b), add(p));
            let name = next_label(Kind::Conic, |name| c.id(name).is_some());
            hidden.extend([a.clone(), b.clone(), p.clone()]);
            let style = geo_kernel::script::saved::Property::DrawingStyle {
                label: name.clone(),
                style: geo_kernel::kernel::DrawingStyle {
                    show_label: false,
                    line_thickness: geo_kernel::kernel::DrawingStyle::GRAPHICS_THICKNESS,
                    ..Default::default()
                },
            };
            lines.push(format!("{name} = {command}({a}, {b}, {p})"));
            style.to_string()
        }
    };
    lines.push(definition);
    Some((lines, hidden))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::Camera;

    #[test]
    fn refuses_oversize_stroke_and_function_pixel_span() {
        let view = Camera::STANDARD.transform(400.0, 400.0);
        let construction = Construction::new();
        let stroke: Vec<_> = (0..=MAX_STROKE_SAMPLES)
            .map(|i| (i as f64 / 50.0, 0.0))
            .collect();
        assert!(recognize(&construction, &stroke, &view, false).is_none());
        assert!(recognize(&construction, &[(0.0, 0.0), (1e8, 1.0)], &view, true).is_none());
    }

    #[test]
    fn recognizes_triangle_circle_and_ellipse_from_closed_strokes() {
        let view = Camera::STANDARD.transform(400.0, 400.0);
        let c = Construction::new();
        let angles = (0..=40).map(|i| i as f64 * std::f64::consts::TAU / 40.0);
        let circle: Vec<_> = angles.clone().map(|a| (a.cos(), a.sin())).collect();
        let ellipse: Vec<_> = angles.map(|a| (2.0 * a.cos(), a.sin())).collect();
        let corners = [(0.0, 0.0), (2.0, 0.0), (1.0, 2.0), (0.0, 0.0)];
        let triangle: Vec<_> = corners
            .windows(2)
            .flat_map(|e| {
                (0..=8).map(move |i| {
                    let t = i as f64 / 8.0;
                    (
                        e[0].0 * (1.0 - t) + e[1].0 * t,
                        e[0].1 * (1.0 - t) + e[1].1 * t,
                    )
                })
            })
            .collect();
        for (stroke, command) in [
            (triangle, "Polygon("),
            (circle, "Circle("),
            (ellipse, "Ellipse("),
        ] {
            let (lines, _) = recognize(&c, &stroke, &view, false).unwrap();
            assert!(lines.iter().any(|line| line.contains(command)), "{lines:?}");
        }
        assert!(recognize(&c, &[(0.0, 0.0), (1.0, 1.0), (0.0, 0.0)], &view, true).is_none());
    }
}

#[cfg(test)]
#[test]
fn round_one_freehand_fits_an_open_hyperbola() {
    let view = crate::view::Camera::STANDARD.transform(892.0, 747.0);
    let points: Vec<_> = (-15..=15)
        .map(|i| {
            let t = f64::from(i) / 8.0;
            (2.0 + t.cosh(), t.sinh())
        })
        .collect();
    let mut wb = crate::workbench::Workbench::default();
    wb.recognize_stroke(&points, &view, false).unwrap();
    let c = wb.construction();
    assert!(c.ids().any(|id| c
        .conic(id)
        .is_some_and(|d| d.ty == geo_kernel::ConicType::Hyperbola)));
    assert!(wb.undo());
    assert!(wb.construction().is_empty());
}
