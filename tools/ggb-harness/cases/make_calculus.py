#!/usr/bin/env python3
"""Writes cases/calculus.json: the case inputs for the calculus commands.

Derivative and NDerivative (orders, the variable form, a slider order),
Integral and NIntegral (the antiderivative, the definite number, the
evaluate boolean and the numeric-interval forms), IntegralBetween, the
Riemann sums LowerSum, UpperSum, LeftSum, RectangleSum and TrapezoidalSum,
Degree, Coefficients, Numerator, Denominator and Polynomial (of a function
and of points).

The golden harness runs GeoGebra without a CAS, so every command answers
through the no-CAS paths: polynomials exactly, everything else through the
rule trees or the adaptive quadrature, and the purely symbolic results
(such as the derivative of `sin(x)`) are undefined.

The inputs only describe what to run. Snapshots come exclusively from
running GeoGebra through the harness (`harness.sh golden calculus`).
"""

from caselib import Cases, num, pt, setn

cases = Cases("calculus")
case = cases.case
topic = cases.topic


def nums(**values):
    return [num(k, v) for k, v in values.items()]


# ------------------------------------------------------------- derivative
topic("derivative")
case("derivative", ["f(x) = sin(x)", "g = Derivative(f)", "h = NDerivative(f)"], [])
case("derivative", ["f(x) = x^2", "g = Derivative(f)", "h = Derivative(f, 2)",
                    "i = Derivative(f, 3)", "j = NDerivative(f)"], [])
case("derivative", ["f(x) = x^3 - 6x^2 + 9x", "g = Derivative(f)", "h = NDerivative(f, 2)",
                    "i = NDerivative(f, 3)"], [])
case("derivative", ["f(x) = x^4 / 12 + x^2", "g = Derivative(f, 2)", "h = NDerivative(f, 2)"], [])
case("derivative", ["f(x) = sin(x)", "g = Derivative(f, x)"], [])
case("derivative", ["f(x) = cos(x)", "g = NDerivative(f, x)", "h = NDerivative(f, x, 2)"], [])
case("derivative", ["f(x) = exp(x)", "g = NDerivative(f)", "h = NDerivative(f, 2)"], [])
case("derivative", ["f(x) = sqrt(x)", "g = NDerivative(f)"], [])
case("derivative", ["f(x) = ln(x)", "g = NDerivative(f)"], [])
case("derivative", ["f(x) = (2x + 3)^3", "g = Derivative(f)", "h = NDerivative(f)"], [])
case("derivative", ["f(x) = x^11", "g = Derivative(f)", "h = NDerivative(f)"], [])
case("derivative", ["f(x) = x * sin(x)", "g = NDerivative(f)"], [])
case("derivative", ["f(x) = sin(x) / x", "g = NDerivative(f)"], [])
case("derivative", ["f(x) = tan(x)", "g = NDerivative(f)"], [])
case("derivative", nums(n=1) + ["f(x) = x^2", "g = Derivative(f, n)"],
     [setn("n", 0), setn("n", 2), setn("n", -1), setn("n", 0.4), setn("n", 2.6), setn("n", 1)])
case("derivative", nums(n=2) + ["f(x) = sin(x)", "g = NDerivative(f, n)"],
     [setn("n", 1), setn("n", 0), setn("n", 3), setn("n", -2), setn("n", 2)])
case("derivative", nums(a=2) + ["f(x) = a x^3", "g = Derivative(f)"],
     [setn("a", -1), setn("a", 0), setn("a", 0.5), setn("a", 2)])
case("derivative", ["f(x) = x^2 + 1", "g = Derivative(x^2 + 1)", "h = NDerivative(sin(x))"], [])
case("derivative", ["f(x) = 3", "g = Derivative(f)", "h = NDerivative(f)"], [])
case("derivative", ["f(x) = x^2", "g(x) = f(f(x))", "h = NDerivative(g)"], [])
case("derivative", ["f(x) = x^3", "g = Derivative(f, x, 2)", "h = Derivative(f, x)"], [])

# -------------------------------------------------------------- integrals
topic("integral")
case("integral", ["f(x) = sin(x)", "g = Integral(f)", "h = NIntegral(f)"], [])
case("integral", ["f(x) = x^2", "g = Integral(f)", "h = Integral(f, 0, 4)",
                  "i = Integral(f, 0, 4, true)", "j = Integral(f, 0, 4, false)"], [])
case("integral", ["f(x) = sin(x)", "g = Integral(f, 0, pi)"], [])
case("integral", ["f(x) = sqrt(x)", "g = Integral(f, -1, 2)"], [])
case("integral", ["f(x) = 1/x", "g = Integral(f, -1, 1)", "h = Integral(f, 1, 4)"], [])
case("integral", ["f(x) = x^2", "g(x) = x", "d = Integral(f, g, 0, 4)"], [])
case("integral", ["f(x) = x^2", "g(x) = x", "d = Integral(f, g, 0, 4, true)",
                  "e = Integral(f, g, 0, 4, false)"], [])
