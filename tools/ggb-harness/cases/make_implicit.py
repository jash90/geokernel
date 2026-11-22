"""Exercise polynomial equations and the commands that use them."""

from caselib import Cases, mv, setn

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
cases.write()
