"""Numeric algebra and logic, starting with upstream CommandsTest examples."""

from caselib import Cases, mv, setn


c = Cases("algebralogic")


def examples(topic, lines):
    for line in lines:
        c.case(topic, [line], [])


# CommandsTest: polynomial division (testDivision and cmdDivision).
c.case("polynomial", [
    "f(x) = x^4-3x^2+x-4", "g(x) = x+1",
    "q = Div(f,g)", "r = Mod(f,g)", "l = Division(f,g)",
], [])
c.case("polynomial", [
    "a = 2", "f(x) = a*x+2", "g(x) = a*x+1",
    "q = Div(f,g)", "r = Mod(f,g)", "l = Division(f,g)",
], [setn("a", 0), setn("a", -3)])
c.case("polynomial", [
    "f(x) = sin(x)", "g(x) = x+1",
    "q = Div(f,g)", "r = Mod(f,g)", "l = Division(f,g)",
], [])

examples("integer", [
    "n = GCD({1,2,3,4,5})", "n = LCM({1,2,3,4,5})",
    "l = PrimeFactors(42)", "l = Division(3,7)",
    "n = Divisors(37)", "l = DivisorsList(42)", "n = DivisorsSum(37)",
    "b = IsPrime(0)", "n = CommonDenominator(1/2,1/3)",
])
for a, b in [(-7, 3), (-7, -3), (7, -3), (3, 0), (0, 0), (3.8, -1.2),
             (9007199254740992, 3), (9007199254740994, 2),
             (9007199254740994, 3), (1000000000000000, 1000000000000000), ("?", 3)]:
    c.case("integer", [f"{name} = {cmd}({a},{b})" for name, cmd in
                       [("m", "Mod"), ("d", "Div"), ("q", "Division"),
                        ("g", "GCD"), ("l", "LCM")]], [])
for values in ["{}", "{-12,0,18}", "{1000000000000000}", "{2.5,0}",
               "{4,?}", "{9223372036854775808,9}", "{1,2.999999999}"]:
    c.case("integer", [f"l = {values}", "g = GCD(l)", "m = LCM(l)"], [])
c.case("integer", [
    "a = 42", "f = PrimeFactors(a)", "d = Divisors(a)", "l = DivisorsList(a)",
    "s = DivisorsSum(a)", "b = IsPrime(a)", "k = IsInteger(a)",
], [setn("a", n) for n in (1, 0, -5, 2.6, 7, 180, 9007199254740992, 9007199254740994)])
c.case("integer", [
    "a = 1/7", "n = CommonDenominator(a, -2/5)",
    "i = CommonDenominator(?, 2)", "j = CommonDenominator(2, 0)",
    "k = CommonDenominator(sqrt(2), pi)",
], [])

# CommandsTest: extrema, intervals, frequency lists, predicates, and conditionals.
examples("extrema", [
    "P = Min(x,1,2)", "P = Max(x,1,2)", "n = Min(2 < x < 3)",
    "n = Min({3,4,5},{0,1,2})", "n = Max({3,4,5},{2,1,0})",
])
c.case("extrema", [
    "a = 2", "f(x) = a*x^2+3*x-2",
    "P = Min(f,-2,2)", "Q = Max(f,-2,2)", "U = Min(f,1,1)", "V = Max(f,2,1)",
    "m = Min(a < x < 3)", "n = Max(a < x < 3)",
], [setn("a", -1), setn("a", 4)])
for weights in ["{}", "{0,0,0}", "{0,-1,1}", "{1,1}", "{?,0,1}", "{0,1/2,0}"]:
    c.case("extrema", [f"l = {weights}", "m = Min({3,4,5},l)", "n = Max({3,4,5},l)"], [])
