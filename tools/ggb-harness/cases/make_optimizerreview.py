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
cases.case("redefine", ["b = 6", "a = Slider(0, b)", "d = 2a"], [
    setn("a", 5), setn("b", 4), redefine("a = Slider(0, b)"),
])
cases.case("redefine", ["a = Slider(-4, -2)", "d = 2a"], [
    redefine("a = Slider(0, 6)"),
])
cases.case("redefine", ["a = 3", "b = 2a"], [
    redefine("a = Slider(0, b + 1)"), setn("a", 5),
])
cases.case("redefine", ["a = Slider(-2, -1)", "d = 2a"], [
    redefine("a = Slider(-0.5, 6)"),
])
for command in ["Dilate(L, 2)", "ApplyMatrix({{2, 0}, {0, 2}}, L)"]:
    cases.case("transform", [
        "t = 1", "A = (1, 2)", "v = Vector((3, 4))",
        "L = If(t > 0, {A}, {v})", f"M = {command}",
    ], [setn("t", 0), setn("t", 1)])
cases.case("roundtrip", ["b = 6", "a = Slider(0, If(b == 6, 6, 2))", "d = 2a"], [
    {"action": "reload_ggb"}, redefine("a = Slider(0, 10)"),
    {"action": "reload_ggb"}, setn("a", 8),
])
cases.case("mixedmover", [
    "A = (0, 0)", "B = (4, 0)", "g = Segment(A, B)", "P = Point(g)",
    "a = Slider(0, 6)", "m = Minimize((a - (x(P) + 1))^2, a)",
    "N = Minimize((m - 3)^2, P)",
], [])
cases.case("finalupdate", [
    "b = 36", "a = Slider(0, sqrt(b))", "m = Minimize((a - 3)^2, a)",
    "t = Slider(0, 6)", "k = 2", "n = Minimize((t - k)^2, t)",
    *(f"d_{{{i}}} = If(IsDefined(m), 0, Sum(Sequence(sqrt(j), j, 1, 2000)))"
      for i in range(1, 21)),
], [setn("b", -1), setn("k", 4)])
cases.case("locus", [
    "b = 4", "t = Slider(0, b)", "Q = (t, 0)", "L = Locus(Q, t)",
], [setn("b", 6), setn("t", 1)])
cases.case("animationoptions", ["h = 0.2", "a = Slider(0, 6, h, 2)"], [
    redefine("a = Slider(0, 8)"),
    {"action": "slider_animation", "label": "a"},
    redefine("a = Slider(0, 9, 0.4)"),
    {"action": "slider_animation", "label": "a"},
])
cases.case("redefine", ["a = Slider(-2, -1)", "b = 2a"], [
    redefine("a = Slider(0, If(IsDefined(b), 6, 0.5))"),
])
cases.case("boundsorder", ["u = -4", "lo = u + 0", "hi = u + 3", "a = Slider(lo, hi)"], [
    setn("u", -0.5),
])
for command in ["Dilate(L, 2)", "ApplyMatrix({{2, 0}, {0, 2}}, L)"]:
    cases.case("transform", [
        "t = 1", "A = (1, 2)", "v = Vector((3, 4))",
        "L = If(t > 0, {v}, {A})", f"M = {command}",
    ], [setn("t", 0), setn("t", 1)])
    cases.case("transform", [
        "t = 1", "A = (1, 2)", "L = If(t > 0, {{A}}, {3})", f"M = {command}",
    ], [setn("t", 0), setn("t", 1)])
# The multiplier starts a new point search without shifting the minimum.
cases.case("mixedmover", [
    "A = (0, 0)", "B = (4, 0)", "g = Segment(A, B)", "P = Point(g)",
    "a = Slider(0, 6)", "m = Minimize((a - (x(P) + 1))^2, a)",
    "b = 6", "s = Slider(0, b)", "q = Minimize((s - 3)^2, s)",
    "k = 1", "N = Minimize(k (m - q)^2, P)",
], [setn("b", 2), setn("k", 2)])
cases.case("redefine", ["a = 3", "d = 2a", "b = 6"], [
    redefine("a = Slider(0, If(d > 0, b, 6))"), setn("b", 2),
    {"action": "reload_ggb"}, setn("b", 1),
])
cases.case("locus", [
    "b = 4", "lo = b - 4", "hi = b + 0", "s = Slider(lo, 10)",
    "t = Slider(0, hi)", "Q = (t, s)", "L = Locus(Q, t)",
], [setn("b", 6), setn("t", 1)])
cases.case("animationoptions", ["h = 0.2", "a = Slider(0, 6, h, 2)"], [
    redefine("a = Slider(0, 8)"), setn("h", 0.4),
    {"action": "slider_animation", "label": "a"},
])
cases.write()
