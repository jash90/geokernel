"""Replay upstream discrete-command tests and construction updates."""

from caselib import Cases, mv, setn


def reject(line):
    return {"action": "reject", "line": line}


c = Cases("discrete")

# CommandsTest at the reference revision. Keep the input order of the tests.
sample = "{(1,1),(2,2),(3,3),(4,1/4),(5,1/5)}"
for command in (
    "ConvexHull",
    "DelaunayTriangulation",
    "MinimumSpanningTree",
    "TravelingSalesman",
    "Voronoi",
):
    c.case("upstream", [f"g = {command}({sample})"], [])
c.case("upstream", [f"g = Hull({sample}, 0.05)"], [])
c.case("upstream", [
    "a = Perimeter(ShortestDistance({1,2,3,4,5}, (3,1/3), (5,1/5), false))",
    "A = (0,0)",
    "b = Perimeter(ShortestDistance({Segment(A,(0,1)), Segment(A,(2,0))}, (2,0), (0,1), false))",
    "d = Perimeter(ShortestDistance({Segment(A,(0,1)), Segment(A,(2,0))}, (2,0), (2,0), false))",
    "f = Perimeter(ShortestDistance({Segment(A,(0,1)), Segment(A,(2,0))}, (2,0), (4,0), false))",
], [])

# DrawablesTest and the fixed ConvexHullTest inputs.
c.case("upstream", ["g = ConvexHull((0,0),(0,1),(1,0))"], [])
for data in (
    "{}", "{(3,1)}", "{(0,0),(0,0)}", "{(2,0),(5,0)}",
    "{(-6,-3),(-8,-3)}", "{(1,-4),(1,4)}", "{(-1,2),(-1,-3)}",
    "{(-2,-3),(2,0)}", "{(-2,3),(2,0)}", "{(-3,2),(1,2),(1,-4),(-3,-4)}",
):
    c.case("upstream", [f"g = ConvexHull({data})"], [])

# The six points on the unit circle from GgbApiTest, expressed as real pairs.
c.case("upstream", [
    "g = DelaunayTriangulation({(cos(1),sin(1)),(cos(2),sin(2)),(cos(3),sin(3)),"
    "(cos(4),sin(4)),(cos(5),sin(5)),(cos(6),sin(6))})",
], [])

# A repeated coordinate, a line, and a cocircular quadrilateral in one edit.
movers = ["A = (0,0)", "B = (2,0)", "C = (2,2)", "D = (0,2)"]
moves = [mv("D", 2, 2), mv("C", 4, 0), mv("D", 1, 0), mv("C", 2, 2), mv("D", 0, 2)]
for command, topic in (
    ("ConvexHull", "hull"),
    ("DelauneyTriangulation", "delaunay"),
    ("Voronoi", "voronoi"),
    ("TravelingSalesman", "tour"),
):
    c.case(topic, movers + [f"g = {command}(A, B, C, D)"], moves)
    for data in ("{}", "{(2,3)}", "{(3,2),(-1,0)}", "{(0,0),(1,1),(2,2)}"):
        c.case(topic, [f"g = {command}({data})"], [])
    c.case(topic, movers + [
        "n = 4", "p = First({A,B,C,D}, n)", f"g = {command}(p)",
    ], [setn("n", 2), setn("n", 1), setn("n", 0), setn("n", 4)])

# Undefined points after a numeric change, followed by recovery.
for command, topic in (
    ("ConvexHull", "hull"), ("DelaunayTriangulation", "delaunay"), ("Voronoi", "voronoi"),
):
    c.case(topic, ["k = 1", "A = (1/k, 1)", f"g = {command}({{A,(0,0),(2,4),(4,0)}})"],
           [setn("k", 0), setn("k", 2)])

# The shared one-list dispatch accepts numbers before the algorithm runs.
c.case("voronoi", ["g = Voronoi(1,2,3)"], [])
c.case("tour", ["h = TravelingSalesman(1,2)"], [reject("g = TravelingSalesman(1,2,3)"),
                    reject("g = TravelingSalesman({(1,1),2,(3,3)})")])
c.case("tree", ["h = MinimumSpanningTree({})"], [reject("g = MinimumSpanningTree(1,2,3)")])
c.case("tree", ["g = MinimumSpanningTree({})", "h = MinimumSpanningTree({(0,0)})"], [])
c.case("tree", movers + ["g = MinimumSpanningTree(A,B,C,D)"], moves)
c.case("tree", ["g = MinimumSpanningTree((0,0),(2,0))"], [])

