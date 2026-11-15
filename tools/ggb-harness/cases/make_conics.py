#!/usr/bin/env python3
"""Writes cases/conics.json: general conics (type `conic` in golden/SCHEMA.md).

`Conic` through five points, `Ellipse` and `Hyperbola` from two foci and a
point or a length, `Parabola` from a focus and a directrix. Besides random
positions the cases put the inputs on the degenerate configurations and move
through them: four collinear points, coincident foci, a point on the focal
axis or on the directrix, a length equal to half the focal distance, and moves
that change the type of the conic.

New cases go at the end of their topic and new topics at the end of the file,
so that existing ids keep their meaning.
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("conics")
case = cases.case
topic = cases.topic
rp = cases.rp
r2 = cases.r2

FIVE = ["A", "B", "C", "D", "E"]


def five(points):
    return [pt(n, *p) for n, p in zip(FIVE, points)] + ["k = Conic(A, B, C, D, E)"]


# ------------------------------------------------------------- five points
topic("fivepoints")
# random positions, every point moved once
for _ in range(8):
    steps = [mv(n, *rp()) for n in FIVE]
    case("fivepoints", five([rp() for _ in FIVE]), steps)

# on a circle, then off it: circle, ellipse or hyperbola
case("fivepoints", five([(1, 0), (0, 1), (-1, 0), (0, -1), (0.6, 0.8)]),
     [mv("E", 0.8, 0.7), mv("E", 2, 2), mv("E", 0.5, 0.5), mv("E", 0.6, 0.8)])
# on an ellipse x^2/9 + y^2/4 = 1
case("fivepoints", five([(3, 0), (0, 2), (-3, 0), (0, -2), (1.8, 1.6)]),
     [mv("A", 4, 0), mv("A", 3, 0), mv("D", 0, -3), mv("D", 0, -2)])
# on a hyperbola x^2 - y^2 = 1
case("fivepoints", five([(1, 0), (-1, 0), (2, 1.732051), (2, -1.732051), (-3, 2.828427)]),
     [mv("E", -3, 2), mv("E", -3, 4), mv("E", -3, 2.828427)])
# on the parabola y = x^2: the matrix is close to det S = 0
case("fivepoints", five([(0, 0), (1, 1), (-1, 1), (2, 4), (-2, 4)]),
     [mv("E", -2, 4.000001), mv("E", -2, 4.1), mv("E", -2, 3.9), mv("E", -2, 4)])
# nearly on a parabola: within and beyond the tolerance of det S = 0
case("fivepoints", five([(0, 0), (1, 1), (-1, 1), (2, 4), (-2, 4.00001)]),
     [mv("E", -2, 4.0001), mv("E", -2, 4.001), mv("E", -2, 4.00001)])
# four collinear points
case("fivepoints", five([(0, 0), (1, 0), (2, 0), (3, 0), (1, 2)]),
     [mv("E", 2, 3), mv("D", 3, 0.5), mv("D", 3, 0), mv("E", 0, 0)])
case("fivepoints", five([(0, 0), (1, 1), (2, 2), (5, 1), (3, 3)]),
     [mv("D", 4, 4), mv("D", 5, 1), mv("E", 3, 3.5)])
# three collinear points: a pair of lines
case("fivepoints", five([(0, 0), (1, 0), (2, 0), (0, 1), (1, 2)]),
     [mv("C", 2, 0.5), mv("C", 2, 0), mv("E", 2, 4), mv("E", 1, 2)])
case("fivepoints", five([(0, 0), (2, 1), (4, 2), (1, 3), (3, 4)]),
     [mv("E", 2, 3.5), mv("E", 3, 4), mv("A", 0, 0.2), mv("A", 0, 0)])
# two pairs of points on parallel lines and a fifth on one of them
case("fivepoints", five([(0, 0), (1, 0), (0, 2), (1, 2), (3, 0)]),
     [mv("E", 3, 2), mv("E", 3, 1), mv("E", 3, 0)])
# coincident points
case("fivepoints", five([(0, 0), (0, 0), (2, 1), (1, 3), (-1, 2)]),
     [mv("B", 1, 0), mv("B", 0, 0), mv("C", -1, 2)])
# a move through the configuration where the conic changes type
case("fivepoints", five([(-2, 0), (2, 0), (0, 1), (0, -1), (1, 0.8)]),
     [mv("E", 1, 0.9), mv("E", 1, 1), mv("E", 1, 1.2), mv("E", 1, 2), mv("E", 1, 0.8)])
# large and small coordinates (the matrix is rescaled by a power of two)
case("fivepoints", five([(1000, 0), (0, 1000), (-1000, 0), (0, -1000), (600, 800)]),
     [mv("E", 700, 700), mv("A", 1200, 0)])
case("fivepoints", five([(0.001, 0), (0, 0.001), (-0.001, 0), (0, -0.002), (0.0006, 0.0008)]),
     [mv("E", 0.0007, 0.0007), mv("D", 0, -0.001)])

# ------------------------------------------------ ellipse from a point
topic("ellipse")
for _ in range(6):
    script = [pt("F", *rp()), pt("G", *rp()), pt("P", *rp()), "e = Ellipse(F, G, P)"]
    case("ellipse", script, [mv("P", *rp()), mv("F", *rp()), mv("G", *rp())])
# coincident foci: a circle; the foci part again
case("ellipse", [pt("F", 1, 1), pt("G", 1, 1), pt("P", 3, 2), "e = Ellipse(F, G, P)"],
     [mv("G", 2, 1), mv("G", 1, 1), mv("P", 1, 1), mv("P", 3, 2)])
# the point between the foci: a double line
case("ellipse", [pt("F", -3, 0), pt("G", 3, 0), pt("P", 1, 0), "e = Ellipse(F, G, P)"],
     [mv("P", 1, 0.000001), mv("P", 1, 1), mv("P", 1, 0), mv("P", 5, 0)])
# the point on a focus
case("ellipse", [pt("F", -2, 1), pt("G", 2, -1), pt("P", -2, 1), "e = Ellipse(F, G, P)"],
     [mv("P", 0, 3), mv("P", 2, -1), mv("F", 2, -1)])
# a tilted ellipse that becomes a circle and back
case("ellipse", [pt("F", -1, -1), pt("G", 1, 1), pt("P", 3, 0), "e = Ellipse(F, G, P)"],
     [mv("F", 0, 0), mv("G", 0, 0), mv("G", 1, 1), mv("F", -1, -1)])

# ---------------------------------------------- hyperbola from a point
topic("hyperbola")
for _ in range(6):
    script = [pt("F", *rp()), pt("G", *rp()), pt("P", *rp()), "h = Hyperbola(F, G, P)"]
    case("hyperbola", script, [mv("P", *rp()), mv("F", *rp()), mv("G", *rp())])
# the point on the perpendicular bisector: semi-major axis 0
case("hyperbola", [pt("F", -3, 0), pt("G", 3, 0), pt("P", 0, 2), "h = Hyperbola(F, G, P)"],
     [mv("P", 0.000001, 2), mv("P", 1, 2), mv("P", 0, 5), mv("P", 0, 2)])
# the point on the focal axis outside the foci: a double line
case("hyperbola", [pt("F", -3, 0), pt("G", 3, 0), pt("P", 4, 0), "h = Hyperbola(F, G, P)"],
     [mv("P", 4, 0.000001), mv("P", 4, 1), mv("P", 4, 0), mv("P", -5, 0)])
# coincident foci
case("hyperbola", [pt("F", 1, 2), pt("G", 1, 2), pt("P", 3, 3), "h = Hyperbola(F, G, P)"],
     [mv("G", 2, 2), mv("G", 1, 2)])
# the point on a focus
case("hyperbola", [pt("F", -2, 0), pt("G", 2, 0), pt("P", 2, 0), "h = Hyperbola(F, G, P)"],
     [mv("P", 3, 1), mv("P", -2, 0)])

# ------------------------------------------------- foci and a length
topic("focilength")
for name in ("Ellipse", "Hyperbola"):
    for _ in range(3):
        script = [pt("F", *rp()), pt("G", *rp()), num("a", r2(0.5, 9)), f"e = {name}(F, G, a)"]
        case("focilength", script, [setn("a", r2(0.5, 9)), mv("F", *rp()), mv("G", *rp())])
for name in ("Ellipse", "Hyperbola"):
    # focal distance 6: a > 3 an ellipse, a = 3 a double line, a < 3 a hyperbola
    case("focilength", [pt("F", -3, 0), pt("G", 3, 0), num("a", 5), f"e = {name}(F, G, a)"],
         [setn("a", 3), setn("a", 2), setn("a", 0), setn("a", -1), setn("a", -0.000000001),
          setn("a", 100000), setn("a", 5)])
    # coincident foci: a circle, a single point for a = 0
    case("focilength", [pt("F", 2, 1), pt("G", 2, 1), num("a", 2), f"e = {name}(F, G, a)"],
         [setn("a", 0), setn("a", 0.000001), setn("a", 3), mv("G", 3, 1)])
    # tilted foci, a through half the focal distance
    case("focilength", [pt("F", -1, -2), pt("G", 2, 2), num("a", 3), f"e = {name}(F, G, a)"],
         [setn("a", 2.5), setn("a", 2.4), setn("a", 2.6), setn("a", 1)])

# --------------------------------------------- focus and a directrix
topic("parabola")
for _ in range(6):
    script = [pt("F", *rp()), pt("A", *rp()), pt("B", *rp()), "g = Line(A, B)", "p = Parabola(F, g)"]
    case("parabola", script, [mv("F", *rp()), mv("A", *rp()), mv("B", *rp())])
# the focus on the directrix and off it again
case("parabola", [pt("F", 1, 2), pt("A", -1, 0), pt("B", 3, 0), "g = Line(A, B)", "p = Parabola(F, g)"],
     [mv("F", 1, 0), mv("F", 1, 0.000001), mv("F", 1, -2), mv("F", 1, 2)])
# vertical, horizontal and steep directrices
case("parabola", [pt("F", 3, 1), pt("A", 0, -2), pt("B", 0, 2), "g = Line(A, B)", "p = Parabola(F, g)"],
     [mv("F", -3, 1), mv("B", 1, 2), mv("B", 4, -2), mv("B", 0, 2)])
# a segment and a ray as directrix
case("parabola", [pt("F", 0, 2), pt("A", -2, -1), pt("B", 2, -1), "s = Segment(A, B)", "p = Parabola(F, s)"],
     [mv("F", 3, 4), mv("A", -2, -3)])
case("parabola", [pt("F", 0, 2), pt("A", -2, -1), pt("B", 2, -1), "r = Ray(A, B)", "p = Parabola(F, r)"],
     [mv("F", 3, 4), mv("B", 2, 1)])
# coincident points of the directrix: undefined, then defined again
case("parabola", [pt("F", 0, 2), pt("A", -2, -1), pt("B", 2, -1), "g = Line(A, B)", "p = Parabola(F, g)"],
     [mv("B", -2, -1), mv("B", 2, -1)])
# large coordinates (the matrix is normalised)
case("parabola", [pt("F", 500, 300), pt("A", -400, -100), pt("B", 600, -200), "g = Line(A, B)",
                  "p = Parabola(F, g)"],
     [mv("F", 0.001, 0.002), mv("A", 0, 0), mv("B", 0.001, 0)])

# ------------------------------------------------- a line and a conic
topic("intersection")
LINE = [pt("U", -7, -2), pt("V", 6, 3), "g = Line(U, V)"]
BOTH = ["S = Intersect(g, k, 1)", "T = Intersect(k, g, 2)"]
CONICS = [
    five([(3, 0), (0, 2), (-3, 0), (0, -2), (1.8, 1.6)]),
    [pt("F", -3, 0), pt("G", 3, 0), pt("P", 0, 4), "k = Ellipse(F, G, P)"],
    [pt("F", -3, 0), pt("G", 3, 0), pt("P", 2, 1), "k = Hyperbola(F, G, P)"],
    [pt("F", 0, 1), pt("A", -1, -1), pt("B", 1, -1), "d = Line(A, B)", "k = Parabola(F, d)"],
]
# random lines through each kind of conic
for conic in CONICS:
    for _ in range(2):
        script = conic + [pt("U", *rp()), pt("V", *rp()), "g = Line(U, V)"] + BOTH
        case("intersection", script, [mv("U", *rp()), mv("V", *rp()), mv("U", *rp())])
# a tangent of the ellipse, then a passing line and a secant
case("intersection", CONICS[1] + [pt("U", -1, 4), pt("V", 1, 4), "g = Line(U, V)"] + BOTH,
     [mv("V", 1, 4.000001), mv("U", -1, 4.000001), mv("U", -1, 5), mv("V", 1, 5), mv("U", -1, 3),
      mv("V", 1, 3), mv("U", -1, 4), mv("V", 1, 4)])
# lines parallel to the axis of the parabola, in both directions, and through it
case("intersection", CONICS[3] + [pt("U", 2, 0), pt("V", 2, 5), "g = Line(U, V)"] + BOTH,
     [mv("V", 2, -5), mv("V", 2.5, 5), mv("V", 2, 5), mv("U", 2, 9)])
case("intersection", CONICS[3] + [pt("U", 2, 5), pt("V", 2, 0), "g = Line(U, V)"] + BOTH,
     [mv("U", 3, 5), mv("U", 2, 5), mv("V", 2, 9)])
# created with two points, then parallel to the axis
case("intersection", CONICS[3] + [pt("U", -3, 0), pt("V", 3, 4), "g = Line(U, V)"] + BOTH,
     [mv("U", 3, 0), mv("U", 3, 9), mv("U", -3, 0)])
# lines parallel to an asymptote of x^2 - y^2 = 1 and the asymptote itself
case("intersection", five([(1, 0), (-1, 0), (2, 1.732051), (2, -1.732051), (-3, 2.828427)])
     + [pt("U", 0, 1), pt("V", 1, 2), "g = Line(U, V)"] + BOTH,
     [mv("U", 0, -1), mv("U", 0, 0), mv("V", 1, 1), mv("V", 1, 2.000001), mv("U", 0, 1)])
# a double line, and a line along it
case("intersection", [pt("F", -3, 0), pt("G", 3, 0), num("a", 3), "k = Ellipse(F, G, a)",
                      pt("U", 1, -2), pt("V", 2, 3), "g = Line(U, V)"] + BOTH,
     [setn("a", 3.000001), setn("a", 5), setn("a", 3), mv("U", -5, 0), mv("V", 5, 0), mv("V", 5, 1)])
# a pair of lines through three collinear points
case("intersection", five([(0, 0), (1, 0), (2, 0), (0, 1), (1, 2)]) + LINE + BOTH,
     [mv("U", -7, 5), mv("V", 6, -4), mv("C", 2, 0.5), mv("C", 2, 0)])
# parallel lines and a single point
case("intersection", five([(0, 0), (1, 0), (0, 2), (1, 2), (3, 0)]) + LINE + BOTH,
     [mv("V", 6, 0), mv("E", 3, 1), mv("E", 3, 0)])
case("intersection", [pt("F", 2, 1), num("a", 0), "k = Ellipse(F, F, a)",
                      pt("U", 0, 0), pt("V", 4, 2), "g = Line(U, V)"] + BOTH,
     [mv("V", 4, 3), mv("V", 4, 2), setn("a", 1), setn("a", 0)])
# a line through a point of the conic by construction: that point comes first
case("intersection", five([(3, 0), (0, 2), (-3, 0), (0, -2), (1.8, 1.6)])
     + [pt("U", -5, 1), "g = Line(U, A)"] + BOTH,
     [mv("U", -5, -1), mv("A", 4, 0), mv("U", 7, 3), mv("A", 3, 0)])
case("intersection", five([(3, 0), (0, 2), (-3, 0), (0, -2), (1.8, 1.6)])
     + [pt("U", -5, 1), "g = Line(E, U)"] + BOTH,
     [mv("U", 0, -4), mv("E", 2, 2), mv("U", -5, 1)])
case("intersection", CONICS[1] + [pt("U", 4, -3), "g = Line(U, P)"] + BOTH,
     [mv("U", -4, -3), mv("P", 1, 5), mv("U", 0, 9)])
case("intersection", CONICS[2] + [pt("U", 4, -3), "g = Line(P, U)"] + BOTH,
     [mv("U", -4, 3), mv("P", 4, 2)])
# the conic becomes undefined and defined again
case("intersection", CONICS[3] + LINE + BOTH,
     [mv("B", -1, -1), mv("B", 1, -1), mv("F", 0, -1), mv("F", 0, 1)])
# large coordinates
case("intersection", [pt("F", -3000, 0), pt("G", 3000, 0), pt("P", 0, 4000), "k = Ellipse(F, G, P)",
                      pt("U", -7000, -2000), pt("V", 6000, 3000), "g = Line(U, V)"] + BOTH,
     [mv("V", 6000, 30000), mv("U", 0.001, 0)])

# ------------------------------------------------------------- readers
topic("readers")
READERS = ["M = Center(k)", "N = Midpoint(k)", "W = Focus(k)", "X = Vertex(k)", "ax = Axes(k)",
           "maj = MajorAxis(k)", "mnr = MinorAxis(k)", "dir = Directrix(k)",
           "la = FirstAxisLength(k)", "lb = SecondAxisLength(k)", "ecc = Eccentricity(k)",
           "lin = LinearEccentricity(k)", "par = Parameter(k)"]
SINGLE = [r for r in READERS if not r.split(" = ")[1].startswith(("Focus", "Vertex", "Axes", "Directrix"))]
# random ellipses, hyperbolas and parabolas
for _ in range(2):
    case("readers", [pt("F", *rp()), pt("G", *rp()), pt("P", *rp()), "k = Ellipse(F, G, P)"] + READERS,
         [mv("P", *rp()), mv("F", *rp()), mv("G", *rp())])
    case("readers", [pt("F", *rp()), pt("G", *rp()), pt("P", *rp()), "k = Hyperbola(F, G, P)"] + READERS,
         [mv("P", *rp()), mv("F", *rp()), mv("G", *rp())])
    case("readers", [pt("F", *rp()), pt("A", *rp()), pt("B", *rp()), "g = Line(A, B)", "k = Parabola(F, g)"]
         + READERS, [mv("F", *rp()), mv("A", *rp()), mv("B", *rp())])
# an ellipse through a double line, a circle and back
case("readers", [pt("F", -3, 0), pt("G", 3, 0), pt("P", 0, 4), "k = Ellipse(F, G, P)"] + READERS,
     [mv("P", 1, 0), mv("P", 0, 4), mv("G", -3, 0), mv("G", 3, 0), mv("P", 5, 0)])
# a hyperbola through a double line and semi-major axis 0
case("readers", [pt("F", -3, 0), pt("G", 3, 0), pt("P", 2, 1), "k = Hyperbola(F, G, P)"] + READERS,
     [mv("P", 4, 0), mv("P", 2, 1), mv("P", 0, 2), mv("P", -2, 1)])
# foci and a length: ellipse, double line, hyperbola, undefined and back
case("readers", [pt("F", -1, -2), pt("G", 2, 2), num("a", 4), "k = Ellipse(F, G, a)"] + READERS,
     [setn("a", 2.5), setn("a", 1), setn("a", -1), setn("a", 4), setn("a", 3)])
# coincident foci: a circle and a single point
case("readers", [pt("F", 2, 1), pt("G", 2, 1), num("a", 2), "k = Ellipse(F, G, a)"] + SINGLE
     + ["X = Vertex(k)", "ax = Axes(k)"],
     [setn("a", 0), setn("a", 3), mv("G", 3, 1), mv("G", 2, 1)])
# a parabola whose focus moves onto its directrix and back
case("readers", [pt("F", 1, 2), pt("A", -1, 0), pt("B", 3, 1), "g = Line(A, B)", "k = Parabola(F, g)"]
     + READERS, [mv("F", 1, 0.5), mv("F", 1, 2), mv("F", -2, -3)])
# five points: circle, ellipse, hyperbola and degenerate conics
case("readers", five([(-2, 0), (2, 0), (0, 1), (0, -1), (1, 0.8)]) + READERS,
     [mv("E", 1, 0.9), mv("E", 1, 1), mv("E", 1, 2), mv("E", 1, 0.8)])
case("readers", five([(0, 0), (1, 0), (2, 0), (0, 1), (1, 2)]) + SINGLE + ["X = Vertex(k)", "ax = Axes(k)"],
     [mv("C", 2, 0.5), mv("C", 2, 0), mv("E", 2, 4)])
case("readers", five([(0, 0), (1, 0), (0, 2), (1, 2), (3, 0)]) + SINGLE + ["X = Vertex(k)", "ax = Axes(k)"],
     [mv("E", 3, 1), mv("E", 3, 2), mv("E", 3, 0)])
# a circle has no directrix: GeoGebra labels neither output
case("readers", five([(1, 0), (0, 1), (-1, 0), (0, -1), (0.6, 0.8)]) + SINGLE
     + ["W = Focus(k)", "X = Vertex(k)", "ax = Axes(k)"],
     [mv("E", 2, 2), mv("E", 0.6, 0.8)])
# the other names of the commands
case("readers", [pt("F", -3, 1), pt("G", 2, -1), pt("P", 0, 4), "k = Ellipse(F, G, P)",
                 "fa = FirstAxis(k)", "sa = SecondAxis(k)", "sm = SemiMajorAxisLength(k)",
                 "sn = SemiMinorAxisLength(k)", "ex = Excentricity(k)"],
     [mv("P", 4, 4), mv("P", 0.5, 0)])

# ------------------------------------------------------ transformations
topic("transform")
SOURCES = [
    five([(3, 0), (0, 2), (-3, 0), (0, -2), (1.8, 1.6)]),
    [pt("F", -3, 1), pt("G", 2, -1), pt("P", 0, 4), "k = Hyperbola(F, G, P)"],
    [pt("F", 1, 2), pt("A", -1, -1), pt("B", 3, 0), "d = Line(A, B)", "k = Parabola(F, d)"],
    five([(0, 0), (1, 0), (2, 0), (0, 1), (1, 2)]),
]
MAPS = [
    [pt("U", 1, 2), pt("V", 3, -1), "v = Vector(U, V)", "m = Translate(k, v)"],
    [num("w", 0.7), "m = Rotate(k, w)"],
    [num("w", -2.1), pt("Q", 2, -3), "m = Rotate(k, w, Q)"],
    [num("f", 1.5), "m = Dilate(k, f)"],
    [num("f", -0.5), pt("Q", -1, 4), "m = Dilate(k, f, Q)"],
    [pt("Q", 2, 1), "m = Reflect(k, Q)"],
    [pt("U", -2, -1), pt("V", 1, 3), "h = Line(U, V)", "m = Reflect(k, h)"],
    [pt("U", -2, -1), pt("V", 1, 3), "h = Line(U, V)", "m = Mirror(k, h)"],
]
MOVES = {
    "Translate": [mv("V", -2, 4), mv("U", 0, 0)],
    "Rotate": [setn("w", 3.14159), setn("w", 0)],
    "Dilate": [setn("f", 0), setn("f", -3), setn("f", 0.000001)],
    "Reflect": [mv("Q", -4, 0)],
    "Mirror": [mv("V", -2, -1)],
}
for src in SOURCES:
    first = src[0].split(" = ")[0]
    for m in MAPS:
        cmd = m[-1].split(" = ")[1].split("(")[0]
        steps = list(MOVES[cmd])
        if cmd == "Reflect" and "h = Line(U, V)" in m:
            steps = [mv("U", 3, -2), mv("V", 3, 5)]
        steps += [mv(first, 0.5, -0.5), mv(first, *rp())]
        case("transform", src + m, steps)
# a transformation of a transformation, and readers of the image
case("transform", SOURCES[0] + [num("w", 0.4), "m = Rotate(k, w)", pt("Q", 1, 1), "n = Reflect(m, Q)",
                                "M = Center(n)", "W = Focus(n)", "la = FirstAxisLength(n)"],
     [setn("w", 1.2), mv("Q", -2, 0), mv("A", 4, 0)])
# a line through an image
case("transform", SOURCES[1] + [num("f", 2), "m = Dilate(k, f)", pt("U", -9, -2), pt("V", 8, 3), "g = Line(U, V)",
                                "S = Intersect(g, m, 1)", "T = Intersect(g, m, 2)"],
     [setn("f", 0.5), mv("V", 8, -3), setn("f", 0), setn("f", 2)])

# --------------------------------------------- tangents, polars, diameters
topic("tangent")
ELL = [pt("F", -3, 0), pt("G", 3, 0), pt("P", 0, 4), "k = Ellipse(F, G, P)"]
HYP = [pt("F", -3, 0), pt("G", 3, 0), pt("P", 2, 1), "k = Hyperbola(F, G, P)"]
PAR = [pt("F", 0, 1), pt("A", -1, -1), pt("B", 1, -1), "d = Line(A, B)", "k = Parabola(F, d)"]
for conic, start in ((ELL, (7, 5)), (HYP, (1, 5)), (PAR, (7, 5))):
    # from a point outside, then inside, onto the conic and out again
    case("tangent", conic + [pt("Q", *start), "t = Tangent(Q, k)", "S = Intersect(t_{1}, k)"],
         [mv("Q", 0.5, 0.2), mv("Q", *start), mv("Q", -6, 3), mv("Q", *rp()), mv("Q", *rp())])
    # parallel to a line
    case("tangent", conic + [pt("U", -2, -6), pt("V", -1, -2), "g = Line(U, V)", "t = Tangent(g, k)"],
         [mv("V", 3, 1), mv("V", -2, 0), mv("V", 4, -6), mv("U", *rp()), mv("V", *rp())])
    # polar and diameters
    case("tangent", conic + [pt("Q", 5, 2), "p = Polar(Q, k)", pt("U", 1, 1), pt("V", 3, -1),
                             "g = Line(U, V)", "m = Diameter(g, k)", "v = Vector(U, V)",
                             "n = ConjugateDiameter(v, k)"],
         [mv("Q", 0, 0), mv("Q", -4, 7), mv("V", 1, 5), mv("V", 1, 1.000001), mv("U", *rp())])
# a tangent from a point on the ellipse: one line
case("tangent", ELL + ["t = Tangent(P, k)"], [mv("P", 3, 2), mv("P", 0, 4), mv("F", -1, 0)])
case("tangent", [pt("Q", 5, 0)] + ELL[:3] + ["k = Ellipse(F, G, Q)", "t = Tangent(Q, k)",
                                            "S = Intersect(t, k)"],
     [mv("Q", 6, 1), mv("P", 9, 9)])
# a tangent of a five-point conic at one of its points
case("tangent", five([(3, 0), (0, 2), (-3, 0), (0, -2), (1.8, 1.6)]) + ["t = Tangent(E, k)"],
     [mv("E", 1, 1), mv("A", 4, 0), mv("E", 1.8, 1.6)])
# the conic becomes degenerate or undefined
case("tangent", [pt("F", -3, 0), pt("G", 3, 0), num("a", 5), "k = Ellipse(F, G, a)", pt("Q", 8, 6),
                 "t = Tangent(Q, k)", pt("U", 0, 0), pt("V", 2, 1), "g = Line(U, V)", "s = Tangent(g, k)",
                 "p = Polar(Q, k)", "m = Diameter(g, k)"],
     [setn("a", 3), setn("a", 2), setn("a", -1), setn("a", 5)])
case("tangent", five([(0, 0), (1, 0), (2, 0.5), (0, 1), (1, 2)]) + [pt("Q", 5, 5), "p = Polar(Q, k)",
                                                                      pt("U", 0, -3), pt("V", 1, -3),
                                                                      "g = Line(U, V)", "m = Diameter(g, k)"],
     [mv("C", 2, 0), mv("C", 2, -0.5), mv("Q", -3, 1)])
# a segment and a ray as the direction of the tangents
case("tangent", HYP + [pt("U", 0, 0), pt("V", 1, 4), "s = Segment(U, V)", "t = Tangent(s, k)", "r = Ray(V, U)",
                       "w = Tangent(r, k)"],
     [mv("V", 4, 1), mv("V", -1, 3)])

cases.write()
