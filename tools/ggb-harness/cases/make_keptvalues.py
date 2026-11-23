"""Regression cases for retained values across construction updates."""

from caselib import Cases, mv, setn

cases = Cases("keptvalues")

step_take = [
    "s = ConstructionStep()",
    "t = Take({1, 2, 3}, If(s == 4, 2, 4))",
    "n = Length(t + 1)",
]
cases.case("checkpoint", step_take, [])
cases.case("checkpoint", step_take + ["Z = (7, 7)"], [])
cases.case(
    "replay",
    ["m = 3", "t = Take({1, 2}, m)", "n = Length(t + 1)", "D = (0, 0)"],
    [setn("m", 1), setn("m", 4), {"action": "delete", "label": "D"}],
)
cases.case(
    "conditional",
    ["a = 0", "m = 3", "u = If(a > 0, Take({1, 2}, m) + 1, {0})"],
    [setn("m", 1), setn("m", 4), setn("a", 1)],
)
cases.case(
    "conditional",
    ["m = 1", "t = Take({1, 2}, m)", "u = If(false, t) + 1"],
    [setn("m", 4)],
)
cases.case(
    "transpose",
    ["m = 1", "t = Take({1, 2}, m)", "rows = {t, {3, 4}}", "u = Transpose(rows)"],
    [setn("m", 3)],
)
cases.case(
    "pointgrowth",
    ["m = 1", "l = If(m == 1, {{1, 2}}, {{1, 2}, {3, 4}})", "P = Point(l)"],
    [setn("m", 2), {"action": "delete", "label": "A"}],
)
cases.cases[-1]["track_point_list_growth"] = True
cases.case(
    "pointgrowth",
    ["s = ConstructionStep()", "l = If(s == 4, {{1, 2}, {3, 4}}, {{1, 2}})",
     "P = Point(l)", "Z = (7, 7)"],
    [],
)
cases.cases[-1]["track_point_list_growth"] = True

self_intersection = [
    "A = (0, 0)", "B = (3, 0)", "r = Distance(A, B)", "k = Circle(A, r)",
    "C = (-5, 1)", "D = (5, 1)", "g = Line(C, D)", "X = Intersect(g, k, 1)",
    "P = Intersect(k, k, 1)",
]
cases.case("selfintersect", self_intersection, [mv("C", -5, 2), mv("B", 0, 3)])
cases.case("selfintersect", self_intersection + ["Q = Intersect(k, k, 2)"], [mv("C", -5, 2)])
cases.case(
    "spline",
    ["A = (0, 0)", "B = (1, 1)", "C = (2, 0)", "q = Spline({A, B, C}, 3)",
     "v = 1", "g: y = v", "P = Intersect(g, q)"],
    [mv("B", 0, 0), setn("v", 0.5)],
)
cases.cases[-1]["track_boundary_growth"] = True
cases.case("defined", ["u = (1/0, 1/0)", "n = Length(u)", "d = IsDefined(u)"], [])
cases.case(
    "defined",
    ["g: y = 0", "h: y = 1", "S = Intersect(g, h)", "f(x) = x(S)", "d = IsDefined(f)"],
    [],
)
cases.case(
    "cartesian",
    ["A = (-1/0, 0)", "B = (0, 0)", "d = Distance(A, B)", "k = Circle(A, 1)",
     "r = Radius(k)", "P = (If(true, 1/0, 0), 1)"],
    [],
)
cases.case("root", ["u = nroot(-1/0, 10000000000)"], [])
cases.case(
    "areasums",
    ["f(x) = If(x == 0, 2, 1)",
     "a = LowerSum(f, 0, 1/0, 4)", "b = UpperSum(f, 0, 1/0, 4)",
     "c = LowerSum(f, 1/0, 0, 4)", "d = UpperSum(f, 1/0, 0, 4)"],
    [],
)
cases.case(
    "areasums",
    ["f(x) = x", "g = Derivative(f)", "a = LowerSum(g, 0, 1/0, 4)",
     "b = UpperSum(g, 1/0, 0, 4)"],
    [],
)
cases.write()
