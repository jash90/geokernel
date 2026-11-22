"""Optimization, hull, coordinate styles, construction steps, view
readers, names, dynamic coordinates, and the remaining Sum forms, with
degenerate inputs (no dependency, no slider bounds, disjoint hulls)."""

from caselib import Cases, mv, num, pt, setn

cases = Cases("misc")

# Maximize and Minimize over a bounded number.
cases.case("optimize", [
    "a = Slider(-10, 10)", "f = -(a - 3)^2 + 7", "m = Maximize(f, a)",
    "n = Minimize(f, a)",
], [])
# the expressions written inline, as GeoGebra's own test does
cases.case("optimize", [
    "s = Slider(-5, 5)", "u = Maximize(5 - (3 - s)^2, s)",
    "v = Minimize(5 + (3 - s)^2, s)",
], [])
# a dependent value that does not read the slider: the search runs anyway
cases.case("optimize", [
    "a = Slider(-10, 10)", "b = 5", "g = b", "p = Minimize(g, a)",
], [setn("b", -3.0), setn("b", 5.0)])
# a number without slider bounds gives an undefined result
cases.case("optimize", [
    "c = 2", "h = 3c", "q = Maximize(h, c)",
], [setn("c", 7.0)])
# the optimum follows the relationship, not the slider's position
cases.case("optimize", [
    "a = Slider(1, 10)", "f = a^2", "m = Minimize(f, a)",
], [setn("a", 4.0), setn("a", 10.0)])
# an expression of the slider, defined by bounds the search cannot reach
cases.case("optimize", [
    "a = Slider(0, 6)", "f = 2a + 1", "m = Maximize(f, a)",
], [setn("a", 2.0), setn("a", 6.0)])
# a degenerate slider: min == max, the interval is one point
cases.case("optimize", [
    "s = Slider(3, 3)", "f = (s - 1)^2 + 2", "m = Minimize(f, s)",
    "n = Maximize(f, s)",
], [])
# a locus whose chain runs through the optimized number
cases.case("optimize", [
    "a = Slider(0, 6)", "f = -(a - 3)^2 + 7", "m = Maximize(f, a)",
    "A = (0, 0)", "B = (10, 0)", "s = Segment(A, B)", "P = Point(s)",
    "C = (m, a)", "Q = Midpoint(P, C)", "l = Locus(Q, P)",
], [mv("P", 2, 0), mv("P", 8, 0)])
# The optimizer depends on the mover. Its current value is held by the
# locus's sampling construction, for both point paths and sliders.
cases.case("optimize", [
    "a = Slider(0, 6)", pt("A", 1, 0), pt("B", 3, 0),
    "s = Segment(A, B)", "P = Point(s)",
    "n = Maximize(-(a - x(P))^2, a)", "Q = (n, x(P))", "l = Locus(Q, P)",
], [])
cases.case("optimize", [
    "a = Slider(0, 6)", "t = Slider(1, 3)",
    "n = Minimize((a - t)^2, a)", "Q = (n, t)", "l = Locus(Q, t)",
], [])
cases.case("optimize", [
    "a = Slider(0, 6)", "b = 1", "f = sqrt(a - b)",
    "m = Minimize(f, a)", "q = Maximize(f, a)",
], [setn("b", 4), setn("b", -1), setn("b", 1)])
cases.case("optimize", [
    "a = Slider(0, 6)", "f = If(a != 3(3 - sqrt(5)), a)",
    "m = Maximize(f, a)", "q = Minimize(f, a)",
], [])
cases.case("optimize", [
    "a = 2", "f = (a - 3)^2", "m = Minimize(f, a)",
], [{"action": "set_slider_bounds", "label": "a", "min": 0, "max": 6}, setn("a", 4)])
cases.case("optimize", [
    "a = Slider(0, 6)", pt("P", 3, 0), pt("A", 0, 0),
    "f = If(x(P) > 0, a * (2 * x(P) - a))", "g = -f",
    "c = Circle(A, Maximize(f, a))", "d = Circle(A, Minimize(g, a))",
], [mv("P", 2, 0), mv("P", 0, 0), mv("P", 4, 0)])
# Optimizers created before the number has slider bounds stay undefined
# after the bounds arrive; one created afterwards searches.
cases.case("optimize", [
    "a = 2", "f = (a - 3)^2", "m = Minimize(f, a)", "n = Maximize(-f, a)",
    "b = Slider(0, 6)", "g = (a - 3)^2 + b", "p = Minimize(g, b)",
], [{"action": "set_slider_bounds", "label": "a", "min": 0, "max": 6}, setn("a", 4),
    setn("b", 2), setn("a", 1)])

