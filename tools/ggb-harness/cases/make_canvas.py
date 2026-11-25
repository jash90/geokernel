#!/usr/bin/env python3
"""Record values for the 2D drawings and tools."""

from caselib import Cases, pt

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
cases.write()
