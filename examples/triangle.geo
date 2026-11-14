# Triangle ABC with its circumscribed circle,
# plus a second circle of adjustable radius around C.

A = (0, 0)
B = (6, 0)
C = (2, 5)

a = Segment(B, C)
b = Segment(A, C)
c = Segment(A, B)

# Perpendicular bisectors of two sides meet in the circumcentre.
Mc = Midpoint(A, B)
Mb = Midpoint(A, C)
sc = Perpendicular(Mc, c)
sb = Perpendicular(Mb, b)
O = Intersect(sc, sb)
k = Circle(O, A)

# A circle with a radius given by a number ("slider").
r = 3
m = Circle(C, r)
P = Intersect(k, m, 1)
Q = Intersect(k, m, 2)
d = Distance(P, Q)
