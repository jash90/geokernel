#!/usr/bin/env python3
"""Writes cases/lists.json: lists in expressions.

List literals of numbers, points, vectors, booleans and lists, element
access `l(n)` and `Element(l, n)` (rounded, negative and out-of-range
indices), `Length`, `Sum` (with a truncation), the forms of `Sequence`
(a count, a range with and without a step, an expression of a variable),
element-wise arithmetic, with lists of unequal length, and a list minus a
point; `Product`, `Min`, `Max`, `Mean` and `Sum` of points; `Reverse`,
`First`, `Last` and `Sort`; `Append` and `Join`; the same operations
and commands on list objects; `If` with list branches; the same
commands on lists of texts (`Sort` by `GeoText.getComparator`,
`Unique`, `Frequency`, the element and set commands) and the string
forms of `First`, `Last` and `Element` (UTF-16 units).

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

# ------------------------------------------------------- limits of a range
# `Sequence(n)` counts to `(int) Math.round(n)`: past `int` it wraps, and 0
# or less is empty. A range past `MyMath.LARGEST_INTEGER` and an infinite
# count are undefined lists, elements of a list around them.
topic("limit")
case("limit", nums(n=3) + ["l = Sequence(n)"],
     [setn("n", 1e20), setn("n", 3), setn("n", 4294967299), setn("n", -5), setn("n", 3),
      setn("n", 3e9), setn("n", 3)])
for rhs in ("{Sequence(1, n), {1}}", "Sequence(Sequence(1, n), k, 1, 3)",
            "{Sequence(n, 1E20), {1}}"):
    case("limit", nums(n=3) + [f"l = {rhs}"], [setn("n", 1e20), setn("n", 3)])
case("limit", nums(n=1) + ["l = {Sequence(k, k, 0, n, 1E-320), {1}}"],
     [setn("n", 2), setn("n", 1)])

# ------------------------------------------------------- Take
# `AlgoTake`: the bounds are truncated towards zero, a missing to-bound is
# the end of the list; an undefined bound gives an empty list, a zero
# to-bound inside the list does too, any other run outside it is undefined.
topic("take")
for rhs in ("Take(l, m)", "Take(l, m, n)", "Take(l, 2)", "Take(l, 2, 4)", "Take(l, 4)",
            "Take(l, 0)", "Take(l, 2, 6)", "Take(l, 3, 2)", "Take(l, 2, 0)", "Take(l, 9, 0)",
            "Take(l, 1.9, 3.7)", "Take(l, -1)", "Take({}, 1, 2)", "Take(w, 1, 2)",
            "Take({{1, 2}, {3, 4}}, 2, 2)", "Take(l, m) + 1", "Take(l, 1, n)",
            "Take(l, u, 3)"):
    case("take", nums(m=2, n=3, a=1) + [pt("A", 1, 2), "u = 0 / 0",
                                        "l = {a, 1, 2, a + 2, 4}", "w = {A, (2, 0), (a, 3)}",
                                        f"p = {rhs}"],
         [setn("m", 3), setn("n", 5), setn("m", 1), setn("n", 0), setn("m", 6), setn("n", 2),
          setn("n", 4), setn("m", 2), setn("n", 3), setn("a", 5), setn("a", 1)])

# ------------------------------------------------------- RemoveUndefined
# `AlgoRemoveUndefined`: undefined numbers and points drop out, a defined
# list stays whatever its elements are.
topic("undef")
for rhs in ("RemoveUndefined(l)", "RemoveUndefined({})", "RemoveUndefined(m)",
            "RemoveUndefined({1 / 0, sqrt(-1), 2})", "RemoveUndefined({(1 / 0, 2), (1, 1)})",
            "RemoveUndefined(q)", "RemoveUndefined(b)", "RemoveUndefined(l) + 1",
            "RemoveUndefined({r, 1})", "RemoveUndefined({a / 0, a})"):
    case("undef", nums(a=2) + [pt("A", 1, 2), pt("B", 4, 0), "r = sqrt(-a)",
                               "l = {a, 1 / 0, a + 1, sqrt(-a)}", "m = {A, B / (a - 2), (1 / 0, 1)}",
                               "q = {{a, 1 / 0}, {2, 3}}", "b = {a < 0, true}", f"p = {rhs}"],
         [setn("a", -1), setn("a", 0), setn("a", 2), mv("B", 3, 3), setn("a", 2)])

# ------------------------------------------------------- Flatten
topic("flatten")
for rhs in ("Flatten({})", "Flatten({{}, {{{}}}})", "Flatten({{(1, 1)}, {{{4}}}})",
            "Flatten(l)", "Flatten(q)", "Flatten({1, {2, {3}}})", "Flatten({l, {1}})",
            "Sum(Flatten({{1, 2}, {3}}))"):
    case("flatten", nums(a=2) + [pt("A", 1, 2), "l = {{a, 1}, {2, a}}",
                                 "q = {{{1}}, {{}, {2}}}", f"p = {rhs}"],
         [setn("a", 5), setn("a", 2)])

# ------------------------------------------------------- Unique
# `AlgoUnique`: numbers in the order of `FrequencyGgb`, which also folds
# values within `1e-8` of each other into the first of them; other kinds
# keep their first appearance.
topic("unique")
for rhs in ("Unique(l)", "Unique({})", "Unique({1, 2, 3, 3, 4, 5})", "Unique({5, 1, 4, 1})",
            "Unique({1 / 3, 1 / 3 + 1E-16})", "Unique({1 / 3 * 3, 1})", "Unique(m)",
            "Unique(b)", "Unique(q)", "Unique(l) + 1", "Unique({2.800000000000001, 2.8, 2.9})"):
    case("unique", nums(a=2) + [pt("A", 1, 2), pt("B", 3, -1),
                                "l = {a, 1, a, 2, a + 1}", "m = {A, B, (1, 2)}",
                                "b = {true, true, false}", "q = {{1}, {1}, {2}}", f"p = {rhs}"],
         [setn("a", 1), setn("a", 3), mv("A", 3, -1), setn("a", 2), mv("A", 1, 2)])

# ------------------------------------------------------- Frequency
# `AlgoFrequency`: counts per unique value, per class bounds (the highest
# class holds both bounds, a falling class list its first), cumulative,
# density and a scale factor.
topic("freq")
for rhs in ("Frequency(l)", "Frequency({2, 3, 4})", "Frequency({1, 2, 3, 3, 4, 5})",
            "Frequency(t, l)", "Frequency(b, l)", "Frequency(k, l)",
            "Frequency(false, k, l)", "Frequency(t, k, l)", "Frequency(b, k, l)",
            "Frequency(k, l, b)", "Frequency(t, k, l, b)", "Frequency(k, l, b, s)",
            "Frequency(t, k, l, b, s)", "Frequency(k, l, b, -s)", "Frequency(k, l, b, 0)",
            "Frequency(k, l, true, 4)", "Frequency(w, l)", "Frequency({1}, l)",
            "Frequency(k, {})", "Frequency(k, {1})", "Frequency(m, l)", "Frequency(k, k)",
            "Frequency({5, 4, 3}, {3, 4, 5})", "Frequency(k, l, t)"):
    case("freq", nums(s=2, a=1) + ["b = false", "t = true",
                                   "l = {a, 2, a + 1, 2, a + 3}", "k = {0, 2, 4, 6}",
                                   "w = {2, 4}", "m = {(1, 1), (2, 2)}", f"p = {rhs}"],
         [setn("a", 2), setn("a", 0), setn("a", 1), setn("s", 4), setn("s", 0), setn("s", 2)])

# ------------------------------------------------------- Classes
topic("classes")
for rhs in ("Classes(l, n)", "Classes(l, 1)", "Classes(l, 2)", "Classes(l, 4)",
            "Classes(l, 2.9)", "Classes(l, 0)", "Classes(l, -1)", "Classes(l, n, w)",
            "Classes(l, 2, 0.5)", "Classes(l, 3, -1)", "Classes(l, 13, 4)", "Classes(l, 1, 2)",
            "Classes({}, 3)", "Classes(m, 2)", "Classes({1 / 0, 2}, 2)"):
    case("classes", nums(n=3, w=1) + ["a = 3", "l = {2, a, 4}", "m = {(1, 1), (2, 2)}",
                                      f"p = {rhs}"],
         [setn("n", 1), setn("n", 5), setn("n", 2), setn("w", 2), setn("w", 0), setn("a", 6),
          setn("w", 1), setn("a", 3), setn("n", 3)])

# ------------------------------------------------------- IndexOf
topic("indexof")
for rhs in ("IndexOf(a, l)", "IndexOf(5, l)", "IndexOf(5, l, n)", "IndexOf(5, l, 4)",
            "IndexOf(5, l, 6)", "IndexOf(5, l, 0)", "IndexOf(5, l, 2.9)", "IndexOf(3, l)",
            "IndexOf(a, l, 2)", "IndexOf((1, 2), m)", "IndexOf(A, m)", "IndexOf(true, b)",
            "IndexOf(A, m, 2)", "IndexOf(a, {})", "IndexOf(a, l) + 1",
            "IndexOf(2, l, -1E300)", "IndexOf(2, l, 1E300)"):
    case("indexof", nums(a=2, n=4) + [pt("A", 1, 2), "l = {1, 3, a, 2, a, 4}",
                                      "m = {A, (1, 2), (0, 0)}", "b = {false, true}",
                                      f"c = {rhs}"],
         [setn("a", 5), setn("n", 5), setn("a", 2), setn("n", 0), mv("A", 0, 0), setn("n", 6),
          mv("A", 1, 2), setn("n", 4)])

# ------------------------------------------------------- Insert
topic("insert")
for rhs in ("Insert(a, l, 2)", "Insert(a, l, 1)", "Insert(a, l, 6)", "Insert(a, l, 0)",
            "Insert(a, l, -1)", "Insert(a, l, -2)", "Insert(a, l, 1.9)", "Insert(a, {}, 1)",
            "Insert(k, l, 2)", "Insert(k, l, 4)", "Insert(A, l, 2)", "Insert(a, l, u)",
            "Insert(a, m, 2)", "Length(Insert(k, l, 2))"):
    case("insert", nums(a=9) + [pt("A", 1, 2), "u = 0 / 0", "l = {1, 2, 3, 4, 5}",
                                "k = {6, 7}", "m = {(1, 1), (2, 2)}", f"p = {rhs}"],
         [setn("a", 4), setn("a", 9)])

# ------------------------------------------------------- set operations
topic("setops")
for rhs in ("Intersection(l, k)", "Intersection(k, l)", "Intersection(l, {})",
            "Intersection({}, {})", "Intersection(m, n)", "Union(l, k)", "Union(k, l)",
            "Union(l, {})", "Union({}, {})", "Union(m, n)", "Union(b, {false})",
            "Remove(l, k)", "Remove(k, l)", "Remove(l, {9})", "Remove(l, {})",
            "Remove({1, 1 / 0, 2}, {1 / 0, 1})", "Remove({1, 2, 3}, {2, 4})"):
    case("setops", nums(a=1) + [pt("A", 1, 2), pt("B", 3, -1), "l = {a, 2, 3, 2}",
                                "k = {2, 2, 2, 4, a}", "m = {A, B, (1, 2)}", "n = {(1, 2), A}",
                                "b = {true, true}", f"p = {rhs}"],
         [setn("a", 2), setn("a", 4), mv("A", 1, 2), setn("a", 1)])

# ------------------------------------------------------- Sort of two lists
# `AlgoSort` with a value list: the keys sort, the values follow their
# keys; every key must be of one class (`geo.getClass()`), and equal
# keys keep the order of the list (their construction indexes tie, so
# the comparator sends the later one to the right).
# ------------------------------------------------------- wrapped forms
topic("wrapped")
for rhs in ("Last(l)", "Reverse(a, b, 3)", "Reverse(A, (0, 1))",
            "Sort(A, (0, 1), B)", "OrdinalRank(A, (0, 0))", "TiedRank(A, (0, 0))"):
    case("wrapped", nums(a=2, b=1) + [pt("A", 1, 2), pt("B", 3, 0),
                                      "l = {a, b, a + b}", f"p = {rhs}"],
         [setn("a", 5), setn("a", 2), mv("A", 0, 1)])

topic("sort2")
for rhs in ("Sort(l, k)", "Sort(k, l)", "Sort({3, 1, 2}, {10, 20, 30})",
            "Sort({3, 1, 3}, {10, 20, 30})", "Sort({3, 1, 2}, {10, 20})",
            "Sort({3, 1, 2}, m)", "Sort({2.800000000000001, 1}, {10, 20})",
            "Sort(m, {A, u})", "Sort(m, {u, u, A})", "Sort({1, 2}, {45°, 1})",
            "Sort({1, 2}, {1, 45°})", "Sort({30, 10, 20}, {2, 1, 2})",
            "Sort({\"c\", \"a\", \"b\"}, {2, 1, 2})", "Sort(l, {2, 1, 2})"):
    case("sort2", nums(a=2) + [pt("A", 1, 2), "u = (1, 0)", "l = {a, 1, 2}", "k = {10, 20, 30}",
                               "m = {A, (0, 0), (5, 5)}", f"p = {rhs}"],
         [setn("a", 3), setn("a", 1), setn("a", 2)])

# ------------------------------------------------------- ranks
topic("ranks")
for rhs in ("OrdinalRank(l)", "TiedRank(l)", "OrdinalRank({3, 1, 3})", "TiedRank({3, 1, 3})",
            "OrdinalRank({1, 2, 3, 4, 5})", "TiedRank({1, 2, 3, 4, 5})",
            "OrdinalRank({})", "TiedRank({})", "OrdinalRank(3, 1, 2)", "TiedRank(3, 1, 3)",
            "OrdinalRank(m)", "OrdinalRank({r, 2})", "TiedRank({r, 2})",
            "Sum(TiedRank(l))", "OrdinalRank({-2, 5, -2, 0})"):
    case("ranks", nums(a=2) + [pt("A", 1, 2), "r = sqrt(-a)", "l = {a, 1, a, 3}",
                               "m = {A, (0, 0)}", f"p = {rhs}"],
         [setn("a", 3), setn("a", 1), setn("a", 2)])

# ------------------------------------------------------- PointList
topic("pointlist")
for rhs in ("PointList(l)", "PointList({{1, 2}, {3, 4}})", "PointList({1, 2, 3, 4, 5})",
            "PointList({{1, 2}, {3}, {1, 2}})", "PointList({{1, 2}, {3, 4}, {5}})",
            "PointList(1, 2)", "PointList(A, (3, 4))", "PointList({})", "PointList({{a, 2}})",
            "Length(PointList(l))", "PointList({{1 / 0, 2}, {3, 4}})"):
    case("pointlist", nums(a=1) + [pt("A", 1, 2), "l = {{a, 2}, {3, 4}}", f"p = {rhs}"],
         [setn("a", 4), setn("a", 1), mv("A", 5, 5), mv("A", 1, 2)])

# ------------------------------------------------------- Zip
# `AlgoZip`: the expression at the elements the lists share, as many as the
# shortest list holds.
topic("zip")
for rhs in ("Zip(t^2, t, l)", "Zip(t, t, l)", "Zip(t + u, t, l, u, k)",
            "Zip(u + t, u, l, t, k)", "Zip(t, t, l, u, {3})", "Zip(2 * P, P, m)",
            "Zip(P + V, P, m, V, w)", "Zip(x(P), P, m)", "Zip((t, t + 1), t, l)",
            "Zip({t, t + 1}, t, l)", "Zip(t^2, t, Sequence(n))", "Zip(t, t, {})",
            "Zip(t + u, t, l, u, {})", "Sum(Zip(t^2, t, l))", "Zip(t, t, q)",
            "Zip(Zip(t + u, t, k), u, l)"):
    case("zip", nums(n=4) + [pt("A", 1, 2), pt("B", 2, 1), "v = (1, 1)",
                             "l = {1, 2, 3}", "k = {4, 5, 6}", "m = {A, B}",
                             "w = {v, (0, 2)}", "q = {{1}, {2}}", f"p = {rhs}"],
         [setn("n", 2), setn("n", 5), mv("A", 3, 3), mv("B", 4, 4), setn("n", 4),
          mv("A", 1, 2), mv("B", 2, 1)])
# a `Zip` earlier in a list of expressions must not retype the local of a
# `Sequence` later in it: the binding of `f` ends with its `Zip`
case("zip", nums(n=4) + [pt("A", 1, 2), pt("B", 2, 1), "v = (1, 1)",
                         "l = {1, 2, 3}", "k = {4, 5, 6}", "m = {A, B}",
                         "w = {v, (0, 2)}", "q = {{1}, {2}}",
                         "p = {Zip(2 * f, f, {(1, 1)}), Sequence(g^2, g, 1, 3)}"],
     [setn("n", 2), setn("n", 5), mv("A", 3, 3), mv("B", 4, 4), setn("n", 4),
      mv("A", 1, 2), mv("B", 2, 1)])

# ------------------------------------------------------- Product of an expression
# `AlgoFoldExpression`: the expression at `from` and at every whole number
# up to `to`.
topic("fold")
for rhs in ("Product(k^2, k, 1, 4)", "Product(k^2, k, a, b)", "Product(k^2, k, 3, 1)",
            "Product(k^2, k, 1.5, 3.8)", "Product(k, k, 1, 0)",
            "Product(2, k, a, b)", "Product(1 / k, k, 1, b)"):
    case("fold", nums(a=1, b=4) + [f"c = {rhs}"],
         [setn("a", 2), setn("b", 6), setn("a", 5), setn("b", 1), setn("a", 1), setn("b", 4)])

# ------------------------------------------------------- Product with frequencies
topic("prodfreq")
for rhs in ("Product(l, k)", "Product({2, 3}, {3, 2})", "Product({2, 3, 4}, {1, 2})", "Product({2, 3, 4}, {-1})", "Product({2, 3, 4}, {t, 2})"):
    case("prodfreq", nums(t=1) + ["l = {2, 3, 4}", "k = {2, 2}", f"c = {rhs}"],
         [setn("t", 3), setn("t", -1), setn("t", 1)])

# ------------------------------------------------------- First of a locus
topic("locusfirst")
for rhs in ("First(c, 2)", "First(c, n)", "First(c, 1)", "First(c, 0)", "First(c, 9)",
            "Length(First(c, n))"):
    case("locusfirst", nums(n=2) + [pt("A", 0, 0), pt("B", 2, 0), "s = Segment(A, B)",
                                    "P = Point(s)", "Q = P + (1, 1)", "c = Locus(Q, P)",
                                    f"p = {rhs}"],
         [setn("n", 1), setn("n", 5), setn("n", 0), mv("A", 1, 0), setn("n", 2), mv("A", 0, 0)])

# ------------------------------------------------------- Element of nested lists
topic("element2")
for rhs in ("Element(m, 1, 2)", "Element(m, 2, 1)", "Element(m, 1, 3)", "Element(m, 2)",
            "Element(m, n, 2)", "Element(m, 1, n)", "Element(q, 1, 1)", "Element(m, n)"):
    case("element2", nums(n=1) + ["m = {{1, 2}, {3, 4}}", "q = {{5}}", f"c = {rhs}"],
         [setn("n", 2), setn("n", 3), setn("n", 0), setn("n", 1)])

# ------------------------------------------------------- commands on text lists
# `AlgoSort` orders texts by `GeoText.getComparator` (accents folded away,
# then the raw strings), `AlgoUnique` and `AlgoFrequency` count equal
# strings, and the element and set commands read texts like any other
# kind.
topic("textlists")
for rhs in ("Sort(l)", "Sort(m)", "Sort({})", "Sort({\"b\", \"a\"})",
            "Sort({\"10\", \"9\", \"100\"})", "Sort({\"b\", 1})", "Sort({true, \"b\"})",
            "Unique(l)", "Unique({\"b\", \"a\", \"b\", \"B\"})", "Unique({})",
            "Unique({\"é\", \"e\"})", "Frequency(l)",
            "Frequency({\"b\", \"a\", \"b\", \"B\"})", "Reverse(l)", "First(l)",
            "First(l, n)", "Last(l, n)", "Take(l, 2, 3)", "Take(l, n, 4)",
            "Element(l, n)", "Element(l, 0)", "IndexOf(\"a\", l)", "IndexOf(\"b\", l, 3)",
            "IndexOf(\"x\", l)", "Append(l, \"z\")", "Append(\"z\", l)",
            "Insert(\"z\", l, 2)", "Insert(\"z\", l, 0)", "Remove(l, {\"b\"})",
            "Remove(l, k)", "Remove(l, {\"x\"})", "Union(l, k)", "Union(k, l)",
            "Intersection(l, k)", "Intersection(k, l)", "Length(Unique(l))"):
    case("textlists", nums(n=2) + ["l = {\"b\", \"a\", \"b\", \"c\", \"A\"}",
                                   "k = {\"a\", \"d\"}",
                                   "m = {\"é\", \"e\", \"È\", \"Z\", \"a\"}", f"p = {rhs}"],
         [setn("n", 0), setn("n", 1), setn("n", 5), setn("n", 3), setn("n", -1), setn("n", 2)])

# ------------------------------------------------------- First, Last, Element of a text
# The string forms of the element commands: a count of UTF-16 units from
# either end, one unit at an index (`AlgoFirstString`, `AlgoLastString`,
# `AlgoTextElement`), all `(int)` casts of their counts and indices.
topic("textelem")
for rhs in ("First(\"hello\")", "First(\"hello\", n)", "First(\"\", 2)", "First(\"hello\", 0)",
            "First(\"hello\", 9)", "First(\"hello\", 1.9)", "Last(\"hello\")", "Last(\"hello\", n)",
            "Last(\"hello\", 0)", "Last(\"hello\", 9)", "Last(\"héllo\", 2)",
            "Last(\"😀ab\", 2)", "Element(\"hello\", n)", "Element(\"hello\", 0)",
            "Element(\"hello\", 9)", "Element(\"hello\", 2.9)", "Element(\"héllo\", 2)",
            "Element(\"😀ab\", 3)", "First(\"😀ab\", 3)"):
    case("textelem", nums(n=2) + [f"p = {rhs}"],
         [setn("n", 0), setn("n", 1), setn("n", 5), setn("n", 6), setn("n", -1), setn("n", 2)])

# The members enclosed by the Create List tool's selection rectangle.
case("canvas", [
    pt("A", 0, 0), pt("B", 2, 2), pt("C", 5, 1),
    "l1 = {A, B}",
], [mv("A", 1, 1)])

cases.write()
