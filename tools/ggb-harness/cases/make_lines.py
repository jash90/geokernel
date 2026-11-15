#!/usr/bin/env python3
"""Writes cases/lines.json: perpendicular and angle bisectors, tangents to
circles and polar lines.

The helpers live in caselib.py. Snapshots come only from running GeoGebra
through the harness (`harness.sh golden lines`).

`AngularBisector(g, h)`, `Tangent(P, c)` and `Tangent(g, c)` create two
lines. When both are defined at creation GeoGebra labels them `t_{1}` and
`t_{2}`; when only one is, it gets the plain label `t` and the other none
(the `parallel` and `oncircle` topics). When none is defined, GeoGebra gives
no label at creation and names the outputs only once they become defined, so
no case starts that way.

New cases go at the end of their topic and use no random numbers, so the
cases already recorded stay the same.

GeoGebra gives an object that is undefined when it is created no label, so
every other object is defined in the initial state and degenerate states are
reached by moves only.
"""

import math

from caselib import Cases, mv, num, pt, setn

cases = Cases("lines")
rng = cases.rng
case = cases.case
topic = cases.topic
r2 = cases.r2
rp = cases.rp

# --------------------------------------------------------------- bisector
topic("bisector")
for _ in range(6):
    a, b = rp(), rp()
    case("bisector", [pt("A", *a), pt("B", *b), "f = LineBisector(A, B)",
                      "g = PerpendicularBisector(B, A)"],
         [mv("B", *rp()),
          mv("B", a[0], a[1]),              # coincident: undefined
          mv("B", a[0] + 1e-6, a[1]),       # very close, still defined
          mv("A", *rp()),
          mv("B", *rp())])

for _ in range(4):
    a, b = rp(), rp()
    case("bisector", [pt("A", *a), pt("B", *b), "s = Segment(A, B)",
                      "f = LineBisector(s)", "g = PerpendicularBisector(s)"],
         [mv("A", *rp()), mv("A", b[0], b[1]), mv("A", *rp()),
          mv("B", a[0], a[1] + 5.5)])

# GeoGebra's name OrthogonalLine for the perpendicular through a point
for _ in range(2):
    a, b, p = rp(), rp(), rp()
    case("bisector", [pt("A", *a), pt("B", *b), pt("P", *p), "l = Line(A, B)",
                      "s = Segment(A, B)", "f = OrthogonalLine(P, l)",
                      "g = OrthogonalLine(P, s)"],
         [mv("P", *rp()), mv("B", a[0], a[1]), mv("B", *rp())])

# segment end points 1e-6 apart, near the origin and far from it
case("bisector", ["A = (2, 3)", "B = (5, -1)", "s = Segment(A, B)", "f = LineBisector(s)",
                  "g = PerpendicularBisector(A, B)"],
     [mv("B", 2.000001, 3), mv("B", 2, 3.000001), mv("B", 2.000001, 3.000001), mv("B", 2, 3),
      mv("B", 1.999999, 3), mv("B", 5, -1)])
case("bisector", ["A = (1000, -1000)", "B = (1003, -996)", "s = Segment(A, B)", "f = LineBisector(s)"],
     [mv("B", 1000.000001, -1000), mv("B", 1000, -999.999999), mv("B", 1000, -1000),
      mv("B", 1003, -996)])

# ------------------------------------------------------------------ angle
topic("angle")
for _ in range(6):
    a, b, c = rp(), rp(), rp()
    case("angle", [pt("A", *a), pt("B", *b), pt("C", *c), "f = AngularBisector(A, B, C)",
                   "g = AngleBisector(C, B, A)"],
         [mv("C", *rp()),
          mv("C", 2 * b[0] - a[0], 2 * b[1] - a[1]),     # straight angle at B
          mv("C", b[0] + 2 * (a[0] - b[0]), b[1] + 2 * (a[1] - b[1])),  # zero angle
          mv("C", a[0], a[1]),                           # C on A
          mv("A", b[0], b[1]),                           # leg of length zero
          mv("A", *rp()),
          mv("B", *rp())])

