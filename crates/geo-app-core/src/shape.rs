//! Plane geometry of polygons on the screen: filling a polygon that need not
//! be convex, and asking whether a point lies inside one. No UI types.

/// Twice the signed area of `pts` (positive when counter-clockwise in a
/// y-up plane).
fn signed_area2(pts: &[(f64, f64)]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum()
}

fn cross(o: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

/// Whether `p` is inside the triangle `a b c` of orientation `sign`, edges
/// included.
fn in_triangle(p: (f64, f64), [a, b, c]: [(f64, f64); 3], sign: f64) -> bool {
    cross(a, b, p) * sign >= 0.0 && cross(b, c, p) * sign >= 0.0 && cross(c, a, p) * sign >= 0.0
}

/// Triangles (as indices into `pts`) that fill the simple polygon `pts`, by
/// ear clipping. A polygon that crosses itself is filled as a fan from its
/// first vertex once no ear is left, which is what it looks like for the
/// slightly broken shapes a drag can make.
pub fn triangulate(pts: &[(f64, f64)]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let sign = if signed_area2(pts) >= 0.0 { 1.0 } else { -1.0 };
    let mut left: Vec<usize> = (0..n).collect();
    let mut out = Vec::with_capacity(n - 2);
    while left.len() > 3 {
        let m = left.len();
        let ear = (0..m).find(|&i| {
            let (a, b, c) = (left[(i + m - 1) % m], left[i], left[(i + 1) % m]);
            let tri = [pts[a], pts[b], pts[c]];
            cross(tri[0], tri[1], tri[2]) * sign > 0.0
                && left
                    .iter()
                    .filter(|&&j| j != a && j != b && j != c)
                    .all(|&j| !in_triangle(pts[j], tri, sign))
        });
        let Some(i) = ear else {
            // No ear: fan out what is left.
            out.extend((1..m - 1).map(|k| [left[0], left[k], left[k + 1]]));
            return out;
        };
        out.push([left[(i + m - 1) % m], left[i], left[(i + 1) % m]]);
        left.remove(i);
    }
    out.push([left[0], left[1], left[2]]);
    out
}

/// Whether `p` is inside the polygon `pts` (even-odd rule).
pub fn contains(pts: &[(f64, f64)], p: (f64, f64)) -> bool {
    let n = pts.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + n - 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) * (b.0 - a.0) / (b.1 - a.1) {
            inside = !inside;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(pts: &[(f64, f64)], tris: &[[usize; 3]]) -> f64 {
        tris.iter()
            .map(|&[a, b, c]| cross(pts[a], pts[b], pts[c]).abs() / 2.0)
            .sum()
    }

    #[test]
    fn a_concave_polygon_is_covered_exactly_once() {
        // An L shape of area 3, clockwise and counter-clockwise.
        let l = [
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 1.0),
            (1.0, 1.0),
            (1.0, 2.0),
            (0.0, 2.0),
        ];
        let tris = triangulate(&l);
        assert_eq!(tris.len(), 4);
        assert!((area(&l, &tris) - 3.0).abs() < 1e-12);
        let mut r = l;
        r.reverse();
        assert!((area(&r, &triangulate(&r)) - 3.0).abs() < 1e-12);
        assert!(triangulate(&l[..2]).is_empty());
    }

    #[test]
    fn inside_follows_the_even_odd_rule() {
        let l = [
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 1.0),
            (1.0, 1.0),
            (1.0, 2.0),
            (0.0, 2.0),
        ];
        assert!(contains(&l, (0.5, 1.5)));
        assert!(contains(&l, (1.5, 0.5)));
        assert!(!contains(&l, (1.5, 1.5)));
        assert!(!contains(&l, (3.0, 0.5)));
    }
}
