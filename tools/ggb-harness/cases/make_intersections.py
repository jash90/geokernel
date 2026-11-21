"""Inputs adapted from CommandsTest plus moving and degenerate intersections."""

from caselib import Cases, mv, setn

cases = Cases("intersections")
reference = Cases("intersections")
reference_topics = {
    "boundary", "boundaryconic", "boundaryfunction", "boundaryseed", "conics",
    "functionconic", "functionline", "functions", "interval", "limitedconic",
    "upstreamconics",
}


def case(topic, objects, arguments, steps=()):
    corpus = reference if topic in reference_topics else cases
    corpus.case(topic, [*objects, f"R = Intersect({arguments})"], list(steps))


def both(topic, objects, a, b, extra="", steps=()):
    case(topic, objects, f"{a}, {b}{extra}", steps)
    case(topic, objects, f"{b}, {a}{extra}", steps)


# intersectLineLineWithOrWithoutIndex and cmdIntersectPathHomogeneousCoords.
for x, y, index, reverse in [
    (4, 4, ", 1", False), (5, 5, ", 2", True),
    (3, 3, "", False), (2, 2, "", True), (-3, 2, "", False),
]:
    case("lines", [
        f"A = ({x}, 0)", f"B = ({x}, 1)", f"C = (0, {y})", f"D = (1, {y})",
        "g = Line(A, B)", "h = Line(C, D)",
    ], ("h, g" if reverse else "g, h") + index)
case("lines", [
    "A = (0, 0)", "B = (0, 1)", "C = (0, 2)", "D = (1, 2)",
    "g = Line(A, B)", "h = Line(C, D)", "n = 7",
], "g, h, n", [setn("n", -3), mv("D", 0, 3), mv("C", 1, 2), mv("D", 1, 3)])
case("limited", [
    "A = (-2, 0)", "B = (1, 0)", "C = (0, 1)", "D = (0, 2)",
    "s = Segment(A, B)", "g = Ray(C, D)",
], "s, g", [mv("D", 0, -1), mv("B", -1, 0), mv("B", 3, 0)])
case("limitedconic", [
    "O = (0, 0)", "r = 2", "k = Circle(O, r)", "A = (0, 0)", "B = (3, 0)",
    "s = Segment(A, B)", "n = 1",
], "s, k, n", [setn("n", 2), mv("A", -3, 0), setn("n", 4), setn("n", 0)])
case("limitedconic", [
    "O = (0, 0)", "A = (2, 0)", "B = (0, 2)", "k = CircularArc(O, A, B)",
    "C = (1, -3)", "D = (1, 3)", "g = Line(C, D)", "P = (1, 2)",
], "g, k, P", [mv("P", 1, -3), mv("D", 3, -3), mv("C", 1, 1)])

# cmdIntersect — preserve the paths and the repeated polygon vertex outputs.
diag = ["G = (0, 0)", "H = (1, 1)", "g = Line(G, H)"]
paths = ["A = (-1, -2)", "B = (-1, 3)", "C = (5, 3)",
         "p = PolyLine(A, B, C)", "q = Polygon(A, B, C)"]
other = ["D = (1, -2)", "E = (1, 4)", "F = (5, 3)",
         "v = PolyLine(D, E, F)", "w = Polygon(D, E, F)"]
both("boundary", paths + diag, "g", "p", steps=[mv("H", 1, 0), mv("G", 0, 3), mv("H", 1, 3)])
both("boundary", paths + diag, "g", "q", steps=[mv("H", 1, 0), mv("G", 0, 3), mv("H", 1, 3)])
case("boundary", paths + other, "v, p", [mv("F", 5, 2), mv("E", 1, -3)])
both("boundary", paths + other, "v", "q", steps=[mv("F", 5, 2), mv("E", 1, -3)])
case("boundary", paths + other, "w, q", [mv("F", 5, 2), mv("E", 1, -3)])

for path in ("p", "q"):
    both("boundaryconic", paths + [
        "O = (-1, 3)", "r = 1", "k = Conic({1, 1, 10 - r^2, 0, 2, -6})",
    ], path, "k", steps=[setn("r", 0), setn("r", 2), mv("B", -3, 3)])
    both("boundaryfunction", paths + ["aa = 0", "f(x) = x^2 + aa"], path, "f",
         steps=[setn("aa", 2), setn("aa", 4), setn("aa", -2)])
    both("boundaryseed", paths + ["f(x) = sin(x) + 2", "P = (0, 3)"], path, "f", ", P",
         steps=[mv("P", 3, 3), mv("B", -1, -1), mv("P", -1, 1)])

# cmdIntersect and cmdIntersectPoly — polynomial roots and starts near either branch.
poly_line = diag + ["a = 1", "f(x) = (x - 1)^2 + a", "P = (2.3, 2)", "n = 1"]
for extra, steps in [
    ("", [setn("a", 1.25), setn("a", 2), setn("a", 0)]),
    (", n", [setn("n", 2), setn("n", 3), setn("n", 0), setn("a", 2)]),
    (", P", [mv("P", 0.7, 1), mv("P", 2.3, 2), setn("a", 2)]),
]:
    both("functionline", poly_line, "f", "g", extra, steps)
