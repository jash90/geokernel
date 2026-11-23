"""Remaining forms of the function group and single-form leftovers of other
groups: `Function()`, `Function(list)` and the commands added after them,
one topic per command form."""

import random

from caselib import Cases, mv, setn

cases = Cases("funcrest")


def case(topic, script, steps=()):
    cases.case(topic, script.split("\n"), list(steps))


def reject(line):
    return {"action": "reject", "line": line}


def bounds(label, lo, hi):
    return {"action": "set_slider_bounds", "label": label, "min": float(lo), "max": float(hi)}


def anim(frames):
    return {"action": "animate", "frames": frames}


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

# --------------------------------------------------------------- Normalize
cases.topic("normalize")
case("normalize", "l = {1, 3, 2}\nr = Normalize(l)\ns = Normalize({(1, 1), (3, 1), (2, 1)})")
case("normalize", "A = (1, -2)\nB = (-3, 4)\nC = (2, 0)\nr = Normalize(A)\ns = Normalize(A, B, C)",
     [mv("A", 2, 4), mv("B", 2, 4)])
case("normalize", "n = 1\nA = (1/n, n)\nB = (0, 0)\nr = Normalize(A, B)\ns = Normalize({n, 1/n, 0})",
     [setn("n", 0), setn("n", -2)])
case("normalize", "r = Normalize({})\ns = Normalize({0, 0})\nt = Normalize({-3})\nu = Normalize({true, false})")
case("normalize", "n = 2\nl = Sequence(k, k, 1, n)\nr = Normalize(l)",
     [setn("n", 0), setn("n", 4)])
case("normalize", "r = Normalize({1, (2, 2), 4})\ns = Normalize({1, true})\nt = Normalize({1, false, 2})")
case("normalize", "A = (1, 2)", [reject("r = Normalize()"), reject("r = Normalize(1)"),
                                 reject("r = Normalize(A, 2)")])
case("normalize", 'r = Normalize({1, 2, true})\ns = Normalize({true, 1})\n'
     'u = Normalize({1, "a"})', [reject("t = Normalize({1, (2, 2)})")])
case("normalize", "r = Normalize({})\ns = Normalize({4, 4, 4})",
     [reject("t = Normalize({(1, 2), 3, (4, 5)})")])

# --------------------------------------------------------------------- SVD
cases.topic("svd")
case("svd", "s = SVD({{1}})\nt = SVD({{-2}})\nu = SVD({{0}})")
case("svd", "s = SVD({{3, 0}, {0, 2}})\nt = SVD({{1, 0}, {0, 4}})")
case("svd", "s = SVD({{3, 4}})\nt = SVD({{3}, {4}})")
case("svd", "s = SVD({{1, 2}, {3, 4}})\nt = SVD({{1, 2, 3}, {4, 5, 6}})")
case("svd", "s = SVD({{1, 2}, {2, 4}, {3, 6}})\nt = SVD({{1, 0}, {0, 1}})\nu = SVD({{0, 0}, {0, 0}})")
case("svd", "n = 2\nl = Sequence({k, 0}, k, 1, n)\ns = SVD(l)",
     [setn("n", 0), setn("n", 1), setn("n", 3)])
case("svd", "s = SVD({})\nt = SVD({{}, {}})\nu = SVD({{1, 2}, {3}})\nv = SVD({1, 2})",
     [reject("r = SVD(1)"), reject("r = SVD({{1}}, {{2}})")])
case("svd", "n = 2\nl = {{1, 2}, Sequence(k + 2, k, 1, n)}\ns = SVD(l)",
     [setn("n", 1), setn("n", 2)])
