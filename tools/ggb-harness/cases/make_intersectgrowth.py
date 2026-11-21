"""A slider reveals boundary crossings and then makes them undefined."""

from caselib import Cases, setn

cases = Cases("intersectgrowth")
cases.case("slider", ["A = (0, 0)", "B = (2, 0)", "C = (2, 2)", "D = (0, 2)",
                      "p = Polygon(A, B, C, D)", "n = 3", "g: y = n", "R = Intersect(g, p)"],
           [setn("n", value) for value in (1, 3, 0.5, 3)])
cases.cases[-1]["track_boundary_growth"] = True
cases.write()