case("integral", ["f(x) = x^2", "d = Integral(f, 0, 2, 10)"], [])
case("integral", ["f(x) = sin(x)", "d = Integral(f, 1, 5, 10)", "e = NIntegral(f, 1, 5)"], [])
case("integral", ["f(x) = x^2", "d = NIntegral(f, 0, 4)"], [])
case("integral", ["f(x) = x^2", "d = NIntegral(f, 0, 4, false)"], [])
case("integral", ["f(x) = x*sin(x)", "g = NIntegral(f)"], [])
case("integral", nums(a=0, b=4) + ["f(x) = x^2", "d = Integral(f, a, b)"],
     [setn("a", 1), setn("b", 1), setn("a", 5), setn("a", 0), setn("b", 0),
      setn("b", -3), setn("a", -3), setn("a", 0), setn("b", 4)])
case("integral", ["f(x) = exp(x)", "d = Integral(f, 0, 1)"], [])
case("integral", ["f(x) = x^3 - 2x", "d = Integral(f, -2, 3)"], [])
case("integral", ["f(x) = x^2", "g = IntegralBetween(f)", "h = IntegralBetween(f, x)"], [])
case("integral", ["f(x) = sin(x)", "g(x) = x^2", "d = NIntegral(f, 1, 5, 10)",
                  "e = NIntegral(f, g, 0, 4, true)", "u = IntegralBetween(f, g, 0, 4, true)",
                  "v = IntegralBetween(f, 0, 2, 10)"], [])

# ---------------------------------------------------- piecewise integrals
topic("piecewise")
case("piecewise", ["f(x) = If(x < 1, x, 2x)", "d = Integral(f, 0, 4)"], [])
case("piecewise", ["f(x) = If(x > 2, 1, 2)", "d = Integral(f, 0, 2.01)"], [])
case("piecewise", ["f(x) = If(x < 0, 0, x <= 2, x)", "d = Integral(f, -1, 3)"], [])
case("piecewise", ["f(x) = If(x < 0, 0, x <= 2, x)", "d = Integral(f, 1, 5)"], [])
case("piecewise", ["f(x) = If(x^2 > 1, 1, x > 7, 0, 0)", "d = Integral(f, -2, 2)",
                   "e = Integral(f, 0, 8)"], [])
case("piecewise", ["f(x) = If(x < 1, x^2, If(x < 3, 4 - x, x - 3))", "d = Integral(f, 0, 5)"], [])
case("piecewise", ["f(x) = If(1 < x < 3, 1, 0)", "d = Integral(f, 0, 4)"], [])
case("piecewise", ["f(x) = If(x < 1, x, 2x)", "d = Integral(f, 4, 0)"], [])
case("piecewise", ["f(x) = If(x < 1, sqrt(x), x)", "d = Integral(f, 0, 4)"], [])
case("piecewise", ["f(x) = If(x < 0, -1, 1)", "d = Integral(f, -2, 2)",
                   "e = Integral(f, 2, -2)"], [])
case("piecewise", ["f(x) = If(x == 2, 1, 0)", "d = Integral(f, 0, 4)"], [])
case("piecewise", ["f(x) = If(x != 2, 1, 0)", "d = Integral(f, 0, 4)"], [])
case("piecewise", ["f(x) = If(x < 2, x, x^2)", "d = LowerSum(f, 0, 3, 6)",
                   "u = UpperSum(f, 0, 3, 6)"], [])
case("piecewise", ["f(x) = 3 If(x > 1, x, 2)", "d = Integral(f, 0, 2)"], [])
case("piecewise", nums(c=1) + ["f(x) = If(x < c, x, 2x)", "d = Integral(f, 0, 4)"],
     [setn("c", 2), setn("c", 0), setn("c", 4), setn("c", -1), setn("c", 1)])

# -------------------------------------------------------- integralbetween
topic("between")
case("between", ["f(x) = sin(x)", "g(x) = x^2", "d = IntegralBetween(f, g, 0, 4)"], [])
case("between", ["f(x) = sin(x)", "g(x) = cos(x)", "d = IntegralBetween(f, g, 0, pi, true)",
                 "e = IntegralBetween(f, g, 0, pi, false)"], [])
case("between", ["f(x) = x^2", "g(x) = x^3", "d = IntegralBetween(f, g, 0, 1)",
                 "e = IntegralBetween(g, f, 0, 1)", "u = IntegralBetween(f, g, 1, 0)"], [])
