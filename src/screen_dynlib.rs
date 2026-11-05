//! Runtime dynamic loading shared by the Darwin and Linux CU backends.
//!
//! The six release cells are cross-compiled on a single macOS runner (zig for
//! Linux, xwin for Windows). A `#[link(name = "X11")]` or framework link would
//! need a foreign SDK on that runner, so every foreign symbol is resolved at
//! first use with `dlopen`/`dlsym`. `dlopen` itself lives in libc (glibc >=
//! 2.34) / libSystem, which every cell already links; no new crate and no new
//! link-time library is introduced.
//!
//! A missing library or symbol is a *named refusal*. It never becomes a panic,
//! an empty observation or a black image.

use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::{Mutex, OnceLock};

extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
}

// RTLD_LAZY is 0x1 on both Darwin and Linux. RTLD_GLOBAL is not used: its
// value differs between the two, and these handles are private to this crate.
const RTLD_LAZY: c_int = 0x1;

fn last_error() -> String {
    let pointer = unsafe { dlerror() };
    if pointer.is_null() {
        "no dlerror detail".to_owned()
    } else {
        unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .trim()
            .to_owned()
    }
}

#[derive(Clone, Copy)]
pub struct Lib {
    handle: usize,
    name: &'static str,
}

// A `dlopen` handle is process-wide and immutable after open. Nothing mutates
// through it here, so sharing it across host threads is sound.
unsafe impl Send for Lib {}
unsafe impl Sync for Lib {}

static HANDLES: OnceLock<Mutex<HashMap<&'static str, usize>>> = OnceLock::new();

impl Lib {
    /// Opens a soname once per process. `dlopen` keeps a reference count; the
    /// handle is deliberately never closed, because function pointers cached
    /// by callers would dangle after `dlclose`.
    pub fn open(name: &'static str) -> Result<Self, String> {
        let handles = HANDLES.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = handles
            .lock()
            .map_err(|_| "cu: unsupported: the dynamic loader cache is poisoned".to_owned())?;
        if let Some(handle) = guard.get(&name) {
            return Ok(Self {
                handle: *handle,
                name,
            });
        }
        let path = CString::new(name)
            .map_err(|_| format!("cu: unsupported: invalid library name {name:?}"))?;
        let handle = unsafe { dlopen(path.as_ptr(), RTLD_LAZY) };
        if handle.is_null() {
            return Err(format!(
                "cu: unsupported: {name} is unavailable on this session ({detail}); the \
                 backend cannot perform this effect and reports a named refusal, not an empty \
                 result",
                detail = last_error()
            ));
        }
        let handle = handle as usize;
        guard.insert(name, handle);
        Ok(Self { handle, name })
    }

    /// Resolves one symbol. The caller states the exact `extern "C"` fn type;
    /// a signature drift is a link error at the call site, not a guess here.
    ///
    /// # Safety
    /// `T` must be the true type of the symbol in `self.name`.
    pub unsafe fn sym<T: Copy>(&self, symbol: &str) -> Result<T, String> {
        debug_assert_eq!(
            std::mem::size_of::<T>(),
            std::mem::size_of::<*mut c_void>(),
            "only pointer-sized function pointers may be resolved"
        );
        let name = CString::new(symbol)
            .map_err(|_| format!("cu: unsupported: invalid symbol name {symbol:?}"))?;
        let pointer = unsafe { dlsym(self.handle as *mut c_void, name.as_ptr()) };
        if pointer.is_null() {
            return Err(format!(
                "cu: unsupported: {} does not export {symbol}; the backend cannot perform this \
                 effect and reports a named refusal",
                self.name
            ));
        }
        Ok(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&pointer) })
    }
}

/// Convenience: open a library and resolve one symbol in one expression.
///
/// # Safety
/// `T` must be the true type of `symbol` in `library`.
pub unsafe fn sym<T: Copy>(library: &'static str, symbol: &str) -> Result<T, String> {
    let lib = Lib::open(library)?;
    unsafe { lib.sym::<T>(symbol) }
}
