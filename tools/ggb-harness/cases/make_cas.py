"""Polynomial CAS cases, including algebra-input cases from the upstream tests."""

from caselib import Cases, mv, setn

c = Cases("cas")
for topic, expression in [
    ("expand", "Expand(x)"),
    ("expand", "Expand((x-2)^3*(x+1))"),
    ("expand", "Expand((x+1)^12)"),
    ("factor", "Factor(x^2)"),
    ("factor", "Factor(x^4+5*x^2+6)"),
    ("factor", "Factor(x^6-3*x^4+3*x^2-1)"),
    ("factor", "Factor(100000000000000000000*x^2-100000000000000000000)"),
    ("factor", "Factor(x^12+2)"),
    ("ifactor", "IFactor(x^4-2)"),
    ("ifactor", "IFactor(6*x^2+7*x-3)"),
    ("simplify", "Simplify((x^2-1)/(x-1))"),
    ("simplify", "Simplify((x^4-1)/(x^2-1))"),
    ("square", "CompleteSquare(x^2)"),
    ("square", "CompleteSquare(x^2+2*x+2)"),
    ("square", "CompleteSquare(-3*x^2+5*x-7)"),
]:
    c.case(topic, [f"result = {expression}"], [])

c.case("simplify", ["a = 2", "f(x) = (x^2-a^2)/(x-a)", "result = Simplify(f)"], [setn("a", 4)])
c.case("denominator", ["f(x) = 1/(x-1)", "g(x) = 1/(x^2-1)",
                        "result = CommonDenominator(f, g)"], [])
for expression in [
    "x", "0.5", "5", "x^2-1", "x*(x+1)", "x*(2*x+2)", "x^3-1",
    "x*(x/2+1/2)", "(x+1)*(x^2-1)", "(1-x)*(1+x)", "3*x-2*x",
    "x^2-2", "x^4+4", "(x^2+1)^2", "x^4+2*x^2+1",
]:
    c.case("factored", [f"result = IsFactored({expression})"], [])

c.case("prime", ["a = 3", "next = NextPrime(a)", "previous = PreviousPrime(next)"], [setn("a", 100)])
c.case("prime", ["result = PreviousPrime(5)"], [])
c.case("prime", ["result = PreviousPrime(2)"], [])
for expr in ["x^4+2*x^2", "0*x", "x^2-x^2+x+1", "(x-1)*(x+1)^2"]:
    c.case("coefficients", [f"f(x) = {expr}", "d = Degree(f)", "l = Coefficients(f)"], [])

for command in ("Solve", "Solutions", "NSolve", "NSolutions"):
    c.case("solve", [f"result = {command}(x^3-x)"], [])
for expression in [
    "Solutions(x^2=3)", "Solutions((x-2)^4=0)", "Solutions(x^2=-1)",
    "Solutions(0*x=1)", "Solutions(x=x)", "NSolutions(x^6-2*x+1=0)",
    "NSolutions(x^4+34*x^3=34, x)", "Solutions({x+y=1,x-y=0},{x,y})",
    "Solutions({x^2+y^2=5,x-y=1},{x,y})",
    "Solutions({84.36=x*y^3,126.56=x*y^4},{x,y})",
    "Solutions({x+y=1,x+y=2},{x,y})", "Solutions({x+y=1,2*x+2*y=2},{x,y})",
]:
    c.case("solve", [f"result = {expression}"], [])

