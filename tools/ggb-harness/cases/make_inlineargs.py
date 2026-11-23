#!/usr/bin/env python3
"""Record inline arguments of existing geometry commands."""

from caselib import Cases, setn

cases = Cases("inlineargs")
cases.topic("equations")
for line in (
    "P = Intersect(x = 4, y = 4, 1)",
    "P = Intersect(y = 5, x = 5, 2)",
    "P = Intersect(100 x = -300, 100 y = 200)",
    "n = Area(x^2 + y^2 = 1)",
    "n = Circumference(x^2 + y^2 = 1)",
    "P = Center(x^2 + y^2 = 1)",
    "n = Radius(x^2 + y^2 = 1)",
    "n = Slope(x + y = 17)",
    "u = Direction(-2 x + 3 y + 1 = 0)",
    "b = AreParallel(x + y = 17, x = 4)",
    "b = ArePerpendicular(x + y = 17, x = 4)",
    "b = AreConcurrent(x + y = 17, x = 4, x = 0)",
    "b = IsTangent(x + y = 17, x^2 + y^2 = 1)",
    "u = PerpendicularVector(3 x = 4 y)",
    "u = UnitPerpendicularVector(3 x = 4 y)",
    "u = UnitVector(3 x = 4 y)",
    "P = Point(x^2 + y^2 = 1, 0.25)",
    "n = PathParameter(Point(x^2 + y^2 = 1))",
):
    cases.case("equations", [line], [])

cases.case("equations", ["a = 1", "P = Intersect(a x = 4, y = 4)"],
           [setn("a", 0), setn("a", -2), setn("a", 1)])
cases.case("equations", ["a = 1", "n = Area(x^2 + y^2 = a)"],
           [setn("a", 0), setn("a", -1), setn("a", 4)])
cases.case("equations", ["a = 1", "n = Slope(x + a y = 17)"],
           [setn("a", 0), setn("a", -2), setn("a", 1)])

cases.topic("points")
cases.case("points", ["c = Circle((1, 1), 42)"], [])
cases.case("points", ["p = Polygon((1, 1), (2, 1 / 2), 4)"], [])
cases.case("points", ["P = Centroid(Polygon((1, 1), (2, 1 / 2), (3, 1 / 3)))"], [])
cases.case("points", ["k = Curvature((1, 1), Curve(sin(t), cos(t), t, 0, 3))"], [])
cases.case("points", ["u = CurvatureVector((1, 1), Curve(sin(t), cos(t), t, 0, 3))"], [])

cases.case("equations", ["s = Arc(x^2 + y^2 = 1, (1, 1), (2, 1 / 2))"], [])
cases.case("equations", ["s = Sector(x^2 + y^2 = 1, (1, 1), (2, 1 / 2))"], [])

cases.topic("arithmetic")
cases.case("arithmetic", ["aa_{1} = 1", "b = aa_1 + aa_{1}"], [setn("aa_{1}", -2)])
cases.case("arithmetic", ["a = 1", "b = 2", "c = ab + a"], [setn("b", -2), setn("a", 0)])
cases.case("arithmetic", ["a = 1", "f = axx + a", "b = f(2)"], [setn("a", 0), setn("a", -3)])

cases.case("points", ["l = PolyLine((1, 1), (3, 1 / 3), (5, 1 / 5))"], [])
cases.case("equations", ["A = (-3, 2)", "B = (-3, -2)", "C = (3, -2)",
                         "t1 = Polygon(A, B, C)", "s = IntersectPath(t1, -3 x + 2 y = 10)"], [])
cases.case("arithmetic", ["a = log_{2}2 / (2)"], [])
cases.case("arithmetic", ["a = 2", "b = log_{2}(a)"], [setn("a", -2), setn("a", 8)])

cases.write()
