//! With the `giac` feature but no Giac library configured, the kernel
//! behaves exactly like the default build (docs/cas-giac.md, "Backends and
//! failures"). Runs in its own process, so the backend is decided here.
#![cfg(feature = "giac")]

use geo_kernel::script;

#[test]
fn without_a_library_the_kernel_answers_as_the_default_build() {
    std::env::remove_var(geo_giac::LIBRARY_VARIABLE);
    assert_eq!(geo_giac::backend(), geo_giac::Backend::KernelOnly);
    assert!(!geo_kernel::giac_backend_active());

    let c = script::load("r = Solve(x^2 = 4)").unwrap();
    let list = c.elements(c.id("r").unwrap()).unwrap();
    let lines: Vec<_> = list.iter().map(|e| format!("{e:?}")).collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].contains("c: 2.0") && lines[1].contains("c: -2.0"),
        "{lines:?}"
    );

    // A start value is a form only the Giac backend accepts: refused as in
    // the default build, never answered with the start value ignored.
    let mut c = script::load("f(x) = x^2 - 2").unwrap();
    let error = script::exec(&mut c, "r = NSolutions(f, x = 1)").unwrap_err();
    assert!(
        error.contains("expected up to three distinct solving variables"),
        "{error}"
    );
    let error = script::exec(&mut c, "p = PlotSolve(f)").unwrap_err();
    assert!(error.contains("PlotSolve"), "{error}");
    assert!(c.id("r").is_none());
}
