"""Replay conic intersections and boundary output order through degeneracies."""

from caselib import Cases, mv, setn

cases = Cases("intersectconics")

# Four intersections coalesce on either coordinate axis, then disappear.
ellipses = [
    "a = 4", "k = Conic({1, 4, -16, 0, 0, 0})",
    "h = Conic({4, 1, a - 16, 0, 0, 0})",
]
cross_tangencies = [setn("a", a) for a in (12, 13, 4, -48, -49, 4)]
cases.case("pencil", ellipses + ["R = Intersect(k, h)"], cross_tangencies)
cases.case("index", ellipses + [
    "n = 1", "R = Intersect(k, h, n)", "S = Intersect(h, k, 2)",
    "T = Intersect(h, k, 4)",
], [setn("n", n) for n in (2, 3, 4, 5, 0, -4, 2.9)] + cross_tangencies)
cases.case("seed", ellipses + ["P = (3, 3)", "R = Intersect(k, h, P)"], [
    mv("P", -3, 3), mv("P", -3, -3), mv("P", 3, -3), mv("P", 0, 0),
] + cross_tangencies)

# CommandsTest.intersectConicConic and cmdIntersect.
cases.case("pencil", [
    "k = Conic({1, 0, 11, 0, -6, -2})",
    "h = Conic({1, 1, 12, 0, -6, -4})",
    "R = Intersect(k, h)",
], [])
cases.case("pencil", [
    "O = (0, 0)", "r = 5", "k = Circle(O, r)", "a = 12",
    "h = Conic({0, 0, -a, 1, 0, 0})", "R = Intersect(k, h)",
], [setn("a", a) for a in (12.5, 13, 0, -12, 12)])
cases.case("index", [
    "a = 2", "k = Conic({1, 0, -4, 0, 0, 0})",
    "h = Conic({1, 0, -a^2, 0, 0, 0})", "R = Intersect(k, h, 1)",
], [setn("a", 3), setn("a", 2)])
cases.case("index", [
    "A = (2, 0)", "r = 2", "k = Circle(A, r)",
    "h = Conic({1, 1, 0, 0, 0, 0})", "R = Intersect(h, k, 1)",
], [setn("r", 3), mv("A", 0, 1), setn("r", 1), setn("r", 0), mv("A", 0, 0)])
cases.case("seed", [
    "a = 0", "k = Conic({1, 0, -4, 0, 0, 0})",
    "h = Conic({1, 0, -4, 0, 0, a})", "P = (2, 1)",
    "R = Intersect(k, h, P)",
], [setn("a", 1), mv("P", -2, -1), setn("a", 0)])

# IntersectToolTest conic-conic click positions, expressed as live selectors.
tool_circles = [
    "A = (2, -2)", "B = (4, -2)", "r = 2",
    "k = Circle(A, r)", "h = Circle(B, r)",
]
cases.case("pencil", tool_circles + ["R = Intersect(k, h)"], [
    mv("B", 6, -2), mv("B", 7, -2), mv("B", 4, -2),
])
cases.case("seed", tool_circles + ["P = (3, -0.26)", "R = Intersect(k, h, P)"], [
    mv("P", 3, -4), mv("B", 6, -2), mv("B", 7, -2), mv("B", 4, -2),
])
cases.case("pencil", [
    "A = (0, 0)", "B = (6, 0)", "P = (3, 4)",
    "k = Circle(A, P)", "h = Circle(B, P)", "R = Intersect(k, h)",
], [mv("B", 0, 0), mv("B", 6, 0), mv("P", 3, -4)])

boundary = [
    "A = (-2, -2)", "B = (2, -2)", "C = (2, 2)", "D = (-2, 2)",
    "p = PolyLine(A, B, C, D)", "q = Polygon(A, B, C, D)",
]
line = ["G = (1, -4)", "H = (1, 4)", "g = Line(G, H)"]
for path in ("p", "q"):
    for pair in (f"g, {path}", f"{path}, g"):
        cases.case("lineboundary", boundary + line + [f"R = Intersect({pair})"], [
            mv("G", 4, -1), mv("H", -4, 1),
            mv("G", 4, 2), mv("H", -4, 2),
            mv("G", 4, 3), mv("H", -4, 3),
            mv("H", -4, -1), mv("G", 4, -1),
        ])

other = [
    "E = (-1, -3)", "F = (-1, 3)", "G = (1, 3)", "H = (1, -3)",
    "u = PolyLine(E, F, G, H)", "v = Polygon(E, F, G, H)",
]
for pair in ("p, u", "p, v", "v, p", "q, v"):
    cases.case("pathboundary", boundary + other + [f"R = Intersect({pair})"], [
        mv("E", -3, 2), mv("F", 3, 2), mv("E", -3, 4), mv("F", 3, 4),
        mv("F", -1, 3), mv("E", -1, -3),
    ])

# IntersectToolTest line-polygon and polyline-polyline crossings.
cases.case("lineboundary", [
    "A = (0, -1)", "B = (4, -1)", "C = (4, -3)", "D = (0, -3)",
    "p = Polygon(A, B, C, D)", "g: y = -2", "R = Intersect(g, p)",
], [mv("A", 0, -2), mv("A", 0, -3), mv("D", 0, -1)])
cases.case("pathboundary", [
    "A = (0, -1)", "B = (4, -3)", "C = (0, -3)", "D = (4, -1)",
    "p = PolyLine(A, B)", "q = PolyLine(C, D)", "R = Intersect(p, q)",
], [mv("D", 4, -5), mv("C", 0, -1), mv("D", 4, -3), mv("D", 4, -1)])
cases.case("lineboundary", boundary + [
    "G = (0, 0)", "H = (1, 1)", "g = Line(G, H)", "R = Intersect(g, p)",
], [mv("H", 1, 0), mv("H", 0, 1), mv("G", 3, 1), mv("H", 4, 1)])

for path in ("p", "q"):
    for pair in (f"k, {path}", f"{path}, k"):
        cases.case("conicboundary", boundary + [
            "O = (0, 0)", "r = 2.5", "k = Conic({1, 1, -r^2, 0, 0, 0})",
            f"R = Intersect({pair})",
        ], [setn("r", r) for r in (2, 1.5, 2.5, 3, 2.5)])

# The same geometric conic can be a circle object or a limited conic.
cases.case("conicboundary", boundary + [
    "O = (0, 0)", "r = 2.5", "k = Circle(O, r)", "R = Intersect(q, k)",
], [setn("r", 2), setn("r", 1.5), setn("r", 2.5)])
cases.case("conicboundary", boundary + [
    "O = (0, 0)", "r = 2.5", "S = (r, 0)", "T = (0, r)",
    "k = CircularArc(O, S, T)", "R = Intersect(q, k)",
], [setn("r", 2), setn("r", 1.5), mv("O", 0.25, 0), setn("r", 2.5)])
cases.case("pencil", [
    "O = (0, 0)", "S = (5, 0)", "T = (0, 5)", "k = CircularArc(O, S, T)",
    "a = 12", "h = Conic({0, 0, -a, 1, 0, 0})", "R = Intersect(k, h)",
], [setn("a", a) for a in (13, 12, -12, 0, 12)])

cases.write()
