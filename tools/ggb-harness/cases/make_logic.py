#!/usr/bin/env python3
"""Writes cases/logic.json: booleans in expressions and `If`.

Comparisons `< > <= >= == !=` and their Unicode spellings at the edge of
GeoGebra's tolerance (values 1e-9, 1e-8 and 1e-7 apart, `0.1 + 0.2 == 0.3`),
points and vectors compared, undefined and infinite operands, the logical
operators `&& || ⊕ -> ¬ !` with their precedence, chains such as
`1 < a < 3`, booleans read as numbers, boolean objects and predicates
inside expressions.

The steps set the free numbers (`set_number`) and move the free points so
that every condition flips, and come back to the first state.
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("logic")
case = cases.case
topic = cases.topic
r2 = cases.r2


def nums(**values):
    return [num(k, v) for k, v in values.items()]


# ------------------------------------------------------- one comparison
topic("cmp")
for op in ("<", ">", "<=", ">=", "==", "!=", "≤", "≥", "≠", "≟", "<>"):
    case("cmp", nums(a=1, b=2) + [f"c = a {op} b"],
         [setn("a", 2), setn("a", 3), setn("a", 2 - 1e-9), setn("a", 2 + 1e-9),
          setn("a", 2 - 1e-7), setn("a", 2 + 1e-7), setn("b", -4), setn("a", -4),
          setn("a", 1), setn("b", 2)])

# ------------------------------------------------------- the tolerance
# `isGreater(a, b)` is `a > b + 1e-8`; `==` is `isEqual(a, b, 1e-8)` with
# strict bounds. The values sit 1e-9, 1e-8 and 1e-7 from each other, at
# both signs and at magnitudes where 1e-8 is below one unit in the last place.
topic("tol")
for rhs in ("0.1 + 0.2 == 0.3", "0.1 + 0.2 != 0.3", "0.1 + 0.2 > 0.3", "0.1 + 0.2 <= 0.3",
            "1 / 3 == 0.333333333", "1 / 3 == 0.3333333333", "sqrt(2)^2 == 2",
            "1E-9 == 0", "1E-8 == 0", "1E-7 == 0", "1E-9 > 0", "1E-8 > 0", "1E-7 > 0",
            "-1E-9 < 0", "-1E-8 < 0", "-1E-7 < 0", "1E-8 >= 0", "-1E-8 >= 0",
            "1E20 + 1 == 1E20", "1E20 + 1E5 > 1E20", "1E-300 > 0"):
    case("tol", [f"c = {rhs}"], [])
for lhs, rhs in (("a", "b"), ("b", "a")):
    for op in ("==", "<", "<=", "!="):
        case("tol", nums(a=1, b=1) + [f"c = {lhs} {op} {rhs}", f"d = {lhs} - {rhs}"],
             [setn("b", 1 + 1e-9), setn("b", 1 + 1e-8), setn("b", 1 + 2e-8), setn("b", 1 + 1e-7),
              setn("b", 1 - 1e-9), setn("b", 1 - 1e-8), setn("b", 1 - 2e-8),
              setn("b", 1 - 1e-7), setn("a", 1e8), setn("b", 1e8 + 1e-8), setn("b", 1e8 + 1e-7),
              setn("a", 0), setn("b", 1e-8), setn("b", -1e-8), setn("b", 1e-9), setn("a", 1),
              setn("b", 1)])
case("tol", nums(a=0.1, b=0.2) + ["c = a + b == 0.3", "d = a + b < 0.3", "g = 0.3 == a + b"],
     [setn("a", 0.7), setn("b", -0.4), setn("a", 0.1), setn("b", 0.2)])

# ------------------------------------------------------- logical operators
topic("logic")
for rhs in ("a < 3 && b > 1", "a < 3 || b > 1", "a < 3 ⊕ b > 1", "a < 3 -> b > 1",
            "a < 3 ∧ b > 1", "a < 3 ∨ b > 1", "a < 3 → b > 1", "a < 3 ⊻ b > 1",
            "¬(a < 3)", "!(a < 3)", "!(a < 3) && b > 1", "1 < a < 3", "1 < a <= b < 4",
            "a < b == b > a", "a < 3 || b > 1 && a > b", "a < 3 && b > 1 || a > b",
            "a < 3 -> b > 1 -> a > b", "(a < 3 -> b > 1) -> a > b", "a < 3 -> (b > 1 -> a > b)",
            "a < 3 ⊕ b > 1 ⊕ a > b", "a > 0 && b > 0 ⊕ a > b",
            "a < 3 == true", "!(a < 3) == false", "a < 3 != b > 1",
            "3 > a > 1", "a == b == true"):
    case("logic", nums(a=2, b=2) + [f"c = {rhs}"],
         [setn("a", 4), setn("b", 0), setn("a", 0.5), setn("b", 3), setn("a", 3), setn("b", 1),
          setn("a", 2), setn("b", 2)])

# ------------------------------------------------------- booleans as numbers
topic("boolnum")
for rhs in ("(a < 3) + 1", "2 (a < b)", "(a < 3) b", "(a < 3) - (b < 3)", "(a < 3)^2",
            "sqrt(a < b)", "(a < 3) / 2", "true + a", "false a", "(a < 3) + (b < 3) + 0.1",
            "round(a < 3)", "Max(a < 3, b < 3)"):
    case("boolnum", nums(a=2, b=4) + [f"c = {rhs}"],
         [setn("a", 5), setn("b", 1), setn("a", 0), setn("a", 2), setn("b", 4)])
for rhs in ("true > false", "true == 1", "false == 0", "true < 2", "true + 1 == 2"):
    case("boolnum", [f"c = {rhs}"], [])

# ------------------------------------------------------- points and vectors
topic("boolpt")
for rhs in ("A == B", "A != B", "A == u", "B == u", "A == (1, 2)", "A == a", "a == A",
            "x(A) < y(B)", "A + u == B", "A == B && a > 0", "Distance(A, B) < 1",
            "abs(u) >= 2", "u == (1, 2)", "A == (x(B), y(B) + 1E-9)", "A == (x(B), y(B) + 1E-7)"):
    case("boolpt", [pt("A", 1, 2), pt("B", 3, 4), "u = (1, 2)", num("a", 1),
                    f"c = {rhs}"],
         [mv("B", 1, 2), mv("A", 3, 4), mv("B", 3, 4), mv("A", 3, 4 + 1e-9),
          mv("A", 3 + 1e-7, 4), mv("A", 1, 2), mv("B", 3, 4)])

# ------------------------------------------------------- undefined and infinite operands
topic("boolundef")
for rhs in ("d > 3", "d < 3", "d == d", "d != 3", "d >= 3", "r < 3", "r > 3", "r == r",
            "r != r", "r != 3", "!(r < 3)", "r < 3 || a < 0", "r < 3 && a < 0", "r < 3 -> a < 0",
            "(r < 3) + 1", "d > 3 ⊕ r > 3", "1 < r < 3"):
    case("boolundef", nums(a=1) + ["d = 1 / a", "r = sqrt(a)", f"c = {rhs}"],
         [setn("a", 0), setn("a", -1), setn("a", 4), setn("a", 16), setn("a", 1)])
case("boolundef", [pt("A", 1, 2), num("a", 1), "C = (1 / a, 2)", "c = C == C", "d = C != A",
                   "g = x(C) < 3", "h = C == A"],
     [setn("a", 0), setn("a", 2), setn("a", 1)])

# ------------------------------------------------------- boolean objects
topic("boolobj")
case("boolobj", nums(a=2, b=1) + ["t = a < 3", "c = !t", "d = t && b > 0", "g = t ⊕ b > 0",
                                  "h = (t == (b > 0))", "k = t + t"],
     [setn("a", 4), setn("b", -1), setn("a", 2), setn("b", 1)])
case("boolobj", [pt("A", 0, 0), pt("B", 1, 1), pt("C", 2, 2), num("a", 1),
                 "p = AreCollinear(A, B, C)", "c = p || a > 2", "d = p && a > 0",
                 "g = !p", "h = AreCollinear(A, B, C) && a > 0"],
     [mv("C", 2, 3), setn("a", 3), mv("C", 2, 2), setn("a", 1)])
case("boolobj", nums(a=2) + ["t = true", "f = false", "c = t && a < 3", "d = f || a < 3",
                             "g = t -> f", "h = f → t"],
     [setn("a", 4), setn("a", 2)])

# ------------------------------------------------------- constant booleans
topic("boolconst")
for rhs in ("true", "false", "True", "False", "1 < 2", "¬false", "!true",
            "true ⊕ false", "true -> false", "false -> false", "2 < 3 < 4", "2 < 3 > 4",
            "true && false || true", "true || false && false"):
    case("boolconst", [f"c = {rhs}"], [])

# ------------------------------------------------------- If of numbers (AlgoIf)
# The first true condition picks its branch; an undefined condition is false;
# no true condition and no else branch is undefined.
topic("if")
for rhs in ("If(a < 3, 1, 2)", "If(a < 3, 5)", "If(a < 1, 10, a < 3, 20, 30)",
            "If(a < 1, 10, a < 3, 20)", "If(a < 3, b, -b)", "If(a < b, a, b)",
            "2 If(a < 3, a, -a) + 1", "0.2 + If(a < 3, 0.1, 0.2)", "If(a < 3, 0.1, 0.2) + 0.2",
            "(If(a < 3, 0.1, 0.2) + 0.2 - 0.3) 1E20", "(0.2 + If(a < 3, 0.1, 0.2) - 0.3) 1E20",
            "If(a < 3, 1 / 0, 2)", "1 / If(a < 3, 1 / 0, 2)", "If(r < 3, 1, 2)", "If(a < 3, r, 2)",
            "If(r != r, 7, 8)", "If(true, a, b)", "If(a == b, 1, 0)", "If(a < 3 && b > 0, a b, 0)",
            "If(a < 3, a, a < 5, -a)", "If(a > 1, If(a > 3, 3, 1), 0)", "If(a < 3, 1, 2) + If(b < 3, 1, 2)",
            "If(a < 3, a + 0.1, a - 0.1)", "If(a < 3, sqrt(a), 0)"):
    case("if", nums(a=2, b=1) + ["r = sqrt(a)", f"c = {rhs}"],
         [setn("a", 0.5), setn("a", 4), setn("a", 3), setn("b", 4), setn("a", -1), setn("a", 6),
          setn("b", 1), setn("a", 2)])
for rhs in ("If(1 < 2, 3, 4)", "If(2 < 1, 3, 4)", "If(2 < 1, 3)", "If(false, 1, true, 2, 3)"):
    case("if", [f"c = {rhs}"], [])

# ------------------------------------------------------- If of points and vectors
# The result has the most general type of the branches: a number in a point
# result is the point (v, 0), whichever branch it is.
topic("ifpt")
for line in ("P = If(a < 3, A, B)", "P = If(a < 3, A, 2)", "P = If(a < 3, 2, A)", "P = If(a < 3, A)",
             "P = If(a < 3, A + B, (1, 1))", "P = If(x(A) < x(B), A, B)", "u = If(a < 3, v, w)",
             "u = If(a < 3, v, 2 w)", "P = If(a < 3, Midpoint(A, B), B)", "P = If(a < 1, A, a < 3, B, (0, 0))",
             "c = x(If(a < 3, A, B))", "P = If(A == B, A, (5, 5))"):
    case("ifpt", nums(a=2) + [pt("A", 1, 2), pt("B", 3, 4), "v = (1, 2)", "w = (2, -1)", line],
         [setn("a", 4), mv("A", 5, 1), setn("a", 0.5), mv("B", 5, 1), mv("A", 1, 2), mv("B", 3, 4),
          setn("a", 2)])

# ------------------------------------------------------- If of booleans
topic("ifbool")
for rhs in ("If(a < 3, a > 0)", "If(a < 3, true, false)", "If(a < 1, a > 0, b > 0)",
            "If(a < 1, a > 0, a < 3, b > 0, false)", "!If(a < 3, b > 0, true)"):
    case("ifbool", nums(a=2, b=1) + [f"c = {rhs}"],
         [setn("a", 0.5), setn("b", -1), setn("a", 4), setn("a", 2), setn("b", 1)])

# ------------------------------------------------------- conditional functions
# With x in a condition, If is an operation of the function: IF, IF_ELSE (an
# undefined condition gives NaN) or IF_LIST. Only the objects outside every
# If decide whether the function is defined.
topic("iffn")
for rhs in ("If(x < 0, -x, x)", "If(x < 0, x^2)", "If(x < -1, 1, x < 1, 2, 3)",
            "If(x < -1, 1, x < 1, 2)", "If(x < a, x, a)", "If(x < r, 1, 2)", "If(x < 0, a, 2)",
            "a + If(x < 0, 1, 2)", "If(x < 0, r, 2)", "r + If(x < 0, 1, 2)", "2 If(x < 0, -x, x) + 1",
            "If(x > 0 && x < 2, x, 0)", "If(x <= 0, sin(x), cos(x))", "If(-1 < x < 1, x^3)",
            "If(x == 0, 1, 1 / x)", "If(x < 0, 1 / r, r)", "If(x < r, x, x < 2 r, 2, 0)",
            "If(x < 0.5, 0.1, 0.2) + 0.2", "If(x != 1, x, 5)", "If(x < 0, 1 / 0, 1)",
            "If(a < 3, x, 2)", "If(a < 3, x^2)"):
    case("iffn", nums(a=1) + ["r = sqrt(a)", f"f(x) = {rhs}"],
         [setn("a", 4), setn("a", -1), setn("a", 0), setn("a", 1)])
case("iffn", nums(a=1) + ["f(x) = If(x < 0, -x, x)", "v = f(a)", "g(x) = f(x) + 1", "w = f(-3)",
                          "h(x) = f(f(x) - 2)", "k = f(a - 3) + f(0.5)"],
     [setn("a", -2), setn("a", 3), setn("a", 1)])
case("iffn", nums(a=1) + ["f(x) = If(x < a, x^2, x)", "v = f(2)", "g(x) = If(x < 0, f(x), -f(x))"],
     [setn("a", 3), setn("a", -1), setn("a", 1)])


# CommandsTest.cmdDefined uses a regular polygon.
case("defined", [
    "A = (1, 1)", "B = (2, 0.5)", "p = Polygon(A, B, 4)",
    "b = Defined(p)", "c = Defined(Polygon(A, B, 4))", "d = Defined(Line(A, B))",
], [mv("A", 0, 0)])
case("defined", [
    "a = 1", "n = 1 / a", "r = sqrt(a)", "A = (n, r)", "v = (n, r)",
    "f(x) = x + r", "g(x) = sqrt(x)", "l = {r, A}",
    "b = Defined(n)", "c = Defined(A)", "d = Defined(v)", "e = Defined(r)",
    "h = Defined(f)", "i = Defined(g)", "j = Defined(l)",
    "k = Defined(1 / a)", "m = Defined((n, r))",
], [setn("a", 0), setn("a", -1), setn("a", 2)])
case("defined", [
    "A = (0, 0)", "B = (2, 0)", "C = (0, 1)", "D = (2, 2)",
    "g = Line(A, B)", "h = Line(C, D)", "P = Intersect(g, h)",
    "b = Defined(P)", "c = Defined(Intersect(g, h))",
], [mv("D", 2, 1), mv("C", 0, 0), mv("D", 2, 2),
    {"action": "reject", "line": "d = Defined(A, B)"}])

case("defined", [
    "A = (0, 0)", "B = (3, 0)", "C = (1, 1)",
    "a = CircumcircularArc(A, B, C)", "b = Defined(a)",
], [mv("C", 1, 0), mv("C", 4, 0), mv("C", 1, 1)])

cases.write()
