"""Record upstream function-analysis tests and numerical edge cases."""

from caselib import Cases, mv, setn


cases = Cases("funcanalysis")


def case(topic, script, steps=(), *, view=None):
    cases.case(topic, script.splitlines(), list(steps))
    if view is not None:
        cases.cases[-1]["view"] = view
    if any(command in script for command in ("Curve(", "Spline(", "ParametricDerivative(")):
        cases.cases[-1]["sample_curves"] = True


# CommandsTest.cmdRoot, cmdRootHighDeg, cmdRoots, and cmdRootList.
for f in (
    "x^3-x",
    "x^3-2x^2+x",
    "x^3-3x^2+3x-1",
    "9x^4-x^2",
    "x^4-4x^2",
    "x^6-2x^5-4x^4+8x^3",
    "x^8-x^4",
    "(x+1)^99",
    "(x+1)^99+1",
    "If(-.1<x<2,x^3-x)",
):
    case("rootupstream", f"R = Root({f})")
case("rootupstream", "R = Root(sin(pi*x),1.3)")
case("rootupstream", "R = Root(sin(pi*x),-3,3)")
case("rootupstream", "R = Root(0)")
case("rootupstream", "R = Root(4/5)")
case("rootsupstream", "R = Roots(sin(x),4,13)")
case("rootsupstream", "R = Roots(sin(x),-1,4)")
case("rootsupstream", "flat(x) = 2.00011sin(x/2)-x\nR = Roots(flat,-0.05,0.05)")
case("rootsupstream", "flat(x) = 2.00011sin(x/2)-x\nR = Roots(flat,-0.005,0.005)")
case("rootlistupstream", "r = RootList({1,2,3,4,5})")

# CommandsTest.cmdExtremum and AlgoExtremumMultiTest.
for args in (
    "sin(x),1,7",
    "x^3-3x",
    "nroot(x^3-3x,3)",
    "If(0<x<2,x^3-3x)",
    "(x+1)^24",
    "(x+1)^98",
    "(x+1)^98+1",
    "cos(x)+abs(cos(x)),1,5",
):
    case("extremumupstream", f"R = Extremum({args})")

# CommandsTest.cmdComplexRoot.
case("complexupstream", "R = ComplexRoot(x^6+7x^3-8)")
case("complexupstream", "R = ComplexRoot(x^2)")

# CommandsTest.cmdCurvature, cmdCurvatureVector, cmdOsculatingCircle.
for name in ("Curvature", "CurvatureVector", "OsculatingCircle"):
    case("curvatureupstream", f"R = {name}((1,1),sin(x))")
    case("curvatureupstream", f"c = Curve(sin(t),cos(t),t,0,3)\nR = {name}((1,1),c)")
    case("curvatureupstream", f"C = (2,3)\nk = Circle(C,2)\nR = {name}((1,1),k)")
case("curvatureupstream", "R = OsculatingCircle((0,0),x^2)")

# CommandsTest.cmdIteration, cmdIterationList and AlgoIterationListTest.testDynamic.
for args in ("x*2,2,5", "x*y,{1,1},6", "x*y,{1,1},0", "x*y,{1,1},-1", "t*2,t,{(2,3)},5"):
    case("iterationupstream", f"R = Iteration({args})")
for args in ("x*2,2,5", "a+b,a,b,{1,1},5", "x*y,{1,1},6"):
    case("iterationupstream", f"r = IterationList({args})")
case(
    "iterationupstream",
    "a = 34\nb = 21\nl = IterationList(Mod(p,q),p,q,{a,b},b)\nr = Iteration(Mod(p,q),p,q,{a,b},3)",
    [setn("b", 13)],
)

# CommandsTest.cmdFunction, cmdCurve, cmdParametricDerivative, cmdNInvert.
case("curveupstream", "f = Function(sin(x),4,13)")
case("curveupstream", "c = Curve(t^2,t^2,t,42,50)")
case("curveupstream", "c = Curve(t^2,sin(t),t,-7,7)\nd = ParametricDerivative(c)")
case("curveupstream", "ni(x) = NInvert(sin(x))")

# CommandsTest.cmdSpline and cmdBezierCurve.
for args in ("{(0,0),(1,1),(3,0)}", "{(0,0),(1,1),(3,0)},3",
             "{(0,0),(1,1),(3,0)},3,sqrt(x^2+y^2)", "{(0,0),(1,1),(1,1),(3,0)},4"):
    case("splineupstream", f"c = Spline({args})")
case("splineupstream", "c = BezierCurve((0,0),(0,1),(1,1),(1,0))")

