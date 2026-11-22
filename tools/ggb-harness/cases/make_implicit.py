"""Exercise polynomial equations and the commands that use them."""

from caselib import Cases, mv, pt, setn

cases = Cases("implicit")
cases.case("equations", ["c: x^3 + y^3 = 1"], [])
cases.case(
    "equations",
    ["a = 2.98", "b = 0.03", "c: (x^2 + y^2)^2 - 2*b^2*(x^2 - y^2) = a^4 - b^4"],
    [setn("a", 3.8), setn("b", 3.9)],
)
cases.case(
    "equations",
    ["A = (2, 1)", "c: x^3/x(A) + y^3 = 1"],
    [mv("A", 0, 1), mv("A", -2, 1)],
)
cases.case(
    "equations",
    ["a = 3", "c: x^a + y = 1"],
    [setn("a", 2), setn("a", 1), setn("a", 4)],
)
cases.case(
    "equations",
    ["a = 0", "c: a*x^3 + x^2 + y^2 = 1"],
    [setn("a", 1), setn("a", 0)],
)
points = ["A = (1, 1)", "B = (2, 2)", "C = (3, 3)", "D = (4, 0.25)", "E = (5, 0.2)"]
cases.case("constructors", points + ["pts = {A, B, C, D, E}", "c = ImplicitCurve(pts)"],
           [mv("E", 4, 0.25), mv("E", 5, 0.2)])
cases.case("constructors", points + ["c = ImplicitCurve(A, B, C, D, E)"],
           [mv("D", 4, 4), mv("D", 4, 0.25)])
cases.case("constructors", ["a = 1", "f(x,y) = x^3/a + y^2 - 1", "c = ImplicitCurve(f)"],
           [setn("a", 0), setn("a", 2)])
cases.case("constructors", ["f(x,y) = x*y", "c = ImplicitCurve(f)"], [])
cases.case("constructors", points[:3] + ["pts = {A, B, C}", "c = ImplicitCurve(pts)"], [])
cases.case("constructors", points[:2] + ["pts = {A, B}", "c = ImplicitCurve(pts)"],
           [mv("B", 1, 1), mv("B", 2, 3)])
cases.case("constructors", ["c = ImplicitCurve(x*y)"], [])

cases.case(
    "sides",
    ["a = 2", "c: x^3/a + y^2 = y^3 - a", "left = LeftSide(c)", "right = RightSide(c)"],
    [setn("a", 0), setn("a", -2)],
)
cases.case(
    "sides",
    ["f(x,y) = x^2*y + 1", "c = ImplicitCurve(f)", "left = LeftSide(c)", "right = RightSide(c)"],
    [],
)
cases.case("asymptotes", ["c: x^3 - x*y^2 = 1", "lines = Asymptote(c)"], [])
cases.case(
    "asymptotes",
    ["a = 2", "b = 1", "c: (x-a)*(y-b)^2 = 1/b", "lines = Asymptote(c)"],
    [setn("b", 0), setn("b", -2), setn("a", -3)],
)
cases.case("asymptotes", ["c: x^4 + y^4 = 1", "lines = Asymptote(c)"], [])
cases.case("asymptotes", ["c: (x-2*y-1)*(x-2*y+3)*y = 2", "lines = Asymptote(c)"], [])
cases.case("asymptotes", ["c: x^3 - 3*x^2*y + 2*x*y^2 = y^2 + x^2", "lines = Asymptote(c)"], [])

cases.case(
    "line", ["c: x^4 + y^4 = 2", "A = (-2, -2)", "B = (2, 2)", "g = Line(A, B)",
             "P = Intersect(c, g)", "Q = Intersect(g, c)", "n = 1",
             "R = Intersect(c, g, n)", "S = Intersect(g, c, 2)"],
    [setn("n", 2), setn("n", 3), mv("B", -2, -2), mv("B", 2, 2), setn("n", -1)],
)
cases.case("line", ["c: y^3 - y = 0", "g: x = 0", "P = Intersect(c, g)"], [])
cases.case("line", ["c: x^3 - x = 0", "g: x = 1", "P = Intersect(c, g)"], [])
cases.case(
    "line", ["a = 2", "c: x^3/a + y^3 = 1", "g: y = 0",
             "P = Intersect(c, g)", "Q = Intersect(c, g, 1)"],
    [setn("a", 0), setn("a", -2)],
)
cases.case("line", ["a = -1", "c: x^4 - 5*x^2 + y = 0", "g: y = a", "P = Intersect(c, g)"],
           [setn("a", 1), setn("a", 9), setn("a", 0)])
