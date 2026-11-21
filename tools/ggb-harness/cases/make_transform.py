#!/usr/bin/env python3
"""Writes cases/transform.json: Mirror/Reflect, Rotate, Dilate, Shear and Stretch.

Every command is applied to a point, a line, a segment and a circle (given by a
centre and a point, and by a centre and a radius), with every syntax. The steps
move the defining points of the object and of the mirror, make the mirror
undefined and defined again, and drive the angle and the factor through 0, pi,
2 pi, negative and large values.

Use only the built-in commands as parents; the images are leaves or feed one
more transformation (chains).
"""

import math

from caselib import Cases, mv, num, pt, setn

cases = Cases("transform")
case = cases.case
topic = cases.topic
rp = cases.rp
r2 = cases.r2


def vec(label, x, y):
    """A free vector: the same literal as a point, with a lower-case label."""
    return pt(label, x, y)


def vec(label, x, y):
    """A free vector: the same literal as a point, with a lower-case label."""
    return pt(label, x, y)


def subject(kind, label_prefix=""):
    """Script lines and the label of an object of the given kind, plus the
    labels of its free points. Kinds: point, line, segment, circle, circle_r."""
    if kind == "point":
        return [pt("P", *rp())], "P", ["P"]
    a, b = rp(), rp()
    lines = [pt("A", *a), pt("B", *b)]
    if kind == "line":
        return lines + ["l = Line(A, B)"], "l", ["A", "B"]
    if kind == "segment":
        return lines + ["s = Segment(A, B)"], "s", ["A", "B"]
    if kind == "circle":
        return lines + ["k = Circle(A, B)"], "k", ["A", "B"]
    if kind == "circle_r":
        return [pt("A", *a), num("r", round(abs(r2(1.0, 5.0)), 2)), "k = Circle(A, r)"], "k", ["A"]
    if kind == "ray":
        return lines + ["g = Ray(A, B)"], "g", ["A", "B"]
    if kind == "rayvec":
        u = (round(r2(-4, 4), 2), round(r2(-4, 4), 2))
        return lines + [vec("u", *u), "g = Ray(A, u)"], "g", ["A", "u"]
    if kind == "vector":
        return lines + ["b = Vector(A, B)"], "b", ["A", "B"]
    if kind == "polygon":
        return lines + [pt("E", *rp()), "p = Polygon(A, B, E)"], "p", ["A", "B", "E"]
    if kind == "polyline":
        return lines + [pt("E", *rp()), "t = PolyLine(A, B, E)"], "t", ["A", "B", "E"]
    if kind == "arc":
        return lines + [pt("E", *rp()), "a = CircularArc(A, B, E)"], "a", ["A", "B", "E"]
    if kind == "sector":
        return lines + [pt("E", *rp()), "e = CircularSector(A, B, E)"], "e", ["A", "B", "E"]
    if kind == "ellipse":
        return lines + [pt("F", *rp()), "c = Ellipse(A, F, B)"], "c", ["A", "B", "F"]
    raise ValueError(kind)


KINDS = ["point", "line", "segment", "circle", "circle_r", "ray", "rayvec", "vector",
         "polygon", "polyline", "arc", "sector", "ellipse"]

# The kinds `Shear` and `Stretch(obj, line, f)` accept (no polyline:
# GeoGebra rejects it too).
MATRIX_KINDS = ["point", "line", "segment", "circle", "ray", "rayvec", "vector",
                "polygon", "arc", "sector", "ellipse"]


def object_steps(free):
    """Moves of the object's own free points, ending near the start."""
    steps = [mv(free[0], *rp())]
    if len(free) > 1:
        steps.append(mv(free[1], *rp()))
        steps.append(mv(free[0], *rp()))
    return steps


# ------------------------------------------------------- mirror in a point
topic("mirrorpoint")
for kind in KINDS:
    for name in ("Mirror", "Reflect"):
        script, obj, free = subject(kind)
        script += [pt("Q", *rp()), f"m = {name}({obj}, Q)"]
        steps = [mv("Q", *rp())] + object_steps(free)
        # the mirror point onto a defining point of the object
        steps.append(mv("Q", *rp(-8, 8)))
        case("mirrorpoint", script, steps)