# reflex angles and the orientation of the result
for k in range(4):
    ang = 0.4 + 1.3 * k
    b = rp()
    a = (b[0] + 3 * math.cos(ang), b[1] + 3 * math.sin(ang))
    steps = []
    for j in range(1, 8):
        t = ang + j * 0.85
        steps.append(mv("C", b[0] + 2 * math.cos(t), b[1] + 2 * math.sin(t)))
    case("angle", [pt("A", *a), pt("B", *b), pt("C", b[0] + 2, b[1]),
                   "f = AngularBisector(A, B, C)"], steps)

# legs through the origin: GeoGebra's cross product rescales tiny z values
case("angle", ["A = (0.3, 0.6)", "B = (0.1, 0.2)", "C = (2, -1)",
               "f = AngularBisector(A, B, C)", "g = AngularBisector(C, B, A)"],
     [mv("A", 0.7, 1.4), mv("C", 0.3, 0.6), mv("A", -0.3, -0.6), mv("B", 0.2, 0.4),
      mv("C", 1.1, 2.2)])
case("angle", ["A = (1, 1)", "B = (2, 2)", "C = (3, 0)", "f = AngularBisector(A, B, C)"],
     [mv("A", 0, 0), mv("A", -1, -1), mv("C", 0.5, 0.5), mv("B", 0.1, 0.1)])

# derived points (midpoints) as legs and vertex, including coincident ones
case("angle", ["A = (0, 0)", "B = (6, 0)", "C = (2, 5)", "D = (-3, 4)", "M = Midpoint(A, B)",
               "N = Midpoint(B, C)", "K = Midpoint(C, D)", "f = AngularBisector(M, N, K)",
               "g = AngularBisector(K, M, N)"],
     [mv("C", 7, 1), mv("D", 6, 0), mv("D", -1, 4), mv("C", 0, 0), mv("A", 4, 3),
      mv("B", -2, 5), mv("C", 0.5, -3.5)])
case("angle", ["A = (1, 1)", "B = (5, 2)", "C = (2, 6)", "s = Segment(A, B)", "M = Midpoint(s)",
               "N = Midpoint(A, C)", "f = AngularBisector(M, A, N)", "g = AngularBisector(N, M, B)"],
     [mv("C", 9, 3), mv("C", 5, 2), mv("B", 2, 6), mv("A", 2, 6), mv("A", -4, 0)])

# each pair of points coincident, then all three; in and back out
case("angle", ["A = (4, 1)", "B = (1, 2)", "C = (2, 5)", "f = AngularBisector(A, B, C)"],
     [mv("A", 1, 2), mv("A", 4, 1),       # A on B
      mv("C", 1, 2), mv("C", 2, 5),       # C on B
      mv("C", 4, 1), mv("C", 2, 5),       # C on A
      mv("A", 1, 2), mv("C", 1, 2),       # all three on B
      mv("C", 2, 5), mv("A", 4, 1)])
case("angle", ["A = (-3, 0)", "B = (0, 0)", "C = (0, 3)", "f = AngularBisector(A, B, C)",
               "g = AngularBisector(C, A, B)"],
     [mv("B", -3, 0), mv("B", 0, 0), mv("A", 0, 3), mv("A", -3, 0), mv("B", 0, 3), mv("A", 0, 3),
      mv("A", -3, 0), mv("B", 0, 0), mv("A", 0.000001, 0), mv("A", -3, 0)])

# ------------------------------------------------------------------ polar
topic("polar")
for _ in range(4):
    p, m = rp(), rp()
    rad = round(rng.uniform(0.5, 4), 2)
    case("polar", [pt("P", *p), pt("M", *m), num("r", rad), "c = Circle(M, r)",
                   "p = Polar(P, c)"],
         [mv("P", *rp()), mv("P", m[0], m[1]), mv("P", m[0] + 1e-6, m[1]),
          mv("P", m[0] + rad, m[1]), setn("r", 0), mv("P", *rp()), mv("M", *rp())])