# Additional algebra-input forms and values observed with the native oracle.
c.case("expand", ["f(x) = (x-1)*(x+1)", "result = Expand(f)"], [])
c.case("expand", ["f(x, y) = (x+y)*(x-y)", "result = Expand(f)"], [])
c.case("factor", ["f(x) = x^4+5*x^2+6", "result = Factor(f)"], [])
c.case("factor", ["f(x) = 0", "result = Factor(f)"], [])
c.case("factor", ["f(x) = 7", "result = Factor(f)"], [])
c.case("factor", ["f(x, y) = x^2-y^2", "result = Factor(f)"], [])
c.case("factor", ["f(x, y) = x^6-4*y^3+3*x^4*y", "result = Factor(f)"], [])
c.case("ifactor", ["f(x) = x^4-2", "result = IFactor(f)"], [])
c.case("ifactor", ["f(x) = x^4-2*x^2", "result = IFactor(f)"], [])
c.case("simplify", ["f(x, y) = (x^2-y^2)/(x-y)", "result = Simplify(f)"], [])
c.case("simplify", ["result = Simplify(15/9)"], [])
c.case("square", ["f(x) = x^4+2*x^2+2", "result = CompleteSquare(f)"], [])
c.case("square", ["f(x) = x+1", "result = CompleteSquare(f)"], [])
c.case("denominator", ["f(x) = 1/(2*x)", "g(x) = 1/(3*x)", "result = CommonDenominator(f, g)"], [])
c.case("factored", ["f(x) = (1-x)*(1+x)", "result = IsFactored(f)"], [])
c.case("factored", ["f(x) = x^10+2", "result = IsFactored(f)"], [])
c.case("prime", ["a = 3.4", "next = NextPrime(a)", "previous = PreviousPrime(a)"], [])
c.case("prime", ["a = -10", "next = NextPrime(a)", "previous = PreviousPrime(a)"], [])
c.case("prime", ["result = NextPrime(1000000000000)"], [])
c.case("coefficients", ["k: x^2+y^2=1", "result = Coefficients(k)"], [])
c.case("coefficients", ["f(x) = x/x", "d = Degree(f)", "l = Coefficients(f)"], [])
c.case("projection", ["f(x) = (x^2-1)/(x-1)", "p = Numerator(f)", "q = Denominator(f)"], [])
c.case("projection", ["a = 15/9", "p = Numerator(a)", "q = Denominator(a)"], [])
c.case("projection", ["f(x) = (x-2)*(x+1)", "p = Polynomial(f)"], [])
c.case("factors", ["f(x) = x^4+5*x^2+6", "result = Factors(f)"], [])
c.case("factors", ["result = Factors(12)"], [])

for command in ("Solve", "Solutions", "NSolve", "NSolutions"):
    c.case("solve", ["f(x) = x^3-x", f"result = {command}(f)"], [])
    c.case("solve", ["g: x=3", f"result = {command}(g)"], [])
    c.case("solve", ["g: x+y=1", "h: x-y=0", f"result = {command}({{g,h}})"], [])
    c.case("solve", ["g: x+y=1", "h: x-y=0", f"result = {command}({{g,h}}, {{x,y}})"], [])
for expression in [
    "Solve(x=x)", "Solutions(1=0)", "Solutions(x^4-x^2-1)", "Solutions((x-1)^4-(x-1)^2-1)",
    "Solutions(x^6-x^3-1)", "Solutions((x^2-1)/(x-1)=0)", "Solve({x+y=1,2*x+2*y=2},{x,y})",
    "Solutions({x^2=1},{x})", "Solve({x^2=1},{x})",
]:
    c.case("solve", [f"result = {expression}"], [])

c.case("solve", ["g: x+y=1", "h: x-y=0", "l = {g,h}", "result = Solutions(l)"], [])
c.case("solve", ["a = 1", "result = Solve(x^2=a)"], [setn("a", 4), setn("a", -1)])
c.case("solve", ["f(x) = x^2-3", "result = Solutions(f, x)"], [])
c.case("expand", ["result = Expand((x^2-1)/(x-1))"], [])
c.case("simplify", ["f(x) = (x^2+2*x+1)/(x+1)", "result = Simplify(f)",
                     "g(x) = result(x)+2"], [])
c.case("delegated", ["f(x) = 1/x", "result = Asymptote(f)"], [])
c.case("delegated", ["f(x) = (x^2-1)/(x-1)", "R = RemovableDiscontinuity(f)"], [])
c.case("delegated", ["f(x) = (x^2-1)/(x-1)", "g = Simplify(f)",
                      "h = CommonDenominator(g, f)"], [])

c.case("projection", ["f(x) = 0.125/(1+0*x)", "p = Numerator(f)"], [])
c.case("projection", ["p = Numerator(123456/789012)", "q = Denominator(123456/789012)"], [])
c.case("projection", ["a = 123456/789012", "p = Numerator(a)", "q = Denominator(a)"], [])
c.case("solve", ["result = Solutions({x*y=0},{x,y})"], [])
c.case("solve", ["result = Solutions({x^2-y^2=0},{x,y})"], [])


def times(a, b):
    """Product of two polynomials given by ascending coefficients."""
    out = [0] * (len(a) + len(b) - 1)
    for i, x in enumerate(a):
        for j, y in enumerate(b):
            out[i + j] += x * y
    return out


def linear_product(roots):
    polynomial = [1]
    for root in roots:
        polynomial = times(polynomial, [-root, 1])
    return polynomial


