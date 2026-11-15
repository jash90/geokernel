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

cases.write()
