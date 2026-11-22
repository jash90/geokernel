#!/usr/bin/env python3
"""Record repeated assignments and the effects on their dependents."""

from caselib import Cases, mv, setn

cases = Cases("redefine")
cases.topic("assignment")

def redefine(line):
    return {"action": "redefine", "line": line}


cases.case("assignment", ["a = 1", "A = (1, 1 / a)"], [redefine("a = 0"), setn("a", 1)])
cases.case("assignment", ["a = 1", "A = (1, 1 / a)"],
           [redefine("a = 0"), redefine("a = 2"), setn("a", 0), setn("a", 1)])
cases.case("assignment", ["a = 1", "A = (1, 1 / a)"], [redefine("a = ?"), setn("a", 1)])
cases.case("assignment", ["a = 1", "A = (1, 1 / a)"], [redefine("a = ?"), redefine("a = 2")])
cases.case("assignment", ["a = 1", "f(x) = 3 a", "l = {f}"], [redefine("a = 2")])
cases.case("assignment", ["seqa = 2 (1..5)"], [redefine("seqa = (1..3) + 3")])
cases.case("assignment", ["la = {1}", "lb = {2}", "lc = la"], [redefine("lc = lb")])
cases.case("assignment", ["f(x) = x (2 x - 2)", "b = IsFactored(f)"], [redefine("f(x) = x (x - 1)")])
cases.case("assignment", ["r = 2", "a = 3", "b = a^2"], [redefine("a = 1 / r"), setn("r", 0), setn("r", 4)])
cases.case("assignment", ["A = (1, 1)", "B = 2 A"], [redefine("A = (3, 2)"), mv("A", -2, 4)])
cases.case("assignment", ["A = (1, 1)"], [redefine("A = Point(xAxis)"), mv("A", 5, 2)])
cases.case("assignment", ["a = 2"], [redefine("a = {1, 2}")])
cases.case("assignment", ["A = (1, 2)"], [redefine("A = 6")])
cases.case("assignment", ["r = 1", "a = 2", "b = a + 1"],
           [redefine("a = r + 2"), {"action": "reject", "line": "a = b + 1"}, setn("r", 3)])
cases.case("assignment", ["A = (1, 2)", "B = 2 A", "n = 2"],
           [redefine("A = (1 + n, 1 / n)"), setn("n", 0), setn("n", 4)])
cases.case("assignment", ["a = 1", "b = a + 1", "n = 2", "d = n + b"],
           [redefine("a = 2 n"), {"action": "reject", "line": "a = d"}, setn("n", 4)])

for starting, replacement in (("Point(c)", "(1, 1)"), ("(1, 1)", "Point(c)")):
    cases.case("intersection", [
        "O = (0, 0)", "c = Circle(O, 2)", f"A = {starting}", "B = (5, 0)",
        "l = Line(A, B)", "X = Intersect(l, c, 1)", "Y = Intersect(l, c, 2)",
    ], [redefine(f"A = {replacement}"), mv("B", 4, 6), mv("B", 5, 0)])

cases.case("order", [
    "O = (0, 0)", "c = Circle(O, 2)", "A = Point(c)", "B = Point(c, 0.25)",
    "t = 0", "l = Line(A, B)", "X = Intersect(l, c, 1)", "Y = Intersect(l, c, 2)",
], [redefine("A = Point(c, t)"), setn("t", 0.25), setn("t", 0),
    mv("O", 2, 1), mv("O", 0, 0)])
cases.case("order", [
    "O = (0, 0)", "c = Circle(O, 2)", "A = Point(c)", "B = (5, 0)",
    "l = Line(A, B)", "X = Intersect(l, c, 1)",
], [redefine("A = (1, 1)"), mv("B", 1, 1), mv("B", 5, 0)])
cases.case("tangent", [
    "O = (0, 0)", "c = Circle(O, 2)", "A = Point(c)", "l = Tangent(A, c)",
    "X = Intersect(l, c, 1)",
], [redefine("A = (5, 1)"), mv("A", 1, 1), mv("A", 5, 1)])
cases.case("circlepair", [
    "O = (0, 0)", "c = Circle(O, 2)", "A = Point(c)", "B = (3, 1)",
    "d = Circle(B, A)", "X = Intersect(c, d, 1)", "Y = Intersect(c, d, 2)",
], [redefine("A = (1, 1)"), redefine("A = Point(c)"), mv("B", 5, 0), mv("B", 3, 1)])

cases.case("assignment", ["a = 1", "b = 2 a"],
           [redefine("a = Slope(y = 2x)")])
cases.case("assignment", ["a_{1} = 2", "b = 2 a_{1}"],
           [redefine("a_1 = 3"), redefine("a_{1} = 5")])
cases.case("deletion", ["X = (1, 1)", "A = (0, 0)", "B = 2 A", "C = (4, 1)"],
           [redefine("A = X + (1, 0)"), {"action": "delete", "label": "X"}])

cases.write()
