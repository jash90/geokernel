#!/usr/bin/env python3
"""Clipped segments and polygonal paths, including recomputation."""

from caselib import Cases, mv, num, pt, setn

cases = Cases("georestclip")
circle = [pt("O", 0, 0), num("r", 2), "k = Circle(O, r)"]
for reverse in (False, True):
    args = ("k, s", "k, p") if reverse else ("s, k", "p, k")
    cases.case("segment", circle + [
        pt("A", -4, 0), pt("B", 4, 0), "s = Segment(A, B)",
        f"q = IntersectPath({args[0]})",
    ], [mv("A", -4, 2), mv("B", 4, 2), mv("O", 0, -1),
        mv("B", -4, 2), setn("r", -1), setn("r", 4), mv("B", 4, 0),
        mv("A", 1, 0), mv("B", 0, 0), mv("B", 1, 0)])
    cases.case("polyline", circle + [
        pt("A", -4, 0), pt("B", 4, 0), pt("C", 4, 1), pt("D", -4, 1),
        "p = PolyLine(A, B, C, D)", f"q = IntersectPath({args[1]})",
    ], [mv("D", -4, 2), mv("O", 0, -2), mv("C", 4, 0),
        mv("D", -4, 0), setn("r", -1), setn("r", 3)])

cases.case("polygon", circle + [
    pt("A", -4, 0), pt("B", 4, 0), pt("C", 4, 4), pt("D", -4, 4),
    "p = Polygon(A, B, C, D)", "q = IntersectPath(p, k)",
], [mv("O", 0, -2), mv("O", 0, -3), setn("r", -1), setn("r", 3),
    mv("A", 4, 0), mv("O", 0, 4)])

ellipse = [pt("M", -1, 0), pt("N", 1, 0), num("r", 2), "k = Ellipse(M, N, r)"]
for segment_args, polyline_args in (("s, k", "p, k"), ("k, s", "k, p")):
    cases.case("segment", ellipse + [
        pt("A", -4, 0), pt("B", 4, 0), "s = Segment(A, B)",
        f"q = IntersectPath({segment_args})",
    ], [mv("B", 4, 1), setn("r", 0.5), setn("r", 2), mv("A", 4, 1)])
    cases.case("polyline", ellipse + [
        pt("A", -4, 0), pt("B", 4, 0), pt("C", 4, 1), pt("D", -4, 1),
        "p = PolyLine(A, B, C, D)", f"q = IntersectPath({polyline_args})",
    ], [mv("D", -4, 3), setn("r", 0.5), mv("N", 1, 2), setn("r", 3)])

cases.case("polygon", ellipse + [
    pt("A", -4, 0), pt("B", 4, 0), pt("C", 4, 4), pt("D", -4, 4),
    "p = Polygon(A, B, C, D)", "q = IntersectPath(p, k)",
], [mv("N", 1, 2), setn("r", 0.5), setn("r", 3)])

# CommandsTest.cmdIntersectPathHomogeneousCoords, with named arguments.
cases.case("homogeneous", [
    "g: 100x = -300", "h: 100y = 200", "A = Intersect(g, h)",
    pt("B", -3, -2), pt("C", 3, -2), "p = Polygon(A, B, C)",
    "f: -3x + 2y = 10", "s = IntersectPath(p, f)",
], [mv("B", -4, -1), mv("C", 5, -2)])

cases.write()