case("svd", "s = SVD({{1/0}})\nt = SVD({{0/0}})")
case("svd", "s = SVD({{1, 2, 3}, {4, 5, 6}, {7, 8, 9}})")
case("svd", "d = IsDefined(SVD(Sequence(Sequence(sin(i*j), j, 1, 200), i, 1, 200)))")
# Random, rank-deficient and decimal matrices of every shape pin down the
# signs and null-space vectors of the decomposition.
_svd_rng = random.Random("funcrest-svd-random")
_svd_lines = []
for _rows, _cols in [(2, 2), (3, 3), (3, 2), (2, 3), (4, 4), (4, 2), (2, 4), (5, 3), (3, 5),
                     (6, 6), (1, 4), (4, 1)]:
    _mats = [[[str(_svd_rng.randint(-9, 9)) for _ in range(_cols)] for _ in range(_rows)]
             for _ in range(4)]
    _base = [_svd_rng.randint(-5, 5) for _ in range(_cols)]
    _mats.append([[str(_base[j] * (i + 1)) for j in range(_cols)] for i in range(_rows)])
    _mats.append([[str(round(_svd_rng.uniform(-3, 3), 3)) for _ in range(_cols)]
                  for _ in range(_rows)])
    for _m in _mats:
        _rows_text = ", ".join("{" + ", ".join(r) + "}" for r in _m)
        _svd_lines.append("m%d = SVD({%s})" % (len(_svd_lines), _rows_text))
for _k in range(0, len(_svd_lines), 12):
    case("svd", "\n".join(_svd_lines[_k:_k + 12]))
# NaN and infinite entries are decomposed; a row of another length is not
# a matrix
case("svd", "a = SVD({{1/0, 1}, {2, 3}})\nb = SVD({{0/0, 1}, {2, 3}})\nd = SVD({{1, 2}, {3, 1/0}})\n"
     "e = SVD({{-1/0}})\nf = SVD({{1/0, 2}})\ng = SVD({{1/0}, {2}})\nh = SVD({{0, 0, 0}, {0, 0, 0}})\n"
     "i = SVD({{10^300, 0}, {0, 10^(-300)}})\nj = SVD({{0, 1}, {1, 0}})\nk = SVD({{1, 2}, {3, 4, 5}})\n"
     "p = SVD({{0/0, 1}})\nq = SVD({{1}, {0/0}})\nr = SVD({{0/0}, {1}})")
# an entry that becomes infinite or NaN does not stick; an input that is no
# matrix does
case("svd", "n = 1\nl = {{n / (n - 1)}}\ns = SVD(l)", [setn("n", 0), setn("n", 2)])
case("svd", "n = 2\nl = {{sqrt(n - 3), 2}, {3, 4}}\ns = SVD(l)", [setn("n", 4), setn("n", 2), setn("n", 7)])
case("svd", "n = 2\nl = If(n > 0, {{1, 2}, {3, 4}}, {1, 2})\ns = SVD(l)", [setn("n", -1), setn("n", 1)])
case("svd", "n = 2\nl = If(n > 0, {{1, 2}, {3, 4}})\ns = SVD(l)", [setn("n", -1), setn("n", 1)])
case("svd", "n = 2\nl = {{1, 2}, Sequence(k, k, 1, n)}\ns = SVD(l)", [setn("n", 3), setn("n", 2)])
# a constant list whose rows hold only undefined numbers is undefined
# (`AlgebraProcessor.processList`); a dependent one is decomposed
case("svd", "a = SVD({{sqrt(-1)}})\nb = SVD({{0/0, 0/0}, {0/0, 0/0}})\nn = -1\nd = SVD({{sqrt(n)}})\n"
     "e = SVD({{0/0, 0/0}, {0/0, 1}})", [setn("n", 4), setn("n", -1)])

# ------------------------------------------------------------- IsVertexForm
cases.topic("vertexform")
case("vertexform", "f(x) = 4(x + 1)^2 + 3\na = IsVertexForm(f)\nb = IsVertexForm(4(-3/7 + x)^2 + 3/7 + sqrt(2))\nc = IsVertexForm(4(x - 3/7)^2 + 3/7 + sqrt(2))")
case("vertexform", "a = IsVertexForm(x^2)\nb = IsVertexForm(4(-x - 3/7)^2 + 3/7 + sqrt(2))\nc = IsVertexForm((2x + 2)^2 + 3)")
case("vertexform", "a = IsVertexForm((1 - x)^2)\nb = IsVertexForm(x^2 / 3)\nc = IsVertexForm(x^2 + x)\nd = IsVertexForm((x + 1)^(1 + 1))")
case("vertexform", "n = 2\nf(x) = (x+1)^n\ng(x) = n*(x+1)^2\nh(x) = 4*(x+1)^2 + n\na = IsVertexForm(f)\nb = IsVertexForm(g)\nc = IsVertexForm(h)",
     [setn("n", 0), setn("n", 2)])
