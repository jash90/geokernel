#!/usr/bin/env python3
"""Writes cases/locus.json: `Locus(Q, P)` and `Locus(Q, slider)`.

A locus is the list of points the point `Q` takes while the point `P` runs
along its path, or while a slider runs through its interval, each with the
flag that tells whether it is joined to the point before it. GeoGebra picks
the samples by measuring the steps in pixels of the graphics view, so every
snapshot depends on the window of the headless application; the harness
checks that window whenever it records a locus (see golden/SCHEMA.md).

The cases put `P` on every kind of path a point can be put on here (lines,
rays, segments, circles, arcs, polygons and polylines), with `P` at an end
of a segment, `Q` undefined on part of the path or at the start, a point on
a path between `P` and `Q` (the locus is undefined), and loci far from the
view. The steps move `P` (a locus does not follow it), the inputs of the
path and the other free objects `Q` depends on, and set the slider and the
numbers `Q` reads.

New cases go at the end of their topic and new topics at the end of the file,
so that existing ids keep their meaning.
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("locus")
case = cases.case
topic = cases.topic
rp = cases.rp


def mid(path, other="C", at=(1, 3)):
    """`P` on `path`, `Q` the midpoint of `P` and another free point, and the locus."""
    return [f"P = Point({path})", pt(other, *at), f"Q = Midpoint(P, {other})", "L = Locus(Q, P)"]


# ----------------------------------------------------------------- circles
topic("circle")
case("circle", [pt("A", 1, 1), num("r", 2), "c = Circle(A, r)"] + mid("c", "B", (5, 0)),
     [mv("P", 3, 1), mv("A", 2, 1), mv("B", -1, 0), setn("r", 3)])
case("circle", [pt("A", 0, 0), pt("D", 3, 0), "c = Circle(A, D)"] + mid("c", "B", (4, 3)),
     [mv("D", 0, 2), mv("P", -2, 0), mv("A", 1, -1)])
for _ in range(3):
    case("circle", [pt("A", *rp(-3, 6)), num("r", round(cases.rng.uniform(0.5, 4), 2)), "c = Circle(A, r)"]
         + mid("c", "B", rp(-3, 9)), [mv("B", *rp(-3, 9)), mv("A", *rp(-3, 6))])
# the radius becomes 0 and comes back
case("circle", [pt("A", 1, 1), num("r", 2), "c = Circle(A, r)"] + mid("c", "B", (5, 0)),
     [setn("r", 0), setn("r", 1)])
# Q is the image of P in a point: a circle through the whole view
case("circle", [pt("A", 2, 0), num("r", 3), "c = Circle(A, r)", "P = Point(c)", pt("O", 0, 0),
                num("k", 3), "Q = Dilate(P, k, O)", "L = Locus(Q, P)"],
     [setn("k", -2), mv("O", 1, 1), setn("k", 0.5)])

# ----------------------------------------------------------------- segments
topic("segment")
case("segment", [pt("A", 0, 0), pt("B", 4, 2), "s = Segment(A, B)"] + mid("s"),
     [mv("B", 5, -1), mv("P", 1, 1), mv("A", -2, 0)])
# P at an end of its segment when the locus is computed again
case("segment", [pt("A", 0, 0), pt("B", 4, 2), "s = Segment(A, B)"] + mid("s"),
     [mv("P", 0, 0), mv("C", 2, 3), mv("P", 4, 2), mv("B", 3, 3)])
case("segment", [pt("A", 0, 0), pt("B", 4, 2), "s = Segment(A, B)", num("u", 0), "P = Point(s, u)", pt("C", 1, 3),
                 "Q = Midpoint(P, C)", "L = Locus(Q, P)"],
     [mv("C", 2, -2), setn("u", 0.5), mv("B", -3, 1)])
case("segment", [pt("A", 0, 0), pt("B", 4, 2), "s = Segment(A, B)", num("u", 1), "P = Point(s, u)", pt("C", 1, 3),
                 "Q = Midpoint(P, C)", "L = Locus(Q, P)"],
     [mv("A", 2, 4), mv("C", -1, 0)])
# the segment shrinks to a point and comes back
case("segment", [pt("A", 0, 0), pt("B", 4, 2), "s = Segment(A, B)"] + mid("s"),
     [mv("B", 0, 0), mv("B", 3, 3)])
# Q also depends on a point R on another path, which the locus follows
case("segment", [pt("A", -2, -1), pt("B", 6, 1), "s = Segment(A, B)", "P = Point(s)", pt("D", 0, 2), num("r", 1),
                 "c = Circle(D, r)", "R = Point(c)", "Q = Midpoint(P, R)", "L = Locus(Q, P)"],
     [mv("R", 1, 2), mv("D", 2, 3), mv("P", 6, 1)])
# Q is the midpoint of a segment whose end is P
case("segment", [pt("A", -2, -1), pt("B", 6, 1), "s = Segment(A, B)", "P = Point(s)", pt("C", 2, 4),
                 "t = Segment(P, C)", "Q = Midpoint(t)", "L = Locus(Q, P)"],
     [mv("C", 2, -3)])

# ------------------------------------------------------ lines and rays
topic("line")
case("line", [pt("A", 0, 0), pt("B", 4, 2), "l = Line(A, B)"] + mid("l"),
     [mv("P", 2, 1), mv("B", 1, 4), mv("C", -2, -2)])
case("line", [pt("A", 1, 0), pt("B", 4, 2), "h = Ray(A, B)"] + mid("h"),
     [mv("A", -1, -1), mv("B", -3, 2)])
case("line", [pt("A", 0, 0), pt("B", 4, 0), "g = Line(A, B)", pt("D", 1, 2), "h = PerpendicularLine(D, g)"]
     + mid("h", "C", (3, 3)), [mv("D", 2, 2), mv("B", 4, 1)])
# a locus far outside the view, whose steps pass near the origin
case("line", [pt("A", 0, 0), pt("B", 1, 1), "l = Line(A, B)", "P = Point(l)", pt("O", 0, 0), num("k", 10),
              "Q = Dilate(P, k, O)", pt("E", 0, 40), "R = Midpoint(Q, E)", "L = Locus(R, P)"],
     [setn("k", 2), mv("E", 0, 4), setn("k", -30)])
# the line becomes undefined (A on B) and comes back
case("line", [pt("A", 0, 0), pt("B", 4, 2), "l = Line(A, B)"] + mid("l"),
     [mv("B", 0, 0), mv("B", 2, -2)])
# Sampling gives each transformed line its own fixed start point.
line = [pt("A", 0, 0), pt("B", 4, 2), "l = Line(A, B)"]
case("line", line + [pt("C", 1, 1), pt("D", 3, 0), "u = Vector(C, D)", "m = Translate(l, u)"]
     + mid("m", "E"), [mv("A", 1, 1), mv("C", 0, 0), mv("B", 0, 3)])
case("line", line + [pt("O", 0, 1), num("k", 2), "m = Dilate(l, k, O)"]
     + mid("m", "E"), [mv("A", 1, 1), mv("O", 0, 0), setn("k", 3), mv("B", 0, 3)])
case("line", line + [pt("C", 1, 3), num("w", 0.7), "m = Rotate(l, w, C)"]
     + mid("m", "E"), [mv("A", 1, 1), mv("B", 0, 3), mv("C", 0, 0), setn("w", 1.1)])
case("line", line + [pt("C", 1, 3), "m = Reflect(l, C)"]
     + mid("m", "E"), [mv("A", 1, 1), mv("C", 0, 0), mv("B", 0, 3)])

# ------------------------------------------------------ arcs
topic("arc")
case("arc", [pt("M", 1, 1), pt("B", 4, 1), pt("D", 1, 4), "g = CircularArc(M, B, D)"] + mid("g", "C", (5, 4)),
     [mv("D", -2, 1), mv("C", 0, 0)])
case("arc", [pt("M", 1, 1), pt("B", 4, 1), pt("D", 1, 4), "g = CircularSector(M, B, D)"] + mid("g", "C", (5, 4)),
     [mv("B", 1, -2)])
case("arc", [pt("A", 0, 0), pt("B", 2, 2), pt("D", 4, 0), "g = CircumcircularArc(A, B, D)"]
     + mid("g", "C", (2, -3)), [mv("B", 2, -1)])

# ------------------------------------------------- polygons and polylines
topic("polygon")
case("polygon", [pt("A", 0, 0), pt("B", 4, 0), pt("D", 2, 3), "t = Polygon(A, B, D)"] + mid("t", "C", (-2, 1)),
     [mv("D", 2, 4), mv("P", 4, 0), mv("C", 6, 5)])
case("polygon", [pt("A", 0, 0), pt("B", 4, 0), pt("D", 2, 3), "t = PolyLine(A, B, D)"] + mid("t", "C", (-2, 1)),
     [mv("B", 5, 2)])
# a polyline that ends where it starts is closed
case("polygon", [pt("A", 0, 0), pt("B", 4, 0), pt("D", 2, 3), "t = PolyLine(A, B, D, A)"]
     + mid("t", "C", (-2, 1)), [mv("A", 1, -1)])
case("polygon", [pt("A", -1, -1), pt("B", 3, -1), pt("D", 3, 2), pt("E", -1, 2), "t = Polygon(A, B, D, E)",
                 "P = Point(t)", pt("O", 1, 0), num("k", -1), "Q = Dilate(P, k, O)", "L = Locus(Q, P)"],
     [setn("k", 2), mv("E", -2, 3)])

# --------------------------------------------- undefined parts of a locus
topic("undefined")
# Q exists only where the perpendicular through P meets the circle
case("undefined", [pt("A", -3, 0), pt("B", 3, 0), "s = Segment(A, B)", "P = Point(s)", "k = PerpendicularLine(P, s)",
                   pt("O", 0, 0), num("r", 2), "c = Circle(O, r)", "Q = Intersect(k, c, 1)", "L = Locus(Q, P)"],
     [setn("r", 1), mv("O", 1, 0), setn("r", 4)])
# Q runs to infinity where x(P) = 0
case("undefined", [pt("A", -3, 0), pt("B", 3, 0), "s = Segment(A, B)", "P = Point(s)",
                   "Q = (x(P), 1 / x(P))", "L = Locus(Q, P)"],
     [mv("B", 3, 1), mv("A", 1, 0)])
# Q is undefined where P is when the locus is computed again
case("undefined", [pt("A", -3, 0), pt("B", 3, 0), "s = Segment(A, B)", "P = Point(s)",
                   "Q = (x(P), 1 / x(P))", "L = Locus(Q, P)"],
     [mv("P", 0, 0), mv("B", 3, 0.5)])
# a point on a path between P and Q: the locus is undefined
case("undefined", [pt("A", 1, 1), num("r", 2), "c = Circle(A, r)", "P = Point(c)", pt("B", 5, 0),
                   "k = Line(P, B)", "R = Point(k)", "Q = Midpoint(R, B)", "L = Locus(Q, P)"],
     [mv("B", 4, 2), mv("A", 0, 0)])

# ----------------------------------------------------------------- sliders
topic("slider")
case("slider", ["a = Slider(-2, 2)", "Q = (a, a^2 / 2)", "L = Locus(Q, a)"],
     [setn("a", 1), setn("a", -2)])
case("slider", ["a = Slider(1, 4)", pt("A", 1, 1), pt("B", 2, 0), "Q = Dilate(B, a, A)", "L = Locus(Q, a)"],
     [mv("A", 0, 0), setn("a", 3), mv("B", -1, 2)])
case("slider", ["a = Slider(-5, -1)", "Q = (a, 1 / (a + 3))", "L = Locus(Q, a)"],
     [setn("a", -3)])
case("slider", ["a = Slider(-2, 2, 0.5)", pt("A", 2, 1), num("r", 2), "c = Circle(A, r)", "P = Point(c)",
                "Q = (x(P) + a, y(P) + a^2)", "L = Locus(Q, a)"],
     [mv("P", 2, 3), mv("A", 0, 0), setn("r", 1)])
# Q exists only for part of the interval
case("slider", ["a = Slider(-3, 3)", pt("O", 0, 0), num("r", 2), "c = Circle(O, r)", pt("D", 0, -1),
                "E = (a, 0)", "k = Line(D, E)", "Q = Intersect(k, c, 1)", "L = Locus(Q, a)"],
     [mv("D", 0, 3), setn("r", 4)])
# a locus far outside the view
case("slider", ["a = Slider(0, 10)", pt("A", 1, 1), "Q = (20 * a, a^2)", "L = Locus(Q, a)"],
     [setn("a", 5)])
case("slider", ["a = Slider(2, 2)", "Q = (a, a^2 / 2)", "L = Locus(Q, a)"], [])
case("slider", ["a = Slider(2, 1)", "Q = (a, a^2 / 2)"], [])
case("slider", ["a = Slider(0, 2 * pi)", "Q = (cos(a), sin(a))", "L = Locus(Q, a)"],
     [setn("a", 3)])

case("samplingbudget", [
    "a = Slider(1,3)", "B = Sequence(a*k,k,1,4)", "Q = (Element(B,1),a)", "L = Locus(Q,a)",
], [])
case("samplingbudget", [
    "a = Slider(0,1,0.01)", "u = Sum(Sequence(i*a,i,1,200000))/20000100000",
    "Q = (a,u)", "L = Locus(Q,a)",
], [])

case("sampledgraph", [
    "a = Slider(0,1)", "T = MinimumSpanningTree(Sequence((k,a*k),k,1,16))",
    "Q = (a,Length(T)/1000)", "L = Locus(Q,a)",
], [])
# The sampling case reads the tree through Length, which depends on the
# number of samples in T. Observe the mover, the live point and its locus.
cases.cases[-1]["observe"] = ["a", "Q", "L"]


cases.write()
