#!/usr/bin/env python3
"""Writes cases/predicates.json: the boolean commands.

AreCollinear, AreParallel, ArePerpendicular, AreConcurrent, AreConcyclic,
AreEqual, AreCongruent and IsTangent.

Every topic has exact configurations, configurations a few multiples of one
micro-unit away from GeoGebra's tolerances (1e-8 for most tests, 1e-5 for
AreConcyclic and IsTangent), degenerate inputs (coinciding points, a line
through one point twice, a circle of radius 0 or below, circles that do not
meet) and moves that flip the answer. The harness rounds every literal and
move to 6 decimals, so near-miss cases use small coordinates: the products
that the tests compare then land on both sides of the tolerance.

The inputs only describe what to run. Snapshots come exclusively from running
GeoGebra through the harness (`harness.sh golden predicates`).
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("predicates")
case = cases.case
topic = cases.topic
rp = cases.rp


# -------------------------------------------------------------- collinear
topic("collinear")
case("collinear", [pt("A", 0, 0), pt("B", 4, 2), pt("C", 2, 1), "t = AreCollinear(A, B, C)"],
     [mv("C", 2, 1.000001), mv("C", 6, 3), mv("B", 0, 0), mv("B", 4, 2), mv("A", 2, 1),
      mv("C", 2, 1), mv("A", -2, -1)])
case("collinear", [pt("A", 1, 1), pt("B", 1, 5), pt("C", 1, -3), "t = AreCollinear(A, B, C)",
                   "u = AreCollinear(C, A, B)", "v = AreCollinear(B, C, A)"],
     [mv("C", 1.000001, -3), mv("C", 0.999999, 7), mv("C", 1, 100), mv("A", 1, 1000000)])
for _ in range(3):
    a, b, c = rp(), rp(), rp()
    case("collinear", [pt("A", *a), pt("B", *b), pt("C", *c), "t = AreCollinear(A, B, C)"],
         [mv("C", *rp()), mv("C", *a), mv("B", *a), mv("B", *rp()), mv("C", *b)])
# Around the tolerance 1e-8 of the cross product (A - B) x (A - C).
case("collinear", [pt("A", 0, 0), pt("B", 0.001, 0), pt("C", 0.002, 0.000004),
                   "t = AreCollinear(A, B, C)"],
     [mv("C", 0.002, 0.000009), mv("C", 0.002, 0.00001), mv("C", 0.002, 0.000011),
      mv("C", 0.002, 0.00002), mv("C", 0.002, -0.000004), mv("C", 0.002, -0.00001),
      mv("C", 0.002, -0.000011), mv("C", 0.002, 0)])
case("collinear", [pt("A", 0, 0), pt("B", 0.0001, 0.0001), pt("C", 0.0002, 0.000199),
                   "t = AreCollinear(A, B, C)"],
     [mv("C", 0.0002, 0.0002), mv("C", 0.0002, 0.0002001), mv("C", 0.0002, 0.000201),
      mv("C", 0.0002, 0.0001999), mv("C", 0.0002, 0.000199), mv("B", 0.001, 0.001)])
case("collinear", [pt("A", 0, 0), pt("B", 100000, 1), pt("C", 200000, 2),
                   "t = AreCollinear(A, B, C)"],
     [mv("C", 200000, 2.000001), mv("C", 200000, 2), mv("C", 200000.000001, 2),
      mv("B", 100000, 1.000001)])
# Coincident points: any three points of which two coincide are collinear.
case("collinear", [pt("A", 3, 4), pt("B", 3, 4), pt("C", 9, -2), "t = AreCollinear(A, B, C)"],
     [mv("B", 3.000001, 4), mv("B", 3.00001, 4), mv("C", 3, 4), mv("C", 3.1, 4.1)])
# Computed points.
case("collinear", [pt("A", 1, 2), pt("B", 7, -3), "M = Midpoint(A, B)",
                   "t = AreCollinear(A, M, B)", "u = AreCollinear(M, A, B)"],
     [mv("B", -2, 5), mv("B", 1, 2), mv("A", 1.5, 0.25), mv("B", 123.456, 654.321)])
case("collinear", [pt("A", 0, 0), pt("B", 4, 1), pt("C", 1, 5), pt("D", 3, -2),
                   "l = Line(A, B)", "m = Line(C, D)", "P = Intersect(l, m)",
                   "t = AreCollinear(A, B, P)", "u = AreCollinear(C, D, P)",
                   "v = AreCollinear(A, C, P)"],
     [mv("D", 3, 5), mv("D", 5, 6), mv("D", 3, 5), mv("C", 1, 6), mv("D", 2, 6)])
case("collinear", [pt("A", 0, 0), num("r", 1), pt("B", 1.5, 0), "c = Circle(A, r)",
                   "d = Circle(B, r)", "P = Intersect(c, d, 1)", "t = AreCollinear(A, B, P)"],
     [setn("r", 0.5), setn("r", 0.75), setn("r", 0.8), setn("r", 3), setn("r", 0.5)])
case("collinear", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 2, 3), "s = Segment(A, B)",
                   "M = Midpoint(A, B)", "t = AreCollinear(A, B, C)", "u = AreCollinear(A, M, B)"],
     [mv("C", 2, 0), mv("C", 10, 0), mv("C", 10, 0.000001)])

# --------------------------------------------------------------- parallel
topic("parallel")
case("parallel", [pt("A", 0, 0), pt("B", 4, 2), pt("C", 1, 5), pt("D", 5, 7),
                  "l = Line(A, B)", "m = Line(C, D)", "t = AreParallel(l, m)",
                  "u = AreParallel(m, l)", "v = AreParallel(l, l)"],
     [mv("D", 5, 7.000001), mv("D", 5, 6.9), mv("D", 3, 6), mv("B", 8, 4), mv("D", 5, 7)])
for _ in range(3):
    a, b, c, d = rp(), rp(), rp(), rp()
    case("parallel", [pt("A", *a), pt("B", *b), pt("C", *c), pt("D", *d),
                      "l = Line(A, B)", "m = Line(C, D)", "t = AreParallel(l, m)"],
         [mv("D", *rp()), mv("D", c[0] + (b[0] - a[0]), c[1] + (b[1] - a[1])),
          mv("B", *rp()), mv("B", *a)])
# Around the tolerance 1e-8 of g.x * y - g.y * x (raw coefficients).
case("parallel", [pt("A", 0, 0), pt("B", 0.001, 0), pt("C", 0, 1), pt("D", 0.001, 1.000004),
                  "l = Line(A, B)", "m = Line(C, D)", "t = AreParallel(l, m)"],
     [mv("D", 0.001, 1.000009), mv("D", 0.001, 1.00001), mv("D", 0.001, 1.000011),
      mv("D", 0.001, 1.00002), mv("D", 0.001, 0.999991), mv("D", 0.001, 0.99999),
      mv("D", 0.001, 0.999989), mv("D", 0.001, 1)])
case("parallel", [pt("A", 0, 0), pt("B", 1, 0), pt("C", 0, 1), pt("D", 1, 1.000001),
                  "l = Line(A, B)", "m = Line(C, D)", "t = AreParallel(l, m)"],
     [mv("D", 1, 1), mv("D", 1000, 1.000001), mv("B", 1000, 0), mv("D", 1000, 1.01)])
case("parallel", [pt("A", 0, 0), pt("B", 100, 0.000001), pt("C", 0, 2), pt("D", 100, 2),
                  "l = Line(A, B)", "m = Line(C, D)", "t = AreParallel(l, m)"],
     [mv("B", 100, 0.0001), mv("B", 100, 0), mv("B", 1000000, 0.000001),
      mv("B", 1000000, 0.01)])
# Segments and rays count as the lines they lie on.
case("parallel", [pt("A", 0, 0), pt("B", 2, 1), pt("C", 0, 3), pt("D", 4, 5), pt("E", 1, 9),
                  pt("F", 5, 11), "s = Segment(A, B)", "r = Ray(C, D)", "l = Line(E, F)",
                  "t = AreParallel(s, r)", "u = AreParallel(r, l)", "v = AreParallel(s, l)"],
     [mv("D", 4, 3.5), mv("D", 2, 1.5), mv("F", 5, 10.5), mv("B", -2, -1), mv("A", 1, 1)])
# Lines made by other commands.
case("parallel", [pt("A", 0, 0), pt("B", 4, 2), pt("C", 1, 5), "l = Line(A, B)",
                  "m = Line(C, l)", "n = Line(A, m)", "t = AreParallel(l, m)",
                  "u = AreParallel(m, n)", "v = AreParallel(l, n)"],
     [mv("B", 0, 7), mv("C", 3, 3), mv("B", 0, 0), mv("B", 5, 5)])
case("parallel", [pt("A", 0, 0), pt("B", 4, 2), pt("C", 1, 5), pt("D", 2, 2),
                  "l = Line(A, B)", "m = PerpendicularLine(C, l)", "n = PerpendicularLine(D, m)",
                  "t = AreParallel(l, n)", "u = AreParallel(l, m)"],
     [mv("B", 7, 1), mv("D", 3, 9), mv("B", 0, 0)])
# A line through one point twice has all-zero coefficients.
case("parallel", [pt("A", 0, 0), pt("B", 4, 2), pt("C", 1, 5), pt("D", 3, 3), "l = Line(A, B)",
                  "m = Line(C, D)", "t = AreParallel(l, m)", "u = AreParallel(m, l)",
                  "v = AreParallel(m, m)"],
     [mv("D", 1, 5), mv("B", 4, 3), mv("C", 7, 7), mv("D", 7, 7), mv("B", 0, 0), mv("D", 3, 3)])
case("parallel", [pt("A", 0, 0), pt("B", 4, 2), pt("C", 1, 5), pt("D", 5, 7),
                  "l = Line(A, B)", "m = Line(C, D)", "t = AreParallel(l, m)"],
     [mv("B", 0, 0), mv("D", 1, 5), mv("B", 4, 2), mv("D", 5, 7)])

# ---------------------------------------------------------- perpendicular
topic("perpendicular")
case("perpendicular", [pt("A", 0, 0), pt("B", 4, 2), pt("C", 1, 5), pt("D", 0, 7),
                       "l = Line(A, B)", "m = Line(C, D)", "t = ArePerpendicular(l, m)",
                       "u = ArePerpendicular(m, l)", "v = ArePerpendicular(l, l)"],
     [mv("D", 3, 3), mv("D", 3.000001, 3), mv("D", 2, -4), mv("B", 1, 1), mv("B", 4, 2)])
for _ in range(3):
    a, b, c, d = rp(), rp(), rp(), rp()
    case("perpendicular", [pt("A", *a), pt("B", *b), pt("C", *c), pt("D", *d),
                           "l = Line(A, B)", "m = Line(C, D)", "t = ArePerpendicular(l, m)"],
         [mv("D", *rp()), mv("D", c[0] - (b[1] - a[1]), c[1] + (b[0] - a[0])),
          mv("B", *rp()), mv("B", *a)])
# Around the tolerance 1e-8 of g.x * x + g.y * y (raw coefficients).
case("perpendicular", [pt("A", 0, 0), pt("B", 0.001, 0), pt("C", 0, 0), pt("D", 0.000004, 1),
                       "l = Line(A, B)", "m = Line(C, D)", "t = ArePerpendicular(l, m)"],
     [mv("D", 0.000009, 1), mv("D", 0.00001, 1), mv("D", 0.000011, 1),
      mv("D", 0.00002, 1), mv("D", -0.000004, 1), mv("D", -0.00001, 1),
      mv("D", -0.000011, 1), mv("D", 0, 1)])
case("perpendicular", [pt("A", 0, 0), pt("B", 1, 0), pt("C", 0, 0), pt("D", 0.000001, 1),
                       "l = Line(A, B)", "m = Line(C, D)", "t = ArePerpendicular(l, m)"],
     [mv("D", 0, 1), mv("D", 0.00001, 1), mv("B", 0.001, 0), mv("D", 0.00001, 0.001)])
case("perpendicular", [pt("A", 0, 0), pt("B", 3, 4), pt("C", 1, 1), pt("D", 5, -2), pt("E", 6, 6),
                       pt("F", 2, 8), "s = Segment(A, B)", "r = Ray(C, D)", "l = Line(E, F)",
                       "t = ArePerpendicular(s, r)", "u = ArePerpendicular(r, l)",
                       "v = ArePerpendicular(s, l)"],
     [mv("D", 5, -2.5), mv("D", 4, 4.0), mv("F", 10, 3), mv("B", 4, -3), mv("A", 1, 1)])
case("perpendicular", [pt("A", 0, 0), pt("B", 4, 2), pt("C", 1, 5), "l = Line(A, B)",
                       "m = PerpendicularLine(C, l)", "n = OrthogonalLine(C, l)",
                       "k = Line(A, C)", "o = LineBisector(A, C)", "t = ArePerpendicular(l, m)",
                       "u = ArePerpendicular(l, n)", "v = ArePerpendicular(o, k)"],
     [mv("B", 9, -3), mv("C", 3, 1), mv("B", 0, 0), mv("C", 0, 0)])
case("perpendicular", [pt("A", 0, 0), pt("B", 4, 2), pt("C", 1, 5), pt("D", 3, 3), "l = Line(A, B)",
                       "m = Line(C, D)", "t = ArePerpendicular(l, m)",
                       "u = ArePerpendicular(m, m)"],
     [mv("D", 1, 5), mv("B", 4, 3), mv("C", 7, 7), mv("D", 7, 7), mv("B", 0, 0), mv("D", 3, 3)])

# ------------------------------------------------------------- concurrent
topic("concurrent")
case("concurrent", [pt("A", 0, 0), pt("B", 2, 2), pt("C", 0, 4), pt("D", 2, 2), pt("E", 0, 2),
                    pt("F", 4, 2), "l = Line(A, B)", "m = Line(C, D)", "n = Line(E, F)",
                    "t = AreConcurrent(l, m, n)"],
     [mv("F", 4, 2.000001), mv("F", 4, 3), mv("F", 5, 2), mv("D", 4, 0), mv("D", 2, 2)])
for _ in range(3):
    p = rp(-4, 4)
    a, b, c = rp(), rp(), rp()
    case("concurrent", [pt("P", *p), pt("A", *a), pt("B", *b), pt("C", *c),
                        "l = Line(P, A)", "m = Line(P, B)", "n = Line(P, C)",
                        "t = AreConcurrent(l, m, n)"],
         [mv("C", *rp()), mv("P", *rp()), mv("A", *rp()), mv("B", *rp())])
# Altitudes of a triangle meet in the orthocentre.
case("concurrent", [pt("A", 0, 0), pt("B", 6, 0), pt("C", 2, 5), "k = Line(B, C)",
                    "q = Line(A, C)", "r = Line(A, B)", "a = PerpendicularLine(A, k)",
                    "b = PerpendicularLine(B, q)", "c = PerpendicularLine(C, r)",
                    "t = AreConcurrent(a, b, c)"],
     [mv("C", 3, 7), mv("B", 9, 2), mv("A", -4, 3)])
case("concurrent", [pt("A", 0, 0), pt("B", 6, 0), pt("C", 2, 5), "a = LineBisector(A, B)",
                    "b = LineBisector(B, C)", "c = LineBisector(A, C)",
                    "t = AreConcurrent(a, b, c)"],
     [mv("C", 3, 7), mv("B", 9, 2), mv("A", -4, 3)])
# Around the tolerance 1e-8 of the determinant of the raw coefficients.
case("concurrent", [pt("A", 0, 0), pt("B", 0.2, 0.2), pt("C", 0, 0.2), pt("D", 0.2, 0),
                    pt("E", 0, 0.1), pt("F", 0.2, 0.1), "l = Line(A, B)", "m = Line(C, D)",
                    "n = Line(E, F)", "t = AreConcurrent(l, m, n)"],
     [mv("F", 0.2, 0.100001), mv("F", 0.2, 0.100002), mv("F", 0.2, 0.10001),
      mv("F", 0.2, 0.1001), mv("F", 0.2, 0.099999), mv("F", 0.2, 0.099998),
      mv("F", 0.2, 0.1)])
case("concurrent", [pt("A", 0, 0), pt("B", 0.1, 0.1), pt("C", 0, 0.1), pt("D", 0.1, 0),
                    pt("E", 0, 0.05), pt("F", 0.1, 0.05), "l = Line(A, B)", "m = Line(C, D)",
                    "n = Line(E, F)", "t = AreConcurrent(l, m, n)"],
     [mv("F", 0.1, 0.050001), mv("F", 0.1, 0.05001), mv("F", 0.1, 0.0501),
      mv("F", 0.1, 0.049999), mv("F", 0.1, 0.05)])
# Parallel lines and segments or rays among the arguments.
case("concurrent", [pt("A", 0, 0), pt("B", 1, 1), pt("C", 0, 1), pt("D", 1, 2), pt("E", 0, 2),
                    pt("F", 1, 3), "l = Line(A, B)", "m = Line(C, D)", "n = Line(E, F)",
                    "t = AreConcurrent(l, m, n)"],
     [mv("F", 1, 4), mv("D", 2, 4), mv("D", 1, 2)])
case("concurrent", [pt("A", 0, 0), pt("B", 2, 0), pt("C", 5, 5), pt("D", 5, 3), pt("E", 8, 0),
                    pt("F", 10, 1), "s = Segment(A, B)", "r = Ray(C, D)", "l = Line(E, F)",
                    "t = AreConcurrent(s, r, l)", "u = AreConcurrent(l, s, r)"],
     [mv("F", 10, 2), mv("D", 6, 3), mv("C", 5, 6), mv("B", 2, 1)])
case("concurrent", [pt("A", 0, 0), pt("B", 1, 1), pt("C", 0, 1), pt("D", 1, 0), pt("E", 3, 1),
                    "l = Line(A, B)", "m = Line(C, D)", "n = Line(A, E)", "t = AreConcurrent(l, m, n)",
                    "u = AreConcurrent(l, l, m)"],
     [mv("E", 0, 0), mv("D", 1, 0.5), mv("B", 0, 0), mv("E", 3, 1)])

# ------------------------------------------------------------- concyclic
topic("concyclic")
case("concyclic", [pt("A", 1, 0), pt("B", 0, 1), pt("C", -1, 0), pt("D", 0, -1),
                   "t = AreConcyclic(A, B, C, D)"],
     [mv("D", 0, -1.000001), mv("D", 0, -1.000004), mv("D", 0, -1.000006),
      mv("D", 0, -1.00001), mv("D", 0, -0.999996), mv("D", 0, -0.99999),
      mv("D", 0, -1), mv("D", 0.6, -0.8), mv("D", 2, 0)])
case("concyclic", [pt("A", 3, 4), pt("B", 4, 3), pt("C", 5, 0), pt("D", 0, -5),
                   "t = AreConcyclic(A, B, C, D)", "u = AreConcyclic(D, C, B, A)",
                   "v = AreConcyclic(B, A, D, C)"],
     [mv("D", 0, -5.000001), mv("D", 0, -5.00001), mv("D", 0, -5.0001), mv("D", -3, -4),
      mv("C", 5, 0.00001), mv("C", 0, 5)])
for _ in range(3):
    a, b, c, d = rp(), rp(), rp(), rp()
    case("concyclic", [pt("A", *a), pt("B", *b), pt("C", *c), pt("D", *d),
                       "t = AreConcyclic(A, B, C, D)"],
         [mv("D", *rp()), mv("D", *a), mv("C", *b), mv("B", *rp())])
# Points of a circle given by its centre and a point.
case("concyclic", [pt("O", 0, 0), pt("A", 5, 0), "c = Circle(O, A)", pt("B", 3, 4), pt("C", -4, 3),
                   pt("D", 0, -5), "t = AreConcyclic(A, B, C, D)", "u = AreConcyclic(O, A, B, C)"],
     [mv("D", 0, -5.000004), mv("D", 0, -5.000006), mv("D", 0, -4.999994),
      mv("D", 0, -4.999996), mv("O", 0.000001, 0), mv("O", 1, 1)])
# Four collinear points satisfy Ptolemy's equality; so do coinciding points.
case("concyclic", [pt("A", 0, 0), pt("B", 1, 1), pt("C", 2, 2), pt("D", 5, 5),
                   "t = AreConcyclic(A, B, C, D)"],
     [mv("D", 5, 5.000001), mv("D", 5, 5.00001), mv("D", 5, 5.01), mv("D", 0, 0),
      mv("B", 0, 0), mv("C", 0, 0)])
case("concyclic", [pt("A", 1, 0), pt("B", 0, 1), pt("C", -1, 0), pt("D", 1, 0),
                   "t = AreConcyclic(A, B, C, D)"],
     [mv("D", 1, 0.000001), mv("D", 1, 0.00001), mv("D", 0, 1), mv("B", 1, 0),
      mv("B", 0.5, 0.5)])
case("concyclic", [pt("A", 0, 0), pt("B", 0.001, 0), pt("C", 0.001, 0.001), pt("D", 0, 0.001),
                   "t = AreConcyclic(A, B, C, D)"],
     [mv("D", 0, 0.002), mv("D", 0, 0.0011), mv("D", 0.0005, 0.001), mv("B", 0.002, 0)])
case("concyclic", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 3), pt("D", 4, 3), "M = Midpoint(A, D)",
                   "N = Midpoint(B, C)", "t = AreConcyclic(A, B, C, D)",
                   "u = AreConcyclic(M, N, A, D)"],
     [mv("D", 4, 3.000001), mv("D", 4, 3.0001), mv("C", 0, 2), mv("B", 4, 1)])
# An undefined point: circles that do not meet.
case("concyclic", [pt("A", 0, 0), pt("B", 1, 0), pt("C", 0, 1), num("r", 1), pt("E", 1.5, 0),
                   "c = Circle(A, r)", "d = Circle(E, r)", "P = Intersect(c, d, 1)",
                   "t = AreConcyclic(A, B, C, P)"],
     [setn("r", 0.5), setn("r", 0.75), setn("r", 0.8), mv("E", 20, 0), mv("E", 1, 0)])

# -------------------------------------------------------------- tangent
topic("tangent")
case("tangent", [pt("A", 0, 1), pt("B", 1, 1), pt("O", 0, 0), num("r", 1), "l = Line(A, B)",
                 "c = Circle(O, r)", "t = IsTangent(l, c)"],
     [setn("r", 1.000001), setn("r", 0.999999), setn("r", 1.00001), setn("r", 0.5),
      setn("r", 2), setn("r", 1), setn("r", 0), setn("r", -1), setn("r", 1)])
case("tangent", [pt("A", 0, 100000), pt("B", 1, 100000), pt("O", 0, 0), num("r", 100000),
                 "l = Line(A, B)", "c = Circle(O, r)", "t = IsTangent(l, c)"],
     [setn("r", 100000.000001), setn("r", 99999.999999), setn("r", 100000.0001),
      setn("r", 100000.01), setn("r", 100000), setn("r", 99999.99)])
case("tangent", [pt("A", 0, 0.001), pt("B", 1, 0.001), pt("O", 0, 0), num("r", 0.001),
                 "l = Line(A, B)", "c = Circle(O, r)", "t = IsTangent(l, c)"],
     [setn("r", 0.001001), setn("r", 0.000999), setn("r", 0.0010001), setn("r", 0.0009999),
      setn("r", 0.001)])
for _ in range(3):
    o, a, b = rp(), rp(), rp()
    case("tangent", [pt("O", *o), pt("A", *a), pt("B", *b), pt("P", *rp()), "c = Circle(O, P)",
                     "l = Line(A, B)", "t = IsTangent(l, c)"],
         [mv("B", *rp()), mv("P", *rp()), mv("A", *rp()), mv("O", *rp())])
# A line through a point of the circle, perpendicular to the radius.
case("tangent", [pt("O", 0, 0), pt("P", 3, 4), pt("Q", 7, 1), "c = Circle(O, P)", "r = Line(O, P)",
                 "l = PerpendicularLine(P, r)", "m = Line(P, Q)", "t = IsTangent(l, c)",
                 "u = IsTangent(m, c)"],
     [mv("P", 5, 12), mv("Q", 3, 4), mv("P", 0, 0), mv("P", 3, 4)])
# Tangents made by Tangent are tangent by construction.
case("tangent", [pt("O", 0, 0), pt("P", 5, 1), pt("R", 2, 0), "c = Circle(O, R)",
                 "t = Tangent(P, c)", "u = IsTangent(t_{1}, c)", "v = IsTangent(t_{2}, c)"],
     [mv("P", 1, 0), mv("P", 2, 0), mv("P", 3, 3), mv("R", 4, 0), mv("R", 0, 0)])
case("tangent", [pt("O", 0, 0), pt("A", 0, 1), pt("B", 1, 2), pt("R", 2, 0), "c = Circle(O, R)",
                 "g = Line(A, B)", "t = Tangent(g, c)", "u = IsTangent(t_{1}, c)",
                 "v = IsTangent(t_{2}, c)", "w = IsTangent(g, c)"],
     [mv("B", 5, 1), mv("R", 3, 0), mv("R", 0.5, 0)])
# A tangent of another circle is not a tangent of this one.
case("tangent", [pt("O", 0, 0), pt("Q", 6, 0), pt("P", 3, 5), pt("R", 2, 0), pt("S", 2, 6),
                 "c = Circle(O, R)", "d = Circle(Q, S)", "t = Tangent(P, c)",
                 "u = IsTangent(t_{1}, c)", "v = IsTangent(t_{1}, d)"],
     [mv("S", 4, 0), mv("P", 5, 5), mv("Q", 3, 0)])
# Segments and rays count as the lines they lie on.
case("tangent", [pt("O", 0, 0), pt("A", -3, 2), pt("B", 3, 2), pt("C", 0, 2), pt("D", 1, 2),
                 pt("E", 5, 0), pt("F", 5, 1), pt("R", 2, 0), "c = Circle(O, R)",
                 "s = Segment(A, B)", "r = Ray(C, D)", "q = Segment(E, F)", "t = IsTangent(s, c)",
                 "u = IsTangent(r, c)", "v = IsTangent(q, c)"],
     [mv("R", 2.5, 0), mv("R", 2, 0), mv("E", 2, 0), mv("F", 2, 1), mv("B", 3, 2.000001)])
case("tangent", [pt("O", 0, 0), pt("A", 0, 2), pt("B", 3, 2), num("r", 2), "c = Circle(O, r)",
                 "l = Line(A, B)", "t = IsTangent(l, c)"],
     [setn("r", 1), mv("B", 0, 2), mv("B", 3, 2), mv("A", 0, 0)])
# Circles that are not defined: radius 0 and below.
case("tangent", [pt("O", 0, 0), pt("A", 0, 0), pt("B", 1, 0), num("r", 0), "c = Circle(O, r)",
                 "l = Line(A, B)", "t = IsTangent(l, c)"],
     [mv("A", 0, 0.000001), mv("A", 0, 1), setn("r", 1), mv("B", 1, 1), setn("r", -1)])

# ----------------------------------------------------------------- equal
topic("equal")
case("equal", [pt("A", 1, 2), pt("B", 1, 2), pt("C", 1, 2.000001), "t = AreEqual(A, B)",
               "u = AreEqual(A, C)"],
     [mv("B", 1.000001, 2), mv("B", 1, 3), mv("C", 1, 2), mv("A", 5, 5), mv("B", 5, 5)])
case("equal", [pt("A", 0, 0), pt("B", 4, 6), "M = Midpoint(A, B)", pt("C", 2, 3),
               "t = AreEqual(M, C)", "u = AreEqual(C, M)"],
     [mv("B", 4.000002, 6), mv("B", 4, 6), mv("C", 2.000001, 3), mv("A", 1, 1)])
case("equal", [pt("A", 0, 0), pt("B", 1, 0), pt("C", 0, 1), pt("D", 0.001, 0), pt("E", 0, 0.01),
               "p = Area(A, D, E)", "q = Area(A, B, C)", "zz = 0", "t = AreEqual(p, zz)",
               "u = AreEqual(p, q)", "v = AreEqual(q, q)"],
     [mv("D", 0.002, 0), mv("D", 0.003, 0), mv("E", 0, 0.001), mv("E", 0, 0.00001),
      mv("D", 0.00001, 0), mv("B", 2, 0)])
case("equal", [pt("A", 0, 0), pt("B", 0.002, 0), pt("C", 0, 0.003), pt("D", 0.004, 0),
               pt("E", 0, 0.006), "p = Area(A, B, C)", "q = Area(A, D, E)", "r = Area(A, B, E)",
               "t = AreEqual(p, q)", "u = AreEqual(p, r)"],
     [mv("E", 0, 0.00600001), mv("E", 0, 0.0061), mv("E", 0, 0.0059), mv("D", 0.004001, 0),
      mv("D", 0.0004, 0)])
case("equal", [num("a", 1), num("b", 1), num("c", 1.000000001), "t = AreEqual(a, b)",
               "u = AreEqual(a, c)"],
     [setn("a", 2), setn("b", 2), setn("c", 2), setn("c", 2.000001), setn("a", 0), setn("b", 0)])
case("equal", [pt("A", 0, 0), pt("B", 3, 4), pt("C", 0, 0), pt("D", 6, 8), num("n", 5),
               "p = Distance(A, B)", "q = Distance(C, D)", "t = AreEqual(p, n)",
               "u = AreEqual(p, q)"],
     [mv("D", 3, 4), mv("D", 3, 4.000001), mv("D", 3.000001, 4), mv("A", 1, 1), setn("n", 0)])
# Objects of different kinds are never equal.
case("equal", [pt("A", 1, 2), num("a", 1), "v = (1, 2)", "k = Circle(A, a)",
               "t = AreEqual(A, a)", "u = AreEqual(a, A)", "w = AreEqual(A, v)",
               "xx = AreEqual(v, A)", "yy = AreEqual(k, A)"],
     [mv("A", 1, 1), setn("a", 3)])
# Lines: equal when their coefficients are proportional.
case("equal", [pt("A", 0, 0), pt("B", 2, 2), pt("C", 1, 1), pt("D", 5, 5), pt("E", 0, 1),
               pt("F", 2, 3), "l = Line(A, B)", "m = Line(C, D)", "n = Line(E, F)",
               "t = AreEqual(l, m)", "u = AreEqual(l, n)", "v = AreEqual(l, l)"],
     [mv("D", 5, 5.000001), mv("D", 5, 5.00001), mv("D", 3, 3), mv("F", 2, 2), mv("E", 1, 0),
      mv("E", 9, 9), mv("B", 1, 1)])
for _ in range(2):
    a, b = rp(), rp()
    c = rp()
    case("equal", [pt("A", *a), pt("B", *b), pt("C", *c), "l = Line(A, B)", "m = Line(B, A)",
                   "n = Line(C, l)", "k = Line(A, B)", "t = AreEqual(l, m)",
                   "u = AreEqual(l, n)", "v = AreEqual(l, k)"],
         [mv("C", *rp()), mv("B", *rp()), mv("A", *b), mv("A", *rp())])
case("equal", [pt("A", 0, 0), pt("B", 0.001, 0), pt("C", 0, 0), pt("D", 0.001, 0.000004),
               "l = Line(A, B)", "m = Line(C, D)", "t = AreEqual(l, m)"],
     [mv("D", 0.001, 0.000009), mv("D", 0.001, 0.00001), mv("D", 0.001, 0.00002),
      mv("D", 0.001, 0), mv("D", 1, 0.00001), mv("D", 1000, 0.01)])
case("equal", [pt("A", 0, 0), pt("B", 1, 0), pt("C", 0, 0), pt("D", 1, 0), "l = Line(A, B)",
               "m = Line(C, D)", "n = Line(A, D)", "t = AreEqual(l, m)", "u = AreEqual(l, n)",
               "v = AreEqual(n, l)"],
     [mv("B", 0, 0), mv("D", 0, 0), mv("D", 5, 5), mv("C", 5, 5), mv("B", 5, 5)])
# A line is never equal to a segment or a ray.
case("equal", [pt("A", 0, 0), pt("B", 2, 2), "l = Line(A, B)", "s = Segment(A, B)",
               "r = Ray(A, B)", "t = AreEqual(l, s)", "u = AreEqual(s, l)", "v = AreEqual(l, r)",
               "w = AreEqual(r, l)", "xx = AreEqual(s, r)"],
     [mv("B", 3, 1), mv("B", 0, 0)])
# Segments: same end points in either order.
case("equal", [pt("A", 0, 0), pt("B", 3, 1), pt("C", 3, 1), pt("D", 0, 0), pt("E", 0, 0),
               pt("F", 3, 1.000001), "s = Segment(A, B)", "r = Segment(C, D)", "q = Segment(E, B)",
               "p = Segment(E, F)", "t = AreEqual(s, r)", "u = AreEqual(s, q)",
               "v = AreEqual(s, p)", "w = AreEqual(s, s)"],
     [mv("F", 3, 1), mv("D", 0.000001, 0), mv("D", 0.00001, 0), mv("C", 4, 4), mv("B", 3, 2),
      mv("B", 0, 0)])
# Rays: same direction and same start point.
case("equal", [pt("A", 0, 0), pt("B", 2, 1), pt("C", 0, 0), pt("D", 4, 2), pt("E", 0, 0),
               pt("F", -2, -1), pt("G", 1, 0.5), "r = Ray(A, B)", "s = Ray(C, D)",
               "q = Ray(E, F)", "p = Ray(G, D)", "t = AreEqual(r, s)", "u = AreEqual(r, q)",
               "v = AreEqual(r, p)", "w = AreEqual(r, r)"],
     [mv("D", 4, 2.000001), mv("D", 4, 2.00001), mv("F", 2, 1), mv("G", 0, 0), mv("C", 1, 1),
      mv("A", 1, 1)])
# Circles: proportional matrices.
case("equal", [pt("A", 0, 0), pt("B", 3, 4), pt("C", 0, 0), num("r", 5), "c = Circle(A, B)",
               "d = Circle(C, r)", "e = Circle(B, A)", "t = AreEqual(c, d)", "u = AreEqual(c, e)",
               "v = AreEqual(c, c)"],
     [setn("r", 5.000001), setn("r", 5.0000001), setn("r", 4), mv("C", 0.000001, 0),
      mv("B", 4, 3), mv("B", 0, 0), setn("r", 0), setn("r", -1), setn("r", 5)])
case("equal", [pt("A", 1, 1), num("r", 0.00001), num("s", 0.000001), "c = Circle(A, r)",
               "d = Circle(A, s)", "t = AreEqual(c, d)"],
     [setn("s", 0.00001), setn("r", 0), setn("s", 0), setn("r", -1), setn("s", -2), setn("r", 3)])
# Vectors.
case("equal", [pt("A", 0, 0), pt("B", 3, 4), "v = (3, 4)", "w = (3, 4.000001)", "k = Vector(A, B)",
               "t = AreEqual(v, w)", "u = AreEqual(v, k)", "yy = AreEqual(k, k)"],
     [mv("w", 3, 4), mv("v", 0, 0), mv("w", 0, 0), mv("B", 0, 0), mv("v", 3, 4)])
# Booleans.
case("equal", [pt("A", 0, 0), pt("B", 1, 1), pt("C", 2, 2), pt("D", 3, 4), "p = AreCollinear(A, B, C)",
               "q = AreCollinear(A, B, D)", "r = AreCollinear(A, C, B)", "t = AreEqual(p, q)",
               "u = AreEqual(p, r)", "v = AreEqual(q, q)"],
     [mv("D", 3, 3), mv("C", 2, 3), mv("D", 5, 5), mv("B", 0, 0)])
case("equal", [pt("A", 0, 0), pt("B", 1, 1), num("a", 1), "p = AreCollinear(A, B, B)",
               "t = AreEqual(p, a)", "u = AreEqual(a, p)", "v = AreEqual(p, A)"],
     [setn("a", 0), mv("B", 2, 2)])
# Undefined objects are not equal to anything, not even to themselves.
case("equal", [pt("A", 0, 0), pt("B", 1, 0), num("r", 1), pt("E", 1.5, 0), "c = Circle(A, r)",
               "d = Circle(E, r)", "P = Intersect(c, d, 1)", "Q = Intersect(c, d, 1)",
               "t = AreEqual(P, Q)", "u = AreEqual(P, A)", "v = AreEqual(P, P)"],
     [setn("r", 0.5), setn("r", 5), mv("E", 20, 0), mv("E", 1, 0)])

# ------------------------------------------------------------- congruent
topic("congruent")
case("congruent", [pt("A", 0, 0), pt("B", 1, 5), pt("C", 3, 4), "t = AreCongruent(A, B)",
                   "u = AreCongruent(A, A)", "v = AreCongruent(A, C)"],
     [mv("B", 0, 0), mv("A", 9, 9)])
case("congruent", [pt("A", 0, 0), pt("B", 3, 4), pt("C", 1, 1), pt("D", 4, 5), pt("E", 7, 7),
                   pt("F", 7, 9), "s = Segment(A, B)", "r = Segment(C, D)", "q = Segment(E, F)",
                   "t = AreCongruent(s, r)", "u = AreCongruent(s, q)", "v = AreCongruent(s, s)"],
     [mv("D", 4, 5.000001), mv("D", 4, 5.00001), mv("D", 4, 6), mv("F", 7, 12), mv("B", 0, 0),
      mv("D", 1, 1)])
for _ in range(2):
    a, b, c, d = rp(), rp(), rp(), rp()
    case("congruent", [pt("A", *a), pt("B", *b), pt("C", *c), pt("D", *d), "s = Segment(A, B)",
                       "r = Segment(C, D)", "t = AreCongruent(s, r)"],
         [mv("D", *rp()), mv("D", c[0] + (b[0] - a[0]), c[1] + (b[1] - a[1])), mv("B", *a),
          mv("D", *c)])
case("congruent", [pt("A", 0, 0), pt("B", 0.000002, 0), pt("C", 1, 1), pt("D", 1.000006, 1),
                   "s = Segment(A, B)", "r = Segment(C, D)", "t = AreCongruent(s, r)",
                   "u = AreCongruent(r, s)"],
     [mv("D", 1.000002, 1), mv("D", 1, 1), mv("B", 0, 0), mv("D", 1.00001, 1)])
# Lines, segments and rays: a line is congruent to any line-like object.
case("congruent", [pt("A", 0, 0), pt("B", 2, 1), pt("C", 5, 5), pt("D", 6, 9), "l = Line(A, B)",
                   "s = Segment(A, B)", "r = Ray(C, D)", "m = Line(C, D)", "t = AreCongruent(l, m)",
                   "u = AreCongruent(l, s)", "v = AreCongruent(s, l)", "w = AreCongruent(l, r)",
                   "xx = AreCongruent(r, l)", "yy = AreCongruent(r, s)", "zz = AreCongruent(s, r)"],
     [mv("D", 5, 5), mv("B", 0, 0)])
case("congruent", [pt("A", 0, 0), pt("B", 3, 4), pt("C", 0, 0), pt("D", 5, 0), num("r", 5),
                   "c = Circle(A, B)", "d = Circle(C, D)", "e = Circle(C, r)", "t = AreCongruent(c, d)",
                   "u = AreCongruent(c, e)", "v = AreCongruent(d, e)"],
     [setn("r", 5.000001), setn("r", 5.0000001), mv("D", 5.000001, 0), mv("D", 6, 0),
      setn("r", 6), mv("B", 3, 5), mv("D", 0, 0), setn("r", 0), setn("r", -1)])
case("congruent", [pt("A", 0, 0), num("r", 0.000001), num("s", 0.000002), "c = Circle(A, r)",
                   "d = Circle(A, s)", "t = AreCongruent(c, d)"],
     [setn("r", 0.00001), setn("s", 0.00001), setn("s", 0), setn("r", 0), setn("r", 4),
      setn("s", 4)])
# Congruent circles at different places.
case("congruent", [pt("A", 0, 0), pt("B", 5, 0), pt("C", 10, 10), pt("D", 10, 15), "c = Circle(A, B)",
                   "d = Circle(C, D)", "t = AreCongruent(c, d)", "u = AreEqual(c, d)"],
     [mv("D", 10, 15.000001), mv("D", 10, 16), mv("C", 0, 0), mv("D", 5, 0)])
# Vectors, numbers and mixed kinds.
case("congruent", [pt("A", 1, 1), "v = (3, 4)", "w = (3, 4)", "k = (4, 3)", num("a", 2), num("b", 2),
                   num("c", 3), "t = AreCongruent(v, w)", "u = AreCongruent(v, k)",
                   "p = AreCongruent(a, b)", "q = AreCongruent(a, c)", "r = AreCongruent(A, a)",
                   "s = AreCongruent(a, A)", "yy = AreCongruent(v, A)"],
     [mv("w", 3, 5), mv("k", 3, 4), setn("c", 2), setn("b", 2.000001), setn("b", 2.00000001)])
case("congruent", [pt("A", 0, 0), pt("B", 1, 1), pt("C", 2, 2), pt("D", 3, 4),
                   "p = AreCollinear(A, B, C)", "q = AreCollinear(A, B, D)",
                   "t = AreCongruent(p, q)", "u = AreCongruent(p, p)", "s = Segment(A, B)",
                   "v = AreCongruent(s, p)", "w = AreCongruent(p, s)"],
     [mv("D", 3, 3), mv("C", 2, 3), mv("D", 5, 5)])
# Undefined objects.
case("congruent", [pt("A", 0, 0), num("r", 1), pt("E", 1.5, 0), "c = Circle(A, r)", "d = Circle(E, r)",
                   "P = Intersect(c, d, 1)", "Q = Intersect(c, d, 1)", "t = AreCongruent(P, Q)",
                   "u = AreCongruent(P, A)", "v = AreCongruent(A, P)"],
     [setn("r", 0.5), setn("r", 5), mv("E", 20, 0), mv("E", 1, 0)])

# ------------------------------------------------------------- more cases
topic("tangent")
case("tangent", [pt("A", 0, 0.05), pt("B", 1, 0.05), pt("O", 0, 0), num("r", 0.05),
                 "l = Line(A, B)", "c = Circle(O, r)", "t = IsTangent(l, c)"],
     [setn("r", 0.050001), setn("r", 0.050005), setn("r", 0.05001), setn("r", 0.0501),
      setn("r", 0.049999), setn("r", 0.049995), setn("r", 0.04999), setn("r", 0.05)])
case("tangent", [pt("A", 0, 0.5), pt("B", 1, 0.5), pt("O", 0, 0), num("r", 0.5),
                 "l = Line(A, B)", "c = Circle(O, r)", "t = IsTangent(l, c)"],
     [setn("r", 0.500001), setn("r", 0.500005), setn("r", 0.49999), setn("r", 0.4999999),
      setn("r", 0.5)])
case("tangent", [pt("O", 3, 4), pt("A", 8, 4), pt("B", 8, 9), num("r", 5), "c = Circle(O, r)",
                 "l = Line(A, B)", "s = Segment(A, B)", "t = IsTangent(l, c)",
                 "u = IsTangent(s, c)"],
     [setn("r", 5.000002), setn("r", 4.99999), setn("r", 5), mv("B", 8.00001, 9),
      mv("B", 8.0000001, 9), mv("O", 3, 4.00001), mv("O", 3.000001, 4)])

topic("concyclic")
case("concyclic", [pt("O", 0, 0), pt("A", 5, 0), pt("B", -3, -7), pt("C", 4, 6), "c = Circle(O, A)",
                   "l = Line(B, C)", "P = Intersect(l, c, 1)", "Q = Intersect(l, c, 2)",
                   "t = AreConcyclic(A, P, Q, A)", "u = AreConcyclic(P, Q, A, B)"],
     [mv("B", 0, 7), mv("C", 1, 1), mv("B", -2, 2), mv("A", 0, 3), mv("C", 9, 9)])

topic("collinear")
case("collinear", [pt("A", 0, 0), pt("B", 3, 1), pt("C", 6, 2), pt("D", 9, 3), pt("E", 12, 4),
                   "t = AreCollinear(A, B, C)", "u = AreCollinear(C, D, E)",
                   "v = AreCollinear(A, C, E)", "w = AreCollinear(B, D, E)"],
     [mv("E", 12, 4.000001), mv("E", 12, 4.1), mv("C", 6, 1), mv("B", 3, 0), mv("D", 0, 0)])
case("collinear", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 2, 3), "M = Midpoint(B, C)",
                   "N = Midpoint(A, C)", "l = Line(A, M)", "m = Line(B, N)", "G = Intersect(l, m)",
                   "t = AreCollinear(A, G, M)", "u = AreCollinear(B, G, N)",
                   "v = AreCollinear(M, G, N)"],
     [mv("C", 3, 7), mv("B", 9, 1), mv("A", -3, 3), mv("C", 4.5, 0.5)])

topic("equal")
case("equal", [pt("A", 0, 0), pt("B", 4, 2), pt("C", 0, 4), pt("D", 4, 0), "l = Line(A, B)",
               "m = Line(C, D)", "P = Intersect(l, m)", pt("Q", 2, 2), "t = AreEqual(P, Q)",
               "u = AreEqual(Q, P)"],
     [mv("Q", 2.000001, 2), mv("Q", 2.00001, 2), mv("D", 4, 1), mv("D", 5, 0), mv("B", 3, 3)])
case("equal", [pt("A", 0, 0), pt("B", 0, 5), pt("C", 5, 0), "c = Circle(A, B)", "d = Circle(A, C)",
               "s = Segment(A, B)", "r = Radius(c)", "q = Length(s)", "t = AreEqual(c, d)",
               "u = AreEqual(r, q)", "v = AreEqual(r, r)"],
     [mv("C", 5.000001, 0), mv("C", 0, 5), mv("C", 3, 4), mv("B", 3, 4), mv("A", 1, 1)])

topic("congruent")
case("congruent", [pt("A", 0, 0), pt("B", 3, 4), pt("C", 10, 10), num("r", 5), "s = Segment(A, B)",
                   "c = Circle(C, r)", "q = Length(s)", "p = Radius(c)", "t = AreCongruent(s, c)",
                   "u = AreCongruent(c, s)", "v = AreCongruent(q, p)"],
     [setn("r", 6), mv("B", 3, 5), setn("r", 5.000001), mv("B", 0, 0)])

topic("perpendicular")
case("perpendicular", [pt("A", 0, 0), pt("B", 6, 0), pt("C", 2, 4), "s = Segment(A, B)", "t = Segment(A, C)",
                       "m = LineBisector(s)", "n = LineBisector(t)", "u = ArePerpendicular(m, s)",
                       "v = ArePerpendicular(m, n)", "w = ArePerpendicular(s, t)"],
     [mv("C", 0, 4), mv("B", 6, 6), mv("C", 6, 6), mv("C", -2, 3), mv("B", 3, 0)])

# ---------------------------------------- degenerate lines, rays and segments
def vec(label, x, y):
    return pt(label, x, y)


topic("parallel")
# Lines and rays made from a vector that becomes zero keep zero coefficients.
case("parallel", [pt("A", 0, 0), pt("C", 0, 3), pt("D", 2, 4), vec("u", 1, 2), "l = Line(A, u)",
                  "m = Line(C, D)", "r = Ray(A, u)", "s = Segment(C, D)", "t = AreParallel(l, m)",
                  "n = AreParallel(m, l)", "o = AreParallel(r, s)", "p = AreParallel(s, r)"],
     [mv("u", 0, 0), mv("u", 2, 1), mv("u", 0, 0), mv("u", 0.000001, 0), mv("u", 0, 0.000001),
      mv("u", 1, 2)])
case("parallel", [pt("A", 0, 0), pt("B", 4, 4), pt("C", 1, 5), vec("u", 3, 1),
                  "l = Line(A, B)", "m = Line(C, u)", "k = Line(B, u)", "t = AreParallel(l, m)",
                  "n = AreParallel(m, k)", "o = AreParallel(m, m)"],
     [mv("u", 0, 0), mv("B", 0, 0), mv("u", 1, 1), mv("B", 4, 4), mv("u", 0, 0)])
case("parallel", [pt("A", 0, 0), pt("B", 3, 0), pt("C", 1, 1), pt("D", 5, 3), "s = Segment(A, B)",
                  "r = Ray(C, D)", "l = Line(C, D)", "t = AreParallel(s, l)",
                  "n = AreParallel(r, s)", "o = AreParallel(l, r)"],
     [mv("B", 0, 0), mv("D", 1, 1), mv("B", 3, 0), mv("D", 5, 3), mv("D", 1, 1)])

topic("perpendicular")
case("perpendicular", [pt("A", 0, 0), pt("B", 3, 1), pt("C", 1, 4), vec("u", 2, 1),
                       "l = Line(A, B)", "p = PerpendicularLine(C, u)", "q = PerpendicularLine(C, l)",
                       "r = Line(C, u)", "t = ArePerpendicular(p, r)", "n = ArePerpendicular(q, l)",
                       "o = ArePerpendicular(p, q)", "w = AreParallel(p, q)"],
     [mv("u", 0, 0), mv("B", 0, 0), mv("u", 1, 3), mv("B", 3, 1), mv("u", 0, 0), mv("B", 0, 0),
      mv("u", 2, 1)])
# A perpendicular to a tiny base line: GeoGebra scales the coefficients.
case("perpendicular", [pt("A", 0, 0), pt("B", 0.000001, 0), pt("C", 1, 1), "l = Line(A, B)",
                       "p = PerpendicularLine(C, l)", "m = Line(C, A)", "t = ArePerpendicular(p, l)",
                       "n = AreParallel(p, m)", "o = ArePerpendicular(p, m)"],
     [mv("B", 0.000002, 0), mv("B", 0, 0.000001), mv("B", 0.00000001, 0), mv("B", 0, 0),
      mv("B", 1, 0), mv("A", 1, 0)])
case("perpendicular", [pt("A", 0, 0), pt("B", 4, 2), pt("C", 1, 3), pt("D", 5, 1), "l = Line(A, B)",
                       "m = Line(C, D)", "p = PerpendicularLine(C, l)", "q = Line(C, l)",
                       "r = PerpendicularLine(D, q)", "t = ArePerpendicular(p, q)",
                       "n = ArePerpendicular(r, p)", "o = AreParallel(r, p)"],
     [mv("B", 0, 0), mv("B", 4, 2), mv("A", 4, 2), mv("A", 0, 0), mv("B", 0, 0)])

topic("equal")
case("equal", [pt("A", 0, 0), pt("B", 3, 1), pt("C", 2, 5), pt("D", 4, 4), vec("u", 1, 1),
               "l = Line(A, u)", "m = Line(C, D)", "k = Line(B, u)", "t = AreEqual(l, k)",
               "n = AreEqual(l, m)", "o = AreEqual(m, l)"],
     [mv("u", 0, 0), mv("D", 2, 5), mv("D", 4, 4), mv("u", 1, 1), mv("D", 2, 5), mv("u", 0, 0)])
# Rays that become degenerate keep their start point.
case("equal", [pt("A", 0, 0), pt("B", 3, 1), pt("C", 0, 0), pt("D", 2, 2), pt("F", 5, 5),
               "r = Ray(A, B)", "s = Ray(C, D)", "q = Ray(F, A)", "t = AreEqual(r, s)",
               "n = AreEqual(r, q)", "o = AreEqual(s, q)", "p = AreEqual(r, r)"],
     [mv("B", 0, 0), mv("D", 0, 0), mv("D", 2, 2), mv("C", 1, 1), mv("D", 1, 1), mv("B", 1, 1),
      mv("F", 0, 0), mv("A", 0, 0)])
case("equal", [pt("A", 0, 0), pt("B", 3, 1), pt("C", 0, 0), pt("D", 2, 2), vec("u", 1, 0),
               "r = Ray(A, u)", "s = Ray(C, D)", "q = Ray(B, u)",
               "t = AreEqual(r, s)", "n = AreEqual(r, q)", "o = AreEqual(s, q)"],
     [mv("u", 0, 0), mv("D", 0, 0), mv("D", 2, 2), mv("u", 1, 0), mv("A", 1, 1)])
case("equal", [pt("A", 0, 0), pt("B", 3, 1), pt("C", 4, 4), pt("D", 2, 2), "l = Line(A, B)",
               "s = Segment(A, B)", "r = Ray(A, B)", "g = Segment(C, D)", "h = Ray(C, D)",
               "t = AreEqual(l, s)", "n = AreEqual(l, r)", "o = AreEqual(s, r)",
               "p = AreEqual(g, h)"],
     [mv("B", 0, 0), mv("D", 4, 4), mv("B", 3, 1), mv("D", 2, 2), mv("C", 0, 0), mv("D", 0, 0)])
# Translations of a line that degenerates.
case("equal", [pt("A", 0, 0), pt("B", 3, 1), pt("C", 5, 5), pt("D", 6, 6), vec("u", 2, 2),
               "l = Line(A, B)", "g = Translate(l, u)", "m = Line(C, D)",
               "t = AreEqual(g, m)", "n = AreParallel(g, l)"],
     [mv("B", 0, 0), mv("D", 5, 5), mv("u", 0, 0), mv("D", 6, 6), mv("B", 3, 1)])

topic("congruent")
case("congruent", [pt("A", 0, 0), pt("B", 3, 1), pt("C", 4, 4), pt("D", 2, 2), "s = Segment(A, B)",
                   "g = Segment(C, D)", "r = Ray(A, B)", "h = Ray(C, D)", "l = Line(A, B)",
                   "t = AreCongruent(s, g)", "n = AreCongruent(r, h)", "o = AreCongruent(l, r)",
                   "p = AreCongruent(r, s)"],
     [mv("B", 0, 0), mv("D", 4, 4), mv("B", 3, 1), mv("D", 2, 2), mv("D", 4.000001, 4),
      mv("D", 4, 4)])

topic("concurrent")
case("concurrent", [pt("A", 0, 0), pt("B", 4, 4), pt("C", 0, 4), pt("D", 4, 0), pt("E", 1, 3),
                    pt("F", 3, 1), "s = Segment(A, B)", "r = Ray(C, D)", "l = Line(E, F)",
                    "t = AreConcurrent(s, r, l)", "n = AreConcurrent(l, s, s)"],
     [mv("B", 0, 0), mv("D", 0, 4), mv("B", 4, 4), mv("D", 4, 0), mv("F", 1, 3), mv("E", 3, 1)])
case("concurrent", [pt("A", 0, 0), pt("B", 4, 4), pt("C", 0, 4), pt("D", 4, 0), pt("E", 1, 3),
                    vec("u", 1, 0), "l = Line(A, B)", "m = Line(C, D)", "k = Line(E, u)",
                    "t = AreConcurrent(l, m, k)", "n = AreConcurrent(k, k, l)"],
     [mv("u", 0, 0), mv("B", 0, 0), mv("u", 1, 1), mv("E", 2, 2), mv("u", 0, 0)])

topic("tangent")
case("tangent", [pt("O", 0, 0), pt("A", 2, 0), pt("B", 2, 5), pt("C", 3, 3), "k = Circle(O, A)",
                 "s = Segment(A, B)", "r = Ray(A, B)",
                 "t = IsTangent(s, k)", "n = IsTangent(r, k)"],
     [mv("B", 2, 0), mv("B", 2, -5), mv("B", 0, 0), mv("B", 2, 5), mv("A", 0, 0)])
case("tangent", [pt("O", 0, 0), pt("A", 2, 0), pt("B", 2, 5), vec("u", 0, 1), "k = Circle(O, A)",
                 "l = Line(A, u)", "r = Ray(B, u)", "t = IsTangent(l, k)", "n = IsTangent(r, k)"],
     [mv("u", 0, 0), mv("u", 1, 1), mv("u", 0, 1), mv("u", 0, 0), mv("A", 3, 0)])

# --------------------------------------------- points at infinity as inputs
topic("infinity")
case("infinity", [pt("A", 0, 0), pt("B", 2, 1), pt("C", 0, 3), pt("D", 2, 4), pt("F", 5, 5),
                  "l = Line(A, B)", "m = Line(C, D)", "S = Intersect(l, m)",
                  "t = AreCollinear(A, B, S)", "n = AreCollinear(S, S, F)",
                  "o = AreEqual(S, S)", "p = AreConcyclic(A, B, C, S)",
                  "q = AreEqual(S, F)", "w = AreCongruent(S, F)"],
     [mv("D", 2, 5), mv("D", 2, 4), mv("F", 0, 6)])
# Computed points whose homogeneous weight is not 1.
topic("homogeneous")
case("homogeneous", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 4, 4), pt("D", 0, 4), pt("E", 2, 6),
                     pt("F", 6, 2), "l = Line(A, C)", "m = Line(B, D)", "n = Line(E, F)",
                     "g = Line(A, B)", "S = Intersect(l, m)", "T = Intersect(m, n)",
                     "U = Intersect(l, n)", "t = AreConcyclic(S, T, U, A)",
                     "o = AreConcyclic(S, A, B, C)", "p = AreCollinear(S, T, U)",
                     "q = AreCollinear(S, A, C)"],
     [mv("E", 3, 7), mv("F", 5, 1), mv("D", 1, 5), mv("C", 6, 1)])

topic("keptcircle")
case("keptcircle", [pt("O", 0, 0), num("r", 2), "k = Circle(O, r)", "j = Circle(O, 2)",
                    "t = AreEqual(k, j)", "u = k == j"],
     [setn("r", -2), setn("r", 3)])
case("keptcircle", [pt("O", 0, 0), num("r", 2), "k = Circle(O, r)", "j = Circle(O, 3)",
                    "ar = Area(k)", "t = AreEqual(k, j)"],
     [setn("r", 3), setn("r", -1), {"action": "delete", "label": "ar"}])

case("keptcircle", [pt("O", 0, 0), num("r", 2), "k = Circle(O, r)", "j = Circle(O, 2)",
                    num("m", 1), "t = AreEqual(k, j)"],
     [{"action": "redefine", "line": "r = -m"}])

cases.write()
