"""Geometry forms and updates, starting from the command regression tests."""
from caselib import Cases, mv, setn

cases = Cases("geomclosure")


def case(topic, script, *steps):
    cases.case(topic, [line.strip() for line in script.strip().splitlines()], list(steps))


# cmdPoint and cmdPointIn: the axis, a parameter on a circle, and a triangle.
case("path", """
k: x^2+y^2=1
P = Point(xAxis)
Q = Point(k, 0.25)
""", mv("P", 2, -4))
case("region", """
A = (1, 1)
B = (2, 1/2)
C = (3, 1/3)
p = Polygon(A, B, C)
P = PointIn(p)
""", mv("P", 2, 0.6), mv("P", 6, 4), mv("C", 3, 0), mv("P", 1, 1))
case("region", """
A = (0, 0)
r = 3
k = Circle(A, r)
P = PointIn(k)
""", mv("P", 1, 1), setn("r", 1), mv("P", 4, 0), setn("r", 0), setn("r", -1),
     setn("r", 2), mv("A", 1, -2))
case("regionmoves", """
A = (0, 0)
B = (6, 0)
C = (0, 6)
p = Polygon(A, B, C)
P = PointIn(p)
""", mv("P", 2, 1), mv("B", 12, 3), mv("C", -3, 9), mv("A", -6, -3))

# cmdClosestPoint and cmdDistance: use both argument orders.
case("projection", """
A = (1, 1)
B = (2, 1/2)
C = (3, 1/3)
p = Polygon(A, B, C)
P = (1, 1)
U = ClosestPoint(p, P)
V = ClosestPoint(P, p)
f = Distance(p, P)
g = Distance(P, p)
""", mv("P", 4, 1), mv("C", 3, 0), mv("B", 1, 1), mv("C", 1, 1))

# cmdLength: the length of a point is its distance from the origin.
case("pointlength", """
A = (3, 4)
l = Length(A)
""", mv("A", 0, 0), mv("A", -3, 4))
case("partlength", """
A = (0, 0)
B = (1/pi, 0)
C = (0, 1)
k = CircleArc(A, B, C)
d = Length(k)
""", mv("C", 1, 0), mv("B", 0, 0), mv("B", 2, 0))

# The endpoints are off the underlying conic. The arc has no value, so its
# dependent point can be moved until the next change to an arc input.
case("openarc", """
k: y^2 = 4x
P = (1, -3)
Q = (2, 4)
a = Arc(k, P, Q)
R = Point(a)
""", mv("R", 2, 1), mv("P", 3, -1), mv("Q", 0, 3))
case("openarc", """
k: x^2-y^2 = 1
P = (2, -2)
Q = (3, 1)
a = Arc(k, P, Q)
R = Point(a)
""", mv("R", 2, 1), mv("P", 3, -1), mv("Q", -3, 1))

# cmdPerimeter and cmdLocus: the locus of a horizontal translation.
case("locusmeasure", """
P = Point(xAxis)
Q = P + (1, 0)
loc = Locus(Q, P)
n = Length(loc)
p = Perimeter(loc)
c = Circumference(loc)
""", mv("P", 2, 0))
case("locusmeasure", """
a = Slider(0, 5)
b = 1
P = (a, b*a)
loc = Locus(P, a)
n = Length(loc)
p = Perimeter(loc)
c = Circumference(loc)
""", setn("a", 2), setn("b", 0), setn("b", -3))
case("ode", """
f(x) = x
P = (1, 1)
loc = Locus(f, P)
""", mv("P", -1, 2))

# cmdLocus also integrates a two-variable slope and a quotient slope field.
case("ode", """
A = (0, 0)
r = 1
k = Circle(A, r)
P = Point(k)
loc = Locus(x + y, P)
""", mv("P", 0, 2))
case("slopefield", """
A = (0, 0)
r = 1
k = Circle(A, r)
P = Point(k)
field = SlopeField(x / y)
loc = Locus(field, P)
""", mv("P", 0, 2))

