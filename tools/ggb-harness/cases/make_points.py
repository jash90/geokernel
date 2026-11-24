#!/usr/bin/env python3
"""Writes cases/points.json: ClosestPoint and Trilinear.

ClosestPoint(path, point) for a line, a segment, a ray and a circle, with the
arguments in either order. The steps move the point onto the path, past the
end points of a segment and behind the start of a ray, to the centre of a
circle, make the path degenerate (coinciding end points, radius 0, radius
below 0) and make the objects undefined.

Trilinear(A, B, C, a, b, c) with the incentre, the vertices, the centroid and
degenerate weights and triangles.

The inputs only describe what to run. Snapshots come exclusively from running
GeoGebra through the harness (`harness.sh golden points`).
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("points")
case = cases.case
topic = cases.topic
rp = cases.rp


# ------------------------------------------------------------ line
topic("closestline")
for _ in range(3):
    a, b, p = rp(), rp(), rp()
    case("closestline", [pt("A", *a), pt("B", *b), pt("P", *p), "l = Line(A, B)",
                         "Q = ClosestPoint(l, P)", "R = ClosestPoint(P, l)"],
         [mv("P", *rp()), mv("P", *a), mv("P", *rp()), mv("B", *rp()), mv("A", *b),
          mv("B", *b), mv("A", *rp())])
case("closestline", [pt("A", 0, 0), pt("B", 4, 2), pt("P", 1, 3), "l = Line(A, B)",
                     "Q = ClosestPoint(l, P)"],
     [mv("P", 2, 1), mv("P", 100000, -100000), mv("P", 0, 0), mv("P", -3, 7),
      mv("B", 0, 4), mv("B", 4, 0), mv("B", 0, 0), mv("B", 0.000001, 0)])
case("closestline", [pt("A", 0, 0), pt("B", 0.001, 0.002), pt("P", 0.5, 0.5), "l = Line(A, B)",
                     "Q = ClosestPoint(l, P)", "d = Distance(P, Q)"],
     [mv("P", 0.001, 0.002), mv("P", 0, 0.0001), mv("B", 0.000001, 0.000002), mv("B", 3, 4)])
case("closestline", [pt("A", 1, 1), pt("B", 5, 1), pt("P", 3, 9), "l = Line(A, B)", "m = Line(B, A)",
                     "Q = ClosestPoint(l, P)", "R = ClosestPoint(m, P)"],
     [mv("P", 3, -4), mv("B", 1, 5), mv("B", 1, 1)])
# Lines made by other commands, a point made by another command.
case("closestline", [pt("A", 0, 0), pt("B", 6, 2), pt("C", 1, 5), "l = Line(A, B)",
                     "m = PerpendicularLine(C, l)", "n = Line(C, l)", "M = Midpoint(A, C)",
                     "Q = ClosestPoint(m, M)", "R = ClosestPoint(n, A)", "S = ClosestPoint(l, C)"],
     [mv("B", 2, 6), mv("C", 3, 3), mv("B", 0, 0), mv("B", 5, 5)])
case("closestline", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 2, 3), "l = Line(A, B)",
                     "m = Line(A, C)", "P = Intersect(l, m)", "Q = ClosestPoint(l, C)",
                     "R = ClosestPoint(m, B)"],
     [mv("C", 2, 6), mv("C", 6, 0), mv("C", 0, 0), mv("C", 0, 3)])

# --------------------------------------------------------- segment
topic("closestsegment")
for _ in range(3):
    a, b, p = rp(), rp(), rp()
    case("closestsegment", [pt("A", *a), pt("B", *b), pt("P", *p), "s = Segment(A, B)",
                            "Q = ClosestPoint(s, P)", "R = ClosestPoint(P, s)"],
         [mv("P", *rp()), mv("P", *a), mv("P", *b), mv("P", *rp()), mv("B", *rp()),
          mv("B", *a), mv("A", *rp()), mv("B", *rp())])
case("closestsegment", [pt("A", 0, 0), pt("B", 4, 0), pt("P", 2, 3), "s = Segment(A, B)",
                        "Q = ClosestPoint(s, P)"],
     [mv("P", -2, 3), mv("P", 6, -3), mv("P", 4, 1), mv("P", 0, -1), mv("P", 4.000001, 1),
      mv("P", -0.000001, 1), mv("P", 3.999999, 1), mv("P", 0.000001, 1), mv("P", 2, 0)])
case("closestsegment", [pt("A", 0, 0), pt("B", 4, 4), pt("P", 6, 2), "s = Segment(A, B)",
                        "Q = ClosestPoint(s, P)"],
     [mv("P", 3, 3), mv("P", 5, 5), mv("P", 4, 4), mv("P", -1, -1), mv("P", 0, 0),
      mv("P", 2, 4), mv("P", 4, 2)])
# Coinciding end points: a segment of length 0, or below the tolerance.
case("closestsegment", [pt("A", 1, 1), pt("B", 1, 1), pt("P", 3, 4), "s = Segment(A, B)",
                        "Q = ClosestPoint(s, P)"],
     [mv("B", 1.000001, 1), mv("B", 1, 1), mv("P", 1, 1), mv("B", 2, 2), mv("A", 2, 2),
      mv("P", 0, 0)])
case("closestsegment", [pt("A", 0, 0), pt("B", 0.000005, 0), pt("P", 0.00001, 1), "s = Segment(A, B)",
                        "Q = ClosestPoint(s, P)"],
     [mv("B", 0.00001, 0), mv("B", 0.000001, 0), mv("B", 0.0001, 0), mv("P", 0, 1),
      mv("P", -1, 1)])
case("closestsegment", [pt("A", 0, 0), pt("B", 5, 0), pt("C", 2, 2), pt("D", 2, 6),
                        "s = Segment(A, B)", "t = Segment(C, D)", "M = Midpoint(C, D)",
                        "Q = ClosestPoint(s, M)", "R = ClosestPoint(t, B)",
                        "S = ClosestPoint(s, C)"],
     [mv("D", 2, -3), mv("B", 1, 1), mv("C", 7, 7), mv("D", 7, 7)])

# ------------------------------------------------------------- ray
topic("closestray")
for _ in range(3):
    a, b, p = rp(), rp(), rp()
    case("closestray", [pt("A", *a), pt("B", *b), pt("P", *p), "r = Ray(A, B)",
                        "Q = ClosestPoint(r, P)", "R = ClosestPoint(P, r)"],
         [mv("P", *rp()), mv("P", *a), mv("P", *b), mv("P", *rp()), mv("B", *rp()),
          mv("B", *a), mv("A", *rp()), mv("B", *rp())])
case("closestray", [pt("A", 0, 0), pt("B", 4, 0), pt("P", 2, 3), "r = Ray(A, B)",
                    "Q = ClosestPoint(r, P)"],
     [mv("P", -2, 3), mv("P", 100, -3), mv("P", -0.000001, 1), mv("P", 0.000001, 1),
      mv("P", 0, -5), mv("P", 0, 0), mv("P", 4, 0), mv("B", 0, 4), mv("B", -4, 0)])
case("closestray", [pt("A", 1, 1), pt("B", 4, 5), pt("P", 7, 1), "r = Ray(A, B)",
                    "Q = ClosestPoint(r, P)", "s = Ray(B, A)", "R = ClosestPoint(s, P)"],
     [mv("P", 0, 0), mv("P", 1, 1), mv("P", 4, 5), mv("P", 10, 10), mv("P", -5, -5),
      mv("B", 1, 1), mv("B", 5, 5)])
case("closestray", [pt("A", 0, 0), pt("B", 2, 2), pt("C", 5, 0), "v = Vector(A, B)",
                    "r = Ray(C, v)", "P = Midpoint(A, B)", "Q = ClosestPoint(r, P)",
                    "R = ClosestPoint(r, A)"],
     [mv("C", 0, 0), mv("C", -3, -3), mv("B", 2, 0), mv("B", 0, 0)])

# ---------------------------------------------------------- circle
topic("closestcircle")
for _ in range(3):
    a, b, p = rp(), rp(), rp()
    case("closestcircle", [pt("A", *a), pt("B", *b), pt("P", *p), "c = Circle(A, B)",
                           "Q = ClosestPoint(c, P)", "R = ClosestPoint(P, c)"],
         [mv("P", *rp()), mv("P", *a), mv("P", *b), mv("P", *rp()), mv("B", *rp()),
          mv("B", *a), mv("A", *rp()), mv("B", *rp())])
case("closestcircle", [pt("O", 0, 0), pt("A", 3, 4), pt("P", 1, 1), "c = Circle(O, A)",
                       "Q = ClosestPoint(c, P)"],
     [mv("P", 0, 0), mv("P", 0.000001, 0), mv("P", 0, 0.000001), mv("P", 3, 4), mv("P", 10, 0),
      mv("P", -10, 0), mv("P", 0, 10), mv("P", 0, -10), mv("P", 100000, 100000),
      mv("A", 0, 0), mv("A", 0.000001, 0), mv("A", 0, 5)])
case("closestcircle", [pt("O", 2, -1), pt("P", 5, 5), num("r", 3), "c = Circle(O, r)",
                       "Q = ClosestPoint(c, P)", "d = Distance(P, Q)"],
     [setn("r", 0), setn("r", 0.000000005), setn("r", 0.00001), setn("r", -1),
      setn("r", 1), mv("P", 2, -1), mv("O", 5, 5), setn("r", 100000), mv("P", 2, 99999)])
case("closestcircle", [pt("O", 0, 0), pt("P", 1, 0), num("r", 1), "c = Circle(O, r)",
                       "Q = ClosestPoint(c, P)"],
     [mv("P", 0, 1), mv("P", -1, 0), mv("P", 0, -1), mv("P", 0.5, 0.5), mv("P", -0.5, -0.5),
      mv("P", 1, 0.000001), mv("P", -1, 0.000001), mv("P", -1, -0.000001), mv("P", -1, 0)])
case("closestcircle", [pt("O", 0, 0), pt("A", 4, 0), pt("B", 0, 3), "c = Circle(O, A)",
                       "d = Circle(B, O)", "P = Midpoint(A, B)", "Q = ClosestPoint(c, P)",
                       "R = ClosestPoint(d, P)", "S = ClosestPoint(c, B)"],
     [mv("A", 6, 0), mv("B", 0, 7), mv("O", 1, 1), mv("O", 6, 0)])
case("closestcircle", [pt("O", 0, 0), pt("A", 1, 0), pt("E", 1.5, 0), pt("P", 3, 3), num("r", 1),
                       "c = Circle(O, r)", "d = Circle(E, r)", "M = Intersect(c, d, 1)",
                       "Q = ClosestPoint(c, M)", "R = ClosestPoint(d, P)"],
     [setn("r", 0.5), setn("r", 5), setn("r", 3), mv("E", 20, 0), mv("E", 1, 0)])

# ------------------------------------------------------- trilinear
topic("trilinear")
case("trilinear", [pt("A", 0, 0), pt("B", 6, 0), pt("C", 2, 5), num("u", 1), num("v", 1),
                   num("w", 1), "I = Trilinear(A, B, C, u, v, w)"],
     [mv("C", 3, 7), mv("B", 9, 2), mv("A", -4, 3), setn("u", 0), setn("v", 0), setn("w", 0),
      setn("u", 1), setn("v", 2), setn("w", 3), setn("u", -1), setn("w", -3)])
case("trilinear", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 3), num("u", 1), num("v", 0),
                   num("w", 0), "P = Trilinear(A, B, C, u, v, w)", "Q = Trilinear(A, B, C, v, u, w)",
                   "R = Trilinear(A, B, C, v, w, u)"],
     [setn("v", 1), setn("w", 1), setn("u", 0), setn("u", 5), setn("v", 0), setn("w", 0)])
# The three weights sum to zero: GeoGebra divides by zero.
case("trilinear", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 3), num("u", 3), num("v", 4),
                   num("w", -5), "P = Trilinear(A, B, C, u, v, w)"],
     [setn("w", -5.000001), setn("w", -4), setn("w", -6), setn("u", 0), setn("v", 0),
      setn("w", 0)])
# Trilinear coordinates of the circumcentre: cos A : cos B : cos C is not
# expressible with the given numbers, so use the numbers as free sliders.
for _ in range(3):
    a, b, c = rp(), rp(), rp()
    case("trilinear", [pt("A", *a), pt("B", *b), pt("C", *c), num("u", cases.r2(0, 3)),
                       num("v", cases.r2(0, 3)), num("w", cases.r2(0, 3)),
                       "P = Trilinear(A, B, C, u, v, w)"],
         [mv("C", *rp()), mv("A", *rp()), setn("u", cases.r2(-3, 3)), setn("v", cases.r2(-3, 3)),
          mv("B", *a), mv("B", *rp())])
# Degenerate triangles.
case("trilinear", [pt("A", 0, 0), pt("B", 2, 2), pt("C", 5, 5), num("u", 1), num("v", 1),
                   num("w", 1), "P = Trilinear(A, B, C, u, v, w)"],
     [mv("C", 5, 5.000001), mv("C", 1, 1), mv("B", 0, 0), mv("C", 0, 0), mv("B", 3, 1)])
case("trilinear", [pt("A", 1, 1), pt("B", 1, 1), pt("C", 4, 5), num("u", 2), num("v", 3),
                   num("w", 5), "P = Trilinear(A, B, C, u, v, w)"],
     [mv("B", 2, 1), mv("B", 1, 1), mv("C", 1, 1), mv("A", 4, 5)])
# Numbers made by commands and points made by commands.
case("trilinear", [pt("A", 0, 0), pt("B", 6, 0), pt("C", 3, 4), "M = Midpoint(A, B)",
                   "a = Distance(B, C)", "b = Distance(A, C)", "c = Distance(A, B)",
                   "P = Trilinear(A, B, C, a, b, c)", "Q = Trilinear(A, B, M, a, b, c)"],
     [mv("C", 3, 8), mv("B", 10, 0), mv("A", -2, 1), mv("C", 0, 0)])
case("trilinear", [pt("A", 0, 0), pt("B", 0.001, 0), pt("C", 0.0005, 0.001), num("u", 1),
                   num("v", 1), num("w", 1), "P = Trilinear(A, B, C, u, v, w)"],
     [mv("C", 0.0005, 1000), mv("B", 1000, 0), setn("u", 0.000001), setn("u", 100000)])

# Weights whose sum is zero: the barycentric combination has weight 0.
case("trilinear", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 3), num("u", 4), num("v", 0),
                   num("w", -5), "P = Trilinear(A, B, C, u, v, w)"],
     [setn("v", 0.000001), setn("v", 0), setn("w", -5.000001), setn("u", 0), setn("w", 0),
      setn("u", 4), setn("w", -5)])
case("trilinear", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 3), num("u", 3), num("v", 5),
                   num("w", -7.5), "P = Trilinear(A, B, C, u, v, w)", "l = Line(A, B)",
                   "Q = ClosestPoint(l, P)"],
     [mv("C", 0, 6), setn("w", -7), setn("w", -7.5), mv("C", 0, 3)])

# Points at infinity as inputs: the intersection of parallel lines.
case("infinity", [pt("A", 0, 0), pt("B", 2, 1), pt("C", 0, 3), pt("D", 2, 4), pt("F", 5, 5),
                  "l = Line(A, B)", "m = Line(C, D)", "S = Intersect(l, m)",
                  "P = ClosestPoint(l, S)", "Q = ClosestPoint(S, l)", "k = Circle(A, B)",
                  "R = ClosestPoint(k, S)"],
     [mv("D", 2, 5), mv("D", 2, 4), mv("F", 0, 6)])
# ClosestPoint of two lines.
case("pathpair", [pt("A", 0, 0), pt("B", 2, 1), pt("C", 0, 3), pt("D", 2, 4), "l = Line(A, B)",
                  "m = Line(C, D)", "P = ClosestPoint(l, m)"],
     [mv("D", 4, 1), mv("D", 2, 4), mv("D", 2, 4.000001), mv("B", 2, 0), mv("D", 4, 3)])

PROJECTIVE = ["A = (0, 0)", "B = (2, 1)", "C = (0, 3)",
              "l = Line(A, B)", "m = Line(C, D)", "S = Intersect(l, m)"]
CIRCLE_PROJECTION = ["k = Circle(A, B)", "R = ClosestPoint(k, S)"]

case("infinityedit", ["D = (2, 5)"] + PROJECTIVE +
     ["F = (5, 5)", "P = ClosestPoint(l, S)", "Q = ClosestPoint(S, l)",
      "k = Circle(A, B)", "R = ClosestPoint(k, B)"],
     [{"action": "redefine", "line": "R = ClosestPoint(k, S)"}, mv("D", 2, 4)])
case("infinityedit", ["D = (2, 5)"] + PROJECTIVE + CIRCLE_PROJECTION,
     [mv("D", 0, 3), mv("D", 2, 4), {"action": "add", "line": "J = (5, 5)"},
      {"action": "undo"}])
case("infinityedit", ["D = (2, 4)"] + PROJECTIVE + CIRCLE_PROJECTION,
     [mv("D", 2, 5), mv("D", 2, 4), {"action": "reload_ggb"}])
case("infinityedit", ["D = (2, 4)"] + PROJECTIVE +
     ["c = Conic({{1,0,0},{0,1,0},{0,0,-4}})", "P = ClosestPoint(c, S)"],
     [mv("D", 2, 5), mv("D", 2, 4)])
case("infinityedit", ["D = (2, 4)"] + PROJECTIVE +
     ["r = 2", "k = Circle(A, r)", "R = ClosestPoint(k, S)"],
     [setn("r", 3), setn("r", 4), setn("r", 5)])
case("infinityedit", ["D = (2, 4)"] + PROJECTIVE + CIRCLE_PROJECTION,
     [mv("D", 2, 5), mv("D", 0, 3), mv("D", 2, 4),
      {"action": "add", "line": "J = (5, 5)"}, {"action": "undo"}])

# Projective coordinates and retained parameters during edits and sampling.
case("projectiveupdates", ["a = Slider(0, 1, 0.01)"],
     [setn("a", 0)] + [{"action": "add", "line": line} for line in [
         "C = (0, 1)", "D = (1, 1+a)", "m = Line(C, D)", "S = Intersect(xAxis, m)",
         "G = (0, 2)", "h = Ray(G, S)", "F = (1, 1)", "g = Line(C, F)",
         "Q = Intersect(h, g)", "L = Locus(Q, a)", "f = (x(Q)+1)^2", "M = Minimize(f, a)",
     ]])
case("projectiveupdates", ["D = (2, 4)"] + PROJECTIVE +
     ["r = Slider(1, 3, 0.1)", "k = Circle(A, r)", "R = ClosestPoint(k, S)",
      "Q = (x(R), y(R))", "L = Locus(Q, r)"], [])
case("projectiveupdates", ["D = (2, 4)"] + PROJECTIVE +
     ["c = Conic({{1,0,0},{0,1,0},{0,0,-4}})", "c2 = Rotate(c, pi/2, A)",
      "R = ClosestPoint(c2, S)"], [])
case("projectiveupdates", ["D = (2, 4)"] + PROJECTIVE +
     ["n = 1", "c = Conic({{1,0,0},{0,n,0},{0,0,-4}})", "R = ClosestPoint(c, S)"],
     [mv("D", 2, 5), mv("D", 2, 4), setn("n", 2), setn("n", 1)])
case("projectiveupdates", ["D = (2, 4)"] + PROJECTIVE +
     ["c = Conic({{1,0,0},{0,1,0},{0,0,0}})", "R = ClosestPoint(c, S)"], [])
case("projectiveupdates", ["O = (0, 0)", "P = (1, 2)", "k = Circle(O, 2)",
                          "R = ClosestPoint(k, P)"],
     [mv("P", *p) for p in [(2, 1), (1, 2), (2, 1), (1, 2), (2, 1)]] +
     [{"action": "add", "line": "J = (5, 5)"}, {"action": "undo"}])

case("infinityedit", ["D = (2, 4)"] + PROJECTIVE +
     ["k = Circle(A, 2)", "R = ClosestPoint(k, S)"],
     [mv("D", 2, 5), mv("D", 2, 6), mv("D", 2, 4),
      {"action": "add", "line": "J = (5, 5)"}, {"action": "undo"}])
case("projectiveupdates", ["a = Slider(0, 1, 0.01)"],
     [setn("a", 0)] + [{"action": "add", "line": line} for line in [
         "C = (0, 1)", "D = (1, 1+a)", "m = Line(C, D)", "S = Intersect(xAxis, m)",
         "G = (0, 2)", "h = Ray(G, S)", "F = (1, 1)", "g = Line(C, F)",
         "Q = Intersect(h, g)", "L = Locus(Q, a)", "f = (x(Q)+1)^2", "M = Minimize(f, a)",
         "k = Circle((0,0),2)", "R = ClosestPoint(k,S)", "u = (a-0.5)^2+(x(R)+2)^2",
         "N = Minimize(u,a)",
     ]])
case("projectiveupdates", ["D = (2, 4)"] + PROJECTIVE +
     ["n = 4", "c = Conic({{1,0,0},{0,1,0},{0,0,-n}})", "R = ClosestPoint(c, S)"],
     [mv("D", 2, 5), mv("D", 2, 4), setn("n", 0), setn("n", 4)])
case("projectiveupdates", ["D = (2, 4)"] + PROJECTIVE +
     ["n = 1", "c = Conic({{1,0,0},{0,n,0},{0,0,-4}})", "R = ClosestPoint(c, S)"],
     [mv("D", 2, 5), mv("D", 2, 4), setn("n", -1), setn("n", 1)])

case("isolatedprojection", ["a = Slider(0, 1, 0.01)"],
     [setn("a", 0)] + [{"action": "add", "line": line} for line in [
         "C = (0, 1)", "D = (1, 1+a)", "m = Line(C, D)", "S = Intersect(xAxis, m)",
         "k = Circle((0,0),2)", "R = ClosestPoint(k,S)", "u = (a-0.5)^2+(x(R)+2)^2",
         "N = Minimize(u,a)",
     ]])

cases.write()
