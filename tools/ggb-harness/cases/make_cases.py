#!/usr/bin/env python3
"""Writes cases/geometry.json: the case inputs (id, group, script, steps).

The helpers live in caselib.py; every other group has its own
make_<group>.py next to this file.

The inputs only describe what to run. Snapshots come exclusively from running
GeoGebra through the harness (`harness.sh golden`). The output is
deterministic: a fixed seed and a fixed order, so case ids stay stable as long
as existing topics are only appended to.

GeoGebra gives an object that is undefined when it is created no label until
it becomes defined, so a script never refers to an object that starts out
undefined (such objects are always leaves).
"""

import math

from caselib import Cases, mv, num, pt, setn

cases = Cases("geometry")
rng = cases.rng
case = cases.case
topic = cases.topic
r2 = cases.r2
rp = cases.rp


# ------------------------------------------------------------------ basic
topic("basic")
for _ in range(10):
    a, b = rp(), rp()
    script = [pt("A", *a), pt("B", *b), "M = Midpoint(A, B)", "d = Distance(A, B)",
              "s = Segment(A, B)"]
    steps = [mv("A", *rp()), mv("B", *rp()), mv("B", a[0], a[1])]
    steps.append(mv("A", a[0], a[1]))
    steps.append(mv("B", *rp()))
    case("basic", script, steps)

for _ in range(5):
    a, b, c = rp(), rp(), rp()
    case("basic", [pt("A", *a), pt("B", *b), pt("C", *c), "M = Midpoint(A, B)",
                   "N = Midpoint(M, C)", "d = Distance(M, C)", "t = Distance(N, A)",
                   "s = Segment(N, M)"],
         [mv("C", *rp()), mv("A", *b), mv("B", *c), mv("C", *rp())])

for _ in range(5):
    a, b = rp(), rp()
    case("basic", [pt("A", *a), pt("B", *b), "s = Segment(B, A)", "d = Distance(B, A)",
                   "M = Midpoint(B, A)"],
         [mv("A", *rp()), mv("B", *rp()), mv("A", *rp(-0.5, 0.5))])

for scale in (1000.0, 0.001, 123.456, 0.37, 54321.0):
    a = (round(rng.uniform(-1, 1) * scale, 6), round(rng.uniform(-1, 1) * scale, 6))
    b = (round(rng.uniform(-1, 1) * scale, 6), round(rng.uniform(-1, 1) * scale, 6))
    case("basic", [pt("A", *a), pt("B", *b), "M = Midpoint(A, B)", "d = Distance(A, B)",
                   "s = Segment(A, B)", "f = Line(A, B)"],
         [mv("B", -b[0], b[1]), mv("A", b[0] * 0.5, -a[1])])

# coincident points at creation (Line(A, B) starts undefined, so it is a leaf)
for _ in range(5):
    a = rp()
    case("basic", [pt("A", *a), pt("B", *a), "M = Midpoint(A, B)", "d = Distance(A, B)",
                   "s = Segment(A, B)", "k = Circle(A, B)", "f = Line(A, B)"],
         [mv("B", *rp()), mv("A", *rp()), mv("A", *a), mv("B", *a)])

# ------------------------------------------------------------------- line
topic("line")
for _ in range(12):
    a, b = rp(), rp()
    steps = [mv("B", a[0], b[1]),          # horizontal
             mv("B", a[0], a[1] + 3.25),   # vertical
             mv("B", a[0], a[1]),          # coincident with A
             mv("B", *rp()),               # back
             mv("A", *rp())]
    case("line", [pt("A", *a), pt("B", *b), "f = Line(A, B)"], steps)

for _ in range(4):
    a, b = rp(), rp()
    case("line", [pt("A", *a), pt("B", *b), "f = Line(A, B)", "g = Line(B, A)"],
         [mv("A", *rp()), mv("B", *rp()), mv("A", -2.5, 0), mv("B", 2.5, 0)])