for kind in ("point", "line", "circle"):
    script, obj, free = subject(kind)
    # the mirror point is the midpoint of a segment, so it follows two points
    script += [pt("C", *rp()), pt("D", *rp()), "Q = Midpoint(C, D)", f"m = Mirror({obj}, Q)"]
    case("mirrorpoint", script, [mv("C", *rp()), mv("D", *rp()), mv("D", *rp(-1, 1))])

# ------------------------------------------------------- mirror in a line
topic("mirrorline")
for kind in KINDS:
    script, obj, free = subject(kind)
    script += [pt("C", *rp()), pt("D", *rp()), "n = Line(C, D)", f"m = Mirror({obj}, n)"]
    steps = [mv("C", *rp()), mv("D", *rp())] + object_steps(free)
    # coincident points make the mirror undefined, then separate them again
    steps += [mv("D", *rp(-8, 8)), mv("C", *rp()), mv("D", *rp())]
    case("mirrorline", script, steps)

for kind in KINDS:
    script, obj, free = subject(kind)
    script += [pt("C", *rp()), pt("D", *rp()), "n = Line(C, D)", f"m = Reflect({obj}, n)"]
    case("mirrorline", script, [mv("C", *rp(-3, 3)), mv("D", *rp(-3, 3)), mv("D", 0.0, 0.0),
                                mv("C", 0.0, 0.0), mv("D", 4.0, 0.0), mv("C", *rp())])

# the mirror is a segment
for kind in ("point", "line", "segment", "circle"):
    script, obj, free = subject(kind)
    script += [pt("C", *rp()), pt("D", *rp()), "n = Segment(C, D)", f"m = Mirror({obj}, n)"]
    case("mirrorline", script, [mv("C", *rp()), mv("D", *rp()), mv("C", 1.0, 1.0),
                                mv("D", 1.0, 1.0), mv("D", *rp())])

# axis-parallel mirrors, a mirror through the origin and a horizontal and a
# vertical mirror (the branch on the larger of the two coefficients)
for c, d in (((0, 1), (5, 1)), ((2, -3), (2, 4)), ((0, 0), (3, 3)), ((0, 0), (-2, 5)),
             ((1, 1), (4, 3)), ((-3, 2), (-1, 7))):
    script, obj, free = subject("line")
    script += [pt("C", *c), pt("D", *d), "n = Line(C, D)", "m = Mirror(l, n)"]
    case("mirrorline", script, [mv("A", *c), mv("B", *rp()), mv("A", *rp())])

for c, d in (((0, 1), (5, 1)), ((2, -3), (2, 4)), ((0, 0), (3, 3))):
    script, obj, free = subject("segment")
    script += [pt("C", *c), pt("D", *d), "n = Line(C, D)", "m = Mirror(s, n)"]
    case("mirrorline", script, [mv("A", *c), mv("B", *rp()), mv("A", *rp())])

# --------------------------------------------------- inversion of a point
topic("invert")
for _ in range(4):
    script = [pt("O", *rp(-2, 2)), pt("R", *rp(-5, 5)), "k = Circle(O, R)",
              pt("P", *rp(-6, 6)), "m = Mirror(P, k)"]
    case("invert", script, [mv("P", *rp(-6, 6)), mv("R", *rp(-5, 5)), mv("O", *rp(-3, 3)),
                            mv("P", *rp(-6, 6))])

# a point at the centre (the image is at infinity), then away again
script = [pt("O", 1, 2), pt("R", 4, 2), "k = Circle(O, R)", pt("P", 3, 3), "m = Mirror(P, k)"]
case("invert", script, [mv("P", 1, 2), mv("P", 2, 2), mv("P", 1.000001, 2), mv("P", 3, 3)])

# a point on the circle is its own image
script = [pt("O", 0, 0), pt("R", 5, 0), "k = Circle(O, R)", pt("P", 0, 5), "m = Reflect(P, k)"]
case("invert", script, [mv("P", 3, 4), mv("P", 5, 0), mv("P", -5, 0), mv("P", 1, 1)])

# the circle shrinks to a single point and grows again
script = [pt("O", 0, 0), pt("R", 3, 0), "k = Circle(O, R)", pt("P", 2, 2), "m = Mirror(P, k)"]
case("invert", script, [mv("R", 0, 0), mv("R", 1e-9, 0), mv("R", 0, 2), mv("R", 3, 0)])

