#!/usr/bin/env python3
"""Writes cases/intersect.json: line-circle and circle-circle intersections.

The cases drive the intersections through the states where GeoGebra has
special rules: tangency in both directions, a point that defines the line and
lies on the circle by construction, circles through a common point that become
identical, concentric circles, zero and tiny radii, and the order of the two
points when the line or the line of centres changes direction.

Every object must be defined when it is created, so most cases start with two
distinct intersection points; the "history" topic starts with one. Snapshots come exclusively from running GeoGebra through the
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


# ------------------------------ three-point circles: points on the circle
topic("three")
# Circle(A, B, C) lies on its three points by construction
# (AlgoCircleThreePoints.setIncidence), in the order A, B, C.
TRI = [pt("A", 3, 4), pt("B", -3, 4), pt("C", 0, -5), "k = Circle(A, B, C)"]
for line, steps in (
        ("f = Line(D, A)", [mv("D", -2, -6), mv("A", 5, 0), mv("A", 4, 3), mv("D", 0, 0),
                            mv("A", 6, 1), mv("D", 3, -4)]),
        ("f = Line(A, B)", [mv("C", 0, -7), mv("A", 4, -3), mv("B", -5, 0), mv("A", 3, 4)]),
        ("f = Line(B, A)", [mv("C", 1, -6), mv("A", 0, 5), mv("A", -4, -3), mv("B", 2, 1)]),
        ("f = Line(C, B)", [mv("A", 5, 0), mv("C", 3, -4), mv("B", -4, 3), mv("D", 1, 1)]),
        ("f = Line(D, C)", [mv("D", 1, 7), mv("C", -5, 0), mv("D", -9, 1), mv("A", 0, 5)])):
    case("three", TRI + [pt("D", 3, -4), line] + LC, steps)

# A perpendicular through a vertex, and a line through an intersection point.
case("three", TRI + [pt("E", -6, -1), pt("F", 6, 1), "g = Line(E, F)",
                     "f = PerpendicularLine(C, g)"] + LC,
     [mv("F", 6, -2), mv("C", 3, -4), mv("E", 1, 7), mv("C", 0, -5)])
case("three", TRI + [pt("E", -6, 1), pt("F", 6, 2), "g = Line(E, F)",
                     "S = Intersect(g, k, 1)", pt("D", 1, -7), "f = Line(D, S)"] + LC,
     [mv("D", -1, -8), mv("F", 6, -1), mv("A", 4, -3), mv("D", 5, 5)])

# Circle and circle: a vertex is the point the other circle was drawn through,
# or a vertex of the other three-point circle.
for other, cmd, steps in (
        ("c = Circle(M, A)", CC, [mv("M", 7, 2), mv("A", 5, 0), mv("M", 2, 9), mv("A", 0, 5)]),
        ("c = Circle(M, B)", ["P = Intersect(c, k, 1)", "Q = Intersect(c, k, 2)"],
         [mv("M", -7, 1), mv("B", -5, 0), mv("M", -1, 8), mv("C", 3, -4)]),
        ("c = Circle(A, M, E)", CC, [mv("M", 9, 1), mv("A", 4, -3), mv("E", 6, 9), mv("A", 3, 4)]),
        ("c = Circle(B, A, E)", CC, [mv("E", 1, 9), mv("A", 5, 0), mv("B", -4, 3), mv("E", 0, 12)]),
        ("c = Circle(B, A, E)", ["P = Intersect(c, k, 1)", "Q = Intersect(c, k, 2)"],
         [mv("E", 1, 9), mv("A", 5, 0), mv("B", -4, 3), mv("E", 0, 12)])):
    case("three", TRI + [pt("M", 6, 6), pt("E", 2, 10), other] + cmd, steps)

# ---------------------- circles that pass through a point by geometry only
topic("plain")
# Circle(M, s), Incircle and transformed circles add no incidence, so a point
# that lies on them is not preferred.
case("plain", [pt("M", 1, 1), pt("A", 4, 5), "s = Segment(M, A)", "k = Circle(M, s)",
               pt("D", -2, -3), "f = Line(D, A)"] + LC,
     [mv("D", 7, -1), mv("A", -2, 9), mv("D", 1, 6), mv("M", 0, 0)])
case("plain", [pt("M", 1, 1), pt("A", 4, 5), "s = Segment(M, A)", "k = Circle(M, s)",
               pt("N", 7, 2), "c = Circle(N, A)"] + CC,
     [mv("N", -5, 4), mv("A", 1, 6), mv("M", 3, -2), mv("N", 8, 8)])
case("plain", [pt("M", 1, 1), pt("A", 4, 5), "k = Circle(M, A)", num("t", 1.2),
               "c = Rotate(k, t, A)"] + CC,
     [setn("t", 2.5), mv("M", -1, 2), setn("t", -0.7), mv("A", 2, 6)])
case("plain", [pt("A", 0, 0), pt("B", 8, 0), pt("C", 2, 6), "k = Incircle(A, B, C)",
               "g = Line(A, B)", "D = Intersect(g, k, 1)", pt("E", 3, 7), "f = Line(E, D)"] + LC,
     [mv("E", -2, 6), mv("C", 5, 5), mv("E", 4, -6), mv("B", 9, 2)])

# --------------------- circle x circle: special case decided at creation
topic("created")
# GeoGebra decides once, when the intersection is created, whether the two
# circles through a common point use the special case; a circle of radius 0
# is not a circle then.
for cmd in ("P = Intersect(k, c, 1)", "P = Intersect(c, k, 1)"):
    case("created", [pt("M", 2, 1), pt("A", 2, 1), pt("N", 5, 5), "k = Circle(M, A)",
                     "c = Circle(N, A)", cmd],
         [mv("M", 0, 0), mv("M", 4, -3), mv("A", -1, 2), mv("M", -1, 2), mv("M", 1, 7),
          mv("N", 0, 6)])
    case("created", [pt("M", 2, 1), pt("A", 5, 5), pt("N", 5, 5), "k = Circle(M, A)",
                     "c = Circle(N, A)", cmd],
         [mv("N", 8, 2), mv("N", 3, 9), mv("A", 1, 8), mv("N", 1, 8), mv("N", 6, 6)])

# ------------------------- the same intersection in both argument orders
topic("reuse")
# A second Intersect of the same two objects reuses the first algorithm
# (AlgoDispatcher.findExistingIntersectionAlgorithm).
for first, second in ((("P", "k, c"), ("R", "c, k")), (("P", "c, k"), ("R", "k, c"))):
    script = [pt("A", 0, 0), pt("B", 8, 0), num("r", 5), num("q", 5),
              "k = Circle(A, r)", "c = Circle(B, q)",
              f"{first[0]} = Intersect({first[1]}, 1)", f"{second[0]} = Intersect({second[1]}, 1)",
              f"S = Intersect({first[1]}, 2)", f"T = Intersect({second[1]}, 2)"]
    case("reuse", script, [mv("B", 0, 8), mv("B", -6, 1), setn("q", 3), mv("A", 1, -2),
                           mv("B", 3, 3), setn("r", 9), mv("B", 8, 0)])
case("reuse", [pt("A", 0, 0), pt("B", 8, 0), pt("E", 4, 3), "k = Circle(A, E)",
               "c = Circle(B, E)", "P = Intersect(c, k, 1)", "R = Intersect(k, c, 1)",
               "S = Intersect(k, c, 2)", "T = Intersect(c, k, 2)"],
     [mv("E", 4, -3), mv("B", 0, 6), mv("E", 1, 1), mv("A", -3, 2)])
case("reuse", [pt("A", 0, 0), num("r", 5), "k = Circle(A, r)", pt("C", -6, 1),
               pt("D", 6, 2), "f = Line(C, D)", "P = Intersect(f, k, 1)",
               "R = Intersect(k, f, 1)", "S = Intersect(k, f, 2)", "T = Intersect(f, k, 2)"],
     [mv("D", -6, -2), mv("C", 6, 1), setn("r", 2), mv("A", 1, 1)])

# ------------------------------- intersections created with one point
topic("history")
# An intersection created tangent has one point; the second appears later.
# A point that is undefined at creation gets no label, so only the first
# intersection can be observed.
case("history", [pt("A", 0, 0), num("r", 3), "k = Circle(A, r)", pt("C", -5, 3),
                 pt("D", 5, 3), "f = Line(C, D)", "P = Intersect(f, k, 1)"],
     [mv("C", -5, 1), mv("D", 5, 1), mv("C", 10, 1), mv("C", -5, 1), mv("D", -10, 1),
      mv("D", 5, 1), mv("C", -5, 3), mv("D", 5, 3), mv("D", 5, 1)])
case("history", [pt("A", 0, 0), num("r", 3), "k = Circle(A, r)", pt("C", -5, 3),
                 pt("D", 5, 3), "f = Line(C, D)", "P = Intersect(f, k, 1)"],
     [mv("C", -5, 1), mv("C", 10, 1), mv("D", 5, -1), mv("C", -5, 1), mv("D", 5, 1)])
case("history", [pt("A", 0, 0), num("r", 3), "k = Circle(A, r)", pt("C", -5, 3),
                 pt("D", 5, 3), "f = Line(C, D)", "P = Intersect(f, k, 1)"],
     [setn("r", 4), mv("C", 10, 3), setn("r", 3), setn("r", 5), mv("C", -5, 3)])
case("history", [pt("A", 0, 0), num("r", 3), num("q", 2), "k = Circle(A, r)", pt("B", 5, 0),
                 "c = Circle(B, q)", "P = Intersect(k, c, 1)"],
     [setn("q", 3), mv("B", -5, 0), mv("B", 0, 4), mv("B", 5, 0), setn("q", 2), setn("q", 2.5)])
case("history", [pt("A", 0, 0), num("r", 3), num("q", 2), "k = Circle(A, r)", pt("B", 5, 0),
                 "c = Circle(B, q)", "P = Intersect(k, c, 1)"],
     [mv("B", 4, 0), mv("B", -4, 0), mv("B", 0, -4), mv("B", 4, 0)])
# Created with two points, the first move reverses the line.
case("history", [pt("A", 0, 0), num("r", 3), "k = Circle(A, r)", pt("C", -5, 1),
                 pt("D", 5, 1), "f = Line(C, D)"] + LC,
     [mv("C", 10, 1), mv("D", 5, 0), mv("C", -5, 1), mv("D", 5, 1)])
case("history", [pt("A", 0, 0), num("r", 3), num("q", 3), "k = Circle(A, r)", pt("B", 4, 0),
                 "c = Circle(B, q)"] + CC,
     [mv("B", -4, 0), mv("B", 0, 4), mv("B", 4, 0)])

# ---------------- parallel and perpendicular lines of a long or short line
topic("rawline")
# GeoGebra's coefficients of these lines scale with the line they are built
# on, and its tangency window depends on the size of the coefficients. The
# circle is moved to within 1e-7 of the line from both sides.
NEAR = [setn("r", 2.9999999), setn("r", 2.99999), setn("r", 3.0000001), setn("r", 2.99999999),
        setn("r", 3)]
for c0, d0, cmd in (((-400, 0), (400, 0), "f = Line(E, g)"),
                    ((0, -400), (0, 400), "f = PerpendicularLine(E, g)"),
                    ((-0.002, 0), (0.002, 0), "f = Line(E, g)"),
                    ((-400, 7), (400, 7), "f = Line(E, g)")):
    case("rawline", [pt("C", *c0), pt("D", *d0), "g = Line(C, D)", pt("E", 0, 1),
                     pt("A", 0, 3), num("r", 3), "k = Circle(A, r)", cmd] + LC,
         [mv("E", 0, 0)] + NEAR + [mv("E", 0, 1)])
# A slanted perpendicular: its coefficients are rescaled by powers of 2.
case("rawline", [pt("C", -300, -400), pt("D", 300, 400), "g = Line(C, D)", pt("E", 0.6, 0.8),
                 pt("A", 1.8, 2.4), num("r", 3), "k = Circle(A, r)",
                 "f = PerpendicularLine(E, g)"] + LC,
     [mv("E", 0, 0)] + NEAR + [mv("E", 0.6, 0.8)])
# The long line itself, whose coefficients the kernel rebuilds.
case("rawline", [pt("C", -400, 0), pt("D", 400, 0), "f = Line(C, D)", pt("A", 0, 4),
                 num("r", 3), "k = Circle(A, r)"] + LC,
     [setn("r", 4)] + [setn("r", v + 1) for v in (2.9999999, 2.99999, 3.0000001, 2.99999999)]
     + [setn("r", 3)])

# ------------------------------------- lines defined as tangents of the circle
topic("deftangent")
# AlgoIntersectLineConic takes the point of a tangent line of the same conic
# from the tangent algorithm (isDefinedAsTangent) and has only one point.
TK = [pt("A", 0, 0), num("r", 3), "k = Circle(A, r)"]
TMOVES = [mv("P", 0, 6), mv("P", -5, -2), mv("P", 3, 0), mv("P", 1, 0.5), mv("P", 6, 2),
          setn("r", 1), setn("r", 0), setn("r", 4), mv("P", 2.4, 3.2), mv("P", 5, 1), setn("r", 3)]
case("deftangent", TK + [pt("P", 5, 1), "t = Tangent(P, k)", "X = Intersect(t_{1}, k, 1)",
                         "Y = Intersect(t_{2}, k, 1)", "Z = Intersect(k, t_{2}, 1)"], TMOVES)
case("deftangent", TK + [pt("P", 5, 1), "t = Tangent(P, k)", "X = Intersect(t_{1}, k, 2)"],
     TMOVES)
case("deftangent", TK + [pt("P", 5, 1), "t = Tangent(P, k)", "X = Intersect(t_{2}, k, 2)",
                         "Y = Intersect(t_{1}, k)"], TMOVES)
# The point starts on the circle: one tangent `t`.
case("deftangent", TK + [pt("P", 3, 0), "t = Tangent(P, k)", "X = Intersect(t, k, 1)"],
     [mv("P", 0, 3), mv("P", 5, 1), mv("P", 0, -3), mv("P", 1, 1), setn("r", 5), mv("P", 3, 4)])
# Tangents parallel to a line.
case("deftangent", TK + [pt("B", -4, -1), pt("C", 4, 2), "g = Line(B, C)", "t = Tangent(g, k)",
                         "X = Intersect(t_{1}, k, 1)", "Y = Intersect(k, t_{2}, 1)",
                         "Z = Intersect(t_{2}, k, 2)"],
     [mv("C", 4, -3), mv("B", 1, 5), mv("A", 2, 1), setn("r", 0), setn("r", 2), mv("C", -4, 7)])
# A tangent of another circle is an ordinary line for this circle.
case("deftangent", TK + [pt("P", 5, 1), "t = Tangent(P, k)", pt("M", 4, 3), num("q", 2.5),
                         "c = Circle(M, q)", "X = Intersect(t_{1}, c, 1)",
                         "Y = Intersect(t_{1}, c, 2)"],
     [mv("P", 6, 3), mv("M", 2, 4), setn("q", 4), mv("P", 3, 0), mv("P", 0, 5)])
# The polar of a point is not a tangent algorithm.
case("deftangent", TK + [pt("P", 1, 1), "p = Polar(P, k)", "X = Intersect(p, k, 1)",
                         "Y = Intersect(p, k, 2)"],
     [mv("P", 2, -1), mv("P", 3, 0), mv("P", 5, 0), mv("P", 0.5, 0.25), setn("r", 1)])

# Large and odd scales, where the rounding of the tangent line can make the
# line cut the circle in two close points.
for a, r, p0, moves in (((0, 0), 100000, (300000, 123457), [(-250001, 77777), (0, 1000001), (99999.5, -3000)]),
                        ((12345.678, -9876.543), 777.777, (13500.1, -9000.3), [(11000.7, -9100.9), (12345.678, -9000), (14000.123, -11000.456)]),
                        ((0.3, 0.7), 0.0123, (0.35, 0.69), [(0.31, 0.75), (0.2, 0.6), (0.3123, 0.7)]),
                        ((-7.1, 3.3), 2.9, (1000000.5, 999999.25), [(-1000000, 3.3), (5, -999999), (-7.1, 1234567)])):
    case("deftangent", [pt("A", *a), num("r", r), "k = Circle(A, r)", pt("P", *p0),
                        "t = Tangent(P, k)", "X = Intersect(t_{1}, k, 1)", "Y = Intersect(t_{2}, k, 1)",
                        "U = Intersect(t_{1}, k, 2)", "V = Intersect(k, t_{2}, 2)"],
         [mv("P", *m) for m in moves])
    case("deftangent", [pt("A", *a), num("r", r), "k = Circle(A, r)", pt("B", *p0), pt("C", a[0] + 3 * r, a[1] - 7 * r),
                        "g = Line(B, C)", "t = Tangent(g, k)", "X = Intersect(t_{1}, k, 1)",
                        "Y = Intersect(t_{2}, k, 1)", "U = Intersect(t_{1}, k, 2)", "V = Intersect(t_{2}, k, 2)"],
         [mv("B", *m) for m in moves])

# --------------------------- lines and circles from the other groups
topic("groups")
# Lines built by the lines and transform groups, with a circle.
GK = [pt("A", 0, 0), num("r", 4), "k = Circle(A, r)", pt("B", -5, 1), pt("C", 3, 2),
      pt("D", 1, -3), num("t", 0.7), num("s", 1.5)]
GMOVES = [mv("B", -3, 4), mv("C", 6, -1), mv("D", -2, -2), setn("t", 2.1), setn("s", -0.5),
          setn("r", 2), mv("A", 1, 1)]
for line in ("f = LineBisector(B, C)", "f = AngularBisector(B, D, C)", "g = Line(B, C)\nf = Reflect(g, D)",
             "g = Line(B, C)\nf = Rotate(g, t, D)", "g = Line(B, C)\nf = Dilate(g, s, D)",
             "f = Polar(B, k)", "g = Line(B, C)\nh = Line(C, D)\nf = AngularBisector(g, h)"):
    script = GK + line.split("\n") + LC
    if "AngularBisector(g, h)" in line:
        script = GK + line.split("\n")[:2] + ["f = AngularBisector(g, h)"] + [
            "P = Intersect(f_{1}, k, 1)", "Q = Intersect(f_{2}, k, 2)"]
    case("groups", script, GMOVES)

# Circles built by the conic and transform groups, with a line and a circle.
GC = [pt("A", 0, 0), pt("B", 4, 1), pt("C", 1, 5), pt("D", -3, -1), pt("E", 6, -2),
      num("t", 0.9), num("s", 1.25)]
CMOVES = [mv("D", -1, -4), mv("B", 5, 3), mv("E", 2, 6), setn("t", 2.4), setn("s", 0.6),
          mv("C", -2, 4), setn("s", -1.5)]
for circle in ("k = Circle(A, B, C)", "k = Incircle(A, B, C)", "u = Segment(A, B)\nk = Circle(C, u)",
               "m = Circle(A, B)\nk = Reflect(m, D)", "m = Circle(A, B)\nl = Line(C, D)\nk = Reflect(m, l)",
               "m = Circle(A, B)\nk = Rotate(m, t, D)", "m = Circle(A, B)\nk = Dilate(m, s, D)"):
    cl = circle.split("\n")
    case("groups", GC + cl + ["f = Line(D, E)"] + LC, CMOVES)
    case("groups", GC + cl + ["c = Circle(E, D)"] + CC, CMOVES)

cases.write()
