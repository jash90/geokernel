#!/usr/bin/env python3
"""Replay the 2D CommandsTest cases and exercise matrix and dilation updates."""

from caselib import Cases, mv, setn

cases = Cases("matrixdilate")
case = cases.case

# CommandsTest.cmdInvert, cmdTranspose, cmdReducedRowEchelonForm, cmdDimension.
case("upstream", ["a = Invert({{1, 1}, {0, 2}})", "b = Transpose({{1, 2}, {3, 4}})",
                  "c = ReducedRowEchelonForm({{1, 2}, {3, 4}})", "d = Dimension((3, 7))"], [])
for src in ("sin(x)", "If(x > 1, x^3 + 1)", "If(x > 2, x) + 1",
            "If(x > 2, sin(x) - x)", "3 + 0/x", "3 + x/0", "3 + 2/x"):
    case("upstream", [f"g = Invert({src})"], [])

# CommandsTest.cmdApplyMatrix uses an inline regular quadrilateral. Name it
# separately so the construction records the source and its dependencies.
case("upstream", ["A = (1, 1)", "B = (2, 0.5)", "p = Polygon(A, B, 4)",
                  "q = ApplyMatrix({{1, 2}, {3, 4}}, p)"], [mv("B", 0, 2)])
# CommandsTest.cmdDilate, with a named source line for the equation argument.
case("upstream", ["A = (4, 5)", "B = (1, 1)", "n = 2", "P = Dilate(A, n, B)", "Q = Dilate(A, n)",
                  "g: y = -3x - 6", "r = Dilate(g, n)"], [mv("B", 0, 0)])

case("elimination", ["n = 2", "m = {{1, 2}, Sequence(n)}",
                     "a = Transpose(m)", "b = ReducedRowEchelonForm(m)",
                     "c = Invert(m)", "d = Dimension(m)"],
     [setn("n", 0), setn("n", 3), setn("n", 2)])
case("elimination", ["n = 4", "m = {{1, 2}, {2, n}}",
                     "a = ReducedRowEchelonForm(m)", "b = Invert(m)"],
     [setn("n", 5), setn("n", 4), setn("n", 4.000000001), setn("n", -2)])
case("elimination", ["n = 2", "m = {{sqrt(n), 0}, {1, n}}",
                     "a = Invert(m)", "b = ReducedRowEchelonForm(m)", "c = Transpose(m)"],
     [setn("n", -1), setn("n", 0), setn("n", 3)])
case("elimination", ["m = {{0, 2, 4}, {0, 1, 2}, {0, 3, 5}}",
                     "a = ReducedRowEchelonForm(m)", "b = Invert(m)"], [])
case("elimination", ["a = Invert({})", "b = Transpose({})", "c = ReducedRowEchelonForm({})",
                     "d = Dimension({})", "e_{m} = Dimension({{1}, {2, 3}})",
                     "f = Invert({{1, 2, 3}, {4, 5, 6}})",
                     "g = ReducedRowEchelonForm({{1, 2, 3}, {4, 5, 6}})"], [])
case("elimination", ["A = (2, 3)", "B = (-1, 8)", "m = {{A, true, {1}}, {B, false, {2}}}",
                     "a = Transpose(m)", "b = Dimension(m)", "c = ReducedRowEchelonForm(m)"],
     [mv("A", 0, 0)])

case("inverse", ["n = 2", "f(x) = n*x + 1", "g = Invert(f)", "h(x) = g(x) + 3"],
     [setn("n", -2), setn("n", 0), setn("n", 0.5)])
case("inverse", ["n = 2", "f(x) = x^n", "g = Invert(f)"],
     [setn("n", 3), setn("n", 0), setn("n", -1), setn("n", 2.5)])
case("inverse", ["n = 2", "f(x) = n/x", "h(x) = 3 + f(x)", "g = Invert(h)"],
     [setn("n", 0), setn("n", 4)])

case("inverse", ["f(x) = x + x", "g = Invert(f)"], [])