case("vertexform", "n = 1\nf(x) = (x+1)^2 + 0/n\ng = IsVertexForm(f)\nh = IsVertexForm(Function())",
     [setn("n", 0), setn("n", 2), reject("a = IsVertexForm()"), reject("a = IsVertexForm({1,2})")])
case("vertexform", "l = Line((0, 0), (1, 1))\nv = Line((0, 0), (0, 1))\n"
     "a = IsVertexForm(3)\nb = IsVertexForm(l)\nd = IsVertexForm(v)",
     [reject("s = IsVertexForm(Vector((1, 1)))")])
case("vertexform", "F = (0, 1)\nl: y = -1\nc = Parabola(F, l)\ne = Circle((0, 0), 2)\n"
     "a = IsVertexForm(c)\nb = IsVertexForm(e)", [mv("F", 0, -1), mv("F", 0, 2)])
case("vertexform", "O = (0, 0)\nA = (1, 0)\nB = (0, 1)\ns = Segment(O, A)\nr = Ray(O, A)\n"
     "c = CircularArc(O, A, B)\nd = CircularSector(O, A, B)\nn = 30°\na = IsVertexForm(s)\n"
     "b = IsVertexForm(r)\ne = IsVertexForm(c)\nf = IsVertexForm(d)\ng = IsVertexForm(n)")

# ---------------------------------------------------------------------- If
cases.topic("conditional")
case("conditional",
     "n = 1\nA = (0, 0)\nB = (4, 0)\nC = (0, 3)\nr = Segment(A, B)\nb = n > 0\ns = If(b, r, Segment(A, C))\nd = Length(s)",
     [setn("n", 0), mv("C", 0, 2), setn("n", 1)])
case("conditional",
     "n = 1\nA = (0, 0)\nB = (4, 0)\nC = (0, 3)\ns = If(n > 0, Line(A, B), n < 0, Line(A, C))",
     [setn("n", 0), setn("n", -1), mv("C", 3, 3)])
case("conditional",
     "n = 1\nA = (3, 0)\nB = (0, 3)\nO = (0, 0)\na = CircularArc(O, A, B)\nb = CircularArc(O, B, A)\nc = If(n > 0, a, b)\nd = Length(c)\nP = Point(c, 0.5)",
     [setn("n", -1), mv("A", 0, 0), mv("A", 3, 0), setn("n", 1)])
case("conditional",
     "n = 1\nA = (0, 0)\nB = (4, 0)\nC = (0, 3)\nD = (0, -3)\np = Polygon(A, B, C)\nq = Polygon(A, B, D)\nr = If(n > 0, p, n < 0, q)\naa = Area(r)",
     [setn("n", 0), setn("n", -1), mv("D", 0, -5), setn("n", 1)])
case("conditional",
     "n = 1\np = Curve(t, t^2, t, 0, 3)\nq = Curve(t, -t^2, t, 0, 3)\nr = If(n > 0, p, q)\nP = Point(r, 0.5)",
     [setn("n", -1), setn("n", 1)])
case("conditional",
     "n = 1\nb = n > 0\na = If(b, true, 5)\nP = If(b, 3, (2, 1))\nq = If(b, {1, 2}, 3)",
     [setn("n", 0), setn("n", 1)])
