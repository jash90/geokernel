#!/usr/bin/env python3
"""Writes cases/intersect.json: line-circle and circle-circle intersections.

The cases drive the intersections through the states where GeoGebra has
special rules: tangency in both directions, a point that defines the line and
lies on the circle by construction, circles through a common point that become
identical, concentric circles, zero and tiny radii, and the order of the two
points when the line or the line of centres changes direction.

Every case starts with two distinct intersection points, so that every object
is defined when it is created and GeoGebra fixes the order of the points at
creation. Snapshots come exclusively from running GeoGebra through the
harness (`harness.sh golden intersect`).
"""

import math

from caselib import Cases, mv, num, pt, setn

cases = Cases("intersect")
rng = cases.rng
case = cases.case
topic = cases.topic
r2 = cases.r2
rp = cases.rp

LC = ["P = Intersect(f, k, 1)", "Q = Intersect(f, k, 2)"]
CC = ["P = Intersect(k, c, 1)", "Q = Intersect(k, c, 2)"]

# ------------------------------------------------- line x circle: tangency
topic("tangent")
# A horizontal line moves onto the top of the circle, beyond it, back onto it
# from outside and back in; then the same at the bottom.
for _ in range(4):
    a = rp(-4, 4)
    r = round(rng.uniform(1, 5), 2)
    y0 = round(a[1] + rng.uniform(-0.6, 0.6) * r, 2)
    top, bottom = round(a[1] + r, 2), round(a[1] - r, 2)
    case("tangent", [pt("A", *a), num("r", r), "k = Circle(A, r)",
                     pt("C", a[0] - 6, y0), pt("D", a[0] + 6, y0), "f = Line(C, D)"] + LC,
         [mv("C", a[0] - 6, top), mv("D", a[0] + 6, top),
          mv("C", a[0] - 6, top + 0.25), mv("D", a[0] + 6, top + 0.25),
          mv("D", a[0] + 6, top), mv("C", a[0] - 6, top),
          mv("C", a[0] - 6, y0), mv("D", a[0] + 6, y0),
          mv("C", a[0] - 6, bottom), mv("D", a[0] + 6, bottom),
          mv("D", a[0] + 6, bottom - 0.5), mv("C", a[0] - 6, bottom - 0.5),
          mv("C", a[0] - 6, bottom), mv("D", a[0] + 6, bottom)])

# A vertical line moves onto the left and the right side of the circle.
for _ in range(4):
    a = rp(-4, 4)
    r = round(rng.uniform(1, 5), 2)
    x0 = round(a[0] + rng.uniform(-0.6, 0.6) * r, 2)
    left, right = round(a[0] - r, 2), round(a[0] + r, 2)
    case("tangent", [pt("A", *a), num("r", r), "k = Circle(A, r)",
                     pt("C", x0, a[1] - 6), pt("D", x0, a[1] + 6), "f = Line(C, D)"] + LC,
         [mv("C", left, a[1] - 6), mv("D", left, a[1] + 6),
          mv("D", left - 0.5, a[1] + 6), mv("C", left - 0.5, a[1] - 6),
          mv("C", left, a[1] - 6), mv("D", left, a[1] + 6),
          mv("C", right, a[1] - 6), mv("D", right, a[1] + 6),
          mv("D", x0, a[1] + 6), mv("C", x0, a[1] - 6)])

# A slanted line is turned around a point outside the circle onto a tangent
# (a 3-4-5 triangle keeps the tangent point at short decimal coordinates).
for _ in range(4):
    a = rp(-3, 3)
    s = round(rng.uniform(0.4, 1.0), 2)
    r = round(5 * s, 2)
    t = (round(a[0] + 3 * s, 4), round(a[1] + 4 * s, 4))      # tangent point
    d = (-4 * s, 3 * s)                                      # tangent direction
    c0 = (round(t[0] - 1.5 * d[0], 4), round(t[1] - 1.5 * d[1], 4))
    d0 = (round(t[0] + 1.5 * d[0], 4), round(t[1] + 1.5 * d[1], 4))
    case("tangent", [pt("A", *a), num("r", r), "k = Circle(A, r)",
                     pt("C", *c0), pt("D", a[0] - 0.5, a[1] - 0.5), "f = Line(C, D)"] + LC,
         [mv("D", *d0), mv("D", d0[0] + 0.3, d0[1] + 0.6), mv("D", *d0),
          mv("D", a[0] + 0.25, a[1]), mv("C", *d0), mv("D", *c0)])