def polynomial(polynomial):
    """GeoGebra input of a polynomial given by ascending coefficients."""
    terms = []
    for exponent, coefficient in enumerate(polynomial):
        if coefficient == 0:
            continue
        if exponent == 0:
            terms.append(str(coefficient))
        elif exponent == 1:
            terms.append("x" if coefficient == 1 else f"{coefficient}*x")
        else:
            power = f"x^{exponent}"
            terms.append(power if coefficient == 1 else f"{coefficient}*{power}")
    return "+".join(terms).replace("+-", "-")


def shifted(constant):
    return [c + 1 if i == 0 else c for i, c in enumerate(constant)]


# Wilkinson-style products: the exact coefficients dwarf f64 precision, so a
# solver that rounds them first cannot return the roots to full precision.
for degree in (18, 20):
    expression = polynomial(shifted(linear_product(range(1, degree + 1))))
    c.case("solve", [f"result = Solutions({expression} = 0)"], [])
    c.case("solve", [f"result = NSolutions({expression})"], [])
c.case("solve", [f"result = Solve({polynomial(shifted(linear_product(range(1, 21))))} = 0)"], [])

# A root shared by the numerator and the denominator must cancel, not drop.
for expression in ["x^2/x", "(x-1)^2/(x-1)"]:
    c.case("solve", [f"result = Solutions({expression} = 0)"], [])
    c.case("solve", [f"result = Solve({expression} = 0)"], [])
    c.case("solve", [f"result = NSolutions({expression})"], [])

# The order of the solutions decides every Element(..., k).
for expression, count in [
    ("x^2-3", 2), ("x^3-x", 3), ("x^4-5*x^2+4", 4),
]:
    script = [f"e{k} = Element(Solutions({expression}), {k})" for k in range(1, count + 1)]
    c.case("solve", script, [])
c.case("solve", [
    "first = Element(NSolutions(%s), 1)" % polynomial(shifted(linear_product(range(1, 21)))),
    "last = Element(NSolutions(%s), 20)" % polynomial(shifted(linear_product(range(1, 21)))),
], [])

# Cyclotomic inputs: every x^n - 1 and x^n + 1 factors over the rationals.
for n in range(15, 31):
    c.case("factor", [f"result = Factor(x^{n}-1)"], [])
for n in [15, 16, 20, 21, 22, 25, 26, 30, 32]:
    c.case("factor", [f"result = Factor(x^{n}+1)"], [])
# Products of many linear factors must peel their rational roots.
for degree in range(16, 21):
    c.case("factor", [f"result = Factor({polynomial(linear_product(range(1, degree + 1)))})"], [])
# Swinnerton-Dyer style polynomials: irreducible over the rationals, yet
# reducible modulo every prime, so only exhaustive recombination proves it.
c.case("factor", ["result = Factor(%s)" % polynomial(
    [46225, 0, -5596840, 0, 13950764, 0, -7453176, 0,
     1513334, 0, -141912, 0, 6476, 0, -136, 0, 1])], [])
c.case("factor", ["result = Factor(%s)" % polynomial(
    [576, 0, -960, 0, 352, 0, -40, 0, 1])], [])

# Inputs at and beyond the largest exact integer GeoGebra accepts.
for expression in [
    "NextPrime(9007199254740991)", "NextPrime(9007199254740992)",
    "PreviousPrime(9007199254740992)", "NextPrime(9007199254741990)",
    "PreviousPrime(9007199254741990)", "NextPrime(-7)", "PreviousPrime(-7)",
    "NextPrime(-2.5)", "PreviousPrime(-2.5)", "NextPrime(1)", "PreviousPrime(1)",
]:
    c.case("prime", [f"result = {expression}"], [])

SOLVERS = ("Solve", "Solutions", "NSolve", "NSolutions")


def results(calls):
    """One labelled result per call: r1, r2, ..."""
    return [f"r{k} = {call}" for k, call in enumerate(calls, 1)]


# A solving variable or a numeric guess after a function or a line.
for command in SOLVERS:
    c.case("hint", ["f(x) = x^2-3", f"result = {command}(f, x)"], [])
for command in SOLVERS:
    c.case("hint", ["g: x+y=1", f"result = {command}(g, x)"], [])
for command in SOLVERS:
    c.case("hint", ["g: 2x+3y=6", f"result = {command}(g, y)"], [])
# A variable the equation does not contain.
c.case("hint", ["f(x) = x^2-3"] + results(f"{command}(f, y)" for command in SOLVERS), [])
c.case("hint", ["g: x=3"] + results(
    [f"{command}(g, x)" for command in SOLVERS] + [f"{command}(g, y)" for command in SOLVERS]), [])
c.case("hint", ["g: y=2"] + results(
    [f"{command}(g, x)" for command in SOLVERS] + [f"{command}(g, y)" for command in SOLVERS]), [])
