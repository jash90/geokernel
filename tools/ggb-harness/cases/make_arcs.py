#!/usr/bin/env python3
"""Writes cases/arcs.json: arcs and sectors, their measures and points on them.

`CircularArc(M, A, B)` and `CircularSector(M, A, B)` run on the circle around
`M` through `A` to the ray `MB`; `CircumcircularArc(A, B, C)` and
`CircumcircularSector(A, B, C)` on the circle through the three points, in the
direction that passes `B`; `Semicircle(A, B)` is the half circle over `AB`;
`Arc(k, ...)` and `Sector(k, ...)` take a part of the circle or conic `k`
between two points or two parameters. GeoGebra also knows the first four by
their internal names (`CircleArc`, `CircleSector`, `CircumcircleArc`,
`CircumcircleSector`). The cases move the inputs through the degenerate
configurations (a radius of 0, coincident points, collinear points, which
turn a circumcircular arc into a segment or two rays) and back, measure the
parts, and put points on them.

New cases go at the end of their topic and new topics at the end of the file,
so that existing ids keep their meaning.
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("arcs")
case = cases.case
topic = cases.topic
rp = cases.rp


def measures(part):
    """Length, area and perimeter of `part`."""
    return [f"len = Length({part})", f"ar = Area({part})", f"per = Perimeter({part})"]


def on(part, point="P", param="t"):
    return [f"{point} = Point({part})", f"{param} = PathParameter({point})"]


def at(part, u, point="U", number="u", param="w"):
    return [num(number, u), f"{point} = Point({part}, {number})", f"{param} = PathParameter({point})"]


def u01():
    return round(cases.rng.uniform(0.05, 0.95), 3)


def three():
    return [pt("M", *rp()), pt("A", *rp()), pt("B", *rp())]


# ------------------------------------------------------ arcs around a centre
topic("circular")
for cmd in ["CircularArc", "CircularSector", "CircleArc", "CircleSector"]:
    for _ in range(2):
        case("circular", three() + [f"a = {cmd}(M, A, B)"] + measures("a"),
             [mv("A", *rp()), mv("B", *rp()), mv("M", *rp())])
# the start point on the centre: radius 0, the part is undefined, and back
case("circular", [pt("M", 1, 1), pt("A", 4, 1), pt("B", 1, 5), "a = CircularArc(M, A, B)"] + measures("a"),
     [mv("A", 1, 1), mv("A", -2, 1), mv("B", 1, 1), mv("B", 4, -3)])
# the end point on the start ray: extent 0, then a full turn just short
case("circular", [pt("M", 0, 0), pt("A", 2, 0), pt("B", 5, 0), "a = CircularSector(M, A, B)"] + measures("a"),
     [mv("B", 5, -0.000001), mv("B", 5, 0.000001), mv("A", 0, 2), mv("B", 0, -3)])
# axis directions: start and end angles at 0, pi/2, pi and -pi/2
case("circular", [pt("M", 0, 0), pt("A", 3, 0), pt("B", 0, 3), "a = CircularArc(M, A, B)",
                  "s = CircularSector(M, B, A)"] + measures("a"),
     [mv("B", -3, 0), mv("B", 0, -3), mv("A", -3, 0), mv("A", 0, -3)])

# ------------------------------------------------ arcs through three points
topic("circum")
for cmd in ["CircumcircularArc", "CircumcircularSector", "CircumcircleArc", "CircumcircleSector"]:
    for _ in range(2):
        case("circum", three() + [f"a = {cmd}(M, A, B)"] + measures("a"),
             [mv("A", *rp()), mv("B", *rp()), mv("M", *rp())])
# B passes from one side of AC to the other: the orientation flips
case("circum", [pt("A", -2, 0), pt("B", 0, 2), pt("C", 2, 0), "a = CircumcircularArc(A, B, C)",
                "s = CircumcircularSector(A, B, C)"],
     [mv("B", 0, -2), mv("B", 0, 0.5), mv("B", 0, -0.5), mv("C", 0, 3)])
# collinear points: a segment when B lies between A and C, two rays otherwise
case("circum", [pt("A", 0, 0), pt("B", 1, 2), pt("C", 2, 0), "a = CircumcircularArc(A, B, C)",
                "s = CircumcircularSector(A, B, C)"],
     [mv("B", 1, 0), mv("B", 3, 0), mv("B", -1, 0), mv("B", 1, -1), mv("C", 4, 4), mv("B", 2, 2)])
# coincident points: A on B, A on C, B on C, and all three together
case("circum", [pt("A", 0, 0), pt("B", 2, 2), pt("C", 4, 0), "a = CircumcircularArc(A, B, C)",
                "s = CircumcircularSector(A, B, C)"],
     [mv("B", 0, 0), mv("B", 4, 0), mv("B", 2, 2), mv("C", 0, 0), mv("C", 4, 0)])
case("circum", [pt("A", 1, 1), pt("B", 1, 1), pt("C", 1, 1), "a = CircumcircularArc(A, B, C)"],
     [mv("B", 3, 1), mv("C", 2, 3), mv("A", 2, 3), mv("A", 1, 1)])
# nearly collinear: a huge circle
case("circum", [pt("A", 0, 0), pt("B", 1, 0.001), pt("C", 2, 0), "a = CircumcircularArc(A, B, C)"],
     [mv("B", 1, -0.001), mv("B", 1, 0.000001), mv("B", 1, 3)])

# ------------------------------------------------------------- semicircles
topic("semicircle")
for _ in range(3):
    case("semicircle", [pt("A", *rp()), pt("B", *rp()), "a = Semicircle(A, B)"] + measures("a"),
         [mv("A", *rp()), mv("B", *rp())])
# A on B: radius 0, then the half circle turns as B goes around A
case("semicircle", [pt("A", 0, 0), pt("B", 2, 0), "a = Semicircle(A, B)"] + measures("a"),
     [mv("B", 0, 0), mv("B", -2, 0), mv("B", 0, -2), mv("B", 0, 2)])

# ------------------------------------------- parts of a circle between points
topic("conicpoints")
for cmd in ["Arc", "Sector"]:
    for _ in range(2):
        case("conicpoints", [pt("M", *rp()), num("r", round(cases.rng.uniform(0.5, 6), 3)),
                             "k = Circle(M, r)", pt("P", *rp()), pt("Q", *rp()),
                             f"a = {cmd}(k, P, Q)"] + measures("a"),
             [mv("P", *rp()), mv("Q", *rp()), setn("r", 2.5), mv("M", *rp())])
# a circle through a point; a point on the centre; the circle of radius 0
case("conicpoints", [pt("M", 0, 0), pt("A", 3, 0), "k = Circle(M, A)", pt("P", 0, 5), pt("Q", -1, -1),
                     "a = Arc(k, P, Q)", "s = Sector(k, Q, P)"] + measures("a"),
     [mv("P", 0, 0), mv("P", 1, 1), mv("A", 0, 0), mv("A", 0, 2)])

# --------------------------------------- parts of a circle between parameters
topic("conicparams")
for cmd in ["Arc", "Sector"]:
    for _ in range(2):
        case("conicparams", [pt("M", *rp()), num("r", round(cases.rng.uniform(0.5, 6), 3)),
                             "k = Circle(M, r)", num("s", round(cases.rng.uniform(-7, 7), 3)),
                             num("e", round(cases.rng.uniform(-7, 7), 3)), f"a = {cmd}(k, s, e)"] + measures("a"),
             [setn("s", round(cases.rng.uniform(-7, 7), 3)), setn("e", round(cases.rng.uniform(-7, 7), 3)),
              setn("r", 1.5)])
# parameters at multiples of 2 pi, equal parameters, and a radius of 0
case("conicparams", [pt("M", 1, 2), num("r", 2), "k = Circle(M, r)", num("s", 0), num("e", 6.283185307179586),
                     "a = Arc(k, s, e)", "b = Sector(k, e, s)"] + measures("a"),
     [setn("e", 0), setn("s", -6.283185307179586), setn("e", 1), setn("s", 1), setn("r", 0), setn("r", 3)])
# parts of an ellipse: the sector's area and the arc's length
for cmd in ["Sector", "Arc"]:
    case("conicparams", [pt("F", -3, 0), pt("G", 3, 0), pt("C", 0, 2), "k = Ellipse(F, G, C)", num("s", 0.5),
                         num("e", 2.5), f"a = {cmd}(k, s, e)", "len = Length(a)", "ar = Area(a)"],
         [setn("e", -1), setn("s", 4), mv("G", 1, 4), mv("C", 6, 6)])

# -------------------------------------------------------- points on parts
topic("points")
case("points", [pt("M", 0, 0), pt("A", 3, 0), pt("B", 0, 3), "a = CircularArc(M, A, B)"] + on("a") + at("a", 0.3),
     [mv("P", 3, 3), mv("P", -3, -1), mv("P", 1, -3), mv("P", 3, -0.1), mv("B", -3, 0), setn("u", 0.9),
      mv("A", 0, -3), mv("P", -2, -2)])
case("points", [pt("M", 1, -1), pt("A", 4, 2), pt("B", -2, 0), "a = CircularSector(M, A, B)"] + on("a")
     + at("a", 0.75), [mv("P", 5, 5), mv("M", 0, 0), mv("P", -5, -5), mv("B", 3, -2), setn("u", 0.1)])
# negative orientation: the parameter runs from the end back to the start
case("points", [pt("A", -2, 0), pt("B", 0, -2), pt("C", 2, 0), "a = CircumcircularArc(A, B, C)"] + on("a")
     + at("a", 0.2), [mv("P", 0, -5), mv("P", -1, 1), mv("B", 0, 2), mv("P", 3, 0), mv("B", 0, -3)])
# the arc becomes a segment, two rays and an arc again
case("points", [pt("A", 0, 0), pt("B", 1, 2), pt("C", 2, 0), "a = CircumcircularArc(A, B, C)"] + on("a")
     + at("a", 0.4), [mv("P", 1, 3), mv("B", 1, 0), mv("P", 1.5, 1), mv("B", 5, 0), mv("B", 1, 1), mv("P", 0, 1)])
# the part becomes undefined (radius 0) and comes back: the point keeps its parameter
case("points", [pt("M", 0, 0), pt("A", 2, 0), pt("B", 0, 2), "a = CircularArc(M, A, B)"] + on("a") + at("a", 0.5),
     [mv("P", 1, 1), mv("A", 0, 0), mv("P", 5, 5), mv("A", 0, -2), mv("P", -1, -1)])
for cmd in ["Semicircle(A, B)", "Arc(k, A, B)", "Sector(k, A, B)"]:
    case("points", [pt("M", 0, 0), num("r", 2), "k = Circle(M, r)", pt("A", 2, 1), pt("B", -1, 2),
                    f"a = {cmd}"] + on("a") + at("a", u01()),
         [mv("P", *rp()), mv("B", *rp()), mv("P", *rp()), mv("A", *rp()), setn("u", u01()), mv("P", *rp())])
case("points", [pt("M", 0, 0), num("r", 3), "k = Circle(M, r)", num("s", 1), num("e", 4), "a = Arc(k, s, e)"]
     + on("a") + at("a", 0.6), [mv("P", 3, 0), mv("P", -3, 0), setn("e", -2), setn("s", 2), setn("r", 1),
                               mv("P", 0, -2)])
# a point on a sector of a circumcircle whose points become collinear: the
# sector has no finite area, but stays a path
case("points", [pt("A", 0, 0), pt("B", 1, 2), pt("C", 2, 0), "a = CircumcircularSector(A, B, C)"] + on("a")
     + at("a", 0.4), [mv("P", 1, 3), mv("B", 1, 0), mv("P", 1.5, 1), mv("B", 1, 1)])

# ------------------------------------------------ parts used further on
topic("chain")
# the arc turns about its centre at the same length: the points on it move
case("chain", [pt("M", 0, 0), pt("A", 3, 0), pt("B", 0, 3), "a = CircularArc(M, A, B)"] + on("a") + at("a", 0.5)
     + ["m = Midpoint(P, U)"], [mv("P", 2, 1), mv("A", 0, 3), mv("B", -3, 0), mv("A", -3, 0), mv("B", 0, -3)])
# a point on an arc as the centre of another arc
case("chain", [pt("M", 0, 0), pt("A", 2, 0), pt("B", 0, 2), "a = CircularArc(M, A, B)", "P = Point(a)",
               pt("C", 5, 0), "b = CircularSector(P, C, M)", "Q = Point(b)", "t = PathParameter(Q)",
               "ar = Area(b)"],
     [mv("P", 1, 2), mv("Q", 5, 5), mv("C", 3, 3), mv("A", -2, 0), mv("Q", 0, 0)])

cases.write()