# The original case 007 moved to the reference-only mixed geometry topic.
cases.counters["conditional"] += 1
case("conditional",
     "a = 2\nt = true\nl = {1, 2}\np = If(a < 3, t, 2)\nq = If(a < 3, 3, a < 4)\nr = If(a < 3, l, 1)",
     [setn("a", 3), setn("a", 4), setn("a", 2)])
case("conditional",
     'n = 1\nb = n > 0\na = If(b, 3, {1, 2})\nP = If(b, (1, 2), 3)\n'
     'c = If(b, 30°, 5)\nd = If(b, 5, 30°)\ne = If(b, "a", 3)\nh = If(b, 3, "a")',
     [setn("n", 0), setn("n", 1)])
case("conditional", "n = 1\na = If(n > 0, 30°, n < 0, 5, 60°)\n"
     "P = If(n > 0, 3, n < 0, {1, 2}, 5)\nQ = If(n > 0, (1, 2), n < 0, 3, (3, 4))",
     [setn("n", -1), setn("n", 0), setn("n", 1)])

# ---------------------------------------------------------------- SlowPlot
cases.topic("slowplot")
case("slowplot", "f(x) = sin(x)\ng = SlowPlot(f)\nu = g(-8)\nv = g(0)")
case("slowplot", "n = 1\nf(x) = n*x\ng = SlowPlot(f, 0)\nh = SlowPlot(f, n + 1)\nu = h(-8)",
     [setn("n", 0), setn("n", -2)])
case("slowplot", "n = 1\nf(x) = x/n\ng = SlowPlot(f)\nu = g(-8)",
     [setn("n", 0), setn("n", 2), reject("s = SlowPlot(3)")])
case("slowplot", "f(x) = sin(x)\ng = SlowPlot(f)\na = 2\nu = g(0)\nv = g(8)")
case("slowplot", "a = 1\nf(x) = sin(x)\ng = SlowPlot(f)\nv = g(0)\nw = g(8)",
     [setn("b", 0.5), setn("b", 1), setn("b", 0)])
case("slowplot", "A = (0, 0)\nB = (2, 2)\nl = Line(A, B)\ng = SlowPlot(l)\n"
     "h = SlowPlot(Line(A, B), 0)\nu = g(-8)\nv = h(-8)",
     [mv("B", 0, 2), mv("B", 2, 4)])
# The slider plays: one frame is a 300th of the interval, rounded to the
# automatic increment 0.001 of a new number while it plays (the stored step
# 0.01 is not used). With the repeat flag it jumps back to 0 past the end, without it
# it stops at 1; a value set by hand is where it goes on from.
case("slowplot", "f(x) = sin(x)\ng = SlowPlot(f)\np = a + 0\nu = g(-4)\nv = g(4)",
     [anim(1), anim(1), anim(150), anim(148), anim(2), anim(1), anim(5)])
case("slowplot", "f(x) = sin(x)\ng = SlowPlot(f, false)\np = a + 0\nv = g(4)",
     [anim(299), anim(1), anim(2), anim(5), setn("a", 0.5), anim(5)])
case("slowplot", "f(x) = x\ng = SlowPlot(f, 1.7)\nh = SlowPlot(f, 0.9)\nk = SlowPlot(f, -1)\n"
     "q = SlowPlot(f, 0 / 0)\np = a + 0\nr = b + 0\ns = c + 0\nt = d + 0",
     [anim(310), anim(1)])
case("slowplot", "f(x) = sin(x)\ng = SlowPlot(f)\np = a + 0",
     [anim(20), setn("a", 0.5), anim(3), setn("a", 0.995), anim(3)])
case("slowplot", "A = (0, 0)\nB = (2, 2)\nl = Line(A, B)\ng = SlowPlot(l, false)\np = a + 0\nu = g(0)",
     [anim(100), mv("B", 0, 2), anim(1), mv("B", 2, 4), anim(250)])
case("slowplot", "n = 1\nf(x) = x / n\ng = SlowPlot(f)\np = a + 0\nu = g(-4)",
     [anim(30), setn("n", 0), anim(30), setn("n", 2), anim(30)])