for _ in range(4):
    a, b = rp(), rp()
    case("line", [pt("A", *a), pt("B", *b), "M = Midpoint(A, B)", "f = Line(A, M)",
                  "g = Line(M, B)"],
         [mv("B", *rp()), mv("B", a[0], a[1]), mv("B", *rp())])

# ------------------------------------------------------------ perpendicular
topic("perp")
for kind in ("line",) * 8 + ("segment",) * 6:
    a, b, c = rp(), rp(), rp()
    base = "f = Line(A, B)" if kind == "line" else "f = Segment(A, B)"
    t = round(rng.uniform(-1, 2), 2)
    on = (a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]))
    case("perp", [pt("A", *a), pt("B", *b), pt("C", *c), base,
                  "n = PerpendicularLine(C, f)"],
         [mv("C", *on), mv("C", *rp()), mv("B", a[0], a[1]), mv("B", *rp()),
          mv("A", *rp())])

for _ in range(3):
    a, b = rp(), rp()
    case("perp", [pt("A", *a), pt("B", *b), "f = Line(A, B)", "n = PerpendicularLine(A, f)",
                  "m = PerpendicularLine(B, f)"],
         [mv("A", *rp()), mv("B", *rp()), mv("A", 0, 0), mv("B", 0, 4.5)])

for _ in range(3):
    a, b, c, d = rp(), rp(), rp(), rp()
    case("perp", [pt("A", *a), pt("B", *b), pt("C", *c), pt("D", *d), "f = Line(A, B)",
                  "n = PerpendicularLine(C, f)", "m = PerpendicularLine(D, n)"],
         [mv("D", *c), mv("C", *rp()), mv("B", *rp())])

# --------------------------------------------------------------- parallel
topic("parallel")
for kind in ("line",) * 8 + ("segment",) * 6:
    a, b, c = rp(), rp(), rp()
    base = "f = Line(A, B)" if kind == "line" else "f = Segment(A, B)"
    t = round(rng.uniform(-1, 2), 2)
    on = (a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]))
    case("parallel", [pt("A", *a), pt("B", *b), pt("C", *c), base, "q = Line(C, f)"],
         [mv("C", *on), mv("C", *rp()), mv("B", a[0], a[1]), mv("B", *rp()),
          mv("A", *rp())])

for _ in range(3):
    a, b = rp(), rp()
    case("parallel", [pt("A", *a), pt("B", *b), "f = Line(A, B)", "q = Line(A, f)",
                      "p = Line(B, f)"],
         [mv("A", *rp()), mv("B", *rp()), mv("B", a[0], a[1])])

for _ in range(3):
    a, b, c, d = rp(), rp(), rp(), rp()
    case("parallel", [pt("A", *a), pt("B", *b), pt("C", *c), pt("D", *d), "f = Line(A, B)",
                      "q = Line(C, f)", "p = Line(D, q)", "n = PerpendicularLine(D, q)"],
         [mv("D", *c), mv("C", *rp()), mv("A", *rp())])

# ----------------------------------------------------------------- circle
topic("circle")
for _ in range(8):
    a, b = rp(), rp()
    case("circle", [pt("A", *a), pt("B", *b), "k = Circle(A, B)"],
         [mv("B", *rp()), mv("B", a[0], a[1]), mv("A", *rp()), mv("B", *rp())])

for _ in range(8):
    a = rp()
    r = round(rng.uniform(0.5, 6), 2)
    case("circle", [pt("A", *a), num("r", r), "k = Circle(A, r)"],
         [setn("r", round(rng.uniform(0.5, 6), 3)), setn("r", 0), setn("r", -1.5),
          setn("r", 1e-9), setn("r", -1e-9), setn("r", 2.75), mv("A", *rp())])

for _ in range(5):
    a, b, c = rp(), rp(), rp()
    case("circle", [pt("A", *a), pt("B", *b), pt("C", *c), "d = Distance(B, C)",
                    "k = Circle(A, d)"],
         [mv("C", *rp()), mv("C", *b), mv("B", *rp())])

