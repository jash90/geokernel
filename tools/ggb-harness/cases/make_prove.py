"""Record the symbolic prover: Prove(boolean) and ProveDetails(boolean[, boolean]).

The reference result depends only on the construction and the statement, not
on coordinates, and is kept while points move (the reference recomputes only
when the statement's text changes). The moves check that the kernel keeps it
too, also through degenerate positions.
"""

from caselib import Cases, mv

cases = Cases("prove")


def proof(topic, script, statement, steps=(), details=True, relation=False):
    lines = list(script) + [f"b = {statement}", "p = Prove(b)"]
    if details:
        lines.append("q = ProveDetails(b)")
    if relation:
        lines.append("r = ProveDetails(b, true)")
    cases.case(topic, lines, list(steps))


TRI = ["A = (0, 0)", "B = (5, 1)", "C = (2, 4)"]
QUAD = TRI + ["D = (-1, 3)"]

# Linear constructions: lines, midpoints, perpendiculars, parallels.
cases.topic("linear")
proof("linear", TRI + ["D = Midpoint(A, B)", "E = Midpoint(A, C)", "f = Line(D, E)",
                       "g = Line(B, C)"], "AreParallel(f, g)",
      [mv("A", 1, 1), mv("A", 5, 1), mv("A", 0, 0)])
proof("linear", TRI, "AreCollinear(A, B, C)", [mv("C", 10, 2)])
proof("linear", TRI + ["D = Midpoint(B, C)", "E = Midpoint(A, C)", "F = Midpoint(A, B)",
                       "f = Line(A, D)", "g = Line(B, E)", "h = Line(C, F)"],
      "AreConcurrent(f, g, h)", [mv("C", 10, 2)])
proof("linear", TRI + ["a = Line(B, C)", "d = Line(A, C)", "e = Line(A, B)",
                       "f = PerpendicularLine(A, a)", "g = PerpendicularLine(B, d)",
                       "h = PerpendicularLine(C, e)"], "AreConcurrent(f, g, h)",
      [mv("C", 10, 2), mv("C", 2, 4)])
proof("linear", TRI + ["f = PerpendicularBisector(A, B)", "g = PerpendicularBisector(B, C)",
                       "h = PerpendicularBisector(A, C)"], "AreConcurrent(f, g, h)")
proof("linear", QUAD + ["E = Midpoint(A, B)", "F = Midpoint(B, C)", "G = Midpoint(C, D)",
                        "H = Midpoint(D, A)", "f = Line(E, F)", "g = Line(G, H)"],
      "AreParallel(f, g)", [mv("D", 3, 3)])
proof("linear", TRI + ["a = Line(B, C)", "e = Line(A, B)", "f = Line(A, a)", "g = Line(C, e)",
                       "D = Intersect(f, g)", "M = Midpoint(A, C)"], "AreCollinear(B, D, M)",
      [mv("C", 10, 2)])
proof("linear", TRI + ["a = Line(B, C)", "f = PerpendicularLine(A, a)",
                       "g = PerpendicularLine(B, f)"], "AreParallel(a, g)")
proof("linear", TRI + ["a = Line(B, C)", "f = Line(A, a)", "g = PerpendicularLine(C, f)"],
      "ArePerpendicular(a, g)")
proof("linear", TRI + ["e = Line(A, B)", "d = Line(A, C)"], "ArePerpendicular(e, d)")
proof("linear", TRI + ["e = Line(A, B)", "d = Line(A, C)"], "AreParallel(e, d)")
proof("linear", TRI + ["D = Midpoint(A, B)", "E = Midpoint(B, A)"], "AreEqual(D, E)")
proof("linear", TRI + ["D = Midpoint(A, B)", "E = Midpoint(B, C)"], "AreEqual(D, E)")
proof("linear", TRI + ["D = Midpoint(A, B)"], "AreCollinear(A, D, B)")
proof("linear", TRI + ["D = Midpoint(A, B)", "E = Midpoint(A, D)", "F = Midpoint(D, B)",
                       "G = Midpoint(E, F)"], "AreEqual(G, D)")
