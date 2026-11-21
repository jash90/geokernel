"""Circle poles and the interval midpoint forms."""

from caselib import Cases, mv, setn

c = Cases("conicreview")

c.case("circlepole", [
    "rr = 2", "O = (0, 0)", "A = (1, 0)", "B = (1, 2)",
    "k = Circle(O, rr)", "g = Line(A, B)", "s = Segment(A, B)", "r = Ray(A, B)",
    "P = Polar(g, k)", "Q = Polar(s, k)", "R = Polar(r, k)",
], [setn("rr", 3), mv("B", 0, 0), mv("B", 1, 0), mv("B", 1, 2),
    setn("rr", 0), setn("rr", 2)])
c.case("circlepole", [
    "O = (1, 2)", "rr = 2", "k = Circle(O, rr)",
    "P = Polar(xAxis, k)", "Q = Polar(yAxis, k)",
], [mv("O", 0, 0), mv("O", -3, 1), setn("rr", 1)])

c.case("interval", [
    "a = 1", "P = (4, 0)", "m = Midpoint(a < x < 3)",
    "n = Midpoint((x > a) && (x < x(P)))", "q = Center(a < x < x(P))",
], [setn("a", 3), mv("P", 0, 0), setn("a", -1), mv("P", 2, 0)])
c.case("interval", [
    "a = 1", "b = 3",
    "d = Center(x < x + 1)", "ee = Center(x < 2x)",
    "f = Center(x^2 < 4)", "g = Center(x <= 3 || x >= 5)",
    "h = Center(!(x > 3))", "j = Center(a < x < b < 5)",
    "k = Center(x == 3)", "m = Midpoint(x < 2x)",
], [setn("a", -3), setn("b", 6)])

c.write()