# Hull: the convex hull of a list of points, as a locus.
cases.case("hull", [
    pt("A", 0, 0), pt("B", 4, 0), pt("C", 4, 3), pt("D", 0, 3),
    pt("E", 2, 1), "l = {A, E, B, A, C, D}", "h = Hull(l, 1)",
], [mv("E", -1, -1), mv("E", 5, 4), mv("D", -3, 3)])
# a single point is a run of two
cases.case("hull", [
    pt("A", 2, 3), "l = {A}", "h = Hull(l, 0)",
], [mv("A", -1, 5)])
# no point at all is undefined, and numbers do not count
cases.case("hull", [
    "l = {}", "h = Hull(l, 1)",
], [])
cases.case("hull", [
    "l = {1, 2}", "h = Hull(l, 1)",
], [])
# an undefined point between defined ones is skipped
cases.case("hull", [
    "a = 1/0", pt("B", 0, 0), pt("C", 5, 0), pt("D", 0, 5),
    "l = {B, D, C}", "h = Hull(l, 1)",
], [])
# collinear points keep only the ends of their run
cases.case("hull", [
    "l = {(1, 1), (2, 2), (3, 3), (4, 0.25), (5, 0.2)}", "h = ConvexHull(l)",
], [])
# two points make a segment there and back
cases.case("hull", [
    "l = {(0, 0), (4, 0)}", "h = ConvexHull(l)",
], [])
# duplicates count once
cases.case("hull", [
    "l = {(0, 0), (0, 0), (4, 0), (4, 0)}", "h = Hull(l, 1)",
], [])
# all points equal
cases.case("hull", [
    "l = {(2, 2), (2, 2), (2, 2)}", "h = Hull(l, 1)",
], [])
# every point on one line
cases.case("hull", [
    "l = {(0, 0), (1, 1), (2, 2), (3, 3)}", "h = ConvexHull(l)",
], [])
# A near duplicate keeps the first point, even in the presence of an
# unrelated point in its coordinate sort order.
cases.case("hull", [
    "l = {(0.000000008, 0.000000008), (0.000000004, 3), (0, 0), (4, 0)}",
    "h = ConvexHull(l)",
], [])

# ToComplex, ToPolar and ToPoint: the same value, another style.
cases.case("convert", [
    pt("A", 1, 2), "cz = ToComplex(A)", "p = ToPolar(A)", "q = ToPoint(cz)",
    "n = 3", "w = ToComplex(n)", "e = ToPolar(n)", "r = ToPoint(n)",
], [mv("A", -3, 4), mv("A", 0, -1), mv("A", 1, 2)])
# a list gives the point of its first two elements
cases.case("convert", [
    "l = {5, 2}", "u = ToComplex(l)", "v = ToPolar(l)", "w = ToPoint(l)",
], [])
# a vector keeps its kind, and ToPoint makes it a point
cases.case("convert", [
    pt("A", 1, 1), pt("B", 4, 5), "w = Vector(A, B)", "u = ToComplex(w)",
    "v = ToPolar(w)", "q = ToPoint(w)",
], [mv("B", 1, 1), mv("B", 4, 5)])
# degenerate points
cases.case("convert", [
    pt("A", 0, 0), "cz = ToComplex(A)", "p = ToPolar(A)",
    pt("B", -2, 0), "u = ToComplex(B)", "r = ToPolar(B)",
], [mv("B", 0, 0)])

# DynamicCoordinates: the point of two numbers, following the point.
cases.case("dynamic", [
    pt("P", 1, 1), "M = DynamicCoordinates(P, x(P) + 2, y(P) * 3)",
], [mv("P", 5, 2), mv("P", -1, 0), mv("P", 1, 1)])
# plain numbers as the coordinates
cases.case("dynamic", [
    pt("P", 1, 1), "M = DynamicCoordinates(P, 3, -4)",
], [mv("P", 2, 5)])
cases.case("dynamic", [
    pt("P", 0, 0), "q = 4", "M = DynamicCoordinates(P, q, -q)",
], [setn("q", 1.5), setn("q", 0.0), setn("q", -2.0)])

