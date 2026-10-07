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
        ZincError::AlreadyExists(_) => -17,   // EEXIST
        ZincError::NotFound(_) => -2,         // ENOENT
        ZincError::InvalidSize { .. } => -22, // EINVAL
        ZincError::InvalidName => -22,
        ZincError::PermissionDenied => -1,
        ZincError::RingFull => -11,        // EAGAIN
        ZincError::TimedOut => -110,       // ETIMEDOUT
        ZincError::CorruptedRegion => -74, // EBADMSG
        ZincError::Platform(io) | ZincError::Syscall { source: io, .. } => {
            -(io.raw_os_error().unwrap_or(1))
        }
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
pub extern "C" fn zinc_create(name: *const c_char, capacity: usize, out: *mut ZincHandle) -> i32 {
    let name = unsafe { CStr::from_ptr(name) }.to_str().unwrap_or("");
    match SharedRegion::create(name, capacity) {
        Ok(r) => {
            unsafe {
                *out = pack(r);
            }
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
            unsafe {
                *out = pack(r);
            }
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
        unsafe {
            to_region(h).notify();
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use crate::region::page_size;
    use std::ptr;

    #[test]
    #[cfg(unix)]
    fn zinc_create_open_close() {
        let name = b"test_cabi\0".as_ptr() as *const c_char;
        let mut handle: ZincHandle = ptr::null_mut();
        let page = page_size();

        let code = zinc_create(name, page, &mut handle);
        assert_eq!(code, 0, "zinc_create failed: {code}");
        assert!(!handle.is_null());

        let ptr = zinc_ptr(handle);
        assert!(!ptr.is_null());
        let cap = zinc_capacity(handle);
        assert_eq!(cap, page);

        // Open second handle
        let mut handle2: ZincHandle = ptr::null_mut();
        let code = zinc_open(name, &mut handle2);
        assert_eq!(code, 0, "zinc_open failed: {code}");
        assert!(!handle2.is_null());

        zinc_close(handle2);
        zinc_close(handle);
    }

    #[test]
    fn zinc_null_handle_safety() {
        let ptr = zinc_ptr(ptr::null_mut());
        assert!(ptr.is_null());
        let cap = zinc_capacity(ptr::null_mut());
        assert_eq!(cap, 0);
        zinc_notify(ptr::null_mut()); // should not crash
        zinc_close(ptr::null_mut()); // should not crash
    }

    #[test]
    fn zinc_version_check() {
        let v = zinc_version();
        assert!(v > 0);
    }
}
