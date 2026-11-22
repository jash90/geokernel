#!/usr/bin/env python3
"""Writes cases/infinity.json: numbers that are infinite in GeoGebra.

GeoGebra keeps a number whose value is `Infinity` or `-Infinity` defined
(`GeoNumeric.isDefined` only asks for a value that is not NaN). Schema 3
records which infinity such a number holds, so these cases check the sign
and tell an infinite number from an undefined one:

- `arith`: the constant arithmetic of GeoGebra's own `ArithmeticTest`
  (powers, products, sums, differences, quotients and functions of `inf`);
- `dynamic`: numbers that become infinite and finite again when a free
  number moves through zero, and what depends on them;
- `lists`: infinite elements and the list commands that read them;
- `inputs`: commands whose number argument is infinite;
- `outputs`: commands and function calls whose result is infinite.
"""

import json
from pathlib import Path

from caselib import Cases, mv, num, pt, setn

cases = Cases("infinity")
case = cases.case
topic = cases.topic

# ------------------------------------------------------- constant arithmetic
topic("arith")
ARITH = (
    "0^inf", "1^inf", "1^(-inf)", "2^inf", "2.1^inf", "(-1)^inf", "inf^inf", "?^inf", "inf^?",
    "(-inf)^inf", "inf^(-inf)", "(-inf)^(-inf)", "inf^0", "inf^(-1)", "1/inf", "0/inf",
    "1*inf", "2*inf", "inf*1", "inf*-1", "0*inf", "inf*0", "inf+1", "inf+0", "inf-0", "inf-1",
    "1-inf", "1+inf", "0+inf", "sin(inf)", "cos(inf)", "tan(inf)", "ln(inf)", "exp(inf)",
    "sin(-inf)", "cos(-inf)", "tan(-inf)", "ln(-inf)", "exp(-inf)", "inf * inf", "inf * -inf",
    "-inf * inf", "-inf * -inf", "inf + inf", "inf + -inf", "-inf + inf", "-inf + -inf",
    "inf - inf", "inf - -inf", "-inf - inf", "-inf - -inf", "inf / inf", "inf / -inf",
    "-inf / inf", "-inf / -inf", "inf * ?",
    "inf", "-inf", "1/0", "-1/0", "0/0", "2/0 + 1", "-(1/0)", "abs(-inf)", "sqrt(inf)",
    "sqrt(-inf)", "floor(inf)", "ceil(-inf)", "round(inf)", "sgn(-inf)", "sgn(inf)",
    "inf^2", "(-inf)^2", "(-inf)^3", "inf^0.5", "2^(-inf)", "0.5^inf", "0.5^(-inf)",
    "ln(0)", "log(10, 0)", "exp(1000)", "-exp(1000)", "1E308 * 10", "-1E308 * 10",
    "atan(inf)", "atan(-inf)", "tanh(inf)", "cosh(inf)", "sinh(-inf)", "cbrt(-inf)",
    "inf!", "arctan2(inf, 1)", "inf == inf", "inf > 1E300", "-inf < 0", "inf == -inf", "inf != inf",
    "1/0 == inf", "inf - inf == 0", "inf >= inf",
)
for rhs in ARITH:
    case("arith", [f"c = {rhs}"], [])

