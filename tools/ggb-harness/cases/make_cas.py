"""Polynomial CAS cases, including algebra-input cases from the upstream tests."""

from caselib import Cases, setn

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
c.case("denominator", ["f(x) = 1/(x - 0.25)", "g(x) = 1/(x + 0.25)",
                       "result = CommonDenominator(f, g)"], [])
c.case("delegated", ["f(x) = 1/(x - 0.25)", "result = Asymptote(f)"], [])
c.write()
