"""Remaining forms of the function group and single-form leftovers of other
groups: `Function()`, `Function(list)` and the commands added after them,
one topic per command form."""

from caselib import Cases, mv, setn

cases = Cases("funcrest")


def case(topic, script, steps=()):
    cases.case(topic, script.split("\n"), list(steps))


def reject(line):
    return {"action": "reject", "line": line}


# ------------------------------------------------------------- Function()
# The empty data function: defined, without a value anywhere, and usable
# inside other expressions.
cases.topic("emptyfunction")
case("emptyfunction", "f = Function()")
case("emptyfunction", "f = Function()\na = f(1)\ng(x) = f(x) + 1")
case("emptyfunction", "f = Function()\nd = Derivative(f)\ni = Integral(f, 0, 1)")

# --------------------------------------------------------- Function(list)
# The freehand function of `{min, max, y1, y2, ...}`: linear between the
# values at equal steps, undefined outside `[min, max]`.
cases.topic("freehand")
case("freehand", "f = Function({1, 2, 3, 4, 5})")
case("freehand", "l = {1, 2, 3, 1, 5, 7, 9}\nf = Function(l)")
case("freehand", "f = Function({1, 2, 3, 4, 5})\na = f(1.3)\nb = f(2)\nc = f(2.5)")
case("freehand", "f = Function({-2, 4, 1, 5, 2})\nh(x) = 2 f(x)")
# the interval follows a number: reversed, empty and wide again
case("freehand", "n = 4\nl = {0, n, 1, 2, 4, 8}\nf = Function(l)\na = f(1)\nb = f(n)",
     [setn("n", -1), setn("n", 0), setn("n", 10), setn("n", 0.5)])
# fewer than four numbers, an empty list and a list of points are undefined
case("freehand", "f = Function({1, 2, 3})\ng = Function({})\nh = Function({(1, 2), (3, 4), (5, 6), (7, 8)})")
case("freehand", "f = Function({1, 2, 3, (1, 2), 4})")
# the list grows past four elements and shrinks again
case("freehand", "n = 2\nl = Sequence(k^2, k, 0, n)\nf = Function(l)",
     [setn("n", 5), setn("n", 3), setn("n", 1), setn("n", 4)])
# an element becomes undefined and defined again
case("freehand", "a = 1\nl = {-8, 8, 1, 1 / a, 3, 2}\nf = Function(l)", [setn("a", 0), setn("a", 2)])
# what reads a freehand function
case("freehand", "f = Function({-2, 4, 1, 5, 2})\ng = Derivative(f)\ni = Integral(f, 0, 1)")
# the integral sums the steps between the bounds, in either direction, and
# is undefined past the interval
case("freehand", "a = -2\nb = 4\nf = Function({-2, 4, 1, 5, 2, 0, 3})\ni = Integral(f, a, b)",
     [setn("a", -1.5), setn("b", 3.25), setn("a", 3.5), setn("b", 5), setn("b", 1), setn("a", -3),
      setn("a", 0.2), setn("b", 0.3)])
# a number, a function and a second argument are errors
case("freehand", "l = {1, 2, 3, 4}\nf(x) = x",
     [reject("g = Function(3)"), reject("g = Function(f)"), reject("g = Function(l, 2)"),
      reject("g = Function(1, 2)")])

# what else reads a freehand function: it is sampled like any function
cases.topic("freehandreaders")
case("freehandreaders", "f = Function({-2, 4, 1, 5, 2})\nh(x) = 2 f(x)\ni = Integral(h, 0, 1)")
case("freehandreaders", "f = Function({-2, 4, 1, 5, 2})\nl = Length(f, 0, 1)")
case("freehandreaders", "f = Function({-2, 4, 1, 5, 2})\nt = Tangent(0, f)")
case("freehandreaders", "f = Function({-2, 4, 1, 5, 2})\nM = Max(f, 0, 3)")
case("freehandreaders", "f = Function({-2, 4, 1, 5, 2})\nr = Root(f, 0, 1)")
case("freehandreaders", "f = Function({-2, 4, 1, 5, 2})\ni = Integral(f)")
case("freehandreaders", "f = Function({-2, 4, 1, 5, 2})\ni = NIntegral(f, 0, 1)")
case("freehandreaders",
     "f = Function({-2, 4, 1, 5, 2})\ng(x) = 1\ni = IntegralBetween(f, g, 0, 1)")
case("freehandreaders", "f = Function({-2, 4, 1, 5, 2})\nA = (1, f(1))\nB = (0.5, f(0.5))")
# the numeric intersection with a freehand function (a known gap)
case("freehandreaders", "f = Function({-2, 4, 1, 5, 2})\ng(x) = x\nS = Intersect(f, g, 0, 3)")

cases.write()