case("polar", ["A = (1, 1)", "B = (3, 2)", "c = Circle(A, B)", "P = (4, -2)", "p = Polar(P, c)",
               "q = Polar(B, c)"],
     [mv("P", 2, 2), mv("P", -3, 5), mv("B", 1, 1), mv("B", 0, 0)])

# --------------------------------------------------------------- oncircle
# The point is on the circle at creation: both tangents are the same line, so
# GeoGebra makes the second one undefined and labels only the first, `t`.
topic("oncircle")
case("oncircle", ["A = (0, 0)", "r = 5", "c = Circle(A, r)", "P = (3, 4)", "t = Tangent(P, c)"],
     [mv("P", -4, 3), mv("P", 6, 8), mv("P", 7, 8), mv("P", 0, -5), mv("P", -9, 1)])
case("oncircle", ["A = (1, 1)", "B = (4, 5)", "c = Circle(A, B)", "t = Tangent(B, c)"],
     [mv("B", -2, 1), mv("A", 0, 0), mv("B", 3, 0)])
case("oncircle", ["M = (-2, 1)", "r = 13", "c = Circle(M, r)", "P = (3, 13)", "t = Tangent(c, P)"],
     [mv("P", 3, 14), mv("P", 3, 16), mv("P", 3, 30), mv("P", -2, 1), mv("P", 3, 13),
      mv("P", 14, -4), mv("P", -2, -12), setn("r", 0), setn("r", 13)])
# leaving the circle in different directions and jumping between sides
for k in range(5):
    cx, cy = rp(-4, 4)
    rad = (5, 10, 13, 25, 2.5)[k]
    start = ((3, 4), (6, 8), (5, 12), (7, 24), (1.5, 2))[k]
    steps = []
    for j in range(8):
        ang = rng.uniform(0, 2 * math.pi)
        dist = rad * rng.choice([0.4, 1.0, 1.0000001, 1.2, 2.0, 5.0])
        steps.append(mv("P", cx + dist * math.cos(ang), cy + dist * math.sin(ang)))
    case("oncircle", [pt("M", cx, cy), num("r", rad), "c = Circle(M, r)",
                      pt("P", cx + start[0], cy - start[1]), "t = Tangent(P, c)"], steps)
case("oncircle", ["A = (0, 0)", "B = (0, 5)", "c = Circle(A, B)", "P = (-3, 4)",
                  "t = Tangent(P, c)", "u = Tangent(B, c)"],
     [mv("P", -6, 8), mv("B", 0, 6), mv("P", 6, -8), mv("A", 2, 1), mv("P", -9, 12)])

# --------------------------------------------------------------- parallel
# Parallel or identical lines at creation: one bisector is undefined, so
# GeoGebra labels only the other, `b` (bisector 1 when the directions agree,
# bisector 2 when they are opposite).
topic("parallel")
for k in range(3):
    a, b, c = (2 * k - 3, 1), (2 * k + 1, 3), (k, -2)
    d_same = (c[0] + b[0] - a[0], c[1] + b[1] - a[1])
    d_opp = (c[0] - b[0] + a[0], c[1] - b[1] + a[1])
    for d in (d_same, d_opp):
        case("parallel", [pt("A", *a), pt("B", *b), pt("C", *c), pt("D", *d),
                          "l = Line(A, B)", "m = Line(C, D)", "b = AngularBisector(l, m)"],
             [mv("D", d[0] + 1, d[1] + 3), mv("D", *d), mv("D", *rp()), mv("C", *a),
              mv("D", *b), mv("D", *a), mv("A", *rp()), mv("C", *rp())])
case("parallel", ["A = (0, 0)", "B = (3, 1)", "P = (1, 4)", "l = Line(A, B)", "m = Line(P, l)",
                  "b = AngleBisector(m, l)"],
     [mv("P", 2, -1), mv("B", 0, 5), mv("P", 0, 2), mv("B", -4, 1)])
case("parallel", ["A = (1, 2)", "B = (5, 3)", "l = Line(A, B)", "m = Line(A, B)",
                  "n = Line(B, A)", "b = AngularBisector(l, m)", "w = AngularBisector(l, n)"],
     [mv("B", 2, 7), mv("A", 2, 7), mv("A", -3, 0)])
