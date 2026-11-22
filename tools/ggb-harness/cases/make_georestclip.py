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

cases.case("overlay", [pt("A", 0, 0), pt("B", 0, 4), pt("C", 4, 4), pt("D", 4, 0),
                       pt("E", 2, -1), pt("F", 5, -1), pt("G", 5, 2), pt("H", 2, 2),
                       "p = Polygon(A, B, C, D)", "q = Polygon(E, F, G, H)",
                       "r = IntersectPath(p, q)", "s = Difference(p, q)"], [mv("H", 1, 3)])

for path in ("PolyLine", "Polygon"):
    cases.case("largepath", [pt("O", 0, 0), "k = Circle(O, 1.5)",
                             f"p = {path}(Sequence((j, If(j < 1000, 0, 3)), j, 0, 1000))",
                             "s = IntersectPath(p, k)"], [])

# CommandsTest.cmdIntersectPath clips the boundary of a square into four pieces.
for args in ("p, k", "k, p"):
    cases.case("polygon", [
        pt("A", 0, 0), pt("B", 4, 0), "p = Polygon(A, B, 4)",
        pt("O", 2, 2), "r = sqrt(5)", "k = Circle(O, r)",
        f"q = IntersectPath({args})",
    ], [mv("O", 2, 1), setn("r", 0), setn("r", 3), mv("B", 0, 0), mv("B", 4, 0)])

cases.case("polygon", ellipse + [
    pt("A", -4, 0), pt("B", 4, 0), pt("C", 4, 4), pt("D", -4, 4),
    "p = Polygon(A, B, C, D)", "q = IntersectPath(k, p)",
], [mv("N", 1, 2), setn("r", 0.5), setn("r", 3)])

def alias_cases(topic, command):
    # Use the same homogeneous point as the upstream polygon and line test.
    cases.case(topic, [
        "g: 100x = -300", "h: 100y = 200", "A = Intersect(g, h)",
        pt("B", -3, -2), pt("C", 3, -2), "p = Polygon(A, B, C)",
        pt("D", 0, 5), pt("E", 2, 8), "f = Line(D, E)",
        f"q = {command}(p, f)",
    ], [mv("E", 0, 5), mv("E", 2, 8), mv("D", 0, 10)])
    for path, build in (
        ("s", "Segment(A, B)"),
        ("s", "PolyLine(A, B, C, D)"),
        ("p", "Polygon(A, B, C, D)"),
    ):
        cases.case(topic, ellipse + [
            pt("A", -4, 0), pt("B", 4, 0), pt("C", 4, 1), pt("D", -4, 1),
            f"{path} = {build}", f"q = {command}({path}, k)", f"u = {command}(k, {path})",
        ], [setn("r", 0.5), mv("N", 1, 2), setn("r", 3), mv("B", -4, 0)])


alias_cases("region", "IntersectRegion")
alias_cases("paths", "IntersectionPaths")

cases.write()
