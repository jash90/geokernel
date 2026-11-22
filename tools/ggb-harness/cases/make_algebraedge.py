"""Rounding, interval extrema, and boolean filter regressions."""

from caselib import Cases, setn


c = Cases("algebraedge")

for n in (8516626983938329, 3978655322955889):
    # The residue detects a one-unit error even at this scale.
    c.case("integer", [f"n = DivisorsSum({n})", "r = Mod(n,16)"], [])

for command, sign, topic in (("Min", "", "minimum"), ("Max", "-", "maximum")):
    for expression, a, b in [
        ("sqrt(x-0.5)", "0", "2"),
        ("ln(x)", "-1", "3"),
        ("floor(x)", "0.5", "3.5"),
        ("((x+abs(x))/2)", "-3", "2"),
        ("(3+0x)", "0", "1"),
        ("x", "0", "10^100"),
    ]:
        script = [f"f(x) = {sign}{expression}", f"P = {command}(f,{a},{b})"]
        if command == "Min" and expression in ("sqrt(x-0.5)", "ln(x)", "floor(x)", "(3+0x)"):
            script.append(f"Q = Min({expression},{a},{b})")
        c.case(topic, script, [])

c.case("filter", [
    "a = 0", "b = {true,a,false}", "l = KeepIf(x==1,b)", "n = CountIf(x==1,b)",
], [setn("a", 1), setn("a", 2)])
c.case("filter", [
    "b = {0,true,false,1}", "l = KeepIf(x==true,b)", "n = CountIf(x==true,b)",
], [])
c.case("filter", [
    "b = {true,false,true}", "l = KeepIf(x==true,b)", "n = CountIf(x==true,b)",
    "m = KeepIf(t && true,t,b)", "k = CountIf(t && true,t,b)",
], [])
c.case("base", ["s = ToBase(10.01,16)", "t = ToBase(10.3,2)"], [])

c.write()
