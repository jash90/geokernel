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
    raise ValueError(kind)


KINDS = ["point", "line", "segment", "circle", "circle_r"]


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
    for kind in ("point", "line", "segment"):
        script, obj, free = subject(kind)
        script += [pt("C", *rp()), pt("D", *rp()), "n = Line(C, D)", num("f", 2),
                   f"m = {cmd}({obj}, n, f)"]
        steps = [setn("f", v) for v in RATIOS] + [mv("C", *rp()), mv("D", *rp())]
        steps += object_steps(free)
        # coincident points make the axis undefined, then separate them again
        steps += [mv("D", *rp(-8, 8)), mv("C", *rp()), mv("D", *rp())]
        case(name, script, steps)
    for kind in ("point", "line", "segment"):
        script, obj, free = subject(kind)
        script += [pt("C", *rp()), pt("D", *rp()), "n = Segment(C, D)", num("f", -1.5),
                   f"m = {cmd}({obj}, n, f)"]
        case(name, script, [mv("C", *rp()), setn("f", 0.5), mv("D", *rp()), mv("C", 1.0, 1.0),
                            mv("D", 1.0, 1.0), mv("D", *rp()), setn("f", 3.0)])
    # horizontal, vertical and diagonal axes, an axis through the origin
    for c, d in (((0, 2), (4, 2)), ((3, -1), (3, 5)), ((0, 0), (2, 2)), ((0, 0), (-3, 1)),
                 ((1, 1), (5, 2))):
        for kind in ("point", "line", "segment"):
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

cases.write()
