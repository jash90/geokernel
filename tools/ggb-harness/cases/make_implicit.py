"""Exercise polynomial equations and the commands that use them."""

from caselib import Cases, mv, setn

cases = Cases("implicit")
cases.case("equations", ["c: x^3 + y^3 = 1"], [])
cases.case(
    "equations",
    ["a = 2.98", "b = 0.03", "c: (x^2 + y^2)^2 - 2*b^2*(x^2 - y^2) = a^4 - b^4"],
    [setn("a", 3.8), setn("b", 3.9)],
)
cases.case(
    "equations",
    ["A = (2, 1)", "c: x^3/x(A) + y^3 = 1"],
    [mv("A", 0, 1), mv("A", -2, 1)],
)
cases.case(
    "equations",
    ["a = 3", "c: x^a + y = 1"],
    [setn("a", 2), setn("a", 1), setn("a", 4)],
)
cases.case(
    "equations",
    ["a = 0", "c: a*x^3 + x^2 + y^2 = 1"],
    [setn("a", 1), setn("a", 0)],
)
points = ["A = (1, 1)", "B = (2, 2)", "C = (3, 3)", "D = (4, 0.25)", "E = (5, 0.2)"]
cases.case("constructors", points + ["pts = {A, B, C, D, E}", "c = ImplicitCurve(pts)"],
           [mv("E", 4, 0.25), mv("E", 5, 0.2)])
cases.case("constructors", points + ["c = ImplicitCurve(A, B, C, D, E)"],
           [mv("D", 4, 4), mv("D", 4, 0.25)])
cases.case("constructors", ["a = 1", "f(x,y) = x^3/a + y^2 - 1", "c = ImplicitCurve(f)"],
           [setn("a", 0), setn("a", 2)])
cases.case("constructors", ["f(x,y) = x*y", "c = ImplicitCurve(f)"], [])
cases.case("constructors", points[:3] + ["pts = {A, B, C}", "c = ImplicitCurve(pts)"], [])
cases.case("constructors", points[:2] + ["pts = {A, B}", "c = ImplicitCurve(pts)"],
           [mv("B", 1, 1), mv("B", 2, 3)])
cases.case("constructors", ["c = ImplicitCurve(x*y)"], [])

cases.case(
    "sides",
    ["a = 2", "c: x^3/a + y^2 = y^3 - a", "left = LeftSide(c)", "right = RightSide(c)"],
    [setn("a", 0), setn("a", -2)],
)
cases.case(
    "sides",
    ["f(x,y) = x^2*y + 1", "c = ImplicitCurve(f)", "left = LeftSide(c)", "right = RightSide(c)"],
    [],
)
cases.case("asymptotes", ["c: x^3 - x*y^2 = 1", "lines = Asymptote(c)"], [])
cases.case(
    "asymptotes",
    ["a = 2", "b = 1", "c: (x-a)*(y-b)^2 = 1/b", "lines = Asymptote(c)"],
    [setn("b", 0), setn("b", -2), setn("a", -3)],
)
cases.case("asymptotes", ["c: x^4 + y^4 = 1", "lines = Asymptote(c)"], [])
cases.case("asymptotes", ["c: (x-2*y-1)*(x-2*y+3)*y = 2", "lines = Asymptote(c)"], [])
cases.case("asymptotes", ["c: x^3 - 3*x^2*y + 2*x*y^2 = y^2 + x^2", "lines = Asymptote(c)"], [])

cases.case(
    "line", ["c: x^4 + y^4 = 2", "A = (-2, -2)", "B = (2, 2)", "g = Line(A, B)",
             "P = Intersect(c, g)", "Q = Intersect(g, c)", "n = 1",
             "R = Intersect(c, g, n)", "S = Intersect(g, c, 2)"],
    [setn("n", 2), setn("n", 3), mv("B", -2, -2), mv("B", 2, 2), setn("n", -1)],
)
cases.case("line", ["c: y^3 - y = 0", "g: x = 0", "P = Intersect(c, g)"], [])
cases.case("line", ["c: x^3 - x = 0", "g: x = 1", "P = Intersect(c, g)"], [])
cases.case(
    "line", ["a = 2", "c: x^3/a + y^3 = 1", "g: y = 0",
             "P = Intersect(c, g)", "Q = Intersect(c, g, 1)"],
    [setn("a", 0), setn("a", -2)],
)
cases.case("line", ["a = -1", "c: x^4 - 5*x^2 + y = 0", "g: y = a", "P = Intersect(c, g)"],
           [setn("a", 1), setn("a", 9), setn("a", 0)])
cases.cases[-1]["track_boundary_growth"] = True
cases.case(
    "polyline",
    ["c: (x + 1)^4 + (y - 3)^4 = 1", "A = (-1, -2)", "B = (-1, 3)", "C = (5, 3)",
     "path = PolyLine(A, B, C)", "P = Intersect(c, path)"],
    [mv("A", -1, 4), mv("A", -1, -2), mv("C", -2, 3)],
)
cases.case(
    "polyline",
    ["c: x^4 + y^4 = 1", "A = (-2, 0)", "B = (-1, 0)", "C = (2, 0)",
     "path = PolyLine(A, B, C)", "P = Intersect(path, c)"],
    [],
)
cases.case(
    "graph",
    ["c: x^4 + y^4 = 2", "a = 1", "f(x) = a*x", "n = 1", "P = Intersect(c, f)",
     "Q = Intersect(f, c)", "R = Intersect(c, f, n)", "S = Intersect(f, c, 2)"],
    [setn("n", 2), setn("a", 0), setn("n", 4), setn("a", -1), setn("n", 1)],
)
cases.case("graph", ["c: x^3 + y = 0", "f(x) = sin(x)", "P = Intersect(c, f)"], [])
cases.case(
    "conic", ["c: x^4 + y^4 = 2", "k: x^2 + y^2 = 2", "n = 1", "P = Intersect(c, k)",
              "R = Intersect(c, k, n)"],
    [setn("n", 2), setn("n", 4), setn("n", 5), setn("n", 1)],
)
cases.case(
    "conic", ["a = 1", "c: x^4 + y^4 = 2", "k: x^2 + 2*y^2 = 3*a",
              "P = Intersect(c, k)"],
    [setn("a", 0.1), setn("a", -1), setn("a", 1)],
)
cases.case("conic", ["c: x^4 + y^4 = 2", "k: x + y^2 = 0", "P = Intersect(c, k)"], [])

cases.case("conic", ["c: x^4 + y^4 = 2", "k: x^2 + y^2 = 2", "Q = Intersect(k, c)"], [])

cases.write()
