//! Visible outlines of arcs and sectors, in the curve cache's screen frame.

use eframe::egui::{pos2, Pos2};
use geo_app_core::hit::{part_at, part_axis_crossings};
use geo_kernel::algo::arc::ArcData;
use geo_kernel::plot::{conic::FLATNESS, Frame};

pub(super) fn plot(part: &ArcData, frame: &Frame, margin: f64) -> Vec<Vec<Pos2>> {
    let bounds = frame.visible(margin + geo_kernel::plot::PAD);
    let inside = |(x, y): (f64, f64)| {
        (bounds.xmin..=bounds.xmax).contains(&x) && (bounds.ymin..=bounds.ymax).contains(&y)
    };
    let screen = |(x, y)| {
        let (x, y) = frame.world_to_screen(x, y);
        pos2(x as f32, y as f32)
    };
    // Clip segments and rays parametrically before converting to screen floats.
    let line = |(x, y): (f64, f64), (dx, dy): (f64, f64), max: f64| {
        let (mut lo, mut hi) = (0.0_f64, max);
        for (p, d, min, max) in [
            (x, dx, bounds.xmin, bounds.xmax),
            (y, dy, bounds.ymin, bounds.ymax),
        ] {
            if d == 0.0 {
                if !(min..=max).contains(&p) {
                    return None;
                }
                continue;
            }
            let (s, t) = ((min - p) / d, (max - p) / d);
            lo = lo.max(s.min(t));
            hi = hi.min(s.max(t));
        }
        (lo <= hi && hi.is_finite()).then(|| {
            vec![
                screen((x + lo * dx, y + lo * dy)),
                screen((x + hi * dx, y + hi * dy)),
            ]
        })
    };

    if !part.defined {
        return Vec::new();
    }
    if let Some([(ax, ay), (bx, by)]) = part.segment() {
        let (dx, dy) = (bx - ax, by - ay);
        return if part.positive {
            line((ax, ay), (dx, dy), 1.0).into_iter().collect()
        } else {
            [
                line((ax, ay), (-dx, -dy), f64::INFINITY),
                line((bx, by), (dx, dy), f64::INFINITY),
            ]
            .into_iter()
            .flatten()
            .collect()
        };
    }
    if !part.is_curved() {
        return Vec::new();
    }

    let [x, y] = part.conic.midpoint;
    let end = part.start + part.extent;
    // Between crossings of the view's sides, the rim is entirely inside
    // or outside. Split at those crossings so zoom never samples a full
    // ellipse just to paint a tiny visible portion.
    let mut cuts = [0.0; 10];
    cuts[0] = part.start;
    cuts[1] = end;
    let mut n = 2;
    for (axis, offset) in [
        (0, bounds.xmin - x),
        (0, bounds.xmax - x),
        (1, bounds.ymin - y),
        (1, bounds.ymax - y),
    ] {
        for t in part_axis_crossings(part, axis, offset) {
            cuts[n] = t;
            n += 1;
        }
    }
    cuts[..n].sort_unstable_by(f64::total_cmp);
    let [a, b] = part.conic.half_axes;
    let step = (8.0 * FLATNESS / (a.max(b) * frame.scale())).sqrt();
    let mut paths = Vec::new();
    for interval in cuts[..n].windows(2) {
        let (lo, hi) = (interval[0], interval[1]);
        if hi <= lo || !inside(part_at(part, f64::midpoint(lo, hi))) {
            continue;
        }
        let steps = (((hi - lo) / step).ceil() as usize).clamp(1, 8192);
        paths.push(
            (0..=steps)
                .map(|i| screen(part_at(part, lo + (hi - lo) * i as f64 / steps as f64)))
                .collect(),
        );
    }
    if part.sector {
        for t in [part.start, end] {
            let (qx, qy) = part_at(part, t);
            paths.extend(line((x, y), (qx - x, qy - y), 1.0));
        }
    }
    paths
}
