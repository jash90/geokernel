// Ggb2giac templates (GeoGebra 112919f, EUPL 1.2).

pub(super) fn template(name: &str, arity: usize) -> Option<&'static str> {
    Some(match (name, arity) {
        ("Assume", 2) => r#"[[assume(%0)],%1][1]"#,
        ("CFactor", 1) => {
            r#"[[ggbcfactans:=%0],[ggbcfactans:=ggbcfactor(ggbcfactans,x,0,1)],ggbcfactans][2]"#
        }
        ("CFactor", 2) => {
            r#"[[ggbcfactans:=%0],[ggbcfactans:=ggbcfactor(ggbcfactans,ggb_is_variable(%1),0,1)],ggbcfactans][2]"#
        }
        ("CIFactor", 1) => {
            r#"[[ggbcfactans:=%0],[ggbcfactans:=ggbcfactor(ggbcfactans,x,1,1)],ggbcfactans][2]"#
        }
        ("CIFactor", 2) => {
            r#"[[ggbcfactans:=%0],[ggbcfactans:=ggbcfactor(ggbcfactans,ggb_is_variable(%1),1,1)],ggbcfactans][2]"#
        }
        ("IntegralSymbolic", 1) => r#"regroup(integrate(%0))"#,
        ("IntegralSymbolic", 2) => r#"regroup(integrate(%0,%1))"#,
        ("Eliminate", 2) => r#"eliminate(%0,%1)"#,
        ("GroebnerLex", 1) => r#"gbasis(%0,indets(%0),plex)"#,
        ("GroebnerLex", 2) => r#"gbasis(%0,%1,plex)"#,
        ("GroebnerDegRevLex", 1) => r#"gbasis(%0,indets(%0),revlex)"#,
        ("GroebnerDegRevLex", 2) => r#"gbasis(%0,%1,revlex)"#,
        ("GroebnerLexDeg", 1) => r#"gbasis(%0,indets(%0),tdeg)"#,
        ("GroebnerLexDeg", 2) => r#"gbasis(%0,%1,tdeg)"#,
        ("Numeric", 1) => {
            r#"[[ggbnumans:=0/0],[ggbnumans:=%0],when(dim(lname(ggbnumans))==0||count_eq(unicode0176u,lname(ggbnumans))>0,evalf(ggbnumans,13),normal(evalf(regroup(ggbnumans),13)))][2]"#
        }
        ("Numeric", 2) => {
            r#"[[ggbnumans:=0/0],[ggbnumans:=%0],when(dim(lname(ggbnumans))==0||count_eq(unicode0176u,lname(ggbnumans))>0,evalf(ggbnumans,%1),normal(evalf(regroup(ggbnumans),%1)))][2]"#
        }
        ("MixedNumber", 1) => r#"unquote(propfrac(%0))"#,
        ("Rationalize", 1) => {
            r#"[[ggbratarg0:=%0],if type(ggbratarg0)==DOM_RAT then ggbratarg0 else normal(exact(ggbratarg0)) fi][1]"#
        }
        ("SolveCubic", 1) => {
            r#"[[j:=exp(2*i*pi/3)],[ggbcub:=%0],[V:=symb2poly(when(ggbcub[0]==equal,left(ggbcub)-right(ggbcub),ggbcub),x)],[n:=size(V)],[V:=V/V[0]],[b:=V[1]],[V:=ptayl(V,-b/3)],[p:=V[2]],[q:=V[3]],[d:=q^2/4+p^3/27],[d:=sqrt(d)],[u:=(-q/2+d)^(1/3)],[v:=-p/3/u],[x1:=u+v-b/3],[x2:=u*j+v*conj(j)-b/3],[x3:=u*conj(j)+v*j-b/3],[x1s:=when(length(lname(x1))==0,regroup(normal(x1)),regroup(x1))],[x2s:=when(length(lname(x2))==0,regroup(normal(x2)),regroup(x2))],[x3s:=when(length(lname(x3))==0,regroup(normal(x3)),regroup(x3))],when(d==0||u==0,csolve(ggbcub,x),[when(real(x1s)==x1s,x1s,x1),x2s,x3s])][19]"#
        }
        // SolveQuartic.1 is disabled by PreviewFeature.SOLVE_QUARTIC.
        ("JordanDiagonalization", 1) => r#"{jordan(%0)}"#,
        ("Eigenvectors", 1) => r#"egv(%0)"#,
        ("Eigenvalues", 1) => r#"{eigenvals(%0)}"#,
        ("Laplace", 1) => {
            r#"[[ggblaparg0:=%0],when(lname(ggblaparg0)[0]=ggbtmpvart,laplace(ggblaparg0,ggbtmpvart,ggbtmpvars),laplace(ggblaparg0,lname(ggblaparg0)[0]))][1]"#
        }
        ("Laplace", 2) => r#"laplace(%0,%1)"#,
        ("Laplace", 3) => r#"laplace(%0,%1,%2)"#,
        ("InverseLaplace", 1) => {
            r#"[[ggblaparg0:=%0],when(lname(ggblaparg0)[0]=ggbtmpvars,ilaplace(ggblaparg0,ggbtmpvars,ggbtmpvart),ilaplace(ggblaparg0,lname(ggblaparg0)[0]))][1]"#
        }
        ("InverseLaplace", 2) => r#"ilaplace(%0,%1)"#,
        ("InverseLaplace", 3) => r#"ilaplace(%0,%1,%2)"#,
        ("Substitute", 2) => r#"regroup(subst(%0,%1))"#,
        ("Substitute", 3) => r#"regroup(subst(%0,%1,%2))"#,
        ("Substitute", 1 | 4..) => r#"regroup(subst(%0,%[1..]))"#,
        ("ToExponential", 1) => r#"unquote(rectangular2polar(%0))"#,
        ("ExtendedGCD", 2) => {
            r#"when((type(%0)==DOM_INT)&&(type(%1)==DOM_INT), iegcd(%0,%1), when(is_polynomial(%0)&&is_polynomial(%1), egcd(%0,%1),)"#
        }
        ("ModularExponent", 3) => r#"powmod(%0,%1,%2)"#,
        ("CharacteristicPolynomial", 1) => r#"pcar(%0, x)"#,
        ("MinimalPolynomial", 1) => r#"pmin(%0, x)"#,
        ("LUDecomposition", 1) => {
            r#"[[luarg:=%0], [lu0:=lu(luarg)], [lu1:=lu0[0]],[lu2:=lu0[1]],[lu3:=lu0[2]],[permu2mat(lu1),lu2,lu3]][-1]"#
        }
        ("QRDecomposition", 1) => r#"{qr(%0)}"#,
        _ => return None,
    })
}
