pub mod error;
pub mod header;
pub mod platform;
pub mod region;
pub mod ring;
pub mod sync;

use std::ffi::{c_char, c_void, CStr};

pub use error::{Result, ZincError};
pub use region::SharedRegion;
pub use sync::{notify, wait};

// --- C ABI surface ---

type ZincHandle = *mut c_void;

fn error_code(e: &ZincError) -> i32 {
    match e {
        ZincError::AlreadyExists(_) => -17,  // EEXIST
        ZincError::NotFound(_) => -2,        // ENOENT
        ZincError::InvalidSize { .. } => -22, // EINVAL
        ZincError::InvalidName => -22,
        ZincError::PermissionDenied => -1,
        ZincError::RingFull => -11,           // EAGAIN
        ZincError::TimedOut => -110,          // ETIMEDOUT
        ZincError::CorruptedRegion => -74,    // EBADMSG
        ZincError::Platform(io) => -(io.raw_os_error().unwrap_or(1)),
    }
}

unsafe fn to_region<'a>(h: ZincHandle) -> &'a SharedRegion {
    &*(h as *const SharedRegion)
}

fn pack(r: SharedRegion) -> ZincHandle {
    Box::into_raw(Box::new(r)) as *mut c_void
}

fn unpack(h: ZincHandle) -> Box<SharedRegion> {
    unsafe { Box::from_raw(h as *mut SharedRegion) }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn zinc_create(
    name: *const c_char,
    capacity: usize,
    out: *mut ZincHandle,
) -> i32 {
    let name = unsafe { CStr::from_ptr(name) }.to_str().unwrap_or("");
    match SharedRegion::create(name, capacity) {
        Ok(r) => {
            unsafe { *out = pack(r); }
            0
        }
        Err(e) => error_code(&e),
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn zinc_open(name: *const c_char, out: *mut ZincHandle) -> i32 {
    let name = unsafe { CStr::from_ptr(name) }.to_str().unwrap_or("");
    match SharedRegion::open(name) {
        Ok(r) => {
            unsafe { *out = pack(r); }
            0
        }
        Err(e) => error_code(&e),
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn zinc_ptr(h: ZincHandle) -> *mut u8 {
    if h.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { to_region(h).as_ptr() }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn zinc_capacity(h: ZincHandle) -> usize {
    if h.is_null() {
        return 0;
    }
    unsafe { to_region(h).capacity() }
}

#[no_mangle]
pub extern "C" fn zinc_close(h: ZincHandle) {
    if !h.is_null() {
        drop(unpack(h));
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn zinc_notify(h: ZincHandle) {
    if !h.is_null() {
        unsafe { to_region(h).notify(); }
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn zinc_wait(h: ZincHandle, timeout_ms: u32) -> i32 {
    if h.is_null() {
        return error_code(&ZincError::InvalidName);
    }
    match unsafe { to_region(h).wait(timeout_ms) } {
        Ok(()) => 0,
        Err(ZincError::TimedOut) => -110,
        Err(e) => error_code(&e),
    }
}

#[no_mangle]
pub extern "C" fn zinc_version() -> u32 {
    (header::VERSION as u32) << 16 | 0x0001
}
