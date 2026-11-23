//! A configured Giac library that is not the pinned build: every Giac result
//! is undefined, with no fallback to the kernel's own solver.
#![cfg(feature = "giac")]

use geo_kernel::script;

#[test]
fn a_corrupt_library_makes_results_undefined() {
    let path = std::env::temp_dir().join(format!("geo-giac-corrupt-{}", std::process::id()));
    std::fs::write(&path, b"not the Giac library").unwrap();
    std::env::set_var(geo_giac::LIBRARY_VARIABLE, &path);
    assert_eq!(geo_giac::backend(), geo_giac::Backend::Failed);
    let reason = geo_giac::backend_reason().unwrap();
    assert!(reason.contains("SHA-256"), "{reason}");
    std::fs::remove_file(&path).unwrap();

    let mut c = script::load("f(x) = x^2 - 2").unwrap();
    for line in [
        "r = Solve(x^2 = 4)",
        "s = NSolutions(f, x = 1)",
        "t = Solutions(f)",
    ] {
        let id = script::exec(&mut c, line).unwrap().unwrap();
        assert!(!c.value(id).is_defined(), "{line} should be undefined");
    }
}