# a circle given by a radius, including 0 and a negative radius
script = [pt("O", 1, 1), num("r", 2), "k = Circle(O, r)", pt("P", 4, 1), "m = Mirror(P, k)"]
case("invert", script, [setn("r", 0), setn("r", 3), setn("r", -1), setn("r", 2),
                        setn("r", 1e-9), setn("r", 5), mv("P", 1, 1), mv("P", 5, 5)])

# the image of an image
script = [pt("O", 0, 0), pt("R", 4, 0), "k = Circle(O, R)", pt("P", 1, 2), "m = Mirror(P, k)",
          "n = Mirror(m, k)"]
case("invert", script, [mv("P", 3, 1), mv("O", 1, 1), mv("P", 1, 1), mv("P", 7, -2)])

# the circle is itself the image of a transformation
script = [pt("O", 0, 0), pt("R", 4, 0), pt("S", 1, 1), "k = Circle(O, R)",
          "c = Mirror(k, S)", pt("P", 1, 2), "m = Mirror(P, c)"]
case("invert", script, [mv("P", 3, 1), mv("S", -2, 3), mv("R", 2, 2)])

# ------------------------------------------------------------------ rotate
topic("rotate")
ANGLES = [0.0, math.pi, 2 * math.pi, -1.1, 100.0, math.pi / 2, -math.pi / 2, 3.0e-9, 7.5]
for kind in KINDS:
    script, obj, free = subject(kind)
    script += [num("w", 0.7), f"m = Rotate({obj}, w)"]
    steps = [setn("w", v) for v in ANGLES] + object_steps(free)
    case("rotate", script, steps)

for kind in KINDS:
    script, obj, free = subject(kind)
    script += [num("w", 1.2), pt("Q", *rp()), f"m = Rotate({obj}, w, Q)"]
    steps = [setn("w", math.pi), mv("Q", *rp()), setn("w", 2 * math.pi), setn("w", -2.3),
             setn("w", 0.0), setn("w", math.pi / 2)] + object_steps(free)
    steps += [mv("Q", *rp(-8, 8)), setn("w", 40.0)]
    case("rotate", script, steps)

# the centre of rotation on a defining point of the object
for kind in ("point", "line", "segment", "circle"):
    script, obj, free = subject(kind)
    script += [num("w", 0.9), pt("Q", *rp()), f"m = Rotate({obj}, w, Q)"]
    case("rotate", script, [mv("Q", *rp()), mv(free[0], 0.0, 0.0), mv("Q", 0.0, 0.0),
                            setn("w", math.pi / 2), mv("Q", 3.0, 4.0)])

# a rotation of a rotation, and an angle that is the same object twice
script, obj, free = subject("segment")
script += [num("w", 0.4), pt("Q", *rp()), "m = Rotate(s, w, Q)", "n = Rotate(m, w)"]
case("rotate", script, [setn("w", 1.5), mv("Q", *rp()), mv("A", *rp()), setn("w", -0.4)])

# --------------------------------------------------------------------- dilate
topic("dilate")
FACTORS = [1.0, 0.0, -1.0, -2.5, 0.5, 100.0, 0.0, 2.0]
for kind in KINDS:
    script, obj, free = subject(kind)
    script += [num("f", 2), f"m = Dilate({obj}, f)"]
    steps = [setn("f", v) for v in FACTORS] + object_steps(free)
    case("dilate", script, steps)

for kind in KINDS:
    script, obj, free = subject(kind)
    script += [num("f", 1.5), pt("Q", *rp()), f"m = Dilate({obj}, f, Q)"]
    steps = [setn("f", 0.0), setn("f", -1.0), mv("Q", *rp()), setn("f", 3.0), setn("f", 1.0),
             setn("f", -0.25)] + object_steps(free)
    steps += [mv("Q", *rp(-8, 8)), setn("f", 2.0)]
    case("dilate", script, steps)

# the centre of dilation on a defining point of the object or at the origin
for kind in ("point", "line", "segment", "circle"):
    script, obj, free = subject(kind)
    script += [num("f", 2.5), pt("Q", *rp()), f"m = Dilate({obj}, f, Q)"]
    case("dilate", script, [mv("Q", *rp()), mv(free[0], 0.0, 0.0), mv("Q", 0.0, 0.0),
                            setn("f", -2.0), mv("Q", 3.0, 4.0), setn("f", 0.0)])

