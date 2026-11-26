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
angle_inputs = ["a = 30°", "b = 60°", "s = Segment((0, 0), (3, 0))", "r = 42"]
cases.case("variadictypes", angle_inputs, [
    {"action": "reject", "line": f"r = {name}({arguments})"}
    for name, arguments in [
        ("Median", "a, b"),
        ("Variance", "1, a, 2"),
        ("SD", "30°, 60°"),
        ("SampleVariance", "a + b, 2"),
        ("SampleSD", "a, 2"),
        ("MAD", "a, b"),
        ("Q1", "a, 2"),
        ("Q3", "1, a"),
        ("mean", "a, b"),
        ("Sum", "a, b, a"),
        ("Median", "s, 1"),
    ]
])
cases.case("variadictypes", angle_inputs[:-1] + [
    "m = Median({a, b})",
    "n = Mean({1, a, b})",
    "p = Sum({1, a, b})",
    "q = Median(a / b, 1, 2)",
    "u = Median(a * b, 1, 2)",
    "v = Mean(s + 1, s + 2)",
    "w = Sum(s + 1, s + 2, s + 3)",
], [])

simple = ("Mode", "GeometricMean", "HarmonicMean", "RootMeanSquare")


def simple_outputs(*datasets):
    return [
        f"r_{{{i * len(simple) + j}}} = {name}({data})"
        for i, data in enumerate(datasets)
        for j, name in enumerate(simple, 1)
    ]


cases.case("nextlists", simple_outputs("{1, 2, 3, 4, 5}") + [
    "r_{5} = Mode({5, 2, 5, 2, 3})",
    "r_{6} = Mode({4})",
    "r_{7} = Mode({1, 1, 1.0000000001, 1.0000000001})",
    "r_{8} = Sum(Mode({5, 2, 5, 2, 3}))",
], [])
cases.case("nextedges", ["u = 0 / 0", "e = If(false, {2, 2})"] + simple_outputs(
    "{}", "e", "{1, 1, u, u}", "{u, u}", "{0, 4}", "{-1, 4}", "{-3, 3}", "{0, -1}",
), [])
cases.case("nextvariadic", simple_outputs("1, 4", "1, 4, 4"), [])
cases.case("nextkinds", angle_inputs[:-1] + simple_outputs(
    "{1, true, false}", "{s, s, 1}", "{a, a, b}", '{"1", "1"}',
), [])
cases.case("nextedit", ["a = 3", "l = {a, 2, a, 4}"] + simple_outputs("l"),
           [setn("a", 0), setn("a", -1), setn("a", 2)])
cases.case("nextpoints", ["P = (-1, 8)", "Q = (3, 2)"] + simple_outputs(
    "{P, Q}", "{1, P}", "{P, 1}",
), [])
cases.case("nextreject", angle_inputs + ["g(x) = x^2"], [
    {"action": "reject", "line": f"r = {name}({arguments})"}
    for name in simple
    for arguments in ("1", "1, a", "a, b", "s, 1", "true, 1", "{1, 2}, {1, 2}", "{1, 2}, 1", "g")
])
cases.case("nextreject", [
    "a = Slider(-2, 2)", "Q = (a, a^2 / 2)", "L = Locus(Q, a)", "r = 42",
], [{"action": "reject", "line": f"r = {name}(L, 1)"} for name in simple])
cases.cases[-1]["observe"] = ["r"]
cases.write()
