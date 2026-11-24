//! Calls into the Giac library through its JNI entry points.
//!
//! The reference application talks to Giac through three operations only:
//! build an expression from text in a context, evaluate it at level 1, and
//! print the result in that context (see `GenImpl` and `ContextImpl` of its
//! JRE binding). This module calls the same wrapper functions with the same
//! arguments, so the library runs the same code it runs for the reference.

use std::ffi::{c_void, CString};
use std::path::Path;

use libloading::Library;

use crate::jni::{self, Env, JString};
use crate::Error;

type Class = *mut c_void;
type Object = *mut c_void;

type NewContext = unsafe extern "system" fn(*mut Env, Class) -> i64;
type DeleteHandle = unsafe extern "system" fn(*mut Env, Class, i64);
type NewGen = unsafe extern "system" fn(*mut Env, Class, JString, i64, Object) -> i64;
type GenEval = unsafe extern "system" fn(*mut Env, Class, i64, Object, i32, i64, Object) -> i64;
type GenPrint = unsafe extern "system" fn(*mut Env, Class, i64, Object, i64, Object) -> *mut c_void;

const PREFIX: &str = "Java_javagiac_giacJNI_";

/// The loaded library and the entry points this bridge uses.
pub(crate) struct Native {
    new_context: NewContext,
    delete_context: DeleteHandle,
    // gen(String, context)
    new_gen: NewGen,
    gen_eval: GenEval,
    // gen.print(context)
    gen_print: GenPrint,
    delete_gen: DeleteHandle,
    // Keep the library and, on Android, its runtime dependency alive.
    _library: Library,
    #[cfg(target_os = "android")]
    _cxx: Library,
}

fn symbol<T: Copy>(library: &Library, name: &str) -> Result<T, Error> {
    let full = format!("{PREFIX}{name}\0");
    // SAFETY: each requested type matches the wrapper's C signature.
    unsafe { library.get::<T>(full.as_bytes()) }
        .map(|found| *found)
        .map_err(|error| Error::Load(format!("{PREFIX}{name}: {error}")))
}

pub(crate) const LIBRARY_NAME: Option<&str> =
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("libjavagiac64.so")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("libjavagiac-arm64.jnilib")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("libjavagiac.jnilib")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("javagiac64.dll")
    } else {
        None
    };

/// SHA-256 of the Giac 70507 library for this platform, as extracted from
/// the reference's release (`tools/giac/fetch.sh` pins the same values).
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const PINNED: Option<&str> =
    Some("d4c7f38357f06cc6a364d551ac3c375568b703c3abc4da76674e9642ce0316ec");
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const PINNED: Option<&str> =
    Some("a2ed7741e02e8569c9cc9354c9818d883e3d3175de8389c557930f13bd48c498");
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const PINNED: Option<&str> =
    Some("b9abc2abb81c3dbc8397754e716483a61ace680106b15ccb66e19507fca10939");
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const PINNED: Option<&str> =
    Some("d54fbab833cc4a6bc07cb5e7f9c3a83cb5b08c33402198b3ca0495c77bdf0d71");
#[cfg(all(target_os = "android", target_arch = "aarch64"))]
const PINNED: Option<&str> =
    Some("f84e31b6f8a55974ff6eaffcd03e12bf911f45190323f52914f3d7c9deb1ae6e");
#[cfg(all(target_os = "android", target_arch = "arm"))]
const PINNED: Option<&str> =
    Some("a8a7823594fa2bc6b6395cf16d960fc91f8f51a6c007510911e0703050ed59b7");
#[cfg(all(target_os = "android", target_arch = "x86"))]
const PINNED: Option<&str> =
    Some("b03501ebf9d53b92c54ee49e8da5b19017936715797e6f69a663e1073ab0332f");
#[cfg(all(target_os = "android", target_arch = "x86_64"))]
const PINNED: Option<&str> =
    Some("8a737510102704b6c59a5d817bdfde7475e0c1723bd2404665f0cd73c99b3983");
#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(
        target_os = "macos",
        any(target_arch = "aarch64", target_arch = "x86_64")
    ),
    all(target_os = "windows", target_arch = "x86_64"),
    all(
        target_os = "android",
        any(
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "x86",
            target_arch = "x86_64"
        )
    ),
)))]
const PINNED: Option<&str> = None;

#[cfg(target_os = "android")]
const CXX_PINNED: &str = match std::env::consts::ARCH.as_bytes() {
    b"aarch64" => "ee66b768f37c9fc5e8b9a0f926e57400101307c8bdcbc59ff4c07b55f93590e7",
    b"arm" => "835c589f8199a35f135c960fa36b05ee2682382a6b4218f1a7a354713665135c",
    b"x86" => "9a8ce7e9b7339d782e807b7d203271791dd21d8be6028f3861122b412af7905b",
    b"x86_64" => "69d9c9695dd3bc15440c1e348abb4be2ecb67497095940e5c0f1ce0cfe7b10c6",
    // verify() rejects an unsupported ABI before this pin is used.
    _ => "",
};