case("elimination", ["n = 1", "m = {{1, 2}, Sequence(n)}", "a = Dimension(m)"],
     [setn("n", 2), setn("n", 0), setn("n", 1)])
case("elimination", ["n = 2", "m = {{1/n, 0}, {1, n}}",
                     "a = Invert(m)", "b = ReducedRowEchelonForm(m)"],
     [setn("n", 0), setn("n", -2)])
case("elimination", ["n = 1", "m = If(n == 1, {{2}, {4}}, If(n == 2, {{2}, {4, 5}}, {}))",
                     "a = Transpose(m)"], [setn("n", 2), setn("n", 3), setn("n", 1)])

case("applyconic", ["A = (1, 2)", "r = 2", "k = Circle(A, r)", "n = 2", "m = {{n, 0}, {0, 1}}",
                    "q = ApplyMatrix(m, k)", "P = Center(q)"],
     [setn("n", 0), setn("n", -2), mv("A", -1, 0), setn("n", 1)])
case("applyconic", ["F = (0, 1)", "g: y = -1", "p = Parabola(F, g)", "n = 1",
                    "m = {{1, 1, 1}, {0, 1, 0}, {0, n, 2}}", "q = ApplyMatrix(m, p)"],
     [setn("n", 0), setn("n", -1), mv("F", 0, -1), mv("F", 0, 2)])
case("applypoly", ["A = (1, 1)", "B = (2, 0.5)", "C = (0, 3)", "p = Polygon(A, B, C)",
                   "l = PolyLine(A, B, C)", "n = 2", "m = {{n, 1}, {0, 1}}",
                   "q = ApplyMatrix(m, p)", "t = ApplyMatrix(m, l)"],
     [setn("n", 0), setn("n", -1), mv("C", -3, -1)])
case("applypoly", ["A = (1, 1)", "B = (2, 0.5)", "C = (0, 3)", "p = Polygon(A, B, C)",
                   "l = PolyLine(A, B, C)", "n = 1", "m = {{1, 0, 1}, {0, 1, 0}, {1, 0, n}}",
                   "q = ApplyMatrix(m, p)", "t = ApplyMatrix(m, l)"],
     [setn("n", 0), setn("n", -1), mv("A", 3, -1), setn("n", 1)])
case("applypoly", ["A = (1, 1)", "B = (2, 0.5)", "r = 4", "p = Polygon(A, B, r)",
                   "n = 1", "m = {{1, 0, 1}, {0, 1, 0}, {1, 0, n}}",
                   "q = ApplyMatrix(m, p)"],
     [setn("n", 0), setn("n", -1), mv("A", 3, -1), setn("n", 1)])
case("applylist", ["A = (1, 1)", "u = (1, 2)", "n = 2", "m = {Sequence(n), {3, 4}}",
                   "l = {A, {u, (2, 3)}, {}}", "t = ApplyMatrix(m, l)"],
     [setn("n", 3), setn("n", 0), mv("A", 0, 0), setn("n", 2)])
case("applylist", ["A = (1, 1)", "B = (2, 0)", "n = 1",
                   "m = {{1, 2, 3}, {0, 1, 3}, {1, 0, n}}",
                   "l = {A, B}", "t = ApplyMatrix(m, l)"],
     [setn("n", -1), mv("A", 2, -2), setn("n", 1)])

case("applylist", ["A = (1, 1)", "n = 1", "l = {A}",
                   "m = {{1, 0, 0}, {0, 1, 0}, {1, 0, n}}", "t = ApplyMatrix(m, l)",
                   "u = {{1, 0, 0}, {0, 1, 0}, {-1, 0, 1}}", "v = ApplyMatrix(u, t)",
                   "p = Dilate(t, 2, A)", "q = ApplyMatrix(u, p)"],
     [setn("n", -1), setn("n", 0), mv("A", 2, 3), setn("n", 2)])