# ---------------------------------------------- line x circle: radius
topic("radius")
for _ in range(6):
    a = rp(-3, 3)
    off = round(rng.uniform(0.5, 2), 2)
    r = round(off + rng.uniform(0.5, 2), 2)
    case("radius", [pt("A", *a), num("r", r), "k = Circle(A, r)",
                    pt("C", a[0] - 5, a[1] + off), pt("D", a[0] + 5, a[1] + off),
                    "f = Line(C, D)"] + LC,
         [setn("r", off), setn("r", off - 0.01), setn("r", off), setn("r", r),
          mv("C", a[0] - 5, a[1]), mv("D", a[0] + 5, a[1]),          # through the centre
          setn("r", 0), setn("r", 1e-9), setn("r", -1e-9), setn("r", -2),
          setn("r", 3e-8), setn("r", r), mv("D", a[0] + 5, a[1] - 1)])

# --------------------------- line x circle: a defining point on the circle
topic("onpoint")
# k = Circle(A, B) passes through B by construction and f = Line(B, C) or
# Line(C, B) is defined through B.
for order in (0, 1, 0, 1):
    a, b, c = rp(-3, 3), rp(), rp()
    line = "f = Line(B, C)" if order == 0 else "f = Line(C, B)"
    case("onpoint", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Circle(A, B)", line] + LC,
         [mv("C", *rp()), mv("C", *a),                                # through the centre
          mv("C", 2 * a[0] - b[0], 2 * a[1] - b[1]),                  # diametrically opposite
          mv("C", b[0] - (b[1] - a[1]), b[1] + (b[0] - a[0])),        # tangent at B
          mv("C", *rp()), mv("A", *rp()), mv("B", *rp()), mv("C", *rp())])

# Perpendicular and parallel lines through the point on the circle.
for kind in ("n = PerpendicularLine(B, g)", "n = Line(B, g)", "n = PerpendicularLine(B, g)",
             "n = Line(B, g)"):
    a, b, d, e = rp(-3, 3), rp(), rp(), rp()
    case("onpoint", [pt("A", *a), pt("B", *b), pt("D", *d), pt("E", *e),
                     "k = Circle(A, B)", "g = Line(D, E)", kind,
                     "P = Intersect(n, k, 1)", "Q = Intersect(n, k, 2)"],
         [mv("E", *rp()), mv("B", *rp()), mv("D", *rp()), mv("A", *rp()), mv("E", *rp())])

# A free point that defines the line slides onto the circle and off again;
# it is not on the circle by construction, so no point is preferred.
for _ in range(4):
    a = rp(-3, 3)
    s = round(rng.uniform(0.4, 1.0), 2)
    r = 5 * s
    on = (round(a[0] - 4 * s, 4), round(a[1] + 3 * s, 4))
    d = rp()
    case("onpoint", [pt("A", *a), num("r", r), "k = Circle(A, r)", pt("C", a[0] + 0.5, a[1]),
                     pt("D", *d), "f = Line(C, D)"] + LC,
         [mv("C", *on), mv("D", *rp()), mv("D", a[0], a[1] - 0.5), mv("C", on[0] - 1, on[1]),
          mv("C", *on)])

# ------------------------- line x circle: both defining points on the circle
topic("both")
# E is an intersection point of k, B is the point k was drawn through.
for line in ("f = Line(B, E)", "f = Line(E, B)", "f = Line(B, E)", "f = Line(E, B)"):
    a, b = rp(-2, 2), rp()
    c, d = rp(), rp()
    case("both", [pt("A", *a), pt("B", *b), pt("C", a[0] - 9, a[1] + 0.5),
                  pt("D", a[0] + 9, a[1] - 0.5), "k = Circle(A, B)", "h = Line(C, D)",
                  "E = Intersect(h, k, 1)", line] + LC,
         [mv("C", *c), mv("D", d[0], d[1]), mv("B", *rp()), mv("A", *rp()),
          mv("C", a[0] - 9, a[1] + 0.5), mv("D", a[0] + 9, a[1] - 0.5)])

# The circle passes through an intersection point S of another circle, and
# the line is drawn through S.
for line in ("f = Line(S, C)", "f = Line(C, S)", "f = Line(S, C)"):
    a, m, c = rp(-2, 2), rp(-2, 2), rp()
    case("both", [pt("A", *a), num("r", round(rng.uniform(3, 6), 2)), "j = Circle(A, r)",
                  pt("G", a[0] - 9, a[1] + 1), pt("H", a[0] + 9, a[1] + 1.5), "h = Line(G, H)",
                  "S = Intersect(h, j, 2)", pt("M", *m), "k = Circle(M, S)", pt("C", *c), line] + LC,
         [mv("C", *rp()), mv("M", *rp()), mv("G", a[0] - 9, a[1] - 1), mv("C", *m),
          mv("A", *rp())])

