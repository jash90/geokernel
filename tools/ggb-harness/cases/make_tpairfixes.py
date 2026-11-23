#!/usr/bin/env python3
"""Writes cases/tpairfixes.json: behaviour the reference's own kernel tests
pin down and the earlier groups did not cover.

- `fold`: `Sum` and `Product` of an expression over a range when the
  expression is a point or a vector (`AlgoFoldExpression` with
  `PointNDFold`): a sum adds the coordinates, a product multiplies them as
  complex numbers.
- `ziptext`: `Zip` whose variable runs over a list of texts: the variable
  is the text element itself.
- `fnundef`: a function whose expression is the undefined constant alone
  is an undefined function, while any other expression without a value is
  a defined function whose values are undefined.
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("tpairfixes")
case = cases.case
topic = cases.topic

# ------------------------------------------------------- folds over points
topic("fold")
for line in ("C = Sum((k, k), k, 1, 5)", "C = Product((k, k), k, 1, 5)",
             "v = Sum((k, k), k, 1, 3)", "v = Product((k, 1), k, 1, 3)",
             "C = Sum((k, 2), k, 3, 3)", "C = Sum((k, 2), k, 3, 1)", "C = Product((k, 2), k, 3, 1)",
             "C = Sum((k, 1 / (k - 2)), k, 1, 3)", "C = Sum((k, 1), k, 1.5, 3.5)",
             "C = Product((1, k), k, -1, 1)", "C = Sum((k; 90°), k, 1, 2)",
             "C = Sum((k, k) + (1, 0), k, 1, 3)", "C = Sum(2 (k, k), k, 1, 3)"):
    case("fold", [line], [])
MOVES = [setn("n", 1), setn("a", -1), setn("n", 0), setn("n", 2.5), setn("n", 4), mv("A", 0, 0),
         setn("a", 0), setn("a", 2), setn("n", 3), mv("A", 1, 2)]
for line in ("C = Sum((k, a), k, 1, n)", "C = Product((k, a), k, 1, n)", "C = Sum(A + (k, 0), k, 1, n)",
             "C = Product(A + (k, 0), k, a, n)", "C = Sum(k A, k, a, n)", "w = Sum(k u, k, 1, n)",
             "w = Product(u + (k, 0), k, 1, n)", "C = Sum((k, 1 / a), k, 1, n)",
             "C = Sum((x(A) k, k^2), k, a, a + n)", "C = Product((1, 1 / (k - a)), k, 1, n)"):
    case("fold", [num("a", 2), num("n", 3), pt("A", 1, 2), "u = (2, -1)", line], MOVES)
# whether the result is a point or a vector follows the folded expression
# (`ExpressionNode.shouldEvaluateToGeoVector`), never the label
for line in ("C = Sum(-(k, k), k, 1, 3)", "C = Sum((k, k) 2, k, 1, 3)",
             "C = Sum(u - k A, k, 1, 3)", "C = Sum(k A - u, k, 1, 3)",
             "C = Sum((k, 0) + u, k, 1, 3)", "w = Sum(k u + A, k, 1, 3)",
             "C = Sum(k u - (1, 0), k, 1, 3)", "w = Sum(A + k u, k, 1, 3)",
             "C = Sum(a k u, k, 1, 3)", "w = Sum(k A a, k, 1, 3)", "C = Sum(a (k, 1), k, 1, 3)",
             "w = Sum((k, 1) / a, k, 1, 3)", "C = Sum(u / k + (k, 0), k, 1, 3)",
             "C = Product(k u, k, 1, 3)", "w = Product(A + (k, 0), k, 1, 3)",
             "C = Sum(-k u, k, 1, 3)", "w = Sum(-(k A), k, 1, 3)", "C = Sum(k (1; 2), k, 1, 3)",
             "C = Sum(k u + k u, k, 1, 3)", "w = Sum(k A + k A, k, 1, 3)",
             "C = Sum(k^2 u, k, 1, 3)", "C = Sum(sqrt(k) (1, 1), k, 1, 3)",
             "C = Sum(u + k A, k, 1, 3)", "C = Sum(k u + 2 A, k, 1, 3)", "C = Sum(u k, k, 1, 3)",
             "w = Sum(A k, k, 1, 3)", "C = Sum((k u), k, 1, 3)", "C = Sum(k (u + A), k, 1, 3)",
             "C = Sum(k (A + u), k, 1, 3)", "C = Sum((u + (k, 0)) 2, k, 1, 3)"):
    case("fold", [num("a", 2), pt("A", 1, 2), "u = (2, -1)", line], [])

# ------------------------------------------------------- a Zip variable that is a text
topic("ziptext")
TEXTS = [num("a", 2), "t = {\"ab\", \"c\", \"def\"}", pt("P", 1, 2)]
for line in ("l = Zip(A, A, t)", "l = Zip(A + \"x\", A, t)", "l = Zip(\"<\" + A + \">\", A, t)",
             "l = Zip(Text(A), A, t)", "l = Zip(Text(A, P), A, t)",
             "l = Zip(Text(A, B, true, true, C, 0), A, t, B, {(0, 1), (0, 2), (0, 3)}, C, {-1, 0, 1})",
             "l = Zip(A + k, A, t, k, {1, a})",
             "l = Zip(A + B, A, t, B, {\"1\", \"2\", \"3\", \"4\"})", "l = Zip(A + P, A, t)",
             "l = Zip(Zip(A + B, B, {\"x\", \"y\"}), A, t)", "l = Zip({A, k}, A, t, k, {1, 2, 3})",
             "l = Zip(A + a, A, t)",
             "l = Zip(Text(a) + A, A, t)", "l = Zip({A, A + A}, A, t)"):
    case("ziptext", TEXTS + [line], [setn("a", 1), mv("P", -3, 4), setn("a", 0), setn("a", 3),
                                    setn("a", 2), mv("P", 1, 2)])

# ------------------------------------------------------- the undefined function
topic("fnundef")
for rhs in ("?", "0 / 0", "? + 0 x", "? x", "sqrt(-1)", "1 / 0", "inf - inf", "x + ?", "?^2",
            "-?", "(?)", "? + 1", "? ?", "ln(-1) + x", "If(x > 0, ?)", "If(x > 0, ?, ?)",
            "inf", "-inf", "NaN", "Undefined", "0 ?", "? / x", "+?", "-(?)", "--?", "((?))",
            "-NaN", "?°", "? + 0", "1 ?"):
    case("fnundef", [f"f(x) = {rhs}", "g(x) = f(x) + 1", "r = f(1)", "b = IsDefined(f)"], [])
for rhs in ("a / a", "sqrt(a)", "a / 0", "ln(a) + x", "x / a"):
    case("fnundef", [num("a", 2), f"f(x) = {rhs}", "g(x) = f(x) + 1", "r = f(1)",
                     "b = IsDefined(f)"],
         [setn("a", 0), setn("a", -1), setn("a", 2)])

topic("matrixrow")
case("matrixrow", ["m = {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}}", "k = 2", "r = m(k)"],
     [setn("k", -1), setn("k", -5), setn("k", 0), setn("k", 1)])
for matrix in ("{{(1, 2), (3, 4)}, {(5, 6), (7, 8)}}",
               '{{"a", "b"}, {"c", "d"}}', "{{true, false}, {false, true}}",
               "{{1, 2}, {3}}", "{{(1, 2)}, {(5, 6), (7, 8)}}"):
    case("matrixrow", [f"m = {matrix}", "k = 2", "r = m(k)"],
         [setn("k", 3), setn("k", 0), setn("k", -1), setn("k", 1)])

topic("symbolic")
case("symbolic", ["f(x) = (3 / 2)^x", "n = 2", "a = f(n)", "b = 5 * 10^(-2)"],
     [setn("n", -1), setn("n", -2), setn("n", 0)])
case("symbolic", ["n = 8", "a = sqrt(n)", "b = sqrt(1024 * 2 * 3)"],
     [setn("n", -3), setn("n", 17), setn("n", 8)])

case("numbertext", ["a = 2", "b = 1 / a", "t = Text(a)", "u = Text(b)"],
     [setn("a", 1e100), setn("a", 1e-100), setn("a", -0.0), setn("a", 0),
      setn("a", -2)])
case("numbertext", ["a = 4", "b = sqrt(a)", "t = Text(b)"],
     [setn("a", -1), setn("a", 0), setn("a", 4)])

cases.write()