/// Refuses a library whose contents are not the pinned build: loading runs
/// its code in this process.
fn verify(path: &Path) -> Result<(), Error> {
    let Some(pinned) = PINNED else {
        return Err(Error::Integrity(
            "no Giac build is pinned for this platform".into(),
        ));
    };
    verify_sha256(path, pinned)
}

fn verify_sha256(path: &Path, pinned: &str) -> Result<(), Error> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).map_err(|e| Error::Load(format!("{}: {e}", path.display())))?;
    let digest = Sha256::digest(&bytes);
    let actual: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if actual != pinned {
        return Err(Error::Integrity(format!(
            "{} has SHA-256 {actual}, expected the Giac 70507 build {pinned}",
            path.display()
        )));
    }
    Ok(())
}

impl Native {
    pub(crate) fn load(path: &Path) -> Result<Self, Error> {
        verify(path)?;
        #[cfg(target_os = "android")]
        let cxx = {
            let cxx_path = path.with_file_name("libc++_shared.so");
            verify_sha256(&cxx_path, CXX_PINNED)?;
            // SAFETY: this is the C++ runtime from the same pinned AAR.
            unsafe { Library::new(&cxx_path) }
                .map_err(|error| Error::Load(format!("{}: {error}", cxx_path.display())))?
        };
        // SAFETY: loading Giac runs its static initialisers, which only set up
        // Giac's own tables.
        let library = unsafe { Library::new(path) }
            .map_err(|error| Error::Load(format!("{}: {error}", path.display())))?;
        Ok(Native {
            new_context: symbol(&library, "new_1context_1_1SWIG_10")?,
            delete_context: symbol(&library, "delete_1context")?,
            new_gen: symbol(&library, "new_1gen_1_1SWIG_125")?,
            gen_eval: symbol(&library, "gen_1eval")?,
            gen_print: symbol(&library, "gen_1print_1_1SWIG_10")?,
            delete_gen: symbol(&library, "delete_1gen")?,
            _library: library,
            #[cfg(target_os = "android")]
            _cxx: cxx,
        })
    }

    fn check(&self, what: &str) -> Result<(), Error> {
        match jni::take_thrown() {
            Some(message) => Err(Error::Call(format!("{what}: {message}"))),
            None => Ok(()),
        }
    }

    pub(crate) fn context(&self) -> Result<Context<'_>, Error> {
        let mut env = Env::new();
        jni::begin_call();
        // SAFETY: the signature matches; the class argument is unused.
        let handle = unsafe { (self.new_context)(&mut env, std::ptr::null_mut()) };
        self.check("context")?;
        if handle == 0 {
            return Err(Error::Call("context: null".into()));
        }
        Ok(Context {
            native: self,
            handle,
        })
    }
}

/// A Giac evaluation context.
pub(crate) struct Context<'a> {
    native: &'a Native,
    handle: i64,
}

impl Drop for Context<'_> {
    fn drop(&mut self) {
        let mut env = Env::new();
        jni::begin_call();
        // SAFETY: the handle came from new_context and is deleted once.
        unsafe { (self.native.delete_context)(&mut env, std::ptr::null_mut(), self.handle) };
    }
}

struct Gen<'a> {
    native: &'a Native,
    handle: i64,
}

impl Drop for Gen<'_> {
    fn drop(&mut self) {
        let mut env = Env::new();
        jni::begin_call();
        // SAFETY: the handle came from new_gen or gen_eval and is deleted once.
        unsafe { (self.native.delete_gen)(&mut env, std::ptr::null_mut(), self.handle) };
    }
}

impl Context<'_> {
    /// `new gen(text, context).eval(1, context).print(context)`, the
    /// reference's evaluation of one input string.
    pub(crate) fn eval(&self, text: &str) -> Result<String, Error> {
        let native = self.native;
        let input = CString::new(text).map_err(|_| Error::Call("input contains NUL".into()))?;
        let null = std::ptr::null_mut();
        let mut env = Env::new();

        jni::begin_call();
        // SAFETY: input outlives the call; the context handle is live.
        let parsed = unsafe { (native.new_gen)(&mut env, null, input.as_ptr(), self.handle, null) };
        native.check("gen")?;
        if parsed == 0 {
            return Err(Error::Call("gen: null handle".into()));
        }
        let parsed = Gen {
            native,
            handle: parsed,
        };

        jni::begin_call();
        // SAFETY: both handles are live.
        let value =
            unsafe { (native.gen_eval)(&mut env, null, parsed.handle, null, 1, self.handle, null) };
        native.check("eval")?;
        if value == 0 {
            return Err(Error::Call("eval: null handle".into()));
        }
        let value = Gen {
            native,
            handle: value,
        };

        jni::begin_call();
        // SAFETY: both handles are live.
        let printed =
            unsafe { (native.gen_print)(&mut env, null, value.handle, null, self.handle, null) };
        native.check("print")?;
        jni::read_created(printed).ok_or_else(|| Error::Call("print: no string".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_library_with_another_hash_is_refused() {
        let path = std::env::temp_dir().join(format!("geo-giac-not-giac-{}", std::process::id()));
        std::fs::write(&path, b"not the Giac library").unwrap();
        let result = verify(&path);
        std::fs::remove_file(&path).unwrap();
        assert!(matches!(result, Err(Error::Integrity(_))), "{result:?}");
    }
}
