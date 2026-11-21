"""Commands that use the two built-in coordinate axes as lines."""

from caselib import Cases, mv, setn

c = Cases("axes")

c.case("lines", [
    "A = (2, 3)", "B = (5, -3)", "f = Line(A, B)",
    "P = Intersect(f, xAxis)", "O = Intersect(yAxis, xAxis)",
    "R = Reflect(A, yAxis)", "n = PerpendicularLine(A, xAxis)",
    "d = Distance(A, xAxis)", "elevation = Distance(yAxis, A)",
], [mv("A", 2, 0), mv("B", 5, 0), mv("A", 0, 0), mv("B", 0, 0),
    mv("A", -1, -2), mv("B", 5, 2)])

c.case("conics", [
    "A = (1, 1)", "r = 3", "k = Circle(A, r)",
    "P = Intersect(k, xAxis, 1)", "Q = Intersect(yAxis, k, 2)",
    "F = (0, 2)", "p = Parabola(F, xAxis)", "T = Intersect(p, yAxis, 1)",
    "d = Distance(F, xAxis)",
], [setn("r", 1), setn("r", 0), setn("r", 4), mv("F", 1, 0), mv("F", 2, -4)])

reference = Cases("axes")
reference.case("functions", [
    "a = 2", "f(x) = x - a", "P = Intersect(f, xAxis)",
], [setn("a", 0), setn("a", -3)])

c.write()
reference.write("reference")