# functions with a pole or an overflow: which of them answer an infinity,
# and of which sign, and which are undefined
topic("functions")
for rhs in ("cot(0)", "csc(0)", "cosec(0)", "sec(pi / 2)", "coth(0)", "csch(0)", "sech(1000)",
            "tan(pi / 2)", "1 / sin(0)", "1 / cos(pi / 2)", "1 / tan(0)", "-cot(0)", "cot(-0.0)",
            "gamma(0)", "gamma(-1)", "gamma(-2)", "gamma(171)", "gamma(172)", "gamma(-0.5)",
            "170!", "171!", "(-1)!", "0.5!", "1000!", "ln(0)", "lg(0)", "ld(0)", "log2(0)",
            "log10(0)", "log(2, 0)", "log(0, 2)", "log(1, 2)", "log(0, 0)", "ln(-0.0)",
            "0^(-1)", "0^(-0.5)", "0^(-2)", "(-0.0)^(-1)", "(-0.0)^(-2)", "0^0", "atanh(1)",
            "atanh(-1)", "atanh(2)", "acosh(0)", "asinh(1E308 * 10)", "exp(710)", "exp(-800)",
            "10^400", "-10^400", "10^(-400)", "1E308 + 1E308", "-1E308 - 1E308", "1E200^2",
            "(-1E200)^3", "(-1E200)^2", "1E200 * 1E200", "1E200 / 1E-200", "-1E200 / 1E-200",
            "sqrt(1E308 * 10)", "cbrt(-1E308 * 10)", "abs(-1E308 * 10)", "nroot(1 / 0, 2)",
            "nroot(-1 / 0, 3)", "nroot(-1 / 0, 2)", "2^1024", "2^1023 * 2", "-2^1024",
            "cosh(1000)", "sinh(1000)", "sinh(-1000)", "tanh(1000)", "floor(1 / 0)",
            "ceil(1 / 0)", "round(1 / 0)", "round(-1 / 0, 2)", "1 / 0 + 1 / 0", "1 / 0 - 1 / 0",
            "(1 / 0) * 0", "(1 / 0) / (1 / 0)", "(1 / 0)^0", "0^(1 / 0)", "1^(1 / 0)",
            "(1 / 0) > 1E308", "-(1 / 0) < -1E308", "1 / (1 / 0)", "1 / (-1 / 0)", "atan(1 / 0)",
            "atan2(1 / 0, 1)", "atan2(1, 1 / 0)", "sgn(1 / 0)", "sgn(-1 / 0)", "sqrt(-1 / 0)",
            "exp(-1 / 0)", "ln(1 / 0)", "(1 / 0)°", "(1 / 0) (1, 2)",
            "nroot(-1 / 0, 4)", "nroot(-1 / 0, 5)", "nroot(1 / 0, 3)", "nroot(-8, 2)",
            "nroot(1 / 0, 0)", "nroot(1 / 0, -2)", "nroot(2, 1 / 0)", "nroot(-1 / 0, 1 / 0)",
            "(-1 / 0)^(1 / 2)", "(-1 / 0)^(1 / 3)", "(-1 / 0)^0.5", "(-1 / 0)^(2 / 3)",
            "(1 / 0)^(1 / 3)", "(-1 / 0)^(-1 / 3)", "cbrt(1 / 0)", "cbrt(-1 / 0)"):
    case("functions", [f"c = {rhs}"], [])
for line in ("r = Div(3, 0)", "r = Mod(3, 0)", "r = Div(-3, 0)", "r = Mod(0, 0)",
             "r = GCD(0, 0)", "r = LCM(0, 0)", "r = nCr(1030, 500)", "r = nCr(3, -1)",
             "r = nPr(200, 150)", "r = Sum({1E308, 1E308})", "r = Product({1E200, 1E200})",
             "r = Mean({1E308, 1E308})", "r = Sum(10^(100 k), k, 1, 5)",
             "r = Product(10^(100 k), k, 1, 5)", "r = Max({1E308, 1E308 * 10})",
             "r = Min({-1E308 * 10, 0})", "r = Length(Sequence(1 / 0, k, 1, 3))",
             "r = Element({1 / 0, -1 / 0}, 2)", "r = Mean({1 / 0, -1 / 0})",
             "r = Mean({1 / 0, 1})", "r = Sum({})", "r = Product({})", "r = Max({})",
             "r = Min({})", "r = Iteration(x^2, 10, 20)", "r = Iteration(2 x, 1, 2000)",
             "g = IterationList(x^2, 10, 12)"):
    case("functions", [line], [])

# ------------------------------------------------------- through a free number
topic("dynamic")
MOVES = [setn("a", 0), setn("a", -2), setn("a", 0), setn("a", 0.5), setn("a", 2)]
for rhs in ("1 / a", "-1 / a", "ln(a)", "1 / a^2", "a / 0", "-a / 0", "1 / a + 1", "3 - 1 / a",
            "2 / a * a", "1 / a - 1 / a", "(1 / a)^2", "(1 / a)^a", "1 / (1 / a)", "exp(1 / a)",
            "abs(-1 / a)", "sqrt(1 / a)", "floor(1 / a)", "round(-1 / a)", "sgn(1 / a)",
            "atan(1 / a)", "If(a < 1, 1 / 0, a)", "If(a > 1, a, -1 / 0)", "1 / a > 5",
            "1 / a == 1 / 0", "Max(1 / a, 3)", "Min(1 / a, 3)"):
    case("dynamic", [num("a", 2), f"c = {rhs}"], MOVES)
# what depends on an infinite number
for rhs in ("c + 1", "-c", "1 / c", "c - c", "0 c", "c^2", "c > 0", "c == c", "If(c > 0, 1, 2)",
            "c < 1E300", "2^c", "exp(-c)", "c / c", "abs(c) + a", "sgn(c)", "c + d",
            "c d", "d - c", "Max(c, d)", "Min(c, d)"):
    case("dynamic", [num("a", 2), "c = 1 / a", "d = -3 / a", f"g = {rhs}"], MOVES)