for _ in range(4):
    a, b, c = rp(), rp(), rp()
    case("circle", [pt("A", *a), pt("B", *b), pt("C", *c), "M = Midpoint(A, B)",
                    "k = Circle(M, C)", "h = Circle(C, M)"],
         [mv("C", *rp()), mv("A", *rp()), mv("C", (a[0] + b[0]) / 2, (a[1] + b[1]) / 2)])

# zero and negative radius at creation
for r in (0, 0, -1.5, -0.25, -3):
    a = rp()
    case("circle", [pt("A", *a), num("r", r), "k = Circle(A, r)"],
         [setn("r", 2.5), setn("r", 0), setn("r", -r if r else 1), mv("A", *rp())])

# ---------------------------------------------------------- line x line
topic("ixll")
for _ in range(10):
    a, b, c, d = rp(), rp(), rp(), rp()
    dx, dy = b[0] - a[0], b[1] - a[1]
    case("ixll", [pt("A", *a), pt("B", *b), pt("C", *c), pt("D", *d), "f = Line(A, B)",
                  "g = Line(C, D)", "S = Intersect(f, g)"],
         [mv("D", c[0] + 2 * dx, c[1] + 2 * dy),        # parallel
          mv("D", *rp()),                               # crossing again
          mv("C", a[0] - dx, a[1] - dy), mv("D", b[0] + dx, b[1] + dy),  # coincident
          mv("A", *rp())])

for _ in range(4):
    a = rp()
    w, h = round(rng.uniform(1, 6), 2), round(rng.uniform(0.5, 4), 2)
    case("ixll", [pt("A", *a), pt("B", a[0] + w, a[1]), pt("C", a[0], a[1] + h),
                  pt("D", a[0] + w, a[1] + h), "f = Line(A, B)", "g = Line(C, D)",
                  "S = Intersect(f, g)"],
         [mv("D", a[0] + w, a[1] + h + 1.5), mv("D", a[0] + w, a[1] + h)])

for _ in range(4):
    a, b, c = rp(), rp(), rp()
    case("ixll", [pt("A", *a), pt("B", *b), pt("C", *c), "f = Line(A, B)",
                  "n = PerpendicularLine(C, f)", "F = Intersect(f, n)", "d = Distance(C, F)"],
         [mv("C", *rp()), mv("B", *rp()), mv("C", *a)])

for _ in range(3):
    a, b, c = rp(), rp(), rp()
    case("ixll", [pt("A", *a), pt("B", *b), pt("C", *c), "f = Line(A, B)", "q = Line(C, f)",
                  "S = Intersect(f, q)"],
         [mv("C", *rp()), mv("C", *a)])

for _ in range(4):
    a, b, c, d = rp(), rp(), rp(), rp()
    case("ixll", [pt("A", *a), pt("B", *b), pt("C", *c), pt("D", *d), "s = Segment(A, B)",
                  "t = Segment(C, D)", "n = PerpendicularLine(C, s)",
                  "m = PerpendicularLine(A, t)", "S = Intersect(n, m)"],
         [mv("D", *rp()), mv("B", *rp()), mv("A", *c)])

# -------------------------------------------------------- line x circle
topic("ixlc")
for _ in range(12):
    a = rp(-4, 4)
    r = round(rng.uniform(1, 5), 2)
    cy = a[1] + round(rng.uniform(-0.9, 0.9) * r, 2)
    case("ixlc", [pt("A", *a), num("r", r), "k = Circle(A, r)",
                  pt("C", a[0] - 6, cy), pt("D", a[0] + 6, cy + r2(-1, 1)), "f = Line(C, D)",
                  "P = Intersect(f, k, 1)", "Q = Intersect(f, k, 2)"],
         [mv("C", a[0] - 6, a[1] + r), mv("D", a[0] + 6, a[1] + r),   # tangent
          mv("D", a[0] + 6, a[1] + r + 0.5), mv("C", a[0] - 6, a[1] + r + 0.5),  # disjoint
          mv("C", a[0] - 6, a[1]), mv("D", a[0] + 6, a[1] - 0.3),   # back
          mv("C", a[0] + 6, a[1] + 0.3)])                           # reversed direction