proof("linear", TRI + ["e = Line(A, B)", "P = Point(e)", "M = Midpoint(A, P)"],
      "AreCollinear(B, M, P)", [mv("A", 2, 4)])
proof("linear", TRI + ["e = Segment(A, B)", "P = Point(e)"], "AreCollinear(A, B, P)")
proof("linear", TRI + ["e = Line(A, B)", "P = Point(e)"], "AreCollinear(A, C, P)")
proof("linear", TRI + ["D = Midpoint(A, B)", "E = Midpoint(A, C)", "F = Midpoint(B, C)",
                       "f = Line(D, E)", "g = Line(E, F)", "h = Line(D, F)"],
      "AreConcurrent(f, g, h)")
proof("linear", TRI + ["a = Line(B, C)", "f = PerpendicularLine(A, a)", "H = Intersect(f, a)",
                       "e = Line(A, H)"], "ArePerpendicular(e, a)")
proof("linear", TRI + ["a = Line(B, C)", "e = Line(A, B)", "f = Line(C, e)", "g = Line(A, a)"],
      "AreParallel(f, e)")
proof("linear", TRI + ["e = Line(A, B)", "f = Line(B, A)"], "AreEqual(e, f)")
proof("linear", TRI + ["e = Line(A, B)", "f = Line(A, C)"], "AreEqual(e, f)")
proof("linear", QUAD + ["E = Midpoint(A, C)", "F = Midpoint(B, D)", "G = Midpoint(A, B)",
                        "H = Midpoint(C, D)", "M = Midpoint(E, F)"], "AreCollinear(M, G, H)")
proof("linear", QUAD + ["f = Line(A, B)", "g = Line(C, D)", "E = Intersect(f, g)",
                        "h = Line(A, C)", "k = Line(B, D)", "F = Intersect(h, k)",
                        "M = Midpoint(A, D)"], "AreCollinear(E, F, M)")
proof("linear", TRI + ["D = Midpoint(A, B)", "G = Midpoint(D, C)"], "AreCollinear(A, B, G)")

# Circles through a point, through three points, and points on them.
cases.topic("circle")
proof("circle", TRI + ["c = Circle(A, B, C)", "P = Point(c)"], "AreConcyclic(A, B, C, P)")
proof("circle", QUAD, "AreConcyclic(A, B, C, D)")
proof("circle", ["A = (0, 0)", "B = (4, 2)", "M = Midpoint(A, B)", "c = Circle(M, A)",
                 "P = Point(c, 0.25)", "f = Line(A, P)", "g = Line(B, P)"], "ArePerpendicular(f, g)",
      [mv("B", 0, 4)])
proof("circle", ["O = (0, 0)", "A = (3, 1)", "c = Circle(O, A)", "P = Point(c)",
                 "f = PerpendicularBisector(A, P)", "M = Midpoint(A, P)", "g = Line(O, M)"],
      "AreEqual(f, g)")
proof("circle", ["O = (0, 0)", "A = (3, 1)", "c = Circle(O, A)", "P = Point(c)",
                 "Q = Point(c)"], "AreCollinear(O, P, Q)")
proof("circle", TRI + ["c = Circle(A, B, C)", "f = PerpendicularBisector(A, B)",
                       "g = PerpendicularBisector(B, C)", "O = Intersect(f, g)", "d = Circle(O, A)"],
      "AreEqual(c, d)")
proof("circle", TRI + ["c = Circle(A, B)", "d = Circle(A, B)"], "AreEqual(c, d)")
proof("circle", TRI + ["c = Circle(A, B)", "P = Point(c)", "M = Midpoint(B, P)",
                       "f = Line(A, M)", "g = Line(B, P)"], "ArePerpendicular(f, g)")