# Name and Object.
cases.case("name", [
    "txt = \"GeoGebra\"", "a = 5", "n = Name(txt)", "m = Name(a)",
    "b = Object(\"a\")",
], [])

# The construction protocol.
cases.case("protocol", [
    "cs = ConstructionStep()",
    pt("A", 1, 2), "i = ConstructionStep()",
    "B = (3, 4)", "j = ConstructionStep()",
    "k = ConstructionStep(B)",
], [])
# the protocol counts construction elements: the polygon's sides share
# its entry
cases.case("protocol", [
    pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 3), "p = Polygon(A, B, C)",
    pt("D", 1, 1), "s = ConstructionStep(D)", "t = ConstructionStep(p)",
    "u = ConstructionStep(A)", "i = ConstructionStep()",
], [])
# the indexed outputs of one command share its entry: both intersection
# points are one element (D is the fourth, not the fifth)
cases.case("protocol", [
    "g: x^2 + y^2 = 4", "h: y = x", "R = Intersect(g, h)",
    pt("D", 5, 5), "s = ConstructionStep(D)",
], [])
# SetConstructionStep clamps to the last step and recomputes
cases.case("protocol", [
    pt("A", 0, 0), pt("B", 1, 1), "SetConstructionStep(1000000000)",
    "i = ConstructionStep()", "SetConstructionStep(5)",
    "j = ConstructionStep()", "SetConstructionStep(1)",
    "k = ConstructionStep()",
], [])
# Infinity and a finite value outside the range of a signed 32-bit step
# both return from the start to the last element.
for end in ("1/0", "3000000000"):
    cases.case("protocol", [
        "cs = ConstructionStep()", pt("A", 0, 0), pt("B", 1, 1),
        "SetConstructionStep(1)", f"SetConstructionStep({end})",
    ], [])
# An inline point used for coordinate conversion has no protocol entry.
cases.case("protocol", [
    pt("A", 1, 2), "T = ToPoint((3, 4))", pt("D", 3, 4),
    "s = ConstructionStep(D)", "t = ConstructionStep(T)",
], [])
# Additions after navigating back are inserted after the current step.
cases.case("protocol", [
    "cs = ConstructionStep()", pt("A", 0, 0), pt("B", 1, 1), pt("C", 2, 2),
    "SetConstructionStep(1)", pt("D", 3, 3), "ds = ConstructionStep(D)",
    "SetConstructionStep(1/0)", "bs = ConstructionStep(B)",
    "aa = ConstructionStep(A)",
], [])

# Step arguments are rounded when the command runs. Later edits of the
# argument do not navigate the protocol again.
cases.case("protocol", [
    "cs = ConstructionStep()", "n = 1.6", pt("A", 0, 0), pt("B", 1, 1),
    "SetConstructionStep(n)",
], [setn("n", 0), setn("n", 4), mv("B", 4, 5)])
for target in ("-3", "0", "0/0", "1.5"):
    cases.case("protocol", [
        "cs = ConstructionStep()", pt("A", 0, 0), pt("B", 1, 1),
        f"SetConstructionStep({target})",
    ], [])

# The view readers.
cases.case("view", [
    "p = Corner(1)", "q = Corner(2)", "r = Corner(3)", "s = Corner(4)",
    "t = Corner(5)", "u = Corner(6)",
    "gx = AxisStepX()", "gy = AxisStepY()",
], [])

