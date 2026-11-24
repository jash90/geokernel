"""Live limits of the numeric Slider command."""

from caselib import Cases, setn

cases = Cases("sliderbounds")
cases.case("clamp", [
    "lo = 2", "hi = 6", "s = Slider(lo, hi)", "d = 2s",
], [
    setn("s", 5), setn("hi", 4), setn("lo", 7), setn("hi", 9), setn("lo", 0),
])
cases.case(
    "widen",
    ["m = 10", "a = Slider(0, m)", "b = 2 a"],
    [setn("a", 8), setn("m", 5), setn("m", 10)],
)
cases.write()