c.case("hint", ["g: x+y=1", "result = Solve(g, z)"], [])
# A numeric guess picks one root, except for Solve.
c.case("hint", ["f(x) = x^2-3", "r1 = Solve(f, x=1)", "r2 = Solutions(f, x=1)",
                "r3 = NSolve(f, x=-1)", "r4 = NSolutions(f, x=10)"], [])
c.case("hint", ["f(x) = x^3-x"] + results(
    f"NSolutions(f, x={guess})" for guess in
    ["0.4", "0.5", "0.6", "0.57", "0.58", "2", "-0.45", "100", "0", "1"]), [])
c.case("hint", ["f(x) = x^5-3x+1"] + results(
    f"NSolutions(f, x={guess})" for guess in ["0", "1", "-1", "0.9", "5", "-2.658", "1.068"]), [])
c.case("hint", ["f(x) = x^4-5x^2+4"] + results(
    f"NSolve(f, x={guess})" for guess in ["3.2", "-0.4", "1.6", "-1.7", "0.9"]), [])
c.case("hint", ["f(x) = x^2+1", "r1 = NSolutions(f, x=1)", "r2 = NSolve(f, x=1)",
                "r3 = Solutions(f, x=1)", "r4 = Solve(f, x=1)"], [])
c.case("hint", ["f(x) = x^2", "r1 = NSolutions(f, x=1)", "r2 = Solve(f, x=1)"], [])
c.case("hint", ["a = 1", "f(x) = x^2-a", "r1 = NSolutions(f, x=2)", "r2 = Solve(f, x)",
                "r3 = Solutions(f, y)"], [setn("a", 4), setn("a", -1), setn("a", 0), setn("a", 2)])
c.case("hint", ["b = 2", "f(x) = x^2-3", "result = NSolutions(f, x=b)"], [setn("b", -2), setn("b", 5)])
c.case("hint", ["g: x=3", "r1 = NSolve(g, x=1)", "r2 = Solve(g, x=1)", "r3 = Solutions(g, x=1)"], [])
c.case("hint", ["g: x+y=1", "r1 = NSolve(g, x=1)", "r2 = Solve(g, x=1)",
                "r3 = Solutions(g, y=1)"], [])
# A line through two points follows them, also when it turns vertical or
# collapses and comes back.
c.case("hint", ["A = (0, 1)", "B = (1, 0)", "g = Line(A, B)", "r1 = Solve(g, x)",
                "r2 = Solutions(g, y)", "r3 = Solve(g, y)", "r4 = NSolutions(g, x)"],
       [mv("B", 3, 0), mv("B", 0, 5), mv("B", 0, 1), mv("B", 2, 2)])
# A list of variables after a single equation follows the system solver.
c.case("hint", ["f(x) = x^2-3", "result = Solve(f, {x})"], [])
c.case("hint", ["f(x) = x^2-3", "result = Solutions(f, {x})"], [])

# Conics. A conic in one variable is a polynomial equation.
for command in SOLVERS:
    c.case("conic", ["k: x^2=4", f"result = {command}(k)"], [])
for command in SOLVERS:
    c.case("conic", ["k: x^2=4", f"result = {command}(k, x)"], [])
c.case("conic", ["k: x^2=4"] + results(f"{command}(k, y)" for command in SOLVERS), [])
c.case("conic", ["k: y^2=4"] + results(
    [f"{command}(k)" for command in SOLVERS] + ["Solve(k, x)", "Solve(k, y)", "NSolve(k, x)"]), [])
c.case("conic", ["k: x^2-5x+6=0"] + results(f"{command}(k)" for command in SOLVERS), [])
# A conic in both variables gives one function per branch.
c.case("conic", ["k: x^2+y^2=4", "r1 = Solutions(k)", "r2 = Solutions(k, x)",
                 "r3 = Solutions(k, y)", "r4 = Solve(k, y)"], [])
c.case("conic", ["k: x^2+y^2=4", "r1 = NSolve(k)", "r2 = NSolutions(k)", "r3 = NSolve(k, x)",
                 "r4 = NSolutions(k, x)", "r5 = NSolve(k, y)", "r6 = NSolutions(k, y)"], [])