# CommandsTest.cmdSolveODE and cmdSlopeField.
case("odeupstream", "s = SolveODE(x*y,42,4,50,0.5)")
for args in ("x/y", "-y/x,5", "-y/x,5,0.1", "-y/x,5,0.1,0,0,1,1"):
    case("odeupstream", f"s = SlopeField({args})")

# Degenerate domains, solver branch choices and dependency updates.
for args in (
    "x^2+1,-2,2",
    "x^2,-2,3",
    "x^2,0.2",
    "(x-2)*(x-3),2,4",
    "x-3,-5,3",
    "x-3,3,-5",
    "1/x,-3,2",
    "sqrt(x)-1,-1,4",
    "ln(x),0.2",
    "sin(pi*x),1.5",
    "(x+2)*(x-1)*(x-4),-4,5",
):
    case("rootedge", f"R = Root({args})")
for args in ("x^2,-1,1", "x^2+1,-2,2", "x*(x-1),0,1", "1/x,-2,2", "sqrt(x)-1,-2,3",
             "sin(x),4,-1", "x-1,1,1"):
    case("rootsedge", f"R = Roots({args})")
case("rootsedge", "f(x) = sin(x)\nR = Roots(f)")
case("rootedge", "a = 1\nf(x) = x-a\nR = Root(f)", [setn("a", -2)])
case("rootedge", "f(x) = (x+1)*(x-2)\ng(x) = -x+2\nR = Root(f,g)")
case("rootlistedge", "r = RootList({})")
case("rootlistedge", "r = RootList({(1,2),2,3,?,(4,5),2})")
case("rootlistedge", "r = RootList(2,-2,0,2)")
case("rootlistedge", "r = RootList((3,4),(5,6))")
for args in ("-x^2,-1,1", "x^3-3x,2,-2", "x^2,0,4", "1/x,-2,3", "abs(x),-2,3",
             "If(x>0,x*ln(x)), -2, 2"):
    case("extremumedge", f"R = Extremum({args})")
for f in ("x^3", "x^4", "x^5-5*x^3", "sin(x)"):
    case("turning", f"R = TurningPoint({f})")
for f in ("(x^2-1)/(x-1)", "(x^2-1)/(x^2-1)", "1/(x-1)", "sin(x)/x", "(1-cos(x))/x^2",
          "(x-1)^2/(x-1)", "(x^2+1)/(x-1)", "x^2"):
    case("discontinuity", f"f(x) = {f}\nR = RemovableDiscontinuity(f)")
for f in ("1/x", "(x^2+1)/(x-1)", "x^2", "(x^2-1)/(x-1)", "exp(x)", "atan(x)", "ln(x)"):
    case("asymptote", f"f(x) = {f}\nr = Asymptote(f)")
case("curveedge", "a = -2\nb = 2\nf = Function(x^2,a,b)", [setn("b", -3), setn("b", 4)])
case("curveedge", "f = Function(t^2,t,-3,3)")
case("curveedge", "c = Curve((t,t^2),t,-2,2)")
case("curveedge", "c = Curve(Vector((t,t^2)),t,-2,2)")
case("curveedge", "a = 1\nc = Curve(a*t,t^2,t,0,3)\nd = ParametricDerivative(c)", [setn("a", 2)])
case("curveedge", "A = (0,0)\nB = (0,1)\nC = (1,1)\nD = (1,0)\nc = BezierCurve(A,B,C,D)", [mv("B", 2, 3)])
case("curveedge", "A = (0,0)\nB = (1,1)\nC = (3,0)\nc = Spline({A,B,C})", [mv("B", 2, 3)])
case("curveedge", "A = (0,0)\nB = (1,1)\nC = (3,0)\nD = (4,3)\nc = Spline(A,B,C,D)")
case("curveedge", "A = (0,0)\nB = (1,1)\nC = (3,0)\nD = (4,3)\nc = Spline({A,B,C,D},4)")
case("curveedge", "c = Spline({(0,0),(1,1),(3,0),(0,0)})")
case("curveedge", "P = (1,2)\nc = Curve(P,t,0,1)")
case("curveedge", "v = (1,2)\nc = Curve(v,t,0,1)")
case("inverted", "a = 2\nf(x) = a*x+1\ng = NInvert(f)", [setn("a", -3), setn("a", 0)])
case("inverted", "g = NInvert(x^2)")
case("inverted", "g = NInvert(x^2+x)")
case("odeedge", "s = SolveODE(y,0,1,1,0.25)")
case("odeedge", "s = SolveODE(-100*y,0,1,1,0.01)")
case("odeedge", "s = SolveODE(y,0,1,1,0)")
case("odeedge", "s = SolveODE(y,0,1,-1,0.25)")
case("odeedge", "s = SolveODE(y,x,0,1,1,0.25)")
case("odeedge", "s = SolveODE(0,1,0,0,1,1,1,0.25)")
case("nsode", "f(x) = x\ns = NSolveODE({f},0,{1},1)")
# Large iteration counts are tested through a live input. The upstream integer
# conversion wraps this value before it can allocate a large list.
case("iterationedge", "n = 2\nr = Iteration(x+1,1,n)\nl = IterationList(x+1,1,n)",
     [setn("n", -1), setn("n", 4294967296)])
