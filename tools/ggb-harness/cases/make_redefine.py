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

cases.write()
