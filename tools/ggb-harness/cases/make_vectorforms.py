#!/usr/bin/env python3
"""Record 2D vector and matrix forms, starting from CommandsTest and ArithmeticTest."""

from caselib import Cases, mv, setn

cases = Cases("vectorforms")
case = cases.case

# CommandsTest.cmdVector, cmdUnitVector, cmdOrthogonalVector,
# cmdUnitOrthogonalVector, cmdLength, and cmdDimension.
case("position", ["A = (1, 1)", "B = (3, 1/3)", "C = (5, 1/5)",
                  "v = Vector(A)", "w = Vector(B, C)", "q = Length(Vector(w))"],
     [mv("C", 3, 1 / 3), mv("B", 5, 2), mv("C", 1, -4)])
case("position", ["u = (3, 4)", "v = Length(Vector(u))", "w = Direction(u)", "d = Dimension(u)"],
     [mv("u", 0, 0), mv("u", -4, 3)])
case("position", ["v = Vector($vector(1, 2))"], [])
case("point", ["A = (3, 4)", "u = UnitVector(A)", "v = PerpendicularVector(A)",
               "w = UnitPerpendicularVector(A)", "n = Length(A)", "d = Dimension(A)"],
     [mv("A", 0, 0), mv("A", 0, -5), mv("A", -4, 3)])
case("point", ["u = UnitVector((3, 4))", "v = PerpendicularVector((3, 4))",
               "w = UnitPerpendicularVector((3, 4))", "n = Length((3, 4))",
               "d = Dimension((3, 7))"], [])
case("point", ["A = (0, 0)", "B = (5, 0)", "C = (0, 2)", "D = (5, -3)",
               "g = Line(A, B)", "h = Line(C, D)", "P = Intersect(g, h)",
               "w = Vector(P, B)", "q = Length(Vector(w))", "s = Direction(w)",
               "u = UnitVector(P)", "v = OrthogonalVector(P)",
               "t = UnitOrthogonalVector(P)", "k = Length(P)", "d = Dimension(w)",
               "j = Dimension(P)"],
     [mv("D", 5, 2), mv("C", 0, 0), mv("D", 5, 0), mv("D", 5, -3), mv("C", 0, -4)])
case("point", ["O = (0, 0)", "A = (-5, 0)", "B = (5, 0)", "r = 2",
               "c = Circle(O, r)", "g = Line(A, B)", "P = Intersect(g, c, 1)",
               "u = UnitVector(P)", "v = OrthogonalVector(P)",
               "w = UnitOrthogonalVector(P)", "n = Length(P)"],
     [mv("A", -5, 2), mv("B", 5, 2), setn("r", 1), setn("r", 3), mv("B", 5, -2)])
case("point", ["A = (3, 4)", "B = (0, 0)", "l = {A, B}", "n = 1",
               "P = Element(l, n)", "u = UnitVector(P)", "v = OrthogonalVector(P)",
               "w = UnitOrthogonalVector(P)", "k = Length(P)", "d = Dimension(P)"],
     [setn("n", 2), setn("n", 3), mv("B", 0, -3), setn("n", 2), setn("n", 1)])

# CommandsTest.cmdDirection and the line variants of the unit/orthogonal tests.
case("line", ["g: -2x + 3y + 1 = 0", "h: 3x = 4y", "v = Direction(g)",
              "u = UnitVector(h)", "w = PerpendicularVector(h)",
              "k = UnitPerpendicularVector(h)"], [])

# CommandsTest.cmdCross and cmdDot, and ArithmeticTest.dotProduct and
# crossProductMissingBracketsTest. Preserve the expressions in the latter.
case("products", ["A = (0, 1)", "B = (2, 0)", "u = (0, 1)", "v = (2, 0)",
                  "s = Cross(A, B)", "t = Cross(A, v)", "p = Cross(u, B)",
                  "q = Cross(u, v)", "r = Dot(A, B)", "k = Dot(u, v)"],
     [mv("B", 0, 2), mv("u", 0, 0), mv("v", 0, -2), mv("A", 3, 0), mv("u", 4, 3)])
case("products", ["A = (1, 2)", "B = (2, 3)", "C = (6, 3)", "D = (0, 4)",
                  "E = Cross(A - B, C - D)", "F = Cross(A, B)^2"],
     [mv("B", 1, 2), mv("D", 6, 3), mv("B", 3, -2), mv("D", 1, 4)])
