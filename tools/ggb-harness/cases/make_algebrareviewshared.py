"""Reference cases for the script and equation review."""

from caselib import Cases, mv, setn


c = Cases("algebrareviewshared")
c.case("circle", [
    "A = (1,2)", "c = Circle(A,2)", "P = Center(c)", "n = Radius(c)",
], [mv("A", -3, 4)])
c.case("sides", [
    "a = 2", "j: a*x+y = a*x+5*x+1", "f = LeftSide(j)", "g = RightSide(j)",
    "n = f(3,4)", "m = g(3,4)",
], [setn("a", -1)])
c.case("text", ['n = Length("a#b")'], [])
c.write()