case("iterationedge", "n = 2.75\nr = Iteration(x+1,1,n)\nl = IterationList(x+1,1,n)")

# Named arguments distinguish the overloads in the command matrix.
for script in (
    "f(x) = x^2-1\nR = Root(f)",
    "f(x) = sin(x)\nR = Root(f,2.9)",
    "f(x) = sin(x)\nR = Root(f,2,4)",
    "f(x) = sin(x)\nR = Roots(f,1,7)",
    "f(x) = x^3-3*x\nR = Extremum(f)",
    "f(x) = sin(x)\nR = Extremum(f,1,7)",
    "f(x) = x^3\nR = TurningPoint(f)",
    "f(x) = x^2+1\nR = ComplexRoot(f)",
    "f(x) = x^2\ng = Function(f,-1,2)",
    "f(x) = x^2\ng = Function(f,t,-1,2)",
    "P = (1,1)\nf(x) = x^2\nk = Curvature(P,f)\nu = CurvatureVector(P,f)\nc = OsculatingCircle(P,f)",
    "P = (1,0)\nq = Curve(cos(t),sin(t),t,0,2)\nk = Curvature(P,q)\nu = CurvatureVector(P,q)\nc = OsculatingCircle(P,q)",
    "P = (2,0)\nk: x^2/4+y^2 = 1\na = Curvature(P,k)\nu = CurvatureVector(P,k)\nc = OsculatingCircle(P,k)",
    "f(x) = 2*x\nr = Iteration(f,2,3)\nl = IterationList(f,2,3)",
    "f(x, y) = x*y\nr = Iteration(f,{1,1},6)\nl = IterationList(f,{1,1},6)",
    "f(x) = x\ns = SlopeField(f)",
    "f(x) = x\ns = SlopeField(f,5)",
    "f(x) = x\ns = SlopeField(f,5,0.1)",
    "f(x) = x\ns = SlopeField(f,5,0.1,0,0,1,1)",
    "f(x) = x\ns = SolveODE(f,0,1,1,0.25)",
    "f(x) = x\ng(x) = x^2\ns = SolveODE(f,g,1,1,1,0.25)",
    "b(x) = 0\nc(x) = 1\nf(x) = 0\ns = SolveODE(b,c,f,0,1,1,1,0.25)",
    "g(x, y) = sqrt(x^2+y^2)\nc = Spline({(0,0),(1,1),(3,0)},3,g)",
):
    case("forms", script)
case("nsode", "f(t, u, v) = v\ng(t, u, v) = -u\ns = NSolveODE({f,g},0,{1,0},0.2)")
case("nsode", "f(t, u) = -1000*u\ns = NSolveODE({f},0,{1},0.05)")

# Review regressions. Use an 800-by-600 view at 100 pixels per world unit.
view = [-4, 4, -3, 3, 100, 100]
case("reviewroots", "R = Roots(sin(60x),0,10)", view=view)
case("reviewroots", "R = Roots(sin(60x))", view=view)
case("reviewroots", "f(x) = If(x<0,x+1,x-1)\nR = Root(f)\nS = Roots(f)")
case("reviewroots", "f(x) = If(x<0,x^2-2*x,-x^2+2*x)\nE = Extremum(f)\nT = TurningPoint(f)")
case("reviewroots", "R = Extremum((x-30)^26)")
case("reviewroots", "R = Roots(x^80+x-1)")
case("reviewroots", "R = Root(0.00000000001*(x+0.00001),0,1)")
case("reviewroots", "R = Root(0.00000001*(x^2+1),0)")
case("reviewroots", "R = Root(x^64+x)")
case("reviewiteration", "n = 2\nR = Iteration(x+1,1,n)", [setn("n", 2000000)])
case("reviewode", "s = SolveODE(y,10000000000000000,1,10000000000000002,0.25)")
case("reviewode", "a = 1\nf(t,u,v) = a*v\ng(t,u,v) = -u\ns = NSolveODE({f,g},0,{1,0},0.1)",
     [setn("a", 2)])

cases.write()
