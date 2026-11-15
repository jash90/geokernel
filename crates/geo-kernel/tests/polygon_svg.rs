//! Polygons and polylines in the SVG drawing.

use geo_kernel::script;
use geo_kernel::svg::{render, View};

#[test]
fn polygons_are_filled_and_polylines_leave_holes() {
    let c = script::load(
        "A = (0, 0)\nB = (4, 0)\nC = (1, 3)\np = Polygon(A, B, C)\n\
         O = (10, 0)\nr = 1\nk = Circle(O, r)\nl = Line(A, C)\nX = Intersect(l, k, 1)\n\
         D = (6, 6)\npl = PolyLine(A, B, X, C, D)\n",
    )
    .unwrap();
    let view = View::fit(&c);
    // the polygon and the polyline are inside the view
    assert!(view.xmin <= 0.0 && view.xmax >= 11.0 && view.ymax >= 6.0);
    let svg = render(&c, view, 400, 300);
    assert_eq!(svg.matches("<polygon points=").count(), 1);
    // X is undefined: the polyline is drawn as A-B and C-D
    assert_eq!(svg.matches("<polyline points=").count(), 2);
}