# cmdPolygon and cmdPolyLine: start from the five points in the upstream list.
case("shapelist", """
A = (1, 1)
B = (2, 2)
C = (3, 3)
D = (4, 1/4)
E = (5, 1/5)
t = 1
b = t > 0
l = If(t > 0, {A, B, C, D, E}, {A, B, C})
p = Polygon(l)
q = PolyLine(l)
v = PolyLine(l, b)
P = Vertex(v, 3)
Q = Vertex(v, 5)
""", mv("E", 4, 0.25), setn("t", 0), mv("C", 2, 2), setn("t", 1))
case("shapelist", """
A = (0, 0)
B = (2, 0)
C = (2, 2)
D = (0, 2)
t = 1
p = Polygon(If(t > 0, {A, B, C}, {A, B, C, D}))
q = PolyLine({A, B, C}, true)
""", setn("t", 0), mv("B", 0, 0), setn("t", 1))
case("polylineflag", """
A = (0, 0)
B = (1, 2)
C = (3, 1)
b = false
q = PolyLine(A, B, C, b)
""", mv("C", 3, -1))

# The upstream rigid triangle, with a descendant already attached to B.
case("rigid", """
A = (0, 0)
B = (1, 0)
C = (0, 1)
P = B + (2, 1)
p = RigidPolygon(A, B, C)
""", mv("A", 2, 3), mv("B", 2, 5), mv("A", 0, -1))

# cmdAreEqual and cmdAreCongruent: reversal and cyclic indexing.
case("comparison", """
A = (1, 1)
B = (2, 1/2)
C = (0, 1)
D = (1, 0)
E = (-1, 0)
F = (0, -1)
s = Segment(C, D)
t = Segment(D, C)
u = Segment(E, F)
b = AreEqual(A, B)
c = AreCongruent(s, t)
d = AreCongruent(s, u)
equality = AreEqual(s, t)
""", mv("B", 1, 1), mv("F", 0, -2), mv("D", 0, 1), mv("F", -1, 0))
case("comparison", """
A = (0, 0)
B = (2, 0)
C = (2, 3)
D = (1, 4)
E = (-1, 4)
p = Polygon(A, B, C, D, E)
q = Polygon(D, E, A, B, C)
r = Polygon(C, B, A, E, D)
u = AreCongruent(p, q)
v = AreCongruent(p, r)
equality = AreEqual(p, q)
""", mv("B", 0, 0), mv("D", 0, 0))

case("comparison", """
A = (0, 0)
P = (1, 0)
Q = (0, 1)
R = (0, 1)
k = Circle(A, P)
u = Arc(k, P, Q)
v = Arc(k, P, R)
s = Sector(k, P, Q)
b = AreEqual(u, v)
c = AreEqual(u, s)
d = AreEqual(k, u)
f = AreEqual(u, k)
l = {{P, Q}, {1, 2}}
m = {{P, R}, {1, 2}}
g = AreEqual(l, m)
""", mv("R", -1, 0), mv("Q", -1, 0))

case("viewcorner", """
n = 1
v = 1
P = Vertex(n)
Q = Vertex(v, n)
""", setn("n", 2), setn("n", 5), setn("n", 6), setn("v", 2), setn("v", 1), setn("n", 0),
     setn("n", 3), setn("n", 4), setn("n", 5), setn("v", 2), setn("n", 2))
case("textlength", '''
n = Length("line 😀")
''')
# cmdLength tests both numeric bounds and endpoints supplied off the curve.
case("curvelength", """
f = Curve(3t, 4t, t, 0, 10)
a = 2
b = 3
A = (3, 5)
B = (6, 9)
l = Length(f, a, b)
m = Length(f, A, B)
""", setn("a", 3), setn("b", 1), mv("A", 6, 9), mv("B", 0, 0))

cases.write()