# a circle shrinking through zero, and the factor reaching tiny values
script = [pt("A", 1, 1), num("r", 2), "k = Circle(A, r)", num("f", 3), "m = Dilate(k, f)"]
case("dilate", script, [setn("r", 0), setn("f", 1.0), setn("r", 4), setn("f", -3.0),
                        setn("r", 0), setn("r", 1)])
script = [pt("A", 1, 1), pt("B", 4, 5), "k = Circle(A, B)", num("f", 3), pt("Q", 2, 0),
          "m = Dilate(k, f, Q)"]
case("dilate", script, [mv("B", 1, 1), setn("f", 0.0), setn("f", 2.0), mv("B", 3, 3),
                        setn("f", 1e-3), setn("f", 2.0)])

# small circles and tiny factors
script = [pt("A", 1, 1), num("r", 2), "k = Circle(A, r)", num("f", 2), pt("Q", 3, -1),
          "m = Dilate(k, f, Q)"]
case("dilate", script, [setn("r", 0.001), setn("r", 0.0001), setn("r", 0.0002),
                        setn("r", 0), setn("f", 0.0), setn("r", 2), setn("f", -0.01),
                        setn("f", 0.001), setn("r", 0.5)])
script = [pt("A", 1, 1), pt("B", 1.001, 1), "k = Circle(A, B)", num("f", 3), "m = Dilate(k, f)"]
case("dilate", script, [mv("B", 1.01, 1), setn("f", -2.0), mv("B", 1, 1), setn("f", 0.0)])

# dilation of a dilation
script, obj, free = subject("line")
script += [num("f", 2), pt("Q", *rp()), "m = Dilate(l, f, Q)", "n = Dilate(m, f)"]
case("dilate", script, [setn("f", -1.5), mv("Q", *rp()), mv("A", *rp()), setn("f", 0.0),
                        setn("f", 0.5)])

# ------------------------------------------------------- shear and stretch
RATIOS = [0.0, 1.0, -1.0, 2.5, -0.5, 7.0]
for name in ("shear", "stretch"):
    topic(name)
    cmd = name.capitalize()
    for kind in MATRIX_KINDS:
        script, obj, free = subject(kind)
        script += [pt("C", *rp()), pt("D", *rp()), "n = Line(C, D)", num("f", 2),
                   f"m = {cmd}({obj}, n, f)"]
        steps = [setn("f", v) for v in RATIOS] + [mv("C", *rp()), mv("D", *rp())]
        steps += object_steps(free)
        # coincident points make the axis undefined, then separate them again
        steps += [mv("D", *rp(-8, 8)), mv("C", *rp()), mv("D", *rp())]
        case(name, script, steps)
    for kind in MATRIX_KINDS:
        script, obj, free = subject(kind)
        script += [pt("C", *rp()), pt("D", *rp()), "n = Segment(C, D)", num("f", -1.5),
                   f"m = {cmd}({obj}, n, f)"]
        case(name, script, [mv("C", *rp()), setn("f", 0.5), mv("D", *rp()), mv("C", 1.0, 1.0),
                            mv("D", 1.0, 1.0), mv("D", *rp()), setn("f", 3.0)])
    # horizontal, vertical and diagonal axes, an axis through the origin
    for c, d in (((0, 2), (4, 2)), ((3, -1), (3, 5)), ((0, 0), (2, 2)), ((0, 0), (-3, 1)),
                 ((1, 1), (5, 2))):
        for kind in MATRIX_KINDS:
            script, obj, free = subject(kind)
            script += [pt("C", *c), pt("D", *d), "n = Line(C, D)", num("f", 1.7),
                       f"m = {cmd}({obj}, n, f)"]
            case(name, script, [setn("f", -2.0), mv(free[0], *c), mv(free[0], *rp()),
                                setn("f", 0.0)])

# ---------------------------------------------------------------- chains
topic("chain")
script = [pt("A", 1, 2), pt("B", 5, 3), pt("C", -2, 4), pt("D", 3, -3), "l = Line(A, B)",
          "n = Line(C, D)", num("w", 0.8), num("f", 1.5), "m = Mirror(l, n)",
          "t = Rotate(m, w, A)", "u = Dilate(t, f, B)"]
case("chain", script, [mv("A", 0, 0), mv("C", 2, 2), setn("w", -1.0), setn("f", -2.0),
                       mv("D", 2, 2), mv("D", 3, -3), setn("f", 0.0)])
