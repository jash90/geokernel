#!/usr/bin/env python3
"""Record values for the 2D drawings and tools."""

from caselib import Cases, mv, pt

cases = Cases("canvas")
cases.case("parts", [
    pt("A", 0, 0), pt("B", 2, 0), pt("C", 0, 2),
    "a = CircularArc(A, B, C)", "s = CircularSector(A, B, C)",
    "k = Circle(A, B)", "P = Point(k)", "Q = Midpoint(P, A)", "l = Locus(Q, P)",
    "chart = PieChart({1, 2}, (4, 0), 2)",
], [])
cases.case("drawing", [
    "curve = Curve(t, t^2, t, -2, 2)", "region(x,y) = y > x",
    "f(x,y) = x + y", 'text1 = "Placed text"',
    "stroke1 = PenStroke((0, 0), (1, -1))",
], [])
cases.case("reviewlimits", [
    *[f"region{k}(x,y) = y > x + {k / 10}" for k in range(16)],
    "L = Sequence((k, 0), k, 1, 257)", "n = Length(L)",
    "g = Curve(t, n*t, t, -2, 2)",
], [])
cases.case("reviewmixed", [
    "L = Append(Sequence((k, 0), k, 1, 256), 7)",
    "M = Join({7}, Sequence((k, 0), k, 1, 256))",
], [])
cases.case("reviewselection", [
    pt("A", 0, 0), pt("B", 2, 0), pt("C", 0, 2), pt("D", 3, 2),
    "poly = Polygon(A, B, C)", "list1 = {D, poly}",
    'text1 = Text("Label", A)', "k = Circle(A, B)", "P = Point(k)",
    "Q = Midpoint(P, A)", "locus1 = Locus(Q, P)",
], [mv("A", 1, 1)])
cases.case("reviewpen", [
    "stroke1 = PenStroke((0, 0), (1, 1), (2, 0))",
], [])
cases.write()