proof("circle", TRI + ["c = Circle(A, B)", "P = Point(c)", "M = Midpoint(B, P)",
                       "f = Line(A, M)", "g = Line(B, P)"], "AreParallel(f, g)")

# Statements without a construction, constants and unsupported objects.
cases.topic("special")
cases.case("special", ["p = Prove(true)", "q = ProveDetails(true)"], [])
cases.case("special", ["p = Prove(false)", "q = ProveDetails(false)"], [])
cases.case("special", ["b = true", "p = Prove(b)", "q = ProveDetails(b)"], [])
proof("special", ["A = (1, 2)", "O = (0, 0)", "P = Point(xAxis)"], "AreCollinear(A, P, O)")
proof("special", TRI + ["f = Line(A, xAxis)", "g = Line(B, xAxis)"], "AreParallel(f, g)")

# The relation-tool form of ProveDetails.
cases.topic("relation")
proof("relation", TRI + ["D = Midpoint(A, B)", "E = Midpoint(A, C)", "f = Line(D, E)",
                         "g = Line(B, C)"], "AreParallel(f, g)", relation=True)
proof("relation", ["A = (0, 0)", "B = (4, 2)", "M = Midpoint(A, B)", "c = Circle(M, A)",
                   "P = Point(c, 0.25)", "f = Line(A, P)", "g = Line(B, P)"],
      "ArePerpendicular(f, g)", relation=True)
proof("relation", TRI, "AreCollinear(A, B, C)", relation=True)

# Intersections of a line and a circle, and of two circles.
cases.topic("intersect")
proof("intersect", ["O = (0, 0)", "A = (3, 1)", "B = (1, 4)", "c = Circle(O, A)",
                    "g = Line(O, B)", "P = Intersect(g, c, 1)", "Q = Intersect(g, c, 2)",
                    "M = Midpoint(P, Q)"], "AreEqual(M, O)", [mv("B", 0, 0), mv("B", 1, 4)])
proof("intersect", ["A = (0, 0)", "B = (4, 2)", "C = (1, 3)", "M = Midpoint(A, B)",
                    "c = Circle(M, A)", "g = Line(A, C)", "P = Intersect(g, c, 1)",
                    "h = Line(P, B)"], "ArePerpendicular(g, h)", [mv("C", 3, -1)])
proof("intersect", ["A = (0, 0)", "B = (4, 1)", "c = Circle(A, B)", "d = Circle(B, A)",
                    "C = Intersect(c, d, 1)", "M = Midpoint(A, B)", "f = Line(C, M)",
                    "g = Line(A, B)"], "ArePerpendicular(f, g)", [mv("B", 0, 0), mv("B", 4, 1)])
proof("intersect", ["A = (0, 0)", "B = (4, 1)", "c = Circle(A, B)", "d = Circle(B, A)",
                    "C = Intersect(c, d, 1)", "D = Intersect(c, d, 2)", "f = Line(C, D)",
                    "g = Line(A, B)"], "ArePerpendicular(f, g)")
proof("intersect", ["A = (0, 0)", "B = (4, 1)", "C = (1, 3)", "c = Circle(A, B)",
                    "d = Circle(C, B)", "P = Intersect(c, d, 1)", "M = Midpoint(B, P)"],
      "AreCollinear(A, C, M)", [mv("C", 8, 2)])
proof("intersect", ["A = (0, 0)", "B = (4, 1)", "C = (1, 3)", "c = Circle(A, B)",
                    "g = Line(B, C)", "P = Intersect(g, c, 1)"], "AreCollinear(P, A, C)")
proof("intersect", ["O = (0, 0)", "A = (3, 1)", "c = Circle(O, A)", "g = Line(O, A)",
                    "P = Intersect(g, c, 1)", "M = Midpoint(A, P)"], "AreEqual(M, O)")