for _ in range(4):
    a, b = rp(), rp()
    case("ixlc", [pt("A", *a), pt("B", *b), "k = Circle(A, B)", "f = Line(A, B)",
                  "P = Intersect(f, k, 1)", "Q = Intersect(f, k, 2)"],
         [mv("B", *rp()), mv("A", *rp()), mv("B", a[0], a[1] + 2.5)])

for _ in range(4):
    a = rp(-3, 3)
    r = round(rng.uniform(1, 4), 2)
    x0 = a[0] + round(rng.uniform(-0.8, 0.8) * r, 2)
    case("ixlc", [pt("A", *a), num("r", r), "k = Circle(A, r)", pt("C", x0, a[1] - 7),
                  pt("D", x0, a[1] + 7), "f = Line(C, D)", "P = Intersect(f, k, 1)",
                  "Q = Intersect(f, k, 2)"],
         [mv("C", x0, a[1] + 9), mv("D", a[0] - r, a[1]), mv("C", a[0] - r, a[1] + 1)])

for _ in range(6):
    a = rp(-3, 3)
    r = round(rng.uniform(2, 4), 2)
    off = round(rng.uniform(0.3, 1.8), 2)
    case("ixlc", [pt("A", *a), num("r", r), "k = Circle(A, r)",
                  pt("C", a[0] - 5, a[1] + off), pt("D", a[0] + 5, a[1] + off),
                  "f = Line(C, D)", "P = Intersect(f, k, 1)", "Q = Intersect(f, k, 2)"],
         [setn("r", off), setn("r", off * 0.5), setn("r", 0), setn("r", -1),
          setn("r", off + 1), setn("r", 7.5)])

for _ in range(5):
    a, b = rp(), rp()
    c = rp()
    case("ixlc", [pt("A", *a), pt("B", *b), pt("C", *c), "k = Circle(A, B)", "f = Line(B, C)",
                  "P = Intersect(f, k, 1)", "Q = Intersect(f, k, 2)"],
         [mv("C", *rp()), mv("C", 2 * a[0] - b[0], 2 * a[1] - b[1]), mv("A", *rp()),
          mv("B", *c), mv("B", *rp())])

for _ in range(4):
    a, c, d = rp(-2, 2), rp(), rp()
    r = round(rng.uniform(3, 6), 2)
    case("ixlc", [pt("A", *a), num("r", r), "k = Circle(A, r)", pt("C", *c), pt("D", *d),
                  "f = Line(C, D)", "P = Intersect(f, k, 1)", "Q = Intersect(f, k, 2)"],
         [mv("D", *c), mv("D", *rp()), mv("C", *a)])

# tangent and disjoint at creation (undefined intersections stay leaves)
for gap in (0, 0, 0, 0, 0.75, 0.75, 2.5, 2.5):
    a = rp(-3, 3)
    r = round(rng.uniform(1, 4), 2)
    y0 = a[1] + r + gap
    case("ixlc", [pt("A", *a), num("r", r), "k = Circle(A, r)", pt("C", a[0] - 5, y0),
                  pt("D", a[0] + 5, y0), "f = Line(C, D)", "P = Intersect(f, k, 1)",
                  "Q = Intersect(f, k, 2)"],
         [mv("C", a[0] - 5, a[1]), mv("D", a[0] + 5, a[1] + 0.5),
          mv("C", a[0] - 5, y0), mv("D", a[0] + 5, y0)])

