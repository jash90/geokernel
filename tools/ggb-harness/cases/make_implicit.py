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

cases.write()
