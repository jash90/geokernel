"""Cartesian curve tangents from the upstream command tests and edit cases."""

from caselib import Cases, mv, setn


cases = Cases("georesttangent")


def tangent(script, *steps):
    cases.case("curve", script.splitlines(), list(steps))


# CommandsTest.cmdTangent: a point specified by a normalized path parameter.
tangent(
    "c = Curve(sin(t), cos(t), t, 0, 3)\ns = 0.05\nP = Point(c, s)\ng = Tangent(P, c)",
    setn("s", 0),
    setn("s", 1),
)

# CommandsTest.cmdTangent: a point near the upper half of a circle.
tangent(
    "c = Curve(cos(t), sin(t), t, 0, pi)\nP = (0,1)\ng = Tangent(P, c)",
    mv("P", 1, 1),
    mv("P", -1, 2),
)

# CommandsTest.cmdTangent: an external point and a cubic spline.
tangent(
    "l = {(2,3),(1,4),(2,5),(3,1)}\nc = Spline(l)\nP = (1,1)\ng = Tangent(P, c)",
    mv("P", 2, 4),
)

# A vertical tangent at the apex, then an oblique interior contact.
tangent(
    "P = (-1,0)\nc = Curve(t^2, t, t, -2, 2)\ng = Tangent(P, c)",
    mv("P", 1, 1),
    mv("P", -1, 0),
)

# The point and the coordinate functions move independently. The source
# point becomes undefined when the two lines defining it become parallel.
tangent(
    "A = (0,1)\nB = (2,0)\nD = (2,2)\nl = Line(B, D)\nP = Intersect(l, xAxis)\n"
    "c = Curve(t, y(A), t, 0, 4)\ng = Tangent(P, c)",
    mv("A", 0, 3),
    mv("D", 3, 0),
    mv("D", 4, 1),
)

# A stationary parameter has no tangent direction. Leaving it restores the
# line at the new parameter.
tangent(
    "c = Curve(t^2, t^3, t, -1, 1)\ns = 0.25\nP = Point(c, s)\ng = Tangent(P, c)",
    setn("s", 0.5),
    setn("s", 0.75),
)

# A point at the midpoint of the line's normalized interval tracks the
# tangency point even while the line's coefficients remain unchanged.
tangent(
    "P = (1,1)\nc = Curve(t, 0, t, -5, 5)\ng = Tangent((x(P), y(P)), c)\ns = 0.5\nQ = Point(g, s)",
    mv("P", 2, 1),
    mv("P", 2, -2),
)

# A point on a curve tracks the parameter assigned during dragging.
tangent(
    "c = Curve(t, 0, t, -5, 5)\nP = Point(c)\ng = Tangent(P, c)",
    mv("P", 1, 2),
    mv("P", -1, 2),
)

# CommandsTest.cmdTangent: an inline point supplies the x coordinate for a
# function tangent. Move a free point read by that same inline expression.
cases.case(
    "function",
    ["f(x) = sin(x)", "A = (0,0)", "g = Tangent((1 + x(A), 1), f)"],
    [mv("A", 1, 2), mv("A", -1, 1)],
)

cases.write()
