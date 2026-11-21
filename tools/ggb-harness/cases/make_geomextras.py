"""Record 2D geometry command forms and their updates."""
from caselib import Cases, mv, setn

cases = Cases("geomextras")
reference_cases = Cases("geomextras")


def case(topic, script, *steps):
    cases.case(topic, script.strip().splitlines(), list(steps))


def reference(topic, script, *steps):
    reference_cases.case(topic, script.strip().splitlines(), list(steps))


# CommandsTest.cmdLength and cmdTangent: separate the function and point
# arguments so the same inputs can be moved or edited after construction.
reference("lengthpoint", """
P = (3, 4)
d = Length(P)
""", mv("P", 0, 0), mv("P", -6, 8))
case("lengthfunction", """
f(x) = 3*x/4
a = 0
b = 4
P = (0, 1)
Q = (4, 4)
d = Length(f, a, b)
s = Length(f, P, Q)
""", setn("b", 0), setn("b", -4), mv("Q", 0, 1), mv("Q", -4, 7))
case("lengthfunction", """
a = 1
b = 4
f(x) = x^2
g(x) = sqrt(x)
d = Length(f, a, b)
s = Length(g, a, b)
""", setn("a", -1), setn("a", 1), setn("b", 1))
case("lengthfunction", """
a = 1
f(x) = a*x^2 + 100000000000000000000
d = Length(f, 0, 4)
""", setn("a", 0), setn("a", -2))
case("lengthfunction", """
a = 1
b = 4
f(x) = sqrt(x)
d = Length(f, a, b)
""", setn("a", 0), setn("a", -1), setn("a", 1))
# The arc length integrand runs through GeoGebra's adaptive Gauss
# quadrature: long and oscillating intervals stay inside its panel budget,
# one that ends at a pole of the integrand does not, and a cusp at the
# endpoint is chorded off (upstream t8..t17).
case("lengthfunction", """
a = 0
b = 120
f(x) = sin(x)
d = Length(f, a, b)
""", setn("b", 150), setn("b", 300), setn("b", 600), setn("b", 4))
case("lengthfunction", """
a = 0
b = 10
f(x) = sin(10x)
d = Length(f, a, b)
""", setn("b", 100), setn("b", 3))
case("lengthfunction", """
a = 0
b = 1
f(x) = tan(x)
d = Length(f, a, b)
""", setn("b", 1.5707963), setn("b", 1.5707963267948966), setn("b", 2))
case("lengthfunction", """
a = -1
b = 1
f(x) = x^(2/3)
d = Length(f, a, b)
""", setn("b", 8), setn("a", -8))
# A function undefined at an endpoint but defined inside: the open
# interval is integrated.
case("lengthfunction", """
f(x) = sin(x)/x
a = 0
b = 1
d = Length(f, a, b)
""", setn("b", 2))
case("lengthfunction", """
f(x) = (x^2 - 1)/(x - 1)
a = 1
b = 2
d = Length(f, a, b)
""", setn("a", 0), setn("a", 1))
case("lengthfunction", """
f(x) = x*ln(x)
a = 0
b = 1
d = Length(f, a, b)
""", setn("b", 2))
# A number expression among the bounds is read as its own object.
case("lengthfunction", """
a = 2
f(x) = sqrt(1 + x^2)
d = Length(f, 0, 1e5)
e = Length(f, 0, 2a)
s = Length(f, 0, 1E5)
""", setn("a", 3))
case("tangentfunction", """
f(x) = sin(x)
P = (1, 1)
a = 42
t = Tangent(a, f)
u = Tangent(P, f)
v = Tangent(f, P)
""", setn("a", 0), setn("a", -1), mv("P", 0, 99), mv("P", -1, 2))
case("tangentfunction", """
f(x) = sqrt(x)
P = (4, 10)
t = Tangent(P, f)
""", mv("P", 0, 3), mv("P", -1, 3), mv("P", 1, -3))
# The derivative of a non-smooth function follows GeoGebra's symbolic
# rules: abs is undefined where its argument is zero, floor, ceil, round
# and sgn have derivative zero, and If differentiates the branch the point
# selects (upstream t1..t7).
case("tangentfunction", """
f(x) = abs(x)
a = 0
t = Tangent(a, f)
""", setn("a", 1), setn("a", -2))
case("tangentfunction", """
f(x) = abs(x - 1)
g(x) = abs(x^2 - 1)
a = 1
s = Tangent(a, f)
b = 0
t = Tangent(b, g)
""", setn("a", 0), setn("b", 1), setn("b", 2), setn("a", 2))
case("tangentfunction", """
f(x) = floor(x)
g(x) = ceil(x)
h(x) = round(x)
a = 1
s = Tangent(a, f)
t = Tangent(a, g)
u = Tangent(0.5, h)
""", setn("a", 2), setn("a", 0))
case("tangentfunction", """
f(x) = sgn(x)
a = 0
s = Tangent(a, f)
t = Tangent(2, f)
""", setn("a", 1), setn("a", -3))
case("tangentfunction", """
f(x) = If(x < 1, x, 2x)
g(x) = If(x < 1, x^2, x + 5)
a = 1
s = Tangent(a, f)
t = Tangent(a, g)
""", setn("a", 0), setn("a", 3))
# A number expression among the arguments is read as its own object.
case("tangentfunction", """
a = 1
f(x) = x^2
t = Tangent(a + 1, f)
u = Tangent(pi, f)
""", setn("a", 0), setn("a", -1))
case("tangentreverse", """
k = Conic({1, 0.5, -1, 0, 0, 0})
P = (1, 1)
t = Tangent(k, P)
""", mv("P", 1, 0), mv("P", 0, 0), mv("P", -3, 2))