examples("logic", [
    "n = CountIf(x>3,{1,2,3,4,5})", "n = CountIf(A>3,A,{1,2,3,4,5})",
    "l = KeepIf(x>3,{1,2,3,4,5})", "l = KeepIf(A>3,A,{1,2,3,4,5})",
    "l = KeepIf(Distance(A,(0,0))>4,A,{(1,1)})",
    "P = If(false,(1,1),(2,1/2))", "P = If(false,(3,1/3))",
    "b = IsInteger(42)", "b = IsInRegion((0,0),Circle((1,1),2))",
    "b = IsInRegion((0,0),Circle((1,1),1))",
    "n = CountIf(x>3,{})", "l = KeepIf(x>3,{})",
])
c.case("logic", [
    "P = (0,0)", "O = (0,0)", "c = Circle(O,2)", "b = IsInRegion(P,c)",
    "Q = If(b,P,(5,5))", "R = If(b,P)", "d = IsDefined(R)",
    "f(x) = If(x>x(P),x^2)", "n = f(1)",
    "l = KeepIf(x>x(P),{0,1,2,?})", "k = CountIf(t>x(P),t,{0,1,2,?})",
], [mv("P", 2, 0), mv("P", 3, 0), mv("P", -1, 0)])
c.case("logic", [
    "b = IsDefined(1/0)", "d = IsDefined(?)", "e = IsDefined({?})",
    "f(x) = x^2", "k = IsDefined(f)", "n = IsInteger(?)",
], [])

# ArithmeticTest: numeric list forms of Dot.
examples("products", [
    "n = Dot((1,2),(3,4))", "n = Dot({1,2},{3,4})",
    "n = Dot({1,2,3,4},{1,1,1,1})", "n = Dot({1,2,3},{1,1,1,1})",
    "n = Cross({1,2},{3,4})", "v = Cross({1,2,3},{4,5,6})",
])
# CommandsTest: radix conversion and the two sides of an equation.
examples("base", ["s = ToBase(1000000,2)", 'n = FromBase("FFA23",16)'])
c.case("base", [
    "a = 3", "b = 16", "s = ToBase(a,b)", "n = FromBase(s,b)",
], [setn("a", x) for x in (0, -42, -2.75, 12.5, 1.99999999, 1e30)] +
   [setn("b", x) for x in (2, 2.7, 36, 37, 1)])
for text, radix in [("ff.8", 16), ("", 2), (".", 2), ("1.1.1", 2), ("-1", 10),
                    ("Z", 35), ("1?1", 16), ("Z", 36), ("AA", 16.9)]:
    c.case("base", [f'n = FromBase("{text}",{radix})'], [])
examples("base", ["s = ToBase(?,16)", "s = ToBase(19,?)"])
c.case("sides", ["j: x^2 = y^2", "f = LeftSide(j)", "g = RightSide(j)"], [])
c.case("sides", ["a = 2", "j: a*x+y = x+1", "f = LeftSide(j)", "g = RightSide(j)"],
       [setn("a", 1), setn("a", -3)])
c.case("sides", ["A = (1,2)", "B = (3,7)", "j = Line(A,B)", "f = LeftSide(j)", "g = RightSide(j)"],
       [mv("B", 1, 7), mv("B", 5, 2), mv("B", 1, 2)])
c.case("sides", ["A = (1,2)", "B = (3,2)", "c = Circle(A,B)", "f = LeftSide(c)", "g = RightSide(c)"],
       [mv("B", 4, 2), mv("A", 0, 0)])
# RelationNumerical reports, independent of the dialog's localization.
c.case("relation", ["a = 2", "b = 2", "message = Relation(a,b)",
                    "summary = Relation({a,b})"],
       [setn("a", 0), setn("a", 2.000000001)])
c.case("relation", ["A = (0,0)", "B = (1,0)", "C = (2,0)", "D = (2,2)",
                    "g = Line(A,B)", "h = Line(C,D)", "m = Relation(g,h)",
                    "n = Relation({A,B,C})", "p = Relation(A,h)"],
       [mv("C", 2, 1), mv("D", 4, 1), mv("C", 0, 0), mv("D", 1, 2)])
c.case("relation", ["A = (0,0)", "B = (1,0)", "C = (1,1)", "D = (0,1)",
                    "m = Relation({A,B,C,D})"], [mv("D", 0, 2)])
