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
    # The third construction step invalidates t before u is created.
    ["s = ConstructionStep()", "t = Take({1, 2}, If(s == 2, 1, 4))",
     "A = (0, 0)", "u = If(false, t) + 1"],
    [],
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
cases.case("cartesian",
           ["B = (0, 0)", "P = (If(true, 1/0, 0), 1)", "d = Distance(P, B)"], [])
cases.case("cartesian",
           ["B = (0, 0)", "Q = (Mod(1/0, 2), 1)", "d = Distance(Q, B)"], [])
cases.case("cartesian",
           ["B = (0, 0)", "Q = (Mod(7, 3) / 0, 1)", "d = Distance(Q, B)"], [])
cases.case("cartesian",
           ["R = ({1/0}, 1)"], [])
cases.case(
    "cartesian",
    ["m = 1", "xs = {1/m, 2, 3}", "ys = {4, 5}", "R = (xs, ys)",
     "P = (Element({1/m}, 1), 1)", "B = (0, 0)", "d = Distance(P, B)"],
    [setn("m", 0), setn("m", 2)],
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
cases.case(
    "vectorlength",
    ["a = 2", "v = Vector((a, 1))", "f(x) = x Length(v)", "nf = NDerivative(f)",
     "j = Integral(f, 0, 3)", "q = NIntegral(f)"],
    [setn("a", 3)],
)
cases.case(
    "vectorlength",
    ["a = 2", "v = Vector((a, 1))", "f(x) = sin(x) + Length(v)", "nf = NDerivative(f)",
     "q = NIntegral(f)", "j = Integral(f, 0, 3)"],
    [setn("a", 3)],
)
cases.case(
    "vectorlength",
    ["a = 2", "v = Vector((a, 1))", "f(x) = x + Length(v)", "df = Derivative(f)",
     "nf = NDerivative(f)", "j = Integral(f, 0, 3)", "q = NIntegral(f)"],
    [setn("a", 0), setn("a", 3)],
)
cases.case(
    "vectorlength",
    ["a = 2", "v = Vector((a, 1))", "f(x) = x Length(v + v)", "df = Derivative(f)",
     "nf = NDerivative(f)", "j = Integral(f, 0, 3)", "q = NIntegral(f)"],
    [setn("a", 3)],
)
cases.case(
    "cartesian",
    ["P = (1/0, 0)", "O = (0, 0)", "k = Circle(P, 3)", "dp = IsDefined(P)",
     "dk = IsDefined(k)", "nz = Length(RemoveUndefined({k}))", "r = Radius(k)",
     "c = Circle(P, O)", "s = Radius(c)"],
    [],
)
cases.case(
    "redefinition",
    ["m = 1", "n = 1", "t = Take({1, 2}, m)", "a = Length(t + 1)", "O = (0, 0)",
     "k = Circle(O, If(m < 3, 2, -1))", "r = Radius(k)"],
    [setn("m", 3), {"action": "redefine", "line": "m = 4*n"}],
)
cases.case(
    "conditional",
    ["m = 0", "u = If(m == 1, If(m == 0, {1, 2}, m == 2, {4}) + 1, {3})"],
    [setn("m", 1)],
)
cases.case("conditional", ["u = If(false, Take(Sequence(600000), 1) + 1, {0})"], [])
cases.case(
    "transpose",
    ["m = 1", "rows = {Take({1, 2}, m), {3, 4}}", "u = Transpose(rows)"],
    [setn("m", 3)],
)
cases.case(
    "pointgrowth",
    ["m = 1", "l = If(m == 1, {{1, 2}}, {{1, 2}, {3, 4}})", "P = Point(l)"],
    [setn("m", 2), {"action": "delete", "label": "P"}],
)
cases.cases[-1]["track_point_list_growth"] = True
cases.case(
    "selfintersect",
    self_intersection[:-1] + ["Q = Intersect(k, k, 2)", "P = Intersect(k, k, 1)"],
    [mv("C", -5, 2)],
)
cases.case(
    "spline",
    ["m = 1", "A = (0, 0)", "B = (1, 1)", "C = (2, 0)",
     "q = Spline(If(m > 0, {A, B, C}), 3)", "v = 1", "g: y = v", "P = Intersect(g, q)"],
    [setn("m", 0), setn("v", 0.5)],
)
cases.cases[-1]["track_boundary_growth"] = True
cases.case("cartesian", ["s = Length(RemoveUndefined(Sequence((1/i, 2), i, 0, 2)))"], [])
cases.case(
    "cartesian",
    ["P = (1/0, 0)", "Q = If(true, P, (0, 0))", "R = Element({P}, 1)", "a = x(Q)", "b = x(R)"],
    [],
)
cases.case(
    "defined",
    ["g: y = 0", "h: y = 1", "S = Intersect(g, h)", "f(x) = x(S)", "j(x) = f(x)",
     "k(x) = j(x) + 0", "d = IsDefined(f)", "e = IsDefined(j)", "b = IsDefined(k)"],
    [],
)
cases.case("defined", ["u = (1/0, 1/0)", "f(x) = x + Length(u)", "d = IsDefined(f)"], [])
cases.case("root", ["u = nroot(-1/0, 10000000001)"], [])
cases.case("root", ["f(x) = 1/nroot(x, 3)", "P = (-8, -0.5)", "t = Tangent(P, f)", "m = Slope(t)"], [])
cases.case("root", ["f(x) = nroot(x, 3)", "df = Derivative(f)", "q = Integral(f)"], [])
cases.case(
    "transpose",
    ["m = 1", "rows = If(m == 1, {{1, 2}, {3, 4}}, m == 2, {{1}, {3, 4}}, {})",
     "u = Transpose(rows)"],
    [setn("m", 2), setn("m", 3), {"action": "add", "line": "Z = (7, 7)"}, {"action": "undo"}],
)
cases.case("root", ["df = Derivative(nroot(x, 3))", "q = Integral(nroot(x, 3))"], [])
cases.case("conditional", ["m = 0", "u = If(m > 0, Take(Sequence(600000), 1) + 1, {0})"], [])
cases.write()

reference = Cases("keptvalues")
reference.case(
    "pointpath",
    ["n = 1", "U = (-2, 0)", "A = If(n > 0, U)", "B = (0, 2)", "C = (2, 0)",
     "paths = {CircumcircleArc(A, B, C)}", "P = Point(paths)"],
    [setn("n", 0), setn("n", 1)],
)
reference.write("reference")
