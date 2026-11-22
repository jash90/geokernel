"""The symbolic `SolveODE` forms (`AlgoSolveODECas`), recorded with the
native CAS oracle: `SolveODE(f)`, `SolveODE(f, P)` and the curve inputs.

Topic `polynomial` holds the right-hand sides the kernel solves itself (a
polynomial in `x`), topic `curve` the curve inputs, which stay undefined
with the CAS too, topic `reject` the argument errors, and topic `cas` the
answers only the CAS gives (listed as known gaps)."""

from caselib import Cases, mv, setn

cases = Cases("odesymbolic")


def case(topic, script, steps=()):
    cases.case(topic, script.split("\n"), list(steps))


def reject(line):
    return {"action": "reject", "line": line}


# y' = p(x): the antiderivative with a zero constant, or the one through
# the point
cases.topic("polynomial")
case("polynomial", "a = 1\nf(x) = a - x\ng = SolveODE(f)\nk = f(1)", [setn("a", 2), setn("a", 0)])
case("polynomial", "f(x) = -x\nP = (1, 1)\ng = SolveODE(f, P)", [mv("P", 2, 3), mv("P", -1, 0)])
case("polynomial", "g = SolveODE(2x)\nh = SolveODE(x^2 - 1, (0, 2))")
case("polynomial", "f(x) = (x + 1)^3 (x - 2)\ng = SolveODE(f)\nh = SolveODE(f, (1, 1))")
# the zero and the constant right-hand side
case("polynomial", "f(x) = 0\ng = SolveODE(f)\nh = SolveODE(f, (1, 1))")
case("polynomial", "f(x) = 2\ng = SolveODE(f)\nh = SolveODE(f, (1, 1))")
# an object exponent follows its whole-number value
case("polynomial", "b = 2\nf(x) = x^b\nP = (1, 1)\ng = SolveODE(f, P)\nh = SolveODE(f)",
     [setn("b", 3), setn("b", 0), setn("b", 1)])
# decimal coefficients and a point off the grid
case("polynomial", "f(x) = 0.1 x^2 + 0.7x\ng = SolveODE(f)\nh = SolveODE(f, (0.3, 1.7))")
case("polynomial", "f(x) = x^12 - 3x^5\ng = SolveODE(f)\nh = SolveODE(f, (1.5, -2))")
# the point becomes undefined and defined again
case("polynomial", "n = 1\nf(x) = 3x^2\nP = (n, 1 / n)\ng = SolveODE(f, P)",
     [setn("n", 0), setn("n", 2)])
# the right-hand side becomes undefined and defined again
case("polynomial", "a = 1\nf(x) = x / a\ng = SolveODE(f)\nh = SolveODE(f, (1, 1))",
     [setn("a", 0), setn("a", 2)])
case("polynomial", "f(x) = 1 / 0\ng = SolveODE(f)\nh = SolveODE(f, (1, 1))")

# a curve is accepted and never solved
cases.topic("curve")
case("curve", "c = Curve(t, t, t, 0, 1)\ng = SolveODE(c)\nh = SolveODE(c, (1, 1))")
cases.cases[-1]["sample_curves"] = True
case("curve", "b = 1\nc = Curve(t^2, 2t, t, 0, b)\nP = (1, 1)\ng = SolveODE(c)\nh = SolveODE(c, P)",
     [setn("b", -1), mv("P", 2, 0), setn("b", 3)])
cases.cases[-1]["sample_curves"] = True

# numbers, points and a wrong second argument are argument errors
cases.topic("reject")
case("reject", "n = 1\nf(x) = x\nc = Curve(t, t, t, 0, 1)",
     [reject("g = SolveODE(3)"), reject("g = SolveODE(n)"), reject("g = SolveODE(f, 2)"),
      reject("g = SolveODE(f, (1, 1), 2)"), reject("g = SolveODE(n, (1, 1))"),
      reject("g = SolveODE(c, 2)"), reject("g = SolveODE((1, 1))")])

# what only the CAS answers: no polynomial in x, a slope that reads y, a
# function of two variables
cases.topic("cas")
case("cas", "f(x) = sin(x)\ng = SolveODE(f)\nh = SolveODE(f, (0, 1))")
case("cas", "f(x) = 1 / x\ng = SolveODE(f)\nh = SolveODE(f, (1, 1))")
case("cas", "g = SolveODE(x y)\nh = SolveODE(y, (0, 1))\ni = SolveODE(x + y, (0, 0))")
case("cas", "f(x, y) = x\ng = SolveODE(f)\nh = SolveODE(f, (1, 1))")
case("cas", "f(x) = If(x < 0, x, 2x)\ng = SolveODE(f)")
case("cas", "b = 2\nf(x) = x^b\ng = SolveODE(f)", [setn("b", -1), setn("b", 0.5), setn("b", 3)])

# These cases run against the backend with symbolic evaluation disabled.
cases.topic("nocas")
case("nocas", "a = 1\nf(x) = -x/a\nP = (1, 1)\ng = SolveODE(f)\nh = SolveODE(f, P)",
     [setn("a", 0), mv("P", 2, -1), setn("a", 2)])
case("nocas", "b = 1\nc = Curve(t^2, 2t, t, 0, b)\nP = (1, 1)\ng = SolveODE(c)\nh = SolveODE(c, P)",
     [setn("b", -1), mv("P", 2, 0), setn("b", 2)])
cases.cases[-1]["sample_curves"] = True

cases.write()
