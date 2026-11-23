//! Needs the `giac` feature and GEO_GIAC_LIBRARY pointing at the Giac library
//! (tools/giac/fetch.sh prints the path).
#![cfg(feature = "giac")]

#[test]
fn plain_evaluation() {
    assert_eq!(geo_giac::eval_plain("1+2").unwrap(), "3");
}

#[test]
fn reference_protocol() {
    assert_eq!(
        geo_giac::eval_raw("nextprime(100)", 5000, 1).unwrap(),
        "101"
    );
    assert_eq!(
        geo_giac::eval_raw("factor(x^2-1)", 5000, 1).unwrap(),
        "(x-1)*(x+1)"
    );
}