proof("intersect", ["O = (0, 0)", "A = (3, 1)", "B = (1, 4)", "C = (-2, 1)", "c = Circle(O, A)",
                    "g = Line(B, C)", "P = Intersect(g, c, 1)", "Q = Intersect(g, c, 2)",
                    "M = Midpoint(P, Q)", "h = Line(O, M)"], "ArePerpendicular(g, h)")
proof("intersect", ["O = (0, 0)", "A = (3, 1)", "B = (1, 4)", "C = (-2, 1)", "c = Circle(O, A)",
                    "g = Line(B, C)", "P = Intersect(g, c, 1)", "M = Midpoint(B, C)",
                    "h = Line(O, M)"], "ArePerpendicular(g, h)")
proof("intersect", ["A = (0, 0)", "B = (4, 1)", "C = (1, 3)", "c = Circle(A, B, C)",
                    "g = Line(A, C)", "D = (5, 5)", "h = Line(B, D)", "P = Intersect(g, c, 1)",
                    "Q = Intersect(h, c, 1)"], "AreConcyclic(A, B, P, Q)")
proof("intersect", ["A = (0, 0)", "B = (4, 1)", "C = (1, 3)", "c = Circle(A, B)",
                    "d = Circle(B, C)", "P = Intersect(c, d, 1)", "Q = Intersect(c, d, 2)",
                    "f = Line(P, Q)", "g = Line(A, B)"], "ArePerpendicular(f, g)")

# Statements whose conditions involve more points.
cases.topic("conditions")
proof("conditions", QUAD + ["f = Line(A, B)", "g = Line(C, D)", "E = Intersect(f, g)"],
      "AreCollinear(A, B, E)", [mv("D", 4, 5), mv("D", -1, 3)])
proof("conditions", QUAD + ["f = Line(A, B)", "g = Line(C, D)", "E = Intersect(f, g)",
                            "h = Line(A, C)", "k = Line(B, D)", "F = Intersect(h, k)"],
      "AreCollinear(E, F, A)")
proof("conditions", TRI + ["e = Line(A, B)", "P = Point(e)", "Q = Point(e)"],
      "AreCollinear(C, P, Q)")
proof("conditions", TRI + ["e = Line(A, B)", "P = Point(e)", "f = Line(P, C)",
                           "g = PerpendicularLine(C, e)"], "AreParallel(f, g)")
proof("conditions", TRI + ["a = Line(B, C)", "f = PerpendicularLine(A, a)", "H = Intersect(f, a)",
                           "M = Midpoint(B, C)"], "AreEqual(H, M)")
proof("conditions", TRI + ["c = Circle(A, B, C)", "D = Midpoint(A, B)", "E = Midpoint(B, C)",
                           "F = Midpoint(A, C)", "d = Circle(D, E, F)"], "AreConcyclic(D, E, F, A)")
proof("conditions", QUAD + ["E = (4, -2)", "f = Line(A, B)", "g = Line(C, D)",
                            "h = Line(E, f)"], "AreParallel(g, h)")
proof("conditions", QUAD + ["f = Line(A, B)", "g = Line(C, D)", "h = Line(C, f)"],
      "AreEqual(g, h)")
proof("conditions", TRI + ["a = Line(B, C)", "d = Line(A, C)", "f = PerpendicularLine(A, a)",
                           "g = PerpendicularLine(B, d)", "H = Intersect(f, g)", "h = Line(C, H)",
                           "e = Line(A, B)"], "ArePerpendicular(h, e)", [mv("C", 2.5, 3)])
proof("conditions", TRI + ["f = PerpendicularBisector(A, B)", "g = PerpendicularBisector(B, C)",
                           "O = Intersect(f, g)", "M = Midpoint(A, C)", "h = Line(O, M)",
                           "d = Line(A, C)"], "ArePerpendicular(h, d)")
proof("conditions", TRI + ["D = Midpoint(B, C)", "E = Midpoint(A, C)", "f = Line(A, D)",
                           "g = Line(B, E)", "G = Intersect(f, g)", "F = Midpoint(A, B)"],
      "AreCollinear(C, G, F)")
