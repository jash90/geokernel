"""Review regressions for sliders, optimizers, projection and transforms."""

from caselib import Cases, mv, setn

cases = Cases("optimizerreview")


def redefine(line):
    return {"action": "redefine", "line": line}


cases.case("redefine", ["a = 3", "b = 6", "d = 2a"], [
    redefine("a = Slider(0, b)"), setn("b", 2),
])
cases.case("redefine", ["a = 3", "d = 2a"], [
    redefine("a = Slider(0, a)"), setn("a", 2),
])
cases.case("reload", ["b = 6", "a = Slider(0, If(b == 6, 6, 2))", "d = 2a"], [
    setn("a", 5), setn("b", 4),
])
cases.case("finalupdate", [
    "b = 36", "a = Slider(0, sqrt(b))", "m = Minimize((a - 3)^2, a)",
    "t = Slider(0, 6)", "k = 2", "n = Minimize((t - k)^2, t)",
], [setn("b", -1), setn("k", 4)])
cases.case("budget", [
    "a = Slider(0, 6)", "f = (a - 3)^2",
    *(f"m_{{{i}}} = Minimize(f, a)" for i in range(1, 6)),
], [])
cases.case("projection", ["g = Function({0, 2, 0, 2, 0})", "A = (0.5, 2)", "P = ClosestPoint(g, A)"], [])
cases.case("projection", [
    "g = Function({0, 2, 0, 2, 0})", "P = ClosestPoint(g, (0.5, 2))",
], [])
for command in ["Dilate(L, 2)", "ApplyMatrix({{2, 0}, {0, 2}}, L)"]:
    cases.case("transform", ["t = 1", "L = If(t > 0, {(1, 2)}, {1})", f"M = {command}"], [
        setn("t", 0), setn("t", 1),
    ])
cases.case("locus", [
    "t = Slider(0, 4)", "s = Slider(0, If(t == 0, 6, 2))",
    "Q = (t, s)", "L = Locus(Q, t)",
], [setn("s", 3)])
cases.case("locus", [
    "A = (0, 0)", "B = (4, 0)", "g = Segment(A, B)", "P = Point(g)",
    "s = Slider(0, If(x(P) == 0, 6, 2))",
    "Q = (x(P), s)", "L = Locus(Q, P)",
], [mv("P", 0, 0), setn("s", 3)])
cases.case("forms", ["b = 6", "h = 0.1", "a = Slider(0, b, h)", "d = 2a"], [
    setn("h", 0.2), setn("a", 5), setn("b", 2),
])
cases.case("forms", ["b = 6", "a = Slider(0, b, 0.1, 2)", "d = 2a"], [
    setn("a", 5), setn("b", 2),
])
cases.write()
