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

line = ["A = (0, 0)", "B = (4, 0)", "g = Line(A, B)", "P = Point(g)"]
for fixed, edits in (
    (["F = (0.12345, 3)"], [mv("F", 0.12345, 3.0001), mv("F", 0.12345, 3.004),
                           mv("F", 0.12345, 3.027)]),
    (["F = (1/3, 1/7)"], [mv("F", -0.33, -0.14)]),
    (["F = (0.1234567890123456, 0.9876543210987654)"], []),
    (["O = (0, 0)", "K = (2, 0)", "k = Circle(O, K)", "j = Circle(K, O)",
      "F = Intersect(k, j, 1)"], [mv("K", 4, 0)]),
):
    traced = fixed + line + ["M = Midpoint(P, F)"]
    cases.case("midpoint", traced + ["c = LocusEquation(M, P)"], edits)
    cases.case("midpointlocus", traced + ["l = Locus(M, P)", "c = LocusEquation(l)"], edits)
    cases.case("boolean", fixed + ["D = (8, 5)", "P = (2, 1)", "d = Distance(P, F)",
                                  "r = Distance(D, F)", "b = d == r", "c = LocusEquation(b, P)"],
               edits)

cases.case("boolean", ["O = (0, 0)", "A = (3, 0)", "k = Circle(O, A)", "P = Point(k)",
                      "d = Distance(P, O)", "r = Distance(P, A)", "b = d == r",
                      "c = LocusEquation(b, P)"], [mv("A", 4, 0), mv("A", 0, 4)])
cases.case("midpoint", line + ["C = (0, 1)", "D = (3, 2)", "h = Line(C, D)", "F = Point(h)",
                               "M = Midpoint(P, F)", "c = LocusEquation(M, P)"],
           [mv("F", 0.12345, 1.1), mv("D", 0, 2), mv("D", 3, 2)])
cases.case("boolean", ["O = (0, 0)", "A = (3, 0)", "P = (2, 1)", "d = Distance(P, O)",
                      "r = Distance(A, O)", "b = d == r", "q = d == r",
                      "c = LocusEquation(b, P)"], [mv("P", 3, 0), mv("A", 2, 0), mv("A", 3, 0)])
cases.case("nondegenerate", line[:3] + ["P = Point(g, 0.75)", "C = (1, 3)", "h = Line(A, P)",
                                      "k = Circle(C, P)", "M = Intersect(h, k, 1)",
                                      "c = LocusEquation(M, P)"],
           [mv("B", 0, 0), mv("B", 0, 4), mv("B", 4, 0)])
cases.case("nondegenerate", ["O = (0, 0)", "A = (0, 3)", "k = Circle(O, A)",
                             "P = Point(k, 0.5)", "C = (2, 1)", "h = Line(A, P)",
                             "n = PerpendicularLine(C, h)", "M = Intersect(h, n)",
                             "c = LocusEquation(M, P)"],
           [mv("C", 3, 2), mv("C", 2, 1)])

cases.case("rejections", line + ["F = (1, 3)", "M = Midpoint(A, F)", "N = Midpoint(P, F)",
                                "d = Distance(N, F)", "r = Distance(A, B)", "b = d == r"],
           [{"action": "reject", "line": source} for source in
            ("c = LocusEquation(M, P)", "c = LocusEquation(M, A)",
             "c = LocusEquation(P, P)", "c = LocusEquation(b, N)")])
cases.case("rejections", ["f(x) = x", "l = SlopeField(f, 5, 0.1, 0, 0, 1, 1)"],
           [{"action": "reject", "line": "c = LocusEquation(l)"}])

cases.write()
