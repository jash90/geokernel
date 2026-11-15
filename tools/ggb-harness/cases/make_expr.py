#!/usr/bin/env python3
"""Writes cases/expr.json: arithmetic expressions.

Numbers: the operators `+ - * / ^ !`, implicit multiplication, the
precedence traps of GeoGebra's grammar (`-2^2`, `2^-a^2`, `a/2a`, `3!^2`),
the built-in functions, the constants `pi`, `e`, `°` and `∞`, degrees and
angle objects, commands inside an expression (`Distance(A, B) / 2`,
`x(A) + y(B)`), undefined results (division by zero, roots of negative
numbers) and the infinities GeoGebra keeps behind them, very large and very
small values, and chains of dependent numbers.

The steps set the free numbers (`set_number`) and move the free points, and
always come back to a defined state after an undefined one.
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("expr")
case = cases.case
topic = cases.topic
rp = cases.rp
r2 = cases.r2


def nums(**values):
    return [num(k, v) for k, v in values.items()]


# ------------------------------------------------------- operators
topic("arith")
for rhs in ("a + b", "a - b", "a * b", "a / b", "a b", "a^b", "b - a - 1", "a + 2*b",
            "(a + b) * (a - b)", "2(a + 1) - b", "a / b / 2", "-a + b", "a * -b",
            "a^2 + b^2", "(a - b)^3"):
    a, b = r2(0.5, 4), r2(0.5, 3)
    case("arith", nums(a=a, b=b) + [f"c = {rhs}"],
         [setn("a", r2(-4, 4)), setn("b", r2(0.5, 3)), setn("a", 0), setn("b", -2),
          setn("a", a), setn("b", b)])

# ------------------------------------------------------- precedence traps
topic("prec")
for rhs in ("-a^2", "-2^2", "2^-a", "2^-a^2", "a^-1", "a/2a", "a/2 a", "1/2/a", "a^b^2",
            "a!^2", "-a!", "2^a!", "2a!", "a 3^2", "(a + 1)(a - 1)", "a(a + 1)",
            "a^2²", "2 - -a", "a - (b - a)", "a/(2b)"):
    case("prec", nums(a=3, b=2) + [f"c = {rhs}"],
         [setn("a", 2), setn("b", 3), setn("a", 4), setn("b", -1), setn("a", 3), setn("b", 2)])

# ------------------------------------------------------- built-in functions
topic("func")
for name, lo, hi in (("sin", -6, 6), ("cos", -6, 6), ("tan", -1.4, 1.4), ("sec", -1.4, 1.4),
                     ("csc", 0.2, 3), ("cot", 0.2, 3), ("sinh", -4, 4), ("cosh", -4, 4),
                     ("tanh", -4, 4), ("asin", -1, 1), ("acos", -1, 1), ("atan", -8, 8),
                     ("asinh", -8, 8), ("acosh", 1, 8), ("atanh", -0.9, 0.9),
                     ("sqrt", 0, 9), ("cbrt", -9, 9), ("abs", -9, 9), ("sgn", -3, 3),
                     ("floor", -5, 5), ("ceil", -5, 5), ("round", -5, 5), ("exp", -5, 5),
                     ("ln", 0.1, 9), ("log", 0.1, 9), ("ld", 0.1, 9), ("gamma", 0.2, 6)):
    v = r2(lo, hi)
    case("func", nums(a=v) + [f"c = {name}(a)", f"d = 2 {name}(a) + 1"],
         [setn("a", r2(lo, hi)), setn("a", r2(lo, hi)), setn("a", 0), setn("a", v)])
for rhs in ("atan2(a, b)", "atan2(b, a)", "log(b, a)", "ln(b, a)", "round(a, 2)",
            "round(a b, 1)", "a sin b", "sin a + cos b", "sqrt(a^2 + b^2)", "|a - b|",
            "abs(a) + |b|", "sin(a)^2 + cos(a)^2", "exp(ln(a))"):
    a, b = r2(0.5, 6), r2(0.5, 6)
    case("func", nums(a=a, b=b) + [f"c = {rhs}"],
         [setn("a", r2(-6, 6)), setn("b", r2(0.5, 6)), setn("a", a), setn("b", -b)])
# factorial and gamma at whole numbers and between them
case("func", nums(a=5) + ["c = a!", "d = gamma(a + 1)"],
     [setn("a", 0), setn("a", 1), setn("a", 10), setn("a", 20), setn("a", 2.5), setn("a", -1),
      setn("a", 170), setn("a", 171), setn("a", 5)])

# ------------------------------------------------------- constants
topic("const")
for rhs in ("a pi", "2pi a", "π a", "e^a", "a e", "pi / a", "a + Infinity", "a / ∞"):
    case("const", nums(a=1.5) + [f"c = {rhs}"],
         [setn("a", -2), setn("a", 0), setn("a", 0.25), setn("a", 1.5)])
# expressions without objects are free numbers
for rhs in ("2 + 3", "2^10 / 4", "sqrt(2)", "pi / 4", "3!"):
    case("const", [f"c = {rhs}", "d = c + 1"], [setn("c", 2), setn("c", -0.5)])

# ------------------------------------------------------- degrees and angles
topic("deg")
for rhs in ("sin(a°)", "cos(a°)", "tan(a°)", "a°", "floor(a°)",
            "ceil(2a°)", "round(a°)", "a deg"):
    case("deg", nums(a=30) + [f"c = {rhs}"],
         [setn("a", 45.7), setn("a", 90), setn("a", -12.3), setn("a", 30)])
for _ in range(4):
    a, b, c = rp(), rp(), rp()
    case("deg", [pt("A", *a), pt("B", *b), pt("C", *c), "w = Angle(A, B, C)",
                 "f = floor(w)", "g = 2 w", "h = round(w / 2)", "k = sin(w)"],
         [mv("A", *rp()), mv("C", *rp()), mv("A", *a)])

# ------------------------------------------------------- undefined results
topic("undef")
for rhs, bad in (("1 / a", 0), ("sqrt(a)", -1), ("ln(a)", -2), ("log(a)", 0),
                 ("a^0.5", -4), ("asin(a)", 2), ("acosh(a)", 0.5), ("atanh(a)", 1),
                 ("tan(a)", 0), ("cot(a)", 0), ("a / a", 0), ("0^a", -1),
                 ("(-8)^a", 0.5), ("gamma(a)", 0), ("a!", -1)):
    case("undef", nums(a=2) + [f"c = {rhs}", "d = c + 1"],
         [setn("a", bad), setn("a", 3), setn("a", bad), setn("a", 2)])
# an infinity behind an undefined number
case("undef", nums(a=2) + ["c = 1 / a", "d = 1 / c", "f = atan(c)", "g = c - c", "h = exp(-c)"],
     [setn("a", 0), setn("a", 2), setn("a", 0), setn("a", -1)])
case("undef", nums(a=-8) + ["c = a^(1/3)", "d = a^(2/3)", "f = a^(-1/3)", "g = a^-(1/3)",
                            "h = a^(1/4)"],
     [setn("a", 8), setn("a", 0), setn("a", -27), setn("a", -8)])
case("undef", ["A = (1, 2)", "B = (1, 2)", "c = 1 / Distance(A, B)", "d = 3 + c"],
     [mv("B", 4, 6), mv("A", 4, 6), mv("A", 1, 2)])

# ------------------------------------------------------- very large and very small values
topic("big")
for rhs in ("a * b", "a / b", "a + b", "a^2", "sqrt(a)", "exp(a)", "ln(a)", "1 / a"):
    case("big", nums(a=2, b=3) + [f"c = {rhs}"],
         [setn("a", 1e200), setn("b", 1e200), setn("a", 1e-200), setn("b", 1e-200),
          setn("a", 700), setn("a", 710), setn("b", -1e300), setn("a", 2), setn("b", 3)])

# ------------------------------------------------------- commands inside expressions
topic("cmd")
# the arguments of a command are objects: a command inside a command
# (`Slope(Line(A, B))`) is not supported by the kernel
for extra, rhs in (([], "Distance(A, B) / 2"), ([], "x(A) + y(B)"),
                   ([], "x(A) y(B) - x(B) y(A)"), ([], "Distance(A, B) + Distance(B, C)"),
                   ([], "sqrt(Distance(A, B))"), ([], "Area(A, B, C) / 2"),
                   (["l = Line(A, B)"], "Slope(l) + 1"),
                   (["l = Line(B, C)"], "Distance(A, l) * 2"),
                   (["k = Circle(A, B)"], "Radius(k) - Distance(A, C)"),
                   ([], "x(Midpoint(A, B)) + 1"),
                   (["u = Vector(A, B)"], "Length(u)^2"), ([], "Angle(A, B, C) / pi"),
                   (["l = Line(A, B)", "m = Line(B, C)"], "y(Intersect(l, m))")):
    a, b, c = rp(), rp(), rp()
    case("cmd", [pt("A", *a), pt("B", *b), pt("C", *c)] + extra + [f"d = {rhs}"],
         [mv("A", *rp()), mv("B", *rp()), mv("C", *rp()), mv("B", *a), mv("A", *a)])

# ------------------------------------------------------- chains of dependent numbers
topic("chain")
for _ in range(4):
    a, b = r2(-4, 4), r2(0.5, 4)
    case("chain", nums(a=a, b=b) + ["c = a + 2*b", "d = c^2 - a", "f = d / (c + 1)",
                                    "g = f + c + d", "h = sqrt(abs(g))"],
         [setn("a", r2(-4, 4)), setn("b", r2(0.5, 4)), setn("a", -1 - 2 * b), setn("a", a)])
case("chain", nums(a=1) + ["b = a + 1", "c = b + 1", "d = c + 1", "f = d + 1", "g = f * a"],
     [setn("a", 2), setn("a", -4), setn("a", 0), setn("a", 1)])
case("chain", ["A = (1, 1)", "B = (4, 5)", "d = Distance(A, B)", "r = d / 5", "s = r^2 + 1",
               "t = sqrt(s - 1)"],
     [mv("B", 7, 9), mv("B", 1, 1), mv("A", 0, 0)])
case("chain", nums(a=2) + ["b = a^2", "c = b - 4", "d = 1 / c", "f = d * 0"],
     [setn("a", -2), setn("a", 3), setn("a", 2)])

# ------------------------------------------------------- rounding
topic("round")
for v in (2.5, -2.5, 0.5, -0.5, 1.005, 2.675, -2.675, 1e-9, -3.0000000001):
    case("round", nums(a=1) + ["c = round(a)", "d = floor(a)", "f = ceil(a)",
                               "g = round(a, 2)", "h = round(a, -1)"],
         [setn("a", v), setn("a", -v), setn("a", 10 * v)])

# ------------------------------------------------------- points
topic("point")
for rhs in ("(a, 2*a)", "(a + 1, a^2)", "A + (1, 2)", "(A + B) / 2", "A + u", "2A - B",
            "A + a (B - A)", "(x(A), y(B))", "B - u", "A + u - v", "(A + B + C) / 3",
            "a A", "A / a", "-A", "A + (a; pi / 3)", "(sqrt(a), y(A))", "A + 2u",
            "(x(B) - x(A), a)", "A + (B - A) / abs(B - A)", "(A + B) / 2 + (C - A) / 2"):
    A, B, C, u, v, a = rp(), rp(), rp(), rp(), rp(), r2(0.5, 4)
    case("point", [pt("A", *A), pt("B", *B), pt("C", *C), pt("u", *u), pt("v", *v),
                   num("a", a), f"P = {rhs}"],
         [mv("A", *rp()), setn("a", r2(-4, 4)), mv("u", *rp()), mv("B", *A), setn("a", 0),
          mv("v", 0, 0), mv("A", *A), setn("a", a)])

# ------------------------------------------------------- vectors
topic("vector")
for rhs in ("u + 2*v", "u - v", "2u", "u / 2", "-u", "a u + v", "B - A", "(A + B) / 2",
            "(a, 1)", "u + (1, -1)", "(u + v) / abs(u + v)", "(y(u), -x(u))",
            "Vector(A, B) + u", "(2; a)", "u + (1; pi / 2)", "A - B + u"):
    A, B, u, v, a = rp(), rp(), rp(), rp(), r2(0.5, 4)
    case("vector", [pt("A", *A), pt("B", *B), pt("u", *u), pt("v", *v), num("a", a),
                    f"w = {rhs}"],
         [mv("u", *rp()), mv("v", *rp()), setn("a", r2(-4, 4)), mv("u", 0, 0), mv("B", *A),
          mv("u", *u), mv("B", *rp()), setn("a", a)])
# numbers made from points and vectors
for rhs in ("u * v", "u v", "abs(v)", "abs(A)", "A * B", "x(u) + y(v)", "abs(u - v)",
            "u * (A - B)", "u^2", "(u + v)^2", "abs(B - A) / 2", "x(A + u)",
            "u * v / (abs(u) abs(v))"):
    A, B, u, v = rp(), rp(), rp(), rp()
    case("vector", [pt("A", *A), pt("B", *B), pt("u", *u), pt("v", *v), f"s = {rhs}"],
         [mv("u", *rp()), mv("v", *rp()), mv("A", *rp()), mv("u", 0, 0), mv("B", *rp()),
          mv("u", *u)])

# ------------------------------------------------------- polar coordinates
topic("polar")
for rhs in ("(a; b)", "(a; b°)", "(2; a°)", "(a; pi / 4)", "A + (a; b°)",
            "(abs(A); b°)", "(a; b) + (b; a)", "(-a; b)"):
    case("polar", nums(a=2, b=30) + ["A = (1, 2)", f"P = {rhs}", f"p = {rhs}"],
         [setn("a", 0), setn("b", 90), setn("a", -3), setn("b", 400), mv("A", -2, 5),
          setn("a", 2), setn("b", 30)])

# ------------------------------------------------------- undefined parts
topic("ptundef")
for rhs in ("(1 / a, 1)", "(sqrt(a), a)", "(a, ln(a))", "A + (0, 1 / a)", "a (1, 2) / a",
            "(1, 2) / a", "A / a", "(tan(a), 0)"):
    case("ptundef", nums(a=2) + ["A = (1, 2)", f"P = {rhs}", f"w = {rhs}", "Q = P + (1, 1)",
                                 "d = Distance(P, A)"],
         [setn("a", 0), setn("a", -1), setn("a", 3), setn("a", 0), setn("a", 2)])
case("ptundef", ["A = (0, 0)", "B = (4, 1)", "C = (1, 3)", "D = (2, -2)", "l = Line(A, B)",
                 "m = Line(C, D)", "S = Intersect(l, m)", "P = (S + A) / 2", "u = S - C",
                 "n = abs(u)"],
     [mv("D", 5, 4), mv("D", 3, -1), mv("C", 2, 2)])
case("ptundef", ["A = (1, 1)", "B = (1, 1)", "u = Vector(A, B)", "w = u / abs(u)",
                 "P = A + w"],
     [mv("B", 4, 5), mv("A", 4, 5), mv("A", 0, 0)])

# ------------------------------------------------------- the label decides point or vector
topic("label")
for rhs in ("(A + B) / 2", "A + u", "u + v", "(a, a^2)", "B - A", "2(u - A)"):
    A, B, u, v = rp(), rp(), rp(), rp()
    case("label", [pt("A", *A), pt("B", *B), pt("u", *u), pt("v", *v), num("a", r2(-3, 3)),
                   f"P = {rhs}", f"p = {rhs}", "Q = P + p", "q = p + P"],
         [mv("A", *rp()), mv("u", *rp()), setn("a", r2(-3, 3)), mv("B", *A)])

# ------------------------------------------------------- commands in point and vector expressions
topic("ptcmd")
for extra, rhs in (([], "Midpoint(A, B) + u"), ([], "A + Vector(B, C)"),
                   (["l = Line(A, B)", "m = Line(B, C)"], "Intersect(l, m) + (1, 0)"),
                   ([], "(Distance(A, B); Angle(A, B, C))"), ([], "a UnitVector(u)"),
                   ([], "Translate(A, u) - B"), ([], "(Midpoint(A, C) + Midpoint(B, C)) / 2"),
                   ([], "A + OrthogonalVector(u)"), ([], "(Area(A, B, C), Distance(A, C))"),
                   ([], "Vector(A, B) * Vector(B, C) u")):
    A, B, C, u, a = rp(), rp(), rp(), rp(), r2(0.5, 4)
    for label in ("P", "w"):
        case("ptcmd", [pt("A", *A), pt("B", *B), pt("C", *C), pt("u", *u), num("a", a)]
             + extra + [f"{label} = {rhs}"],
             [mv("A", *rp()), mv("u", *rp()), mv("C", *rp()), mv("B", *A), mv("u", 0, 0),
              setn("a", -a), mv("B", *B)])

cases.write()
