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
c.write()
