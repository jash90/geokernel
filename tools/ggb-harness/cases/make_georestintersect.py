"""Cartesian intersections, including the 2D curve cases from CommandsTest."""

from caselib import Cases, mv, setn


cases = Cases("georestintersect")


def intersect(topic, setup, args, steps=()):
    cases.case(topic, setup + [f"P = Intersect({args})"], list(steps))


# cmdIntersect: the two answers are ordered by curve parameter.
circle = ["A = (0, 0)", "q = Curve(x(A)+5*sin(t), y(A)+5*cos(t), t, 0, 6)",
          "g: 3x = 4y"]
for args in ("g, q", "q, g"):
    intersect("trig", circle, args, [mv("A", 1, -1), mv("A", 1, 8), mv("A", 0, 0)])

# Polynomial and spline inputs from the same method.
intersect("polynomial", ["q = Curve(t, -t^4+2t^2+0.25t+5, t, -10, 10)",
                         "A = (0, 4.07875)", "B = (9.6, 5.07875)", "g = Line(A, B)"],
          "q, g", [mv("B", 9.6, 6), mv("B", 0, 4.07875)])
intersect("spline", ["A = (1, 0)", "B = (1, 1)", "C = (0, 1)",
                     "q = Spline({A,B,C}, 3)", "g: x = y"],
          "g, q", [mv("B", 1, 2), mv("B", 1, 0), mv("B", 1, 1)])
# AlgoIntersectCurveTest checks the start and the end of a spline.
for height in (5, 2):
    intersect("splineend", ["A = (1, 5)", "B = (2, 4)", "C = (1, 3)", "D = (1, 2)",
                           "q = Spline({A,B,C,D}, 3)", f"g: y = {height}"],
              "q, g", [mv("B", 2, 4.5), mv("B", 2, 4)])

parabola = ["A = (-2, 1)", "B = (2, 1)", "g = Line(A, B)",
            "q = Curve(t, t^2, t, -2, 2)"]
for args in ("g, q", "q, g"):
    intersect("tangent", parabola, args,
              [mv("A", -2, 0), mv("B", 2, 0), mv("A", -2, -1), mv("B", 2, -1),
               mv("B", -2, -1), mv("A", -2, 1), mv("B", 2, 1)])

# Only the in-domain root is initially labelled. It must keep its slot.
intersect("bounded", ["A = (-2, 1)", "B = (2, 1)", "g = Line(A, B)",
                      "aa = 0", "bb = 3", "q = Curve(t, t^2, t, aa, bb)"],
          "g, q", [setn("aa", 1.25), setn("aa", 0.5), setn("bb", 0), setn("bb", 3)])

# Two-parameter Newton: switch branches and then make the curves disjoint.
crossing = ["A = (0, 0)", "q = Curve(t, t^2+y(A), t, -2, 2)",
            "r = Curve(s, 1, s, -2, 2)", "aa = 1.25", "bb = 0.5"]
intersect("seed", crossing, "q, r, aa, bb",
          [setn("aa", -1.25), mv("A", 0, 0.75), mv("A", 0, 1), mv("A", 0, 2), mv("A", 0, 0)])
# A crossing, then parallel, then coincident and undefined.
intersect("seedlines", ["A = (0, 1)", "B = (1, 2)",
                        "q = Curve(t, 0, t, -4, 4)",
                        "r = Curve(s, y(A)+s*(y(B)-y(A))/(x(B)-x(A)), s, -4, 4)"],
          "q, r, 0, 0", [mv("B", 1, 1), mv("A", 0, 0), mv("B", 1, 0),
                          mv("B", 0, 0), mv("A", 0, 1), mv("B", 1, 2)])

# Existing projection-based incidence, plus a Cartesian path that requires
# minimization in its given parameter domain.
for args in ("g, A", "A, g"):
    intersect("pathline", ["g: x = y", "A = (2, 2)"], args,
              [mv("A", 2, -2), mv("A", -3, -3)])
for args in ("q, A", "A, q"):
    intersect("pathcurve", ["q = Curve(t, t^2, t, -2, 2)", "A = (0, 0)"], args,
              [mv("A", 0, 1), mv("A", 1, 1), mv("A", -2, 4), mv("A", 3, 9)])

intersect("growth", ["A = (0, 0)", "q = Curve(t, t^2+y(A), t, -2, 2)", "g: y = 0"],
          "g, q", [mv("A", 0, -1), mv("A", 0, 1), mv("A", 0, -1)])
cases.cases[-1]["track_boundary_growth"] = True

cases.write()
