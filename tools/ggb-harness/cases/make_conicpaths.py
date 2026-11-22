#!/usr/bin/env python3
"""Writes cases/conicpaths.json: points on conic paths and loci driven by them.

`Point` on an ellipse, hyperbola, parabola or a degenerate conic (two lines,
a double line, a single point) follows GeoGebra's conic path behaviour: the
move of the point projects it onto the conic (`GeoConicND.pointChanged`,
which picks the nearer branch of a hyperbola or the nearer line of two), and
a move of the conic's defining points puts the point back at the parameter it
kept (`GeoConicND.pathChanged`, which re-projects only when the conic changed
to an incompatible type). `Point(conic, u)` takes `u` from 0 to 1 over the
conic's parameter range (`toParentPathParameter`), and `PathParameter` reads
it back.

The cases move the point itself (across the branches of a hyperbola, across
the crossing of two lines, to the centre of an ellipse), the foci, the point
an ellipse or hyperbola runs through, the focus and the directrix of a
parabola, and the points of a conic through five points, so that the conic
degenerates (an ellipse whose through-point crosses between its foci, a
parabola whose directrix meets its focus, five points with three on one
line) and comes back while the point keeps its parameter.

The `locus` topic drives `Locus(Q, P)` by a point on an ellipse, a parabola
and a hyperbola; those cases go to their own files here, not to the `locus`
group, which is append-only.

New cases go at the end of their topic and new topics at the end of the file,
so that existing ids keep their meaning.
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("conicpaths")
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


# ----------------------------------------------------------------- ellipses
topic("ellipse")
for _ in range(3):
    f1, f2, q = rp(-5, 2), rp(2, 5), rp(-3, 3)
    script = [pt("F", *f1), pt("G", *f2), pt("R", *q), "k = Ellipse(F, G, R)"] + on("k") + at("k", u01())
    case("ellipse", script, [mv("P", *rp()), mv("F", *rp(-5, 2)), mv("P", *rp()), mv("G", *rp(2, 5)),
                             mv("R", *rp(-3, 3)), setn("u", u01()), mv("P", *rp())])
# a rotated ellipse, and the point moved to its centre: the angle of (0, 0)
case("ellipse", [pt("F", -1, -2), pt("G", 1, 2), pt("R", 0, 2.5), "k = Ellipse(F, G, R)"] + on("k"),
     [mv("P", 2, 4), mv("P", 0, 0), mv("P", 0.000001, 0.000001), mv("R", -1, 2.5), mv("P", -3, -1)])
# the foci meet: the ellipse becomes a circle and the point is projected again
case("ellipse", [pt("F", -2, 0), pt("G", 2, 0), pt("R", 0, 1.5), "k = Ellipse(F, G, R)"] + on("k") + at("k", 0.7),
     [mv("P", 0, 2), mv("F", 0, 0), mv("P", 1, 1), mv("F", -1, 1), setn("u", 0.2)])
# the through-point crosses between the foci: the ellipse degenerates and comes back
case("ellipse", [pt("F", -3, 0), pt("G", 3, 0), pt("R", 0, 2), "k = Ellipse(F, G, R)"] + on("k") + at("k", 0.3),
     [mv("P", 3, 2), mv("R", 0, 1), mv("R", 0, 0), mv("R", 0, -2), mv("P", 7, 7), mv("R", 0, 1),
      mv("P", -3, 1), mv("R", 0, 3)])
# an ellipse from a number (the sum of the distances): the number shrinks the
# ellipse onto the segment of the foci and widens it again
case("ellipse", [pt("F", -2, 0), pt("G", 2, 0), num("s", 6), "k = Ellipse(F, G, s)"] + on("k") + at("k", 0.6),
     [mv("P", 0, 3), setn("s", 4), mv("P", 5, 0), setn("s", 4.0000001), setn("s", 4), setn("s", 8),
      mv("G", 1, 1), mv("P", 0, -4)])
# a point at the parameters 0, 1 and beyond: the parameter is clamped
case("ellipse", [pt("F", -2, 0), pt("G", 2, 0), pt("R", 0, 1.5), "k = Ellipse(F, G, R)"] + at("k", 0.5),
     [setn("u", 0), setn("u", 1), setn("u", 1.5), setn("u", -0.5), setn("u", 0.25), mv("G", 3, 1)])

# --------------------------------------------------------------- hyperbolas
topic("hyperbola")
for _ in range(3):
    f1, f2, q = rp(-5, -1), rp(1, 5), rp(-3, 3)
    script = [pt("F", *f1), pt("G", *f2), pt("R", *q), "k = Hyperbola(F, G, R)"] + on("k") + at("k", u01())
    case("hyperbola", script, [mv("P", *rp()), mv("R", *rp(-3, 3)), mv("P", *rp()), mv("F", *rp(-5, -1)),
                               setn("u", u01()), mv("P", *rp())])
# the point switches branch: near the left branch, the centre, the right branch
case("hyperbola", [pt("F", -2, 0), pt("G", 2, 0), pt("R", 3, 1.5), "k = Hyperbola(F, G, R)"] + on("k"),
     [mv("P", -3, 1), mv("P", -3, -1), mv("P", 0, 0), mv("P", 0.000001, 0), mv("P", 4, 2), mv("P", 4, -2),
      mv("P", 1000, 1), mv("P", -1000, -1)])
# the foci trade places
case("hyperbola", [pt("F", -2, 0), pt("G", 2, 0), pt("R", 3, 1.5), "k = Hyperbola(F, G, R)"] + on("k"),
     [mv("P", 4, 2), mv("G", -3, 1), mv("F", 3, -1), mv("P", -4, 2), mv("R", -3, 1.5)])
# the through-point crosses onto the segment of the foci: the hyperbola
# degenerates (two rays) and comes back
case("hyperbola", [pt("F", -2, 0), pt("G", 2, 0), pt("R", 3, 1.5), "k = Hyperbola(F, G, R)"] + on("k"),
     [mv("P", 4, 2), mv("R", 0.5, 0), mv("R", 1, 0), mv("P", 7, 7), mv("R", 0.5, 2), mv("P", -4, 2),
      mv("R", 3, 1.5)])
# a hyperbola from a number (the difference of the distances): the number
# grows to the distance of the foci (the rays) and past it (nothing)
case("hyperbola", [pt("F", -2, 0), pt("G", 2, 0), num("s", 3), "k = Hyperbola(F, G, s)"] + on("k"),
     [mv("P", 4, 2), setn("s", 4), mv("P", 7, 7), setn("s", 4.0000001), setn("s", 4), setn("s", 1),
      mv("P", -4, -2), mv("G", 0, 2)])
# a point at parameters over both branches; the middle of the range is the
# point at infinity between them
case("hyperbola", [pt("F", -2, 0), pt("G", 2, 0), num("s", 3), "k = Hyperbola(F, G, s)"] + at("k", 0.1),
     [setn("u", 0.75), setn("u", 0.5), setn("u", 0.5001), setn("u", 0.9), setn("u", 0), setn("u", 1)])

# ---------------------------------------------------------------- parabolas
topic("parabola")
for _ in range(3):
    a, b, f = rp(-4, 4), rp(-4, 4), rp(-3, 3)
    script = [pt("A", *a), pt("B", *b), "g = Line(A, B)", pt("F", *f), "k = Parabola(F, g)"] + on("k") + at("k", u01())
    case("parabola", script, [mv("P", *rp()), mv("F", *rp(-3, 3)), mv("P", *rp()), mv("A", *rp(-4, 4)),
                              setn("u", u01()), mv("B", *rp(-4, 4))])
# the point at the vertex, and far out on both arms
case("parabola", [pt("A", -1, -2), pt("B", 1, -2), "g = Line(A, B)", pt("F", 0, 1), "k = Parabola(F, g)"] + on("k"),
     [mv("P", 0, -2), mv("P", 5, 7), mv("P", -5, 7), mv("P", 1000, 1000), mv("P", 0.000001, -2)])
# the directrix meets the focus: the parabola degenerates and comes back
case("parabola", [pt("A", -1, -2), pt("B", 1, -2), "g = Line(A, B)", pt("F", 0, 1), "k = Parabola(F, g)"]
     + on("k") + at("k", 0.6),
     [mv("P", 3, -2), mv("F", 0, -2), mv("P", 7, 7), mv("F", 0, 1), mv("P", -3, -2), mv("A", -1, -3)])
# a point at the parameters 0, 1 and the middle (the vertex)
case("parabola", [pt("A", -1, -2), pt("B", 1, -2), "g = Line(A, B)", pt("F", 0, 1), "k = Parabola(F, g)"]
     + at("k", 0.5),
     [setn("u", 0.25), setn("u", 0.75), setn("u", 0), setn("u", 1), setn("u", -1), setn("u", 2), setn("u", 0.5)])

# -------------------------------------------------------- degenerate conics
topic("degenerate")
# two lines through a point: three of the five points on one line, the other
# two on a second one; the point takes the nearer line and switches as it or
# the lines move, and the conic comes back into one piece
line1 = [pt("A", -3, -1), pt("B", 0, 0), pt("C", 3, 1)]
case("degenerate", line1 + [pt("D", 1, 3), pt("E", 2, -2), "k = Conic(A, B, C, D, E)"] + on("k"),
     [mv("P", 0, 3), mv("P", 2, -3), mv("P", 1, 1), mv("P", -2, 1), mv("D", 0, 2), mv("E", 0, -2),
      mv("P", 4, 4), mv("C", 4, 2), mv("C", 3, 1)])
case("degenerate", line1 + [pt("D", 1, 3), pt("E", 2, -2), "k = Conic(A, B, C, D, E)"] + on("k") + at("k", 0.2),
     [setn("u", 0.4), setn("u", 0.6), setn("u", 0.8), mv("B", 1, 1), setn("u", 0.3)])
# two parallel lines
case("degenerate", [pt("A", -2, 0), pt("B", 0, 0), pt("C", 2, 0), pt("D", -2, 1), pt("E", 2, 1),
                    "k = Conic(A, B, C, D, E)"] + on("k"),
     [mv("P", 1, 2), mv("P", 1, 0.5), mv("P", 1, -1), mv("P", 100, 100), mv("D", -2, -1), mv("P", 1, 0.5)])
# a double line (all five points on one line)
case("degenerate", [pt("A", -2, 0), pt("B", -1, 0), pt("C", 0, 0), pt("D", 1, 0), pt("E", 2, 0),
                    "k = Conic(A, B, C, D, E)"] + on("k"),
     [mv("P", 5, 5), mv("P", -5, -5), mv("C", 0, 2), mv("P", 3, 3), mv("C", 0, 0), mv("P", 1, 1)])
# a single point: x^2 + y^2 = 0, which widens into a circle when the
# constant is set
case("degenerate", [num("a", 0), "k: x^2 + y^2 = a"] + on("k"),
     [mv("P", 5, 5), setn("a", 4), mv("P", 1, 1), setn("a", 0), mv("P", -3, -3), setn("a", 1)])
# an empty conic: x^2 + y^2 = -1, defined again by a move
case("degenerate", [pt("A", -2, 0), pt("B", 0, 1), pt("C", 2, 0), pt("D", 0, -1), pt("E", 1, 1),
                    "k = Conic(A, B, C, D, E)"] + on("k"),
     [mv("P", 1, 1), mv("E", 0, 1), mv("P", 7, 7), mv("E", 1, 1), mv("P", -2, 0), mv("C", 2, 1)])

# ------------------------------------------------ loci driven by conic paths
topic("locus")
# the midpoints of a point on an ellipse and a free point: an ellipse again
case("locus", [pt("F", -2, 0), pt("G", 2, 0), pt("R", 0, 1.5), "k = Ellipse(F, G, R)", "P = Point(k)",
               pt("B", 5, 0), "Q = Midpoint(P, B)", "L = Locus(Q, P)"],
     [mv("P", 0, 2), mv("F", -1, 0), mv("B", -1, 2), mv("G", 3, 1)])
# a point on a parabola: the locus is a parabola
case("locus", [pt("A", -1, -2), pt("B", 1, -2), "g = Line(A, B)", pt("F", 0, 1), "k = Parabola(F, g)",
               "P = Point(k)", pt("C", 2, 3), "Q = Midpoint(P, C)", "L = Locus(Q, P)"],
     [mv("P", 3, 1), mv("A", -1, -3), mv("C", -2, 0), mv("F", 0, 2)])
# a point on a hyperbola: the locus runs over both branches in two pieces
case("locus", [pt("F", -2, 0), pt("G", 2, 0), num("s", 3), "k = Hyperbola(F, G, s)", "P = Point(k)",
               pt("B", 5, 0), "Q = Midpoint(P, B)", "L = Locus(Q, P)"],
     [mv("P", -4, 2), mv("B", 0, 2), mv("G", 3, 0), setn("s", 2)])
# the image of a point on an ellipse in a point: a whole ellipse through the view
case("locus", [pt("F", -2, 0), pt("G", 2, 0), pt("R", 0, 1.5), "k = Ellipse(F, G, R)", "P = Point(k)",
               pt("O", 0, 0), num("c", 3), "Q = Dilate(P, c, O)", "L = Locus(Q, P)"],
     [setn("c", -2), mv("O", 1, 1), setn("c", 0.5), mv("R", 0, 2)])
# a locus driven by a point on an ellipse that degenerates (the through-point
# crosses between the foci) and comes back
case("locus", [pt("F", -3, 0), pt("G", 3, 0), pt("R", 0, 2), "k = Ellipse(F, G, R)", "P = Point(k)",
               pt("B", 6, 0), "Q = Midpoint(P, B)", "L = Locus(Q, P)"],
     [mv("P", 3, 2), mv("R", 0, 0), mv("R", 0, 2), mv("F", -2, 0), mv("P", 0, -2)])

cases.write()