for equation in [
    "x^2+4y^2=4", "x^2+x*y+y^2=3", "x^2+x*y+y^2=1", "x^2-y^2=1", "2x^2+3y^2+x-y=5",
    "(x-1)^2+(y-2)^2=9", "3x^2-y^2+x=2", "x^2+x*y=1", "x^2+2x*y+3y^2=6", "x^2+2x-y^2=-3",
    "x^2+2x-y^2=3", "x^2-4x*y+y^2+2x=1", "x^2+x*y+y^2+x=3", "x^2+x*y-y^2=-5", "x^2+3y^2-x=5",
    "x*y=1", "x*y+x+y=1", "y^2=x",
]:
    c.case("conic", [f"k: {equation}", "r1 = Solutions(k)", "r2 = Solutions(k, y)",
                     "r3 = Solve(k, y)"], [])
# Solving a parabola for its squared variable needs both roots.
for equation in ["y=x^2", "y=x^2+x", "x^2+2x-y=3", "x^2+y=3"]:
    c.case("conic", [f"k: {equation}", "r1 = Solutions(k)", "r2 = Solutions(k, y)"], [])
# Degenerate conics split into lines.
for equation in [
    "x^2-y^2=0", "x^2+2x*y+y^2=1", "x^2-2x*y+y^2=0", "x*y=0", "x^2-2y^2=0", "(x-2y)*(x-3y)=0",
    "(x-y/2)*(x-2y)=0", "(x-y)*(x-2)=0", "(x-2y-1)*(x+2y-3)=0", "x^2+3x*y+2y^2=0",
    "2x^2-x*y-y^2+x+2y=1", "(x-2y+5)*(x-2y-4)=0", "(x+y/3)*(x-y/2)=0", "x^2-2x*y-y^2=0",
    "(x-2)*(y-1)=0",
]:
    c.case("conic", [f"k: {equation}", "r1 = Solve(k)", "r2 = Solutions(k)", "r3 = Solve(k, y)",
                     "r4 = Solutions(k, y)"], [])
# Conics without real points.
for equation in ["x^2+y^2=-4", "x^2+y^2=0", "x^2+1=0", "x^2=0"]:
    c.case("conic", [f"k: {equation}"] + results(
        [f"{command}(k)" for command in SOLVERS] + ["Solve(k, y)", "Solutions(k, y)"]), [])
# The conic changes its type while a number moves.
c.case("conic", ["a = 4", "k: x^2=a", "r1 = Solve(k)", "r2 = NSolutions(k)", "r3 = Solutions(k, x)"],
       [setn("a", 9), setn("a", -1), setn("a", 0), setn("a", 2)])
c.case("conic", ["a = 4", "k: x^2+y^2=a", "r1 = Solutions(k)", "r2 = Solve(k, y)", "r3 = NSolve(k)"],
       [setn("a", 9), setn("a", -1), setn("a", 0), setn("a", 1)])
c.case("conic", ["A = (1, 2)", "k = Circle(A, 3)", "r1 = Solutions(k)", "r2 = Solve(k, y)"],
       [mv("A", 0, 0), mv("A", -2, 1.5)])
c.case("conic", ["k: x^2=4", "r1 = NSolutions(k, x=1)", "r2 = NSolve(k, x=-1)", "r3 = Solve(k, x=1)",
                 "r4 = Solutions(k, x=1)"], [])
c.case("conic", ["k: x^2+y^2=4", "r1 = Solutions(k, x=1)", "r2 = NSolve(k, y=1)"], [])


def curve_case(script, steps=()):
    """A case whose curves are recorded with sample points."""
    c.case("curve", script, list(steps))
    c.cases[-1]["sample_curves"] = True


def reject(line):
    return {"action": "reject", "line": line}


# Expand, Factor and IFactor rewrite a curve coordinate by coordinate.
REWRITES = ("Expand", "Factor", "IFactor")
for command in REWRITES:
    curve_case(["k = Curve((t+1)^2, t^2-1, t, 0, 2)", f"result = {command}(k)"])
# A rational coordinate loses its removable pole, and keeps a real one.
curve_case(["k = Curve((t^2-1)/(t-1), t, t, 0, 2)"] + results(f"{command}(k)" for command in REWRITES))
curve_case(["k = Curve(1/t, t, t, -1, 1)"] + results(f"{command}(k)" for command in REWRITES))
curve_case(["k = Curve((t^2-4)/(t+2), 1/(t-1), t, -3, 3)"]
           + results(f"{command}(k)" for command in REWRITES))
# Coordinates the polynomial commands cannot rewrite stay as they are.
curve_case(["k = Curve(cos(t), sin(t), t, 0, 3)"] + results(f"{command}(k)" for command in REWRITES))
curve_case(["k = Curve(t^2-2, sqrt(t), t, 0, 4)"] + results(f"{command}(k)" for command in REWRITES))
curve_case(["k = Curve(t^2-2, t^4-4, t, -2, 2)"] + results(f"{command}(k)" for command in REWRITES))
curve_case(["f(x) = x^2", "k = Curve(f(t)+1, t*f(t), t, 0, 2)"]
           + results(f"{command}(k)" for command in REWRITES))
