#!/usr/bin/env python3
"""Writes cases/measure.json: the case inputs for the measuring commands.

Length, Radius, Circumference, Perimeter, Area, Slope, Distance (point and
line, point and segment, two lines), AffineRatio, CrossRatio and Angle. Only
Line, Segment, Circle(A, B) and Circle(A, r) are used as parents.

The inputs only describe what to run. Snapshots come exclusively from running
GeoGebra through the harness (`harness.sh golden measure`).
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("measure")
case = cases.case
topic = cases.topic
rp = cases.rp


# ----------------------------------------------------------------- length
topic("length")
for _ in range(4):
    a, b = rp(), rp()
    case("length", [pt("A", *a), pt("B", *b), "s = Segment(A, B)", "d = Length(s)"],
         [mv("A", *rp()), mv("B", *rp()), mv("B", *a), mv("B", *rp())])
case("length", [pt("A", 0, 0), pt("B", 3, 4), "s = Segment(A, B)", "d = Length(s)"],
     [mv("B", 0, 0), mv("B", 3, 4), mv("A", 0.000001, 0), mv("A", 1e6, -2e6)])
case("length", [pt("A", 0.001, 0.002), pt("B", 0.004, -0.001), "s = Segment(A, B)",
                "d = Length(s)", "t = Length(s)"],
     [mv("B", 0.001, 0.002), mv("B", 100000, 50000)])

# ----------------------------------------------------------------- radius
topic("radius")
for _ in range(3):
    a, b = rp(), rp()
    case("radius", [pt("A", *a), pt("B", *b), "k = Circle(A, B)", "r = Radius(k)"],
         [mv("B", *rp()), mv("B", *a), mv("B", *rp()), mv("A", *rp())])
case("radius", [pt("A", 1, 2), num("q", 2.5), "k = Circle(A, q)", "r = Radius(k)"],
     [setn("q", 0), setn("q", 7), setn("q", 0), setn("q", 0.5), mv("A", -3, 4)])
case("radius", [pt("A", 1, 2), pt("B", 4, 6), "k = Circle(A, B)", "r = Radius(k)",
                "c = Circle(B, A)", "u = Radius(c)"],
     [mv("B", 1, 2), mv("A", 1, 2), mv("A", 5, 5), mv("B", 9, 9)])
case("radius", [pt("A", 0, 0), num("q", 1e-5), "k = Circle(A, q)", "r = Radius(k)"],
     [setn("q", 1e-8), setn("q", 123456.5), setn("q", 3)])

case("radius", [pt("A", 0, 0), pt("B", 1, 0), "k = Circle(A, B)", "r = Radius(k)"],
     [mv("B", 0.000001, 0), mv("B", 0.00001, 0), mv("B", 0.0001, 0), mv("B", 0, 0),
      mv("B", 1, 0)])
case("radius", [pt("A", 0, 0), num("q", 1), "k = Circle(A, q)", "r = Radius(k)"],
     [setn("q", 1e-7), setn("q", 2e-8), setn("q", 1e-8), setn("q", 0), setn("q", 1)])

# ---------------------------------------------------------- circumference
topic("circumference")
for _ in range(2):
    a, b = rp(), rp()
    case("circumference", [pt("A", *a), pt("B", *b), "k = Circle(A, B)",
                           "p = Circumference(k)", "q = Perimeter(k)"],
         [mv("B", *rp()), mv("B", *a), mv("B", *rp())])
case("circumference", [pt("A", 0, 0), num("r", 2), "k = Circle(A, r)",
                       "p = Circumference(k)", "q = Perimeter(k)"],
     [setn("r", 0), setn("r", 4.25), setn("r", 0), setn("r", 1)])
case("circumference", [pt("A", 2, -1), pt("B", 5, 3), "k = Circle(A, B)",
                       "p = Perimeter(k)"],
     [mv("A", 5, 3), mv("A", -2, 8), mv("B", -2, 8), mv("B", 7, 7)])

case("circumference", [pt("A", 0, 0), num("q", 1), "k = Circle(A, q)", "p = Circumference(k)"],
     [setn("q", 1e-7), setn("q", 2e-8), setn("q", 1e-8), setn("q", 0), setn("q", 1)])

# ------------------------------------------------------------------- area
topic("area")
for _ in range(2):
    a, b = rp(), rp()
    case("area", [pt("A", *a), pt("B", *b), "k = Circle(A, B)", "ar = Area(k)"],
         [mv("B", *rp()), mv("B", *a), mv("B", *rp())])
case("area", [pt("A", 0, 0), num("r", 3), "k = Circle(A, r)", "ar = Area(k)"],
     [setn("r", 0), setn("r", 0.1), setn("r", 100), setn("r", 0), setn("r", 3)])
case("area", [pt("A", 0, 0), pt("B", 1, 1), "k = Circle(A, B)", "ar = Area(k)",
              "c = Circle(B, A)", "br = Area(c)"],
     [mv("A", 1, 1), mv("A", 4, 5), mv("B", 4, 5)])

case("area", [pt("A", 0, 0), num("q", 1), "k = Circle(A, q)", "ar = Area(k)"],
     [setn("q", 1e-7), setn("q", 2e-8), setn("q", 1e-8), setn("q", 0), setn("q", 1)])

# ------------------------------------------------------------ area points
topic("areapts")
for _ in range(3):
    a, b, c = rp(), rp(), rp()
    case("areapts", [pt("A", *a), pt("B", *b), pt("C", *c), "ar = Area(A, B, C)"],
         [mv("C", *rp()), mv("B", *rp()), mv("A", *rp())])
case("areapts", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 3), "ar = Area(A, B, C)"],
     [mv("C", 8, 0), mv("C", 2, 0), mv("C", 4, 0), mv("B", 0, 0), mv("B", 4, 0),
      mv("C", 0, 3), mv("C", 0, -3)])
case("areapts", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 3), "ar = Area(A, B, C)"],
     [mv("B", 0, 0), mv("C", 0, 0), mv("B", 2, 2), mv("C", 3, 3), mv("C", 1, 5)])
case("areapts", [pt("A", 1, 1), pt("B", 5, 1), pt("C", 5, 4), "ar = Area(A, B, C)",
                 "br = Area(C, B, A)"],
     [mv("C", 1, 4), mv("C", 3, -2)])
for _ in range(3):
    a, b, c, d = rp(), rp(), rp(), rp()
    case("areapts", [pt("A", *a), pt("B", *b), pt("C", *c), pt("D", *d),
                     "ar = Area(A, B, C, D)"],
         [mv("D", *rp()), mv("C", *rp()), mv("B", *rp())])
case("areapts", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 4, 4), pt("D", 0, 4),
                 "ar = Area(A, B, C, D)"],
     [mv("C", 0, 4), mv("D", 4, 4), mv("D", 0, 4), mv("C", 4, 4), mv("A", 0, 4),
      mv("B", 0, 4), mv("C", 0, 4), mv("D", 0, 4)])
case("areapts", [pt("A", 0, 0), pt("B", 2, 0), pt("C", 4, 0), pt("D", 6, 0),
                 "ar = Area(A, B, C, D)"],
     [mv("D", 6, 3), mv("D", 6, 0), mv("B", 2, 2), mv("B", 2, 0)])
case("areapts", [pt("A", 0, 0), pt("B", 4, 4), pt("C", 4, 0), pt("D", 0, 4),
                 "ar = Area(A, B, C, D)"],
     [mv("D", 0, -4), mv("D", 0, 4)])

# ------------------------------------------------------------------ slope
topic("slope")
for _ in range(3):
    a, b = rp(), rp()
    case("slope", [pt("A", *a), pt("B", *b), "l = Line(A, B)", "m = Slope(l)"],
         [mv("B", *rp()), mv("A", *rp()), mv("B", *rp())])
case("slope", [pt("A", 1, 1), pt("B", 4, 3), "l = Line(A, B)", "m = Slope(l)"],
     [mv("B", 1, 5), mv("B", 4, 1), mv("B", 1, 1), mv("B", 4, 3), mv("B", 1, -5),
      mv("B", -4, 1), mv("B", -4, 3)])
case("slope", [pt("A", 1, 1), pt("B", 4, 3), "s = Segment(A, B)", "m = Slope(s)"],
     [mv("B", 1, 5), mv("B", 4, 1), mv("B", 1, 1), mv("B", 4, 3), mv("B", -2, 0)])
case("slope", [pt("A", 0, 0), pt("B", 2, 3), pt("P", 5, 5), "l = Line(A, B)",
               "n = PerpendicularLine(P, l)", "p = Line(P, l)", "m = Slope(n)",
               "q = Slope(p)", "t = Slope(l)"],
     [mv("B", 2, 0), mv("B", 0, 2), mv("B", 2, 3), mv("P", 6, -1)])
case("slope", [pt("A", 0, 0), pt("B", 1, 1000000), "l = Line(A, B)", "m = Slope(l)"],
     [mv("B", 0, 1000000), mv("B", 1000000, 1), mv("B", 1, 1000000)])

# --------------------------------------------- distance, point and line
topic("distpl")
for _ in range(3):
    a, b, c = rp(), rp(), rp()
    case("distpl", [pt("A", *a), pt("B", *b), pt("P", *c), "l = Line(A, B)",
                    "d = Distance(P, l)", "g = Distance(l, P)"],
         [mv("P", *rp()), mv("B", *rp()), mv("A", *rp())])
case("distpl", [pt("A", 0, 0), pt("B", 4, 0), pt("P", 1, 3), "l = Line(A, B)",
                "d = Distance(P, l)"],
     [mv("P", 1, 0), mv("P", 9, -2), mv("B", 0, 0), mv("B", 4, 0), mv("A", 2, 0),
      mv("B", 4, 1), mv("P", 2, 0.5)])
case("distpl", [pt("A", 0, 0), pt("B", 3, 3), pt("P", 0, 4), "l = Line(A, B)",
                "d = Distance(P, l)"],
     [mv("P", 2, 2), mv("P", 100, 100), mv("P", 100, 100.000001), mv("A", 100, 100),
      mv("A", 0, 0)])
case("distpl", [pt("A", 0, 0), pt("B", 5, 0), pt("P", 2, 2), "l = Line(A, B)",
                "n = PerpendicularLine(P, l)", "d = Distance(P, n)", "g = Distance(P, l)"],
     [mv("P", 2, -3), mv("B", 5, 5), mv("B", 0, 5)])
case("distpl", [pt("A", 1, 2), pt("B", 4, 6), pt("P", 7, 1), "l = Line(A, B)",
                "q = Line(P, l)", "d = Distance(P, q)", "g = Distance(A, q)"],
     [mv("B", 1, 6), mv("P", 1, 2), mv("A", 7, 1), mv("A", 0, 0)])

# ------------------------------------------- distance, point and segment
topic("distps")
for _ in range(3):
    a, b, c = rp(), rp(), rp()
    case("distps", [pt("A", *a), pt("B", *b), pt("P", *c), "s = Segment(A, B)",
                    "d = Distance(P, s)", "g = Distance(s, P)"],
         [mv("P", *rp()), mv("B", *rp()), mv("A", *rp()), mv("P", *rp())])
case("distps", [pt("A", 0, 0), pt("B", 4, 0), pt("P", 2, 3), "s = Segment(A, B)",
                "d = Distance(P, s)"],
     [mv("P", -3, 4), mv("P", 7, 4), mv("P", 4, 2), mv("P", 2, 0), mv("P", 5, 0),
      mv("P", -1, 0), mv("P", 2, -3)])
case("distps", [pt("A", 1, 1), pt("B", 4, 5), pt("P", 7, 1), "s = Segment(A, B)",
                "d = Distance(P, s)"],
     [mv("B", 1, 1), mv("P", 1, 1), mv("P", 3, 3), mv("B", 3, 3), mv("B", 4, 5),
      mv("P", 7, 1)])
case("distps", [pt("A", 0, 0), pt("B", 4, 0), pt("P", 2, 3), "s = Segment(A, B)",
                "l = Line(A, B)", "d = Distance(P, s)", "g = Distance(P, l)"],
     [mv("P", 9, 3), mv("P", -9, 3), mv("P", 2, 3)])

# ------------------------------------------------ distance, two lines
topic("distll")
case("distll", [pt("A", 0, 0), pt("B", 4, 0), pt("P", 1, 3), "l = Line(A, B)",
                "m = Line(P, l)", "d = Distance(l, m)"],
     [mv("P", 1, 5), mv("P", 1, 0), mv("P", 7, -2), mv("B", 4, 1), mv("B", 4, 0)])
case("distll", [pt("A", 0, 0), pt("B", 4, 1), pt("C", 0, 3), pt("D", 5, 2),
                "l = Line(A, B)", "m = Line(C, D)", "d = Distance(l, m)"],
     [mv("D", 4, 4), mv("D", 4, 1), mv("D", 8, 2), mv("C", 0, 0), mv("C", 0, 3),
      mv("D", 5, 2)])
case("distll", [pt("A", 1, 1), pt("B", 4, 5), pt("P", 6, 0), "l = Line(A, B)",
                "m = Line(P, l)", "d = Distance(m, l)"],
     [mv("B", 1, 5), mv("B", 4, 1), mv("B", 4, 5), mv("P", 2, 2), mv("P", 6, 0)])
case("distll", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 1, 2), pt("D", 6, 2),
                "s = Segment(A, B)", "t = Segment(C, D)", "d = Distance(s, t)",
                "l = Line(C, D)", "g = Distance(s, l)"],
     [mv("D", 6, 5), mv("D", 6, 2), mv("D", -4, 2), mv("C", 1, 0), mv("C", 1, 2)])
case("distll", [pt("A", 0, 0), pt("B", 2, 3), pt("P", 5, 1), "l = Line(A, B)",
                "n = PerpendicularLine(P, l)", "p = Line(P, l)", "d = Distance(l, p)",
                "g = Distance(l, n)", "h = Distance(n, p)"],
     [mv("B", 3, 3), mv("B", 2, 0), mv("P", 0, 0), mv("P", 8, 8)])
case("distll", [pt("A", 0, 0), pt("B", 1, 0), pt("C", 0, 0.000001), pt("D", 1, 0.000001),
                "l = Line(A, B)", "m = Line(C, D)", "d = Distance(l, m)"],
     [mv("D", 1, 0.000002), mv("D", 1, 0.00001), mv("D", 1, 0), mv("D", 1, 0.000001)])

# ----------------------------------------------------------- affine ratio
topic("affine")
case("affine", [pt("A", 0, 0), pt("B", 2, 0), pt("C", 6, 0), "r = AffineRatio(A, B, C)"],
     [mv("C", 8, 0), mv("C", 2, 0), mv("C", 0, 0), mv("C", 6, 1), mv("C", 6, 0),
      mv("B", 0, 0), mv("B", 3, 0)])
case("affine", [pt("A", 1, 1), pt("B", 3, 3), pt("C", 7, 7), "r = AffineRatio(A, B, C)"],
     [mv("C", -5, -5), mv("A", 3, 3), mv("A", 7, 7), mv("C", 3, 3), mv("A", 1, 1),
      mv("C", 7, 7.000001)])
case("affine", [pt("A", 0, 0), pt("B", 0, 4), pt("C", 0, -2), "r = AffineRatio(A, B, C)"],
     [mv("B", 0, 1), mv("C", 0, 1), mv("A", 0, 1), mv("A", 5, 1), mv("A", 0, 0)])
for _ in range(2):
    a = rp()
    case("affine", [pt("A", *a), pt("B", a[0] + 2, a[1] + 1), pt("C", a[0] + 5, a[1] + 2.5),
                    "r = AffineRatio(A, B, C)"],
         [mv("C", a[0] + 8, a[1] + 4), mv("B", a[0] + 4, a[1] + 2), mv("B", *rp()),
          mv("C", *rp())])
case("affine", [pt("A", 0, 0), pt("B", 5, 0), pt("C", 9, 0), "s = Segment(A, C)",
                "r = AffineRatio(A, B, C)", "q = AffineRatio(C, B, A)"],
     [mv("B", 10, 0), mv("B", 4.5, 0.5), mv("B", 4.5, 0)])

# ------------------------------------------------------------ cross ratio
topic("cross")
case("cross", [pt("A", 0, 0), pt("B", 1, 0), pt("C", 3, 0), pt("D", 7, 0),
               "r = CrossRatio(A, B, C, D)"],
     [mv("D", 9, 0), mv("B", 2, 0), mv("B", 3, 0), mv("B", 1, 0), mv("A", 7, 0),
      mv("A", 0, 0), mv("D", 3, 0), mv("D", 7, 0), mv("D", 7, 1), mv("D", 7, 0)])
case("cross", [pt("A", 1, 1), pt("B", 2, 2), pt("C", 4, 4), pt("D", 8, 8),
               "r = CrossRatio(A, B, C, D)"],
     [mv("A", 8, 8), mv("A", 3, 3), mv("B", 4, 4), mv("B", 2, 2), mv("C", 8, 8),
      mv("C", 4, 4)])
case("cross", [pt("A", 0, 1), pt("B", 2, 5), pt("C", 4, 9), pt("D", -2, -3),
               "r = CrossRatio(A, B, C, D)", "q = CrossRatio(B, A, C, D)"],
     [mv("D", 6, 13), mv("C", 6, 13), mv("C", 6, 14), mv("C", 4, 9), mv("B", 0, 1)])
for _ in range(2):
    a = rp()
    case("cross", [pt("A", *a), pt("B", a[0] + 1, a[1] + 2), pt("C", a[0] + 3, a[1] + 6),
                   pt("D", a[0] + 4, a[1] + 8), "r = CrossRatio(A, B, C, D)"],
         [mv("D", a[0] + 6, a[1] + 12), mv("B", a[0] + 2, a[1] + 4), mv("B", *rp()),
          mv("B", a[0] + 2, a[1] + 4)])

# ------------------------------------------------------------------ angle
topic("angle")
case("angle", [pt("A", 4, 0), pt("B", 0, 0), pt("C", 0, 3), "w = Angle(A, B, C)"],
     [mv("C", 4, 0), mv("C", -4, 0), mv("C", 0, -3), mv("C", 3, -3), mv("C", 0, 0),
      mv("C", 0, 3), mv("A", 0, 0), mv("A", 4, 0)])
for _ in range(3):
    a, b, c = rp(), rp(), rp()
    case("angle", [pt("A", *a), pt("B", *b), pt("C", *c), "w = Angle(A, B, C)"],
         [mv("A", *rp()), mv("C", *rp()), mv("B", *rp()), mv("A", *c), mv("C", *a)])
case("angle", [pt("A", 1, 1), pt("B", 2, 2), pt("C", 3, 3), "w = Angle(A, B, C)",
               "u = Angle(C, B, A)"],
     [mv("C", 0, 0), mv("C", 3, 1), mv("A", 2, 3), mv("B", 1, 1), mv("B", 2, 2)])

# ------------------------------------------------ tolerance windows
# Cases that land between a zero test on GeoGebra's raw line coefficients
# (as long as the line's defining points are apart) and the same test on unit
# normals. `mv` rounds to 6 decimals, so the windows are reached with long
# lines.
topic("distll")
case("distll", [pt("A", 0, 0), pt("B", 10000, 0), pt("C", 0, 1), pt("D", 10000, 1.000001),
                "l = Line(A, B)", "m = Line(C, D)", "d = Distance(l, m)"],
     [mv("D", 10000, 1), mv("D", 10000, 1.000001), mv("D", 10000, 1.00001),
      mv("C", 0, 5), mv("C", 0, 1)])
case("distll", [pt("A", 0, 0), pt("B", 10000, 0), pt("C", 0, 1), pt("D", 10000, 1.000001),
                "l = Line(A, B)", "s = Segment(C, D)", "d = Distance(l, s)",
                "e = Distance(s, l)"],
     [mv("D", 10000, 1), mv("D", 10000, 1.000001), mv("B", 10000, 0.000001),
      mv("B", 10000, 0)])
case("distll", [pt("A", 0, 0), pt("B", 10000, 0), pt("C", 0, 1), pt("D", 10000, 1.000001),
                "s = Segment(A, B)", "t = Segment(C, D)", "d = Distance(s, t)"],
     [mv("D", 10000, 1), mv("D", 10000, 1.000001), mv("B", 10000, 0.000001),
      mv("B", 10000, 0)])
case("distll", [pt("A", 0, 0), pt("B", 10000, 0), pt("P", 0, 1), "l = Line(A, B)",
                "m = Line(P, l)", "n = PerpendicularLine(P, l)", "d = Distance(l, m)",
                "e = Distance(l, n)", "f = Distance(m, n)"],
     [mv("B", 10000, 0.000001), mv("B", 10000, 0.001), mv("P", 5, 7), mv("B", 10000, 0)])
case("distll", [pt("A", 0, 0), pt("B", 100, 0), pt("C", 0, 1), pt("D", 100, 1.000001),
                "l = Line(A, B)", "m = Line(C, D)", "d = Distance(l, m)"],
     [mv("D", 100, 1.00001), mv("D", 100, 1.000001), mv("D", 100, 1)])
# one line becomes undefined and comes back
case("distll", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 2), pt("D", 4, 2),
                "l = Line(A, B)", "m = Line(C, D)", "d = Distance(l, m)",
                "g = Distance(m, l)"],
     [mv("B", 0, 0), mv("B", 4, 0), mv("D", 0, 2), mv("D", 4, 2), mv("A", 4, 0),
      mv("A", 0, 0)])
case("distll", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 2), pt("D", 4, 2),
                "s = Segment(A, B)", "m = Line(C, D)", "d = Distance(s, m)",
                "g = Distance(m, s)"],
     [mv("B", 0, 0), mv("B", 4, 0), mv("D", 0, 2), mv("D", 4, 2)])

topic("slope")
case("slope", [pt("A", 0, 0), pt("B", 0.001, 1000000), "l = Line(A, B)", "m = Slope(l)"],
     [mv("B", 0.01, 1000000), mv("B", 0.00001, 1000000), mv("B", 0.000001, 1000000),
      mv("B", 0, 1000000), mv("B", 0.001, 1000000)])
case("slope", [pt("A", 0, 0), pt("B", 0.001, 1000000), "s = Segment(A, B)", "m = Slope(s)"],
     [mv("B", 0.00001, 1000000), mv("B", 0.000001, 1000000), mv("B", 0, 1000000),
      mv("B", 0.001, 1000000)])
case("slope", [pt("A", 0, 0), pt("B", 1000000, 0.001), pt("P", 3, 4), "l = Line(A, B)",
               "n = PerpendicularLine(P, l)", "p = Line(P, l)", "m = Slope(n)",
               "q = Slope(p)"],
     [mv("B", 1000000, 0.000001), mv("B", 1000000, 0), mv("B", 1000000, 0.001)])

topic("length")
# Segments of length about 1.6e-8 and 7.8e-9 from chained midpoints (6-decimal
# moves cannot make a shorter segment directly).
midpoints = ["F = Midpoint(A, E)", "G = Midpoint(A, F)", "H = Midpoint(A, G)",
             "J = Midpoint(A, H)", "K = Midpoint(A, J)", "L = Midpoint(A, K)",
             "N = Midpoint(A, L)"]
case("length", [pt("A", 0, 0), pt("E", 0.000001, 0)] + midpoints +
     ["s = Segment(A, L)", "t = Segment(A, N)", "d = Length(s)", "g = Length(t)"],
     [mv("E", 0.000002, 0), mv("E", 0.000001, 0), mv("A", 5, 5), mv("E", 5.000001, 5)])

topic("angle")
case("angle", [pt("B", 0, 0), pt("E", 0.000001, 0)] + [
    m.replace("A", "B") for m in midpoints] +
     [pt("C", 0, 3), "w = Angle(L, B, C)", "u = Angle(N, B, C)", "v = Angle(C, B, N)"],
     [mv("E", 0.000002, 0), mv("E", 0.000001, 0), mv("C", -3, 0)])

topic("affine")
case("affine", [pt("A", 0, 0), pt("B", 0.001, 0), pt("C", 0.002, 0), "r = AffineRatio(A, B, C)"],
     [mv("C", 0.002, 0.000001), mv("C", 0.002, 0.00001), mv("C", 0.002, 0.00002),
      mv("C", 0.002, 0), mv("B", 0.001, 0.000001)])
case("affine", [pt("A", 0, 0), pt("B", 1, 0), pt("C", 2, 0), "r = AffineRatio(A, B, C)"],
     [mv("C", 2, 0.000001), mv("C", 2, 0.00001), mv("C", 2, 0)])

topic("cross")
case("cross", [pt("A", 0, 0), pt("B", 0.001, 0), pt("C", 0.003, 0), pt("D", 0.007, 0),
               "r = CrossRatio(A, B, C, D)"],
     [mv("D", 0.007, 0.000001), mv("D", 0.007, 0.00002), mv("B", 0.001, 0.000001),
      mv("B", 0.001, 0), mv("D", 0.007, 0)])

# ------------------------------------------------ raw scale of the lines
# GeoGebra keeps the raw coefficients each algorithm computes and tests them
# against 1e-8; these cases probe that scale.
topic("distll")
# The reverse window: short lines whose raw cross product is below 1e-8
# although their directions differ clearly, so GeoGebra calls them parallel.
for a, b in (("Line", "Line"), ("Segment", "Segment")):
    case("distll", [pt("A", 0, 0), pt("B", 0.0001, 0), pt("C", 0, 1), pt("D", 0.0001, 1.0003),
                    f"l = {a}(A, B)", f"m = {b}(C, D)", "d = Distance(l, m)",
                    "g = Distance(m, l)"],
         [mv("D", 0.0001, 1.00005), mv("D", 0.0001, 1.00001), mv("D", 0.0002, 1.00005),
          mv("B", 0.001, 0), mv("B", 0.0001, 0), mv("C", 0.00003, 1), mv("D", 0.0001, 0.99995),
          mv("D", 0.0001, 1.0003)])
# Chains of perpendiculars and parallels on a tiny and on a very long base
# line: GeoGebra rescales each perpendicular by powers of two into [1e-2, 1e7].
CHAIN = ["l = Line(A, B)", pt("P", 3, 1), pt("Q", -2, 4), pt("R", 1, -3),
         "f = PerpendicularLine(P, l)", "g = PerpendicularLine(Q, f)",
         "h = PerpendicularLine(R, g)", "i = PerpendicularLine(P, h)", "p = Line(Q, i)",
         "q = Line(R, p)", "j = PerpendicularLine(Q, q)", "da = Distance(f, h)",
         "db = Distance(l, g)", "dc = Distance(g, q)", "dd = Distance(j, l)",
         "sa = Slope(h)", "sb = Slope(q)"]
case("distll", [pt("A", 0, 0), pt("B", 0.000002, 0.000001)] + CHAIN,
     [mv("B", 0.000001, 0), mv("B", 0, 0.000001), mv("B", 0.000001, 0.000003), mv("P", -5, 2),
      mv("A", 0.000001, 0.000001), mv("A", 0, 0)])
case("distll", [pt("A", -20000000, 10000000), pt("B", 20000000, -15000000)] + CHAIN,
     [mv("B", 20000000, 10000000.5), mv("B", 20000000, 10000001), mv("P", 70000000, 1),
      mv("B", -20000000, -15000000), mv("A", 30000000, 0)])
# Parallel and perpendicular through an intersection point. GeoGebra keeps
# the point in homogeneous coordinates (x, y, z) with z the raw cross product
# of the two lines, and both lines carry that z in their raw coefficients.
case("distll", [pt("A", 0, 0), pt("B", 4, 1), pt("C", 1, 3), pt("D", 2, -1), "l = Line(A, B)",
                "m = Line(C, D)", "S = Intersect(l, m)", pt("E", -2, 2), pt("F", 3, 4),
                "g = Line(E, F)", "p = Line(S, g)", "n = PerpendicularLine(S, g)",
                "da = Distance(p, g)", "db = Distance(n, l)", "sa = Slope(n)"],
     [mv("D", 5, 0), mv("B", 40, 10), mv("E", 0, 0), mv("D", 2, -1)])
# Intersection of long, nearly parallel lines: GeoGebra takes the cross
# product of the raw coefficients, which stays well above zero although the
# angle between the lines is far below 1e-12.
case("distll", [pt("A", 0, 0), pt("B", 10000000, 1), pt("C", -10000000, -1.000001),
                pt("D", 10000000, 1.000001), "l = Line(A, B)", "m = Line(C, D)",
                "S = Intersect(l, m)", "k = Distance(A, S)"],
     [mv("D", 10000000, 1.000002), mv("C", -10000000, -1), mv("D", 10000000, 1),
      mv("D", 10000000, 1.000001), mv("A", 0, 0.5), mv("B", 10, 0.000001)])
# Nearly parallel lines through points 0.001 apart: the intersection is at
# infinity when its z is negligible against 1e-8 and against its x and y
# (DoubleUtil.isEpsilon in GeoPoint.updateCoords), and finite otherwise.
for x0, y0 in ((0.001, 0.002), (10000, 20000), (1000000000, -3)):
    case("distll", [pt("A", x0, y0), pt("B", x0 + 0.001, y0 + 0.001), pt("C", x0, y0 + 0.001),
                    pt("D", x0 + 0.001, y0 + 0.002), "l = Line(A, B)", "m = Line(C, D)",
                    "S = Intersect(l, m)", "k = Distance(A, S)"],
         [mv("D", x0 + 0.001, y0 + 0.002001), mv("D", x0 + 0.001, y0 + 0.002002),
          mv("D", x0 + 0.001, y0 + 0.00201), mv("D", x0 + 0.001, y0 + 0.002),
          mv("D", x0 + 0.001, y0 + 0.0021)])

topic("keptcircle")
case("keptcircle", [pt("O", 0, 0), num("r", 2), "k = Circle(O, r)", "rr = Radius(k)",
                    "cc = Circumference(k)", "Q = Point(k)"],
     [setn("r", -2), mv("Q", 0, 10), setn("r", 3)])

cases.write()