# ------------------------------------------------------ circle x circle
topic("ixcc")
for _ in range(10):
    a = rp(-3, 3)
    ang = rng.uniform(0, 2 * math.pi)
    r1, r2_ = round(rng.uniform(2, 4), 2), round(rng.uniform(1.5, 4), 2)
    dist = round(rng.uniform(abs(r1 - r2_) + 0.3, r1 + r2_ - 0.3), 2)
    b = (round(a[0] + dist * math.cos(ang), 2), round(a[1] + dist * math.sin(ang), 2))
    case("ixcc", [pt("A", *a), pt("B", *b), num("r", r1), num("q", r2_), "k = Circle(A, r)",
                  "c = Circle(B, q)", "P = Intersect(k, c, 1)", "Q = Intersect(k, c, 2)"],
         [mv("B", a[0] + 20, a[1]), mv("B", *b), mv("B", 2 * a[0] - b[0], 2 * a[1] - b[1]),
          mv("A", *rp()), mv("A", *a)])

for sign in (1, 1, 1, 1, -1, -1, -1, -1):
    a = rp(-3, 3)
    r1, r2_ = round(rng.uniform(2.5, 4), 2), round(rng.uniform(0.5, 2), 2)
    dist = r1 + r2_ if sign > 0 else r1 - r2_
    case("ixcc", [pt("A", *a), pt("B", a[0] + dist, a[1]), num("r", r1), num("q", r2_),
                  "k = Circle(A, r)", "c = Circle(B, q)", "P = Intersect(k, c, 1)",
                  "Q = Intersect(k, c, 2)"],
         [mv("B", a[0], a[1] + dist), mv("B", a[0] + dist * 0.9, a[1]),
          mv("B", a[0] + dist, a[1])])

for _ in range(3):
    a = rp()
    case("ixcc", [pt("A", *a), num("r", 2), num("q", 3.5), "k = Circle(A, r)",
                  "c = Circle(A, q)", "P = Intersect(k, c, 1)", "Q = Intersect(k, c, 2)"],
         [setn("q", 2), setn("q", 2.5), mv("A", *rp())])

for _ in range(2):
    a, b = rp(), rp()
    case("ixcc", [pt("A", *a), pt("B", *b), "k = Circle(A, B)", "c = Circle(A, B)",
                  "P = Intersect(k, c, 1)", "Q = Intersect(k, c, 2)"],
         [mv("B", *rp()), mv("A", *rp())])

for _ in range(3):
    a = rp(-3, 3)
    case("ixcc", [pt("A", *a), pt("B", a[0] + 0.5, a[1] - 0.25), num("r", 5), num("q", 1.25),
                  "k = Circle(A, r)", "c = Circle(B, q)", "P = Intersect(k, c, 1)",
                  "Q = Intersect(k, c, 2)"],
         [mv("B", a[0] + 3.9, a[1]), mv("B", a[0] + 3.75, a[1]), mv("B", a[0] + 3.5, a[1])])

for _ in range(5):
    a = rp(-3, 3)
    d = round(rng.uniform(2, 5), 2)
    case("ixcc", [pt("A", *a), pt("B", a[0] + d, a[1] + 0.5), num("r", 1), num("q", 1),
                  "k = Circle(A, r)", "c = Circle(B, q)", "P = Intersect(k, c, 1)",
                  "Q = Intersect(k, c, 2)"],
         [setn("r", d), setn("q", 1.5), setn("r", 0.25), setn("q", d + 0.25), setn("q", 30)])

for _ in range(4):
    a, b = rp(), rp()
    case("ixcc", [pt("A", *a), pt("B", *b), "k = Circle(A, B)", "c = Circle(B, A)",
                  "P = Intersect(k, c, 1)", "Q = Intersect(k, c, 2)", "f = Line(P, Q)"],
         [mv("B", *rp()), mv("A", *rp()), mv("B", *rp())])

