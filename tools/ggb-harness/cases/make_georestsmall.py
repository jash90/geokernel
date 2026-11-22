"""Record point surds, transformations of Cartesian curves and geometry conditionals."""

from caselib import Cases, mv, setn


cases = Cases("georestsmall")


def case(topic, script, steps=(), *, curve=False):
    cases.case(topic, script.splitlines(), list(steps))
    if curve:
        cases.cases[-1]["sample_curves"] = True


# CommandsTest.cmdSurdText tests the point form at (1, 1).
case("surdpoint", """A = (1, 1)
s = SurdText(A)""", [mv("A", 0.5, -0.75), mv("A", 0, 0)])
case("surdpoint", """A = (1, 2)
B = (2, 3)
g = Line(A, B)
P = Intersect(g, xAxis)
s = SurdText(P)
Q = (sqrt(2), -sqrt(3) / 2)
u = SurdText(Q)""", [mv("B", 2, 2), mv("A", 0, 2), mv("B", 2, -2)])

case("dilatecurve", """A = (1, 2)
n = 2
r = 1 / n
c = Curve(t + x(A), t^2 + y(A), t, -2, 3)
d = Dilate(c, r)
q = Dilate(c, r, A)""", [mv("A", -1, 3), setn("n", -2), setn("n", 0), setn("n", 1)], curve=True)
case("dilatecurve", """A = (1, 2)
B = (4, 0)
n = 2
c = Curve(t + x(B), t^2 + y(B), t, 0, n)
d = Dilate(c, n)
q = Dilate(c, n, A)""", [mv("B", 1, 2), setn("n", 0), setn("n", -1), setn("n", 2)], curve=True)

case("matrixcurve", """A = (2, 1)
n = 2
c = Curve(t + x(A), t^2 + y(A), t, -2, 2)
m = {{n, 1}, {0, 1}}
d = ApplyMatrix(m, c)
r = {{1, 0, 2}, {0, 1, 1}, {1, 0, n}}
q = ApplyMatrix(r, d)""", [setn("n", 0), mv("A", 0, -2), setn("n", -1)], curve=True)
case("matrixcurve", """A = (1, 2)
n = 2
c = Curve(t, t^2 + x(A), t, -2, 3)
m = {{1, 0}, Sequence(n)}
d = ApplyMatrix(m, c)""", [setn("n", 3), mv("A", 0, 2), setn("n", 2)], curve=True)

case("matrixfunction", """A = (1, 2)
n = 2
f(x) = x^2 + x(A)
m = {{n, 1}, {1, 0}}
c = ApplyMatrix(m, f)
r = {{1, 1, 0}, {1, 0, 1}, {0, 1, n}}
d = ApplyMatrix(r, f)""", [mv("A", -1, 2), setn("n", -1), setn("n", 0)], curve=True)
case("matrixfunction", """A = (1, 2)
n = 2
f = Function(sin(x) + x(A), -1, 2)
m = {{1, 0}, Sequence(n)}
c = ApplyMatrix(m, f)""", [setn("n", 3), mv("A", 0, 2), setn("n", 2)], curve=True)

case("dilatecurve", """A = (2, 3)
n = 2
c = Curve(t, x(A) * t, t, 0, 1)
d = Dilate(c, n, A)
l = Length(d, 0, 1)""", [mv("A", 0, 0), setn("n", -1)], curve=True)
case("matrixcurve", """A = (2, 3)
n = 2
c = Curve(t, x(A) * t, t, 0, 1)
m = {{1, 0}, {0, n}}
d = ApplyMatrix(m, c)
l = Length(d, 0, 1)""", [mv("A", 1, 0), setn("n", 0)], curve=True)

# CommandsTest.cmdIf covers coordinate branches. These constructions test
# the same selection rules with geometric objects and dependent readers.
case("ifgeometry", """A = (0, 0)
B = (2, 1)
C = (1, 0)
D = (1, 1)
g = Line(A, B)
h = Line(C, D)
b = x(B) > 0
e_{l} = y(B) > 0
l = If(b, g, e_{l}, h, xAxis)
q = If(x(B) > 0, g)
P = Intersect(l, yAxis)""", [mv("B", -2, 1), mv("B", -2, -1), mv("B", 0, 0), mv("B", 2, 0)])
case("ifgeometry", """A = (0, 0)
B = (2, 1)
C = (1, -2)
D = (1, 1)
g = Segment(A, B)
h = Segment(C, D)
b = x(B) > 0
l = If(b, g, h)
M = Midpoint(l)
r = Ray(A, B)
s = Ray(C, D)
u = If(b, r, s)""", [mv("B", -2, 1), mv("D", 1, -2), mv("B", 2, 0)])
case("ifgeometry", """A = (0, 0)
B = (2, 1)
C = (1, -2)
r = 1 / x(B)
k = Circle(A, r)
m = Circle(C, 2)
b = y(B) > 0
q = If(b, k, m)
d = Radius(q)""", [mv("B", 0, 1), mv("B", 1, -1), mv("C", 2, 3), mv("B", 1, 1)])
case("ifgeometry", """A = (0, 0)
B = (2, 1)
r = x(A)
k: 2x^2 + 3y^2 = 7
m: 3x^2 + 2y^2 + r x = 7
b = y(B) > 0
q = If(b, k, m)
d = FirstAxisLength(q)""", [mv("B", 2, -1), mv("A", 1, 2), mv("B", 2, 1)])

cases.write()
