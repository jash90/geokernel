"""Geometry form regressions recorded during review."""

from caselib import Cases, mv, setn

cases = Cases("geomformsreview")
case = cases.case

for slope in ("x/y*2", "x*(1/y)", "(x/y)/2", "x*y^(-1)"):
    case("ode", ["P = (1, 1)", f"loc = Locus({slope}, P)"], [])

case("odenamed", ["f = x/y*2", "P = (1, 1)", "loc = Locus(f, P)"], [])

case("angle", [
    "a = 180°", "n = 3.141592653589793",
    "u = AreEqual(n, a)", "v = AreEqual(a, n)",
    "A = (0, 0)", "B = (2, 0)", "C = (0, 3.141592653589793)",
    "p = Polygon(A, B, C)", "w = AreCongruent(p, a)", "q = AreCongruent(a, p)",
], [setn("n", 2), mv("C", 0, 2)])

case("shapes", [
    "A = (0, 0)", "B = (2, 0)", "C = (4, 0)", "D = (2, 0)", "E = (2, 0)",
    "s = Segment(A, B)", "t = Segment(D, A)",
    "g = Line(A, B)", "h = Line(A, C)",
    "c = Circle(A, B)", "d = Circle(A, E)",
    "l = {{s}, g, c}", "m = {{t}, h, d}",
    "u = AreEqual(l, m)",
], [mv("C", 4, 1), mv("C", 4, 0), mv("E", 3, 0), mv("E", 2, 0), mv("D", 3, 0)])

case("function", [
    "a = 1.000000005", "f(x) = x^4", "g(x) = a*x^4", "u = AreEqual(f, g)",
], [setn("a", 1), setn("a", 1.000000005)])

case("booleanfunction", [
    "f(x) = x>0", "g(x) = x>=0", "h(x) = 0<x", "j(x) = 1",
    "u = AreEqual(f, g)", "v = AreEqual(f, h)", "w = AreEqual(f, j)",
], [])

case("functionofy", [
    "f(y) = y>0", "g(y) = y>=0", "h(y) = 0<y", "j(x) = x>0",
    "u = AreEqual(f, g)", "v = AreEqual(f, h)", "w = AreEqual(f, j)",
], [])

case("orthogonalspace", [
    "P = (2, 2)", "g: y = 2", "h = OrthogonalLine(P, g, space)",
], [mv("P", 3, -1)])


case("orthogonalgrouped", [
    "P = (2, 2)", "g: y = 2", "h = OrthogonalLine(P, g, (space))",
], [mv("P", 3, -1)])

cases.write()
