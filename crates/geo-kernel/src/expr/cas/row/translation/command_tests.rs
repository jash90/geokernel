//! Round trips recorded by the Linux x86_64 GeoCasCell harness.
use super::*;

#[test]
fn assume() {
    let c = Construction::new();
    // cas-assume-001
    let tree = parse("Assume(x>2, Simplify(sqrt(x-2sqrt(x-1))))", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[assume(x > 2)],[[[ggbsimparg02:=sqrt((x)-((2)*(sqrt((x)-(1)))))],[ggbsimpans:=0/0],[ggbsimpans:=normal(simplify(regroup(texpand(ggbsimparg02))))],[ggbsimpans2:=factor(exp2pow(lin(pow2exp(ggbsimparg02))))]],when(length(\"\"+ggbsimpans)<length(\"\"+ggbsimpans2)||contains(lname(ggbsimpans2),lname(0/0)[0]),ggbsimpans,ggbsimpans2)][1]][1]"));
    let row = read_back(&tree, "√(x-1)-1").unwrap();
    assert_eq!(row.text, "sqrt(x - 1) - 1");
    assert_eq!(row.latex, "\\mathbf{\\sqrt{x - 1} - 1}");
    // cas-assume-002
    let tree = parse("Assume(1)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn cfactor() {
    let c = Construction::new();
    // cas-cfactor-001
    let tree = parse("CFactor(x^2+1)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[ggbcfactans:=((x)^(2))+(1)],[ggbcfactans:=ggbcfactor(ggbcfactans,x,0,1)],ggbcfactans][2]"));
    let row = read_back(&tree, "(x+ί)*(x-ί)").unwrap();
    assert_eq!(row.text, "(x + ί) (x - ί)");
    assert_eq!(
        row.latex,
        "\\mathbf{\\left(x + i \\right) \\; \\left(x - i \\right)}"
    );
    // cas-cfactor-002
    let tree = parse("CFactor(y^2+1, y)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[ggbcfactans:=((y)^(2))+(1)],[ggbcfactans:=ggbcfactor(ggbcfactans,ggb_is_variable(y),0,1)],ggbcfactans][2]"));
    let row = read_back(&tree, "(y+ί)*(y-ί)").unwrap();
    assert_eq!(row.text, "(y + ί) (y - ί)");
    assert_eq!(
        row.latex,
        "\\mathbf{\\left(y + i \\right) \\; \\left(y - i \\right)}"
    );
    // cas-cfactor-003
    let tree = parse("CFactor(1,2,3)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn cifactor() {
    let c = Construction::new();
    // cas-cifactor-001
    let tree = parse("CIFactor(x^2-2)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[ggbcfactans:=((x)^(2))-(2)],[ggbcfactans:=ggbcfactor(ggbcfactans,x,1,1)],ggbcfactans][2]"));
    let row = read_back(&tree, "(x-√2)*(x+√2)").unwrap();
    assert_eq!(row.text, "(x - sqrt(2)) (x + sqrt(2))");
    assert_eq!(
        row.latex,
        "\\mathbf{\\left(x - \\sqrt{2} \\right) \\; \\left(x + \\sqrt{2} \\right)}"
    );
    // cas-cifactor-002
    let tree = parse("CIFactor(y^2-2, y)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[ggbcfactans:=((y)^(2))-(2)],[ggbcfactans:=ggbcfactor(ggbcfactans,ggb_is_variable(y),1,1)],ggbcfactans][2]"));
    let row = read_back(&tree, "(y-√2)*(y+√2)").unwrap();
    assert_eq!(row.text, "(y - sqrt(2)) (y + sqrt(2))");
    assert_eq!(
        row.latex,
        "\\mathbf{\\left(y - \\sqrt{2} \\right) \\; \\left(y + \\sqrt{2} \\right)}"
    );
    // cas-cifactor-003
    let tree = parse("CIFactor(1,2,3)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn integralsymbolic() {
    let c = Construction::new();
    // cas-integralsymbolic-001
    let tree = parse("IntegralSymbolic(x^2)", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("regroup(integrate((x)^(2)))")
    );
    let row = read_back(&tree, "1/3*x^3+c_1").unwrap();
    assert_eq!(row.text, "1 / 3 x³ + c_{1}");
    assert_eq!(row.latex, "\\mathbf{\\frac{1}{3} \\; x^{3} + c_{1}}");
    // cas-integralsymbolic-002
    let tree = parse("IntegralSymbolic(cos(t), t)", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("regroup(integrate(cos(ggbtmpvart),ggbtmpvart))")
    );
    let row = read_back(&tree, "sin(ggbtmpvart)+c_2").unwrap();
    assert_eq!(row.text, "sin(t) + c_{1}");
    assert_eq!(
        row.latex,
        "\\mathbf{\\operatorname{sin} \\left( t \\right) + c_{1}}"
    );
    // cas-integralsymbolic-003
    let tree = parse("IntegralSymbolic(1,2,3)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn eliminate() {
    let c = Construction::new();
    // cas-eliminate-001
    let tree = parse("Eliminate({x+y=3,x-y=1}, {y})", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("eliminate({(x)+(y)=3, (x)-(y)=1},{y})")
    );
    let row = read_back(&tree, "{2*x-4}").unwrap();
    assert_eq!(row.text, "{2x - 4}");
    assert_eq!(row.latex, "\\mathbf{ \\left\\{ 2 \\; x - 4 \\right\\} }");
    // cas-eliminate-002
    let tree = parse("Eliminate(1)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn groebnerlex() {
    let c = Construction::new();
    // cas-groebnerlex-001
    let tree = parse("GroebnerLex({x+y, y^2-1})", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("gbasis({(x)+(y), ((y)^(2))-(1)},indets({(x)+(y), ((y)^(2))-(1)}),plex)")
    );
    let row = read_back(&tree, "{y^2-1,-x-y}").unwrap();
    assert_eq!(row.text, "{y² - 1, -x - y}");
    assert_eq!(
        row.latex,
        "\\mathbf{ \\left\\{ y^{2} - 1, -x - y \\right\\} }"
    );
    // cas-groebnerlex-002
    let tree = parse("GroebnerLex({x+y, y^2-1}, {x,y})", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("gbasis({(x)+(y), ((y)^(2))-(1)},{x, y},plex)")
    );
    let row = read_back(&tree, "{y^2-1,-x-y}").unwrap();
    assert_eq!(row.text, "{y² - 1, -x - y}");
    assert_eq!(
        row.latex,
        "\\mathbf{ \\left\\{ y^{2} - 1, -x - y \\right\\} }"
    );
    // cas-groebnerlex-003
    let tree = parse("GroebnerLex(1,2,3)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn groebnerdegrevlex() {
    let c = Construction::new();
    // cas-groebnerdegrevlex-001
    let tree = parse("GroebnerDegRevLex({x+y, y^2-1})", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("gbasis({(x)+(y), ((y)^(2))-(1)},indets({(x)+(y), ((y)^(2))-(1)}),revlex)")
    );
    let row = read_back(&tree, "{y^2-1,x+y}").unwrap();
    assert_eq!(row.text, "{y² - 1, x + y}");
    assert_eq!(
        row.latex,
        "\\mathbf{ \\left\\{ y^{2} - 1, x + y \\right\\} }"
    );
    // cas-groebnerdegrevlex-002
    let tree = parse("GroebnerDegRevLex({x+y, y^2-1}, {x,y})", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("gbasis({(x)+(y), ((y)^(2))-(1)},{x, y},revlex)")
    );
    let row = read_back(&tree, "{y^2-1,x+y}").unwrap();
    assert_eq!(row.text, "{y² - 1, x + y}");
    assert_eq!(
        row.latex,
        "\\mathbf{ \\left\\{ y^{2} - 1, x + y \\right\\} }"
    );
    // cas-groebnerdegrevlex-003
    let tree = parse("GroebnerDegRevLex(1,2,3)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn groebnerlexdeg() {
    let c = Construction::new();
    // cas-groebnerlexdeg-001
    let tree = parse("GroebnerLexDeg({x+y, y^2-1})", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("gbasis({(x)+(y), ((y)^(2))-(1)},indets({(x)+(y), ((y)^(2))-(1)}),tdeg)")
    );
    let row = read_back(&tree, "{y^2-1,x+y}").unwrap();
    assert_eq!(row.text, "{y² - 1, x + y}");
    assert_eq!(
        row.latex,
        "\\mathbf{ \\left\\{ y^{2} - 1, x + y \\right\\} }"
    );
    // cas-groebnerlexdeg-002
    let tree = parse("GroebnerLexDeg({x+y, y^2-1}, {x,y})", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("gbasis({(x)+(y), ((y)^(2))-(1)},{x, y},tdeg)")
    );
    let row = read_back(&tree, "{y^2-1,x+y}").unwrap();
    assert_eq!(row.text, "{y² - 1, x + y}");
    assert_eq!(
        row.latex,
        "\\mathbf{ \\left\\{ y^{2} - 1, x + y \\right\\} }"
    );
    // cas-groebnerlexdeg-003
    let tree = parse("GroebnerLexDeg(1,2,3)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn numeric() {
    let c = Construction::new();
    // cas-numeric-001
    let tree = parse("Numeric(745/1137)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[ggbnumans:=0/0],[ggbnumans:=(745)/(1137)],when(dim(lname(ggbnumans))==0||count_eq(unicode0176u,lname(ggbnumans))>0,evalf(ggbnumans,13),normal(evalf(regroup(ggbnumans),13)))][2]"));
    let row = read_back(&tree, "0.6552330694811").unwrap();
    assert_eq!(row.text, "0.66");
    assert_eq!(row.latex, "\\mathbf{0.66}");
    // cas-numeric-002
    let tree = parse("Numeric(2/3,10)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[ggbnumans:=0/0],[ggbnumans:=(2)/(3)],when(dim(lname(ggbnumans))==0||count_eq(unicode0176u,lname(ggbnumans))>0,evalf(ggbnumans,10),normal(evalf(regroup(ggbnumans),10)))][2]"));
    let row = read_back(&tree, "0.6666666667").unwrap();
    assert_eq!(row.text, "0.67");
    assert_eq!(row.latex, "\\mathbf{0.6666666667}");
    // cas-numeric-003
    let tree = parse("Numeric(1,2,3)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn mixednumber() {
    let c = Construction::new();
    // cas-mixednumber-001
    let tree = parse("MixedNumber(11/3)", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("unquote(propfrac((11)/(3)))")
    );
    let row = read_back(&tree, "3+2/3").unwrap();
    assert_eq!(row.text, "3 + 2 / 3");
    assert_eq!(row.latex, "\\mathbf{3 + \\frac{2}{3}}");
    // cas-mixednumber-002
    let tree = parse("MixedNumber(1,2)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn rationalize() {
    let c = Construction::new();
    // cas-rationalize-001
    let tree = parse("Rationalize(1/(2+sqrt(3)))", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[ggbratarg01:=(1)/((2)+(sqrt(3)))],if type(ggbratarg01)==DOM_RAT then ggbratarg01 else normal(exact(ggbratarg01)) fi][1]"));
    let row = read_back(&tree, "-√3+2").unwrap();
    assert_eq!(row.text, "-sqrt(3) + 2");
    assert_eq!(row.latex, "\\mathbf{-\\sqrt{3} + 2}");
    // cas-rationalize-002
    let tree = parse("Rationalize(1,2)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn solvecubic() {
    let c = Construction::new();
    // cas-solvecubic-001
    let tree = parse("SolveCubic(x^3-x)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[j:=exp(2*i*pi/3)],[ggbcub:=((x)^(3))-(x)],[V:=symb2poly(when(ggbcub[0]==equal,left(ggbcub)-right(ggbcub),ggbcub),x)],[n:=size(V)],[V:=V/V[0]],[b:=V[1]],[V:=ptayl(V,-b/3)],[p:=V[2]],[q:=V[3]],[d:=q^2/4+p^3/27],[d:=sqrt(d)],[u:=(-q/2+d)^(1/3)],[v:=-p/3/u],[x1:=u+v-b/3],[x2:=u*j+v*conj(j)-b/3],[x3:=u*conj(j)+v*j-b/3],[x1s:=when(length(lname(x1))==0,regroup(normal(x1)),regroup(x1))],[x2s:=when(length(lname(x2))==0,regroup(normal(x2)),regroup(x2))],[x3s:=when(length(lname(x3))==0,regroup(normal(x3)),regroup(x3))],when(d==0||u==0,csolve(ggbcub,x),[when(real(x1s)==x1s,x1s,x1),x2s,x3s])][19]"));
    let row = read_back(&tree, "{1,-1,0}").unwrap();
    assert_eq!(row.text, "{1, -1, 0}");
    assert_eq!(row.latex, "\\mathbf{ \\left\\{ 1, -1, 0 \\right\\} }");
    // cas-solvecubic-002
    let tree = parse("SolveCubic(x^3-x=0)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[j:=exp(2*i*pi/3)],[ggbcub:=((x)^(3))-(x)=0],[V:=symb2poly(when(ggbcub[0]==equal,left(ggbcub)-right(ggbcub),ggbcub),x)],[n:=size(V)],[V:=V/V[0]],[b:=V[1]],[V:=ptayl(V,-b/3)],[p:=V[2]],[q:=V[3]],[d:=q^2/4+p^3/27],[d:=sqrt(d)],[u:=(-q/2+d)^(1/3)],[v:=-p/3/u],[x1:=u+v-b/3],[x2:=u*j+v*conj(j)-b/3],[x3:=u*conj(j)+v*j-b/3],[x1s:=when(length(lname(x1))==0,regroup(normal(x1)),regroup(x1))],[x2s:=when(length(lname(x2))==0,regroup(normal(x2)),regroup(x2))],[x3s:=when(length(lname(x3))==0,regroup(normal(x3)),regroup(x3))],when(d==0||u==0,csolve(ggbcub,x),[when(real(x1s)==x1s,x1s,x1),x2s,x3s])][19]"));
    let row = read_back(&tree, "{1,-1,0}").unwrap();
    assert_eq!(row.text, "{1, -1, 0}");
    assert_eq!(row.latex, "\\mathbf{ \\left\\{ 1, -1, 0 \\right\\} }");
    // cas-solvecubic-003
    let tree = parse("SolveCubic(1,2)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn solvequartic() {
    let c = Construction::new();
    // cas-solvequartic-001
    let tree = parse("SolveQuartic(x^4-4x^2+4)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
    // cas-solvequartic-002
    let tree = parse("SolveQuartic(1,2)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn jordandiagonalization() {
    let c = Construction::new();
    // cas-jordandiagonalization-001
    let tree = parse("JordanDiagonalization({{1,1},{0,1}})", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("{jordan({{1, 1}, {0, 1}})}")
    );
    let row = read_back(&tree, "{{{1,0},{0,1}},{{1,1},{0,1}}}").unwrap();
    assert_eq!(row.text, "{{{1, 0}, {0, 1}}, {{1, 1}, {0, 1}}}");
    assert_eq!(row.latex, "\\mathbf{ \\left\\{ \\left(\\begin{array}{rr}1&0\\\\0&1\\\\\\end{array}\\right), \\left(\\begin{array}{rr}1&1\\\\0&1\\\\\\end{array}\\right) \\right\\} }");
    // cas-jordandiagonalization-002
    let tree = parse("JordanDiagonalization(1,2)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn eigenvectors() {
    let c = Construction::new();
    // cas-eigenvectors-001
    let tree = parse("Eigenvectors({{2,0},{0,3}})", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("egv({{2, 0}, {0, 3}})")
    );
    let row = read_back(&tree, "{{1,0},{0,1}}").unwrap();
    assert_eq!(row.text, "{{1, 0}, {0, 1}}");
    assert_eq!(
        row.latex,
        "\\mathbf{\\left(\\begin{array}{rr}1&0\\\\0&1\\\\\\end{array}\\right)}"
    );
    // cas-eigenvectors-002
    let tree = parse("Eigenvectors(1,2)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn eigenvalues() {
    let c = Construction::new();
    // cas-eigenvalues-001
    let tree = parse("Eigenvalues({{2,0},{0,3}})", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("{eigenvals({{2, 0}, {0, 3}})}")
    );
    let row = read_back(&tree, "{2,3}").unwrap();
    assert_eq!(row.text, "{2, 3}");
    assert_eq!(row.latex, "\\mathbf{ \\left\\{ 2, 3 \\right\\} }");
    // cas-eigenvalues-002
    let tree = parse("Eigenvalues(1,2)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn laplace() {
    let c = Construction::new();
    // cas-laplace-001
    let tree = parse("Laplace(t)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[ggblaparg01:=ggbtmpvart],when(lname(ggblaparg01)[0]=ggbtmpvart,laplace(ggblaparg01,ggbtmpvart,ggbtmpvars),laplace(ggblaparg01,lname(ggblaparg01)[0]))][1]"));
    let row = read_back(&tree, "1/ggbtmpvars^2").unwrap();
    assert_eq!(row.text, "1 / s²");
    assert_eq!(row.latex, "\\mathbf{\\frac{1}{s^{2}}}");
    // cas-laplace-002
    let tree = parse("Laplace(t, t)", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("laplace(ggbtmpvart,ggbtmpvart)")
    );
    let row = read_back(&tree, "1/ggbtmpvart^2").unwrap();
    assert_eq!(row.text, "1 / t²");
    assert_eq!(row.latex, "\\mathbf{\\frac{1}{t^{2}}}");
    // cas-laplace-003
    let tree = parse("Laplace(t, t, s)", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("laplace(ggbtmpvart,ggbtmpvart,ggbtmpvars)")
    );
    let row = read_back(&tree, "1/ggbtmpvars^2").unwrap();
    assert_eq!(row.text, "1 / s²");
    assert_eq!(row.latex, "\\mathbf{\\frac{1}{s^{2}}}");
    // cas-laplace-004
    let tree = parse("Laplace(1,2,3,4)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn inverselaplace() {
    let c = Construction::new();
    // cas-inverselaplace-001
    let tree = parse("InverseLaplace(1/s^2)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[ggblaparg01:=(1)/((ggbtmpvars)^(2))],when(lname(ggblaparg01)[0]=ggbtmpvars,ilaplace(ggblaparg01,ggbtmpvars,ggbtmpvart),ilaplace(ggblaparg01,lname(ggblaparg01)[0]))][1]"));
    let row = read_back(&tree, "ggbtmpvart").unwrap();
    assert_eq!(row.text, "t");
    assert_eq!(row.latex, "\\mathbf{t}");
    // cas-inverselaplace-002
    let tree = parse("InverseLaplace(1/s^2, s)", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("ilaplace((1)/((ggbtmpvars)^(2)),ggbtmpvars)")
    );
    let row = read_back(&tree, "ggbtmpvars").unwrap();
    assert_eq!(row.text, "s");
    assert_eq!(row.latex, "\\mathbf{s}");
    // cas-inverselaplace-003
    let tree = parse("InverseLaplace(1/s^2, s, t)", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("ilaplace((1)/((ggbtmpvars)^(2)),ggbtmpvars,ggbtmpvart)")
    );
    let row = read_back(&tree, "ggbtmpvart").unwrap();
    assert_eq!(row.text, "t");
    assert_eq!(row.latex, "\\mathbf{t}");
    // cas-inverselaplace-004
    let tree = parse("InverseLaplace(1,2,3,4)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn substitute() {
    let c = Construction::new();
    // cas-substitute-001
    let tree = parse("Substitute(x^2+y^2, x=aaa)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[[ggbevalarg:=0/0],[ggbevalarg:=regroup(subst(((x)^(2))+((y)^(2)),x=ggbtmpvaraaa))],[ggbevalans:=regroup(normal(ggbevalarg))],[ggbevalans2:=regroup(ggbevalarg)]],when(length(\"\"+ggbevalans)<=length(\"\"+ggbevalans2),ggbevalans,ggbevalans2)][1]"));
    let row = read_back(&tree, "ggbtmpvaraaa^2+y^2").unwrap();
    assert_eq!(row.text, "aaa² + y²");
    assert_eq!(row.latex, "\\mathbf{aaa^{2} + y^{2}}");
    // cas-substitute-002
    let tree = parse("Substitute(2x+3y-z, {x=a,y=2,z=d})", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[[ggbevalarg:=0/0],[ggbevalarg:=regroup(subst((((2)*(x))+((3)*(y)))-(z),{x=ggbtmpvara, y=2, z=ggbtmpvard}))],[ggbevalans:=regroup(normal(ggbevalarg))],[ggbevalans2:=regroup(ggbevalarg)]],when(length(\"\"+ggbevalans)<=length(\"\"+ggbevalans2),ggbevalans,ggbevalans2)][1]"));
    let row = read_back(&tree, "2*ggbtmpvara-ggbtmpvard+6").unwrap();
    assert_eq!(row.text, "2a - d + 6");
    assert_eq!(row.latex, "\\mathbf{2 \\; a - d + 6}");
    // cas-substitute-003
    let tree = parse("Substitute((3m-3)^2-(m+3)^2, m, a)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[[ggbevalarg:=0/0],[ggbevalarg:=regroup(subst(((((3)*(ggbtmpvarm))-(3))^(2))-(((ggbtmpvarm)+(3))^(2)),ggbtmpvarm,ggbtmpvara))],[ggbevalans:=regroup(normal(ggbevalarg))],[ggbevalans2:=regroup(ggbevalarg)]],when(length(\"\"+ggbevalans)<=length(\"\"+ggbevalans2),ggbevalans,ggbevalans2)][1]"));
    let row = read_back(&tree, "8*ggbtmpvara^2-24*ggbtmpvara").unwrap();
    assert_eq!(row.text, "8a² - 24a");
    assert_eq!(row.latex, "\\mathbf{8 \\; a^{2} - 24 \\; a}");
    // cas-substitute-004
    let tree = parse("Substitute(x+y+z, x=1, y=2, z=3)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[[ggbevalarg:=0/0],[ggbevalarg:=regroup(subst(((x)+(y))+(z),[((x)+(y))+(z),x=1,y=2,z=3][1..]))],[ggbevalans:=regroup(normal(ggbevalarg))],[ggbevalans2:=regroup(ggbevalarg)]],when(length(\"\"+ggbevalans)<=length(\"\"+ggbevalans2),ggbevalans,ggbevalans2)][1]"));
    let row = read_back(&tree, "6").unwrap();
    assert_eq!(row.text, "6");
    assert_eq!(row.latex, "\\mathbf{6}");
    // cas-substitute-005
    let tree = parse("Substitute(1)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[[ggbevalarg:=0/0],[ggbevalarg:=regroup(subst(1,1[1..]))],[ggbevalans:=regroup(normal(ggbevalarg))],[ggbevalans2:=regroup(ggbevalarg)]],when(length(\"\"+ggbevalans)<=length(\"\"+ggbevalans2),ggbevalans,ggbevalans2)][1]"));
    let row = read_back(&tree, "1").unwrap();
    assert_eq!(row.text, "1");
    assert_eq!(row.latex, "\\mathbf{1}");
}

#[test]
fn toexponential() {
    let c = Construction::new();
    // cas-toexponential-001
    let tree = parse("ToExponential(1+ί)", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("unquote(rectangular2polar((1)+(i)))")
    );
    let row = read_back(&tree, "√2*ℯ^(ί*pi/4)").unwrap();
    assert_eq!(row.text, "sqrt(2) ℯ^(ί π / 4)");
    assert_eq!(
        row.latex,
        "\\mathbf{\\sqrt{2} \\; \\textit{e}^{i \\; \\frac{\\pi }{4}}}"
    );
    // cas-toexponential-002
    let tree = parse("ToExponential(1,2)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn extendedgcd() {
    let c = Construction::new();
    // cas-extendedgcd-001
    let tree = parse("ExtendedGCD(240, 46)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("when((type(240)==DOM_INT)&&(type(46)==DOM_INT), iegcd(240,46), when(is_polynomial(240)&&is_polynomial(46), egcd(240,46),)"));
    let row = read_back(&tree, "{-9,47,2}").unwrap();
    assert_eq!(row.text, "{-9, 47, 2}");
    assert_eq!(row.latex, "\\mathbf{ \\left\\{ -9, 47, 2 \\right\\} }");
    // cas-extendedgcd-002
    let tree = parse("ExtendedGCD(x^2-1, x^2-3x+2)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("when((type(((x)^(2))-(1))==DOM_INT)&&(type((((x)^(2))-((3)*(x)))+(2))==DOM_INT), iegcd(((x)^(2))-(1),(((x)^(2))-((3)*(x)))+(2)), when(is_polynomial(((x)^(2))-(1))&&is_polynomial((((x)^(2))-((3)*(x)))+(2)), egcd(((x)^(2))-(1),(((x)^(2))-((3)*(x)))+(2)),)"));
    let row = read_back(&tree, "{1,-1,3*x-3}").unwrap();
    assert_eq!(row.text, "{1, -1, 3x - 3}");
    assert_eq!(
        row.latex,
        "\\mathbf{ \\left\\{ 1, -1, 3 \\; x - 3 \\right\\} }"
    );
    // cas-extendedgcd-003
    let tree = parse("ExtendedGCD(1)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn modularexponent() {
    let c = Construction::new();
    // cas-modularexponent-001
    let tree = parse("ModularExponent(3,13,7)", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("powmod(3,13,7)"));
    let row = read_back(&tree, "3").unwrap();
    assert_eq!(row.text, "3");
    assert_eq!(row.latex, "\\mathbf{3}");
    // cas-modularexponent-002
    let tree = parse("ModularExponent(1)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn characteristicpolynomial() {
    let c = Construction::new();
    // cas-characteristicpolynomial-001
    let tree = parse("CharacteristicPolynomial({{1,2},{3,4}})", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("pcar({{1, 2}, {3, 4}}, x)")
    );
    let row = read_back(&tree, "x^2-5*x-2").unwrap();
    assert_eq!(row.text, "x² - 5x - 2");
    assert_eq!(row.latex, "\\mathbf{x^{2} - 5 \\; x - 2}");
    // cas-characteristicpolynomial-002
    let tree = parse("CharacteristicPolynomial(1,2)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn minimalpolynomial() {
    let c = Construction::new();
    // cas-minimalpolynomial-001
    let tree = parse("MinimalPolynomial({{2,0},{0,2}})", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("pmin({{2, 0}, {0, 2}}, x)")
    );
    let row = read_back(&tree, "x-2").unwrap();
    assert_eq!(row.text, "x - 2");
    assert_eq!(row.latex, "\\mathbf{x - 2}");
    // cas-minimalpolynomial-002
    let tree = parse("MinimalPolynomial(1,2)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn ludecomposition() {
    let c = Construction::new();
    // cas-ludecomposition-001
    let tree = parse("LUDecomposition({{2,1},{4,3}})", true).unwrap();
    assert_eq!(translate(&c, &tree).as_deref(), Some("[[luarg:={{2, 1}, {4, 3}}], [lu0:=lu(luarg)], [lu1:=lu0[0]],[lu2:=lu0[1]],[lu3:=lu0[2]],[permu2mat(lu1),lu2,lu3]][-1]"));
    let row = read_back(&tree, "{{{1,0},{0,1}},{{1,0},{2,1}},{{2,1},{0,1}}}").unwrap();
    assert_eq!(
        row.text,
        "{{{1, 0}, {0, 1}}, {{1, 0}, {2, 1}}, {{2, 1}, {0, 1}}}"
    );
    assert_eq!(row.latex, "\\mathbf{ \\left\\{ \\left(\\begin{array}{rr}1&0\\\\0&1\\\\\\end{array}\\right), \\left(\\begin{array}{rr}1&0\\\\2&1\\\\\\end{array}\\right), \\left(\\begin{array}{rr}2&1\\\\0&1\\\\\\end{array}\\right) \\right\\} }");
    // cas-ludecomposition-002
    let tree = parse("LUDecomposition(1,2)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}

#[test]
fn qrdecomposition() {
    let c = Construction::new();
    // cas-qrdecomposition-001
    let tree = parse("QRDecomposition({{1,0},{0,2}})", true).unwrap();
    assert_eq!(
        translate(&c, &tree).as_deref(),
        Some("{qr({{1, 0}, {0, 2}})}")
    );
    let row = read_back(&tree, "{{{1,0},{0,1}},{{1,0},{0,2}}}").unwrap();
    assert_eq!(row.text, "{{{1, 0}, {0, 1}}, {{1, 0}, {0, 2}}}");
    assert_eq!(row.latex, "\\mathbf{ \\left\\{ \\left(\\begin{array}{rr}1&0\\\\0&1\\\\\\end{array}\\right), \\left(\\begin{array}{rr}1&0\\\\0&2\\\\\\end{array}\\right) \\right\\} }");
    // cas-qrdecomposition-002
    let tree = parse("QRDecomposition(1,2)", true).unwrap();
    assert!(translate(&c, &tree).is_none());
}
