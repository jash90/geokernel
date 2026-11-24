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

#[test]
fn errors_name_the_integrity_check() {
    let error = geo_giac::Error::Integrity("x".into());
    assert!(error.to_string().starts_with("refusing to load Giac"));
}

#[test]
fn loads_the_pinned_library_next_to_the_executable() {
    let library = std::env::var_os(geo_giac::LIBRARY_VARIABLE)
        .expect("the native tests need a pinned library");
    let library = std::path::Path::new(&library);
    let executable = std::env::current_exe().unwrap();
    let directory = std::env::temp_dir().join(format!("geo-giac-bundle-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let copy = directory.join(executable.file_name().unwrap());
    std::fs::copy(&executable, &copy).unwrap();
    std::fs::copy(library, directory.join(library.file_name().unwrap())).unwrap();
    let result = std::process::Command::new(copy)
        .args(["--exact", "plain_evaluation"])
        .env_remove(geo_giac::LIBRARY_VARIABLE)
        .output()
        .unwrap();
    std::fs::remove_dir_all(directory).unwrap();
    assert!(
        result.status.success() && String::from_utf8_lossy(&result.stdout).contains("1 passed"),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