case("parallel", ["A = (0, 0)", "B = (4, 0)", "C = (1, 2)", "D = (-3, 2)", "s = Segment(A, B)",
                  "h = Segment(C, D)", "b = AngularBisector(s, h)"],
     [mv("D", -3, 5), mv("D", 5, 2), mv("D", 5, -1), mv("C", 4, 0)])

# -------------------------------------------------------------- anglelines
topic("anglelines")
for _ in range(6):
    a, b, c, d = rp(), rp(), rp(), rp()
    case("anglelines", [pt("A", *a), pt("B", *b), pt("C", *c), pt("D", *d),
                        "l = Line(A, B)", "m = Line(C, D)", "b = AngularBisector(l, m)"],
         [mv("D", c[0] + b[0] - a[0], c[1] + b[1] - a[1]),   # parallel, same direction
          mv("D", c[0] - b[0] + a[0], c[1] - b[1] + a[1]),   # parallel, opposite direction
          mv("D", *rp()),
          mv("C", a[0], a[1]), mv("D", b[0], b[1]),          # identical lines
          mv("D", *rp()),
          mv("A", *rp())])

# exactly representable parallel and identical positions
for k in range(3):
    case("anglelines", ["A = (0, 0)", "B = (4, 2)", "C = (1, 3)", "D = (3, -1)",
                        "l = Line(A, B)", "m = Line(C, D)", f"w = AngleBisector(m, l)"],
         [mv("D", 5 + k, 5 + k * 0.5), mv("D", -3, 1), mv("C", 2, 1), mv("C", 6, 3 + k),
          mv("D", 2 - 2 * k, 1 - k), mv("C", -4, -2)])

# perpendicular lines and segments as arguments
case("anglelines", ["A = (0, 0)", "B = (4, 0)", "C = (1, -2)", "D = (1, 5)",
                    "s = Segment(A, B)", "h = Segment(C, D)", "b = AngularBisector(s, h)"],
     [mv("D", 1, -6), mv("B", -3, 0), mv("D", 4, 1), mv("D", 8, 0), mv("D", 9, 3)])
case("anglelines", ["A = (-2, 1)", "B = (3, 2)", "C = (0, -3)", "P = (1, 1)",
                    "l = Line(A, B)", "n = PerpendicularLine(P, l)", "m = Line(C, P)",
                    "b = AngularBisector(l, n)", "u = AngularBisector(n, m)"],
     [mv("P", 4, -2), mv("B", 5, -1), mv("C", 4, -2), mv("C", 1, 7)])

# long, nearly parallel lines. For l = Line((0, 0), (1000, 1000)) and
# m = Line((0, 1), (1000, 1001 + d)) GeoGebra's raw cross product is 1000 d
# and the unit-normal one d / 2000, and the lines meet near x = -1000 / d.
# GeoGebra calls the intersection infinite when the raw value is at most
# 1e-8, so d in (1e-11, 2e-5) is the window where the two tests disagree
# (moves are rounded to 6 decimals, so d >= 1e-6 here).
case("anglelines", ["A = (0, 0)", "B = (1000, 1000)", "C = (0, 1)", "D = (1000, 900)",
                    "l = Line(A, B)", "m = Line(C, D)", "b = AngularBisector(l, m)"],
     [mv("D", 1000, 1001.001), mv("D", 1000, 1001.00001), mv("D", 1000, 1001.000002),
      mv("D", 1000, 1001.000001), mv("D", 1000, 1001), mv("D", 1000, 1000.999999),
      mv("D", 1000, 900)])
# Opposite directions and a shallow slope: the intersection is far along x
# but not along y, which keeps it finite for both tests.
case("anglelines", ["A = (-500, 0)", "B = (1500, 1)", "C = (-500, 3)", "D = (1500, -2)",
                    "l = Line(A, B)", "m = Line(D, C)", "b = AngularBisector(l, m)"],
     [mv("D", 1500, 4.000001), mv("D", 1500, 4), mv("D", 1500, 3.999999), mv("C", -500, 2.999),
      mv("C", -500, 3)])