case("products", ["p = Cross((0, 1), (2, 0))", "q = Cross((0, 1), (0, 2))",
                  "r = Dot((0, 3), (0, 2))", "s = Dot((1, 2), (3, 4))",
                  "t = Dot({1, 2}, {3, 4})", "u = Dot({1, 2, 3, 4}, {1, 1, 1, 1})",
                  "v = Dot({1, 2, 3}, {1, 1, 1, 1})"], [])
case("products", ["n = 2", "l = Sequence(n)", "m = {3, 4}", "a = Dot(l, m)"],
     [setn("n", 3), setn("n", 1), setn("n", 2)])
case("products", ["A = (1, 2)", "B = (2, 3)", "l = {A}", "m = {B}",
                  "a = Dot(l, m)", "b = Dot({A}, {B})", "c = Dot(l, m) + 1"],
     [mv("A", 0, 0), mv("A", 2, -3)])
case("arithmetic", ["A = (1, 2)", "B = (2, 3)", "u = $vector(1, 2)", "n = 2",
                    "v = B - A", "w = u + v", "p = u - v", "q = n * v",
                    "r = u / n", "s = -v", "t = u * v", "j = u^2"],
     [mv("B", 1, 2), setn("n", 0), mv("u", -3, 4), setn("n", -2), mv("A", -3, 2)])

# CommandsTest.cmdDeterminant and cmdMatrixRank.
case("matrixnumbers", ["n = 4", "l = {{1, 2}, {3, n}}", "p = Determinant(l)",
                       "q = MatrixRank(l)"], [setn("n", 6), setn("n", 1), setn("n", 0)])
case("matrixnumbers", ["p = Determinant({{1, 2}, {3, 4}})", "q = MatrixRank({{1}})"], [])
case("matrixnumbers", ["n = 2", "l = {Sequence(n), {2, 4}}", "p = Determinant(l)",
                       "q = MatrixRank(l)"], [setn("n", 1), setn("n", 3), setn("n", 2)])
case("matrixnumbers", ["p = Determinant({{1, 2, 3}, {4, 5, 6}})",
                       "q = MatrixRank({{0, 0, 0}, {0, 0, 0}})"], [])

# CommandsTest.cmdIdentity, cmdTranspose, cmdReducedRowEchelonForm and cmdInvert.
case("identity", ["l = Identity(3)", "n = 3", "m = Identity(n)"],
     [setn("n", 1.9), setn("n", 2.999999), setn("n", 1), setn("n", 0), setn("n", -1)])
case("identity", ["n = 3", "m = Identity(n)"], [setn("n", 0), setn("n", 2)])
for command in ("Transpose", "ReducedRowEchelonForm", "Invert", "Dimension"):
    case("matrixlists", ["n = 4", "l = {{1, 2}, {3, n}}", f"m = {command}(l)"],
         [setn("n", 6), setn("n", 1), setn("n", 4)])
for command in ("Transpose", "ReducedRowEchelonForm"):
    case("matrixlists", ["n = 1", f"m = {command}(n, 2, 3)"], [setn("n", 0)])
    case("matrixlists", ["A = (3, 4)", "B = (1, 2)", f"m = {command}(A, B)"], [mv("A", 0, 0)])
case("matrixlists", ["f(x) = sin(x)", "g = Invert(f)"], [])

# CommandsTest.cmdApplyMatrix uses this matrix and the first point. Vary its
# determinant, matrix shape, and (for a projective image) the denominator.
case("images", ["A = (1, 1)", "u = (1, 1)", "B = (2, 1/2)", "g = Line(A, B)",
                "n = 4", "l = {{1, 2}, {3, n}}", "P = ApplyMatrix(l, A)",
                "v = ApplyMatrix(l, u)", "h = ApplyMatrix(l, g)"],
     [setn("n", 6), mv("u", 0, 0), mv("A", 2, 0.5), mv("B", -2, 1), setn("n", 4)])
case("images", ["A = (1, 1)", "u = (1, 1)", "B = (2, 1/2)", "g = Line(A, B)",
                "n = 1", "l = {{1, 2, 3}, {3, 4, -1}, {1, 2, n}}", "P = ApplyMatrix(l, A)",
                "v = ApplyMatrix(l, u)", "h = ApplyMatrix(l, g)"],
     [setn("n", -3), mv("A", 0, 0), mv("u", 2, 1), setn("n", 1)])
case("images", ["A = (1, 1)", "n = 2", "l = {Sequence(n), {3, 4}}",
                "P = ApplyMatrix(l, A)"], [setn("n", 3), setn("n", 1), setn("n", 2)])
case("images", ["P = ApplyMatrix({{1, 2}, {3, 4}}, (1, 1))"], [])

cases.write()