# CommandsTest.cmdVertex: move each endpoint and exercise the rounded index.
case("vertexsegment", """
A = (1, 0)
B = (0, 1)
s = Segment(A, B)
n = 1
V = Vertex(s, n)
""", setn("n", 1.5), mv("B", 3, 4), setn("n", 3), setn("n", 1), mv("A", 3, 4))
case("vertexsegment", """
a = 1
A = (0, 0)
B = (2, sqrt(a))
s = Segment(A, B)
n = 1
V = Vertex(s, n)
""", setn("a", -1), mv("A", 3, 4), setn("n", 2), setn("a", 4))
# An index that is a number expression is read as its own object.
case("vertexsegment", """
A = (1, 0)
B = (0, 1)
s = Segment(A, B)
a = 1
V = Vertex(s, a + 1)
""", setn("a", 0), setn("a", -1), mv("B", 3, 4))

# CommandsTest.cmdPoint, including both list shapes supported in 2D.
case("pointlist", """
a = 1
b = 2
l = {a, b}
P = Point(l)
""", setn("a", 0), setn("b", -2))
case("pointlist", """
a = 1
b = 2
l = {{a, b}, {b, a}}
P = Point(l)
""", setn("a", -2), setn("b", 0))
case("pointlist", """
a = 1
l = If(a > 0, {{a, 2}, {3, a}}, {{a, 2}})
P = Point(l)
""", setn("a", 0), setn("a", 3))
# A list literal stays one argument: a flat list makes a single point, a
# list of rows one point per row, and a row of three numbers an undefined
# (3D) point.
case("pointlist", """
P = Point({1, 2})
""")
case("pointlist", """
Q = Point({{1, 2}, {3, 4}})
""")
case("pointlist", """
R = Point({1, 2, 3})
""")
# A list that gains a row it never had: GeoGebra appends a labelled point
# for the new row, the kernel's outputs are fixed when the command runs.
# Record the label GeoGebra gives the appended point on the update.
case("pointlist", """
a = 1
l = If(a > 0, {{a, 2}}, {{a, 2}, {3, 4}})
P = Point(l)
""", setn("a", 0), setn("a", 1))
cases.cases[-1]["track_point_list_growth"] = True
case("pointlist", """
a = 1
l = If(a > 0, {{a, 2}, {3, 4}}, {{a, 2}, {3, 4, 5}})
P = Point(l)
""", setn("a", 0), setn("a", 1))
case("pointvector", """
P = (1, 1)
v = (3, 4)
Q = Point(P, v)
""", mv("v", 0, 0), mv("P", -5, 2), mv("v", 3, -1))
case("pointpath", """
A = (0, 0)
r = 1
k = Circle(A, r)
a = 0.25
P = Point(xAxis)
Q = Point(k, a)
""", mv("P", -1, 3), setn("a", 0.75), setn("r", 0), setn("r", 2))

# CommandsTest.cmdArc and cmdSector: start off the conic and then
# cross the parameter cut, including coincident endpoints.
reference("conicparts", """
k = Conic({1, 0.5, -1, 0, 0, 0})
P = (1, 1)
Q = (2, 1/2)
a = Arc(k, P, Q)
s = Sector(k, P, Q)
""", mv("Q", 1, 1), mv("P", -1, -1), mv("Q", 0, -2))
reference("conicparts", """
k = Conic({1, 1, -1, 0, 0, 0})
P = (1, 1)
Q = (2, 1/2)
s = Sector(k, P, Q)
""", mv("Q", 1, 1), mv("P", -1, -1), mv("Q", 0, -2))
# cmdCircumference and cmdPerimeter use a unit circle entered as a conic.
case("perimeterconic", """
a = 1
b = -1
k = Conic({1, a, b, 0, 0, 0})
c = Circumference(k)
d = Perimeter(k)
""", setn("a", 0.5), setn("b", 0), setn("b", -1), setn("a", -1), setn("a", 0.01))
case("circumferencepolygon", """
A = (1, 1)
B = (2, 1/2)
C = (3, 1/3)
p = Polygon(A, B, C)
d = Circumference(p)
""", mv("C", 3, 0), mv("C", 2, 0.5), mv("C", -3, 1))

