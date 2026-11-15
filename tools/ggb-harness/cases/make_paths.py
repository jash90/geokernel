#!/usr/bin/env python3
"""Writes cases/paths.json: points on paths and their parameters.

`Point(path)` is a point the user moves along its path (a `move` step on it
projects the moved position onto the path), `Point(path, u)` is the point at
the parameter `u` from 0 to 1, and `PathParameter(P)` reads the parameter of
a point on a path. The cases move the point itself and the inputs of its
path, put the path on its degenerate configurations (coincident end points,
a circle of radius 0, an undefined path) and back, so that the point has to
follow the parameter it kept. Every point on a path is defined when it is
created: GeoGebra places a point it cannot put at (0, 0) or (1, 0) at a
random position.

New cases go at the end of their topic and new topics at the end of the file,
so that existing ids keep their meaning.
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("paths")
case = cases.case
topic = cases.topic
rp = cases.rp
r2 = cases.r2


def on(path, point="P", param="t"):
    """A point on `path`, its parameter, a point at a fixed parameter and its parameter."""
    return [f"{point} = Point({path})", f"{param} = PathParameter({point})"]


def at(path, u, point="U", number="u", param="w"):
    return [num(number, u), f"{point} = Point({path}, {number})", f"{param} = PathParameter({point})"]


def u01():
    return round(cases.rng.uniform(0.05, 0.95), 3)


# ------------------------------------------------------------------- lines
topic("line")
for _ in range(8):
    a, b = rp(), rp()
    script = [pt("A", *a), pt("B", *b), "g = Line(A, B)"] + on("g") + at("g", u01())
    case("line", script, [mv("P", *rp()), mv("A", *rp()), mv("P", *rp()), mv("B", *rp()),
                          setn("u", u01()), mv("P", *rp())])
# vertical and horizontal lines: both branches of the parameter
case("line", [pt("A", 1, -2), pt("B", 1, 5), "g = Line(A, B)"] + on("g") + at("g", 0.25),
     [mv("P", 4, 3), mv("B", 6, -2), mv("P", -3, 0), mv("A", -4, -2), setn("u", 0.8)])
# the line becomes undefined (A on B) and comes back: the point keeps its parameter
case("line", [pt("A", 0, 0), pt("B", 4, 2), "g = Line(A, B)"] + on("g") + at("g", 0.7),
     [mv("P", 3, 5), mv("B", 0, 0), mv("B", 1, 3), mv("P", -2, 1), mv("A", 1, 3), mv("A", -1, 0)])
# the point is moved while its line is undefined, then the line comes back
case("line", [pt("A", 0, 0), pt("B", 4, 2), "g = Line(A, B)"] + on("g"),
     [mv("P", 3, 5), mv("B", 0, 0), mv("P", 7, 7), mv("B", 2, 2)])
# moves of the point to the same position, and far away
case("line", [pt("A", -1, 1), pt("B", 2, 3), "g = Line(A, B)"] + on("g"),
     [mv("P", 1, 1), mv("P", 1, 1), mv("P", 1000, -2000), mv("P", 0.000001, 0)])
# the start point moves along the line: the line stays, the point follows the start point
case("line", [pt("A", 0, 0), pt("B", 3, 0), "g = Line(A, B)"] + on("g") + at("g", 0.6),
     [mv("P", 2, 1), mv("A", 1, 0), mv("A", -2, 0), mv("B", 5, 0)])

# ---------------------------------------------- lines with a start point
topic("start")
# a perpendicular through C to a horizontal line: C moves along the perpendicular
case("start", [pt("A", 0, 0), pt("B", 4, 0), "g = Line(A, B)", pt("C", 1, 2), "h = PerpendicularLine(C, g)"]
     + on("h"), [mv("P", 3, 5), mv("C", 1, 4), mv("C", 1, -1), mv("B", 4, 1), mv("C", 2, 0)])
# a parallel through C: C moves along it
case("start", [pt("A", 0, 0), pt("B", 4, 0), "g = Line(A, B)", pt("C", 1, 2), "h = Line(C, g)"]
     + on("h") + at("h", 0.3), [mv("P", 3, 5), mv("C", 5, 2), mv("C", -1, 2), mv("A", 0, 1)])
case("start", [pt("A", 1, 1), pt("B", 3, 4), "s = Segment(A, B)", pt("C", -2, 1), "h = PerpendicularLine(C, s)"]
     + on("h"), [mv("P", 0, 0), mv("C", -3, 3), mv("B", 5, 0)])
# perpendicular bisectors: the midpoint is the start point
case("start", [pt("A", 1, 1), pt("B", 5, 3), "h = PerpendicularBisector(A, B)"] + on("h") + at("h", 0.4),
     [mv("P", 0, 6), mv("A", -1, 3), mv("B", 3, 5), mv("A", 3, 5), mv("A", 0, 0)])
case("start", [pt("A", 1, 1), pt("B", 5, 3), "s = Segment(A, B)", "h = PerpendicularBisector(s)"] + on("h"),
     [mv("P", 0, 6), mv("A", -1, 3), mv("B", 3, 5)])
# angle bisector of three points: the vertex is the start point
case("start", [pt("A", 4, 0), pt("B", 0, 0), pt("C", 0, 3), "h = AngleBisector(A, B, C)"] + on("h"),
     [mv("P", 2, 5), mv("B", 1, -1), mv("C", -3, 1), mv("A", -2, -2)])
# angle bisectors of two lines: their intersection is the start point
case("start", [pt("A", 0, 0), pt("B", 4, 1), pt("C", 1, 3), pt("D", 2, -2), "g = Line(A, B)", "l = Line(C, D)",
               "h = AngleBisector(g, l)", "P = Point(h_{1})", "t = PathParameter(P)", "Q = Point(h_{2})",
               "w = PathParameter(Q)"],
     [mv("P", 5, 5), mv("Q", -3, 4), mv("A", -1, 1), mv("D", 3, -3), mv("C", 0, 4)])
# tangents through a point: the point is the start point
case("start", [pt("M", 0, 0), pt("R", 2, 0), "k = Circle(M, R)", pt("Q", 5, 1), "h = Tangent(Q, k)",
               "P = Point(h_{1})", "t = PathParameter(P)", "U = Point(h_{2})", "w = PathParameter(U)"],
     [mv("P", 0, 6), mv("Q", 4, 4), mv("R", 3, 0), mv("U", -2, -3), mv("Q", 6, -1)])
# tangents parallel to a line: the points of tangency are the start points
case("start", [pt("M", 0, 0), pt("R", 2, 0), "k = Circle(M, R)", pt("A", -3, 3), pt("B", 3, 4), "g = Line(A, B)",
               "h = Tangent(g, k)", "P = Point(h_{1})", "t = PathParameter(P)", "U = Point(h_{2})",
               "w = PathParameter(U)"],
     [mv("P", 1, 6), mv("B", 3, 2), mv("R", 1, 0), mv("U", -4, -3), mv("M", 1, 1)])
# lines through a point along a vector
case("start", [pt("A", 0, 0), pt("B", 2, 1), "v = Vector(A, B)", pt("C", 1, 3), "h = Line(C, v)"] + on("h")
     + at("h", 0.2), [mv("P", 4, 0), mv("C", 3, 4), mv("B", -1, 2), setn("u", 0.9)])
case("start", [pt("A", 0, 0), pt("B", 2, 1), "v = Vector(A, B)", pt("C", 1, 3), "h = PerpendicularLine(C, v)"]
     + on("h"), [mv("P", 4, 0), mv("C", 3, 4), mv("B", -1, 2)])
# lines without a start point: a polar and a reflected line
case("start", [pt("M", 0, 0), pt("R", 2, 0), "k = Circle(M, R)", pt("Q", 5, 1), "h = Polar(Q, k)"] + on("h"),
     [mv("P", 3, 3), mv("Q", 1, 4), mv("R", 3, 0)])
case("start", [pt("A", 0, 0), pt("B", 4, 1), "g = Line(A, B)", pt("C", 1, 3), "h = Reflect(g, C)"] + on("h")
     + at("h", 0.35), [mv("P", 0, 0), mv("C", -2, 3), mv("B", 1, 5), setn("u", 0.65)])

# ---------------------------------------------------------------- segments
topic("segment")
for _ in range(6):
    script = [pt("A", *rp()), pt("B", *rp()), "s = Segment(A, B)"] + on("s") + at("s", u01())
    case("segment", script, [mv("P", *rp()), mv("A", *rp()), mv("P", *rp()), mv("B", *rp()),
                             setn("u", u01())])
# beyond the end points the point stays at the nearer one
case("segment", [pt("A", 0, 0), pt("B", 4, 2), "s = Segment(A, B)"] + on("s") + at("s", 0.25),
     [mv("P", 9, 5), mv("P", -3, -4), mv("P", 2, 1), setn("u", 1.5), setn("u", -1), setn("u", 0.5)])
# length 0 and back: the point stays at the parameter it had
case("segment", [pt("A", 0, 0), pt("B", 4, 2), "s = Segment(A, B)"] + on("s") + at("s", 0.25),
     [mv("P", 3, 1), mv("B", 0, 0), mv("B", 2, 0), mv("P", 7, 0), mv("B", 0, 0), mv("P", 1, 1), mv("B", 0, 3)])
# nearly length 0 (below the precision of GeoGebra's zero test)
case("segment", [pt("A", 1, 1), pt("B", 1.000001, 1), "s = Segment(A, B)"] + on("s"),
     [mv("P", 3, 3), mv("B", 1, 3), mv("P", 0, 2)])
# vertical segment
case("segment", [pt("A", 2, -1), pt("B", 2, 4), "s = Segment(A, B)"] + on("s"),
     [mv("P", 0, 1), mv("P", 3, 9), mv("A", 2, 6)])

# -------------------------------------------------------------------- rays
topic("ray")
for _ in range(4):
    script = [pt("A", *rp()), pt("B", *rp()), "r = Ray(A, B)"] + on("r") + at("r", u01())
    case("ray", script, [mv("P", *rp()), mv("A", *rp()), mv("P", *rp()), mv("B", *rp()), setn("u", u01())])
# behind the start point the point stays at it
case("ray", [pt("A", 0, 0), pt("B", 2, 1), "r = Ray(A, B)"] + on("r"),
     [mv("P", -4, -1), mv("P", 6, 4), mv("B", -2, 1), mv("B", 0, 0), mv("B", 1, 1)])
case("ray", [pt("A", 1, 1), pt("B", 3, 2), "v = Vector(A, B)", pt("C", 0, 2), "r = Ray(C, v)"] + on("r")
     + at("r", 0.5), [mv("P", 4, 4), mv("C", -1, 0), mv("B", 1, 4), setn("u", 0.1)])

# ----------------------------------------------------------------- circles
topic("circle")
for _ in range(5):
    script = [pt("M", *rp(-4, 4)), pt("R", *rp(-4, 4)), "k = Circle(M, R)"] + on("k") + at("k", u01())
    case("circle", script, [mv("P", *rp()), mv("M", *rp(-4, 4)), mv("P", *rp()), mv("R", *rp(-4, 4)),
                            setn("u", u01())])
# radius 0 (a single point) and back: the point returns to its parameter
case("circle", [pt("A", 1, 1), pt("B", 4, 1), "k = Circle(A, B)"] + on("k") + at("k", 0.3),
     [mv("P", 1, 5), mv("B", 1, 1), mv("B", 3, 1), mv("P", -5, 1), mv("B", 1, 1), mv("P", 7, 7), mv("B", 1, 4)])
# a radius given by a number: 0, negative (undefined) and back
case("circle", [pt("M", 0, 0), num("r", 2), "k = Circle(M, r)"] + on("k") + at("k", 0.8),
     [mv("P", 0, 5), setn("r", 0), setn("r", 3), setn("r", -1), setn("r", 1), mv("P", -1, -1)])
# the point moved to the centre: the angle of (0, 0)
case("circle", [pt("M", 1, 2), num("r", 2), "k = Circle(M, r)"] + on("k"),
     [mv("P", 1, 2), mv("P", 1, 2.000001), mv("P", 5, 2)])
# circles through three points, which become collinear and come back
case("circle", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 1, 3), "k = Circle(A, B, C)"] + on("k") + at("k", 0.6),
     [mv("P", 5, 5), mv("C", 1, 2), mv("A", 1, -1), mv("P", -3, 0), mv("B", 2, 1)])
case("circle", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 1, 3), "k = Incircle(A, B, C)"] + on("k"),
     [mv("P", 5, 5), mv("C", 1, 2), mv("A", -1, -1)])

# ---------------------------------------------------------------- polygons
topic("polygon")
case("polygon", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 1, 3), "q = Polygon(A, B, C)"] + on("q") + at("q", 0.5),
     [mv("P", 5, 5), mv("P", -1, 1), mv("B", 6, 1), mv("P", 3, -2), setn("u", 0.95), setn("u", 0.1)])
case("polygon", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 4, 3), pt("D", 0, 3), "q = Polygon(A, B, C, D)"]
     + on("q") + at("q", 0.6), [mv("P", 2, 2), mv("P", 9, 1), mv("D", -1, 5), mv("P", -3, 4), setn("u", 1)])
# a vertex moves parallel to the opposite side: the area stays, the point follows
case("polygon", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 1, 3), "q = Polygon(A, B, C)"] + on("q") + at("q", 0.6),
     [mv("P", 1, 2.5), mv("C", 2, 3), mv("C", 3, 3), mv("C", -1, 3)])
# coincident vertices: a side of length 0
case("polygon", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 1, 3), "q = Polygon(A, B, C)"] + on("q") + at("q", 0.4),
     [mv("P", 3, 3), mv("B", 0, 0), mv("P", 5, -1), mv("B", 4, 0), mv("C", 0, 0), mv("P", 1, 1)])
# a regular polygon
case("polygon", [pt("A", 0, 0), pt("B", 2, 0), num("n", 5), "q = Polygon(A, B, n)"] + on("q") + at("q", 0.3),
     [mv("P", 3, 3), mv("B", 1, 2), setn("n", 3), mv("P", -2, 0), setn("n", 6)])
# a point on the corner of two sides, and a self-intersecting polygon
case("polygon", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 4), pt("D", 4, 4), "q = Polygon(A, B, C, D)"]
     + on("q"), [mv("P", 5, -1), mv("P", 2, 2), mv("P", 2, 2.5), mv("C", 1, 5)])
for _ in range(3):
    script = [pt("A", *rp()), pt("B", *rp()), pt("C", *rp()), pt("D", *rp()), "q = Polygon(A, B, C, D)"] \
        + on("q") + at("q", u01())
    case("polygon", script, [mv("P", *rp()), mv("A", *rp()), mv("P", *rp()), mv("C", *rp()), setn("u", u01())])

# --------------------------------------------------------------- polylines
topic("polyline")
case("polyline", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 4, 3), "l = PolyLine(A, B, C)"] + on("l") + at("l", 0.5),
     [mv("P", 5, 5), mv("P", -2, 1), mv("B", 2, -2), setn("u", 1), setn("u", 0), setn("u", 0.75)])
case("polyline", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 4, 3), pt("D", 0, 3), "l = PolyLine(A, B, C, D)"]
     + on("l") + at("l", 1), [mv("P", -1, 4), mv("D", -2, 2), mv("P", 2, 5), setn("u", 0.4)])
# coincident points
case("polyline", [pt("A", 0, 0), pt("B", 4, 0), pt("C", 4, 3), "l = PolyLine(A, B, C)"] + on("l"),
     [mv("P", 3, 1), mv("C", 4, 0), mv("P", 9, 9), mv("C", 1, 5)])
for _ in range(3):
    script = [pt("A", *rp()), pt("B", *rp()), pt("C", *rp()), pt("D", *rp()), "l = PolyLine(A, B, C, D)"] \
        + on("l") + at("l", u01())
    case("polyline", script, [mv("P", *rp()), mv("B", *rp()), mv("P", *rp()), mv("D", *rp()), setn("u", u01())])

# --------------------------------------------------- parameters of other points
topic("closest")
case("closest", [pt("A", 0, 0), pt("B", 4, 2), "g = Line(A, B)", pt("Q", 1, 4), "C = ClosestPoint(g, Q)",
                 "t = PathParameter(C)"], [mv("Q", -3, 2), mv("B", 1, 5), mv("A", 2, 0)])
case("closest", [pt("A", 0, 0), pt("B", 4, 2), "s = Segment(A, B)", pt("Q", 1, 4), "C = ClosestPoint(s, Q)",
                 "t = PathParameter(C)"], [mv("Q", 9, 2), mv("Q", -3, -2), mv("B", 1, 5)])
case("closest", [pt("A", 0, 0), pt("B", 4, 2), "r = Ray(A, B)", pt("Q", 1, 4), "C = ClosestPoint(r, Q)",
                 "t = PathParameter(C)"], [mv("Q", -3, -2), mv("Q", 9, 2)])
case("closest", [pt("M", 0, 0), num("r", 2), "k = Circle(M, r)", pt("Q", 1, 4), "C = ClosestPoint(k, Q)",
                 "t = PathParameter(C)", pt("F", 3, 3), "pf = PathParameter(F)"],
     [mv("Q", -3, -2), setn("r", 4), mv("Q", 0, -1)])

# ------------------------------------------------ points on paths used further
topic("chain")
# a point on a segment whose end point is a point on a circle
case("chain", [pt("M", 0, 0), num("r", 3), "k = Circle(M, r)", "P = Point(k)", pt("A", 5, 5),
               "s = Segment(A, P)", "Q = Point(s)", "t = PathParameter(Q)", "g = Line(P, Q)", "m = Midpoint(P, Q)"],
     [mv("P", 0, 5), mv("Q", 0, 0), setn("r", 1), mv("A", -4, 1), mv("P", 3, -3)])
# a circle through a point on a line, and a point on that circle
case("chain", [pt("A", 0, 0), pt("B", 3, 1), "g = Line(A, B)", "P = Point(g)", pt("C", 2, 4),
               "k = Circle(C, P)", "Q = Point(k)", "t = PathParameter(Q)", "d = Distance(P, Q)"],
     [mv("P", 5, 0), mv("Q", 10, 10), mv("C", -1, 2), mv("B", -1, 3)])
# a point on a polygon with a vertex on a path
case("chain", [pt("A", 0, 0), pt("B", 4, 0), "s = Segment(A, B)", "V = Point(s)", pt("C", 1, 3),
               "q = Polygon(A, V, C)", "P = Point(q)", "t = PathParameter(P)"],
     [mv("V", 3, 3), mv("P", 5, 0), mv("B", 2, 0), mv("V", -1, 0)])

cases.write()
