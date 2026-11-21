"""Boundary output labels and conic tangency and permutation regressions."""

from caselib import Cases, mv, setn

cases = Cases("intersectreview")


def growing(topic, script, steps):
    cases.case(topic, script, steps)
    cases.cases[-1]["track_boundary_growth"] = True


hexagon = [
    "A = (-3, -2)", "B = (3, -2)", "C = (3, 3)",
    "D = (1, 0)", "E = (-1, 3)", "F = (-3, 3)",
    "q = Polygon(A, B, C, D, E, F)",
    "p = PolyLine(A, B, C, D, E, F)",
]
for carrier, path in (("Line", "q"), ("Segment", "p"), ("Ray", "q"), ("PolyLine", "p")):
    growing("growth", hexagon + [
        "G = (-4, 5)", "H = (4, 5)", f"g = {carrier}(G, H)",
        f"R = Intersect(g, {path})",
    ], [mv("G", -4, 2.5), mv("H", 4, 2.5), mv("G", -4, 1), mv("H", 4, 1)])
growing("growth", hexagon + [
    "G = (-4, 5)", "H = (4, 5)", "I = (4, 6)", "J = (-4, 6)",
    "w = Polygon(G, H, I, J)", "R = Intersect(q, w)",
], [mv("G", -4, 2.5), mv("H", 4, 2.5), mv("I", 4, 1), mv("J", -4, 1)])
growing("labels", [
    "A = (0, 0)", "B = (2, 0)", "p = PolyLine(A, B)",
    "G = (0, 1)", "H = (1, 1)", "g = Line(G, H)", "R = Intersect(g, p)",
], [mv("H", 1, -1), mv("H", 1, 1)])
cases.case("labels", [
    "A = (2, 0)", "B = (3, 1)", "p = PolyLine(A, B)",
    "O = (0, 0)", "r = 2", "k = Circle(O, r)", "R = Intersect(k, p)",
], [setn("r", 1), setn("r", 2.5), setn("r", 2)])

circle = ["O = (0, 0)", "r = 2", "k = Circle(O, r)"]
cases.case("tangent", ["a = 1", *circle, "h = Conic({0, 0, 2a, 1, -a, -2})",
                        "R = Intersect(k, h)"], [setn("a", 4), setn("a", 1)])
product = [*circle, "h = Conic({0, 0, -a, 1, 0, 0})", "R = Intersect(k, h)"]
cases.case("tangent", ["a = 1.5", *product], [
    setn("a", a) for a in (2, 2 + 1e-11, 2 + 1e-9, 2 + 1e-7, 1.5)
])
cases.case("tangent", ["a = 2", *product], [setn("a", 1.5), setn("a", 2)])
cases.case("near", [
    "a = 0.5", "k = Conic({1, 0, 0, 0, 0, -1})",
    "h = Conic({1, 1, a^2 - 1, 0, 0, -2a})", "R = Intersect(k, h)",
], [setn("a", a) for a in (1 - 1e-9, 1, 1 + 1e-11, 1 + 1e-9, 1 + 1e-7)])
cases.case("near", [
    "a = 3", "k = Conic({1, 4, -16, 0, 0, 0})",
    "h = Conic({4, 1, -a, 0, 0, 0})", "R = Intersect(h, k, 2)",
], [setn("a", a) for a in (4 - 1e-7, 4 - 1e-9, 4, 4 + 1e-11, 4 + 1e-9, 4 + 1e-7)])
cases.case("near", ["a = 8", *circle, "h = Conic({1, 4, -a, 0, 0, 0})",
                     "R = Intersect(k, h)"], [
    setn("a", a) for a in (16 - 1e-7, 16 - 1e-9, 16 - 1e-11, 16, 16 + 1e-11, 16 + 1e-9, 16 + 1e-7)
])

parabola_hyperbola = [
    "F = (0, 1)", "d: y = -1", "G = (-3, 0)", "H = (3, 0)", "Q = (2, 1)",
    "k = Parabola(F, d)", "h = Hyperbola(G, H, Q)",
]
turn = [
    mv("F", -1.371405, 4.129732), mv("F", -2.241806, -1.676317),
    mv("Q", -3.39776, 1.899624), mv("F", 0, 1), mv("Q", 2, 1),
]
cases.case("permutation", parabola_hyperbola + ["R = Intersect(k, h)"], turn)
for index in (2, 3, 4):
    cases.case("permutation", parabola_hyperbola + [f"R = Intersect(k, h, {index})"], turn)
cases.case("permutation", parabola_hyperbola + [
    "R = Intersect(k, h)", "S = Intersect(k, h, 2)",
], turn)
cases.write()
