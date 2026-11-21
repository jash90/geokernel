"""Angle command signatures, interval boundaries, and dependent updates."""

from math import pi
from caselib import Cases, mv, num, pt, setn

cases = Cases("angle")

cases.case("points", [
    pt("A", 2, 0), pt("B", 0, 0), pt("C", 0, 2),
    "ang = Angle(A, B, C)", "back = Angle(C, B, A)", "ss = sin(ang)",
], [mv("C", 0, -2), mv("C", -2, 0), mv("C", 2, 0), mv("C", 0, 0),
    mv("C", 0, 2), mv("A", 0, 0), mv("A", 3, 1), mv("B", 1, 2)])
cases.case("points", [
    pt("A", 0, 0), pt("B", 0, 0), pt("C", 0, 2),
    "ang = Angle(A, B, C)",
], [mv("A", 1, 0), mv("B", 0, 2), mv("B", 1, 1)])
cases.topic("points")
for _ in range(12):
    a, b, c = (cases.rp() for _ in range(3))
    cases.case("points", [
        pt("A", *a), pt("B", *b), pt("C", *c), "ang = Angle(A, B, C)",
    ], [mv("C", *a), mv("A", *b), mv("A", *cases.rp()), mv("C", *cases.rp())])

cases.case("lines", [
    pt("A", 0, 0), pt("B", 2, 0), pt("C", 0, 2), pt("D", 2, 2),
    "ll = Line(A, B)", "mm = Line(C, D)", "ang = Angle(ll, mm)",
    "back = Angle(mm, ll)", "same = Angle(ll, ll)",
], [mv("D", -2, 2), mv("D", 0, 0), mv("D", 2, 4), mv("D", 0, 2), mv("D", 3, 1)])
cases.case("lines", [
    pt("A", 0, 0), pt("B", 2, 0), pt("C", 0, 2),
    "ll = Line(A, B)", "mm = Line(B, A)",
    "nn = PerpendicularLine(C, ll)", "op = Angle(ll, mm)", "rt = Angle(ll, nn)",
], [mv("B", 0, 0), mv("B", 1, 1), mv("C", 4, 0)])
cases.topic("lines")
for _ in range(8):
    a, b, c, d = (cases.rp() for _ in range(4))
    cases.case("lines", [
        pt("A", *a), pt("B", *b), pt("C", *c), pt("D", *d),
        "ll = Line(A, B)", "mm = Line(C, D)", "ang = Angle(ll, mm)",
    ], [mv("B", *c), mv("D", *b), mv("B", *a), mv("B", *cases.rp())])

cases.case("lines", [
    pt("A", 0, 0), pt("B", 2, 0), pt("C", 0, 0), pt("D", 0, 2),
    "ll = Line(A, B)", "mm = Line(C, D)", "ang = Angle(ll, mm)",
], [
    {"action": "move", "label": "D", "x": 1e-9, "y": 1e-9},
    {"action": "move", "label": "D", "x": -1e-9, "y": 1e-9},
    mv("D", 0, 0),
])
cases.case("lines", [
    pt("A", 0, 0), pt("B", 3, 0), pt("C", 0, 3),
    "ss = Segment(A, B)", "rr = Ray(A, C)", "ang = Angle(ss, rr)",
], [mv("C", 0, -3), mv("B", -3, 0), mv("B", 0, 0), mv("B", 3, 1)])

cases.case("directions", [
    pt("A", 2, 1), pt("B", 1, -2), pt("C", 0, 0), pt("D", 3, 0),
    "vv = Vector(C, D)", "ww = Vector(D, B)",
    "ap = Angle(A)", "av = Angle(vv)", "pp = Angle(A, B)", "pv = Angle(A, ww)",
    "vp = Angle(vv, B)", "vw = Angle(vv, ww)",
], [mv("A", -2, -1), mv("B", 0, 2), mv("D", 0, 0), mv("B", 0, 0),
    mv("C", 3, 1), mv("D", -1, 4)])