cases.cases[-1]["track_boundary_growth"] = True
cases.case(
    "polyline",
    ["c: (x + 1)^4 + (y - 3)^4 = 1", "A = (-1, -2)", "B = (-1, 3)", "C = (5, 3)",
     "path = PolyLine(A, B, C)", "P = Intersect(c, path)"],
    [mv("A", -1, 4), mv("A", -1, -2), mv("C", -2, 3)],
)
cases.case(
    "polyline",
    ["c: x^4 + y^4 = 1", "A = (-2, 0)", "B = (-1, 0)", "C = (2, 0)",
     "path = PolyLine(A, B, C)", "P = Intersect(path, c)"],
    [],
)
cases.case(
    "graph",
    ["c: x^4 + y^4 = 2", "a = 1", "f(x) = a*x", "n = 1", "P = Intersect(c, f)",
     "Q = Intersect(f, c)", "R = Intersect(c, f, n)", "S = Intersect(f, c, 2)"],
    [setn("n", 2), setn("a", 0), setn("n", 4), setn("a", -1), setn("n", 1)],
)
cases.case("graph", ["c: x^3 + y = 0", "f(x) = sin(x)", "P = Intersect(c, f)"], [])
cases.case(
    "conic", ["c: x^4 + y^4 = 2", "k: x^2 + y^2 = 2", "n = 1", "P = Intersect(c, k)",
              "R = Intersect(c, k, n)"],
    [setn("n", 2), setn("n", 4), setn("n", 5), setn("n", 1)],
)
cases.case(
    "conic", ["a = 1", "c: x^4 + y^4 = 2", "k: x^2 + 2*y^2 = 3*a",
              "P = Intersect(c, k)"],
    [setn("a", 0.1), setn("a", -1), setn("a", 1)],
)
cases.case("conic", ["c: x^4 + y^4 = 2", "k: x + y^2 = 0", "P = Intersect(c, k)"], [])

cases.case("conic", ["c: x^4 + y^4 = 2", "k: x^2 + y^2 = 2", "Q = Intersect(k, c)"], [])

cases.case(
    "pair", ["a = 2", "c: x^4 + y^4 = 2", "d: (x-a)^4 + y^4 = 2", "P = Intersect(c, d)"],
    [setn("a", 3), setn("a", 0), setn("a", 2)],
)
cases.case("pair", ["c: y^3 - y = 0", "d: x^3 - x + y = 0", "P = Intersect(c, d)"], [])
cases.case("pair", ["c: x^3 + y^3 = 1", "d: x^3 - y^3 = 1", "P = Intersect(c, d)"], [])
cases.case(
    "pair", ["c: (x^2 + y^2 - 1)*(x+3) = 0", "d: (x^2 + y^2 - 1)*(y+3) = 0",
             "P = Intersect(c, d)"],
    [],
)

cases.case(
    "tangents", ["f(x,y) = x^2 + y^2 - 1", "c = ImplicitCurve(f)", "A = (1, 1)",
                 "t = Tangent(A, c)"],
    [mv("A", 0, 0), mv("A", 1, 0), mv("A", 1, 1)],
)
cases.case(
    "tangents", ["f(x,y) = x^2 + y^2 - 1", "c = ImplicitCurve(f)", "A = (1, 1)",
                 "t = Tangent(c, A)"],
    [mv("A", 1, 0), mv("A", 2, 0), mv("A", 1, 1)],
)
cases.case(
    "tangents", ["c: x^4 + y^4 = 1", "A = (0, 0)", "B = (2, 0)", "g = Line(A, B)",
                 "t = Tangent(g, c)"],
    [mv("B", 0, 0), mv("B", 0, 2), mv("B", 2, 0)],
)
cases.case(
    "tangents", ["c: x^4 + y^4 = 1", "A = (1, 0)", "t = Tangent(A, c)"],
    [mv("A", -1, 0), mv("A", 2, 0), mv("A", 1, 0)],
)
cases.cases[-1]["track_boundary_growth"] = True
cases.case("tangents", ["c: x^4 + y^4 = 1", "A = (1, 1)", "t = Tangent(A, c)"], [])

