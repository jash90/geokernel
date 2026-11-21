#!/usr/bin/env python3
"""Writes cases/equations.json: lines and conics typed as equations in the
input bar, `label: lhs = rhs`, exactly as GeoGebra reads them.

The helpers live in caselib.py. Snapshots come only from running GeoGebra
through the harness (`harness.sh golden equations`).

Every equation names its label before the `:`. A linear equation (degree 0
or 1 of its normal form) makes a line, a quadratic one a conic, and
`y = <rhs in x>` of any other kind a function; the cases record the raw
line coefficients and the conic matrices GeoGebra keeps. Equations whose
coefficients read objects (`y = m x + q`, `x^2 + y^2 = r^2`) are dependent
objects: the steps change the numbers and move the points they read.

Degenerate states (a degree 0 equation, a conic that falls into two lines,
a point or nothing) are reached at creation and by number steps. No case
may need an implicit curve or an inequality: GeoGebra makes those, the
kernel does not.

Labels avoid `e`, `i`, `x`, `y`, `z` and letter-digit names; points have
upper-case labels.
"""

from caselib import Cases, mv, num, pt, setn

cases = Cases("equations")
case = cases.case
topic = cases.topic

# ----------------------------------------------------------------- lines
# A linear equation makes a line with the raw coefficients of its normal
# form `lhs - rhs`: `2x + 3y = 6` is the line (2, 3, -6).
topic("lines")
case("lines", ["f: 2x + 3y = 6"], [])
case("lines", ["g: y = 2x + 1"], [])
case("lines", ["h: x = 3"], [])
case("lines", ["k: 3x - y + 2 = 0"], [])
case("lines", ["l: 2 = y", "f: x + y = 0"], [])
case("lines", ["f: 2(x + y) = 6", "g: x - 2y = 1"], [])
case("lines", ["f: sqrt(2) x + y = 1"], [])
case("lines", ["f: y = 2 x + 1 x", "g: y = 3 x"], [])
# a degree 0 equation makes the undefined line (0, 0, c)
case("lines", ["f: 0x + 0y = 1"], [])
case("lines", ["f: 0x + y = 2"], [])
# equations whose coefficients read objects: the line follows every change
case("lines", [num("m", 2), num("q", 1), "g: y = m x + q"],
     [setn("m", 3), setn("q", -2), setn("m", 0), setn("m", -1.5)])
case("lines", [num("m", 1), "g: m x = 2"],
     [setn("m", 0), setn("m", 1), setn("m", -4)])
case("lines", [num("q", 1), "g: q x + 2y = q + 4"],
     [setn("q", 0), setn("q", -3.5)])
case("lines", [num("m", 1), "g: y = m x"],
     [setn("m", 0), setn("m", -1), setn("m", 0.5)])
case("lines", [num("m", 2), num("q", -3), "g: m (x + q) = m y"],
     [setn("m", -1), setn("q", 0), setn("m", 3)])
case("lines", [pt("A", 1, 5), "g: x(A) + y = 2"],
     [mv("A", 3, 0), mv("A", -1, -4)])
case("lines", [pt("A", 2, -1), "g: x(A) x + y(A) y = 2"],
     [mv("A", 1, 1), mv("A", -3, 0.5)])
case("lines", [pt("A", 1, 1), pt("B", 4, 5), "g: (y(A) - y(B)) x + (x(B) - x(A)) y = 3"],
     [mv("A", 0, 0), mv("B", 2, 2), mv("A", 1, 1)])
case("lines", [pt("A", 1, 1), num("q", 2), "g: y = x(A) + q"],
     [mv("A", 5, 0), setn("q", -1)])
case("lines", [num("m", 1), num("q", 1), "g: y = m x + q", "h: y = -1 / m x + q"],
     [setn("m", 2), setn("q", -1), setn("m", -0.5)])
# the terms one monomial sums to cancel: every merged term goes, also when
# the sum folds to the constant 0, so `x = x` is the undefined line (0, 0, 0)
# and `x = x + 1` the undefined line (0, 0, -1)
case("lines", ["f: x = x", "g: x + y = x + y", "h: x = x + 1",
               "k: 2x + y = 2x + 5"], [])
case("lines", ["f: x^2 - x^2 + y = 1"], [])
case("lines", [num("a", 1), "f: x + a = x + 2"],
     [setn("a", 2), setn("a", 0)])
