//! How objects look: their colour, the colours of their algebra marker and
//! the geometry of an angle's sector. No drawing calls.

use crate::theme;
use eframe::egui::Color32;
use geo_kernel::algo::measure::MeasureDef;
use geo_kernel::kernel::{Construction, Def, Id, Value};

/// The default colour of an object: blue free points, grey dependent
/// points, green angles, near-black lines and circles.
pub fn color(c: &Construction, id: Id) -> Color32 {
    match (c.value(id), c.def(id)) {
        (Value::Point { .. }, def) if def.is_free() => theme::FREE_POINT,
        (Value::Point { .. }, _) => theme::DEPENDENT_POINT,
        (_, Def::Measure(MeasureDef::Angle(..))) => theme::ANGLE,
        _ => theme::CURVE,
    }
}

/// `color` laid over white with opacity `alpha`.
pub fn over_white(color: Color32, alpha: f32) -> Color32 {
    let mix = |v: u8| (255.0 - (255.0 - v as f32) * alpha).round() as u8;
    Color32::from_rgb(mix(color.r()), mix(color.g()), mix(color.b()))
}

/// The marker of an object's row in the algebra view: a light fill and a
/// darker ring of the object's colour.
pub fn marker(c: &Construction, id: Id) -> (Color32, Color32) {
    let base = color(c, id);
    (over_white(base, 0.35), over_white(base, 0.75))
}

/// The three points `A`, `B` (the vertex), `C` of an angle `Angle(A, B, C)`.
pub fn angle_points(c: &Construction, id: Id) -> Option<[(f64, f64); 3]> {
    let Def::Measure(MeasureDef::Angle(a, b, v)) = c.def(id) else {
        return None;
    };
    let p = |id: Id| match c.value(id) {
        Value::Point { x, y } => Some((x, y)),
        _ => None,
    };
    Some([p(a)?, p(b)?, p(v)?])
}

/// The sector of the angle at `b` from the arm `ba` counter-clockwise to `bc`
/// (world coordinates, as the kernel measures it), as a start direction and
/// a sweep in radians on the screen, where y points down.
pub fn sector(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<(f64, f64)> {
    let (ux, uy) = (a.0 - b.0, a.1 - b.1);
    let (vx, vy) = (c.0 - b.0, c.1 - b.1);
    if ux.hypot(uy) == 0.0 || vx.hypot(vy) == 0.0 {
        return None;
    }
    // World y points up; screen y points down, so world counter-clockwise is
    // a negative screen angle.
    let start = (-uy).atan2(ux);
    let mut sweep = (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    if sweep < 0.0 {
        sweep += std::f64::consts::TAU;
    }
    Some((start, -sweep))
}

/// Whether an angle in radians is a right angle (drawn as a square).
pub fn is_right(angle: f64) -> bool {
    (angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI};

    #[test]
    fn markers_are_light_fills_of_the_object_colour() {
        let c = geo_kernel::script::load("A = (0, 0)\nB = (1, 0)\nM = Midpoint(A, B)\n").unwrap();
        let a = c.id("A").unwrap();
        let m = c.id("M").unwrap();
        assert_eq!(color(&c, a), theme::FREE_POINT);
        assert_eq!(color(&c, m), theme::DEPENDENT_POINT);
        let (fill, ring) = marker(&c, a);
        assert!(fill.r() > 180 && fill.b() > 250, "{fill:?}");
        assert!(ring.r() < fill.r());
        assert_eq!(over_white(Color32::BLACK, 0.0), Color32::WHITE);
    }

    #[test]
    fn sectors_run_from_the_first_arm_to_the_second() {
        // A right angle at the origin from the x axis to the y axis.
        let (start, sweep) = sector((1.0, 0.0), (0.0, 0.0), (0.0, 1.0)).unwrap();
        assert!(start.abs() < 1e-12);
        assert!((sweep + FRAC_PI_2).abs() < 1e-12);
        // The other way round is the reflex angle.
        let (start, sweep) = sector((0.0, 1.0), (0.0, 0.0), (1.0, 0.0)).unwrap();
        assert!((start - FRAC_PI_2).abs() < 1e-12 || (start + FRAC_PI_2).abs() < 1e-12);
        assert!((sweep + 1.5 * PI).abs() < 1e-12);
        assert!(sector((0.0, 0.0), (0.0, 0.0), (1.0, 0.0)).is_none());
        assert!(is_right(FRAC_PI_2) && !is_right(1.5));
    }

    #[test]
    fn angle_points_come_from_the_definition() {
        let c = geo_kernel::script::load(
            "A = (1, 0)\nB = (0, 0)\nC = (0, 1)\nα = Angle(A, B, C)\nd = Distance(A, C)\n",
        )
        .unwrap();
        let alpha = c.id("α").unwrap();
        assert_eq!(
            angle_points(&c, alpha),
            Some([(1.0, 0.0), (0.0, 0.0), (0.0, 1.0)])
        );
        assert_eq!(color(&c, alpha), theme::ANGLE);
        assert_eq!(angle_points(&c, c.id("d").unwrap()), None);
    }
}