for path in ("Segment", "Ray"):
    cases.case(
        "line", ["c: x^4 + y^4 = 2", "A = (-2, 0)", "B = (2, 0)", f"s = {path}(A, B)",
                 "P = Intersect(c, s)", "Q = Intersect(s, c)",
                 "R = Intersect(c, s, 1)", "S = Intersect(s, c, 2)"],
        [mv("B", 0, 0), mv("A", 0.5, 0), mv("B", 0.5, 0), mv("A", -2, 0), mv("B", 2, 0)],
    )
    cases.cases[-1]["track_boundary_growth"] = True
    cases.case(
        "line", ["c: x^4 + y^4 = 2", "A = (0, 0)", "B = (2, 0)", f"s = {path}(A, B)",
                 "P = Intersect(c, s)", "R = Intersect(c, s, 2)"],
        [mv("A", -2, 0), mv("B", -2, 0), mv("B", 2, 0)],
    )
    cases.cases[-1]["track_boundary_growth"] = True

for path in ("CircularArc", "CircularSector"):
    arc = ["c: x^4 + y^4 = 2", "O = (0, 0)", "A = (sqrt(2), 0)", "B = (0, 2)",
           f"k = {path}(O, A, B)"]
    cases.case("conic", arc + ["P = Intersect(c, k)", "Q = Intersect(k, c)",
                              "R = Intersect(c, k, 2)"],
               [mv("B", 0, -2), mv("A", 0, 0), mv("A", 2, 0), mv("A", 1.414214, 0)])
    cases.cases[-1]["track_boundary_growth"] = True
    cases.case("conic", arc, [{"action": "reject", "line": "R = Intersect(k, c, 2)"}])
    transversal = arc[:2] + ["A = (1.3, 0)"] + arc[3:]
    cases.case("conic", transversal + ["P = Intersect(c, k)", "Q = Intersect(k, c)",
                                      "R = Intersect(c, k, 2)"],
               [mv("B", 0, -2), mv("A", 0, 0), mv("A", 1.3, 0)])
    cases.cases[-1]["track_boundary_growth"] = True

for count in (5, 9, 14):
    labels = "ABCDEFGHIJKLMN"[:count]
    collinear = [pt(label, i, i) for i, label in enumerate(labels[:-1])]
    cases.case("constructors", collinear + [pt(labels[-1], 0, 1),
                                             f"c = ImplicitCurve({', '.join(labels)})"],
               [mv(labels[-1], count - 1, count - 1), mv("B", 0, 0), mv("B", 1, 1)])

for scale in (0.01, 0.001, 0.0001, 0.00001):
    labels = "ABCDEFGHI"
    grid = [(i, j) for i in range(4) for j in range(4 - i)][1:]
    scaled = [pt(label, x * scale, y * scale) for label, (x, y) in zip(labels, grid)]
    cases.case("constructors", scaled + [f"pts = {{{', '.join(labels)}}}", "c = ImplicitCurve(pts)"],
               [])

cases.case("asymptotes", ["a = 0.000000001", "c: a*x^3 + x*y = 1", "lines = Asymptote(c)"],
           [setn("a", 1e-8), setn("a", 2e-8), setn("a", 0)])
cases.case("pair", ["a = 0", "c: a*x^3 + y^3 - y = 0", "d: y^3 + y = 0", "P = Intersect(c, d)"],
           [setn("a", 1), setn("a", 0)])
cases.cases[-1]["track_boundary_growth"] = True
cases.case("pair", ["c: y^3 - y = 0", "d: y^3 + y = 0", "P = Intersect(c, d)"], [])
cases.case("pair", ["a = 0", "b = 0", "c: a*x^3 + x^2 + y^2 = 1", "d: b*x^3 + y = x^2",
                    "P = Intersect(c, d)"], [setn("a", 1), setn("b", 1), setn("a", 0), setn("b", 0)])
cases.cases[-1]["track_boundary_growth"] = True
cases.case("pair", ["c: -5*x^4+x^2+y^2=0", "d: -20*x^3+2*x^2+2*x+2*y^2+4*y=0",
                    "P = Intersect(c, d)"], [])
cases.case("pair", ["a = 0", "c: x^4 + y^4 = 1", "d: x^4 + y^4 + (x^2-0.5)^2 = 1 + a",
                    "P = Intersect(c, d)"], [setn("a", -1e-10), setn("a", 1e-10), setn("a", 0)])