case("functionline", diag + ["f(x) = sin(x)"], "g, f")
case("functionline", ["f(x) = (x * ln(x + 1)) / (exp(2x) - 1)"], "f, xAxis")
case("functionline", [
    "f(x) = -x^4 + 2*x^2 + 0.25*x + 5",
    "A = (0, 4.07875)", "B = (9.6, 5.07875)", "g = Line(A, B)",
], "f, g", [mv("A", 0, 9), mv("B", 9.6, 10)])
case("functionline", [
    "f(x) = sin(x)", "P = (3.1, 2)", "a = 0", "A = (0, a)", "B = (1, a)", "g = Line(A, B)",
], "f, g, P", [mv("P", 6.3, 1), setn("a", 0.5), setn("a", 2)])
poly_pair = ["a = 2", "f(x) = x^2 + 1", "h(x) = x^3 - x + a", "n = 1", "P = (-1.4, 2)"]
case("functions", poly_pair, "f, h", [setn("a", 1), setn("a", 4), setn("a", 2)])
case("functions", poly_pair, "f, h, n", [setn("n", 2), setn("n", 3), setn("n", 9), setn("a", 1)])
case("functions", poly_pair, "f, h, P", [mv("P", 1.2, 2), mv("P", -1.4, 2), setn("a", 1)])
case("functions", ["f(x) = x^2 + x", "h(x) = x^2 + x"], "f, h")
case("functions", ["f(x) = x^2 + x", "h(x) = x^2 + x + 1"], "f, h")
case("interval", [
    "a = -4", "b = 4", "n = 0", "f(x) = sin(x)", "h(x) = n*x",
], "f, h, a, b", [setn("a", 0.5), setn("b", 2), setn("a", -7), setn("b", 7)])
case("interval", ["f(x) = x^2", "h(x) = 0"], "f, h, -1, 1")

# Closest line/conic, including the upstream x=y and x^2+y^2=2 case.
both("lineconicseed", diag + ["k = Conic({1, 1, -2, 0, 0, 0})", "P = (-5, -3)"],
     "g", "k", ", P", [mv("P", 5, 3), mv("H", 0, 1), mv("G", 3, 1), mv("H", 3, 2)])
both("lineconicseed", diag + ["O = (0, 0)", "r = sqrt(2)", "k = Circle(O, r)", "P = (-5, -3)"],
     "g", "k", ", P", [mv("P", 5, 3), mv("P", 0, 0), mv("G", 3, 1), mv("H", 3, 2)])
case("conicline", [
    "F = (-3, 0)", "G = (3, 0)", "radius = 5", "k = Ellipse(F, G, radius)", "A = (-6, 0)", "B = (6, 0)", "g = Line(A, B)",
], "k, g", [mv("A", -6, 8), mv("B", 6, 8), mv("A", -6, 4), mv("B", 6, 4)])
case("conicline", [
    "F = (-3, 0)", "G = (3, 0)", "radius = 5", "k = Ellipse(F, G, radius)", "A = (-6, 0)", "B = (6, 0)",
    "g = Line(A, B)", "n = 1",
], "k, g, n", [setn("n", 2), setn("n", 3), setn("n", -2), mv("A", -6, 8), mv("B", 6, 8)])

# Upstream conic-conic equations expressed as the same six coefficients.
for a, b in [
    ("0, 0, -158, 1, 6, -5", "0, 0, -128, 1, 0, 0"),
    ("0, 7, -3, 0, 2, 14", "0, -6, -7, 0, -20, 0"),
    ("-5, 0, 0, 0, -9, 9", "2, 0, -7, 0, -12, 14"),
    ("0, 0, -10, -7, -10, -7", "0, 0, -9, 2, -9, 2"),
    ("0, 0, 15, 1, -1, 3", "1, 1, -143, 0, -12, -2"),
    ("0, 0, 88, -4, 2, -7", "1, 1, -3, 0, 2, 8"),
    ("-1, 0, 291, 0, 6, 20", "0, 0, 83, -1, -1, 2"),
    ("-7, 0, -118, 0, -10, 18", "-5, -5, 272, 0, -7, -3"),
    ("0, 4, -13, 0, 3, -6", "0, 0, 32, -7, -2, 8"),
    ("1, 0, 11, 0, -6, -2", "1, 1, 12, 0, -6, -4"),
    ("0, -9, 992, 0, -2, 8", "5, 5, -566, 0, -9, 6"),
    ("0, 0, 820, 7, 3, -9", "-7, -7, 1220, 0, -4, 14"),
    ("-9, 0, 1106, 0, 20, 2", "0, 0, -96, 5, 6, -2"),
    ("0, 0, 270, -10, 8, 7", "7, 7, -564, 0, 6, 4"),
    ("0, 0, -240, 5, -8, 2", "-3, 0, -643, 14, -6, 6"),
    ("0, 4, -66, 0, -9, 1", "-9, -9, 920, 0, -4, -2"),
    ("-4, -4, 405, 0, 9, -9", "1, 0, 108, 0, -2, -12"),
    ("0, 0, 108, 2, -2, -9", "0, 5, -724, -5, 6, 7"),
]:
    both("upstreamconics", [f"k = Conic({{{a}}})", f"h = Conic({{{b}}})"], "k", "h")

