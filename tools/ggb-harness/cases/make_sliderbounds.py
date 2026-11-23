#!/usr/bin/env python3
"""Records a slider clamping and cascading when its live upper bound changes."""

from caselib import Cases, setn

cases = Cases("sliderbounds")
cases.case(
    "clamp",
    ["m = 10", "a = Slider(0, m)", "b = 2 a"],
    [setn("a", 8), setn("m", 5), setn("m", 10)],
)
cases.write()
