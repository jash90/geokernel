#!/usr/bin/env python3
"""Writes cases/conic.json: the case inputs (id, group, script, steps).

Circles and their parts: Circle through three points, Circle with a segment
as radius, Incircle, Center and Midpoint of a circle, Midpoint of a segment,
Diameter of a circle.

The inputs only describe what to run. Snapshots come exclusively from running
GeoGebra through the harness (`harness.sh golden conic`).
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("conic")
case = cases.case
topic = cases.topic
r2 = cases.r2
rp = cases.rp


def area2(a, b, c):
    return abs((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))


def tri():
    """Three random points that are clearly not collinear."""
    while True:
        a, b, c = rp(), rp(), rp()
        if area2(a, b, c) > 8:
            return a, b, c


def on_line(a, b, t):
    return (round(a[0] + t * (b[0] - a[0]), 6), round(a[1] + t * (b[1] - a[1]), 6))


# --------------------------------------------------- Circle through 3 points
topic("three")
for t in (0.5, 2.0, -1.5, 0.25):
    a, b, c = tri()
    p = on_line(a, b, t)
    case("three", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Circle(A, B, C)"],
         [mv("C", *p), mv("C", *rp()), mv("A", *on_line(b, c, 3.0)), mv("A", *rp())])

for which in ("AB", "BC", "AC"):
    a, b, c = tri()
    first, second = which
    pts = {"A": a, "B": b, "C": c}
    steps = [mv(second, *pts[first]), mv(second, *rp()), mv(first, *rp())]
    case("three", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Circle(A, B, C)"], steps)

for _ in range(2):
    a, b, c = tri()
    case("three", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Circle(A, B, C)"],
         [mv("B", *a), mv("C", *a), mv("B", *rp()), mv("C", *rp()), mv("A", *rp())])

# tiny offsets around the tolerance of 1e-8 for coincident points
a, b, c = tri()
case("three", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Circle(A, B, C)"],
     [mv("B", a[0] + 5e-9, a[1]), mv("B", a[0] + 2e-8, a[1]), mv("B", *b)])

# very large and very small triangles, and a nearly collinear one
for scale in (1000.0, 0.001):
    a, b, c = (0.0, 0.0), (scale, 0.0), (scale * 0.3, scale * 0.8)
    case("three", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Circle(A, B, C)"],
         [mv("C", scale * 0.5, scale * 1e-6), mv("C", scale * 0.3, scale * 0.8)])

a, b, c = (0.0, 0.0), (4.0, 1.0), (2.0, 2.0)
case("three", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Circle(A, B, C)"],
     [mv("C", 8.0, 2.0), mv("C", 8.0, 2.0000001), mv("C", 8.0, 2.001), mv("C", 2.0, 2.0)])

# the circle through three points used as a parent
for _ in range(2):
    a, b, c = tri()
    case("three", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Circle(A, B, C)",
                   "O = Center(k)", "m = Circle(O, A)", "M = Midpoint(k)"],
         [mv("C", *on_line(a, b, 0.5)), mv("C", *rp()), mv("B", *a), mv("B", *rp())])

# tiny-offset coincidence for the A = C and B = C pairs
for mover, other in (("C", "A"), ("C", "B")):
    pts = {"A": (0.0, 0.0), "B": (5.0, 1.0), "C": (2.0, 4.0)}
    o = pts[other]
    case("three", [pt("A", *pts["A"]), pt("B", *pts["B"]), pt("C", *pts["C"]),
                   "k = Circle(A, B, C)"],
         [mv(mover, o[0] + 5e-9, o[1]), mv(mover, o[0] + 2e-8, o[1]),
          mv(mover, o[0], o[1] + 5e-9), mv(mover, *pts["C"])])

# ------------------------------------------- Circle with a segment as radius
topic("segrad")
for _ in range(4):
    p, b, c = rp(), rp(), rp()
    case("segrad", [pt("P", *p), pt("B", *b), pt("C", *c), "s = Segment(B, C)",
                    "k = Circle(P, s)"],
         [mv("P", *rp()), mv("C", *b), mv("C", *rp()), mv("B", *rp()), mv("B", *p)])

for _ in range(2):
    p, b, c = rp(), rp(), rp()
    case("segrad", [pt("P", *p), pt("B", *b), pt("C", *c), "s = Segment(B, C)",
                    "k = Circle(B, s)", "m = Circle(P, s)", "O = Center(m)"],
         [mv("B", *rp()), mv("C", *rp()), mv("C", *b), mv("C", *c)])

p, b, c = rp(), rp(), rp()
case("segrad", [pt("P", *p), pt("B", *b), pt("C", *c), "s = Segment(B, C)",
                "k = Circle(P, s)"],
     [mv("C", b[0] + 5e-9, b[1]), mv("C", b[0] + 2e-8, b[1]), mv("C", *c)])

# the segment is a side built from a midpoint
p, b, c = rp(), rp(), rp()
case("segrad", [pt("P", *p), pt("B", *b), pt("C", *c), "M = Midpoint(B, C)",
                "s = Segment(B, M)", "k = Circle(P, s)"],
     [mv("C", *b), mv("C", *rp()), mv("P", *b)])

# the end point of the segment is a derived point that becomes undefined
case("segrad", [pt("A", 0, 0), pt("B", 5, 1), pt("C", 2, 4), pt("P", -3, 2),
                "k = Circle(A, B, C)", "O = Center(k)", "s = Segment(P, O)",
                "m = Circle(B, s)"],
     [mv("C", 10, 2), mv("C", 2, 4), mv("B", 0, 0), mv("B", 5, 1)])

# ------------------------------------------------------------------ Incircle
topic("incircle")
for _ in range(4):
    a, b, c = tri()
    case("incircle", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Incircle(A, B, C)"],
         [mv("A", *rp()), mv("C", *rp()), mv("B", *rp())])

for t in (0.5, 2.0, -1.0):
    a, b, c = tri()
    case("incircle", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Incircle(A, B, C)"],
         [mv("C", *on_line(a, b, t)), mv("C", *rp())])

for which in ("AB", "BC", "AC"):
    a, b, c = tri()
    first, second = which
    pts = {"A": a, "B": b, "C": c}
    case("incircle", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Incircle(A, B, C)"],
         [mv(second, *pts[first]), mv(second, *rp())])

a, b, c = tri()
case("incircle", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Incircle(A, B, C)"],
     [mv("B", *a), mv("C", *a), mv("B", *rp()), mv("C", *rp())])

# the right-angled triangle with the exact inradius, and a collinear exact case
case("incircle", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 3), "k = Incircle(A, B, C)",
                  "O = Center(k)"],
     [mv("C", 2, 0), mv("C", 0, 3), mv("C", 10, 0), mv("C", 0, 3)])

for scale in (1000.0, 0.001):
    case("incircle", [pt("A", 0, 0), pt("B", scale, 0), pt("C", scale * 0.3, scale * 0.8),
                      "k = Incircle(A, B, C)"],
         [mv("C", scale * 0.5, scale * 1e-6), mv("C", scale * 0.3, scale * 0.8)])

# tiny-offset coincidence for Incircle
case("incircle", [pt("A", 0, 0), pt("B", 5, 1), pt("C", 2, 4), "k = Incircle(A, B, C)"],
     [mv("C", 5e-9, 0), mv("C", 2e-8, 0), mv("C", 5.000000005, 1), mv("C", 5.00000002, 1),
      mv("C", 2, 4)])

# --------------------------------------------------- Center / Midpoint(circle)
topic("center")
for _ in range(3):
    a, b = rp(), rp()
    case("center", [pt("A", *a), pt("B", *b), "k = Circle(A, B)", "O = Center(k)",
                    "M = Midpoint(k)"],
         [mv("A", *rp()), mv("B", *rp()), mv("B", *a), mv("B", *rp())])

for _ in range(3):
    a = rp()
    case("center", [pt("A", *a), num("r", 2.5), "k = Circle(A, r)", "O = Center(k)",
                    "M = Midpoint(k)"],
         [mv("A", *rp()), setn("r", 0), setn("r", 4), setn("r", -1), setn("r", 1.5),
          setn("r", 0)])

for _ in range(2):
    a, b, c = tri()
    case("center", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Circle(A, B, C)",
                    "O = Center(k)", "m = Circle(O, C)"],
         [mv("C", *on_line(a, b, 0.5)), mv("C", *rp()), mv("B", *a), mv("B", *rp())])

a, b, c = tri()
case("center", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Incircle(A, B, C)",
                "O = Center(k)", "M = Midpoint(k)"],
     [mv("C", *on_line(a, b, 0.5)), mv("C", *rp()), mv("B", *a), mv("B", *rp())])

# a circle of tiny radius: GeoGebra snaps |r| < 1e-8 to 0
a = rp()
case("center", [pt("A", *a), num("r", 2), "k = Circle(A, r)", "O = Center(k)"],
     [setn("r", 5e-9), setn("r", 2e-8), setn("r", 0), setn("r", 3)])

# ----------------------------------------------------------- Midpoint(segment)
topic("midseg")
for _ in range(4):
    a, b = rp(), rp()
    case("midseg", [pt("A", *a), pt("B", *b), "s = Segment(A, B)", "M = Midpoint(s)",
                    "N = Midpoint(A, B)"],
         [mv("A", *rp()), mv("B", *a), mv("B", *rp()), mv("A", *rp(-0.5, 0.5))])

a, b = rp(), rp()
case("midseg", [pt("A", *a), pt("B", *b), "s = Segment(B, A)", "M = Midpoint(s)",
                "t = Segment(M, A)", "N = Midpoint(t)"],
     [mv("B", *a), mv("B", *rp()), mv("A", *rp())])

# the segment end point is a derived point that becomes undefined
case("midseg", [pt("A", 0, 0), pt("B", 5, 1), pt("C", 2, 4), pt("P", -3, 2),
                "k = Circle(A, B, C)", "O = Center(k)", "s = Segment(P, O)",
                "M = Midpoint(s)"],
     [mv("C", 10, 2), mv("C", 2, 4), mv("B", 0, 0), mv("B", 5, 1)])

# --------------------------------------------------------- Diameter(line, circle)
topic("diameter")
for _ in range(3):
    a, b, c, d = rp(), rp(), rp(), rp()
    case("diameter", [pt("A", *a), pt("B", *b), pt("C", *c), pt("D", *d),
                      "f = Line(A, B)", "k = Circle(C, D)", "g = Diameter(f, k)"],
         [mv("A", *rp()), mv("B", *rp()), mv("C", *rp()), mv("D", *c), mv("D", *rp())])

a, b, c = tri()
case("diameter", [pt("A", *a), pt("B", *b), pt("C", *c), "s = Segment(A, B)",
                  num("r", 3), "k = Circle(C, r)", "g = Diameter(s, k)"],
     [setn("r", 0), setn("r", 2), mv("B", *a), mv("B", *rp())])

# the line becomes undefined and defined again
case("diameter", [pt("A", 1, 1), pt("B", 4, 3), pt("C", -2, 0), pt("D", 1, -3),
                  "f = Line(A, B)", "k = Circle(C, D)", "g = Diameter(f, k)"],
     [mv("B", 1, 1), mv("B", 4, 3), mv("A", 4, 3), mv("A", 1, 1)])

# a three-point circle driven collinear and back
case("diameter", [pt("A", 0, 0), pt("B", 5, 1), pt("C", 2, 4), pt("P", 1, 1), pt("Q", 4, 3),
                  "f = Line(P, Q)", "k = Circle(A, B, C)", "g = Diameter(f, k)"],
     [mv("C", 10, 2), mv("C", 2, 4), mv("B", 0, 0), mv("B", 5, 1), mv("C", 0, 0), mv("C", 2, 4)])

# a negative radius and back
case("diameter", [pt("A", 1, 1), pt("B", 4, 3), pt("C", -2, 0), num("r", 2),
                  "f = Line(A, B)", "k = Circle(C, r)", "g = Diameter(f, k)"],
     [setn("r", -1), setn("r", 2), setn("r", 0), setn("r", -3), setn("r", 1.5)])

# a segment driven to length 0 and to a tiny length
case("diameter", [pt("A", 1, 1), pt("B", 4, 3), pt("C", -2, 0), num("r", 2),
                  "s = Segment(A, B)", "k = Circle(C, r)", "g = Diameter(s, k)"],
     [mv("B", 1, 1), mv("B", 4, 3), mv("B", 1 + 5e-9, 1), mv("B", 1 + 2e-8, 1), mv("B", 4, 3)])

cases.write()