proof("conditions", TRI + ["D = Midpoint(B, C)", "E = Midpoint(A, C)", "f = Line(A, D)",
                           "g = Line(B, E)", "G = Intersect(f, g)", "a = Line(B, C)",
                           "d = Line(A, C)", "k = PerpendicularLine(A, a)",
                           "l = PerpendicularLine(B, d)", "H = Intersect(k, l)",
                           "m = PerpendicularBisector(A, B)", "n = PerpendicularBisector(B, C)",
                           "O = Intersect(m, n)"], "AreCollinear(G, H, O)")
proof("conditions", ["A = (0, 0)", "B = (4, 0)", "e = Line(A, B)", "P = Point(e)",
                     "C = (1, 3)"], "AreCollinear(A, P, C)")

# Points on lines, which keep the second free point free, and two outputs of
# one intersection.
cases.topic("paths")
proof("paths", TRI + ["e = Line(A, B)", "P = Point(e, 0.2)", "M = Midpoint(P, C)",
                      "N = Midpoint(A, C)", "f = Line(M, N)"], "AreParallel(f, e)")
proof("paths", TRI + ["e = Line(A, B)", "P = Point(e, 0.3)", "f = Line(P, C)", "g = Line(B, C)",
                      "k = Line(A, f)", "D = Intersect(k, g)"], "AreCollinear(D, B, C)")
proof("paths", TRI + ["e = Line(A, B)", "P = Point(e, 0.4)", "Q = Point(e, 0.5)",
                      "M = Midpoint(P, Q)"], "AreCollinear(A, M, B)", [mv("B", 0, 0), mv("B", 5, 1)])
proof("paths", TRI + ["e = Line(A, B)", "P = Point(e, 0.6)", "f = PerpendicularLine(P, e)",
                      "g = PerpendicularLine(C, e)"], "AreParallel(f, g)")
proof("paths", TRI + ["D = (4, 5)", "c = Circle(A, B)", "d = Circle(C, D)",
                      "P = Intersect(c, d, 1)", "Q = Intersect(c, d, 2)", "f = Line(P, Q)",
                      "g = Line(A, C)"], "ArePerpendicular(f, g)")
proof("paths", TRI + ["D = (4, 5)", "c = Circle(A, B)", "g = Line(C, D)",
                      "P = Intersect(g, c, 1)", "Q = Intersect(g, c, 2)", "M = Midpoint(P, Q)",
                      "f = Line(A, M)"], "ArePerpendicular(f, g)")
proof("paths", TRI + ["D = (4, 5)", "c = Circle(A, B)", "g = Line(C, D)",
                      "P = Intersect(g, c, 1)"], "AreCollinear(P, C, D)")
proof("paths", TRI + ["c = Circle(A, B)", "g = Line(A, C)", "P = Intersect(g, c, 1)",
                      "Q = Intersect(g, c, 2)", "M = Midpoint(P, Q)"], "AreEqual(M, A)")
proof("paths", TRI + ["c = Circle(A, B)", "g = Line(A, C)", "P = Intersect(g, c, 1)",
                      "M = Midpoint(P, B)", "h = Line(A, M)", "k = Line(P, B)"],
      "ArePerpendicular(h, k)")
proof("paths", TRI + ["e = Line(A, B)", "P = Point(e, 0.2)", "Q = Point(e, 0.3)",
                      "R = Point(e, 0.4)"], "AreCollinear(P, Q, R)")
proof("paths", ["A = (0, 0)", "B = (5, 1)", "f = PerpendicularBisector(A, B)",
                "P = Point(f, 0.3)", "c = Circle(P, A)", "d = Circle(P, B)"], "AreEqual(c, d)")
proof("paths", TRI + ["e = Line(A, B)", "P = Point(e, 0.4)", "f = Line(A, P)"], "AreEqual(f, e)")
proof("paths", TRI + ["e = Line(A, B)", "P = Point(e, 0.4)", "Q = Point(e, 0.7)",
                      "f = Line(P, Q)"], "AreEqual(f, e)")