# The result follows the numbers of the curve, also through an empty
# interval and back.
curve_case(["a = 2", "k = Curve(a*t^2, (t+a)^2, t, 0, a)"]
           + results(f"{command}(k)" for command in REWRITES),
           [setn("a", 3), setn("a", -1), setn("a", 0), setn("a", 1)])
curve_case(["a = 1", "k = Curve((t^2-a^2)/(t-a), t, t, 0, 2)", "r1 = Expand(k)", "r2 = Factor(k)"],
           [setn("a", 0.5), setn("a", 3), setn("a", 0)])
curve_case(["k = Curve(t^2, t, t, 0, 2)", "r1 = Expand(k)", "r2 = Factor(r1)", "r3 = IFactor(r2)"])
curve_case(["k = Curve(t^2, t, t, 2, 0)"] + results(f"{command}(k)" for command in REWRITES))

# The reference refuses a line, a conic and a list.
for command in REWRITES:
    c.case("rejected", ["g: x+y=1", "k: x^2+y^2=4", "l = {1, 2}", "A = (0, 1)", "B = (1, 0)",
                        "s = Segment(A, B)"],
           [reject(f"r = {command}({argument})") for argument in
            ["g", "k", "l", "s", "x+y=1", "x^2+y^2=4", "{1, 2}", "{x^2-1, (x+1)^2}"]])

# Simplify reads a line and a parabola in y as the function it graphs.
c.case("simplify", ["g: x+y=1", "result = Simplify(g)"], [])
c.case("simplify", ["k: y=x^2", "result = Simplify(k)"], [])
c.case("simplify", ["g: 2x+3y=6", "h: y=2", "v: x=3", "r1 = Simplify(g)", "r2 = Simplify(h)",
                     "r3 = Simplify(v)"], [])
c.case("simplify", ["A = (0, 1)", "B = (1, 0)", "g = Line(A, B)", "s = Segment(A, B)",
                     "r1 = Simplify(g)", "r2 = Simplify(s)"],
       [mv("B", 3, 0), mv("B", 0, 5), mv("B", 0, 1), mv("B", 2, 2)])
c.case("simplify", ["r1 = Simplify(x+y=1)", "r2 = Simplify(y=x^2-x)", "r3 = Simplify(x=2)"], [])
c.case("simplify", ["k: x^2+y^2=4", "h: x*y=1", "p: y^2=x", "q: 2y=x^2+4x", "r1 = Simplify(k)",
                     "r2 = Simplify(h)", "r3 = Simplify(p)", "r4 = Simplify(q)"], [])
c.case("simplify", ["A = (0, 1)", "k = Circle(A, 2)", "result = Simplify(k)"], [mv("A", 1, 1)])
c.case("simplify", ["a = 1", "k: y=a*x^2+x+a", "result = Simplify(k)"],
       [setn("a", -2), setn("a", 0), setn("a", 0.5)])
c.case("simplify", ["a = 1", "k: x^2+a*y^2+y=1", "result = Simplify(k)"],
       [setn("a", 0), setn("a", 2), setn("a", 0)])

# Simplify rewrites a curve like the other three commands.
curve_case(["k = Curve((t+1)^2, t^2-1, t, 0, 2)", "result = Simplify(k)"])
curve_case(["k = Curve((t^2-1)/(t-1), cos(t), t, 0, 2)", "j = Curve(1/t, sqrt(t), t, 0, 4)",
            "r1 = Simplify(k)", "r2 = Simplify(j)"])
curve_case(["a = 2", "k = Curve(a*t^2, (t^2-a^2)/(t-a), t, 0, a)", "result = Simplify(k)"],
           [setn("a", 3), setn("a", -1), setn("a", 0), setn("a", 1)])

# Simplify of a text tidies its signs as a text.
c.case("text", ['t = "x+x"', "result = Simplify(t)"], [])
c.case("text", ['result = Simplify("x+-x--x")'], [])
for k, text in enumerate([
    "+1x++x--x+-1x-+1x++x", "x-x-x-x", "x=x=x=x+x+x+x", "1x - 2", "+", "a - -b+ +c", "3 -1x",
    "-1x", "1x", "11x+1xy", "x - - 1 x", "+++", "---", "x+ +1 x", "2=-1x", "y = - 1x+ - 3",
    " +x", "x^2+-1x^2", "1x1x-1x", "+ 1x", "a+b=c-d", "- - -x",
]):
    c.case("text", [f't = "{text}"', "result = Simplify(t)"], [])
