#!/usr/bin/env python3
"""All Cartesian-curve intersections recorded with the native oracle."""

from caselib import Cases, setn

cases = Cases("curvepairs")
for args in ("c, d", "d, c"):
    cases.case("parabola", [
        "a = 1", "b = -3", "c = Curve(t, t^2, t, b, 3)",
        "d = Curve(2s, a, s, -2, 2)", f"P = Intersect({args})",
    ], [setn("a", 0), setn("a", -1), setn("a", 4), setn("b", 0), setn("b", 4),
        setn("b", -3), setn("a", 1)])
cases.case("independent", [
    "a = 2", "c = Curve(t^2, t, t, -3, 3)",
    "d = Curve(s, a - s^2, s, -3, 3)", "P = Intersect(c, d)",
], [setn("a", 0), setn("a", -1), setn("a", 2)])
cases.case("coincident", [
    "a = 2", "c = Curve(t, t, t, -10, 10)",
    "d = Curve(2s, a - 1 + 2a*s, s, -5, 5)", "P = Intersect(c, d)",
], [setn("a", 1), setn("a", 0), setn("a", 2)])
# A single labelled root can move in and out of the parameter interval.
cases.case("bounded", [
    "a = 0", "c = Curve(t, t^2, t, a, 3)",
    "d = Curve(s, 1, s, -3, 3)", "P = Intersect(c, d)",
], [setn("a", 2), setn("a", -2), setn("a", 0)])
cases.case("order", [
    "c = Curve(t, (t + 3)*(t + 1)*(t - 1)*(t - 2), t, -5, 5)",
    "d = Curve(s, 0, s, -5, 5)", "P = Intersect(c, d)",
], [])
cases.case("radical", [
    "c = Curve(t, sqrt(t), t, 0, 4)",
    "d = Curve(s, 1, s, 0, 4)", "P = Intersect(c, d)",
], [])
cases.case("growth", [
    "a = 0", "c = Curve(t, t^2, t, -3, 3)",
    "d = Curve(s, a, s, -3, 3)", "P = Intersect(c, d)",
], [setn("a", 1), setn("a", -1), setn("a", 2)])
cases.cases[-1]["track_boundary_growth"] = True

for height in (2, 3, 5, 7):
    cases.case("radical", [
        f"a = {height}", "c = Curve(t, t^2, t, -4, 4)",
        "d = Curve(s, a, s, -4, 4)", "P = Intersect(c, d)",
    ], [setn("a", 4), setn("a", -1), setn("a", height)])

cases.write()
