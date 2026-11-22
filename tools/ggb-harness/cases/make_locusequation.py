"""Record implicit equations from point constructions and conditions."""

from caselib import Cases, mv

cases = Cases("locusequation")
circle = ["O = (1, 2)", "A = (4, 2)", "k = Circle(O, A)", "P = Point(k)",
          "M = Midpoint(O, P)"]
cases.case("circle", circle + ["c = LocusEquation(M, P)"],
           [mv("A", 1, 2), mv("A", 5, 2), mv("O", 0, 0)])
cases.case("circle", circle + ["l = Locus(M, P)", "c = LocusEquation(l)"],
           [mv("A", 5, 2), mv("O", 0, 0)])

directrix = ["A = (-3, -1)", "B = (3, -1)", "F = (0, 1)", "g = Line(A, B)",
             "P = Point(g)", "n = PerpendicularLine(P, g)", "b = PerpendicularBisector(P, F)",
             "M = Intersect(n, b)"]
cases.case("parabola", directrix + ["c = LocusEquation(M, P)"],
           [mv("F", 0, -1), mv("F", 1, 2), mv("B", 3, 1)])
cases.case("midpoint", ["A = (-3, -1)", "B = (3, -1)", "F = (2, 3)", "g = Line(A, B)",
                        "P = Point(g)", "M = Midpoint(P, F)", "c = LocusEquation(M, P)"],
           [mv("B", -3, -1), mv("B", 3, 1)])

cases.case("conchoid", ["O = (0, 0)", "A = (1, -2)", "B = (1, 2)", "g = Line(A, B)",
                        "P = Point(g)", "h = Line(O, P)", "a = 2", "k = Circle(P, a)",
                        "M = Intersect(h, k, 1)", "c = LocusEquation(M, P)"],
           [mv("B", 1, -2), mv("B", 1, 3), mv("O", -1, 0)])
cases.case("conchoid", ["O = (0, 0)", "A = (1, -2)", "B = (1, 2)", "R = (2, 0)",
                        "g = Line(A, B)", "P = Point(g)", "h = Line(O, P)", "a = Distance(O, R)",
                        "k = Circle(P, a)", "M = Intersect(h, k, 1)", "c = LocusEquation(M, P)"],
           [mv("R", 3, 0), mv("B", 1, 3), mv("O", -1, 0)])
cases.case("conchoid", ["O = (0, 0)", "A = (1, -2)", "B = (1, 2)", "R = (2, 0)",
                        "g = Line(A, B)", "P = Point(g)", "h = Line(O, P)", "s = Segment(O, R)",
                        "k = Circle(P, s)", "M = Intersect(h, k, 1)", "c = LocusEquation(M, P)"],
           [mv("R", 3, 0), mv("B", 1, 3), mv("O", -1, 0)])
cases.case("boolean", ["O = (0, 0)", "A = (3, 0)", "P = (2, 1)", "d = Distance(P, O)",
                      "r = Distance(A, O)", "b = d == r", "c = LocusEquation(b, P)"],
           [mv("P", 3, 0), mv("A", 2, 0), mv("A", 0, 0)])
cases.case("boolean", ["O = (0, 0)", "A = (3, 0)", "P = (2, 1)", "d = Distance(P, O)",
                      "r = Distance(A, O)", "b = d == r", "c = LocusEquation(b, P)"],
           [mv("P", 3, 0), mv("P", 0, 3)])

cases.write()