# ------------------------------------------------------------------ chain
topic("chain")
for _ in range(5):
    a, b = rp(-2, 2), rp(4, 9)
    script = [pt("A", *a), pt("B", *b),
              "f = Line(A, B)", "k = Circle(A, B)", "n = PerpendicularLine(A, f)",
              "C = Intersect(n, k, 1)", "M = Midpoint(B, C)", "g = Line(A, M)",
              "P = Intersect(g, k, 2)",
              "h = Line(A, P)", "c = Circle(A, P)", "m = PerpendicularLine(A, h)",
              "D = Intersect(m, c, 1)", "N = Midpoint(P, D)", "j = Line(A, N)",
              "Q = Intersect(j, c, 2)"]
    steps = []
    for i in range(6):
        t = i * 0.05 + rng.uniform(0, 0.5)
        steps.append(mv("A", a[0] + math.cos(t), a[1] + math.sin(t)))
    steps.append(mv("A", *b))
    steps.append(mv("A", *a))
    case("chain", script, steps)

for _ in range(6):
    a, b, c = rp(), rp(), rp()
    case("chain", [pt("A", *a), pt("B", *b), pt("C", *c), "M = Midpoint(A, B)",
                   "N = Midpoint(B, C)", "s = Segment(A, B)", "t = Segment(B, C)",
                   "f = PerpendicularLine(M, s)", "g = PerpendicularLine(N, t)",
                   "O = Intersect(f, g)", "k = Circle(O, A)", "r = Distance(O, C)"],
         [mv("C", *rp()), mv("C", 2 * b[0] - a[0], 2 * b[1] - a[1]),   # collinear
          mv("C", *c), mv("A", *rp())])

for _ in range(4):
    a, b, c = rp(), rp(), rp()
    case("chain", [pt("A", *a), pt("B", *b), pt("C", *c), "s = Segment(B, C)",
                   "t = Segment(A, C)", "f = PerpendicularLine(A, s)",
                   "g = PerpendicularLine(B, t)", "H = Intersect(f, g)",
                   "d = Distance(H, C)"],
         [mv("A", *rp()), mv("B", *rp()), mv("C", *rp())])

for _ in range(4):
    a, b, c = rp(), rp(), rp()
    case("chain", [pt("A", *a), pt("B", *b), pt("C", *c), "M = Midpoint(B, C)",
                   "N = Midpoint(A, C)", "f = Line(A, M)", "g = Line(B, N)",
                   "G = Intersect(f, g)", "k = Circle(G, M)"],
         [mv("C", *rp()), mv("B", *rp()), mv("A", *rp())])

for _ in range(3):
    a = rp(-3, 3)
    case("chain", [pt("A", *a), pt("B", a[0] + 3, a[1] + 1), num("r", 2.5), num("q", 2),
                   "k = Circle(A, r)", "c = Circle(B, q)", "P = Intersect(k, c, 1)",
                   "Q = Intersect(k, c, 2)", "f = Line(P, Q)", "g = Line(A, B)",
                   "R = Intersect(f, g)", "M = Midpoint(P, Q)", "d = Distance(R, M)"],
         [mv("B", a[0] + 3.5, a[1] + 1), mv("B", a[0] + 6, a[1] + 2),
          mv("B", a[0] + 3, a[1] + 1)])

for _ in range(3):
    a = rp(-3, 3)
    case("chain", [pt("A", *a), num("r", 3), "k = Circle(A, r)", pt("C", a[0] - 5, a[1] + 1),
                   pt("D", a[0] + 5, a[1] + 1.5), "f = Line(C, D)", "P = Intersect(f, k, 1)",
                   "Q = Intersect(f, k, 2)", "c = Circle(P, Q)", "n = PerpendicularLine(P, f)",
                   "T = Intersect(n, c, 1)", "U = Intersect(n, c, 2)"],
         [mv("D", a[0] + 5, a[1] + 4.5), mv("C", a[0] - 5, a[1] + 4.5),
          mv("C", a[0] - 5, a[1] + 1)])

cases.write()