# The line passes through an intersection point of a circle given by a radius.
for _ in range(3):
    a, c = rp(-2, 2), rp()
    case("both", [pt("A", *a), num("r", round(rng.uniform(3, 6), 2)), "k = Circle(A, r)",
                  pt("G", a[0] - 9, a[1] + 1), pt("H", a[0] + 9, a[1] + 1.5), "h = Line(G, H)",
                  "S = Intersect(h, k, 1)", pt("C", *c), "f = Line(C, S)"] + LC,
         [mv("C", *rp()), mv("H", a[0] + 9, a[1] - 2), mv("C", *a), setn("r", 2.5),
          mv("A", *rp())])

# -------------------------------------- line x circle: direction and centre
topic("order")
# The line through the centre turns, and C and D swap places.
for _ in range(4):
    a = rp(-3, 3)
    r = round(rng.uniform(1, 5), 2)
    c, d = rp(), rp()
    case("order", [pt("A", *a), num("r", r), "k = Circle(A, r)", pt("C", *c), pt("D", *a),
                   "f = Line(C, D)"] + LC,
         [mv("C", 2 * a[0] - c[0], 2 * a[1] - c[1]), mv("C", a[0] + 3, a[1]),
          mv("C", a[0] - 3, a[1]), mv("C", a[0], a[1] + 3), mv("C", a[0], a[1] - 3),
          mv("D", *c), mv("C", *a)])

# Line(A, C) with A the centre of k = Circle(A, B): A is not on the circle.
for _ in range(3):
    a, b, c = rp(-3, 3), rp(), rp()
    case("order", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Circle(A, B)",
                   "f = Line(A, C)"] + LC,
         [mv("C", 2 * a[0] - c[0], 2 * a[1] - c[1]), mv("B", *rp()), mv("C", *b),
          mv("A", *rp())])

# --------------------------------------------- circle x circle: tangency
topic("cctangent")
# The second centre moves onto external and internal tangency, through it and
# back, along the axes and along a 3-4-5 direction.
for direction in ((1, 0), (0, 1), (-1, 0), (0, -1), (0.6, 0.8), (-0.8, 0.6), (0.6, -0.8),
                  (-0.6, -0.8)):
    a = rp(-3, 3)
    r1 = round(rng.uniform(2.5, 4), 1)
    r2_ = round(rng.uniform(0.5, 2), 1)
    ext, inn = r1 + r2_, r1 - r2_
    mid = (ext + inn) / 2

    def at(dist):
        return (round(a[0] + dist * direction[0], 4), round(a[1] + dist * direction[1], 4))

    case("cctangent", [pt("A", *a), pt("B", *at(mid)), num("r", r1), num("q", r2_),
                       "k = Circle(A, r)", "c = Circle(B, q)"] + CC,
         [mv("B", *at(ext)), mv("B", *at(ext + 0.5)), mv("B", *at(ext)), mv("B", *at(mid)),
          mv("B", *at(inn)), mv("B", *at(inn - 0.25)), mv("B", *at(inn)), mv("B", *at(mid))])

# --------------------------------------------- circle x circle: radii
topic("ccradius")
for _ in range(4):
    a = rp(-3, 3)
    dist = round(rng.uniform(2, 4), 1)
    r1 = round(rng.uniform(1, dist - 0.5), 1)
    r2_ = round(dist - r1 + rng.uniform(0.5, 1.5), 1)
    case("ccradius", [pt("A", *a), pt("B", a[0] + dist, a[1]), num("r", r1), num("q", r2_),
                      "k = Circle(A, r)", "c = Circle(B, q)"] + CC,
         [setn("q", round(dist - r1, 1)), setn("q", round(dist - r1 - 0.1, 1)), setn("q", r2_),
          setn("q", round(dist + r1, 1)), setn("q", r2_), setn("r", 0), setn("r", 1e-9),
          setn("r", -1e-9), setn("r", -1), setn("r", r1), setn("q", 0), setn("q", r2_)])

# ------------------------------- circle x circle: identical and concentric
topic("ccsame")
# Both circles pass through B by construction: B is intersection 1.
for _ in range(4):
    a, b, c = rp(-3, 3), rp(), rp(-3, 3)
    case("ccsame", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Circle(A, B)",
                    "c = Circle(C, B)"] + CC,
         [mv("C", *a), mv("C", *rp()), mv("B", *rp()), mv("A", *c),
          mv("C", 2 * c[0] - a[0], 2 * c[1] - a[1]), mv("C", (a[0] + c[0]) / 2, (a[1] + c[1]) / 2)])

# Two circles given by a radius become identical, then concentric.
for _ in range(3):
    a, b = rp(-3, 3), rp(-3, 3)
    r1 = round(rng.uniform(2, 4), 1)
    case("ccsame", [pt("A", *a), pt("B", a[0] + 1.5, a[1] + 0.5), num("r", r1), num("q", r1),
                    "k = Circle(A, r)", "c = Circle(B, q)"] + CC,
         [mv("B", *a), setn("q", r1 + 1), mv("B", a[0] + 0.5, a[1]), setn("q", r1),
          mv("B", *b), mv("A", *b), mv("A", b[0] - 1, b[1])])

