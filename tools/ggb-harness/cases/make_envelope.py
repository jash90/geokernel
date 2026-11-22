"""Record implicit envelopes from moving lines and circles."""

from caselib import Cases, mv, setn

cases = Cases("envelope")
cases.case("parabola", ["A = (-3, -1)", "B = (3, -1)", "F = (0, 1)", "g = Line(A, B)",
                        "P = Point(g)", "b = PerpendicularBisector(P, F)", "c = Envelope(b, P)"],
           [mv("F", 0, -1), mv("F", 1, 2), mv("B", 3, 1)])
cases.case("circle", ["O = (0, 0)", "A = (2, 0)", "k = Circle(O, A)", "P = Point(k)",
                      "g = Line(O, P)", "n = PerpendicularLine(P, g)", "c = Envelope(n, P)"],
           [mv("A", 0, 0), mv("A", 3, 0), mv("O", 1, 1)])
cases.case("circle", ["A = (-3, 0)", "B = (3, 0)", "g = Line(A, B)", "P = Point(g)",
                      "r = 2", "k = Circle(P, r)", "c = Envelope(k, P)"],
           [mv("B", -3, 0), mv("B", 3, 0)])
cases.case("circle", ["A = (-3, 0)", "B = (3, 0)", "R = (-3, 2)", "s = Segment(A, R)",
                      "g = Line(A, B)", "P = Point(g)", "k = Circle(P, s)", "c = Envelope(k, P)"],
           [mv("R", -3, 1), mv("R", -3, 0), mv("R", -3, 2), mv("B", 3, 2)])
cases.case("circle", ["A = (-3, 0)", "B = (3, 0)", "F = (1, 2)", "g = Line(A, B)", "P = Point(g)",
                      "k = Circle(P, F)", "c = Envelope(k, P)"],
           [mv("F", 1, 0), mv("F", 1, -2), mv("B", -3, 0), mv("B", 3, 0)])
cases.case("circle", ["A = (-3, 0)", "B = (3, 0)", "g = Line(A, B)", "P = Point(g)",
                      "r = Slider(0, 5, 1)", "k = Circle(P, r)", "c = Envelope(k, P)"],
           [setn("r", 2), setn("r", 0), setn("r", 3)])
cases.case("circle", ["A = (-3, 0)", "B = (3, 0)", "g = Line(A, B)", "P = Point(g)",
                      "F = (0, 0)", "k = Circle(P, F)", "c = Envelope(k, P)"],
           [mv("F", 0, 2), mv("F", 0, 0), mv("F", 0, -2)])
cases.write()
