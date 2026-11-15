#!/usr/bin/env python3
"""Writes cases/lists.json: lists in expressions.

List literals of numbers, points, vectors, booleans and lists, element
access `l(n)` and `Element(l, n)` (rounded, negative and out-of-range
indices), `Length`, `Sum` (with a truncation), the forms of `Sequence`
(a count, a range with and without a step, an expression of a variable),
element-wise arithmetic, with lists of unequal length, and a list minus a
point; `Product`, `Min`, `Max`, `Mean` and `Sum` of points; `Reverse`,
`First`, `Last` and `Sort`; `Append` and `Join`; the same operations
and commands on list objects; `If` with list branches.

The steps set the free numbers (`set_number`) and move the free points so
that lists grow, shrink and change, and come back to the first state.
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("lists")
case = cases.case
topic = cases.topic


def nums(**values):
    return [num(k, v) for k, v in values.items()]


# ------------------------------------------------------- literals
topic("lit")
for rhs in ("{1, 2, 3}", "{}", "{0.1 + 0.2, 1 / 0, sqrt(-1)}", "{(1, 2), (3, 4)}",
            "{{1, 2}, {3}}", "{true, 1 < 2}", "{1, (1, 2)}", "{{}, {{1}}}"):
    case("lit", [f"l = {rhs}"], [])
for rhs in ("{a, 2 a, a^2}", "{A, B}", "{A, v}", "{a < 3, true}", "{{a, 1}, {A}}",
            "{A + B, A - B, 2 A, A + v, v + v, -v, A / 2}", "{(1, 2), (a, 3)}", "{r, a}",
            "{x(A), y(B), Distance(A, B)}", "{a, A}"):
    case("lit", nums(a=2) + [pt("A", 1, 2), pt("B", 3, -1), "v = (1, 1)", "r = sqrt(a)",
                             f"l = {rhs}"],
         [setn("a", 4), mv("A", -2, 5), setn("a", -1), mv("B", 0, 0), setn("a", 2),
          mv("A", 1, 2), mv("B", 3, -1)])
case("lit", nums(a=2, b=3) + ["m = {a, b}", "l = {m, {1}}", "p = m"],
     [setn("a", 5), setn("b", -1), setn("a", 2), setn("b", 3)])

# ------------------------------------------------------- element access
# `l(n)` rounds `n` and counts a negative index from the end; 0 and an
# index past the end are undefined.
topic("index")
for rhs in ("l(n)", "l(-n)", "Element(l, n)", "Element(l, -n)", "l(n + 0.5)", "l(2.5)", "l(0)",
            "l(4)", "l(n) + 1", "2 l(n)^2", "l(l(1) / 10)"):
    case("index", nums(n=1) + ["l = {10, 20, 30}", f"c = {rhs}"],
         [setn("n", 2), setn("n", 3), setn("n", 0), setn("n", -1), setn("n", -3), setn("n", 4),
          setn("n", 2.6), setn("n", 2.4), setn("n", -2.5), setn("n", 1)])
for line in ("P = l(n)", "P = Element(l, n)", "c = x(l(n))", "P = l(n) + (1, 1)"):
    case("index", nums(n=1) + [pt("A", 1, 2), pt("B", 3, -1), "l = {A, B}", line],
         [setn("n", 2), mv("B", 5, 5), setn("n", 3), setn("n", -1), mv("A", 0, 0), setn("n", 1),
          mv("A", 1, 2), mv("B", 3, -1)])
case("index", nums(n=1, a=2) + ["l = {a < 3, a > 3}", "c = l(n)"],
     [setn("n", 2), setn("a", 4), setn("n", 1), setn("n", 3), setn("a", 2), setn("n", 1)])
case("index", nums(n=1) + ["l = {{1, 2}, {3}}", "m = l(n)", "c = Length(l(n))"],
     [setn("n", 2), setn("n", 3), setn("n", -1), setn("n", 1)])
case("index", nums(n=1) + ["l = {0.1, 0.2}", "c = (l(n) + 0.2 - 0.3) 1E20",
                           "d = (0.2 + l(n) - 0.3) 1E20"],
     [setn("n", 2), setn("n", 1)])
case("index", nums(n=1, a=0.1) + ["l = {a, 0.2}", "c = (l(n) + 0.2 - 0.3) 1E20"],
     [setn("n", 2), setn("a", 0.3), setn("n", 1), setn("a", 0.1)])

# ------------------------------------------------------- Length
topic("length")
for rhs in ("Length({})", "Length({1, 2, 3})", "Length(l)", "Length(Sequence(n))",
            "Length({a, {1, 2}})", "Length(l) + Length({A})"):
    case("length", nums(n=3, a=1) + [pt("A", 1, 2), "l = {a, 2, n}", f"c = {rhs}"],
         [setn("n", 0), setn("n", 5), setn("a", -1), setn("n", 3), setn("a", 1)])

# ------------------------------------------------------- Sum
# `AlgoStats1D`: plain doubles; `Sum({})` is 0; `Sum(l, n)` truncates.
topic("sum")
for rhs in ("Sum(l)", "Sum({})", "(Sum({0.1, 0.2}) - 0.3) 1E20", "(Sum(l) - 0.6) 1E20",
            "Sum(l, n)", "Sum({r, 1})", "Sum({a < 1, true})", "Sum(Sequence(k, k, 1, n))",
            "Sum(Sequence(1 / k^2, k, 1, n))", "Sum(l) + Sum({1, 2}, n)", "Sum({1 / 0, 1})"):
    case("sum", nums(a=0.1, n=2) + ["r = sqrt(a)", "l = {a, 0.2, 0.3}", f"c = {rhs}"],
         [setn("n", 0), setn("n", 1), setn("n", 3), setn("n", 4), setn("n", -1), setn("n", 1.7),
          setn("a", -1), setn("a", 0.5), setn("a", 0.1), setn("n", 2)])

# ------------------------------------------------------- Sequence
topic("seq")
case("seq", nums(n=3) + ["l = Sequence(n)"],
     [setn("n", 0), setn("n", -2), setn("n", 2.5), setn("n", 2.4), setn("n", 5), setn("n", 1),
      setn("n", 3)])
for rhs in ("Sequence(a, b)", "Sequence(a, b, s)", "Sequence(k, k, a, b)", "Sequence(k, k, a, b, s)",
            "Sequence(k^2, k, a, b)", "Sequence(k a, k, 1, b)", "Sequence(k < b, k, a, 4)",
            "Sequence((k, k^2), k, a, b)", "Sequence(A + (k, 0), k, 0, b)",
            "Sequence(Sequence(j + k, j, 1, k), k, a, b)", "Sequence(k, k, b, a, -s)",
            "Sequence({k, a}, k, 1, b)"):
    case("seq", nums(a=1, b=4, s=0.5) + [pt("A", 1, 2), f"l = {rhs}"],
         [setn("a", 4), setn("b", 1.5), setn("a", -2.4), setn("s", 0.1), setn("s", 0),
          setn("s", -0.5), setn("b", -3), mv("A", 3, 3), setn("s", 0.3), setn("a", 1),
          setn("b", 4), setn("s", 0.5), mv("A", 1, 2)])
for rhs in ("Sequence(0, 1, 0.1)", "Sequence(0.1, 1, 0.1)", "Sequence(1, 2, 0.25)",
            "Sequence(k, k, 0, 1, 0.1)", "Sequence(k - 0.3, k, 0, 1, 0.1)", "Sequence(5, 1)",
            "Sequence(2.5, 4.5)", "Sequence(k, k, 1, 3, 0)", "Sequence(k, k, 1, 1, 0)",
            "Sequence(k, k, 3, 1)"):
    case("seq", [f"l = {rhs}"], [])

# ------------------------------------------------------- arithmetic
# Element by element; a list and a number combine with every element; two
# lists of unequal length give an empty list.
topic("arith")
for rhs in ("l + m", "l + 1", "2 l", "l^2", "l / m", "l - m", "m l", "-l", "l + a", "sin(l)",
            "l + {1, 2}", "abs(m)", "l + Sequence(b)", "{A, B} + v", "{A, B} + (1, 1)",
            "2 {A, B}", "{v, 2 v} + v", "{1, 2} + (1, 2)", "{A, B} - A", "x({A, B})",
            "{a < 3, true} + 1"):
    case("arith", nums(a=2, b=3) + [pt("A", 1, 2), pt("B", 3, -1), "v = (1, 1)", "l = {1, 2, 3}",
                                   "m = {a, b, 1}", f"p = {rhs}"],
         [setn("a", 5), setn("b", 2), mv("A", -2, 5), setn("b", -1), setn("a", 0), mv("A", 1, 2),
          setn("a", 2), setn("b", 3)])

# `ExpressionNodeEvaluator.handleMinus`: a list minus a point or vector (or
# the other way round) is not element-wise but `GeoVec2D.sub`, which reads
# a list of two numbers as `(x, y)`; anything else is undefined.
for line in ("p = {1, 2} - A", "P = {1, 2} - A", "p = A - {1, 2}", "p = {a, b} - v", "P = {a, 2 b} - A",
             "p = {1, 2, 3} - A", "p = {A} - A", "p = {true, 1} - A", "c = x({a, b} - A) + 1",
             "P = 2 ({1, a} - A)", "p = v - {a, {1}}", "p = {} - v", "P = {A, B} - A"):
    case("arith", nums(a=2, b=3) + [pt("A", 1, 2), pt("B", 3, -1), "v = (1, 1)", line],
         [setn("a", 5), mv("A", -2, 5), setn("b", -1), setn("a", 2), mv("A", 1, 2), setn("b", 3)])

# ------------------------------------------------------- statistics
# `AlgoStats1D` (`Product`, `Mean`, `Sum` with a truncation) and
# `AlgoListMinMax`; `Sum` of points and vectors; `Min` and `Max` of two
# numbers stay the functions.
topic("stats")
for line in ("c = Product(l)", "c = Product({})", "c = Product(l, n)", "c = Product({a < 1, 2})",
             "c = Min(l)", "c = Max(l)", "c = Min({})", "c = Max({a, 1 / 0})", "c = Mean(l)",
             "c = Mean({})", "c = Mean({a, sqrt(a - 1)})", "c = Min(l) + Max(l) Mean(l)", "c = Min(a, 3)",
             "c = Max(n, a)", "P = Sum({A, B})", "P = Sum({A, v})", "u = Sum({v, 2 v})", "P = Sum({A, B}, n)",
             "c = Product(l) - Product(Sequence(n))", "c = Max(Sequence(k (-1)^k, k, 1, n))"):
    case("stats", nums(n=2, a=0.1) + [pt("A", 1, 2), pt("B", 3, -1), "v = (1, 1)",
                                      "l = {a, 0.2, 0.3, -1}", line],
         [setn("n", 0), setn("n", 3), setn("n", 5), setn("n", -1), setn("n", 1.7), setn("a", 2),
          setn("a", -3), mv("A", -2, 5), setn("a", 0.1), setn("n", 2), mv("A", 1, 2)])

# ------------------------------------------------------- reordering
# `Reverse`, `First`, `Last` (a count cut to a whole number) and `Sort` of
# numbers, booleans or points.
topic("order")
for line in ("p = Reverse(l)", "p = Reverse({})", "p = First(l)", "p = First(l, n)", "p = Last(l, n)",
             "p = First({})", "p = Last(l, 0)", "p = Sort(l)", "p = Sort(m)", "p = Sort({})", "p = Sort({1, A})",
             "p = Sort({a < 2, true, false})", "p = Sort({v, 2 v})", "p = Sort(Sequence(k (-1)^k, k, 1, n))",
             "p = Last(Sort(l))", "p = Reverse(m)", "p = First(m, n)", "p = Sort({{2}, {1}})",
             "c = Length(First(l, n))", "c = Sum(Reverse(l))", "p = Sort({a, a, 1})", "p = Sort({A, (1, a)})"):
    case("order", nums(n=2, a=3) + [pt("A", 1, 2), pt("B", 3, -1), "v = (1, 1)", "l = {a, 1, 2, -a}",
                                    "m = {A, B, (a, 0)}", line],
         [setn("n", 0), setn("n", 3), setn("n", 4), setn("n", 5), setn("n", -1), setn("n", 2.7), setn("a", 0),
          setn("a", -2), mv("A", 3, -1), setn("a", 1.5), mv("A", 1, 2), setn("a", 3), setn("n", 2)])

# ------------------------------------------------------- joining
# `Append` at either end and `Join` of its arguments or of a list of lists.
topic("join")
for line in ("p = Append(l, a)", "p = Append(a, l)", "p = Append(l, m)", "p = Append(m, l)", "p = Append({}, 1)",
             "p = Append(l, A)", "p = Append(A, {B})", "p = Join(l, m)", "p = Join(l, m, {})", "p = Join({l, m})",
             "p = Join({{1}, {2, a}})", "p = Join(l, Sequence(n))", "c = Length(Join(l, m))", "p = Join({A}, {B})",
             "p = Join(Append(l, n), Reverse(m))", "c = Sum(Append(l, n)) + Length(Append(l, {1, 2}))"):
    case("join", nums(n=2, a=3) + [pt("A", 1, 2), pt("B", 3, -1), "l = {a, 1}", "m = {2, a}", line],
         [setn("n", 0), setn("n", 3), setn("a", -1), mv("A", -2, 5), setn("n", -1), setn("a", 3),
          mv("A", 1, 2), setn("n", 2)])

# ------------------------------------------------------- list objects
# Every operation and command reads list objects, not literals, so that a
# step changes the list it reads: arithmetic, a list minus a point or
# vector (`GeoVec2D.sub`), element access and the list commands.
topic("objects")
for line in ("p = l + k", "p = l + 1", "p = 2 l", "p = l^2", "p = -l", "p = sin(l)", "p = l / k",
             "p = m + v", "p = 2 m", "c = x(m)", "p = b + 1", "p = w - A", "P = w - A",
             "p = A - w", "c = x(w - A)", "c = y(A - w) + 1", "p = l - A", "p = b - A",
             "P = 2 (w - A)", "p = w - v", "c = l(n)", "c = Element(l, n)", "P = m(n)",
             "c = x(m(n))", "p = q(n)", "c = Length(q(n))", "c = b(n)", "c = Length(l)",
             "c = Sum(l)", "c = Sum(l, n)", "P = Sum(m)", "c = Product(l)", "c = Min(l)",
             "c = Max(l)", "c = Mean(l)", "p = Reverse(l)", "p = First(l, n)", "p = Last(m, n)",
             "p = Sort(l)", "p = Sort(m)", "p = Sort(b)", "p = Append(l, a)", "p = Append(a, l)",
             "p = Join(l, k)", "p = Join(q)", "p = {l, 1}", "p = l", "p = Sequence(l(j) + j, j, 1, n)"):
    case("objects", nums(a=2, n=2) + [pt("A", 1, 2), pt("B", 3, -1), "v = (1, 1)", "l = {a, 1, 2 a}",
                                      "k = {3, a, 1}", "w = {a, 2}", "m = {A, B, (a, 0)}",
                                      "q = {{a, 1}, {2}}", "b = {a < 3, true}", line],
         [setn("a", 3), setn("n", 3), mv("A", -2, 5), setn("a", -1), setn("n", 1), setn("a", 5),
          mv("A", 1, 2), setn("a", 2), setn("n", 2)])

# ------------------------------------------------------- If of lists
# `AlgoIf` with list branches: the first true condition picks its branch,
# then the else branch; without one the list is undefined.
topic("if")
for line in ("p = If(a > 0, {1, 2})", "c = Length(If(a > 0, {1, 2}))", "p = If(a > 0, l)",
             "p = If(a > 0, l, k)", "p = If(a > 2, {1}, a > 0, l, k)", "p = If(a > 2, {1}, a > 0, l)",
             "p = {If(a > 0, l, {0}), 5}", "P = Sum(If(a > 0, m, {A}))", "p = If(a > 0, {a}, {})",
             "p = If(a > 0, m, l)", "p = If(a < 3, q, {{1}})", "p = If(a > 0, l, Sequence(n))",
             "c = Sum(If(a > 0, l, k)) + 1", "p = If(a > 0, l) + 1"):
    case("if", nums(a=1, n=2) + [pt("A", 1, 2), pt("B", 3, -1), "l = {a, 1}", "k = {3, a, 1}",
                                 "m = {A, B}", "q = {{a, 1}, {2}}", line],
         [setn("a", -1), setn("a", 2), setn("a", 3), mv("A", -2, 5), setn("n", 4), setn("a", 0),
          setn("a", 1), mv("A", 1, 2), setn("n", 2)])

cases.write()