script = [pt("A", 1, 2), pt("B", 5, 3), pt("Q", 0, 1), "s = Segment(A, B)", num("w", 0.8),
          num("f", -0.5), "m = Rotate(s, w, Q)", "n = Dilate(m, f)", "t = Mirror(n, Q)"]
case("chain", script, [mv("A", 4, 4), mv("B", 4, 4), mv("B", 0, 5), setn("w", 3.0),
                       mv("Q", 3, 3), setn("f", 2.0)])
script = [pt("O", 0, 0), pt("R", 3, 0), pt("C", -2, 4), pt("D", 3, -3), "k = Circle(O, R)",
          "n = Line(C, D)", num("w", 0.8), "m = Mirror(k, n)", "t = Rotate(m, w)",
          "p = Mirror(O, n)"]
case("chain", script, [mv("O", 1, 1), mv("R", 5, 1), mv("D", -2, 4), mv("D", 0, 0),
                       mv("C", 0, 0), mv("C", 4, 1)])
script = [pt("A", 1, 2), pt("B", 5, 3), pt("C", 2, 6), "l = Line(A, B)", "m = Mirror(C, l)",
          "n = Mirror(m, l)", "p = Mirror(l, C)", "q = Mirror(p, C)"]
case("chain", script, [mv("C", 3, 2.5), mv("A", 3, 2.5), mv("B", 3, 2.5), mv("B", 0, 0)])

# ------------------------------------------------------------- recorded edge cases
# Mirror in a line or segment whose end points are 1e-6 apart.
topic("mirrorline")
for kind in ("point", "line", "segment", "circle"):
    for mirror in ("Line", "Segment"):
        script, obj, free = subject(kind)
        script += [pt("C", 1, 1), pt("D", 1.000001, 1), f"n = {mirror}(C, D)", f"m = Mirror({obj}, n)"]
        case("mirrorline", script, [mv("D", 1, 1.000001), mv("D", 1.000001, 1.000001),
                                    mv("C", 0, 0), mv("D", 0.000001, 0), mv("D", 3, 4)])

# Rotation by +-pi/2 about and of points with large coordinates.
topic("rotate")
for kind in ("point", "circle", "circle_r"):
    script, obj, free = subject(kind)
    script += [num("w", 0.3), pt("Q", 100000.5, -200000.25), f"m = Rotate({obj}, w, Q)"]
    steps = [mv(free[0], 123456.789, -98765.4321), setn("w", math.pi / 2),
             setn("w", -math.pi / 2), setn("w", math.pi), setn("w", 3 * math.pi / 2),
             mv("Q", 123456.789, -98765.4321), setn("w", math.pi / 2)]
    case("rotate", script, steps)
for w in (math.pi / 2, -math.pi / 2):
    script = [pt("A", 123456.789, 98765.4321), num("r", 3), "k = Circle(A, r)", num("w", w),
              "m = Rotate(k, w)", "n = Rotate(k, w, A)"]
    case("rotate", script, [setn("w", 0.0), setn("w", -w), setn("w", w)])

# Dilation of circles: GeoGebra classifies the image from the conic matrix,
# so very small circles and extreme factors can become a single point.
topic("dilate")
CIRCLE_REGIMES = [
    # (initial r, initial f, steps)
    (4e-5, 2.0, [setn("r", 5e-5), setn("r", 1e-4), setn("f", 3.0), setn("f", 10.0), setn("r", 2.0)]),
    (5e-5, 0.5, [setn("r", 4e-5), setn("r", 1e-4), setn("r", 9e-5), setn("f", 0.01),
                 setn("r", 5e-5), setn("f", 1.0), setn("r", 2.0)]),
    (2.0, 1e-7, [setn("f", 1e-6), setn("f", 1e-5), setn("f", 1e-4), setn("f", 1e-3),
                 setn("f", 0.003), setn("f", 0.004), setn("f", 1.0), setn("f", -1e-7)]),
    (1e-6, 50.0, [setn("f", 100.0), setn("r", 1e-5), setn("f", 30.0), setn("r", 1e-3),
                  setn("f", -50.0), setn("f", 0.5)]),
    (1e-4, 0.5, [setn("r", 2e-4), setn("f", -0.5), setn("f", 0.0032), setn("f", 0.0031),
                 setn("f", 0.0), setn("f", 1.0)]),
    (1.0, 150.0, [setn("f", 1000.0), setn("f", 120.0), setn("f", 100.0), setn("f", 99.0),
                  setn("r", 0.001), setn("f", -150.0), setn("f", 2.0)]),
]
for centre in (False, True):
    for r0, f0, steps in CIRCLE_REGIMES:
        script = [pt("A", 1, 1), num("r", r0), "k = Circle(A, r)", num("f", f0)]
        if centre:
            script += [pt("Q", 3, -1), "m = Dilate(k, f, Q)"]
            steps2 = steps + [mv("Q", -4, 2), mv("A", 5, 5)]
        else:
            script += ["m = Dilate(k, f)"]
            steps2 = steps + [mv("A", 5, 5)]
        case("dilate", script, steps2)