# a function call whose argument is `x` is the body of the function, so
# `h(x) = y` is the line `y = 2x`; the line follows the objects h reads
case("lines", ["h(x) = 2x", "f: h(x) = y", "g: y + 1 = h(x)"], [])
case("lines", ["h(x) = x^2", "f: x^2 + y = h(x)"], [])
case("lines", [num("w", 2), "h(x) = w x", "f: h(x) = y"],
     [setn("w", -3), setn("w", 0)])
# x(...) and y(...) applied to something that is not a point or a vector is
# the product with the variable (`replaceXYZnodes`)
case("lines", ["f: x(3) = 1", "g: y(5) = 1"], [])
# a call whose argument is a constant stays one constant coefficient, so
# the equation keeps its degree: `y = h(2)` is the line y = 4 and
# `y = h(1) x` the line y = 2x
case("lines", ["h(x) = 2x", "f: y = h(2)", "g: y = h(1) x"], [])

# ---------------------------------------------------------------- conics
# A quadratic equation makes a conic with the coefficients of
# `a x^2 + b xy + c y^2 + dx + ey + f = 0`; the classification may scale
# the matrix by a power of two.
topic("conics")
case("conics", ["c: x^2 + y^2 = 9"], [])
case("conics", ["c: (x - 1)^2 + (y + 2)^2 = 4"], [])
case("conics", ["c: x^2/4 + y^2/9 = 1"], [])
case("conics", ["c: y = x^2 - 3"], [])
case("conics", ["c: x y = 1"], [])
case("conics", ["c: 3x^2 - 2x y + y^2 + x - 5 = 0"], [])
case("conics", ["c: x^2 - 4y^2 = 4"], [])
case("conics", ["c: y^2 = 4x", "d: x^2 = 4y"], [])
case("conics", ["c: (x + y)^2 + x = 1"], [])
case("conics", ["c: 2(x^2 + y^2) = 8", "d: (2x - 1)^2 + (2y + 3)^2 = 16"], [])
# coefficients that read objects: the conic follows every change
case("conics", [num("r", 3), "c: x^2 + y^2 = r^2"],
     [setn("r", 5), setn("r", 0), setn("r", 1), setn("r", -2)])
case("conics", [num("r", 3), num("s", 1), "c: x^2 / r^2 + y^2 / s^2 = 1"],
     [setn("s", 2), setn("r", 1), setn("s", 0.5), setn("r", 4)])
case("conics", [num("p", 2), "c: y^2 = 2 p x"],
     [setn("p", 1), setn("p", 0), setn("p", -1), setn("p", 0.25)])
case("conics", [num("m", 1), "c: y = m x^2"],
     [setn("m", -1), setn("m", 0), setn("m", 0.5)])
case("conics", [num("s", 1), "c: x^2 + s y^2 = s"],
     [setn("s", 2), setn("s", 0.5), setn("s", -1)])
case("conics", [pt("A", 1, -2), "c: (x - x(A))^2 + (y - y(A))^2 = 4"],
     [mv("A", -3, 1), mv("A", 0, 0)])
case("conics", [pt("A", 0, 0), "c: x^2 + y^2 + x(A) x + y(A) y = 4"],
     [mv("A", 2, 0), mv("A", -1, 3), mv("A", 0, -2)])
case("conics", [num("r", 2), pt("A", 1, 1), "c: (x - x(A))^2 + (y - y(A))^2 = r^2"],
     [setn("r", 4), mv("A", -2, -1), setn("r", 0), setn("r", 3)])
# the terms one monomial sums to cancel across the two sides
case("conics", ["c: x^2 + y^2 = 1 + x^2", "d: (x + 1)(x - 1) = y",
                "e: (x + y)(x - y) = 1"], [])
# a digit-prefixed name of letters is a product of single letters (`2xy` is
# `2*x*y`), and x(...)/y(...) of a non-point multiplies
case("conics", ["c: 2xy = 1", "d: 2 xy = 3", "e: x(y) = 1", "f: x(y + 1) = 1",
                "g: y = x(x + 1)"], [])