# CommandsTest.cmdBarycenter: move across positive, zero and negative
# total weight. Also record differing lengths.
case("barycenter", """
A = (1, 1)
B = (2, 1/2)
C = (3, 1/3)
a = 42
l = {A, B, C}
w = {a, 4, 13}
P = Barycenter(l, w)
""", setn("a", -17), setn("a", -42), mv("C", 2, 0.5), setn("a", 0))
case("barycenter", """
A = (1, 1)
B = (2, 1/2)
l = {A, B}
w = {42}
P = Barycenter(l, w)
""", mv("A", 1, -1))
# Inline list literals stay whole as arguments, and the weights can be
# number expressions of a moved number.
case("barycenter", """
A = (1, 1)
B = (2, 1/2)
P = Barycenter({A, B}, {1, 2})
""", mv("A", 1, -1))
case("barycenter", """
A = (1, 1)
B = (2, 1/2)
C = (3, 1/3)
a = 1
P = Barycenter({A, B, C}, {a, a + 1, 2a})
""", setn("a", -1), setn("a", 0))

case("barycenter", """
A = (1, 2)
B = (3, 5)
a = 1
l = {A, B}
w = If(a > 0, {1, a}, {1})
P = Barycenter(l, w)
""", setn("a", 0), mv("B", 4, 6), setn("a", 2))

# CommandsTest.cmdPointIn, cmdClosestPoint, cmdDistance, cmdAreEqual,
# cmdAreCongruent and cmdRigidPolygon illustrate existing dispatch.
case("pathandregion", """
A = (1, 1)
B = (2, 1/2)
C = (3, 1/3)
P = (1, 1)
p = Polygon(A, B, C)
Q = PointIn(p)
R = ClosestPoint(p, P)
S = ClosestPoint(P, p)
u = Distance(P, p)
v = Distance(p, P)
""", mv("P", 2, -1), mv("P", 3, 1), mv("C", 3, 0))
case("predicates", """
A = (1, 1)
B = (2, 1/2)
C = (0, 1)
D = (1, 0)
E = (-1, 0)
F = (0, -1)
s = Segment(C, D)
t = Segment(E, F)
b = AreEqual(A, B)
g = AreCongruent(s, t)
""", mv("F", 0, -2), mv("B", 1, 1), mv("F", 0, -1))
case("rigidpolygon", """
A = (0, 0)
B = (1, 0)
C = (0, 1)
q = Polygon(A, B, C)
p = RigidPolygon(q)
""", mv("B", 3, 0), mv("C", 0, 2), mv("A", -1, 1))
case("rigidpolygon", """
A = (0, 0)
B = (1, 0)
C = (0, 1)
q = Polygon(A, B, C)
p = RigidPolygon(q, 2, 1)
""", mv("A", 0, 1), mv("B", 2, 2))
# The points form replaces its second and later points with dependent
# ones upstream, so moving the first point carries them along; the kernel
# keeps the free points (an exact gap, recorded in known-gaps.toml).
case("rigidpoints", """
A = (0, 0)
B = (1, 0)
C = (0, 1)
p = RigidPolygon(A, B, C)
""", mv("A", 2, 3), mv("A", 0, -1))

# CommandsTest.cmdPolygon and cmdPolyLine: their list input contains points.
five_points = """
A = (1, 1)
B = (2, 2)
C = (3, 3)
D = (4, 1/4)
E = (5, 1/5)
l = {A, B, C, D, E}
"""
reference("polygonlist", five_points + "p = Polygon(l)", mv("E", 4, 0.25))
reference("polylinelist", five_points + "p = PolyLine(l)", mv("E", 4, 0.25))
reference("polylinelist", five_points + """b = false
p = PolyLine(l, b)
""", mv("E", 4, 0.25))
case("polylineflag", """
A = (1, 1)
C = (3, 1/3)
E = (5, 1/5)
a = 1
b = a < 0
p = PolyLine(A, C, E, b)
""", mv("E", 3, 1.0 / 3), setn("a", -1), mv("C", 1, 1))

reference("directionvector", """
v = (3, 2)
d = Direction(v)
""", mv("v", 0, 0), mv("v", -2, 3))

cases.write()
reference_cases.write("reference")
