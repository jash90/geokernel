"""Slider edits reveal additional crossings and then make them undefined."""

from caselib import Cases, setn

cases = Cases("intersectgrowth")
cases.case("slider", ["A = (0, 0)", "B = (2, 0)", "C = (2, 2)", "D = (0, 2)",
                      "p = Polygon(A, B, C, D)", "n = 3", "g: y = n", "R = Intersect(g, p)"],
           [setn("n", value) for value in (1, 3, 0.5, 3)])
cases.cases[-1]["track_boundary_growth"] = True
cases.case("boundarypair", ["A = (0, 0)", "B = (2, 0)", "C = (2, 2)", "D = (0, 2)",
                            "p = Polygon(A, B, C, D)", "n = 3", "E = (n, 0.5)",
                            "F = (n + 2, 0.5)", "G = (n + 2, 1.5)", "H = (n, 1.5)",
                            "q = Polygon(E, F, G, H)", "R = Intersect(p, q)"],
           [setn("n", value) for value in (1, 3, 1)])
cases.cases[-1]["track_boundary_growth"] = True
cases.case("interval", ["f(x) = x^3 - x", "h(x) = 0", "a = -2", "b = 0.5",
                         "R = Intersect(f, h, a, b)"],
           [setn("b", value) for value in (2, 0.5, 2)])
cases.cases[-1]["track_root_growth"] = True
cases.case("redefinition", [
    "A = (0, 0)", "B = (2, 0)", "C = (2, 2)", "D = (0, 2)",
    "p = Polygon(A, B, C, D)", "n = 3", "g: y = n", "R = Intersect(g, p)", "m = 3",
], [{"action": "redefine", "line": "n = m"}, setn("m", 1), setn("m", 3), setn("m", 1)])
cases.cases[-1]["track_boundary_growth"] = True

cases.case("interval", ["f(x) = sin(x)", "h(x) = 0", "a = 1", "b = 4",
                         "R = Intersect(f, h, a, b)"],
           [setn("b", value) for value in (10, 2, 13, 7)])
cases.cases[-1]["track_root_growth"] = True
cases.case("interval", ["A = (5, 5)", "C = (1, 1)", "f(x) = x^3 - 4x", "k = 3", "h(x) = k",
                         "a = -3", "b = 3", "R = Intersect(f, h, a, b)"],
           [setn("k", value) for value in (0, 5, 1, 0)])
cases.cases[-1]["track_root_growth"] = True
cases.case("interval", ["c = 0", "f(x) = sqrt(x - c) - 1", "h(x) = 0.5 x - 1", "a = 0", "b = 6",
                         "R = Intersect(f, h, a, b)"],
           [setn("c", value) for value in (8, -1, 0)])
cases.cases[-1]["track_root_growth"] = True
cases.case("interval", ["f(x) = x^2 - 1", "h(x) = 0", "a = 3", "b = -3",
                         "R = Intersect(f, h, a, b)"],
           [setn("a", value) for value in (-3, 1, 0.5, -0.5)])
cases.cases[-1]["track_root_growth"] = True
cases.case("interval", ["f(x) = cos(x)", "h(x) = 0", "a = 0", "b = 1",
                         "R = Intersect(f, h, a, b)"],
           [setn("b", value) for value in (5, 0, 8, 1, 11)])
cases.cases[-1]["track_root_growth"] = True
cases.case("interval", ["b = 1.5", "f(x) = (x - 1) (x - 2)", "g(x) = (x - 3) (x - 4) (x - 5)",
                         "h(x) = 0", "R = Intersect(f, h, b, 3)", "S = Intersect(g, h, 0, b)"],
           [setn("b", 0.5), setn("b", 5.5)])
cases.cases[-1]["track_root_growth"] = True
# Label reuse: AlgoRoots.initPoints removes the undefined outputs no object
# reads, which frees their labels; AlgoGeoPointsFunction.updateLabels gives
# a slot that has a root again the next default point label.
cases.case("reuse", ["f(x) = sin(x)", "h(x) = 0", "b = 10", "R = Intersect(f, h, 1, b)",
                      "d = x(R_{2})"],
           [setn("b", value) for value in (4, 10, 7, 13)])
cases.cases[-1]["track_root_growth"] = True
cases.case("reuse", ["f(x) = sin(x)", "h(x) = 0", "b = 10", "R = Intersect(f, h, 1, b)",
                      "d = x(R_{3})"],
           [setn("b", value) for value in (2, 10, 13)])
cases.cases[-1]["track_root_growth"] = True
cases.case("reuse", ["b = 3", "c = 2.6", "f(x) = (x - 1) (x - 2)",
                      "g(x) = (x - 3) (x - 4) (x - 5)", "k(x) = (x - 6) (x - 7)", "h(x) = 0",
                      "R = Intersect(f, h, 0, b)", "S = Intersect(g, h, 2.5, c)",
                      "T = Intersect(k, h, 5.5, c)"],
           [setn("b", 1.5), setn("c", 8), setn("b", 3), setn("c", 2.6), setn("b", 1.5),
            setn("b", 3), setn("c", 8)])
cases.cases[-1]["track_root_growth"] = True
cases.case("reuse", ["b = 1.5", "f(x) = (x - 1) (x - 2)", "g(x) = (x - 3) (x - 4) (x - 5)",
                      "h(x) = 0", "R = Intersect(f, h, b, 3)", "S = Intersect(g, h, 0, b)"],
           [setn("b", value) for value in (0.5, 5.5, 0.5, 1.5, 5.5)])
cases.cases[-1]["track_root_growth"] = True
cases.write()