# A dilated circle that is dilated, mirrored, rotated and inverted again.
for r0, f0 in ((4e-5, 2.0), (5e-5, 0.5), (2.0, 1e-7), (3.0, 2.0)):
    script = [pt("A", 1, 1), num("r", r0), "k = Circle(A, r)", num("f", f0), pt("Q", 3, -1),
              num("g", 2.0), num("w", 0.7), "m = Dilate(k, f, Q)", "n = Dilate(m, g)",
              "p = Mirror(m, Q)", "t = Rotate(m, w)", pt("P", 2, 5), "u = Mirror(P, m)"]
    case("dilate", script, [setn("g", 0.5), setn("g", -3.0), setn("w", math.pi / 2),
                            setn("f", 1.5), setn("r", 2.0), setn("g", 2.0), mv("P", 1, 1)])

# A line dilated by a tiny factor (GeoGebra tests its raw coefficients, which
# scale with the length of the defining segment) and short lines.
for a, b, tag in (((0, 0), (30, 40), "long"), ((0, 0), (3, 4), "mid"), ((0, 0), (0.3, 0.4), "short"),
                  ((10, 10), (10.003, 10.004), "tiny")):
    for centre in (False, True):
        script = [pt("A", *a), pt("B", *b), "l = Line(A, B)", num("f", 2.0)]
        if centre:
            script += [pt("Q", 1, 2), "m = Dilate(l, f, Q)"]
        else:
            script += ["m = Dilate(l, f)"]
        case("dilate", script, [setn("f", 5e-9), setn("f", 1e-8), setn("f", 5e-8),
                                setn("f", 1e-7), setn("f", -1e-8), setn("f", 1e-6), setn("f", 0.0),
                                setn("f", 2.0)])

# Shear and stretch of a line that is vertical or nearly vertical.
for name in ("shear", "stretch"):
    topic(name)
    cmd = name.capitalize()
    for a, b in (((3, -1), (3, 5)), ((0, -50), (0.000001, 50)), ((3, 0), (3.000001, 0.5)),
                 ((3, -50), (3.000001, 50)), ((0, 0), (0, 80))):
        script = [pt("A", *a), pt("B", *b), "l = Line(A, B)", pt("C", 0, 1), pt("D", 4, 3),
                  "n = Line(C, D)", num("f", 1.5), f"m = {cmd}(l, n, f)"]
        case(name, script, [setn("f", -2.0), setn("f", 0.0), mv("C", 3, 0), mv("D", 3, 6),
                            setn("f", 2.0), mv("C", 0, 1), mv("D", 4, 3), mv("A", 1, 1),
                            mv("B", 1, 9)])
    # the axis itself is vertical, nearly vertical or very long
    for c, d in (((3, 0), (3, 5)), ((3, 0), (3.000001, 5)), ((0, 0), (0.000001, 100))):
        for kind in ("point", "line", "segment"):
            script, obj, free = subject(kind)
            script += [pt("C", *c), pt("D", *d), "n = Line(C, D)", num("f", 1.5),
                       f"m = {cmd}({obj}, n, f)"]
            case(name, script, [setn("f", -1.0), setn("f", 0.5), mv(free[0], 3, 3)])