# ------------------------------------------------------------- degenerate
# Conics that fall apart: two crossing lines, a double line, parallel
# lines, a single point, nothing; and the undefined degree 0 line.
topic("degenerate")
case("degenerate", ["c: x^2 - y^2 = 0"], [])
case("degenerate", ["c: x^2 = 4"], [])
case("degenerate", ["c: x^2 - 2x y + y^2 = 0"], [])
case("degenerate", ["c: (x + 1)^2 + y^2 = 0"], [])
case("degenerate", ["c: x^2 + y^2 = -4"], [])
case("degenerate", ["c: x^2 + 2x y + y^2 = 1", "d: (x + y)^2 - 1 = 0"], [])
case("degenerate", [num("r", 3), "c: x^2 + y^2 = r^2", "f: 0x + 0y = 1"],
     [setn("r", 0), setn("r", 1), setn("r", 0)])
case("degenerate", [num("m", 1), "c: x^2 - m y^2 = 0"],
     [setn("m", 4), setn("m", 0), setn("m", -1)])
case("degenerate", [num("s", 1), "c: x^2 + s y^2 = 0"],
     [setn("s", -1), setn("s", 0), setn("s", -4)])
case("degenerate", [num("q", 0), "c: y^2 = q^2"],
     [setn("q", 2), setn("q", 0), setn("q", -3)])
case("degenerate", [num("m", 1), num("q", 1), "c: m x^2 + q y^2 = 1"],
     [setn("q", 0), setn("m", 0), setn("q", -1), setn("m", 2)])
# the cancelled terms make the conic fall apart: `y = x^2 + y` is the double
# line x^2 = 0 and the merged x^2 terms leave two crossing lines
case("degenerate", ["c: y = x^2 + y", "d: x^2 + 2x y + y^2 = x^2"], [])

# --------------------------------------------------------------- functions
# `y = <rhs in x>` that is no line and no conic makes a function, like
# `f(x) = ...`; the samples of golden/SCHEMA.md record its values.
topic("functions")
case("functions", ["f: y = sin(x)"], [])
case("functions", ["f: y = x^3 - x"], [])
case("functions", ["f: y = 2^x"], [])
case("functions", ["f: y = sqrt(x + 4)"], [])
case("functions", [num("m", 1), "f: y = m x + sin(x)"],
     [setn("m", 2), setn("m", -1), setn("m", 0)])
case("functions", [num("q", 1), "f: y = abs(x) + q"],
     [setn("q", -1), setn("q", 3)])
# a variable exponent is fine in the function form (the equation form of it
# would be an implicit polynomial, which the kernel refuses)
case("functions", [num("n", 2), "f: y = x^n"],
     [setn("n", 3), setn("n", 1)])
# a function applied to an argument that reads x makes the equation
# function dependent (`isFunctionDependent`), so `y = <rhs>` is a function
# however polynomial the body of the function expands to: `y = h(x)` with
# `h(x) = x^2` is a function, not a parabola, and `y = h(x)` with
# `h(x) = 2x` a function, not a line
case("functions", ["h(x) = x^2", "f: y = h(x)"], [])
case("functions", ["h(x) = x^2", "f: y = h(x) + 1"], [])
case("functions", ["h(x) = 2x", "f: y = h(x)"], [])
# through a second function: the body of g applies h to its own variable
case("functions", ["h(x) = x + 1", "g(x) = h(x)^2", "f: y = g(x)"], [])
# the function reads an object: the function follows its steps, also when
# the body empties or the equation gains a plain polynomial term
case("functions", [num("a", 2), "h(x) = a x^2", "f: y = h(x) + x"],
     [setn("a", -1), setn("a", 0)])
case("functions", [num("a", 2), "h(x) = a x", "f: y = h(x)"],
     [setn("a", 0), setn("a", -3)])

# ------------------------------------------------------------------ mixed
# Equations together with the rest of a construction: other objects read
# the line and the conic they make.
topic("mixed")
case("mixed", [pt("A", 1, 1), "g: x + y = 4", "d = Distance(A, g)"],
     [mv("A", 4, 4), mv("A", 0, 0)])
case("mixed", [pt("P", 5, 1), "c: x^2 + y^2 = 4", "t = Tangent(P, c)"],
     [mv("P", 2, 0), mv("P", 5, 1), mv("P", -3, -3)])
case("mixed", [pt("A", 0, 0), pt("B", 4, 0), "g: y = 0", "c: x^2 + y^2 = 4",
               "k: x^2 + y^2 = 16"],
     [mv("A", 1, 1), mv("B", -2, 3)])
case("mixed", [num("m", 1), num("q", -1), "g: y = m x + q", "h: y = 2 x",
               "d = Distance(g, h)"],
     [setn("m", 2), setn("q", 1), setn("m", 0.5)])

cases.write()