c.case("text", ["a = 2", 't = "x+" + a', "result = Simplify(t)"],
       [setn("a", -3), setn("a", 1), setn("a", -1)])
c.case("text", ['t = "1"', 'u = t + "x-x"', "result = Simplify(u)"], [])
c.case("text", ['t = ""'], [reject("r = Simplify(t)"), reject('r = Simplify("")')])

c.case("rejected", ["l = {1, 2}", "A = (0, 1)", "B = (1, 0)", "C = (2, 3)", "p = Polygon(A, B, C)"],
       [reject(f"r = Simplify({argument})") for argument in
        ["l", "{1, 2}", "{x^2-1, (x+1)^2}", "A", "p"]])

# PlotSolve: the solutions of one equation as points on the x-axis.
c.case("plot", ["f(x) = x^2-3", "result = PlotSolve(f)"], [])
c.case("plot", ["g: x=3", "result = PlotSolve(g)"], [])
c.case("plot", ["k: x^2=4", "result = PlotSolve(k)"], [])
c.case("plot", ["g: x+y=1", "h: x-y=0", "l = {g, h}", "result = PlotSolve(l)"], [])
# The upstream test and other equations in x.
c.case("plot", results(f"PlotSolve({equation})" for equation in
                       ["x^3-x", "x^2=2", "x=x", "(x-1)^2*(x+2)", "x^2+1", "x^2=0", "2x=7"]), [])
c.case("plot", ["f(x) = 0x", "g(x) = x^2+1", "h(x) = (x^2-1)/(x-1)", "r1 = PlotSolve(f)",
                "r2 = PlotSolve(g)", "r3 = PlotSolve(h)"], [])
# A second variable stays as the parameter of a line along the x-axis.
c.case("plot", ["g: x+y=1", "h: 2x+3y=6", "i: y=2", "j: 2x-y=0", "r1 = PlotSolve(g)",
                "r2 = PlotSolve(h)", "r3 = PlotSolve(i)", "r4 = PlotSolve(j)"], [])
c.case("plot", ["A = (0, 1)", "B = (1, 0)", "g = Line(A, B)", "result = PlotSolve(g)"],
       [mv("B", 3, 0), mv("B", 0, 5), mv("B", 0, 1), mv("B", 2, 2)])
c.case("plot", ["a = 1", "f(x) = x^2-a", "result = PlotSolve(f)"],
       [setn("a", 4), setn("a", -1), setn("a", 0), setn("a", 2)])
# Conics in one variable and degenerate conics.
for equation in ["y^2=4", "x^2-5x+6=0", "x^2+1=0", "x^2=0", "x^2-y^2=0", "x*y=0", "x^2-2y^2=0",
                 "(x-y)*(x-2)=0", "x^2+2x*y+y^2=1", "x^2+y^2=-4"]:
    c.case("plot", [f"k: {equation}", "result = PlotSolve(k)"], [])
c.case("plot", ["a = 4", "k: x^2=a", "result = PlotSolve(k)"],
       [setn("a", 9), setn("a", -1), setn("a", 0), setn("a", 2)])
# A list always passes its variables as a second argument, which the
# command does not take.
c.case("plot", ["g: x+y=1", "h: x-y=0", "r1 = PlotSolve({g, h})", "r2 = PlotSolve({x^2=1})"], [])
c.case("plot", ["f(x) = x^2-3", "g: x=3"],
       [reject("r = PlotSolve(f, x)"), reject("r = PlotSolve(g, y)"), reject("r = PlotSolve(2)"),
        reject("r = PlotSolve(x, y)"), reject("r = PlotSolve(f, x=1)")])
# Results the kernel does not reach: curves inside the list, and a
# function that is no polynomial.
c.case("plot", ["k: x^2+y^2=4", "result = PlotSolve(k)"], [])
c.case("plot", ["k: x*y=1", "result = PlotSolve(k)"], [])
c.case("plot", ["f(x) = sin(x)", "result = PlotSolve(f)"], [])
# A parabola solved for its linear variable is the conic itself, which
# the kernel cannot hold inside a list.
c.case("conic", ["k: y=x^2+x", "result = Solve(k, y)"], [])

# The numeric solver from a start value: Newton's method where every step
# lowers the residual, at most halved once, and ends on a simple root.
c.case("start", ["f(x) = x^2-2"] + results(
    f"NSolutions(f, x={start})" for start in ["1", "-1", "0.5", "-3", "100"]), [])