# segments: parallel, opposite, collinear, sharing an end point, zero length
case("anglelines", ["A = (0, 0)", "B = (4, 1)", "C = (1, 3)", "D = (3, -2)", "s = Segment(A, B)",
                    "h = Segment(C, D)", "b = AngularBisector(s, h)", "w = AngleBisector(h, s)"],
     [mv("D", 5, 4), mv("D", -3, 2), mv("D", 3, -2), mv("C", 8, 2), mv("D", 12, 3), mv("D", 3, -2),
      mv("C", 0, 0), mv("C", 1, 3), mv("D", 1, 3), mv("D", 3, -2), mv("B", 0, 0), mv("B", 4, 1)])

# ---------------------------------------------------------------- tangent
topic("tangent")
# exact positions: on the circle, inside, at the centre, back out
case("tangent", ["A = (0, 0)", "r = 2", "c = Circle(A, r)", "P = (5, 1)", "t = Tangent(P, c)"],
     [mv("P", 2, 0), mv("P", 1.2, 1.6), mv("P", 0.5, 0), mv("P", 0, 0), mv("P", -5, 1),
      mv("P", 0, -3), mv("P", 3, 4)])
case("tangent", ["A = (1, -1)", "B = (4, 3)", "c = Circle(A, B)", "P = (-6, 2)",
                 "t = Tangent(P, c)", "u = Tangent(c, P)"],
     [mv("P", 4, 3), mv("P", -2, -4), mv("P", 1, 2), mv("P", 6, -1), mv("B", 1, -1),
      mv("B", 2, 1), mv("A", 7, -1)])
# the radius goes to zero, negative and back
case("tangent", ["M = (2, 3)", "r = 1.5", "c = Circle(M, r)", "P = (-3, 4)", "t = Tangent(P, c)"],
     [setn("r", 0), setn("r", 1e-6), setn("r", 0.5), setn("r", -1), setn("r", 5.09902),
      setn("r", 5.0990195135927845), setn("r", 7), setn("r", 2.25)])
# the same end positions reached directly and through detours
for end in ((-5, 1), (1, -6), (4.5, 2.5)):
    case("tangent", ["A = (0, 0)", "r = 2", "c = Circle(A, r)",
                     pt("P", *end), "t = Tangent(P, c)"], [mv("P", end[0] + 0.25, end[1])])
    case("tangent", ["A = (0, 0)", "r = 2", "c = Circle(A, r)", "P = (5, 1)",
                     "t = Tangent(P, c)"],
         [mv("P", 0, 2), mv("P", 0.3, -0.2), mv("P", -5, -1), mv("P", -1, 5),
          mv("P", *end), mv("P", end[0] + 0.25, end[1])])
# the point circles the centre at several distances, crossing the circle
for _ in range(5):
    cx, cy = rp(-4, 4)
    rad = round(rng.uniform(0.5, 4), 2)
    steps = []
    for j in range(10):
        ang = rng.uniform(0, 2 * math.pi)
        dist = rad * rng.choice([0.3, 0.999999, 1.0, 1.000001, 1.5, 3.0])
        steps.append(mv("P", cx + dist * math.cos(ang), cy + dist * math.sin(ang)))
    case("tangent", [pt("M", cx, cy), num("r", rad), "c = Circle(M, r)",
                     pt("P", cx + 2.5 * rad, cy - 0.5 * rad), "t = Tangent(P, c)"], steps)
# the circle moves under a fixed point
for _ in range(3):
    p = rp()
    m = (p[0] + 5, p[1] - 1)
    case("tangent", [pt("P", *p), pt("M", *m), pt("B", m[0] + 2, m[1]), "c = Circle(M, B)",
                     "t = Tangent(P, c)"],
         [mv("M", p[0] + 1, p[1]), mv("M", p[0] - 3, p[1] + 1), mv("B", p[0] - 3, p[1] + 6),
          mv("M", *rp()), mv("B", *rp())])