# k drawn through C, c drawn through the intersection point S of k and a line.
for _ in range(3):
    a, m, c = rp(-2, 2), rp(-2, 2), rp()
    case("ccsame", [pt("A", *a), pt("B", a[0] + 4, a[1]), "k = Circle(A, B)",
                    pt("G", a[0] - 9, a[1] + 1), pt("H", a[0] + 9, a[1] + 1.5), "h = Line(G, H)",
                    "S = Intersect(h, k, 1)", pt("M", *m), "c = Circle(M, S)"] + CC,
         [mv("M", *rp()), mv("M", *a), mv("G", a[0] - 9, a[1] - 1), mv("M", *rp()),
          mv("B", a[0], a[1] + 5)])

# --------------------------------------- circle x circle: order of points
topic("ccorder")
# The second centre goes round the first one, so the line of centres turns.
for _ in range(4):
    a = rp(-3, 3)
    r1 = round(rng.uniform(2, 4), 1)
    r2_ = round(rng.uniform(1.5, 3), 1)
    dist = round(rng.uniform(abs(r1 - r2_) + 0.5, r1 + r2_ - 0.5), 1)
    steps = []
    for k in range(1, 9):
        ang = k * math.pi / 4
        steps.append(mv("B", a[0] + dist * math.cos(ang), a[1] + dist * math.sin(ang)))
    case("ccorder", [pt("A", *a), pt("B", a[0] + dist, a[1]), num("r", r1), num("q", r2_),
                     "k = Circle(A, r)", "c = Circle(B, q)"] + CC, steps)

# The circles are given in the other order.
for _ in range(2):
    a = rp(-3, 3)
    case("ccorder", [pt("A", *a), pt("B", a[0] + 3, a[1] + 1), num("r", 2.5), num("q", 2),
                     "k = Circle(A, r)", "c = Circle(B, q)",
                     "P = Intersect(c, k, 1)", "Q = Intersect(c, k, 2)"],
         [mv("B", a[0] - 3, a[1] - 1), mv("B", a[0] + 4.5, a[1]), mv("A", *rp())])

# ------------------------- line x circle: sliding along an unchanged line
topic("slide")
# The line reaches tangency along an axis, then its points slide along it:
# the line stays the same, but GeoGebra's coefficients scale with the
# distance between C and D.
for vertical in (False, False, False, True, True):
    a = rp(-4, 4)
    r = round(rng.uniform(1, 5), 2)
    if vertical:
        x0, t = round(a[0] + 0.5 * r, 2), round(a[0] + r, 2)
        script = [pt("C", x0, a[1] - 6), pt("D", x0, a[1] + 6)]
        steps = [mv("C", t, a[1] - 6), mv("D", t, a[1] + 6)]
        for lo, hi in ((-1, 1), (-25, 3), (-0.125, 40), (2, 2.5), (-7.3, 0.6)):
            steps += [mv("C", t, a[1] + lo), mv("D", t, a[1] + hi)]
        steps += [mv("D", t, a[1] - 9)]
    else:
        y0, t = round(a[1] + 0.5 * r, 2), round(a[1] + r, 2)
        script = [pt("C", a[0] - 6, y0), pt("D", a[0] + 6, y0)]
        steps = [mv("C", a[0] - 6, t), mv("D", a[0] + 6, t)]
        for lo, hi in ((-1, 1), (-25, 3), (-0.125, 40), (2, 2.5), (-7.3, 0.6)):
            steps += [mv("C", a[0] + lo, t), mv("D", a[0] + hi, t)]
        steps += [mv("D", a[0] - 9, t)]
    case("slide", [pt("A", *a), num("r", r), "k = Circle(A, r)"] + script
         + ["f = Line(C, D)"] + LC, steps)

# The radius makes the line tangent, then the points slide along the line.
for _ in range(3):
    a = rp(-3, 3)
    off = round(rng.uniform(0.5, 2), 2)
    r = round(off + rng.uniform(0.5, 2), 2)
    case("slide", [pt("A", *a), num("r", r), "k = Circle(A, r)",
                   pt("C", a[0] - 5, a[1] + off), pt("D", a[0] + 5, a[1] + off),
                   "f = Line(C, D)"] + LC,
         [setn("r", off), mv("C", a[0] - 0.5, a[1] + off), mv("D", a[0] + 30, a[1] + off),
          mv("C", a[0] + 3, a[1] + off), mv("D", a[0] - 3, a[1] + off), setn("r", r)])

cases.write()