case("between", ["f(x) = x^2", "d = IntegralBetween(f, 3, 0, 4)"], [])
case("between", ["f(x) = x^2", "g(x) = x", "d = IntegralBetween(f, g, 0, 4, true)"], [])
case("between", nums(a=0, b=2) + ["f(x) = x^2", "g(x) = x", "d = IntegralBetween(f, g, a, b)"],
     [setn("a", 2), setn("b", 2), setn("a", -1), setn("b", 0), setn("a", 0), setn("b", 2)])
case("between", ["f(x) = sin(x)", "d = IntegralBetween(f, 0, pi)",
                 "e = IntegralBetween(f, 0, pi, true)"], [])

# ------------------------------------------------------------------- sums
topic("sums")
case("sums", ["f(x) = x^2", "d = LowerSum(f, 0, 2, 4)", "u = UpperSum(f, 0, 2, 4)",
              "l = LeftSum(f, 0, 2, 4)", "t = TrapezoidalSum(f, 0, 2, 4)",
              "r = RectangleSum(f, 0, 2, 4, 0.5)"], [])
case("sums", ["f(x) = sin(x)", "d = LowerSum(f, -2, 5, 10)", "u = UpperSum(f, -2, 5, 10)",
              "l = LeftSum(f, -2, 5, 10)", "t = TrapezoidalSum(f, -2, 5, 10)"], [])
case("sums", ["f(x) = sqrt(x)", "d = LowerSum(f, -1, 2, 4)", "l = LeftSum(f, -1, 2, 4)",
              "t = TrapezoidalSum(f, -1, 2, 4)", "u = UpperSum(f, -1, 2, 4)"], [])
case("sums", ["f(x) = x^2", "d = LowerSum(f, 2, 0, 4)", "u = UpperSum(f, 2, 0, 4)",
              "l = LeftSum(f, 2, 0, 4)", "t = TrapezoidalSum(f, 2, 0, 4)"], [])
case("sums", ["f(x) = 0.5x + 1", "d = LowerSum(f, 0, 100, 200)", "u = UpperSum(f, 0, 100, 200)",
              "t = TrapezoidalSum(f, 0, 100, 200)"], [])
case("sums", ["f(x) = x^2", "d = RectangleSum(f, 0, 2, 4, 2)",
              "e = RectangleSum(f, 0, 2, 4, -0.5)", "h = RectangleSum(f, 0, 2, 4, 1.5)"], [])
case("sums", ["f(x) = x^2", "d = LowerSum(f, 1, 5, 10.7)", "u = UpperSum(f, 1, 5, 10.2)",
              "l = LeftSum(f, 1, 5, 10.5)", "t = TrapezoidalSum(f, 1, 5, 9.5)"], [])
case("sums", ["f(x) = x^2", "d = LowerSum(f, 0, 2, 1)", "u = UpperSum(f, 0, 2, 1)",
              "l = LeftSum(f, 0, 2, 1)", "t = TrapezoidalSum(f, 0, 2, 1)"], [])
case("sums", nums(n=4) + ["f(x) = x^2", "d = LowerSum(f, 0, 2, n)",
                          "u = UpperSum(f, 0, 2, n)", "l = LeftSum(f, 0, 2, n)",
                          "t = TrapezoidalSum(f, 0, 2, n)"],
     [setn("n", 0), setn("n", -3), setn("n", 0.4), setn("n", 1), setn("n", 10001),
      setn("n", 4)])
case("sums", nums(d=0.5) + ["f(x) = x^2", "r = RectangleSum(f, 0, 2, 4, d)"],
     [setn("d", 0), setn("d", 1), setn("d", -1), setn("d", 2), setn("d", 0.25)])
case("sums", ["f(x) = If(x > 1, sqrt(x - 1), 0)", "d = LowerSum(f, 0, 2, 4)",
              "l = LeftSum(f, 0, 2, 4)", "t = TrapezoidalSum(f, 0, 2, 4)"], [])

# ------------------------------------------------- degree and coefficients
topic("degree")
case("degree", ["f(x) = x^2", "d = Degree(f)"], [])
case("degree", ["f(x) = sin(x)", "d = Degree(f)"], [])
case("degree", ["f(x) = 3", "d = Degree(f)"], [])
case("degree", ["f(x) = x^3 - 6x^2 + 9x", "d = Degree(f)"], [])
case("degree", ["f(x) = x*x*x", "d = Degree(f)"], [])
case("degree", ["f(x) = x^4 / 12 + x^2", "d = Degree(f)"], [])
case("degree", nums(a=2) + ["f(x) = a x^3 + x", "d = Degree(f)"],
     [setn("a", 0), setn("a", 1), setn("a", 2)])