# which tangent comes first when the point goes to the other side of the
# circle: around the outside, by one jump, and straight through the circle
case("tangent", ["A = (0, 0)", "r = 2", "c = Circle(A, r)", "P = (5, 1)", "t = Tangent(P, c)"],
     [mv("P", 4, 3), mv("P", 1, 5), mv("P", -3, 4), mv("P", -5, 1), mv("P", -4, -3),
      mv("P", 1, -5), mv("P", 5, 1)])
case("tangent", ["A = (0, 0)", "r = 2", "c = Circle(A, r)", "P = (5, 1)", "t = Tangent(P, c)"],
     [mv("P", -5, 1), mv("P", 5, 1), mv("P", -5, -1), mv("P", 1, 5), mv("P", 1, -5)])
case("tangent", ["A = (0, 0)", "r = 2", "c = Circle(A, r)", "P = (5, 0)", "t = Tangent(P, c)"],
     [mv("P", 3, 0), mv("P", 2, 0), mv("P", 1, 0), mv("P", 0, 0), mv("P", -1, 0), mv("P", -2, 0),
      mv("P", -3, 0), mv("P", -5, 0), mv("P", 5, 0)])
case("tangent", ["A = (0, 0)", "r = 2", "c = Circle(A, r)", "P = (5, 1.2)", "t = Tangent(P, c)"],
     [mv("P", 1.6, 1.2), mv("P", 0, 1.2), mv("P", -1.6, 1.2), mv("P", -5, 1.2), mv("P", 0, 1.2),
      mv("P", 5, 1.2)])
case("tangent", ["A = (0, 0)", "r = 2", "c = Circle(A, r)", "P = (5, 1)", "t = Tangent(P, c)"],
     [mv("P", 0.5, 0.5), mv("P", -5, 1), mv("P", 0.5, 0.5), mv("P", 5, 1), mv("P", 1, 5)])
# ------------------------------------------------------------ tangentline
topic("tangentline")
for _ in range(4):
    a, b, m = rp(), rp(), rp()
    rad = round(rng.uniform(0.5, 4), 2)
    case("tangentline", [pt("A", *a), pt("B", *b), pt("M", *m), num("r", rad),
                         "l = Line(A, B)", "c = Circle(M, r)", "t = Tangent(l, c)"],
         [mv("B", *rp()), mv("M", *rp()), setn("r", 0), setn("r", 2.5),
          mv("B", a[0], a[1]), mv("B", *rp()), mv("A", *rp())])
case("tangentline", ["A = (0, 0)", "B = (3, 4)", "M = (1, 2)", "N = (2, 2)", "s = Segment(A, B)",
                     "c = Circle(M, N)", "t = Tangent(s, c)"],
     [mv("B", -3, 4), mv("N", 1, 2), mv("N", 1, 7), mv("A", 3, 4), mv("A", 5, -1)])
case("tangentline", ["A = (-1, 2)", "B = (4, 2)", "M = (0, 0)", "r = 1", "l = Line(A, B)",
                     "c = Circle(M, r)", "t = Tangent(l, c)", "u = Tangent(l, c)"],
     [mv("B", -1, 7), mv("B", -6, 2), mv("B", -1, -3), mv("M", 4, 4)])

# a segment: through the centre, zero length, vertical, touching the circle
case("tangentline", ["A = (-2, -1)", "B = (3, 1)", "M = (0, 2)", "r = 1.5", "s = Segment(A, B)",
                     "c = Circle(M, r)", "t = Tangent(s, c)"],
     [mv("B", 2, 6), mv("B", -2, -1), mv("B", -2, 4), mv("A", -1.5, 0), mv("B", 1.5, 7),
      mv("B", 3, 1), setn("r", 0), setn("r", 1.5)])

# ------------------------------------------------------------ smallcircle
# Circles with a tiny radius, set by number or by moving the through point
# (moves are rounded to 6 decimals, so the centre itself carries the two
# smallest radii in the last cases).
topic("smallcircle")
SMALL = (2e-4, 5e-5, 1e-5, 1e-6, 2e-8, 5e-9)
REST = ["P = (4, 3)", "E = (-2, 5)", "F = (3, -1)", "l = Line(E, F)", "t = Tangent(P, c)",
        "u = Tangent(l, c)", "p = Polar(P, c)"]