# ------------------------------------------------ raw scale of the lines
# Images of lines with tiny raw coefficients used where GeoGebra tests raw
# coefficients against 1e-8: Distance(line, line), Slope and
# AngularBisector(line, line).
topic("mirrorline")
# a line through points 1e-6 apart, mirrored in a point and in a line
case("mirrorline", [pt("A", 0, 0), pt("B", 0.000001, 0.000002), "l = Line(A, B)", pt("Q", 2, 1),
                    pt("C", 0, 3), pt("D", 1, 4), "g = Line(C, D)", "m = Mirror(l, Q)",
                    "n = Mirror(l, g)", "d = Distance(l, m)", "e = Distance(m, n)",
                    "s = Slope(n)", "b = AngularBisector(m, n)"],
     [mv("B", 0.000001, 0), mv("B", 0, 0.000001), mv("D", 1, 3), mv("B", 3, 4), mv("Q", 5, 5),
      mv("B", 0.000001, 0.000002)])

topic("rotate")
# a short line and its image under a small rotation
case("rotate", [pt("A", 0, 0), pt("B", 0.000001, 0.000002), "l = Line(A, B)", num("w", 0.5),
                pt("Q", 1, 1), "m = Rotate(l, w)", "n = Rotate(l, w, Q)", "d = Distance(l, m)",
                "e = Distance(l, n)", "s = Slope(n)", "b = AngularBisector(l, n)"],
     [setn("w", 0.001), setn("w", 1e-6), setn("w", 0.0), setn("w", 0.5), mv("B", 3, 1),
      setn("w", 1e-6), setn("w", 0.5)])

topic("dilate")
# a nearly vertical line dilated by a small factor: its raw y coefficient
# scales with the factor
case("dilate", [pt("A", 0, 0), pt("B", 0.000001, 1), "l = Line(A, B)", num("f", 1),
                pt("Q", 2, 1), "m = Dilate(l, f)", "n = Dilate(l, f, Q)", pt("C", 1, 0),
                pt("D", 3, 1), "g = Line(C, D)", "s = Slope(m)", "d = Distance(m, n)",
                "e = Distance(n, g)", "b = AngularBisector(n, g)"],
     [setn("f", 0.001), setn("f", 1e-5), setn("f", -0.001), mv("D", 1.000001, 1),
      mv("D", 3, 1), setn("f", 1)])

# ------------------------------------------------------------- new forms
# Everything below covers the kinds the commands above did not reach when
# this file only made points, lines, segments and circles.

# -------------------------------------------- Stretch along a vector
topic("stretchvec")
for kind in MATRIX_KINDS:
    script, obj, free = subject(kind)
    a, b = rp(), rp()
    script += [pt("C", *a), pt("D", *b), "v = Vector(C, D)", f"m = Stretch({obj}, v)"]
    steps = [mv("C", *rp()), mv("D", *rp())] + object_steps(free)
    # the vector shrinks to nothing, flips and grows again
    steps += [mv("D", *a), mv("D", *rp()), mv("C", *rp())]
    case("stretchvec", script, steps)

# the vector starts at the origin and at a point of the object
for kind in ("line", "circle", "arc", "polygon"):
    script, obj, free = subject(kind)
    script += [vec("v", *rp()), f"m = Stretch({obj}, v)"]
    case("stretchvec", script, [mv("v", *rp()), mv("v", 0, 0), mv("v", 3, -2),
                                mv(free[0], *rp()), mv("v", *rp())])
for kind in ("line", "arc"):
    script, obj, free = subject(kind)
    script += [pt("C", *rp()), pt("D", *rp()), "v = Vector(C, D)", f"m = Stretch({obj}, v)"]
    case("stretchvec", script, [mv("C", *rp()), mv("D", *rp()),
                                mv("C", *rp()), mv("D", *rp())])

# ------------------------------------------------------------- Translate
topic("translate")
for kind in ("ray", "vector", "polygon", "polyline", "arc", "sector", "ellipse", "circle_r"):
    script, obj, free = subject(kind)
    script += [vec("d", *rp()), f"m = Translate({obj}, d)"]
    steps = [mv("d", *rp())] + object_steps(free)
    steps += [mv("d", 0, 0), mv("d", *rp())]
    case("translate", script, steps)

# a polygon translated by a vector between points, and a chain of two
script = [pt("A", *rp()), pt("B", *rp()), pt("C", *rp()), "p = Polygon(A, B, C)",
          pt("D", *rp()), pt("E", *rp()), "u = Vector(D, E)", "q = Translate(p, u)",
          "r = Translate(q, u)"]