conics = ["a = 4", "k = Conic({1, 4, -16, 0, 0, 0})",
          "h = Conic({4, 1, -16 + a, 0, 0, 0})", "P = (2, 2)", "n = 1"]
case("conics", conics, "k, h", [setn("a", -30), setn("a", 12), setn("a", 17), setn("a", 4)])
case("conics", conics, "k, h, n", [setn("n", 4), setn("n", 5), setn("n", 2), setn("a", 16)])
case("conics", conics, "k, h, P", [mv("P", -2, -2), mv("P", 2, -2), setn("a", 16)])
case("circles", [
    "O = (-3.04, -0.11)", "C = (4.16, -0.11)", "ra = 12", "rb = 4.8", "k = Circle(O, ra)", "h = Circle(C, rb)",
], "k, h", [mv("C", 4.2, -0.11), mv("C", 20, -0.11)])
case("circles", [
    "O = (0, 0)", "C = (6, 0)", "radius = 5", "k = Circle(O, radius)", "h = Circle(C, radius)", "P = (3, 4)", "n = 1",
], "k, h, P", [mv("P", 3, -4), mv("C", 10, 0), mv("C", 0, 0), mv("C", 11, 0)])
case("circles", [
    "O = (0, 0)", "C = (6, 0)", "radius = 5", "k = Circle(O, radius)", "h = Circle(C, radius)", "n = 1",
], "k, h, n", [setn("n", 2), setn("n", 3), setn("n", 0), mv("C", 10, 0)])

# The polynomial/conic substitution includes repeated, disappearing and extra roots.
for extra, steps in [
    ("", [setn("a", 0), setn("a", 5), setn("a", -6), setn("a", -1)]),
    (", n", [setn("n", 4), setn("n", 5), setn("n", 2), setn("a", 5)]),
]:
    both("functionconic", ["a = -1", "f(x) = x^2 + a",
         "k = Conic({1, 4, -16, 0, 0, 0})", "n = 1"], "f", "k", extra, steps)

# Path/point incidence from cmdIntersect.
both("pathpoint", diag + ["P = (2, 2)"], "g", "P", steps=[mv("P", 2, -2), mv("P", -3, -3)])
both("pathpoint", ["f(x) = x", "P = (2, 2)"], "f", "P", steps=[mv("P", 2, -2), mv("P", -3, -3)])
case("conicline", [
    "F = (-3, 0)", "G = (3, 0)", "radius = 5", "k = Ellipse(F, G, radius)",
    "A = (-6, 0)", "B = (6, 0)", "g = Line(A, B)",
], "k, g", [mv("B", 6, 2), mv("B", 6, -2)])
case("conicline", [
    "F = (-3, 0)", "G = (3, 0)", "radius = 5", "k = Ellipse(F, G, radius)",
    "A = (-6, 4)", "B = (6, 4)", "g = Line(A, B)",
], "k, g", [mv("B", 6, 8), mv("A", -6, 8)])
# Well-conditioned seeds also exercise branch switching at full precision.
case("functions", ["f(x) = x^2", "h(x) = 4", "P = (-2, 0)"], "f, h, P",
     [mv("P", 2, 0), mv("P", -2, 0)])
for path in ("p", "q"):
    both("boundaryseed", paths + ["f(x) = sin(x)", "P = (-1, 0)"], path, "f", ", P",
         steps=[mv("P", 1, 3), mv("B", -1, -1), mv("P", -1, 0)])
both("lineconicseed", [
    "F = (-3, 0)", "G = (3, 0)", "radius = 5", "k = Ellipse(F, G, radius)",
    "aa = 0", "A = (-6, aa)", "B = (6, aa)", "g = Line(A, B)", "P = (4, 1)",
], "g", "k", ", P", [mv("P", -4, 1), setn("aa", 4), setn("aa", 5), setn("aa", 0)])
case("lineconicseed", [
    "O = (0, 0)", "A = (0, 2)", "B = (0, -3)", "k = Circle(O, A)",
    "g = Line(A, B)", "P = (0, 4)",
], "g, k, P", [mv("P", 0, -4), mv("A", 2, 0), mv("P", 2, 0)])
both("conicline", [
    "F = (-3, 0)", "G = (3, 0)", "radius = 5", "k = Ellipse(F, G, radius)",
    "A = (-6, 0)", "B = (6, 0)", "g = Line(A, B)",
], "k", "g", ", 9", [mv("A", -6, 1), mv("B", 6, 1)])
both("pathpoint", [
    "A = (0, 0)", "B = (2, 0)", "C = (2, 2)", "q = Polygon(A, B, C)", "P = (1, 0)",
], "q", "P", steps=[mv("A", 0, 1), mv("P", 1, 0.5), mv("P", 1, 1)])
cases.write()
reference.write("reference")
