//! Optional bridge to the Giac computer algebra system.
//!
//! The reference application evaluates its CAS commands with Giac, build
//! 70507, through a JNI library. With the `giac` feature this crate loads that
//! same library at run time (the path is read from `GEO_GIAC_LIBRARY`) and
//! calls the same entry points without a Java virtual machine; see
//! `docs/cas-giac.md`. Giac is GPL software: nothing of it is compiled into
//! this crate, and without the feature the crate does nothing at all.

use std::fmt;

#[cfg(feature = "giac")]
mod jni;
#[cfg(feature = "giac")]
mod native;
#[cfg(feature = "giac")]
pub mod session;
#[cfg(feature = "giac")]
mod worker;

/// Environment variable naming the Giac library to load.
pub const LIBRARY_VARIABLE: &str = "GEO_GIAC_LIBRARY";

/// Why an evaluation produced no result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Built without the `giac` feature.
    Disabled,
    /// `GEO_GIAC_LIBRARY` is not set.
    NoLibrary,
    /// The library or one of its entry points could not be loaded.
    Load(String),
    /// A call into the library failed.
    Call(String),
    /// The worker thread is gone (it panicked or Giac crashed it).
    Worker,
    /// Giac did not answer within its time limit plus a margin.
    Timeout,
    /// Giac is still running an evaluation that timed out.
    Busy,
    /// The library's SHA-256 is not the pinned one for this platform.
    Integrity(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Disabled => write!(f, "built without the giac feature"),
            Error::NoLibrary => write!(f, "{LIBRARY_VARIABLE} is not set"),
            Error::Load(why) => write!(f, "cannot load Giac: {why}"),
            Error::Call(why) => write!(f, "Giac call failed: {why}"),
            Error::Worker => write!(f, "the Giac worker stopped"),
            Error::Timeout => write!(f, "Giac did not answer in time"),
            Error::Busy => write!(f, "Giac is still busy with an evaluation that timed out"),
            Error::Integrity(why) => write!(f, "refusing to load Giac: {why}"),
        }
    }
}

impl std::error::Error for Error {}

/// Which CAS answers, decided once per process at the first use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    /// The pinned Giac library is loaded: CAS commands go to Giac, and a
    /// failed Giac call makes its result undefined.
    Giac,
    /// No library is configured (`GEO_GIAC_LIBRARY` unset or empty, or the
    /// file absent), or the build has no `giac` feature: the kernel behaves
    /// exactly like the default build.
    KernelOnly,
    /// A library is configured but cannot be used (wrong hash, unpinned
    /// platform, load failure): every CAS call that would go to Giac gives
    /// undefined. The reason is reported once, on standard error.
    Failed,
}

/// The backend of this process (see [`Backend`]).
pub fn backend() -> Backend {
    #[cfg(feature = "giac")]
    {
        worker::backend().0
    }
    #[cfg(not(feature = "giac"))]
    {
        Backend::KernelOnly
    }
}

/// Why the backend is not [`Backend::Giac`], if it is not.
pub fn backend_reason() -> Option<String> {
    #[cfg(feature = "giac")]
    {
        worker::backend().1.clone()
    }
    #[cfg(not(feature = "giac"))]
    {
        Some("built without the giac feature".into())
    }
}

/// Test support: while set, every evaluation fails with this error, as if
/// Giac had (for [`Error::Timeout`], [`Error::Busy`], ...). Not for use
/// outside tests.
#[doc(hidden)]
pub fn simulate_failure(error: Option<Error>) {
    #[cfg(feature = "giac")]
    worker::simulate_failure(error);
    #[cfg(not(feature = "giac"))]
    let _ = error;
}

/// Whether this build can evaluate with Giac at all.
pub const fn enabled() -> bool {
    cfg!(feature = "giac")
}

/// Evaluates `input` (Giac syntax, as the reference's translation produces
/// it) the way the reference's `EvalFunction.evalRaw` does, and returns the
/// printed result with its surrounding quotes removed.
///
/// `seed` replaces the random seed the reference picks for inputs that do not
/// call `fsolve`.
pub fn eval_raw(input: &str, timeout_ms: u64, seed: i32) -> Result<String, Error> {
    #[cfg(feature = "giac")]
    {
        worker::eval_raw(input, timeout_ms, seed)
    }
    #[cfg(not(feature = "giac"))]
    {
        let _ = (input, timeout_ms, seed);
        Err(Error::Disabled)
    }
}

/// Evaluates one string exactly as given (`gen(text).eval(1).print()`),
/// without the reference's reset and wrapping. For tests and diagnostics.
pub fn eval_plain(text: &str) -> Result<String, Error> {
    #[cfg(feature = "giac")]
    {
        worker::eval_plain(text)
    }
    #[cfg(not(feature = "giac"))]
    {
        let _ = text;
        Err(Error::Disabled)
    }
}