case("slowplot", "A = (0, 0)\nB = (2, 2)\nl = Line(A, B)\ng = SlowPlot(l, 1)\np = a + 0\nu = g(0)",
     [anim(160), mv("B", 2, 0), anim(160)])
# A reversed interval makes the slider undefined; a frame then sets it by
# the ordered tests of the interval, and the interval recovers.
case("slowplot", "f(x) = x\ng = SlowPlot(f)\np = a + 0\nu = g(-4)",
     [anim(10), bounds("a", 1, 0), anim(1), anim(3), bounds("a", 0, 1), anim(5)])
# The repeat value of an angle is its radians cut to an integer.
case("slowplot", "f(x) = x\nr = 90°\ng = SlowPlot(f, r)\nh = SlowPlot(f, 90°)\n"
     "k = SlowPlot(f, 2)\np = a + 0\nq = b + 0\ns = c + 0",
     [anim(310), anim(1)])

# ---------------------------------------------------------------- Derivative
case("derivative", "n = 0\nk = n/n\nf(x) = x^3 + 1\ng = Derivative(f, k)\nh = NDerivative(f, k)\n"
     "p = Derivative(f, 2147483648)\nq = Derivative(f, 4294967296)\n"
     "r = Derivative(f, -1.5)\ns = Derivative(f, -0.5)", [setn("n", 1), setn("n", 0)])
case("derivative", "n = 0\nk = n/n\nc = Curve(t, t^2, t, 0, 1)\nd = Derivative(c, k)\n"
     "e = NDerivative(c, t, k)\np = Derivative(c, 2147483648)", [setn("n", 1), setn("n", 0)])
cases.cases[-1]["sample_curves"] = True
case("derivative", "f(x) = x^3\nd = Derivative(Curve(t, t^2, t, 0, 1))\n"
     "e = NDerivative(Curve(t^3, t^2, t, 0, 1), t, 2)\nh = Derivative(f, pi)",
     [reject("g = Derivative(f, pi, 2)"),
      reject("g = Derivative(Curve(t, t^2, t, 0, 1), pi, 2)")])
cases.cases[-1]["sample_curves"] = True
case("derivative", "n = 5\ns = Spline(Sequence((k, sin(k)), k, 1, n))\nd = Derivative(s)\ne = NDerivative(s)",
     [setn("n", 260), setn("n", 5)])
cases.cases[-1]["sample_curves"] = True

# ------------------------------------------------------------ long splines
# Splines over exact inputs, so the long systems compare the elimination
# itself: an open spline growing to 260 points and back, a closed one, a
# quartic one that becomes undefined with too few points, and coincident
# points.
cases.topic("splinelong")
case("splinelong", "n = 5\ns = Spline(Sequence((k, Mod(k^2, 7)), k, 1, n))\nd = Derivative(s)\ne = NDerivative(s)",
     [setn("n", 260), setn("n", 5)])
cases.cases[-1]["sample_curves"] = True
case("splinelong", "n = 6\nl = Append(Sequence((k, Mod(k, 3)), k, 0, n), (0, 0))\ns = Spline(l)\nd = Derivative(s)",
     [setn("n", 200), setn("n", 6)])
cases.cases[-1]["sample_curves"] = True
case("splinelong", "n = 6\ns = Spline(Sequence((k, Mod(k, 2)), k, 1, n), 4)\nd = Derivative(s)",
     [setn("n", 120), setn("n", 3), setn("n", 6)])
cases.cases[-1]["sample_curves"] = True
case("splinelong", "A = (1, 1)\nl = {(0, 0), A, (3, 0), (4, 2)}\ns = Spline(l)\nd = Derivative(s)\ne = NDerivative(s)",
     [mv("A", 0, 0), mv("A", 3, 0), mv("A", 1, 1)])
cases.cases[-1]["sample_curves"] = True
# The degree is checked against the number of points of the last solve
# that passed the check (the initial list at creation).
case("splinelong", "n = 5\ns = Spline(Sequence((k, Mod(k^2, 7)), k, 1, n))\nd = Derivative(s)",
     [setn("n", 2), setn("n", 5), setn("n", 4)])