case("smallcircle", ["A = (1, 2)", "r = 1", "c = Circle(A, r)"] + REST,
     [setn("r", v) for v in SMALL] + [setn("r", -5e-9), setn("r", 0), setn("r", 1e-8),
                                         setn("r", 1)])
case("smallcircle", ["A = (1, 2)", "B = (2, 2)", "c = Circle(A, B)"] + REST,
     [mv("B", 1.0002, 2), mv("B", 1, 2.00005), mv("B", 1.00001, 2), mv("B", 1.000001, 2),
      mv("B", 1, 2), mv("B", 2, 2)])
for centre in ("0.00000002", "0.000000005"):
    case("smallcircle", [f"A = ({centre}, 0)", "B = (3, 0)", "c = Circle(A, B)"] + REST,
         [mv("B", 0, 0), mv("B", 3, 0)])

# ------------------------------------------------ raw scale of the lines
# Lines of this group with tiny raw coefficients used where GeoGebra tests
# raw coefficients against 1e-8: Distance(line, line), Slope and
# AngularBisector(line, line). The tiny offsets are carried by script
# literals, since moves are rounded to 6 decimals.
topic("bisector")
# perpendicular bisectors of two pairs of points 1e-6 apart
case("bisector", [pt("A", 0, 0), pt("B", 0.000001, 0.000003), pt("C", 2, 1),
                  pt("D", 2.000002, 1.000001), "f = LineBisector(A, B)", "g = LineBisector(C, D)",
                  "d = Distance(f, g)", "e = Distance(g, f)", "m = Slope(f)",
                  "b = AngularBisector(f, g)"],
     [mv("D", 2.000001, 1.000003), mv("D", 2.000003, 1.000001), mv("B", 0.000001, 0),
      mv("B", 3, 1), mv("D", 4, 2), mv("D", 2.000002, 1.000001)])

topic("polar")
# the pole 2e-8 and 5e-9 off the centre: the polar is defined, its raw y
# coefficient passes GeoGebra's zero test
case("polar", ["A = (0.00000002, 0.000000005)", pt("B", 2, 0), "c = Circle(A, B)", pt("P", 1, 1),
               "p = Polar(P, c)", pt("E", -1, 3), pt("F", 4, 2), "l = Line(E, F)",
               "m = Slope(p)", "d = Distance(p, l)", "b = AngularBisector(p, l)"],
     [mv("P", 0, 0), mv("F", 4, 3), mv("P", 0, 1), mv("P", 0, 0), mv("P", 1, 1)])

topic("tangent")
# the point approaches the circle: the two tangents become short and nearly
# the same line
case("tangent", [pt("A", 0, 0), num("r", 2), "c = Circle(A, r)", pt("P", 3, 0),
                 "t = Tangent(P, c)", "d = Distance(t_{1}, t_{2})", "m = Slope(t_{1})",
                 "b = AngularBisector(t_{1}, t_{2})"],
     [mv("P", 2.001, 0), mv("P", 2.00001, 0), mv("P", 2.000001, 0), mv("P", 0, 2.000001),
      mv("P", 1.414214, 1.414214), mv("P", 3, 0)])
# the polar and the tangents of a dilated circle: GeoGebra scales the
# circle's matrix, and the polar and the tangent at a point on the circle
# carry that scale
case("polar", [pt("A", 1, 1), num("r", 2), "k = Circle(A, r)", num("f", 3), "c = Dilate(k, f)",
               pt("P", 12, 9), "p = Polar(P, c)", "t = Tangent(P, c)", pt("E", 2, -1),
               pt("F", 4, 0), "l = Line(E, F)", "u = Tangent(l, c)", "d = Distance(p, l)"],
     [setn("f", 0.5), mv("P", 2.5, 1.5), setn("f", -2), mv("P", 9, 4), setn("f", 3)])

cases.write()
