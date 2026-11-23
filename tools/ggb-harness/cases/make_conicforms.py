"""Conics from coefficients, centers and poles."""

from caselib import Cases, mv, setn

c = Cases("conicforms")
reference = Cases("conicforms")

c.case("coefficients", [
    "a = 4", "b = 1", "cc = -16", "d = 0", "ee = 0", "ff = 0",
    "k = Conic(a, b, cc, d, ee, ff)", "C = Center(k)",
], [setn("d", 2), setn("cc", 0), setn("d", 0), setn("b", -1), setn("cc", -16)])
c.case("coefficients", [
    "a = 4", "k = Conic(a, 1, -16, 0, 0, 0)", "m = Conic(4, 1, -16, 0, 0, 0)",
], [setn("a", -4), setn("a", 1)])
c.case("coefficients", [
    "A = (0, -4)", "k = Conic(1, 1, y(A), 0, 0, 0)",
], [mv("A", 0, 0), mv("A", 0, 4), mv("A", 0, -2)])

c.case("list", [
    "a = -4", "l = {1, 1, a, 0, 0, 0}", "k = Conic(l)",
], [setn("a", 0), setn("a", 4), setn("a", -9)])
c.case("list", [
    "a = 2", "l = {{4, a, -2}, {0, 2, 0}, {0, -4, -9}}", "k = Conic(l)",
    "C = Center(k)",
], [setn("a", 4), setn("a", 8), setn("a", -2)])
c.case("list", [
    "n = 6", "l = First({1, 4, -4, 0, 0, 0}, n)", "k = Conic(l)",
], [setn("n", 2), setn("n", 3), setn("n", 6)])
c.case("list", [
    "A = (0, -4)", "k = Conic({1, 1, y(A), 0, 0, 0})",
    "m = Conic({{4, 2, -2}, {0, 2, 0}, {0, -4, -9}})",
], [mv("A", 0, 0), mv("A", 0, 4), mv("A", 0, -2)])

c.case("center", [
    "A = (2, 4)", "B = (-1, 0)", "s = Segment(A, B)",
    "C = Center(A, B)", "D = Center(s)", "d = Distance(C, A)",
], [mv("A", -1, 0), mv("B", 6, -4), mv("A", 2, 4)])
c.case("center", [
    "A = (0, 0)", "B = (4, 4)", "C = (0, 4)", "D = (4, 0)",
    "g = Line(A, B)", "h = Line(C, D)", "P = Intersect(g, h)",
    "s = Segment(A, P)", "M = Center(s)", "N = Center(A, P)",
], [mv("D", 4, 8), mv("D", 0, 4), mv("D", 4, 0)])

c.case("interval", [
    "a = -4", "b = 2", "m = Center(a < x < b)",
    "n = Center((x >= a) && (x <= b))", "q = Center(x < b)",
], [setn("b", -4), setn("b", -4.000000001), setn("b", -6), setn("a", -8), setn("b", 4)])
c.case("interval", [
    "A = (-5, 1)", "m = Center(3 >= x > x(A))",
    "n = Center((x > x(A)) && (x > -2) && (x < 3))",
], [mv("A", -1, 1), mv("A", 6, 2), mv("A", -5, 1)])
reference.case("namedinterval", [
    "a = -4", "b = 2", "f(x) = a < x < b", "m = Center(f)",
], [setn("b", -6), setn("b", 4)])

reference.case("namedmidpoint", [
    "a = -4", "b = 2", "f(x) = a < x < b", "m = Midpoint(f)",
], [setn("b", -6), setn("b", 4)])

c.case("pole", [
    "A = (1, 0)", "B = (1, 2)", "g = Line(A, B)", "a = 5",
    "F = (-3, 0)", "G = (3, 0)", "k = Ellipse(F, G, a)",
    "P = Polar(g, k)", "h = Polar(P, k)",
], [mv("B", 1, 0), mv("B", 1, 2), mv("A", 3, 2), mv("B", 0, 2),
    setn("a", 3), setn("a", 5)])
c.case("pole", [
    "A = (0, 2)", "B = (2, 2)", "F = (0, 0)",
    "H = (0, -2)", "J = (2, -2)", "g = Line(A, B)", "h = Line(H, J)",
    "k = Parabola(F, h)", "P = Polar(g, k)", "Apex = Polar(h, k)",
], [mv("J", 2, 2), mv("A", 0, 0), mv("F", 0, -2), mv("F", 0, 0)])

c.write()
reference.write("reference")