# A two-edge route is longer than the three-edge route. Track both modes.
c.case("path", [
    "A = (0,0)", "B = (1,5)", "C = (1,0)", "D = (2,0)", "E = (3,0)",
    "a = Segment(B,A)", "b = Segment(B,E)", "c = Segment(C,A)",
    "d = Segment(D,C)", "f = Segment(E,D)",
    "g = ShortestDistance({a,b,c,d,f}, A, E, true)",
    "h = ShortestDistance({a,b,c,d,f}, A, E, false)",
    "j = ShortestDistance({a,b,c,d,f}, E, A, true)",
    "k = ShortestDistance({a,f}, A, E, false)",
    "m = Perimeter(g)",
    "p = Perimeter(k)",
], [mv("B", 1, 0), mv("B", 1, 9)])

# Test the distinction between a repeated point object and coincident sites.
c.case("tree", [
    "A = (0,0)", "B = (2,0)", "n = 3", "p = First({A,A,B}, n)",
    "g = MinimumSpanningTree(p)",
], [setn("n", 2), setn("n", 1), setn("n", 3), mv("B", 0, 0)])
c.case("tree", [
    "k = 1", "A = (0,0)", "B = (1/k,1)", "g = MinimumSpanningTree({A,B})",
], [setn("k", 0), setn("k", 2)])
c.case("path", [
    "A = (0,0)", "B = (1,0)", "C = (1,0)", "D = (2,0)",
    "n = 2", "k = 1", "F = (1/k, 2)",
    "a = Segment(A,B)", "b = Segment(C,D)", "c = Segment(B,C)", "d = Segment(D,F)",
    "p = First({a,b,c,d}, n)",
    "g = ShortestDistance(p, A, D, false)",
    "h = ShortestDistance({a,b,c}, (1,0), D, true)",
    "m = ShortestDistance({a,b,c}, B, D, true)",
    "q = ShortestDistance({d}, D, F, false)",
], [setn("n", 3), mv("C", 1, 1), setn("k", 0), setn("k", 2), setn("n", 0)])
c.case("path", [
    "A = (0,0)", "B = (2,0)", "C = (3,1)", "p = {Segment(B,A),Segment(B,C)}",
    "g = ShortestDistance(p, A, C, true)",
], [mv("B", 1, 2)])

# An input whose 2-opt local minimum is longer than the global optimum.
c.case("tour", [
    "g = TravelingSalesman({(-4,-5),(8,-8),(7,-3),(3,-4),(2,0),(-2,8),(-8,8)})",
    "p = Perimeter(g)",
], [])
c.case("tour", [
    "k = 0", "A = Intersect(xAxis, yAxis)", "B = (1/k, 0)",
    "h = TravelingSalesman({(0,0),(1,0),(0,1)})",
], [reject("g = TravelingSalesman({A,B,(2,2)})")])

# Interior sites exercise traversal around an entire Voronoi cell. Use both
# integer grids and seeded sites in varying insertion orders.
for command, topic in (("DelauneyTriangulation", "delaunay"), ("Voronoi", "voronoi")):
    c.topic(topic)
    for points in (
        [(0,0), (4,0), (4,4), (0,4), (2,2)],
        [(0,0), (4,0), (2,0), (0,4), (4,4), (2,3), (0,2)],
        [(0,0), (4,0), (3,5), (2,0), (2,2), (4,4)],
        *[[c.rp() for _ in range(n)] for n in range(4, 11)],
    ):
        sites = ",".join(f"({x},{y})" for x, y in points)
        c.case(topic, [f"g = {command}({{{sites}}})"], [])

c.case("tree", [
    "A = (0,0)", "B = (2,0)", "g = MinimumSpanningTree({A,A,B})",
], [mv("B", 0, 0), mv("B", 3, 4)])
c.case("tree", [
    "A = (0,0)", "B = (2,0)", "n = 2", "p = First({A,B}, n)", "g = MinimumSpanningTree(p)",
], [setn("n", 1), mv("B", 3, 5), setn("n", 0), setn("n", 1), setn("n", 2)])
c.case("path", [
    "A = (0,0)", "B = (1,1)", "C = (1,-1)", "D = (2,0)",
    "a = Segment(A,B)", "b = Segment(B,D)", "c = Segment(A,C)", "d = Segment(C,D)",
    "p = {a,b,c,d}", "n = 1", "weighted = n > 0",
    "g = ShortestDistance(p, A, D, weighted)", "h = ShortestDistance(p, A, D, true)",
], [setn("n", 0), mv("B", 1, 2), mv("B", 1, 1)])

c.write()
