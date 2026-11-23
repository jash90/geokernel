"""Pen strokes: `PenStroke(point, point, ...)` and
`PenStroke(number, number, ...)`, the smoothed stroke with cubic control
points and subpath breaks that the command freezes at creation."""

from caselib import Cases, setn

cases = Cases("stroke")


def case(topic, script, steps=(), view=None):
    cases.case(topic, script.split("\n"), list(steps))
    if view is not None:
        cases.cases[-1]["view"] = view


def reject(line):
    return {"action": "reject", "line": line}


def tuples(points):
    return ", ".join(f"({x}, {y})" for x, y in points)


# ------------------------------------------------------------------ points
# Point literals: an empty stroke, one point, a segment, and smoothed
# strokes of three or more points (long pieces are subdivided first).
cases.topic("points")
case("points", "s = PenStroke()")
case("points", "s = PenStroke((1, 3))")
case("points", "s = PenStroke((1, 1), (2, 2))")
case("points", "s = PenStroke((-3, 3), (4, 3), (2, 5))")
case("points", "s = PenStroke((0, 0), (0.3, 0.2), (0.5, 0.6), (0.6, 0.4))")
# collinear and nearly collinear points stay straight
case("points", "s = PenStroke((0, 0), (1, 0), (2, 0))\nt = PenStroke((0, 0), (0.4, 0.001), (0.8, 0))")
# coordinates are expressions inside the tuple; repeated points
case("points", "s = PenStroke((1 + 1, 2), (3, 2^2), (sqrt(2), -1))")
case("points", "s = PenStroke((1, 1), (1, 1), (1, 1))\nt = PenStroke((1, 1), (1, 1))")
# many points: close neighbours on screen are averaged first
case("points", "s = PenStroke(" + tuples([(round(0.03 * i, 2), round(0.02 * i * i % 1.7, 3))
                                         for i in range(14)]) + ")")
case("points", "s = PenStroke(" + tuples([(round(0.5 * i, 2), round((-1) ** i * 0.05 * i, 3))
                                         for i in range(12)]) + ")")
# huge and tiny coordinates
case("points", "s = PenStroke((1E200, 0), (2E200, 1E200), (3E200, 0))\n"
     "t = PenStroke((0, 0), (1E-9, 2E-9), (3E-9, 0))")

# ----------------------------------------------------------------- numbers
# Consecutive numbers are read as coordinate pairs.
cases.topic("numbers")
case("numbers", "s = PenStroke(1, 1, 2, 2)")
case("numbers", "s = PenStroke(-3, 3, 4, 3, 2, 5)")
case("numbers", "s = PenStroke(1, 3)")
case("numbers", "s = PenStroke(0, 0, (1, 1), 2, 0)")
case("numbers", "s = PenStroke(1, 2)", [reject("t = PenStroke(1, 2, 3)")])

# ------------------------------------------------------------------ breaks
# Undefined coordinates split the stroke into subpaths.
cases.topic("breaks")
case("breaks", "s = PenStroke((0, 0), (1, 1), (?, ?), (2, 0), (3, 1), (4, 0))")
case("breaks", "s = PenStroke(0, 0, 1, 1, ?, ?, 2, 0, 3, 1, 4, 0)")
case("breaks", "s = PenStroke((?, ?), (0, 0), (1, 2), (?, ?), (?, ?), (3, 3), (?, ?))")
case("breaks", "s = PenStroke((0, 0), (?, 1), (1, 1), (2, 0), (1, ?), (5, 5))")
case("breaks", "s = PenStroke((?, ?))\nt = PenStroke(?, ?, ?, ?)")
case("breaks", "s = PenStroke((0, 0), (1 / 0, 1), (1, 1), (2, 3))")

# ------------------------------------------------------------------ frozen
# The coordinates are read once: the stroke reads no objects.
cases.topic("frozen")
case("frozen", "a = 1\ns = PenStroke((1, 2), (3, 4))", [setn("a", 2)])
case("frozen", "A = (1, 2)\nB = (3, 4)\nn = 1",
     [reject("s = PenStroke(A, B)"), reject("t = PenStroke(n, 2)"),
      reject("u = PenStroke(1 + 1, 2)"), reject("v = PenStroke(\"a\", 2)")])
case("frozen", "a = 1\ns = PenStroke((a, 2), (3, 4), (5, 1))", [setn("a", 3)])

# -------------------------------------------------------------------- view
# Subdivision and averaging measure the stroke in pixels of the view.
cases.topic("view")
case("view", "s = PenStroke((-3, 3), (4, 3), (2, 5))", view=[-10, 10, -10, 10, 10, 10])
case("view", "s = PenStroke((-3, 3), (4, 3), (2, 5))", view=[-1, 1, -1, 1, 400, 400])
case("view", "s = PenStroke(" + tuples([(round(0.03 * i, 2), round(0.02 * i * i % 1.7, 3))
                                        for i in range(14)]) + ")",
     view=[-1, 2, -1, 2, 100, 20])
case("view", "s = PenStroke(" + tuples([(round(0.03 * i, 2), round(0.02 * i * i % 1.7, 3))
                                        for i in range(14)]) + ")",
     view=[-1, 2, -1, 2, 20, 300])

# ---------------------------------------------------------------- literals
# Which arguments count as a number or a point literal: names are not
# resolved, so a coordinate that reads one is undefined.
cases.topic("literals")
case("literals", "a = 1\nA = (1, 2)\ns = PenStroke((a + 1, 2), (x(A), 2), (2a, 1), (q, 1), (0, 0), (1, 1))")
case("literals", "s = PenStroke((pi, e), (\"a\", 1), (1, 3), (2, 2))")
case("literals", "s = PenStroke((2; 0), (1; pi / 2), (3, 3))")
case("literals", "s = PenStroke(pi, 1, 2, 2)", [reject("t = PenStroke(-pi, 1, 2, 2)"),
                                                reject("u = PenStroke(2pi, 1, 2, 2)")])
case("literals", "s = PenStroke(.5, 1E1, 2, 2)", [reject("t = PenStroke(30°, 1, 2, 2)")])
case("literals", "s = PenStroke(1, 2)", [reject("t = PenStroke((1, 2, 3), (2, 2))"),
                                         reject("u = PenStroke({1, 2}, 2, 2)"),
                                         reject("v = PenStroke(-(3), 1, 2, 2)")])
case("literals", "s = PenStroke(-?, 1, 2, 2)\nt = PenStroke((-1, -2), (-0.5, 1))")
case("literals", "s = PenStroke(e, 1, ∞, 1, -∞, 2, +1, 1., NaN, 2, −1, - 1)\n"
     "t = PenStroke(°, 1, π, -0, --1, 3)",
     [reject("u = PenStroke(-e, 1, 2, 2)"), reject("v = PenStroke(1e-2, 1, 2, 2)"),
      reject("w = PenStroke(undefined, 1, 2, 2)"), reject("a = PenStroke(1/2, 1, 2, 2)"),
      reject("b = PenStroke(2^2, 1, 2, 2)"), reject("c = PenStroke(pi/2, 1, 2, 2)"),
      reject("d = PenStroke(1°, 1, 2, 2)"), reject("f = PenStroke(-π, 1, 2, 2)")])

# ------------------------------------------------------------------ limits
# A stroke too long for the saved attribute: the reference keeps it.
cases.topic("limits")
case("limits", "s = PenStroke(" + tuples([(round(1.37 * i, 2), round((i % 2) * 1.91, 2))
                                          for i in range(2500)]) + ")")

cases.write()