proof("paths", TRI + ["e = Line(A, B)", "P = Point(e, 0.4)", "f = Line(C, P)",
                      "g = Line(C, A)"], "AreEqual(f, g)")
proof("paths", QUAD + ["E = (6, 3)", "f = Line(A, B)", "g = Line(C, D)", "h = Line(E, f)"],
      "AreParallel(g, h)")

# A statement written as an expression: the reference has the CAS rewrite it.
cases.topic("expression")
proof("expression", TRI + ["D = Midpoint(A, B)", "E = Midpoint(B, A)"], "D == E")
# Two objects compared with `==` are congruent (`AlgoAreCongruent`): lines,
# segments by length, circles by radius. A distance operand is replaced by a
# segment, which also rewrites the Boolean itself; the moves show that.
proof("expression", TRI + ["f = Line(A, B)", "g = Line(B, A)"], "f == g")
proof("expression", TRI + ["f = Line(A, B)", "g = Line(A, C)"], "f == g")
proof("expression", TRI + ["f = PerpendicularBisector(A, B)", "P = Point(f)",
                           "s = Segment(P, A)", "t = Segment(P, B)"], "s == t",
      [mv("A", 1, 1), mv("C", 3, 3)])
proof("expression", TRI + ["s = Segment(A, B)", "t = Segment(A, C)"], "s == t",
      [mv("C", 1, 5)])
proof("expression", TRI + ["f = PerpendicularBisector(A, B)", "P = Point(f)",
                           "d = Distance(P, A)", "r = Distance(P, B)"], "d == r",
      [mv("A", 1, 1), mv("C", 3, 3)])
proof("expression", TRI + ["d = Distance(A, B)", "r = Distance(A, C)"], "d == r",
      [mv("C", 1, 5), mv("C", 2, 4)])
proof("expression", TRI + ["s = Segment(A, B)", "d = Distance(A, B)", "r = Distance(B, A)"],
      "d == r", [mv("C", 1, 5), mv("B", 3, 0)])
proof("expression", TRI + ["M = Midpoint(A, B)", "d = Distance(M, A)", "r = Distance(M, B)"],
      "d == r", [mv("A", 1, 1)])
proof("expression", TRI + ["c = Circle(A, B)", "k = Circle(B, A)"], "c == k")
proof("expression", TRI + ["c = Circle(A, B)", "k = Circle(B, C)"], "c == k")
# Arithmetic over segments: the CAS eliminates the segment variables.
proof("expression", TRI + ["M = Midpoint(A, B)", "s = Segment(A, M)", "t = Segment(A, B)"],
      "2s == t")
proof("expression", TRI + ["M = Midpoint(A, B)", "s = Segment(A, M)", "t = Segment(A, B)"],
      "s == 2t")
proof("expression", ["A = (0, 0)", "B = (5, 1)", "f = Line(A, B)",
                     "g = PerpendicularLine(A, f)", "C = Point(g)", "a = Segment(B, C)",
                     "b1 = Segment(A, C)", "c1 = Segment(A, B)"], "a^2 == b1^2 + c1^2")
proof("expression", TRI + ["a = Segment(B, C)", "b1 = Segment(A, C)", "c1 = Segment(A, B)"],
      "a^2 == b1^2 + c1^2")
proof("expression", TRI + ["s = Segment(A, B)"], "s == 3")
# A distance operand of a formula; a second proof sees the rewritten
# Boolean, and a redefinition makes a new one.
cases.case("expression", TRI + ["M = Midpoint(A, B)", "s = Segment(A, M)",
                                "b = Distance(A, B) == 2s", "p = Prove(b)", "p2 = Prove(b)",
                                "q = ProveDetails(b)"],
           [mv("A", 1, 1), {"action": "redefine", "line": "b = Distance(A, B) == 2s"},
            mv("C", 3, 3)])