# the sign of a zero that a command computes decides the infinity: the
# ratio is 0 or -0 with the side of A that P lies on
for script, steps in (
        ([num("q", -5), pt("A", 2, 4), pt("C", 1, 0), "P = C + (q, 1)",
          "o = AffineRatio(A, P, A)", "g = 1 / o", "l = {o, 2 o, 1 / o}"],
         [setn("q", 7), setn("q", -5), mv("A", -9, 4), setn("q", 7), mv("A", 2, 4)]),
        ([num("q", 3), pt("A", 0, 0), pt("B", 4, 0), "d = Distance(A, B) - 4", "g = 1 / (q d)"],
         [setn("q", -3), mv("B", 5, 0), mv("B", 4, 0), setn("q", 3), mv("B", -4, 0)]),
        ([num("q", 3), pt("A", 0, 2), "m = x(A) q", "g = 1 / m", "h = -1 / m"],
         [setn("q", -3), mv("A", 1, 2), mv("A", 0, 2), mv("A", -0.0, 2), setn("q", 3)]),
):
    case("dynamic", script, steps)

# ------------------------------------------------------- infinite list elements
topic("lists")
for rhs in ("Sum(l)", "Max(l)", "Min(l)", "Mean(l)", "Product(l)", "Element(l, 2)", "Length(l)",
            "Sort(l)", "Reverse(l)", "2 l", "l + 1", "First(l, 2)", "Last(l)",
            "Sum(l, 2)", "IndexOf(c, l)", "Element(Sort(l), 1)", "Element(Sort(l), 3)",
            "Unique(l)", "Zip(k + 1, k, l)", "Sequence(c k, k, -1, 1)",
            "CountIf(x > 5, l)", "KeepIf(x > 5, l)", "Join(l, {c})", "Take(l, 2, 3)",
            "Flatten({l, {c}})", "Append(l, c)"):
    case("lists", [num("a", 2), "c = 1 / a", "d = -3 / a", "l = {1, c, d}", f"g = {rhs}"], MOVES)
for rhs in ("{inf, -inf, 1}", "{1 / 0, 0 / 0}", "Sum({inf, 1})", "Sum({inf, -inf})",
            "Max({inf, 1})", "Min({-inf, 1})", "Product({inf, -1})", "Product({inf, 0})",
            "Mean({inf, 1})", "Sort({inf, 1, -inf})", "Element({1, inf}, 2)",
            "Max(inf, 1)", "Min(-inf, 1)", "Max(-inf, inf)", "Sum({1 / 0})"):
    case("lists", [f"g = {rhs}"], [])

# ------------------------------------------------------- an infinite argument
topic("inputs")
BASE = [num("a", 2), pt("A", 1, 2), pt("B", 4, 3), "c = 1 / a", "j = Line(A, B)",
        "w = Segment(A, B)", "Q = (c, 0)", "u = (1, c)"]
for line in ("k = Circle(A, c)", "s = Segment(A, c)", "P = Rotate(B, c, A)", "P = Dilate(B, c, A)",
             "P = Rotate(B, c)", "P = Dilate(B, c)", "P = A + (c, 1)", "v = (c, 1)", "P = (c, 1)",
             "P = (1, c)", "v = 2 u", "P = A + c", "v = c Vector(A, B)",
             "P = Point(j, c)", "P = Point(w, c)", "t = \"\" + c",
             "t = \"c = \" + (-c)", "t = Text(c)", "t = FractionText(c)", "t = SurdText(-c)",
             "g = Line(A, u)", "P = Translate(A, u)", "g: y = c x",
             "g: x + y = c", "k = Circle(A, 1 / c)", "f(x) = c x", "f(x) = x + c",
             "f(x) = x / c", "P = Midpoint(A, Q)", "r = Distance(A, Q)",
             "e = Ellipse(A, B, c)", "h = Hyperbola(A, B, c)", "q = Polygon(A, B, Q)",
             "r = Length(u)", "alpha = Angle(c)", "r = Div(c, 2)", "r = Mod(c, 2)",
             "r = GCD(c, 2)", "b = IsInteger(c)", "b = IsDefined(c)", "r = round(c, 2)",
             "r = nroot(c, 3)", "P = Point({c, 1})", "r = c°", "r = x(Q)", "r = y(u)",
             "k = Circle(Q, 1)", "g = Line(Q, A)", "b = c == x(Q)"):
    case("inputs", BASE + [line], MOVES)

# ------------------------------------------------------- an infinite result
topic("outputs")
for line in ("r = f(a)", "r = f(0)", "r = -f(a - a)", "r = f(a) + f(a)", "r = Sum({f(a), 1})",
             "r = Max(f(a), 1)", "r = Min(-f(a), 1)", "r = If(a < 1, f(a), 7)",
             "r = Element({f(a), 2}, 1)", "b = f(a) > 3", "r = f(f(a))", "r = Mean({f(a), 2})",
             "r = Product({f(a), -2})", "r = Sum(f(a) k, k, 1, 3)", "r = Product(f(a) k, k, 1, 3)",
             "r = Max({f(a), f(-a)})", "r = Min({f(a), f(-a)})", "r = 1 / f(a)"):
    case("outputs", [num("a", 2), "f(x) = 1 / x^2 - 1 / 4", line],
         [setn("a", 0), setn("a", 1), setn("a", 0), setn("a", 2)])
