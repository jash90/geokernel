#!/usr/bin/env python3
"""Record descriptive statistics on raw, weighted and grouped data."""

from caselib import Cases, setn

cases = Cases("statistics")
moments = ("Mean", "Variance", "SD", "SampleVariance", "SampleSD", "MAD")
ranks = ("Median", "Q1", "Q3")
commands = moments + ranks


def outputs(names, arguments):
    return [f"r_{{{i}}} = {name}({arguments})" for i, name in enumerate(names, 1)]


cases.case("list", outputs(commands, "{5, 1, 4, 2, 3}"), [])
for values in ("{7, 3}", "{7, 9, 3}", "{7, 9, 3, 5}"):
    cases.case("ranks", outputs(ranks, values), [])
cases.case("frequency", outputs(commands, "{1, 2, 3, 4, 5}, {1, 2, 3, 4, 5}"), [])
cases.case("frequency", outputs(commands, "{9, 1, 3, 1, 7}, {1, 2, 0, 1, 2}"), [])
cases.case("frequency", outputs(commands, "{7, 1, 5}, {2.9, 1.9, 0.9}"), [])
cases.case("frequency", outputs(ranks, "{9, 1, 9}, {20000000, 20000000, 20000000}"), [])
cases.case("classes", outputs(moments + ("Median",), "{20, 40, 41, 42, 40, 54}, {20, 6, 4, 5, 2}"), [])
cases.case("classes", outputs(("Median",), "{0, 10, 30}, {2.5, 5.5}"), [])
cases.case("classes", outputs(("Median", "Q1", "Q3"), "{0, 10, 30}, {2, 4}"), [])
for arguments in ("{}", "{7}", "{7, 7, 7}", "{7}, {2}", "{2, 7}, {0, 0}", "{2, 7}, {1, 0}"):
    cases.case("degenerate", outputs(commands, arguments), [])
for arguments in ("{1, 2, 3}, {1}", "{1, 2}, {1, -1}", "{1, sqrt(-1), 3}", "{1, 2}, {1, sqrt(-1)}"):
    cases.case("undefined", outputs(commands, arguments), [])
cases.case("variadic", outputs(commands, "7, 3, 9, 5"), [])
cases.case("variadic", outputs(("MAD", "mad"), "1, 3"), [])
aliases = ("mean", "stdev", "stdevp", "mad", "Quartile1", "Quartile3")
for arguments in ("{5, 1, 4, 2, 3}", "{1, 2, 3, 4, 5}, {1, 2, 3, 4, 5}"):
    cases.case("aliases", outputs(aliases, arguments), [])
cases.case("edit", ["a = 2", "b = 3", "l = {5, a, 1, a}", "f = {1, b, 0, 2}"]
           + outputs(commands, "l, f"),
           [setn("a", 7), setn("b", 0), setn("b", -1), setn("a", 2), setn("b", 3)])
cases.case("rejected", [], [{"action": "reject", "line": f"r = {name}({arguments})"}
                           for name in commands for arguments in ("1", "{1, 2}, 1")])
cases.write()
