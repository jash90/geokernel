"""Numerical Relation cases recorded without opening the relation dialog."""

from caselib import Cases, mv, setn


c = Cases("relationreview")

c.case("paths", [
    "A = (0,0)", "B = (100,0)", "P = (50,0.000000005)",
    "g = Line(A,B)", "s = Segment(A,B)", "r = Ray(A,B)", "v = Vector(A,B)",
    "ml = Relation(P,g)", "ms = Relation(P,s)", "mr = Relation(r,P)",
    "mv = Relation(v,P)",
], [
    {"action": "move", "label": "P", "x": 100.0000005, "y": 0},
    {"action": "move", "label": "P", "x": -0.0000005, "y": 0},
    {"action": "move", "label": "P", "x": -0.000002, "y": 0},
    mv("P", 101, 0),
])

c.case("paths", [
    "A = (0,0)", "B = (4,0)", "Q = (0,0)", "s = Segment(A,B)",
    "P = Point(s)", "m = Relation(P,s)", "n = Relation(Q,s)",
], [mv("B", 0, 0), mv("B", 4, 0)])

c.case("segments", [
    "A = (0,0)", "B = (2,0)", "C = (1,1)", "D = (3,1)",
    "s = Segment(A,B)", "t = Segment(C,D)", "m = Relation(s,t)",
], [mv("D", 1, 3), mv("D", 2, 2), mv("C", 0, 0), mv("D", 2, 0)])

c.case("vectors", [
    "A = (2,3)", "B = (102,3)", "C = (0,0)", "D = (100,0)",
    "P = (52,3.000000005)", "v = Vector(A,B)", "w = Vector(C,D)",
    "m = Relation(v,w)", "n = Relation(P,v)",
], [mv("D", 50, 0), mv("D", 0, 1), mv("P", 52, 0)])

c.case("polygons", [
    "A = (0,0)", "B = (100,0)", "C = (100,2)",
    "D = (0,10)", "E = (100,10)", "F = (100,12)",
    "P = (50,0.000000005)", "p = Polygon(A,B,C)", "q = Polygon(D,E,F)",
    "m = Relation(p,P)", "n = Relation(p,q)",
], [mv("P", 50, 0.5), mv("P", 50, 1), mv("F", 100, 14)])

c.case("arcs", [
    "O = (0,0)", "A = (3,0)", "B = (0,3)", "C = (-3,0)", "P = (3,0)",
    "a = CircularArc(O,A,B)", "b = CircularArc(O,B,C)",
    "m = Relation(a,b)", "n = Relation(P,a)",
], [mv("P", -3, 0), mv("P", 0, 3), mv("P", 0, 3.000001), mv("C", 0, -3)])

c.case("conics", [
    "O = (0,0)", "A = (4,0)", "r = 2", "c = Circle(O,r)",
    "d = Circle(A,r)", "m = Relation(c,d)",
], [mv("A", 5, 0), mv("A", 3, 0), mv("A", 0, 0)])

c.case("conics", [
    "O = (0,0)", "A = (2,0)", "r = 2", "s = 0", "c = Circle(O,r)",
    "d = Circle(A,s)", "m = Relation(c,d)",
], [mv("A", 3, 0), mv("A", 2, 0), setn("s", 2)])

c.case("asymptotes", [
    "d = 1", "k: x^2-d*y^2 = 1", "h = Asymptote(k)", "g: x-y = 0",
    "m = Relation(h_{1},k)", "n = Relation(k,h_{2})", "o = Relation(g,k)",
], [setn("d", 4), setn("d", -1), setn("d", 1)])

c.write()