cases.case("directions", [
    pt("vv", 0, 0), pt("ww", 1, 1), "ang = Angle(vv, ww)", "head = Angle(vv)",
], [mv("vv", -2, 0), mv("ww", 1, -1), mv("ww", 0, 0), mv("vv", 0, -2)])
cases.topic("directions")
for _ in range(8):
    a, b, v, w = (cases.rp() for _ in range(4))
    cases.case("directions", [
        pt("A", *a), pt("B", *b), pt("vv", *v), pt("ww", *w),
        "aa = Angle(A)", "bb = Angle(vv)", "ab = Angle(A, B)",
        "av = Angle(A, vv)", "va = Angle(ww, A)", "vw = Angle(vv, ww)",
    ], [mv("A", *b), mv("vv", *w), mv("B", *cases.rp()), mv("ww", *cases.rp())])

cases.case("numeric", [
    num("nn", 1.0), "ang = Angle(nn)", "literal = Angle(7)", "literaldeg = Angle(30°)",
    "turn = 30°", "copied = Angle(turn)", "dep = 2 * turn", "tr = sin(ang)",
    "sum = ang + turn", "ra = round(ang)", "scale = ang / (1°)",
], [setn("nn", -0.5), setn("nn", 0), setn("nn", 2*pi), setn("nn", -2*pi),
    setn("nn", 4*pi), setn("turn", -0.25), setn("turn", 9)])
cases.case("numeric", [
    "big = 765°", "negative = -450°", "direct = Angle(2 * pi)", "mult = 3 * negative",
], [])

cases.case("polygon", [
    pt("A", 0, 0), pt("B", 4, 0), pt("C", 2, 1), pt("D", 0, 3),
    "pp = Polygon(A, B, C, D)", "ang = Angle(pp)", "inside = InteriorAngles(pp)",
    "total = inside_{1} + inside_{2} + inside_{3} + inside_{4}",
], [mv("B", -4, 0), mv("C", -2, 1), mv("B", 0, 0), mv("B", 4, 0), mv("C", 2, 2)])
cases.case("polygon", [
    pt("A", 0, 0), pt("B", 3, 0), num("nn", 5),
    "pp = Polygon(A, B, nn)", "ang = Angle(pp)", "inside = InteriorAngles(pp)",
], [mv("B", 3, 2), setn("nn", 3), setn("nn", 7), setn("nn", 5)])
cases.case("polygon", [
    pt("A", 0, 0), pt("B", 1, 1), pt("C", 2, 2), pt("D", 3, 3),
    "pp = Polygon(A, B, C, D)", "ang = Angle(pp)", "inside = InteriorAngles(pp)",
], [mv("B", 0, 1), mv("B", 2, 0)])

cases.case("fixed", [
    pt("A", 4, 1), pt("B", 1, 1), num("nn", 1.0),
    "ang = Angle(A, B, nn)", "ak = Angle(A, B, -60°)",
], [setn("nn", -1.0), setn("nn", 0), setn("nn", pi), setn("nn", 2*pi),
    mv("A", 1, 1), mv("B", -2, 3), setn("nn", pi/2)])
cases.case("fixed", [
    pt("A", 1, 0), pt("B", 0, 0), pt("C", 0, -1), "turn = Angle(A, B, C)",
    "ang = Angle(A, B, turn)",
], [mv("C", -1, 0), mv("C", 0, 1), mv("B", 1, 0), mv("B", 2, 1)])
cases.case("fixed", [
    pt("A", 1, 0), pt("B", 1, 0), "ang = Angle(A, B, 45°)",
], [mv("B", 0, 0), mv("A", 0, 1)])

cases.case("conic", [
    pt("A", -2, -1), pt("B", 2, 1), pt("P", 1, 4),
    "ell = Ellipse(A, B, P)", "hyp = Hyperbola(A, B, P)", "cc = Circle(A, B)",
    "ee = Angle(ell)", "hh = Angle(hyp)", "oo = Angle(cc)",
], [mv("B", -2, 3), mv("P", 1, -1), mv("B", -2, -1), mv("B", 4, -2)])
cases.case("conic", [
    pt("A", 0, 0), pt("B", 4, 2), pt("F", 0, 2),
    "ll = Line(A, B)", "par = Parabola(F, ll)", "ang = Angle(par)",
], [mv("B", 4, -2), mv("F", 0, -2), mv("B", 0, 0), mv("B", 3, 3)])
cases.case("conic", [
    pt("A", 0, 0), pt("B", 2, 0), pt("C", 0, 2),
    "arc = CircularArc(A, B, C)", "ang = Angle(arc)",
], [mv("C", 0, -2), mv("B", 0, 0), mv("B", 3, 2)])

cases.write()
