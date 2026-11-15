#!/usr/bin/env python3
"""Writes cases/vector.json: vectors, rays and the commands made from them.

Covers Vector(A, B), Vector(A), free vectors (`u = (x, y)` with a lower-case
label), Ray(A, B), Ray(A, u), Line(A, u), OrthogonalLine/PerpendicularLine(A, u),
Translate of a point, line, segment, circle, ray and vector, Translate(u, A),
Direction, UnitVector, OrthogonalVector/PerpendicularVector,
UnitOrthogonalVector/UnitPerpendicularVector and Length of a vector.

The steps make the two points of a vector or ray coincide and separate again,
set free vectors to zero and back, flip the direction of rays, and make the
points that vectors and rays are built on undefined (parallel lines).
`Direction` and `OrthogonalVector` of a line depend on GeoGebra's raw scale of
the line; their cases use the lines whose raw scale the kernel rebuilds and
keep the others in the topic `rawscale`.
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("vector")
case = cases.case
topic = cases.topic
rp = cases.rp
r2 = cases.r2


def vec(label, x, y):
    """A free vector: the same literal as a point, with a lower-case label."""
    return pt(label, x, y)


# ------------------------------------------------------- Vector(A, B), Vector(A)
topic("vec")
for _ in range(3):
    a, b = rp(), rp()
    case("vec", [pt("A", *a), pt("B", *b), "v = Vector(A, B)", "w = Vector(A)"],
         [mv("B", *rp()), mv("A", *rp()), mv("B", *a)] + [mv("A", *a)] + [mv("B", *rp())])
# coincident points give a zero vector, then separate again
a = rp()
case("vec", [pt("A", *a), pt("B", *rp()), "v = Vector(A, B)", "n = Length(v)"],
     [mv("B", *a), mv("A", *rp()), mv("B", *rp())])
# on derived points: a midpoint and an intersection that can vanish
case("vec", [pt("A", *rp()), pt("B", *rp()), pt("C", *rp()), "M = Midpoint(A, B)",
             "v = Vector(M, C)", "w = Vector(M)"],
     [mv("A", *rp()), mv("C", *rp()), mv("B", *rp())])
case("vec", [pt("A", 0, 0), pt("B", 4, 1), pt("C", 1, 3), pt("D", 2, -2),
             "l = Line(A, B)", "m = Line(C, D)", "S = Intersect(l, m)", "v = Vector(S, C)",
             "w = Vector(S)"],
     # D such that m is parallel to l: S and both vectors become undefined
     [mv("D", 5, 4), mv("D", 3, -1), mv("C", 2, 2)])

# ------------------------------------------------------- free vectors
topic("free")
for name in ("UnitVector", "OrthogonalVector", "PerpendicularVector", "UnitOrthogonalVector",
             "UnitPerpendicularVector", "Length"):
    u = rp()
    case("free", [vec("u", *u), f"q = {name}(u)"],
         [mv("u", *rp()), mv("u", 0, 0), mv("u", *rp()), mv("u", -u[0], -u[1])])
case("free", [vec("u", *rp()), vec("v", *rp()), pt("A", *rp()), "w = Translate(u, A)",
              "t = Translate(u, v)", "n = Length(t)"],
     [mv("u", *rp()), mv("v", 0, 0), mv("A", *rp()), mv("u", 0, 0), mv("u", *rp())])
case("free", [vec("u", 3, 4), "n = Length(u)", "q = UnitVector(u)", "o = OrthogonalVector(q)",
              "k = UnitOrthogonalVector(o)"],
     [mv("u", -3, 4), mv("u", 0, -2), mv("u", 0, 0), mv("u", 1e6, 1e-3)])

# ------------------------------------------------------- Ray(A, B), Ray(A, u)
topic("ray")
for _ in range(3):
    a, b = rp(), rp()
    mirror = (2 * a[0] - b[0], 2 * a[1] - b[1])
    case("ray", [pt("A", *a), pt("B", *b), "g = Ray(A, B)"],
         [mv("B", *mirror), mv("B", *a), mv("B", *rp()), mv("A", *rp())])
for _ in range(3):
    a, u = rp(), rp()
    case("ray", [pt("A", *a), vec("u", *u), "g = Ray(A, u)"],
         [mv("u", -u[0], -u[1]), mv("u", 0, 0), mv("u", *rp()), mv("A", *rp())])
# the vector of a ray built from two points that coincide
case("ray", [pt("A", *rp()), pt("C", 1, -1), pt("D", 3, 2), "v = Vector(C, D)",
             "g = Ray(A, v)"],
     [mv("D", -2, 4), mv("C", -2, 4), mv("C", 0, 0), mv("D", 0, 0), mv("D", 2, -3)])
# rays from an intersection that can vanish
case("ray", [pt("A", 0, 0), pt("B", 4, 1), pt("C", 1, 3), pt("D", 2, -2),
             "l = Line(A, B)", "m = Line(C, D)", "S = Intersect(l, m)", "g = Ray(S, C)",
             "h = Ray(C, S)"],
     [mv("D", 5, 4), mv("D", 3, -1), mv("A", 1, -2)])

# ------------------------------------------------------- Line(A, u), OrthogonalLine(A, u)
topic("line")
for name in ("Line", "OrthogonalLine", "PerpendicularLine"):
    for _ in range(2):
        a, u = rp(), rp()
        case("line", [pt("A", *a), vec("u", *u), f"m = {name}(A, u)"],
             [mv("A", *rp()), mv("u", -u[0], -u[1]), mv("u", 0, 0), mv("u", *rp())])
case("line", [pt("A", 1, -1), pt("B", 3, 2), pt("C", *rp()), "v = Vector(A, B)",
              "m = Line(C, v)", "n = OrthogonalLine(C, v)"],
     [mv("B", -2, 4), mv("A", -2, 4), mv("C", *rp()), mv("A", 0, 1)])

# ------------------------------------------------------- Translate
topic("translate")
# a point, by a free vector and by a vector between points
for _ in range(2):
    case("translate", [pt("P", *rp()), vec("u", *rp()), "Q = Translate(P, u)"],
         [mv("P", *rp()), mv("u", *rp()), mv("u", 0, 0), mv("P", *rp())])
case("translate", [pt("P", *rp()), pt("C", *rp()), pt("D", *rp()), "v = Vector(C, D)",
                   "Q = Translate(P, v)"],
     [mv("C", *rp()), mv("D", *rp()), mv("D", 1, 1), mv("C", 1, 1), mv("D", *rp())])
# a line, a segment and circles
for kind in ("line", "segment", "circle", "circle_r"):
    for _ in range(2):
        a, b, u, pa = rp(), rp(), rp(), rp()
        if kind == "circle_r":
            script = [pt("A", *a), num("r", abs(r2(1, 5))), "k = Circle(A, r)"]
            obj = "k"
            steps = [setn("r", 0), setn("r", 3.5)]
        else:
            make = {"line": "l = Line(A, B)", "segment": "l = Segment(A, B)",
                    "circle": "l = Circle(A, B)"}[kind]
            script = [pt("A", *a), pt("B", *b), make]
            obj = "l"
            # B onto A, then apart again
            steps = [mv("B", *pa), mv("B", *rp())]
        script += [vec("u", *u), f"t = Translate({obj}, u)"]
        case("translate", script,
             [mv("A", *pa), mv("u", *rp())] + steps + [mv("u", 0, 0), mv("u", *rp())])
# rays of both kinds
for make in ("g = Ray(A, B)", "g = Ray(A, w)"):
    for _ in range(2):
        a, b, w = rp(), rp(), rp()
        script = [pt("A", *a), pt("B", *b), vec("w", *w), vec("u", *rp()), make,
                  "t = Translate(g, u)"]
        if "B)" in make:
            # the direction flips, then B meets A
            moves = [mv("B", 2 * a[0] - b[0], 2 * a[1] - b[1]), mv("B", *a)]
        else:
            moves = [mv("w", -w[0], -w[1]), mv("w", 0, 0)]
        case("translate", script,
             [mv("u", *rp())] + moves + [mv(moves[0]["label"], *rp()), mv("A", *rp()),
                                         mv("u", 0, 0), mv("u", *rp())])
# vectors: by a vector (unchanged) and to a point
case("translate", [pt("A", *rp()), pt("B", *rp()), vec("u", *rp()), "v = Vector(A, B)",
                   "t = Translate(v, u)", "s = Translate(v, B)"],
     [mv("u", *rp()), mv("B", *rp()), mv("A", *rp()), mv("B", 0, 0), mv("A", 0, 0)])
# the vector is undefined: the translation is undefined too
case("translate", [pt("A", 0, 0), pt("B", 4, 1), pt("C", 1, 3), pt("D", 2, -2),
                   "l = Line(A, B)", "m = Line(C, D)", "S = Intersect(l, m)", "v = Vector(S)",
                   pt("P", 3, 3), "Q = Translate(P, v)", "t = Translate(l, v)",
                   "k = Circle(P, C)", "h = Translate(k, v)"],
     [mv("D", 5, 4), mv("D", 3, -1), mv("P", -2, 1)])

# ------------------------------------------------------- Direction
topic("direction")
for make in ("l = Line(A, B)", "l = Segment(A, B)", "l = Ray(A, B)"):
    for _ in range(2):
        a, b = rp(), rp()
        case("direction", [pt("A", *a), pt("B", *b), make, "d = Direction(l)"],
             [mv("B", *rp()), mv("B", *a), mv("A", *rp()), mv("B", *rp())])
for make in ("l = Ray(A, u)", "l = Line(A, u)", "l = OrthogonalLine(A, u)"):
    u = rp()
    case("direction", [pt("A", *rp()), vec("u", *u), make, "d = Direction(l)"],
         [mv("u", *rp()), mv("A", *rp()), mv("u", 0, 0), mv("u", -u[0], -u[1])])
# parallel and perpendicular lines through a free point
for make in ("m = Line(P, l)", "m = PerpendicularLine(P, l)", "m = OrthogonalLine(P, l)"):
    a, b = rp(), rp()
    case("direction", [pt("A", *a), pt("B", *b), pt("P", *rp()), "l = Line(A, B)", make,
                       "d = Direction(m)", "o = OrthogonalVector(m)"],
         [mv("B", *rp()), mv("P", *rp()), mv("B", *a), mv("B", *rp())])
# a perpendicular to a perpendicular to a short segment: GeoGebra rescales
case("direction", [pt("A", 0.01, 0.02), pt("B", 0.012, 0.025), pt("P", 3, 1),
                   "s = Segment(A, B)", "m = PerpendicularLine(P, s)",
                   "n = PerpendicularLine(A, m)", "d = Direction(m)", "e = Direction(n)"],
     [mv("B", 0.011, 0.021), mv("B", 50, 70), mv("P", -2, 4)])
# moves that change only the raw scale of the line: the stored line stays
# the same, the direction and the orthogonal vector change length
for make in ("l = Line(A, B)", "l = Segment(A, B)", "l = Ray(A, B)"):
    case("direction", [pt("A", 1, 2), pt("B", 4, 6), make, "d = Direction(l)",
                       "o = OrthogonalVector(l)"],
         [mv("B", 7, 10), mv("B", 2.5, 4), mv("A", -5, -6), mv("B", -2, -2)])
for make in ("l = Ray(A, u)", "l = Line(A, u)", "l = OrthogonalLine(A, u)"):
    case("direction", [pt("A", 1, 2), vec("u", 3, -1), make, "d = Direction(l)",
                       "o = OrthogonalVector(l)"],
         [mv("u", 6, -2), mv("u", 1.5, -0.5), mv("A", 4, 1), mv("u", -3, 1)])
case("direction", [pt("A", 1, 2), pt("B", 4, 6), pt("P", 5, 1), vec("u", 2, 3),
                   "l = Line(A, B)", "m = Line(P, l)", "n = PerpendicularLine(P, l)",
                   "t = Translate(l, u)", "dm = Direction(m)", "dn = Direction(n)",
                   "dt = Direction(t)", "ot = OrthogonalVector(t)"],
     [mv("B", 7, 10), mv("A", -2, -2), mv("B", 2.5, 4), mv("u", -1, 0)])

# ------------------------------------------------------- unit and orthogonal vectors
topic("unit")
for make in ("l = Line(A, B)", "l = Segment(A, B)", "l = Ray(A, B)"):
    for name in ("UnitVector", "OrthogonalVector", "UnitOrthogonalVector",
                 "UnitPerpendicularVector", "PerpendicularVector"):
        a, b = rp(), rp()
        case("unit", [pt("A", *a), pt("B", *b), make, f"q = {name}(l)"],
             [mv("B", *rp()), mv("B", *a), mv("A", *rp()), mv("B", *(2 * a[0], 2 * a[1]))])
for name in ("UnitVector", "OrthogonalVector", "UnitOrthogonalVector"):
    case("unit", [pt("A", *rp()), pt("B", *rp()), "v = Vector(A, B)", f"q = {name}(v)"],
         [mv("B", *rp()), mv("A", 1, 1), mv("B", 1, 1), mv("B", *rp())])

# ------------------------------------------------------- Length of a vector
topic("length")
case("length", [pt("A", *rp()), pt("B", *rp()), "v = Vector(A, B)", "n = Length(v)",
                pt("C", *rp()), "k = Circle(C, n)"],
     [mv("B", *rp()), mv("A", 2, 2), mv("B", 2, 2), mv("A", *rp())])
case("length", [pt("A", *rp()), "w = Vector(A)", "n = Length(w)"],
     [mv("A", 0, 0), mv("A", -3, 4), mv("A", 1e7, 1e-7)])

# ------------------------------------------------------- chains
topic("chain")
# translate a translated object, several kinds
case("chain", [pt("A", *rp()), pt("B", *rp()), vec("u", *rp()), vec("v", *rp()),
               "g = Ray(A, B)", "s = Segment(A, B)", "l = Line(A, B)",
               "ga = Translate(g, u)", "gb = Translate(ga, v)", "sa = Translate(s, u)",
               "sb = Translate(sa, v)", "la = Translate(l, u)", "lb = Translate(la, v)"],
     [mv("u", *rp()), mv("B", *rp()), mv("v", 0, 0), mv("B", *rp()), mv("A", *rp())])
case("chain", [pt("A", *rp()), vec("w", *rp()), vec("u", *rp()), "g = Ray(A, w)",
               "ga = Translate(g, u)", "gb = Translate(ga, u)", "d = Direction(gb)",
               "q = UnitVector(gb)"],
     [mv("w", *rp()), mv("u", *rp()), mv("w", 0, 0), mv("w", *rp())])
# direction of a translated line, and a line through a point with that direction
case("chain", [pt("A", *rp()), pt("B", *rp()), pt("P", *rp()), vec("u", *rp()),
               "l = Line(A, B)", "t = Translate(l, u)", "d = Direction(t)", "m = Line(P, d)",
               "n = OrthogonalLine(P, d)", "o = OrthogonalVector(t)"],
     [mv("B", *rp()), mv("u", *rp()), mv("A", *rp()), mv("B", *rp())])
# a ray from a translated point, a vector between translated points
case("chain", [pt("A", *rp()), pt("B", *rp()), vec("u", *rp()), "C = Translate(A, u)",
               "D = Translate(B, u)", "v = Vector(C, D)", "g = Ray(C, v)", "h = Ray(D, C)",
               "w = Vector(A, B)", "n = Length(v)"],
     [mv("u", *rp()), mv("B", *rp()), mv("A", *rp()), mv("u", 0, 0)])
# a vector built from vectors, its unit vector moves a circle
case("chain", [pt("A", *rp()), pt("B", *rp()), "v = Vector(A, B)", "o = OrthogonalVector(v)",
               "q = UnitOrthogonalVector(o)", pt("C", *rp()), num("r", 2), "k = Circle(C, r)",
               "t = Translate(k, q)", "M = Translate(C, o)"],
     [mv("B", *rp()), setn("r", 0.5), mv("A", *rp()), mv("B", *rp())])

# ------------------------------------------------------- raw scale of other lines
topic("rawscale")
# lines from other groups: their stored coefficients are normalised, so
# Direction and OrthogonalVector differ in length from GeoGebra's
case("rawscale", [pt("A", *rp()), pt("B", *rp()), "l = LineBisector(A, B)",
                  "d = Direction(l)", "q = UnitVector(l)"],
     [mv("B", *rp()), mv("A", *rp())])
case("rawscale", [pt("A", *rp()), pt("B", *rp()), pt("Q", *rp()), "l = Line(A, B)",
                  "m = Mirror(l, Q)", "o = OrthogonalVector(m)", "q = UnitOrthogonalVector(m)"],
     [mv("B", *rp()), mv("Q", *rp())])
# a chain of perpendicular lines: the raw scale of n depends on P, which
# moves m but leaves n where it is
case("rawscale", [pt("A", 0, 0), pt("B", 0.001, 0), pt("P", 0, 0), pt("Q", 0, 3),
                  "l = Line(A, B)", "m = PerpendicularLine(P, l)",
                  "n = PerpendicularLine(Q, m)", "d = Direction(n)"],
     [mv("P", 5, 0), mv("P", -2, 0), mv("B", 0.002, 0)])
# the same through a parallel line
case("rawscale", [pt("A", 0, 0), pt("B", 0.001, 0), pt("P", 0, 0), pt("Q", 0, 3),
                  "l = Line(A, B)", "m = PerpendicularLine(P, l)", "n = Line(Q, m)",
                  "o = OrthogonalVector(n)"],
     [mv("P", 5, 0), mv("P", -2, 0), mv("B", 0.002, 0)])

cases.write()
