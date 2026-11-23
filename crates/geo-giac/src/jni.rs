//! The smallest JNI environment the Giac wrapper functions need.
//!
//! The Giac release used by the reference application ships as a JNI library:
//! its entry points are the `Java_javagiac_giacJNI_*` functions that a Java
//! class would call. Those functions take a `JNIEnv` and use it only to read
//! the bytes of an input string, release them, and build an output string
//! (plus, on a null argument, to raise a Java exception). This module provides
//! a function table with exactly those slots, so the entry points can be called
//! without a Java virtual machine. Every other slot aborts the process: a call
//! there would mean the library does something this bridge does not expect.
//!
//! The slot numbers are those of the JNI specification's function table.

use std::cell::RefCell;
use std::ffi::{c_char, c_void, CStr, CString};

const FIND_CLASS: usize = 6;
const THROW_NEW: usize = 14;
const EXCEPTION_CLEAR: usize = 17;
const NEW_STRING_UTF: usize = 167;
const GET_STRING_UTF_LENGTH: usize = 168;
const GET_STRING_UTF_CHARS: usize = 169;
const RELEASE_STRING_UTF_CHARS: usize = 170;

/// Large enough for every slot defined by any JNI version so far.
const SLOTS: usize = 240;

/// A `JNIEnv`: a pointer to the function table.
#[repr(C)]
pub(crate) struct Env {
    table: *const [*const c_void; SLOTS],
}

/// A value handed to the wrapper functions where they expect a `jstring`.
/// It is the address of a NUL-terminated byte string owned by the caller.
pub(crate) type JString = *const c_char;

thread_local! {
    /// Strings created by the library through `NewStringUTF` during the
    /// current call. Cleared before each call.
    static CREATED: RefCell<Vec<CString>> = const { RefCell::new(Vec::new()) };
    /// The message of an exception the library raised during the current call.
    static THROWN: RefCell<Option<String>> = const { RefCell::new(None) };
}

unsafe extern "system" fn unexpected() {
    eprintln!("geo-giac: the Giac library called a JNI function this bridge does not provide");
    std::process::abort();
}

unsafe extern "system" fn find_class(_env: *mut Env, _name: *const c_char) -> *mut c_void {
    // Any non-null value: it is only passed back to ThrowNew.
    std::ptr::NonNull::<u8>::dangling().as_ptr().cast()
}

unsafe extern "system" fn throw_new(
    _env: *mut Env,
    _class: *mut c_void,
    message: *const c_char,
) -> i32 {
    let text = if message.is_null() {
        String::from("exception without message")
    } else {
        // SAFETY: the library passes a NUL-terminated message.
        unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned()
    };
    THROWN.with(|thrown| *thrown.borrow_mut() = Some(text));
    0
}

unsafe extern "system" fn exception_clear(_env: *mut Env) {
    THROWN.with(|thrown| *thrown.borrow_mut() = None);
}

unsafe extern "system" fn new_string_utf(_env: *mut Env, bytes: *const c_char) -> *mut c_void {
    if bytes.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: the library passes a NUL-terminated string.
    let owned = unsafe { CStr::from_ptr(bytes) }.to_owned();
    let address = owned.as_ptr() as *mut c_void;
    // Moving a CString keeps its heap buffer, so the address stays valid.
    CREATED.with(|created| created.borrow_mut().push(owned));
    address
}

unsafe extern "system" fn get_string_utf_length(_env: *mut Env, string: JString) -> i32 {
    // SAFETY: every JString this bridge passes is NUL-terminated.
    unsafe { CStr::from_ptr(string) }.to_bytes().len() as i32
}

unsafe extern "system" fn get_string_utf_chars(
    _env: *mut Env,
    string: JString,
    is_copy: *mut u8,
) -> *const c_char {
    if !is_copy.is_null() {
        // SAFETY: the library passes a valid out pointer when it is not null.
        unsafe { *is_copy = 0 };
    }
    string
}

unsafe extern "system" fn release_string_utf_chars(
    _env: *mut Env,
    _string: JString,
    _chars: *const c_char,
) {
}

struct Table([*const c_void; SLOTS]);

// SAFETY: the table only holds addresses of functions, which never change.
unsafe impl Sync for Table {}

static TABLE: Table = {
    let mut slots = [unexpected as *const c_void; SLOTS];
    let mut reserved = 0;
    while reserved < 4 {
        slots[reserved] = std::ptr::null();
        reserved += 1;
    }
    slots[FIND_CLASS] = find_class as *const c_void;
    slots[THROW_NEW] = throw_new as *const c_void;
    slots[EXCEPTION_CLEAR] = exception_clear as *const c_void;
    slots[NEW_STRING_UTF] = new_string_utf as *const c_void;
    slots[GET_STRING_UTF_LENGTH] = get_string_utf_length as *const c_void;
    slots[GET_STRING_UTF_CHARS] = get_string_utf_chars as *const c_void;
    slots[RELEASE_STRING_UTF_CHARS] = release_string_utf_chars as *const c_void;
    Table(slots)
};

impl Env {
    pub(crate) fn new() -> Self {
        Env { table: &TABLE.0 }
    }
}

/// Clears the per-call state before a wrapper function is called.
pub(crate) fn begin_call() {
    CREATED.with(|created| created.borrow_mut().clear());
    THROWN.with(|thrown| *thrown.borrow_mut() = None);
}

/// The exception the last wrapper call raised, if any.
pub(crate) fn take_thrown() -> Option<String> {
    THROWN.with(|thrown| thrown.borrow_mut().take())
}

/// Reads a string the last wrapper call returned through `NewStringUTF`.
pub(crate) fn read_created(address: *mut c_void) -> Option<String> {
    if address.is_null() {
        return None;
    }
    CREATED.with(|created| {
        created
            .borrow()
            .iter()
            .find(|owned| owned.as_ptr() as *mut c_void == address)
            .map(|owned| owned.to_string_lossy().into_owned())
    })
}