case("dilatefunction", ["A = (2, 1)", "n = 2", "f(x) = x^2 + x(A)", "g = Dilate(f, n)",
                        "h = Dilate(f, n, A)", "j(x) = h(x) - g(x)"],
     [setn("n", -2), setn("n", 0), mv("A", -1, 3), setn("n", 0.5), setn("n", 1)])
case("dilatefunction", ["A = (-1, 2)", "n = 2", "r = 1/n", "f(x) = If(x < r, sin(x))",
                        "g = Dilate(f, r, A)"],
     [setn("n", 0), setn("n", -2), mv("A", 0, -2), setn("n", 1)])
case("dilatefunction", ["A = (0, 0)", "B = (2, 1)", "C = (0, 1)", "D = (2, 0)",
                        "g = Line(A, B)", "h = Line(C, D)", "P = Intersect(g, h)",
                        "n = 2", "u = sqrt(n)", "f(x) = x + u", "q = Dilate(f, 2, P)"],
     [mv("D", 2, 2), setn("n", -1), mv("D", 2, 0), setn("n", 1)])
case("dilatelist", ["A = (2, 3)", "B = (1, -2)", "u = (3, 1)", "n = 2",
                    "l = {A, {u, B, {}}, A}", "p = Dilate(l, n)", "q = Dilate(l, n, B)"],
     [setn("n", 0), mv("B", -2, 2), setn("n", -3), mv("u", -1, 3), setn("n", 1)])
case("dilatelist", ["A = (2, 3)", "n = 2", "r = 1/n",
                    "l = Sequence(A * k, k, 1, n)", "m = Dilate(l, r)"],
     [setn("n", 3), setn("n", 0), setn("n", 1)])
case("dilatelist", ["A = (0, 1)", "B = (1, 0)", "n = 1", "l = {A, Element({B}, n)}",
                    "p = Dilate(l, 0, A)", "e_{m} = {}", "q = Dilate(e_{m}, 2)"],
     [setn("n", 2), mv("A", 0, 0), setn("n", 1)])

case("elimination", ["a = Invert({{1, 0}, {0, 0/0}})"], [])
case("elimination", ["a = ReducedRowEchelonForm({{-1, 0, 2}, {0, 1, 1}})"], [])
case("inverse", ["f(x) = 3", "h(x) = f(x) + x", "g = Invert(h)"], [])
case("inverse", ["f(x) = x^(-0.0099)", "g = Invert(f)"], [])
case("dilatefunction", ["A = (2, 7)", "n = 2", "f(x) = x > 1",
                        "g = Dilate(f, n)", "h = Dilate(f, n, A)", "j(x) = !h(x)"],
     [setn("n", -2), setn("n", 0), setn("n", 1)])
case("dilatefunction", ["v = (2, 7)", "A = (1, 2)", "f(x) = x + 1", "l = {A}",
                        "h = Dilate(f, 2, (2, 7))"],
     [{"action": "reject", "line": line} for line in (
         "g = Dilate(f, 2, v)", "p = Dilate(l, 2, v)", "B = Dilate(A, 2, v)",
         "g = Dilate(f, 2, 2v)", "g = Dilate(f, 2, Vector((2, 7)))")])

for topic, command, prefix in (
    ("dilatelist", "Dilate(l, 2)", []),
    ("applylist", "ApplyMatrix(m, l)", ["m = {{2, 0}, {0, 2}}"]),
):
    for src in ("{A, 1}", "{A, {1, A, 1}, 1}"):
        case(topic, ["A = (1, 3)", *prefix, f"l = {src}", f"t = {command}"],
             [mv("A", 2, -1)])
    case(topic, ["A = (1, 3)", *prefix, "l = {1, A}"],
         [{"action": "reject", "line": f"t = {command}"}])


case("dilateorigin", ["A = (2, 3)", "v = (-1, 2)", "n = 2",
                      "l = {A, {v, A}, 1}", "p = Dilate(l, n)"],
     [setn("n", -2), setn("n", 0), mv("A", -1, 2), setn("n", 0.5)])

cases.write()