cases.cases[-1]["sample_curves"] = True
case("splinelong", "n = 2\ns = Spline(Sequence((k, k^2), k, 1, n))",
     [setn("n", 3), setn("n", 6)])
cases.cases[-1]["sample_curves"] = True
case("splinelong", "n = 6\ns = Spline(Sequence((k, Mod(k, 2)), k, 1, n), 4)\nd = Derivative(s)",
     [setn("n", 3), setn("n", 6), setn("n", 4)])
cases.cases[-1]["sample_curves"] = True
case("splinelong", "n = 4\ns = Spline(Sequence((k, Mod(k, 3)), k, 1, n))\nd = Derivative(s)\ne = NDerivative(s)",
     [setn("n", 3), setn("n", 2), setn("n", 1), setn("n", 4), setn("n", 0)])
cases.cases[-1]["sample_curves"] = True
# A locus over the slider samples a copy: the live spline keeps its own
# remembered point count.
case("splinelong", "a = Slider(2, 4)\ns = Spline(Sequence((k, k^2), k, 1, 6 - a))\nP = Point(s, 0.5)\nO = (0, 0)\nQ = Midpoint(P, O)\nl = Locus(Q, a)",
     [setn("a", 3), setn("a", 4), setn("a", 2.5), setn("a", 2)])
cases.cases[-1]["sample_curves"] = True
# High degrees: the end conditions use factorials computed in 32-bit
# integers, which wrap from 13! on.
case("splinelong", "n = 13\nl0 = Sequence((k, Mod(k, 2)), k, 1, n)\ns = Spline(l0, 13)\nt = Spline(l0, 12)\nu = Spline(l0, 9)",
     [setn("n", 14), setn("n", 13)])
cases.cases[-1]["sample_curves"] = True

# ------------------------------------------------------ First(function, n)
# The freehand function as a locus: one point per value of its list, at
# min + step * i, with the function's value there.
cases.topic("freehandpoints")
case("freehandpoints", "n = 2\nf = Function({-2, 4, 1, 5, 2})\nl = First(f, n)",
     [setn("n", 4), setn("n", 1)])
case("freehandpoints", "n = 3\nf = Function({0, 1, 1, 5, 2, 7})\nl = First(f, n)\nm = First(f, 4)\no = First(f, 0)",
     [setn("n", 5), setn("n", -1), setn("n", 2.7), setn("n", 0)])
case("freehandpoints", "a = 0.1\nf = Function({a, 0.7, 1, 5, 2, 7})\nl = First(f, 4)\nm = First(f, 3)",
     [setn("a", 1), setn("a", 0.7), setn("a", 0.3), setn("a", 0.1)])
case("freehandpoints", "n = 4\nl0 = Sequence(k, k, 1, n)\nf = Function(l0)\nl = First(f, 1)\nm = First(f, 2)",
     [setn("n", 3), setn("n", 5), setn("n", 4)])
case("freehandpoints", "f = Function({-1, 1, 0, 1/0, 2})\nl = First(f, 3)\ng(x) = f(x) + 1",
     [reject("m = First(g, 1)"), reject("m = First(x^2, 1)")])
case("freehandpoints", "n = 2\nl = First(Function({-2, 4, 1, 5, 2}), n)\nm = First(Function({0, 1, n, 2, 3}), 3)",
     [setn("n", 3), setn("n", 4), setn("n", 2)])
case("freehandpoints", "l = First(Function({-1, 1, 0, 1/0, 2}), 3)\nm = First(Function({-1, 1, 0, 0/0, 2}), 3)")

cases.write()

cases = Cases("funcrest")
case("conditionalmixed",
     "n = 1\nA = (0, 0)\nB = (4, 0)\nC = (0, 3)\nr = Segment(A, B)\ng = Line(A, C)\ns = If(n > 0, r, g)",
     [setn("n", -1), setn("n", 1)])
cases.write("reference")