# Optimize a live point on a bounded path.
cases.case("maximizepoint", [
    "C = (3, 4)", "c = Circle(C, 5)", "P = Point(c)", "a = y(P)", "M = Maximize(a, P)",
], [mv("P", 8, 4), mv("P", -2, 4)])
cases.case("maximizepoint", [
    "A = (0, 0)", "B = (10, 0)", "s = Segment(A, B)", "P = Point(s)",
    "n = 3", "f = If(n != 0, -(x(P) - n)^2)", "M = Maximize(f, P)", "w = x(P)",
], [setn("n", 0), setn("n", 7), mv("B", 14, 0)])
cases.case("maximizepoint", [
    "P = (0, 0)", "a = x(P)", "M = Maximize(a, P)",
], [mv("P", 4, 3)])
cases.case("maximizepoint", [
    "c = Curve(t, t^2, t, -4, 4)", "P = Point(c, 0.75)",
    "M = Maximize(-(x(P) - 1)^2, P)", "w = x(P)",
], [])
cases.case("maximizepoint", [
    "A = (0, 0)", "B = (6, 0)", "P = Point(Segment(A, B))",
    "b = 1", "f = sqrt(x(P) - b)", "M = Maximize(f, P)", "w = x(P)",
], [setn("b", 4), setn("b", -1), setn("b", 1)])

cases.case("maximizepoint", [
    "r = 5", "C = (1, 2)", "c = Circle(C, r)", "P = Point(c)", "M = Maximize(x(P) + y(P), P)",
], [setn("r", -1), setn("r", 2), mv("C", -3, 0)])
cases.case("maximizepoint", [
    "A = (0, 0)", "B = (4, 0)", "C = (4, 3)", "D = (0, 3)", "q = Polygon(A, B, C, D)",
    "F = (1, 1)", "P = Point(q)", "M = Maximize(Distance(P, F), P)", "F1 = (0, 0)", "F2 = (3, 0)", "k = 4", "e = Ellipse(F1, F2, k)",
    "Q = Point(e)", "N = Maximize(x(Q) * y(Q), Q)",
], [mv("C", 6, 5)])
cases.case("minimizepoint", [
    "C = (3, 4)", "c = Circle(C, 5)", "P = Point(c)", "a = y(P)", "M = Minimize(a, P)",
], [mv("P", 8, 4)])
cases.case("minimizepoint", [
    "A = (0, 0)", "B = (10, 0)", "s = Segment(A, B)", "P = Point(s)",
    "n = 3", "f = If(n != 0, (x(P) - n)^2)", "M = Minimize(f, P)", "w = x(P)",
], [setn("n", 0), setn("n", 7), mv("B", 14, 0)])
cases.case("minimizepoint", [
    "P = (2, 3)", "M = Minimize(y(P), P)",
], [mv("P", 4, 3)])
cases.case("minimizepoint", [
    "c = Curve(t, t^2, t, -4, 4)", "P = Point(c, 0.75)",
    "M = Minimize((x(P) + 1)^2, P)", "w = x(P)",
], [])
cases.case("minimizepoint", [
    "A = (0, 0)", "B = (6, 0)", "P = Point(Segment(A, B))",
    "b = 1", "f = sqrt(x(P) - b)", "M = Minimize(f, P)",
], [setn("b", 4), setn("b", -1), setn("b", 1)])
cases.case("minimizepoint", [
    "r = 5", "C = (1, 2)", "c = Circle(C, r)", "P = Point(c)", "M = Minimize(x(P) - 2y(P), P)",
], [setn("r", -1), setn("r", 2), mv("C", -3, 0)])
cases.case("minimizepoint", [
    "A = (0, 0)", "B = (4, 0)", "C = (4, 3)", "D = (0, 3)", "q = Polygon(A, B, C, D)",
    "F = (1, 1)", "P = Point(q)", "M = Minimize(Distance(P, F), P)", "F1 = (0, 0)", "F2 = (3, 0)", "k = 4", "e = Ellipse(F1, F2, k)",
    "Q = Point(e)", "N = Minimize(x(Q) * y(Q), Q)",
], [mv("C", 6, 5)])

