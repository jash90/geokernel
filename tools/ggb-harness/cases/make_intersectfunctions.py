"""Function intersections from CommandsTest and dynamic selector regressions."""

from caselib import Cases, mv, setn

cases = Cases("intersectfunctions")


def case(topic, objects, args, steps=()):
    cases.case(topic, [*objects, f"R = Intersect({args})"], list(steps))


# CommandsTest.cmdIntersect and cmdIntersectPoly are also recorded in intersections/.
diagonal = ["A = (0, 0)", "B = (1, 1)", "g = Line(A, B)"]
case("line", diagonal + ["f(x) = sin(x)"], "f, g")
case("line", ["f(x) = (x * ln(x + 1)) / (exp(2x) - 1)"], "xAxis, f")
case("pair", ["f(x) = x^2 + 1", "h(x) = x^3 - x + 2"], "h, f")
case("pair", ["f(x) = x^2 + x", "h(x) = x^2 + x"], "h, f")
case("pair", ["f(x) = x^2 + x", "h(x) = x^2 + x + 1"], "h, f")

for reverse in (False, True):
    for suffix in ("", ", n + 1", ", S"):
        objects = [
            "a = 4", "f(x) = x^2 - a", "G = (0, 0)", "H = (1, 0)",
            "g = Line(G, H)", "n = 0", "S = (-2, 8)",
        ]
        steps = ([setn("n", 1), setn("n", 8), setn("n", -3)] if "n" in suffix else
                 [mv("S", 2, -4), mv("S", -2, 8)] if "S" in suffix else [])
        steps += [setn("a", 0), setn("a", -1), setn("a", 9)]
        case("line", objects, ("g, f" if reverse else "f, g") + suffix, steps)

case("line", ["S = (1, 10)", "a = 1", "g: x = a", "f(x) = sqrt(x)"],
     "f, g, S", [setn("a", -1), setn("a", 4)])
case("line", ["A = (0, 0)", "B = (3, 0)", "g = Segment(A, B)", "f(x) = x^2 - 4"],
     "f, g", [mv("A", -3, 0), mv("B", 1, 0), mv("A", -1, 0)])
case("line", ["A = (0, 0)", "B = (3, 0)", "g = Ray(A, B)", "f(x) = x^2 - 4"],
     "f, g", [mv("A", 3, 0), mv("B", 0, 0), mv("A", -3, 0)])
case("line", ["f(x) = (x^2 - 1) / (x - 1)"], "f, xAxis")

case("pair", ["a = 4", "f(x) = x^2", "h(x) = a", "n = 1"],
     "f, h, n", [setn("n", 2), setn("n", 3), setn("n", 0), setn("a", 0), setn("a", -1)])
case("pair", ["a = 4", "f(x) = x^2", "h(x) = a", "S = (-2, 9)"],
     "f, h, S", [mv("S", 2, -9), mv("S", -2, 9), setn("a", 0), setn("a", -1)])
case("pair", ["f(x) = sin(x)", "h(x) = 0", "S = (3, 0)"],
     "f, h, S", [mv("S", 6, 0), mv("S", -3, 0), mv("S", 0, 0)])

for args in ("f, k", "k, f", "f, k, n", "k, f, n"):
    case("conic", ["a = -3", "f(x) = x^2 + a", "k = Conic({1, 1, -4, 0, 0, 0})", "n = 1"],
         args, ([setn("n", i) for i in (4, 5, 0, 2)] if args.endswith(", n") else [])
         + [setn("a", -2), setn("a", 3), setn("a", -3)])
case("conic", ["a = 0", "f(x) = a", "O = (0, 0)", "r = 2", "k = Circle(O, r)"],
     "k, f", [setn("a", 2), setn("a", 3), setn("a", -1)])
case("conic", ["f(x) = x^3 - x", "k = Conic({1, 1, -4, 0, 0, 0})"], "f, k")

case("interval", ["a = -4", "b = 4", "f(x) = sin(x)", "h(x) = 0"],
     "f, h, a, b", [setn("a", -1), setn("b", 2), setn("a", 4), setn("b", 7)])
case("interval", ["f(x) = cos(x)", "h(x) = 0"], "h, f, 0, 7")
case("interval", ["f(x) = x^2", "h(x) = 0"], "f, h, -1, 1")
case("interval", ["f(x) = 1/x", "h(x) = 0"], "f, h, -1, 1")

cases.write()