# A segment made after the Boolean replaces its distance.
cases.case("expression", TRI + ["D = (-1, 3)", "b = Distance(A, B) == Distance(C, D)",
                                "s = Segment(A, B)", "p = Prove(b)"],
           [mv("A", 1, 1), mv("A", 2, 0), mv("D", 4, 4)])
# Formulas whose terms have mixed signs: the elimination squares each length.
ISO = ["A = (0, 0)", "B = (4, 0)", "f = PerpendicularBisector(A, B)", "P = Point(f)",
       "s = Segment(P, A)", "t = Segment(P, B)"]
proof("expression", ISO, "s + t == 0")
proof("expression", ISO, "s - t == 0")
proof("expression", ISO, "s == -t")
proof("expression", TRI + ["s = Segment(A, B)", "t = Segment(B, C)", "u = Segment(A, C)"],
      "s - t == u")
proof("expression", TRI + ["s = Segment(A, B)", "t = Segment(B, C)", "u = Segment(A, C)"],
      "s + t == u")
# Rays, lines and segments compared with ==.
proof("expression", TRI + ["f = Ray(A, B)", "g = Ray(B, A)"], "f == g")
proof("expression", TRI + ["f = Ray(A, B)", "g = Line(A, B)"], "f == g")
proof("expression", TRI + ["f = Segment(A, B)", "g = Line(A, B)"], "f == g")
proof("expression", TRI + ["M = Midpoint(A, B)", "f = Ray(A, M)", "g = Ray(A, B)"], "f == g")

# Points on a circle that lie on a line through them, a list named like the
# command, and circles of radius zero.
cases.topic("review")
proof("review", ["O = (0, 0)", "A = (4, 0)", "c = Circle(O, A)", "P = Point(c, 0.25)",
                 "g = Line(O, P)", "Q = Intersect(g, c, 2)"], "AreEqual(P, Q)")
proof("review", ["O = (0, 0)", "A = (4, 0)", "c = Circle(O, A)", "P = Point(c, 0.25)",
                 "g = Line(O, P)", "Q = Intersect(g, c, 1)", "M = Midpoint(P, Q)"],
      "AreEqual(M, O)")
proof("review", ["A = (0, 0)", "B = (0, 0)", "c = Circle(A, B)", "d = Circle(A, B)"],
      "AreEqual(c, d)", [mv("B", 1, 0)])
proof("review", ["A = (0, 0)", "B = (2, 1)", "c = Circle(A, B)", "d = Circle(A, B)"],
      "AreEqual(c, d)", [mv("B", 0, 0)])
cases.case("review", ["Prove = {2, 3}", "n = Prove(1)"], [])
proof("review", ["O = (0, 0)", "A = (3, 1)", "B = (1, 4)", "c = Circle(O, A)",
                 "g = Line(A, B)", "P = Intersect(g, c)", "M = Midpoint(P_{1}, P_{2})",
                 "f = Line(O, M)"], "ArePerpendicular(f, g)")
proof("review", ["A = (0, 0)", "B = (4, 0)", "g = Line(A, B)", "P = Point(g, 0.25)",
                 "c = Circle(A, P)", "Q = Intersect(g, c, 2)"], "AreEqual(P, Q)")
proof("review", ["O = (0, 0)", "A = (4, 0)", "c = Circle(O, A)", "P = Point(c, 0.25)",
                 "d = Circle(A, P)", "Q = Intersect(c, d, 2)"], "AreEqual(P, Q)")
proof("review", ["A = (0, 0)", "B = (4, 1)", "CZ = (1, 3)", "Ca = (3, -2)",
                 "f = PerpendicularBisector(A, B)", "g = PerpendicularBisector(B, CZ)",
                 "h = PerpendicularBisector(B, Ca)", "O = Intersect(f, g)", "R = Intersect(f, h)",
                 "M = Midpoint(A, B)"], "AreCollinear(O, R, M)")

cases.write()