case("translate", script, [mv("D", *rp()), mv("E", *rp()), mv("A", *rp()), mv("B", *rp()),
                           mv("E", *rp()), mv("C", *rp())])

# ------------------------------------------------- inversion of objects
topic("invert")
# a line, a segment, a circle, an arc and a sector in a circle; none of
# these pass through the mirror's centre (that image is a single line
# GeoGebra keeps, which the kernel holds undefined)
for kind in ("line", "segment", "circle", "circle_r", "arc", "sector"):
    for _ in range(2):
        script, obj, free = subject(kind)
        o = (round(r2(-4, 4), 2), round(r2(-4, 4), 2))
        r = round(abs(r2(1.0, 5.0)), 2)
        script += [pt("O", *o), num("q", r), "h = Circle(O, q)", f"m = Mirror({obj}, h)"]
        steps = [mv("O", *rp()), mv("A", *rp()), setn("q", 0.5), setn("q", 4.0)]
        steps += object_steps(free)
        steps += [mv("O", *rp()), setn("q", r)]
        case("invert", script, steps)

# a circle that grows to take in the mirror's centre and back
script = [pt("A", *rp()), pt("B", *rp()), "c = Circle(A, B)", pt("O", *rp()), pt("R", *rp()),
          "k = Circle(O, R)", "m = Mirror(c, k)"]
case("invert", script, [mv("B", *rp()), mv("R", *rp()), mv("A", *rp()), mv("O", *rp()),
                        mv("B", *rp()), mv("R", *rp())])

# the mirror is an image itself (the image of the line conic cannot be
# inverted again: GeoGebra turns it into an implicit curve)
script = [pt("O", 0, 0), pt("R", 4, 0), "k = Circle(O, R)", pt("A", 1, 2), pt("B", 6, 2),
          "l = Line(A, B)", "m = Mirror(l, k)", pt("Q", 1, 1),
          "c = Mirror(k, Q)", "d = Mirror(A, c)"]
case("invert", script, [mv("A", *rp()), mv("Q", *rp()), mv("B", *rp()), mv("R", *rp())])

# segments and arcs whose ends move around the mirror's centre
script = [pt("O", 0, 0), pt("R", 4, 0), "k = Circle(O, R)", pt("A", 2, 1), pt("B", 2, 5),
          "s = Segment(A, B)", "m = Mirror(s, k)", pt("C", 3, 0), "a = CircularArc(O, C, A)",
          "n = Mirror(a, k)"]
case("invert", script, [mv("A", -2, 1), mv("A", 2, 1), mv("B", *rp()), mv("C", *rp()),
                        mv("A", *rp()), mv("O", 1, 1), mv("O", 0, 0)])

# ------------------------------------------------------------- chains
topic("chain")
script = [pt("M", *rp()), pt("P", *rp()), pt("Q", *rp()), "a = CircularArc(M, P, Q)",
          pt("C", *rp()), num("w", 0.7), num("f", 1.5), "b = Rotate(a, w)",
          "c = Dilate(b, f)", "d = Mirror(c, M)"]
case("chain", script, [mv("M", *rp()), mv("P", *rp()), setn("f", -1.0), setn("w", 3.14159),
                       mv("Q", *rp()), setn("f", 0.0), setn("f", 2.0)])
script = [pt("A", *rp()), pt("B", *rp()), pt("C", *rp()), "p = Polygon(A, B, C)",
          pt("D", *rp()), pt("E", *rp()), "n = Line(D, E)", num("w", 0.4), num("g", 2),
          "q = Rotate(p, w)", "r = Mirror(q, n)", "s = Dilate(r, g, A)"]
case("chain", script, [mv("A", *rp()), mv("D", *rp()), setn("w", 1.9), mv("E", *rp()),
                       mv("B", *rp()), setn("g", -1.0)])
script = [pt("A", *rp()), pt("B", *rp()), "u = Vector(A, B)", pt("C", *rp()), pt("D", *rp()),
          "v = Vector(C, D)", num("w", 1.1), num("f", 2),
          "i = Rotate(u, w)", "j = Mirror(v, A)", "h = Dilate(i, f)",
          "s = Translate(A, j)", "g = Ray(s, h)"]
case("chain", script, [mv("A", *rp()), mv("B", *rp()), mv("C", *rp()), mv("D", *rp()),
                       mv("B", *rp())])

cases.write()