cases.cases[-1]["track_boundary_growth"] = True
cases.case("pair", ["c: x^8 + y^2 = 1", "d: x^9 + y^2 = 1", "P = Intersect(c, d)"], [])
cases.case("pair", ["c: x^8 - y = 0", "d: x - y^9 = 0", "P = Intersect(c, d)"], [])
cases.case("pair", ["a = 1", "b = 1", "c: a*x^3 + x^2 + y^2 = 1", "d: b*x^3 + y = x^2",
                    "P = Intersect(c, d)"], [setn("a", 0), setn("b", 0), setn("a", 1), setn("b", 1)])
cases.cases[-1]["track_boundary_growth"] = True

cases.case("cleanup", ["A = (0, 0)", "a = 2", "k = Circle(A, 2*a)", "B = (2, 2)",
                       "M = Midpoint(A, B)"],
           [{"action": "reject", "line": "P = Intersect((0,1), (2,3))"}])
cases.case("cleanup", ["c: x^4 + y^4 = 1", "g: y = 2", "P = Intersect(c, g)",
                       "A = (0, 0)", "a = 2", "k = Circle(A, 2*a)"], [])
cases.case("tangents", ["c: x^4 + y^4 = 1", "A = (1, 1)", "t = Tangent(c, A)"],
           [mv("A", 0, 0), mv("A", 2, 2), mv("A", 1, 1)])
cases.cases[-1]["track_boundary_growth"] = True
cases.case("tangents", ["a = 0.000000001", "c: a*x^3 + x*y = 1", "g: x + y = 0", "t = Tangent(g, c)"],
           [setn("a", 0), setn("a", 2e-8), setn("a", 1e-9)])

cases.case("graph", ["k = 0.1", "c: y^3 + y = 0", "f(x) = sin(k*x)", "P = Intersect(c, f)"],
           [setn("k", 13), setn("k", 0.1)])
cases.cases[-1]["track_boundary_growth"] = True
zigzag = [pt(f"A_{{{i + 1}}}", (-1) ** i * 2, 0) for i in range(34)]
cases.case("polyline", ["c: x^4 + y^4 = 1"] + zigzag
           + ["s = PolyLine(" + ", ".join(f"A_{{{i + 1}}}" for i in range(len(zigzag))) + ")",
              "P = Intersect(c, s)"], [])

scalene = ["P = (-1, 1)", "Q = (4, 0)", "R = (1, 3)"]
for indices in ((1, 2, 3, 4, 10, 42, 42.9), (29, 30, 31, 129, -129, -400, 400),
                (300, 401, 500, 550, 600, 656, 657)):
    first, *rest = indices
    cases.case("cubics", scalene + [f"n = {first}", "c = Cubic(P, Q, R, n)"],
               [setn("n", n) for n in rest] + [setn("n", first), mv("R", 1.5, 0.5),
                                              mv("R", 1, 3), mv("P", -2, 1)])
cases.case("cubics", ["c = Cubic((1,1), (2,1/2), (3,1/3), 42)"], [])

cases.case(
    "trianglecurve", ["A = (0, 0)", "B = (4, 0)", "C = (0, 2)", "h = 2",
                      "c = TriangleCurve(A, B, C, A^2 + h*B*C = C^2)",
                      "M = Midpoint(A, B)"],
    [mv("B", 0, 4), mv("B", 4, 0), setn("h", 3), mv("C", 1, 3)],
)
cases.case("trianglecurve", scalene + ["h = 1", "c = TriangleCurve(P, Q, R, (A+B)^3/h = C)"],
           [setn("h", 0), setn("h", 2), mv("R", 1.5, 0.5), mv("R", 1, 3)])
cases.case("trianglecurve", ["c = TriangleCurve((1,1), (2,1/2), (3,1/3), A+B=C)"], [])

triangle = ["P = (1, 1)", "Q = (2, 0.5)", "R = (3, 1/3)"]
cases.case("barycentric", triangle + ["n = 42", "c = Cubic(P, Q, R, n)"],
           [setn("n", 0), setn("n", 42), mv("Q", 1, 1), mv("Q", 2, 0.5)])
cases.case("barycentric", triangle + ["c = TriangleCurve(P, Q, R, A+B=C)"],
           [mv("Q", 1, 1), mv("Q", 2, 0.5)])

cases.write()