GEOMETRY = [num("a", 2), pt("A", 1, 2), pt("B", 4, 3), "j = Line(A, B)", "w = Segment(A, B)",
            "k = Circle(A, a)", "m = Slope(j)", "d = Distance(A, B)"]
for line in ("r = d / (a - a)", "r = Length(w) / 0", "r = x(A) / 0", "r = y(B) / (a - a)",
             "r = Radius(k) / 0", "r = 1 / Distance(A, A)", "r = 1 / m", "r = -1 / m",
             "r = 1 / (d - d)", "r = Area(k) / (a - a)", "r = m / 0", "r = 1 / (a m)"):
    case("outputs", GEOMETRY + [line],
         [setn("a", 0), mv("B", 1, 5), setn("a", 3), mv("B", 4, 2), mv("B", 4, 3), setn("a", 2)])

# ------------------------------------------------------- commands with an infinite number
# Every command whose answer changed when an infinite number became a
# value instead of an undefined object: the number is finite when the
# command is created, infinite after the first step, and finite again.
topic("commands")
OBJECTS = [num("a", 2), num("n", 3), "c = 1 / a", "d = -1 / a", pt("A", 1, 2), pt("B", 4, 3),
           pt("C", 2, 5), "l = {1, 2, 3}", "f(x) = x^2", "k = Circle(A, 2)",
           "L = PolyLine(A, B, C)", "w = Curve(cos(s), sin(s), s, 0, 6)"]
for line in ("b = AreEqual(c, c)", "b = AreEqual(c, d)", "b = AreEqual(d, d)",
             "b = AreCongruent(c, c)", "b = AreCongruent(c, d)", "M = Dilate(L, c)",
             "M = Dilate(L, c, A)", "r = Dot(c, n)", "r = Dot(d, c)",
             "r = FutureValue(n, n, c)", "r = FutureValue(n, c, n, n)",
             "r = FutureValue(n, n, n, d)", "r = Payment(n, n, c)", "r = Payment(n, n, n, d)",
             "r = PresentValue(n, n, c)", "r = PresentValue(n, n, n, d)",
             "r = Periods(n, n, n, d)", "r = Periods(n, n, c)", "g = Insert(c, l, n)",
             "r = Integral(f, c, c)", "r = Integral(f, d, d)", "r = Integral(f, n, c)",
             "r = Integral(f, f, c, c)", "r = IntegralBetween(f, f, c, c)",
             "r = NIntegral(f, c, c)", "r = Length(w, c, c)", "r = Length(w, 0, c)",
             "g = OrdinalRank(c, n)", "g = OrdinalRank(d, c)", "g = TiedRank(c, n)",
             "g = TiedRank(c, c)", "P = Point(k, c)", "P = Point(k, d)",
             "r = Product(c j, j, 1, n)", "r = Product(d j, j, 1, n)", "r = nPr(c, n)",
             "r = nPr(c, c)", "r = Max(c, n)", "r = Max(d, d)", "r = Min(c, c)",
             "r = Min(d, n)", "r = If(n > 2, c)", "r = If(n > 2, d, 1)"):
    case("commands", OBJECTS + [line], MOVES)

# Re-record the original inputs with signed infinity fields.
legacy_pairs = Path(__file__).with_name("legacy-infinity.txt").read_text().splitlines()

legacy = {}
sources = Path(__file__).parent
for file in sorted([*sources.glob("*.json"), *(sources / "reference").glob("*.json")]):
    if file.name != "infinity.json":
        legacy.update((entry["id"], entry) for entry in json.loads(file.read_text())["cases"])
for pair in legacy_pairs:
    original, recording = pair.split()
    entry = legacy.get(original)
    if entry is None:
        group, topic, _ = original.split("-")
        file = sources.parents[2] / "golden" / group / f"{topic}.json"
        recorded = next(entry for entry in json.loads(file.read_text())["cases"]
                        if entry["id"] == original)
        entry = {key: value for key, value in recorded.items() if key != "snapshots"}
    case("legacy", entry["script"], entry["steps"][1:])
    assert cases.cases[-1]["id"] == recording
    for key in entry.keys() - {"id", "group", "script", "steps"}:
        cases.cases[-1][key] = entry[key]

case("setnumber", ["a = 1", "b = 1 / a", "P = (b, 1)", "c = IsDefined(a)"],
     [{"action": "set_number", "label": "a", "infinity": sign} for sign in (1, -1)]
     + [setn("a", 0), setn("a", 2)])

cases.write()
