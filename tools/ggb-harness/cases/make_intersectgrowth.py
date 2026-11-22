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
cases.write()
