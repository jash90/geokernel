#!/usr/bin/env python3
"""Writes cases/polygon.json: polygons, polylines and what is measured on them.

Covers Polygon(A, B, C, ...) with two to seven points, Polygon(A, B, n) with a
literal and with a number object, PolyLine, Area of a polygon and of five or
more points, Perimeter, Centroid, Vertex(p), Vertex(p, n), InteriorAngles and
Angle of a polygon, and the labels GeoGebra gives to the sides and the new
vertices.

The steps make polygons degenerate (collinear and coincident points), turn
their orientation, make them self-intersect, move vertices so that the area
stays the same (a vertex parallel to a diagonal), change the vertex count of
regular polygons, and make polyline points undefined (holes).

Further topics cover what is labelled when a point or the vertex count is
undefined or too small at creation, vertex counts and Vertex indices beyond
a Java int, and Area of points one of which is undefined.

The inputs only describe what to run. Snapshots come exclusively from running
GeoGebra through the harness (`harness.sh golden polygon`).
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("polygon")
case = cases.case
topic = cases.topic
rp = cases.rp
r2 = cases.r2

LETTERS = "ABCDEFG"


def points(n, labels=LETTERS):
    return [pt(labels[i], *rp()) for i in range(n)]


def moves(labels, k):
    return [mv(labels[cases.rng.randrange(len(labels))], *rp()) for _ in range(k)]


def mirror(p, a, b):
    """`p` mirrored at the line through `a` and `b`."""
    (px, py), (ax, ay), (bx, by) = p, a, b
    dx, dy = bx - ax, by - ay
    t = ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)
    fx, fy = ax + t * dx, ay + t * dy
    return (2 * fx - px, 2 * fy - py)


MEASURES = ["ar = Area(p)", "pe = Perimeter(p)", "M = Centroid(p)"]

# ----------------------------------------------------------------- triangles
topic("triangle")
for _ in range(20):
    a, b, c = rp(), rp(), rp()
    case("triangle", [pt("A", *a), pt("B", *b), pt("C", *c), "p = Polygon(A, B, C)"] + MEASURES,
         moves("ABC", 3)
         # collinear, then the other orientation, then coincident points
         + [mv("C", (a[0] + b[0]) / 2, (a[1] + b[1]) / 2), mv("C", *mirror(c, a, b)),
            mv("C", *a), mv("B", *a), mv("C", *c), mv("B", *b)])
# the vertices of a fixed triangle in both orientations
case("triangle", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 1, 3), "p = Polygon(A, B, C)"] + MEASURES,
     [mv("C", 1, -3), mv("C", 2, 0), mv("C", 8, 0), mv("A", 1e6, 1e6), mv("A", 0.000001, 0)])
# equal-area moves: C parallel to AB keeps the area, perimeter and centroid move
case("triangle", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 1, 3), "p = Polygon(A, B, C)"] + MEASURES
     + ["W = Vertex(p, 3)"],
     [mv("C", 2, 3), mv("C", -5, 3), mv("C", 7.5, 3), mv("C", 7.5, -3)])
for _ in range(4):
    a, b, c = rp(), rp(), rp()
    dx, dy = b[0] - a[0], b[1] - a[1]
    steps = [mv("C", c[0] + t * dx, c[1] + t * dy) for t in (0.5, -1.25, 2.0)]
    case("triangle", [pt("A", *a), pt("B", *b), pt("C", *c), "p = Polygon(A, B, C)"] + MEASURES
         + ["V = Vertex(p)"], steps)

# ----------------------------------------------------------------- quadrilaterals
topic("quad")
for _ in range(20):
    case("quad", points(4) + ["p = Polygon(A, B, C, D)"] + MEASURES, moves("ABCD", 6))
# a square becomes a bow tie (self-intersecting), then the other orientation
case("quad", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 4, 3), pt("D", 0, 3),
              "p = Polygon(A, B, C, D)"] + MEASURES,
     [mv("C", 0, 3), mv("D", 4, 3), mv("D", 5, 4), mv("C", 4, -3), mv("D", 0, -3),
      mv("C", 4, 3), mv("D", 0, 3)])
# self-intersecting with a non-zero signed area
case("quad", [pt("A", 0, 0), pt("B", 6, 0), pt("C", 1, 4), pt("D", 3, -1),
              "p = Polygon(A, B, C, D)"] + MEASURES,
     [mv("D", 4, 5), mv("C", 6, 4), mv("B", 0, 4)])
# equal-area moves: D parallel to the diagonal AC
for _ in range(4):
    a, b, c, d = rp(), rp(), rp(), rp()
    dx, dy = c[0] - a[0], c[1] - a[1]
    case("quad", [pt("A", *a), pt("B", *b), pt("C", *c), pt("D", *d),
                  "p = Polygon(A, B, C, D)"] + MEASURES + ["V = Vertex(p)"],
         [mv("D", d[0] + t * dx, d[1] + t * dy) for t in (0.5, -0.75, 1.5)])
# all points on one line, then all equal
case("quad", [pt("A", 0, 0), pt("B", 1, 1), pt("C", 3, 3), pt("D", -2, 1),
              "p = Polygon(A, B, C, D)"] + MEASURES,
     [mv("D", -2, -2), mv("B", 0, 0), mv("C", 0, 0), mv("D", 0, 0), mv("D", 1, 5)])
# a vertex repeated in the list
case("quad", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 2, 3), "p = Polygon(A, B, C, A)"] + MEASURES,
     [mv("C", 2, -3), mv("B", 0, 0)])

# ----------------------------------------------------------------- more vertices
topic("many")
for n in (5, 5, 5, 6, 6, 6, 7, 7, 7):
    labels = LETTERS[:n]
    case("many", points(n) + [f"p = Polygon({', '.join(labels)})"] + MEASURES,
         moves(labels, 5))
# a convex hexagon turned inside out vertex by vertex
hexagon = [(4, 0), (2, 3.4), (-2, 3.4), (-4, 0), (-2, -3.4), (2, -3.4)]
case("many", [pt(LETTERS[i], *hexagon[i]) for i in range(6)]
     + ["p = Polygon(A, B, C, D, E, F)"] + MEASURES,
     [mv("B", 0.5, 0.2), mv("C", 1, -1), mv("A", -5, -5), mv("A", 4, 0)])
# a star: self-intersecting pentagon
star = [(0, 5), (2.94, -4.05), (-4.76, 1.55), (4.76, 1.55), (-2.94, -4.05)]
case("many", [pt(LETTERS[i], *star[i]) for i in range(5)]
     + ["p = Polygon(A, B, C, D, E)"] + MEASURES + ["w = InteriorAngles(p)"],
     [mv("A", 0, 6), mv("C", -4, -1), mv("E", 1, 1)])
# Area of five or more points
for n in (5, 5, 6, 6, 7):
    labels = LETTERS[:n]
    case("many", points(n) + [f"ar = Area({', '.join(labels)})",
                              f"br = Area({', '.join(reversed(labels))})"],
         moves(labels, 4))

# ----------------------------------------------------------------- two points
topic("two")
for _ in range(5):
    case("two", points(2) + ["p = Polygon(A, B)", "ar = Area(p)", "pe = Perimeter(p)",
                             "M = Centroid(p)", "V = Vertex(p)"],
         moves("AB", 3) + [mv("B", 0, 0), mv("A", 0, 0)])

# ----------------------------------------------------------------- regular polygons
topic("regular")
for n in (3, 4, 5, 6, 7, 8, 10, 12):
    case("regular", points(2) + [f"p = Polygon(A, B, {n})", "ar = Area(p)", "pe = Perimeter(p)",
                                 "S = Centroid(p)"],
         moves("AB", 3) + [mv("B", 0, 0), mv("A", 0, 0), mv("B", 1, 0)])
# a number object: fewer, more and not whole vertex counts
for start, values in [(4, [3, 2, 4, 6, 5]), (5, [3.4, 3.5, 4.49, 1, -3, 5]),
                      (6, [4, 9, 6, 2.5, 7])]:
    case("regular", points(2) + [num("nv", start), "p = Polygon(A, B, nv)", "ar = Area(p)",
                                 "pe = Perimeter(p)", "S = Centroid(p)", "V = Vertex(p)"],
         [setn("nv", v) for v in values] + moves("AB", 2))
# the same with literals that round
for n in ("3.5", "4.4", "2.6"):
    case("regular", [pt("A", 0, 0), pt("B", 2, 0), f"p = Polygon(A, B, {n})", "ar = Area(p)",
                     "pe = Perimeter(p)"],
         [mv("B", 0, 2), mv("A", -1, -1)])
# a regular polygon on derived points, and its vertices used further
case("regular", [pt("A", 0, 0), pt("B", 4, 1), pt("U", 2, 5), "Q = Midpoint(A, U)",
                 "p = Polygon(Q, B, 5)", "ar = Area(p)", "W = Vertex(p, 4)",
                 "w = InteriorAngles(p)"],
     [mv("U", 6, 3), mv("A", 4, 1), mv("B", -2, -2)])
for _ in range(12):
    case("regular", points(2) + [num("nv", 4), "p = Polygon(A, B, nv)", "pe = Perimeter(p)"],
         [setn("nv", r2(1, 10)) for _ in range(3)] + moves("AB", 2))

# ----------------------------------------------------------------- polylines
topic("polyline")
for n in (2, 3, 3, 4, 4, 5, 6, 7):
    labels = LETTERS[:n]
    case("polyline", points(n) + [f"pl = PolyLine({', '.join(labels)})", "V = Vertex(pl)",
                                  "W = Vertex(pl, 2)"],
         moves(labels, 4) + [mv("B", *(0, 0)), mv("A", 0, 0)])
# a point that becomes undefined leaves a hole
for _ in range(3):
    case("polyline", [pt("A", -6, 1), pt("B", -2, 4), pt("O", 3, 0), num("r", 7),
                      "k = Circle(O, r)", "l = Line(A, B)", "X = Intersect(l, k, 1)",
                      pt("C", *rp()), pt("D", *rp()), "pl = PolyLine(A, B, X, C, D)",
                      "V = Vertex(pl)"],
         [setn("r", 0.5), mv("B", -2, 1), setn("r", 4), mv("O", 20, 20), mv("O", 3, 0),
          setn("r", 7)])
case("polyline", [pt("A", 0, 0), pt("B", 3, 4), pt("C", 3, 4), pt("D", 0, 0),
                  "pl = PolyLine(A, B, C, D, A)"],
     [mv("C", 6, 0), mv("B", 1e6, 0), mv("A", 0.000001, 0)])

# ----------------------------------------------------------------- vertices
topic("vertex")
for _ in range(8):
    case("vertex", points(4) + ["p = Polygon(A, B, C, D)", num("idx", 2), "W = Vertex(p, idx)",
                                "V = Vertex(p)"],
         [setn("idx", v) for v in (1, 4, 4.9, 5, 0, 0.5, -1, 3)] + moves("ABCD", 2))
# a polygon that becomes undefined: its vertices too
case("vertex", [pt("A", -6, 1), pt("B", -2, 4), pt("O", 3, 0), num("r", 7), "k = Circle(O, r)",
                "l = Line(A, B)", "X = Intersect(l, k, 1)", pt("C", 4, -4),
                "p = Polygon(A, X, C)", "V = Vertex(p)", "W = Vertex(p, 2)",
                "ar = Area(p)", "pe = Perimeter(p)", "M = Centroid(p)",
                "w = InteriorAngles(p)"],
     [setn("r", 0.5), setn("r", 7), mv("B", -2, 1), mv("O", 30, 0), mv("O", 3, 0)])

# ----------------------------------------------------------------- angles
topic("angles")
for n in (3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 6, 6, 7, 7):
    labels = LETTERS[:n]
    case("angles", points(n) + [f"p = Polygon({', '.join(labels)})", "w = InteriorAngles(p)",
                                "u = Angle(p)"],
         moves(labels, 4))
# orientation flips: interior angles stay inside, Angle(p) turns
case("angles", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 4, 3), pt("D", 0, 3),
                "p = Polygon(A, B, C, D)", "w = InteriorAngles(p)", "u = Angle(p)"],
     [mv("C", 4, -3), mv("D", 0, -3), mv("C", 0, 3), mv("D", 4, 3), mv("D", 2, 0),
      mv("B", 0, 0)])
case("angles", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 1, 3), "p = Polygon(A, B, C)",
                "w = InteriorAngles(p)", "u = Angle(p)"],
     [mv("C", 1, -3), mv("C", 2, 0), mv("C", 0, 0), mv("C", 1, 3)])

# ----------------------------------------------------------------- labels
topic("labels")
# sides are named after vertices; taken names get an index
case("labels", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 1, 3), pt("D", -1, 2),
                "p = Polygon(A, B, C)", "q = Polygon(A, B, C, D)", "s = Polygon(A, B)"],
     [mv("C", 3, 3)])
case("labels", [num("c", 3), pt("A", 0, 0), pt("B", 4, 0), pt("C", 1, 3),
                "a = Polygon(A, B, C)"],
     [mv("A", 1, -1)])
# x, y and z are reserved names: the sides get x_1, y_1, z_1
case("labels", [pt("X", 0, 0), pt("E", 4, 0), pt("I", 4, 3), pt("Y", 0, 3), pt("Z", -2, 1),
                "p = Polygon(X, E, I, Y, Z)"],
     [mv("Z", -1, 5)])
# longer vertex labels: lower case when free, else the next default label
case("labels", [pt("PA", 0, 0), pt("PB", 4, 0), pt("PC", 2, 3), num("pb", 2),
                "p = Polygon(PA, PB, PC, PA)", "q = Polygon(PA, PB, PC)"],
     [mv("PC", 2, -3)])
# a regular polygon next to existing points and segments
case("labels", [pt("A", 0, 0), pt("B", 2, 0), pt("D", 5, 5), "f = Segment(A, D)",
                "p = Polygon(A, B, 6)", "q = Polygon(B, A, 3)"],
     [mv("B", 0, 2)])
# many polygons on the same points: indices go past 9
case("labels", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 1, 3)]
     + [f"{name} = Polygon(A, B, C)" for name in ("p", "q", "r", "s", "t", "u", "v", "w",
                                                  "pp", "qq", "rr")],
     [mv("C", 2, 2)])

# ----------------------------------------------------------------- derived points
topic("derived")
# polygons through midpoints and intersections, measured further
for _ in range(4):
    case("derived", points(4) + ["M = Midpoint(A, B)", "N = Midpoint(C, D)",
                                 "p = Polygon(M, B, N, D)"] + ["ar = Area(p)", "pe = Perimeter(p)",
                                                               "G = Centroid(p)",
                                                               "w = InteriorAngles(p)"],
         moves("ABCD", 5))
for _ in range(3):
    case("derived", points(4) + ["l = Line(A, B)", "m = Line(C, D)", "S = Intersect(l, m)",
                                 "p = Polygon(A, S, C)", "ar = Area(p)", "G = Centroid(p)"],
         moves("ABCD", 4))
# the centroid of one polygon as a vertex of the next
for _ in range(3):
    case("derived", points(4) + ["p = Polygon(A, B, C, D)", "G = Centroid(p)",
                                 "q = Polygon(A, G, D)", "ar = Area(q)", "pe = Perimeter(q)",
                                 "V = Vertex(q)"],
         moves("ABCD", 4))

# ----------------------------------------------------------------- undefined at creation
# A point that is undefined at creation: the line AB misses the circle of
# radius 0.5 (its distance from O is 6.2) and meets the one of radius 7.
MISSING = [pt("A", -6, 1), pt("B", -2, 4), pt("O", 3, 0), num("r", 0.5), "k = Circle(O, r)",
           "l = Line(A, B)", "X = Intersect(l, k, 1)"]
APPEAR = [setn("r", 7), mv("B", -2, 2), setn("r", 0.5), setn("r", 7)]

# regular polygons with at most two vertices at creation: no vertices or
# sides are labelled, also when the vertex count grows later
topic("regsmall")
for n in ("0", "1", "2", "-3", "2.4", "-0.4"):
    case("regsmall", [pt("A", 0, 0), pt("B", 2, 0), f"p = Polygon(A, B, {n})", "ar = Area(p)"],
         [mv("B", 0, 2)])
case("regsmall", [pt("A", 0, 0), pt("B", 2, 0), num("nv", 2), "p = Polygon(A, B, nv)",
                  "ar = Area(p)", "pe = Perimeter(p)", "S = Centroid(p)"],
     [setn("nv", v) for v in (4, 6, 3, 2, 5)] + [mv("B", 1, 3)])
# the vertex count from a number that is undefined at creation
case("regsmall", MISSING + [pt("C", 4, -4), pt("D", 1, -6), "d = Area(A, B, C, X, D)",
                            "p = Polygon(C, D, d)", "ar = Area(p)"],
     APPEAR)

# regular polygons on a point that is undefined at creation
topic("regundef")
case("regundef", MISSING + [pt("C", 4, -4), "p = Polygon(X, C, 4)", "ar = Area(p)",
                            "pe = Perimeter(p)"], APPEAR)
case("regundef", MISSING + [pt("C", 4, -4), "p = Polygon(C, X, 5)", "ar = Area(p)",
                            "W = Vertex(p, 3)"], APPEAR)

# vertex counts are rounded to a Java int, which wraps
topic("wrap")
for n in ("4294967299", "-4294967293", "4294967296", "1000000000000000"):
    case("wrap", [pt("A", 0, 0), pt("B", 2, 0), f"p = Polygon(A, B, {n})", "ar = Area(p)",
                  "pe = Perimeter(p)"],
         [mv("B", 1, 2)])
case("wrap", [pt("A", 0, 0), pt("B", 2, 0), num("nv", 4), "p = Polygon(A, B, nv)",
              "ar = Area(p)", "V = Vertex(p)"],
     [setn("nv", v) for v in (4294967299, 4294967296, 1e20, -4294967293, -1e20, 4)])

# Vertex(p, n) with indices beyond an int, below one, or undefined
topic("vertexidx")
case("vertexidx", [pt("A", 0, 0), pt("B", 2, 0), "p = Polygon(A, B, 5)", num("idx", 2),
                   "W = Vertex(p, idx)"],
     [setn("idx", v) for v in (7, 1e20, -1e20, 4294967298, 4294967297, 2147483648,
                               -2147483648, -0.5, 0.99, 5, 6, 1)])
case("vertexidx", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 4, 3), pt("D", 0, 3),
                   "p = Polygon(A, B, C, D)", num("idx", 2), "W = Vertex(p, idx)"],
     [setn("idx", v) for v in (4294967298, 2147483649, -2147483649, 5, 4)])
case("vertexidx", MISSING + [pt("C", 4, -4), pt("D", 1, -6), "d = Area(A, B, C, X, D)",
                             "q = Polygon(A, B, C, D)", "W = Vertex(q, d)"], APPEAR)

# polygons from one point, and the sides of polygons with a vertex that is
# undefined at creation
topic("small")
case("small", [pt("A", 1, 2), "p = Polygon(A)", "ar = Area(p)", "pe = Perimeter(p)",
               "M = Centroid(p)"],
     [mv("A", -3, 0)])
case("small", MISSING + [pt("C", 4, -4), "p = Polygon(A, X, C)", "ar = Area(p)",
                         "pe = Perimeter(p)"], APPEAR)
case("small", MISSING + [pt("C", 4, -4), "p = Polygon(A, B, X, C)", "ar = Area(p)"], APPEAR)
case("small", MISSING + ["p = Polygon(A, X)", "pe = Perimeter(p)"], APPEAR)

# Area of five or more points, one of them undefined
topic("areaundef")
case("areaundef", MISSING + [pt("C", 4, -4), pt("D", 1, -6), "ar = Area(A, B, X, C, D)",
                             "as = Area(A, B, C, D, X, A)"], APPEAR)

# a regular polygon whose vertex count shrinks below three and grows again
topic("shrink")
case("shrink", [pt("A", 0, 0), pt("B", 2, 0), num("nv", 5), "p = Polygon(A, B, nv)",
                "w = InteriorAngles(p)", "V = Vertex(p)", "W = Vertex(p, 4)"],
     [setn("nv", v) for v in (3, 2, 1, 0, 4, 5, 6)] + [mv("B", 0, 2)])

# The indexed label family for Arabic heh starts with heh and tatweel.
topic("sidelabels")
triangle = [pt("A", 0, 0), pt("B", 2, 0), pt("C", 0, 2), "ه = Midpoint(A, B)"]
case("sidelabels", triangle + ["p = Polygon(ه, B, C)"], [])
case("sidelabels", triangle + ["هـ = 1", "هـ_{1} = 2", "p = Polygon(ه, B, C)"], [])

topic("keptsides")
for count in (4, 5):
    case("keptsides", [num("n", 1), pt("U", 0, 0), "A = If(n > 0, U)", pt("B", 2, 0),
                       num("nv", count), "p = Polygon(A, B, nv)"],
         [setn("n", 0)] + ([setn("nv", 3), setn("nv", 5)] if count == 5 else [setn("n", 1)]))

# A vertex keeps the coordinates it gained while the first endpoint was defined.
topic("keptvertices")
case("keptvertices", [num("n", 0), pt("U", 0, 0), "A = If(n > 0, U)", pt("B", 2, 0),
                      "p = Polygon(A, B, 5)"],
     [setn("n", 1), setn("n", 0)])

cases.write()
