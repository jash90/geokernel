//! With the Giac backend a failed Giac call (time limit, busy worker) makes
//! the result undefined, with no fallback to the kernel's own solver, and a
//! later recompute succeeds once Giac answers again. Needs the library
//! (GEO_GIAC_LIBRARY, see tools/giac/fetch.sh).
#![cfg(feature = "giac")]

use geo_kernel::script;

#[test]
fn failed_giac_calls_give_undefined_until_giac_answers() {
    assert_eq!(
        geo_giac::backend(),
        geo_giac::Backend::Giac,
        "set GEO_GIAC_LIBRARY to the Giac library (tools/giac/fetch.sh)"
    );
    for error in [geo_giac::Error::Timeout, geo_giac::Error::Busy] {
        let mut c = script::load("a = 4\nb = 1\nf(x) = x^2 - 2").unwrap();
        geo_giac::simulate_failure(Some(error.clone()));
        let r = script::exec(&mut c, "r = Solve(x^2 = a)").unwrap().unwrap();
        let s = script::exec(&mut c, "s = NSolutions(f, x = b)")
            .unwrap()
            .unwrap();
        assert!(!c.value(r).is_defined(), "{error:?}: Solve");
        assert!(!c.value(s).is_defined(), "{error:?}: start value");
        geo_giac::simulate_failure(None);
        script::exec(&mut c, "a = 9").unwrap();
        script::exec(&mut c, "b = 2").unwrap();
        assert_eq!(
            c.elements(r).unwrap().len(),
            2,
            "{error:?}: Solve after recovery"
        );
        let roots = c.elements(s).unwrap();
        assert_eq!(roots.len(), 1, "{error:?}: start value after recovery");
    }
}
