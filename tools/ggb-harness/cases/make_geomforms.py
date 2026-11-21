#!/usr/bin/env python3
"""GeoGebra command overloads for lines, measures, paths and polygons."""

from caselib import Cases, mv, num, pt, setn

cases = Cases("geomforms")
case = cases.case

for start, end, through in [
    ((-2, 3), (4, 1), (3, 7)),
    ((1, 1), (1, 5), (0, 0)),
    ((3, 2), (-4, 2), (-3, 2)),
]:
    case("ray", [pt("A", *start), pt("B", *end), pt("P", *through),
                 "r = Ray(A, B)", "l = Line(P, r)", "m = OrthogonalLine(P, r)",
                 "t = Slope(r)", "d = Distance(P, r)", "u = Distance(r, P)",
                 "v = Distance(l, r)", "W = ClosestPoint(r, m)"],
         [mv("P", *start), mv("B", *start), mv("B", 0, 6), mv("P", -4, 2)])

case("normal", [pt("A", 0, 0), pt("P", 5, 2), num("r", 3),
                "k = Circle(A, r)", "l = OrthogonalLine(P, k)"],
     [mv("A", -2, 1), setn("r", 0), setn("r", 2), mv("P", 0, 1), mv("P", 1, 2)])
case("normal", [pt("A", -3, 0), pt("B", 3, 0), pt("P", 4, 3), num("r", 5),
                "k = Ellipse(A, B, r)", "l = OrthogonalLine(P, k)"],
     [setn("r", 4), mv("P", 0, 0), mv("P", -2, 4), mv("B", -3, 0)])

for length in (0, 2, 4.5):
    case("segment", [pt("A", 2, -1), num("n", length), "s = Segment(A, n)"],
         [mv("A", 0, 3), setn("n", 5), setn("n", 0), setn("n", -2), setn("n", 2)])
case("segment", [pt("A", 2, 2), "s = Segment(A, 2.5)"], [mv("A", -3, 1)])
case("segment", [pt("A", 0, 0), pt("B", 3, 0), pt("C", 0, 2),
                 "p = Polygon(A, B, C)", "s = Segment(A, B, p)"],
     [mv("B", 0, 0), mv("B", 2, 0)])

for f in ("a * x + 3", "x^2 + a * x", "a * sin(x)", "sqrt(1 + x)"):
    case("slope", [num("a", 3), f"f(x) = {f}", "m = Slope(f)"],
         [setn("a", -4), setn("a", 0), setn("a", 2.5)])

case("area", [pt("A", -3, 0), pt("B", 3, 0), num("r", 5),
              "k = Ellipse(A, B, r)", "s = Area(k)", "q = Radius(k)"],
     [setn("r", 3), setn("r", 2), setn("r", 4), mv("B", -3, 0)])
case("area", [pt("A", 0, 0), pt("B", 3, 0), pt("C", 0, 2),
              "k = CircularArc(A, B, C)", "s = CircularSector(A, B, C)",
              "r = Radius(k)", "q = Radius(s)"],
     [mv("B", 0, 0), mv("B", 6, 0), mv("C", 0, -2)])
case("area", [pt("A", -3, 0), pt("B", 3, 0), num("r", 2),
              "k = Hyperbola(A, B, r)", "s = Area(k)", "q = Radius(k)"],
     [setn("r", 4), mv("B", -3, 0)])

for index in (1, 2, 3, 4, 5, 6, 9, 13, 99, 100, 354, 999, 1500, 2201, 2925, 3053):
    case("trianglecenter", [pt("A", -2, 1), pt("B", 6, -1), pt("C", 1, 5),
                            num("n", index), "X = TriangleCenter(A, B, C, n)"],
         [mv("C", 2, 0), mv("C", -2, 1), mv("C", 1, 5),
          setn("n", 1.9), setn("n", 2), setn("n", 3054), setn("n", index)])
case("trianglecenter", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 3),
                        "X = TriangleCenter(A, B, C, 2)"], [mv("A", 4, 0), mv("C", 4, 0)])