c.case("relation", ["a = 1/0", "b = ?", "m = Relation(a,b)"], [])
c.case("extrema", [
    "f(x) = sin(x)", "a = 0", "b = 3", "P = Max(f,a,b)", "Q = Min(f,a,b)",
    "v(x) = -2 < x < b", "m = Min(v)", "n = Max(v)",
], [setn("b", 8), setn("b", -1)])
c.case("logic", [
    "a = 2", "test(x) = x>a", "f(x) = x^2", "g(x) = x+1",
    "h(x) = If(test,f,g)", "j(x) = If(a>0,f,g)", "n = h(2)", "k = j(2)",
    "l = KeepIf(test,{1,2,3,4})", "m = CountIf(test,{1,2,3,4})",
], [setn("a", -1), setn("a", 4)])
c.case("logic", [
    "a = 1", "l = If(a>0,{1,2,3})", "n = CountIf(x>1,l)",
    "m = CountIf(t>1,t,l)", "k = KeepIf(x>1,l)", "j = KeepIf(t>1,t,l)",
], [setn("a", 0), setn("a", 2)])
examples("logic", [
    "l = KeepIf(x<3,{1,2,(4,4)})",
    "l = KeepIf(x(A)<3,A,{(1,1),(4,4),5})",
])
c.case("logic", [
    "A = (0,0)", "B = (2,0)", "C = (0,2)", "P = (1,1)",
    "p = Polygon(A,B,C)", "inside = IsInRegion(P,p)",
], [mv("P", 2, 2), mv("P", 0, 1), mv("P", -1, 0)])
c.case("logic", [
    "A = (0,0)", "P = (1,0)", "d = 1", "c = Circle(A,d)", "b = IsInRegion(P,c)",
], [setn("d", 0), setn("d", -1), mv("P", 0, 0), setn("d", 0)])
c.case("logic", ["P = (1,1)", "c: x^2+y^2=1", "b = IsInRegion(P,c)"],
       [mv("P", 1, 0), mv("P", 0, 0)])
c.case("products", [
    "A = (1,2)", "B = (2,3)", "v = (3,4)", "w = (4,5)", "a = 2", "b = 3",
    "r = Dot(A,B)", "s = Dot(A,v)", "t = Dot(v,B)", "u = Dot(v,w)", "d = Dot(a,b)",
    "n = Dot({1,2,3}, {4,5,6})", "l = Cross({1,2,3}, {4,5,6})",
], [mv("A", -2, 3), mv("w", 0, 1)])
c.case("sides", [
    "a = 2", "j: a*x+y=x+1", "f = LeftSide(j)", "g = RightSide(j)",
    "n = f(3,4)", "m = g(3,4)",
], [setn("a", -1), setn("a", 0)])
c.case("sides", [
    "j = Conic(2,3,-4,1,0,0)", "f = LeftSide(j)", "g = RightSide(j)",
], [])
c.case("logic", [
    "P = (0,0)", "a = 1", "f(x) = x<a", "b = IsInRegion(P,f)",
], [mv("P", 1, 0), setn("a", 2)])
c.case("logic", [
    "P = (0,0)", "a = 1", "f(x) = x<a", "inside = IsInRegion(P,f)",
], [setn("a", -1), setn("a", 2)])
c.case("logic", [
    "O = (0,0)", "A = (2,0)", "B = (0,2)", "P = (1,1)",
    "arc = CircleArc(O,A,B)", "sector = CircleSector(O,A,B)",
    "r = IsInRegion(P,arc)", "s = IsInRegion(P,sector)",
], [mv("P", 0.5, 0.5), mv("P", 1.2, 1.2), mv("P", -0.5, -0.5)])
c.case("relation", [
    "O = (0,0)", "A = (0,0)", "n = 2", "c = Circle(O,n)", "d = Circle(A,n)",
    "P = (-1,2)", "Q = (1,2)", "g = Line(P,Q)",
    "m = Relation(c,d)", "s = Relation(g,c)", "t = Relation(P,c)",
], [mv("A", 4, 0), mv("A", 5, 0), setn("n", 3)])
c.case("relation", [
    "f(x) = x^2-1", "g(x) = (x-1)*(x+1)", "h(x) = x^2+1",
    "m = Relation(f,g)", "n = Relation(f,h)",
], [])
c.case("logic", ["P = (3,0)", "f(x) = x<1", "b = IsInRegion(P,f)"], [])
c.write()
