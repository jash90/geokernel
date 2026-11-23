//! The reference's protocol around each Giac evaluation.
//!
//! Ported from `CASgiacB.init`, `EvalFunction.evalRaw` and the
//! `CASgiac.CustomFunctions` table of the reference application. Before every
//! evaluation Giac is reset (`restart`) and put into the reference's mode, the
//! helper functions the input names are defined again, the time limit is set
//! and the random generator is seeded; then the input is evaluated as
//! `caseval(evalfa(input))`.

use crate::native::Context;
use crate::Error;

/// Puts Giac into the reference's mode; not affected by `restart`.
pub const INIT: &str = "caseval(\"init geogebra\")";

/// Results longer than this are replaced with `?`.
pub const MAX_RESULT_LENGTH: usize = 10_000;

/// The reference's default time limit, in milliseconds.
pub const DEFAULT_TIMEOUT_MS: u64 = 5000;

/// Seed the reference uses for inputs that call `fsolve`, so that NSolve and
/// NSolutions are reproducible.
pub const FSOLVE_SEED: i32 = 9;

#[allow(
    clippy::upper_case_acronyms,
    clippy::enum_variant_names,
    non_camel_case_types
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum F {
    RESTART,
    DISABLE_WARNING,
    PROBA_EPSILON,
    GGBIS_POLYNOMIAL,
    GGB_IS_EQUALS,
    CHECK_DERIVATIVE,
    GGB_IS_GREATER_OR_GREATER_THAN_OR_EQUALS,
    GGB_FACTOR,
    GGB_CFACTOR,
    GGB_IS_VARIABLE,
    GGB_IS_NUMBER,
    GGB_LIST_DIFFERENCE,
    GGBZIPANS,
    GGBMOD,
    GGB_IS_LESS_THAN,
    XCOORD,
    YCOORD,
    ZCOORD,
    DEG_ASIN,
    DEG_ACOS,
    DEG_ATAN,
    DEG_ATAN2,
    COEFFICIENT_CONIC,
    COEFFICIENT_QUADRIC,
    IS_ZERO,
    IS_ALMOST_ZERO,
    PRIM_POLY,
    GGB_IN_STRING,
    INDEX_OF,
    FACTOR_SQR_FREE,
    GEOM_ELIM,
    JACOBI_PREPARE,
    JACOBI_DET,
    GEOM_JACOBI_DET,
    ENVELOPE_EQU,
    LOCUS_EQU,
    COEFF_MATRIX,
    COEFF_MATRICES,
    IMPLICIT_CURVE_COEFFS,
    IRRED,
    AFACTOR_ALG_NUM,
    ABSFACT,
    GGB_ROUND,
    COS_2PI_OVER_N_MINPOLY,
    IS_EQUAL_EQUATIONS,
}

