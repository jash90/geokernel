"""Live limits of the numeric Slider command."""

from caselib import Cases, setn

cases = Cases("sliderbounds")
cases.case("clamp", [
    "lo = 2", "hi = 6", "s = Slider(lo, hi)", "d = 2s",
], [
    setn("s", 5), setn("hi", 4), setn("lo", 7), setn("hi", 9), setn("lo", 0),
])
cases.write()