topic("coefficients")
case("coefficients", ["f(x) = x^2", "c = Coefficients(f)"], [])
case("coefficients", ["f(x) = sin(x)", "c = Coefficients(f)"], [])
case("coefficients", ["f(x) = 7x^3 + sin(x)", "c = Coefficients(f)"], [])
case("coefficients", ["f(x) = x^3/6 + x^2/4 + 3", "c = Coefficients(f)"], [])
case("coefficients", ["f(x) = (x - 1)(x + 2)", "c = Coefficients(f)"], [])
case("coefficients", ["f(x) = 3", "c = Coefficients(f)"], [])
case("coefficients", ["f(x) = x^2 + x", "c = Coefficients(f)", "d = Degree(f)"], [])
case("coefficients", nums(a=1) + ["f(x) = a*x^2 + 3", "c = Coefficients(f)", "d = Degree(f)"],
     [setn("a", 0), setn("a", 0.5), setn("a", 1)])

# ------------------------------------------------------ numerator, denominator
topic("fraction")
case("fraction", ["a = 3/6", "n = Numerator(a)", "d = Denominator(a)"], [])
case("fraction", ["a = 5/(-8)", "n = Numerator(a)", "d = Denominator(a)"], [])
case("fraction", ["a = 2/0", "n = Numerator(a)", "d = Denominator(a)"], [])
case("fraction", ["a = 0", "n = Numerator(a)", "d = Denominator(a)"], [])
case("fraction", ["a = 1/(-3)", "n = Numerator(a)", "d = Denominator(a)"], [])
case("fraction", ["a = 10/6", "n = Numerator(a)", "d = Denominator(a)"], [])
case("fraction", ["a = (-10)/(-6)", "n = Numerator(a)", "d = Denominator(a)"], [])
case("fraction", ["a = 0.125/3", "n = Numerator(a)", "d = Denominator(a)"], [])
case("fraction", ["a = 3/0.16666666666666666", "n = Numerator(a)", "d = Denominator(a)"], [])
case("fraction", ["a = 2.5", "n = Numerator(a)", "d = Denominator(a)"], [])
case("fraction", ["a = sqrt(2)", "n = Numerator(a)", "d = Denominator(a)"], [])
case("fraction", nums(a=3, b=6) + ["q = a/b", "n = Numerator(q)", "d = Denominator(q)"],
     [setn("a", -5), setn("b", 0), setn("b", 8), setn("a", 8), setn("a", 3), setn("b", 6)])
case("fraction", ["f(x) = x^2 + 1", "n = Numerator(f)", "d = Denominator(f)"], [])
case("fraction", ["f(x) = (x^2 + 1)/(x - 3)", "n = Numerator(f)", "d = Denominator(f)"], [])
case("fraction", ["f(x) = (x + 2)/(x + 1)", "n = Numerator(f)", "d = Denominator(f)"], [])
case("fraction", ["f(x) = 2/(3x)", "n = Numerator(f)", "d = Denominator(f)"], [])

# ------------------------------------------------------------- polynomial
topic("polynomial")
case("polynomial", ["f(x) = x^2", "g = Polynomial(f)"], [])
case("polynomial", ["f(x) = sin(x)", "g = Polynomial(f)"], [])
case("polynomial", ["f(x) = (2x + 3)^3", "g = Polynomial(f)"], [])
case("polynomial", ["f(x) = x^3/6 + x^2/4 + 3", "g = Polynomial(f)"], [])
case("polynomial", ["g = Polynomial(x^2 + 2x)", "h = Polynomial(sin(x))"], [])
case("polynomial", ["P = (1, 2)", "Q = (2, 5)", "R = (3, 10)", "g = Polynomial(P, Q, R)"], [])
case("polynomial", ["P = (1, 2)", "Q = (2, 5)", "g = Polynomial(P, Q)"], [])
case("polynomial", ["P = (1, 2)", "Q = (2, 5)", "R = (1, 7)", "g = Polynomial(P, Q, R)"], [])
case("polynomial", ["P = (1, 2)", "Q = (1, 2)", "R = (3, 10)", "g = Polynomial(P, Q, R)"], [])
case("polynomial", ["l = {(1, 2), (2, 5)}", "g = Polynomial(l)"], [])
case("polynomial", ["l = {(1, 1), (-1, 1), (0, 0)}", "g = Polynomial(l)"], [])
case("polynomial", ["l = {(0, 0), (1, 1), (2, 4), (3, 9)}", "g = Polynomial(l)"], [])
case("polynomial", ["g = Polynomial({(1, 2), (2, 5)})"], [])
case("polynomial", ["l = {(1, 2)}", "g = Polynomial(l)"], [])
case("polynomial", ["P = (1, 2)", "Q = (2, 5)", "g = Polynomial(P, Q, Q)"], [])

cases.write()