/// `(function, name searched for in the input, definition)`, in the
/// reference's order. A function without a name is defined for every input.
const CUSTOM: &[(F, Option<&str>, &str)] = &[
    (F::RESTART, None, r#"restart"#),
    (F::DISABLE_WARNING, None, r#"warn_equal_in_prog(0)"#),
    (F::PROBA_EPSILON, None, r#"proba_epsilon:=0;"#),
    (
        F::GGBIS_POLYNOMIAL,
        Some("ggbisPolynomial"),
        r#"ggbisPolynomial(a):= when (size(lname(a)) == 1, is_polynomial(a,lname(a)[0]),when (size(lname(a)) == 2, is_polynomial(a,lname(a)[0]) && is_polynomial(a,lname(a)[1]), ?))"#,
    ),
    (
        F::GGB_IS_EQUALS,
        Some("ggb_is_equals"),
        r#"ggb_is_equals(a):=when(a==equal||a=='%=',true,false)"#,
    ),
    (
        F::CHECK_DERIVATIVE,
        Some("check_derivative"),
        r#"check_derivative(a,b):=when(size(a)==1,a[0],flatten1([revlist(a),sort(remove(undef,map(a,r->when(isAlmostZero(evalf(subst(r,x=xcoord(b))-ycoord(b))),r,undef))))])[-1])"#,
    ),
    (
        F::GGB_IS_GREATER_OR_GREATER_THAN_OR_EQUALS,
        Some("ggb_is_gt_or_ge_or_equals"),
        r#"ggb_is_gt_or_ge_or_equals(a):=when(a=='>'||a=='>='||a==equal||a=='%=',true,false)"#,
    ),
    (
        F::GGB_FACTOR,
        Some("ggbfactor"),
        r#"ggbfactor(a, b, c, d):=[with_sqrt(c), factor(a, b), with_sqrt(d)][1]"#,
    ),
    (
        F::GGB_CFACTOR,
        Some("ggbcfactor"),
        r#"ggbcfactor(a, b, c, d):=[with_sqrt(c), cfactor(a, b), with_sqrt(d)][1]"#,
    ),
    (
        F::GGB_IS_VARIABLE,
        Some("ggb_is_variable"),
        r#"ggb_is_variable(a):=when(length(lvar(a))==1,lvar(a)[0],?)"#,
    ),
    (
        F::GGB_IS_NUMBER,
        Some("ggb_is_number"),
        r#"ggb_is_number(a):=[[ggbtype:=type(a)], when(ggbtype==DOM_INT||ggbtype==DOM_FLOAT||ggbtype==DOM_RAT,1,0)][1]"#,
    ),
    (
        F::GGB_LIST_DIFFERENCE,
        Some("ggbListDifference"),
        r#"ggbListDifference(x,y):=flatten1(map(x,e->when(member(e, y)==0,[e],[])))"#,
    ),
    (
        F::GGBZIPANS,
        Some("ggbzipans"),
        r#"ggbzipans(l):=begin local len0,res,sbl,xpr,k,j;xpr:=l[0];len0:=length(l[2]);res:={};for k from 4 to length(l)-1 step +2 do len0:=min(len0,length(l[k])); od;for k from 0 to len0-1 do sbl:={};for j from 2 to length(l)-1 step +2 do sbl:=append(sbl,l[j-1]=l[j][k]);od;res:=append(res,subst(xpr,sbl));od; res; end"#,
    ),
    (
        F::GGBMOD,
        Some("ggbmod"),
        r#"ggbmod(a,b):=when(typeof(a)=='?',?,when(type(a)!=DOM_INT||type(b)!=DOM_INT,rem(a,b,when(length(lname(b))>0,lname(b)[0],x)),irem(a,b)))"#,
    ),
    (
        F::GGB_IS_LESS_THAN,
        Some("ggb_is_less_than"),
        r#"ggb_is_less_than(a):=when(a=='<'||a=='<=',true,false)"#,
    ),
    (
        F::XCOORD,
        Some("xcoord"),
        r#"xcoord(a):=when(type(evalf(a))==DOM_COMPLEX, real(a), when(type(a)==DOM_IDENT,xcoordsymb(a),when(a[0]=='pnt',when(is_3dpoint(a),a[1][0],real(a[1])),when(a[0]==equal,coeff(a[1]-a[2],x,1),a[0]))))"#,
    ),
    (
        F::YCOORD,
        Some("ycoord"),
        r#"ycoord(a):=when(type(evalf(a))==DOM_COMPLEX, im(a), when(type(a)==DOM_IDENT,ycoordsymb(a),when(a[0]=='pnt',when(is_3dpoint(a),a[1][1],im(a[1])),when(a[0]==equal,coeff(a[1]-a[2],y,1),a[1]))))"#,
    ),
    (
        F::ZCOORD,
        Some("zcoord"),
        r#"zcoord(a):=when(type(a)==DOM_IDENT,zcoordsymb(a),when(a[0]=='pnt',when(is_3dpoint(a),a[1][2],0),when(length(a)<3 && a[0] != equal,0,when(a[0]==equal,coeff(a[1]-a[2],z,1),a[2]))))"#,
    ),
    (
        F::DEG_ASIN,
        Some("asind"),
        r#"asind(x):=normal(asin(x)/pi*180)*unicode0176u"#,
    ),
    (
        F::DEG_ACOS,
        Some("acosd"),
        r#"acosd(x):=normal(acos(x)/pi*180)*unicode0176u"#,
    ),
    (
        F::DEG_ATAN,
        Some("atand"),
        r#"atand(x):=normal(atan(x)/pi*180)*unicode0176u"#,
    ),
    (
        F::DEG_ATAN2,
        Some("atan2d"),
        r#"atan2d(y,x):=normal(arg(x+i*y)/pi*180)*unicode0176u"#,
    ),
    (
        F::COEFFICIENT_CONIC,
        Some("ggbcoeffconic"),
        r#"ggbcoeffconic(coeffsarg):={coeffs(coeffsarg,[x,y],[2,0]),coeffs(coeffsarg,[x,y],[0,2]),coeffs(coeffsarg,[x,y],[0,0]),coeffs(coeffsarg,[x,y],[1,1]),coeffs(coeffsarg,[x,y],[1,0]),coeffs(coeffsarg,[x,y],[0,1])}"#,
    ),
    (
        F::COEFFICIENT_QUADRIC,
        Some("ggbcoeffquadric"),
        r#"ggbcoeffquadric(coeffsarg):={coeffs(coeffsarg,[x,y,z],[2,0,0]),coeffs(coeffsarg,[x,y,z],[0,2,0]),coeffs(coeffsarg,[x,y,z],[0,0,2]),coeffs(coeffsarg,[x,y,z],[0,0,0]),coeffs(coeffsarg,[x,y,z],[1,1,0]),coeffs(coeffsarg,[x,y,z],[1,0,1]),coeffs(coeffsarg,[x,y,z],[0,1,1]),coeffs(coeffsarg,[x,y,z],[1,0,0]),coeffs(coeffsarg,[x,y,z],[0,1,0]),coeffs(coeffsarg,[x,y,z],[0,0,1])}"#,
    ),
    (
        F::IS_ZERO,
        Some("ggbIsZero"),
        r#"ggbIsZero(ggbx):=when(ggbx==0 || simplify(texpand(ggbx))==0 || exp2pow(lin(pow2exp(ggbx)))==0,true,when(type(ggbx)=='DOM_LIST',max(flatten({ggbx,0}))==min(flatten({ggbx,0}))&&min(flatten({ggbx,0}))==0,when(ggbx[0]==equal,lhs(ggbx)==0&&rhs(ggbx)==0,ggbx[0]=='pnt' && ggbx[1] == ggbvect[0,0,0])))"#,
    ),
    (
        F::IS_ALMOST_ZERO,
        Some("isAlmostZero"),
        r#"isAlmostZero(a):=len(lname(a))==0 && evalf(a) < 10^-8"#,
    ),
    (
        F::PRIM_POLY,
        Some("primpoly"),
        r#"primpoly(x):=begin local pps,ii; if (x==[0]) return [0]; pps:=[]; for ii from 0 to size(x)-1 do pps[ii]:=primpart(x[ii],lvar(x[ii])); od return pps end"#,
    ),
    (
        F::GGB_IN_STRING,
        Some("ggbinString"),
        r#"ggbinString(x,y):=begin local ret; ret := inString(x,y); if (ret == -1) return undef; else return ret; end"#,
    ),
    (
        F::INDEX_OF,
        Some("indexOf"),
        r#"indexOf(x, mylist):=begin local ii; for ii from 0 to length(mylist)-1 do if (mylist[ii] == x) begin print(ii); return ii; end; od; return undef; end"#,
    ),
    (
        F::FACTOR_SQR_FREE,
        Some("factorsqrfree"),
        r#"factorsqrfree(p):=begin local pf,r,ii; pf:=factor(p); if (sommet(pf)!='*') begin if (sommet(pf)=='^') return op(pf)[0]; else begin if (sommet(pf)!=sommet(-x)) return pf; else return factorsqrfree(-pf); end; end; opPf:=op(pf); r:=1; for ii from 0 to size(opPf)-1 do r:=r*factorsqrfree(opPf[ii]); od return r end"#,
    ),
    (
        F::GEOM_ELIM,
        Some("geomElim"),
        r#"geomElim(polys,elimvars,precision):=begin local ee, ll, ff, gg, ii; ee:=eliminate(polys,revlist(elimvars));  ll:=lvar(ee);  if (size(ee)>1) begin  ff:=round(fsolve(ee,ll)*precision)/precision;  gg:=1; for ii from 0 to size(ff)-1 do gg:=gg*(((ll[0]-ff[ii,0])^2+(ll[1]-ff[ii,1])^2));  od; ee:=[expand(lcm(denom(coeff(gg)))*gg)]; end; if (size(ee)==0) return 0; else return primpoly(ee)[0]; end;"#,
    ),
    (
        F::JACOBI_PREPARE,
        Some("jacobiPrepare"),
        r#"jacobiPrepare(polys,excludevars):=begin local ii, degs, pos, vars, linvar; vars:=lvar(polys); ii:=0; while (ii<size(polys)-1) do degs:=degree(polys[ii],vars); if (sum(degs)=1) begin pos:=find(1,degs); linvar:=vars[pos[0]]; if (!is_element(linvar,excludevars)) begin substval:=op(solve(polys[ii]=0,linvar)[0])[1]; polys:=remove(0,expand(subs(polys,[linvar],[substval])));  ii:=-1; end; end; ii:=ii+1; od; return polys; end"#,
    ),
    (
        F::JACOBI_DET,
        Some("jacobiDet"),
        r#"jacobiDet(polys,excludevars):=begin local J, ii, vars, s, j, k; vars:=lvar(polys); for ii from 0 to size(excludevars)-1 do vars:=remove(excludevars[ii], vars); od; s:=size(vars); J:=matrix(s,s,(j,k)->diff(polys[j],vars[k])); return det_minor(J); end"#,
    ),
    (
        F::GEOM_JACOBI_DET,
        Some("geomJacobiDet"),
        r#"geomJacobiDet(polys,excludevars):=begin local J; J:=jacobiPrepare(polys,excludevars); return jacobiDet(J,excludevars); end"#,
    ),
    (
        F::ENVELOPE_EQU,
        Some("envelopeEqu"),
        r#"envelopeEqu(polys,elimvars,precision,curvevarx,curvevary):=begin local D; D:=geomJacobiDet(polys,[curvevarx,curvevary]); polys:=append(polys,D); return locusEqu(polys,elimvars,precision,curvevarx,curvevary); end"#,
    ),
    (
        F::LOCUS_EQU,
        Some("locusEqu"),
        r#"locusEqu(polys,elimvars,precision,curvevarx,curvevary):=implicitCurveCoeffs(subst(geomElim(jacobiPrepare(polys,[curvevarx,curvevary]),elimvars,precision),[curvevarx=x,curvevary=y]))"#,
    ),
    (
        F::COEFF_MATRIX,
        Some("coeffMatrix"),
        r#"coeffMatrix(aa):=begin local bb, sx, sy, ii, jj, ee, cc, kk; bb:=coeffs(aa,x); sx:=size(bb); sy:=size(coeffs(aa,y)); cc:=[sx,sy]; for ii from sx-1 to 0 by -1 do dd:=coeff(bb[ii],y); sd:=size(dd); for jj from sd-1 to 0 by -1 do ee:=dd[jj]; cc:=append(cc,ee); od; for kk from sd to sy-1 do ee:=0; cc:=append(cc,ee); od; od; return cc; end"#,
    ),
    (
        F::COEFF_MATRICES,
        Some("coeffMatrices"),
        r#"coeffMatrices(aa):=begin local ff, bb, ccf, ll, aaf; ff:=factors(aa); ccf:=[size(ff)/2]; for ll from 0 to size(ff)-1 by 2 do aaf:=ff[ll]; bb:=coeffMatrix(aaf); ccf:=append(ccf,bb); od; return flatten(ccf); end"#,
    ),
    (
        F::IMPLICIT_CURVE_COEFFS,
        Some("implicitCurveCoeffs"),
        r#"implicitCurveCoeffs(aa):=begin local bb; bb:=factorsqrfree(aa); return [coeffMatrix(bb),coeffMatrices(bb)]; end"#,
    ),
    (
        F::IRRED,
        Some("irred"),
        r#"irred(p,x):=begin local f; f:=factors(primpart(p,x)); return (size(f)==2 && f[1]==1); end"#,
    ),
    (
        F::AFACTOR_ALG_NUM,
        Some("afactorAlgNum"),
        r#"afactorAlgNum(p):=begin local k,l,j,d,extdeg,xx,lv,px,lc,lv2,py,fy,lfy,yy,fydeg,deg,pm,pdeg; l:=lname(p); if (!irred(p,l[0])) return "Not irreducible"; if (size(l)<2) return p; d:=[]; for j in l do d:=append(d,degree(p,j)); od; extdeg:=lgcd(d); if (extdeg==1) return "Absolutely irreducible"; xx:=head(l); pdeg:=degree(p,xx); l:=tail(l); for j from 1 to 1000 do lv:=ranv(size(l),j); px:=primpart(subst(p,l,lv),xx); if (degree(px,xx)!=pdeg) continue; if (irred(px,xx)) break; od; lc:=lcoeff(px,xx); if (lc!=1) px:=primpart(subst(px,xx,xx/lc),xx); for j from j to 1000 do lv2:=ranv(size(l),extdeg+j); if (lv2==lv) continue; py:=primpart(subst(p,l,lv2),xx); if (degree(py,xx)!=pdeg || !irred(py,xx)) continue; fy:=factors(py,rootof(px)); fydeg:=map(fy,yy->degree(yy,xx)); deg:=gcd(fydeg); deg:=d[0]/deg; if (deg==extdeg && degree(px)==extdeg) break; extdeg:=gcd(deg,extdeg); if (extdeg==1) return "Absolutely irreducible"; if (deg>extdeg) continue; for k from 0 to size(fydeg)-1 do if (fydeg[k]*extdeg==d[0]) break; od; if (k==size(fydeg)) continue; lfy:=coeff(fy[k],xx); for k from 0 to size(lfy)-1 do pm:=pmin(lfy[k]); if (degree(pm)==extdeg) begin px:=pm; break; end; od; od; return px; end"#,
    ),
    (
        F::ABSFACT,
        Some("absfact"),
        r#"absfact(p):=begin local algnum; algnum:=afactorAlgNum(p);  if (type(algnum)==DOM_LIST || type(algnum)==DOM_SYMBOLIC) return factor(p,rootof(algnum)); else return p; end"#,
    ),
    (
        F::GGB_ROUND,
        Some("ggbround"),
        r#"ggbround(x):=when(evalf(x)==?||evalf(x)=={?},?,when(type(evalf(x))==DOM_LIST,seq(ggbround(x[j]),j,0,length(x)-1),when(type(evalf(x))==DOM_COMPLEX,ggbround(real(x))+i*ggbround(im(x)),when(x<0,when(type(x)==DOM_LIST&&length(x)==2,-round(-x[0], x[1]),-round(-x)),round(x)))))"#,
    ),
    (
        F::COS_2PI_OVER_N_MINPOLY,
        Some("cos2piOverNMinpoly"),
        r#"cos2piOverNMinpoly(n):=begin local j, p, q, r; p:=simplify((tchebyshev1(n)-1)/(x-1)); for j from 1 to n/2 do q:=tchebyshev1(j)-1; r:=gcd(p,q); p:=simplify(p/r); od; return factorsqrfree(primpart(p)); end"#,
    ),
    (
        F::IS_EQUAL_EQUATIONS,
        Some("ggbIsEqualEquations"),
        r#"ggbIsEqualEquations(eq1,eq2):=when(ggbIsZero(eq1)&&ggbIsZero(eq2),true,when(ggbIsZero(eq1)||ggbIsZero(eq2),false,type(normal(equal2diff(eq1)/equal2diff(eq2)))!=DOM_SYMBOLIC))"#,
    ),
];

/// `CustomFunctions.setDependencies`: a function on the left needs the one on
/// the right.
const DEPENDENCIES: &[(F, F)] = &[
    (F::IMPLICIT_CURVE_COEFFS, F::COEFF_MATRIX),
    (F::IMPLICIT_CURVE_COEFFS, F::COEFF_MATRICES),
    (F::IMPLICIT_CURVE_COEFFS, F::FACTOR_SQR_FREE),
    (F::GEOM_ELIM, F::PRIM_POLY),
    (F::LOCUS_EQU, F::IMPLICIT_CURVE_COEFFS),
    (F::LOCUS_EQU, F::GEOM_ELIM),
    (F::LOCUS_EQU, F::JACOBI_PREPARE),
    (F::ENVELOPE_EQU, F::LOCUS_EQU),
    (F::ENVELOPE_EQU, F::GEOM_JACOBI_DET),
    (F::GEOM_JACOBI_DET, F::JACOBI_PREPARE),
    (F::GEOM_JACOBI_DET, F::JACOBI_DET),
    (F::AFACTOR_ALG_NUM, F::IRRED),
    (F::ABSFACT, F::AFACTOR_ALG_NUM),
    (F::COS_2PI_OVER_N_MINPOLY, F::FACTOR_SQR_FREE),
    (F::CHECK_DERIVATIVE, F::XCOORD),
    (F::CHECK_DERIVATIVE, F::YCOORD),
    (F::CHECK_DERIVATIVE, F::IS_ALMOST_ZERO),
    (F::IS_EQUAL_EQUATIONS, F::IS_ZERO),
];

fn definition(function: F) -> &'static str {
    CUSTOM
        .iter()
        .find(|(f, _, _)| *f == function)
        .map(|(_, _, text)| *text)
        .unwrap_or("")
}

/// `CustomFunctions.prereqs`: every prerequisite of `function`, in the order
/// the reference defines them.
fn prereqs(function: F) -> Vec<F> {
    let mut list = Vec::new();
    for &(key, value) in DEPENDENCIES {
        if key == function {
            list.insert(0, value);
            let deeper = prereqs(value);
            list.splice(0..0, deeper);
        }
    }
    list
}

/// The strings `CASgiacB.init` evaluates before the input, in order.
pub fn init_inputs(input: &str, timeout_ms: u64, seed: i32) -> Vec<String> {
    let mut out = vec![INIT.to_string()];
    for &(function, name, text) in CUSTOM {
        let found = name.is_some_and(|name| input.contains(name));
        if name.is_none() || found {
            out.push(text.to_string());
            if found {
                for dependency in prereqs(function) {
                    out.push(definition(dependency).to_string());
                }
            }
        }
    }
    // Giac's limit is in whole seconds; a shorter limit rounds up, never to 0
    // (which would mean no limit).
    out.push(format!(
        "caseval(\"timeout {}\")",
        timeout_ms.div_ceil(1000).max(1)
    ));
    out.push("caseval(\"ckevery 20\")".to_string());
    out.push(format!("srand({seed})"));
    out
}

/// `CASgiac.getSeed`, except that inputs without `fsolve` get `other` instead
/// of a fresh random number, so that runs are reproducible.
pub fn seed(input: &str, other: i32) -> i32 {
    if input.contains("fsolve(") {
        FSOLVE_SEED
    } else {
        other
    }
}

/// `EvalFunction.evalRaw` without the cache: the raw printed result.
pub(crate) fn eval_raw(
    context: &Context<'_>,
    input: &str,
    timeout_ms: u64,
    seed: i32,
) -> Result<String, Error> {
    for text in init_inputs(input, timeout_ms, seed) {
        context.eval(&text)?;
    }
    let wrapped = format!("caseval(evalfa({input}))");
    let mut out = context.eval(&wrapped)?;
    if out.len() >= 2 && out.starts_with('"') && out.ends_with('"') {
        out = out[1..out.len() - 1].to_string();
    }
    // Java's String.length counts UTF-16 units.
    if out.encode_utf16().count() > MAX_RESULT_LENGTH {
        out = "?".to_string();
    }
    Ok(out)
}
