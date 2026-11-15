//! Where an object's label goes, in screen coordinates, with no UI types.
//!
//! The screen area is the rectangle `[0, w] x [0, h]`, y pointing down.

/// A point and the unit direction into the area where a line enters it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub x: f64,
    pub y: f64,
    pub dx: f64,
    pub dy: f64,
}

/// Where the infinite line through `p` with direction `d` first meets the
/// edges of `[0, w] x [0, h]`, trying the left, top, right and bottom edge in
/// that order. `(dx, dy)` of the result points into the area.
pub fn line_entry(p: (f64, f64), d: (f64, f64), w: f64, h: f64) -> Option<Entry> {
    let len = d.0.hypot(d.1);
    if !(len.is_finite() && len > 0.0) {
        return None;
    }
    let (ux, uy) = (d.0 / len, d.1 / len);
    let eps = 1e-6;
    // (edge coordinate that is fixed, along which axis, extent of the other)
    let candidates = [
        (0.0, true, h),  // left: x = 0
        (0.0, false, w), // top: y = 0
        (w, true, h),    // right: x = w
        (h, false, w),   // bottom: y = h
    ];
    for (k, &(edge, vertical_edge, extent)) in candidates.iter().enumerate() {
        let (num, den) = if vertical_edge {
            (edge - p.0, ux)
        } else {
            (edge - p.1, uy)
        };
        if den.abs() < 1e-12 {
            continue;
        }
        let s = num / den;
        let (x, y) = (p.0 + s * ux, p.1 + s * uy);
        let other = if vertical_edge { y } else { x };
        if other < -eps || other > extent + eps {
            continue;
        }
        let entry = Entry {
            x: x.clamp(0.0, w),
            y: y.clamp(0.0, h),
            dx: ux,
            dy: uy,
        };
        return Some(orient(entry, k));
    }
    None
}

/// Makes the direction point into the area for the edge `k` that was hit
/// (0 left, 1 top, 2 right, 3 bottom).
fn orient(mut e: Entry, k: usize) -> Entry {
    let component = if k.is_multiple_of(2) { e.dx } else { e.dy };
    let inward_is_positive = k < 2;
    if (component > 0.0) != inward_is_positive {
        e.dx = -e.dx;
        e.dy = -e.dy;
    }
    e
}

/// The label centre for a line that enters at `entry`: `along` pixels in from
/// the edge, `side` pixels off the line toward the middle of the area.
pub fn line_label(entry: Entry, w: f64, h: f64, along: f64, side: f64) -> (f64, f64) {
    let (px, py) = (entry.x + entry.dx * along, entry.y + entry.dy * along);
    let (nx, ny) = (-entry.dy, entry.dx);
    let toward_centre = (0.5 * w - entry.x) * nx + (0.5 * h - entry.y) * ny;
    let sign = if toward_centre >= 0.0 { 1.0 } else { -1.0 };
    (px + sign * nx * side, py + sign * ny * side)
}

/// The label centre `side` pixels off the middle of the segment `a`-`b`,
/// on the upper side.
pub fn segment_label(a: (f64, f64), b: (f64, f64), side: f64) -> (f64, f64) {
    let (mx, my) = (0.5 * (a.0 + b.0), 0.5 * (a.1 + b.1));
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx.hypot(dy);
    if len == 0.0 {
        return (mx, my - side);
    }
    let (mut nx, mut ny) = (-dy / len, dx / len);
    if ny > 0.0 || (ny == 0.0 && nx < 0.0) {
        nx = -nx;
        ny = -ny;
    }
    (mx + nx * side, my + ny * side)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f64 = 1000.0;
    const H: f64 = 800.0;

    #[test]
    fn a_line_leaving_through_the_left_edge_is_labelled_there() {
        // Slightly descending towards the right, crossing x = 0 at y = 140.
        let e = line_entry((500.0, 200.0), (500.0, 60.0), W, H).unwrap();
        assert!(e.x.abs() < 1e-6 && (e.y - 140.0).abs() < 1e-6);
        assert!(e.dx > 0.0, "points into the area");
        let (x, y) = line_label(e, W, H, 24.0, 11.0);
        assert!((x - 24.0).abs() < 3.0);
        // The middle of the area is below, so the label is below the line.
        assert!(y > e.y + 5.0);
    }

    #[test]
    fn a_vertical_line_is_labelled_at_the_top_next_to_the_line() {
        let e = line_entry((233.0, 400.0), (0.0, 1.0), W, H).unwrap();
        assert_eq!((e.x, e.y), (233.0, 0.0));
        assert!(e.dy > 0.0 && e.dx.abs() < 1e-9);
        let (x, y) = line_label(e, W, H, 20.0, 11.0);
        // Middle of the area is to the right: the label is right of the line.
        assert!((x - 244.0).abs() < 1e-6 && (y - 20.0).abs() < 1e-6);
    }

    #[test]
    fn a_line_left_of_centre_entering_at_the_top_puts_the_label_on_the_inner_side() {
        // Steep line crossing the top edge at x = 604 of a 1000 px area... mirrored:
        let e = line_entry((800.0, 0.0), (-0.3, 1.0), W, H).unwrap();
        assert!(e.y.abs() < 1e-6);
        let (x, _) = line_label(e, W, H, 20.0, 11.0);
        // The centre (500) is to the left of the entry (800): label is left.
        assert!(x < e.x + e.dx * 20.0);
    }

    #[test]
    fn a_line_outside_the_area_has_no_entry() {
        assert!(line_entry((-50.0, 10.0), (0.0, 1.0), W, H).is_none());
        assert!(line_entry((0.0, 0.0), (0.0, 0.0), W, H).is_none());
    }

    #[test]
    fn a_horizontal_line_enters_at_the_left() {
        let e = line_entry((300.0, 100.0), (1.0, 0.0), W, H).unwrap();
        assert_eq!((e.x, e.y), (0.0, 100.0));
        assert!(e.dx > 0.0);
    }

    #[test]
    fn segment_labels_sit_above_the_middle() {
        let (x, y) = segment_label((0.0, 100.0), (100.0, 100.0), 10.0);
        assert_eq!((x, y), (50.0, 90.0));
        let (_, y) = segment_label((100.0, 100.0), (0.0, 100.0), 10.0);
        assert_eq!(y, 90.0);
        assert_eq!(segment_label((5.0, 5.0), (5.0, 5.0), 10.0), (5.0, -5.0));
    }
}
