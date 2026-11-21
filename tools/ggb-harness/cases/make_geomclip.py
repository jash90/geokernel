#!/usr/bin/env python3
"""GeoGebra cases for clipping paths and polygon regions."""

from caselib import Cases, mv, num, pt, setn

cases = Cases("geomclip")
case = cases.case

rect = [pt("A", 0, 0), pt("B", 6, 0), pt("C", 6, 5), pt("D", 0, 5),
        "p = Polygon(A, B, C, D)"]
for path, start, end in (
    ("Line", (-2, 2), (9, 2)),
    ("Line", (9, 2), (-2, 2)),
    ("Ray", (2, 2), (5, 3)),
    ("Segment", (2, 2), (8, 4)),
):
    case("carrier", rect + [pt("U", *start), pt("V", *end), f"l = {path}(U, V)",
                            "s = IntersectPath(l, p)", "t = IntersectPath(p, l)"],
         [mv("U", -2, 0), mv("V", 8, 0), mv("V", -2, 0), mv("V", 8, 7), mv("B", 0, 0)])

case("carrier", [pt("A", 0, 0), pt("B", 5, 0), pt("C", 5, 5), pt("D", 3, 5),
                 pt("E", 3, 2), pt("F", 2, 2), pt("G", 2, 5), pt("H", 0, 5),
                 "p = Polygon(A, B, C, D, E, F, G, H)", pt("U", -2, 3), pt("V", 8, 3),
                 "l = Line(U, V)", "s = IntersectPath(l, p)"],
     [mv("U", -2, 4), mv("V", 8, 4), mv("U", -2, 8), mv("V", 8, 8)])

case("conic", rect + [pt("M", 3, 1), num("r", 2), "k = Circle(M, r)",
                      "s = IntersectPath(p, k)", "t = IntersectPath(k, p)"],
     [mv("M", 3, 4), setn("r", 3), mv("M", -3, 4), setn("r", 8)])
case("conic", rect + [pt("M", 3, 2), num("r", 4), "k = Circle(M, r)",
                      "s = IntersectPath(p, k)"],
     [setn("r", 2), setn("r", 5), mv("M", 5, 3)])

for bounds in ((3, 2, 8, 7), (2, 1, 4, 3), (2, -2, 4, 8), (8, 1, 10, 3)):
    x, y, u, v = bounds
    other = [pt("E", x, y), pt("F", u, y), pt("G", u, v), pt("H", x, v),
             "q = Polygon(E, F, G, H)"]
    case("difference", rect + other + ["s = Difference(p, q)"],
         [mv("E", x - 1, y), mv("F", u + 1, y), mv("C", 4, 5)])
    case("difference", rect + other + [num("r", 1), "bb = r > 0",
                                       "s = Difference(p, q, bb)"],
         [setn("r", -1), mv("G", u, v + 1), setn("r", 1)])
    if x < 6:
        case("polygon", rect + other + ["s = IntersectPath(p, q)"],
             [mv("E", x - 1, y), mv("F", u + 1, y), mv("C", 4, 5)])

case("conic", rect + [pt("M", 1, 1), pt("N", 5, 1), num("r", 3),
                      "k = Ellipse(M, N, r)", "s = IntersectPath(k, p)"],
     [setn("r", 4), setn("r", 2), mv("N", 5, 3), setn("r", 3)])
case("conic", rect + [pt("M", 3, 2), pt("N", 6, 2), pt("O", 3, 5),
                      "k = CircularSector(M, N, O)", "s = IntersectPath(p, k)"],
     [mv("N", 7, 2), mv("O", 3, 6), mv("M", 3, 0)])

cases.write()