c.case("start", ["f(x) = x^3-2x+2"] + results(
    f"NSolutions(f, x={start})" for start in ["-2", "-1.5", "-5"]), [])
c.case("start", ["f(x) = x^4-x^2-1"] + results(
    f"NSolutions(f, x={start})" for start in ["1", "-1", "2", "-3", "0.9"]), [])
c.case("start", ["f(x) = (x+1)^2*(x-4)"] + results(
    f"NSolutions(f, x={start})" for start in ["5", "4.5", "3.5"]), [])
c.case("start", ["f(x) = x^2-3", "r1 = NSolve(f, x=2)", "r2 = Solutions(f, x=-2)",
                 "r3 = Solve(f, x=2)", "r4 = NSolutions(f, x=1000)"], [])
c.case("start", ["a = 2", "f(x) = x^2-a", "result = NSolutions(f, x=a)"],
       [setn("a", 9), setn("a", 0.25), setn("a", -1), setn("a", 3)])
# Where the iteration stalls, cycles, needs shorter steps or runs out of
# steps the solver searches on in a way the start value does not explain.
for function, starts in [
    ("x^2-2", ["-0.01", "0", "10^(-17)", "10^300", "10^30"]),
    ("x^2-3", ["10^(-17)", "10^300", "-0.0003", "-0.00029", "-0.0001", "0.00001"]),
    ("x^3-2x+2", ["-1", "0", "1", "0.5"]),
    ("x^3+1", ["0"]),
    ("x^5-x+3", ["0", "1"]),
    ("x^4-x^2-1", ["0", "0.1", "-0.1"]),
    ("x^4+5x^3+2x^2-4x+2", ["0.193392"]),
    ("x^4+x^3-4x^2-5x+5", ["-1"]),
    ("(x+1)^2*(x-4)", ["-2.752"]),
    ("(x-3)^3", ["-1.292"]),
    ("x^2", ["1"]),
    ("x^5+x^4-6x^3-5x^2+4", ["100000"]),
]:
    c.case("start", [f"f(x) = {function}"] + results(
        f"NSolutions(f, x={start})" for start in starts), [])
# The solver is not bound to polynomials.
c.case("start", ["f(x) = sqrt(x)-2", "r1 = NSolutions(f, x=1)", "r2 = NSolutions(f, x=-1)"], [])

# A text holding an equals sign is a text, not an equation.
c.case("text", ['r1 = Simplify("α=β")', 'r2 = Simplify("a=b")', 'r3 = Simplify("x+-1x=2")',
                'r4 = Simplify("a,b=c")', 'r5 = Simplify("(a=b")'], [])

# A coordinate whose expansion is too large for the kernel, and coordinates
# that are no polynomials: the CAS rewrites them, also where the value
# changes (a cancelled pole, a square root squared).
curve_case(["k = Curve((t^65-1)/(t-1), t, t, 0, 2)", "r1 = Expand(k)", "r2 = Simplify(k)"])
curve_case(["k = Curve((t^2-1)/(t-1)+sin(t), t, t, 0, 2)",
            "j = Curve((t^2-1)/(t-1)*sin(t), t, t, 0, 2)", "r1 = Simplify(k)", "r2 = Expand(j)"])
curve_case(["k = Curve(sin(t)/t, t, t, -1, 1)", "j = Curve(sqrt(t)^2, t, t, -1, 1)",
            "r1 = Simplify(k)", "r2 = Simplify(j)"])
curve_case(["k = Curve(sin(t)^2+t, cos(t)*t^2, t, 0, 2)"]
           + results(f"{command}(k)" for command in REWRITES + ("Simplify",)))
curve_case(["a = 2", "k = Curve(a*sin(t)+t^2, t^3-a, t, 0, 2)", "r1 = Expand(k)",
            "r2 = Simplify(k)"], [setn("a", -1), setn("a", 0.5)])

# A line through points with fractions for coordinates.
c.case("simplify", ["A = (1/3, 2/7)", "B = (5/11, 1/13)", "g = Line(A, B)",
                     "result = Simplify(g)"], [mv("B", 0.3, 0.7), mv("A", 0.123456, -0.654321)])
c.case("simplify", ["A = (0.1, 0.2)", "B = (0.7, 0.3)", "g = Line(A, B)", "k: y=0.1x^2+0.7x-1/3",
                     "r1 = Simplify(g)", "r2 = Simplify(k)"], [])
c.write()