# The remaining Sum forms.
cases.case("sum", [
    "l = {1, 10, 0.5}", "a = Sum(l, l)", "b = Sum(l)", "c = Sum(l, 2)",
    "d = Sum(l, 0)",
], [])
# class bounds one longer than the frequencies
cases.case("sum", [
    "b = {1, 2, 3}", "f = {1, 1}", "a = Sum(b, f)",
], [])
# a negative frequency is undefined
cases.case("sum", [
    "b = {1, 2}", "f = {1, -1}", "a = Sum(b, f)",
], [])
# three or more arguments fold; of two, GeoGebra wants a list first
cases.case("sum", [
    "a = Sum(1, 2, 3)", "p = Sum((1, 2), (3, 4), (5, 6))",
    "t = Sum(\"Geo\", \"Gebra\", \"!\")",
], [])
# texts concatenate, one list of them as several
cases.case("sum", [
    "t = Sum({\"Geo\", \"Gebra\"})", "u = Sum({\"a\", \"b\", \"c\"})",
], [])
# functions add as one expression tree (`FunctionFold`)
cases.case("sum", [
    "f(x) = x", "g(x) = 2x", "h = Sum(f, g, f)",
], [])
# the index shadows an existing object (`processSymb` binds it locally)
cases.case("sum", [
    "k = 7", "a = Sum(k^2, k, 1, 5)", "b = k",
], [])
# the index may be the function variable itself: the fold stays a
# function of it
cases.case("sum", [
    "f(x) = x^2", "g = Sum(x^2, x, 1, 3)", "a = g(4)", "b = f(2)",
], [])
# Fractions keep the initial term and then use whole-number indices.
# A reversed range still counts the initial term, even for a constant.
cases.case("sum", [
    "f = Sum(x^2, x, 1.2, 3.9)", "g = Sum(x^2, x, -1.2, 1.9)",
    "h = Sum(x^2, x, 4, 2)", "a = Sum(7, x, 4, 2)",
    "p = Product(x^2, x, 1.2, 3.9)", "q = Product(x^2, x, 1, 3)",
], [])

# Direct vector arguments are refused. A list of vectors does fold.
cases.case("sum", [
    "A = (0, 0)", "u = (1, 2)", "v = (-3, 5)", "w = (2, -7)",
    "d = IsDefined(Sum({u, v, w}))",
], [{"action": "reject", "line": "s1 = Sum(u)"},
    {"action": "reject", "line": "s2 = Sum(u, v)"},
    {"action": "reject", "line": "s3 = Sum(u, v, w)"},
    {"action": "reject", "line": "s4 = Sum(u, v, w, u)"},
    {"action": "reject", "line": "s5 = Sum(A, v, w)"}])
cases.case("sum", [
    "n = 1", "A = (0, 0)", "B = (1/n, n)", "u = Vector(A, B)",
    "v = (-1, 2)", "w = (3, -4)", "d = IsDefined(Sum({u, v, w}))",
], [setn("n", 0), setn("n", 2)])

# The deprecated IntersectPath aliases.
cases.case("alias", [
    pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 3), pt("D", 4, 3),
    "p = Polygon(A, B, C)", "q = Polygon(A, B, D)",
    "r = IntersectRegion(p, q)",
], [mv("C", -2, 3), mv("C", 0, 3)])
cases.case("alias", [
    pt("A", 0, 0), pt("B", 4, 0), pt("C", 0, 3), pt("D", 4, 3),
    "p = Polygon(A, B, C)", "q = Polygon(A, B, D)",
    "r = IntersectionPaths(p, q)",
], [mv("D", -4, 3), mv("D", 4, 3)])
# disjoint polygons: no common region
cases.case("alias", [
    pt("A", 0, 0), pt("B", 2, 0), pt("C", 1, 2),
    pt("D", 5, 5), pt("E", 7, 5), pt("F", 6, 7),
    "p = Polygon(A, B, C)", "q = Polygon(D, E, F)",
    "r = IntersectRegion(p, q)",
], [mv("F", 1, 1)])
# a carrier cuts a polygon to a segment, through both deprecated aliases
cases.case("alias", [
    pt("A", 0, 0), pt("B", 1, 0), pt("C", 2, 1), "d = (1, 0.5)",
    "gl = Line(A, d)", "p = Polygon(A, B, C)",
    "q = IntersectRegion(gl, p)",
], [mv("C", 2, 3), mv("C", 2, 1)])
cases.case("alias", [
    pt("A", 0, 0), pt("B", 1, 0), pt("C", 2, 1), "d = (1, 0.5)",
    "gl = Line(A, d)", "p = Polygon(A, B, C)",
    "q = IntersectionPaths(gl, p)",
], [mv("C", 2, 3), mv("C", 2, 1)])

cases.write()
