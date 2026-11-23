"""Bounded line/conic intersections and shared selector slots."""

from caselib import Cases, mv, setn

cases = Cases("lineconic")
parabola = [
    "D = (0, -1)", "E = (1, -1)", "d = Line(D, E)", "F = (0, 1)",
    "k = Parabola(F, d)", "A = (-5, 1)", "B = (0, 1)",
]
cases.case("parabola", parabola + ["g = Segment(A, B)", "R = Intersect(g, k)"],
           [mv("B", 5, 1), mv("A", 0, 1), mv("B", 5, -1)])
cases.case("parabola", parabola + ["g = Ray(B, A)", "R = Intersect(k, g, (-3, 1))"],
           [mv("B", 5, 1), mv("A", 6, 1), mv("A", -5, 1)])
circle = ["O = (0, 0)", "k = Circle(O, 5)", "n = 5", "A = (-6, n)", "B = (6, n)"]
cases.case("tangent", circle + ["g = Segment(A, B)", "R = Intersect(g, k)"],
           [setn("n", 0), setn("n", 1), setn("n", 5), setn("n", 6)])
cases.case("shared", circle + [
    "C = (-6, 0)", "D = (6, 0)", "g = Line(C, D)", "R = Intersect(g, k)",
    "S = Intersect(k, g, 2)", "T = Intersect(g, k, (4, 0))",
], [mv("C", 8, 0), mv("C", -8, 0), mv("D", 8, 3), mv("D", 8, 5)])
cases.write()
