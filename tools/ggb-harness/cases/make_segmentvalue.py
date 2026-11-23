"""Record segments used as values: in lists, conditions, sums, texts and formulas.

A segment is a number in arithmetic (its length); these cases check the other
places where an expression reads a segment.
"""

from caselib import Cases, mv

cases = Cases("segmentvalue")
BASE = ["A = (0, 0)", "B = (3, 4)", "C = (6, 0)", "s = Segment(A, B)", "t = Segment(B, C)"]
MOVES = [mv("A", 1, 0), mv("B", 3, 1), mv("C", 3, 1)]
for line in ("l = {s, t}", "m = If(x(A) < 0.5, s, t)", "u = Text(s)",
             "f(x) = s x", "k = s + 1", "w = 2s", "v = s / t", "e = s == t", "g = s < t",
             "h = {s + 1, 2t}", "r = sqrt(s)", "q = -s"):
    cases.case("contexts", BASE + [line], MOVES)
cases.write()
