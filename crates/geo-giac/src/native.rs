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
    // Kept last so the entry points above are dropped before the library.
    _library: Library,
}

fn symbol<T: Copy>(library: &Library, name: &str) -> Result<T, Error> {
    let full = format!("{PREFIX}{name}\0");
    // SAFETY: each requested type matches the wrapper's C signature.
    unsafe { library.get::<T>(full.as_bytes()) }
        .map(|found| *found)
        .map_err(|error| Error::Load(format!("{PREFIX}{name}: {error}")))
}

impl Native {
    pub(crate) fn load(path: &Path) -> Result<Self, Error> {
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
        let parsed = Gen {
            native,
            handle: parsed,
        };

        jni::begin_call();
        // SAFETY: both handles are live.
        let value =
            unsafe { (native.gen_eval)(&mut env, null, parsed.handle, null, 1, self.handle, null) };
        native.check("eval")?;
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