for path, script, steps in [
    ("k", [pt("A", 0, 0), num("r", 3), "k = Circle(A, r)"],
     [setn("r", 1), setn("r", 0), mv("A", 2, 1), setn("r", 3)]),
    ("k", [pt("A", -3, 0), pt("B", 3, 0), num("r", 5), "k = Ellipse(A, B, r)"],
     [setn("r", 4), mv("B", 0, 3), mv("P", 0, 0)]),
    ("k", [pt("A", -3, 0), pt("B", 3, 0), num("r", 2), "k = Hyperbola(A, B, r)"],
     [setn("r", 1), mv("P", -4, -3), mv("P", 0, 0)]),
    ("k", [pt("A", 0, 1), pt("B", 0, -1), pt("C", 3, -1),
           "l = Line(B, C)", "k = Parabola(A, l)"],
     [mv("P", -3, 4), mv("P", 0, 5), mv("A", 0, 0)]),
    ("k", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 4), "k = CircularArc(A, B, C)"],
     [mv("P", -4, 2), mv("C", 0, -4), mv("B", 0, 0)]),
    ("k", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 4), "k = CircularSector(A, B, C)"],
     [mv("P", -4, 2), mv("C", 0, -4)]),
    ("k", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 4), "k = Polygon(A, B, C)"],
     [mv("P", 1, 1), mv("B", 0, 0), mv("B", 4, 4)]),
    ("k", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 4), "k = PolyLine(A, B, C)"],
     [mv("P", 1, 1), mv("B", 0, 0), mv("B", 4, 4)]),
    ("k", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 4), "k = {A, B, C}"],
     [mv("P", 1, 1), mv("A", 4, 0), mv("C", 1, 1)]),
    ("f", [num("a", 2), "f(x) = a * x + 1"],
     [setn("a", 0), setn("a", -3), mv("P", 1, 1)]),
    ("f", [num("a", 1), "f(x) = a * x^2"],
     [setn("a", 0.25), mv("P", -2, 4), setn("a", 0)]),
]:
    case("closest", script + [pt("P", 6, 2), f"X = ClosestPoint({path}, P)",
                               f"Y = ClosestPoint(P, {path})", f"d = Distance(P, {path})",
                               f"q = Distance({path}, P)"], steps)

case("tangent", [pt("A", -3, 0), pt("B", 3, 0), pt("P", -4, 4), pt("Q", 4, 4), num("r", 5),
                 "k = Ellipse(A, B, r)", "l = Line(P, Q)", "s = Ray(P, Q)",
                 "u = IsTangent(l, k)", "v = IsTangent(s, k)"],
     [setn("r", 4), mv("Q", -4, 4), mv("P", -4, 0), setn("r", 3), setn("r", 5)])
case("tangent", [pt("A", 0, 0), pt("B", 0, 2), pt("P", -4, 2), pt("Q", -2, 2),
                 "k = Circle(A, B)", "l = Ray(P, Q)", "u = IsTangent(l, k)"],
     [mv("Q", -5, 2), mv("B", 0, 3), mv("B", 0, 0)])

case("pointin", [pt("A", 1, 1), num("r", 3), "k = Circle(A, r)", "P = PointIn(k)"],
     [mv("A", 6, 3), setn("r", 2), setn("r", 0), setn("r", 5)])
case("pointin", [pt("A", 5, 2), num("r", 3), "k = Circle(A, r)", "P = PointIn(k)"],
     [mv("A", 6, 3), setn("r", 2), setn("r", 0), setn("r", 5)])
case("pointin", [pt("A", -3, -2), pt("B", 5, -2), pt("C", 0, 5),
                 "k = Polygon(A, B, C)", "P = PointIn(k)"],
     [mv("A", -5, -4), mv("C", 0, -2), mv("C", 0, 6)])
case("pointin", [pt("A", 2, 1), pt("B", 5, 1), pt("C", 2, 4),
                 "k = Polygon(A, B, C)", "P = PointIn(k)"],
     [mv("A", 2, 2), mv("C", 2, 2), mv("C", -2, 4)])
case("pointin", [pt("A", -3, 0), pt("B", 3, 0), num("r", 5),
                 "k = Ellipse(A, B, r)", "P = PointIn(k)"],
     [mv("B", 0, 3), setn("r", 4), mv("A", -4, 0)])
case("pointin", [pt("A", 1, 1), pt("B", 4, 1), pt("C", 1, 4),
                 "k = CircularSector(A, B, C)", "P = PointIn(k)"],
     [mv("A", -1, -1), mv("B", 2, -1), mv("C", -1, 2)])

case("rigidpolygon", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 1, 2),
                      "p = RigidPolygon(A, B, C)"], [mv("A", 3, 2)])
case("rigidpolygon", [pt("A", 1, 1), pt("B", 5, 1), pt("C", 2, 3),
                      "p = Polygon(A, B, C)", "q = RigidPolygon(p)"],
     [mv("B", 4, 4), mv("C", 5, 1)])
case("rigidpolygon", [pt("A", 1, 1), pt("B", 5, 1), pt("C", 2, 3), num("u", 2), num("v", -3),
                      "p = Polygon(A, B, C)", "q = RigidPolygon(p, u, v)"],
     [mv("B", 4, 4), setn("u", 4), setn("v", 2), mv("C", 5, 1)])

# Direct manipulation.
case("segment", [pt("A", 0, 0), num("n", 2), "s = Segment(A, n)"],
     [mv("B", 0, 4), setn("n", 3), mv("A", -2, 0)])
case("pointin", [pt("A", 0, 0), num("r", 2), "k = Circle(A, r)", "P = PointIn(k)"],
     [mv("P", 0, 3), mv("A", 2, 0), setn("r", 3)])
case("pointin", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 4),
                 "k = CircularArc(A, B, C)", "P = PointIn(k)"],
     [mv("C", -4, 0), mv("B", 3, 0), mv("A", 1, 1)])
case("closest", ["f(x) = sin(x)", pt("P", 2, 3),
                 "X = ClosestPoint(f, P)", "d = Distance(P, f)"],
     [mv("P", -2, -2), mv("P", 0, 0)])

cases.write()
