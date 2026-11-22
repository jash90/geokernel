"""Common tangents through the transitions between zero and four lines."""

from caselib import Cases, mv, setn

cases = Cases("commontangents")

# CommandsTest.cmdTangent intersects two coincident conics. Starting with
# separated conics exposes all four line labels before they become undefined.
cases.case("circles", ["A = (0, 0)", "B = (4, 0)", "r = 1", "c = Circle(A, r)",
                       "d = Circle(B, r)", "t = Tangent(c, d)"],
           [mv("B", 3, 0), mv("B", 2, 0), mv("B", 1, 0), mv("B", 0, 0), mv("B", 4, 0)])
cases.case("circles", ["A = (-1, 2)", "B = (7, 3)", "r = 1", "s = 2",
                       "c = Circle(A, r)", "d = Circle(B, s)", "t = Tangent(d, c)"],
           [setn("r", 3), setn("s", 0), setn("s", 1)])
cases.case("circles", ["A = (0, 0)", "B = (1, 0)", "r = 1", "c = Circle(A, r)",
                       "d = Circle(B, r)", "t = Tangent(c, d)"],
           [mv("B", 0.5, 0), mv("B", 4, 0)])

cases.case("conics", ["a = 6", "c = Conic({1, 4, -4, 0, 0, 0})",
                      "d = Conic({1, 4, a^2 - 4, 0, -2a, 0})", "t = Tangent(c, d)"],
           [setn("a", 5), setn("a", 3), setn("a", 0), setn("a", 6)])
cases.case("conics", ["c = Conic({1, 0, 0, 0, 0, -1})",
                      "d = Conic({4, 1, 15, 0, 0, 8})", "t = Tangent(c, d)"], [])
cases.case("conics", ["c = Conic({1, 4, -4, 0, 0, 0})", "A = (4, 3)", "r = 1",
                      "d = Circle(A, r)", "t = Tangent(d, c)"], [mv("A", 5, 3)])

arc = ["O = (0, 0)", "A = (1, 0)", "B = (0, 1)", "k = CircularArc(O, A, B)",
       "D = (4, 0)", "r = 1", "d = Circle(D, r)"]
cases.case("arc", [*arc, "t = Tangent(k, d)"], [])
cases.case("arc", [*arc, "t = Tangent(d, k)"], [])
cases.case("arc", [arc[0], "A = (-0.6, -0.8)", "B = (-1, 1)", *arc[3:],
                  "t = Tangent(k, d)"], [mv("A", 1, 0), mv("B", -1, -1)])
cases.case("arc", [arc[0], "A = (-1, -1)", "B = (-1, 1)", *arc[3:],
                  "t = Tangent(k, d)"], [mv("A", 1, 0), mv("B", -1, -1)])

cases.write()
