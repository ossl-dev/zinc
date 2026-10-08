#![deny(unsafe_op_in_unsafe_fn)]

pub mod error;
pub mod header;
pub mod platform;
pub mod region;
pub mod ring;
pub mod sync;

use std::ffi::{c_char, c_void, CStr};

pub use error::{Result, ZincError};
pub use region::{page_size, SharedRegion};
pub use sync::{notify, wait};

type ZincHandle = *mut c_void;

fn error_code(e: &ZincError) -> i32 {
    match e {
        ZincError::AlreadyExists(_) => -17,
        ZincError::NotFound(_) => -2,
        ZincError::InvalidSize { .. }
        | ZincError::InvalidName
        | ZincError::InvalidRingCapacity
        | ZincError::OutOfBounds { .. } => -22,
        ZincError::PermissionDenied => -1,
        ZincError::RingFull | ZincError::WouldBlock => -11,
        ZincError::TimedOut => -110,
        ZincError::CorruptedRegion => -74,
        ZincError::Platform(io) | ZincError::Syscall { source: io, .. } => {
            -io.raw_os_error().unwrap_or(1)
        }
    }
}

unsafe fn to_region<'a>(h: ZincHandle) -> &'a SharedRegion {
    unsafe { &*h.cast::<SharedRegion>() }
}

unsafe fn make_handle(
    name: *const c_char,
    out: *mut ZincHandle,
    create: impl FnOnce(&str) -> Result<SharedRegion>,
) -> i32 {
    if out.is_null() {
        return -22;
    }
    unsafe { out.write(std::ptr::null_mut()) };
    if name.is_null() {
        return -22;
    }
    let Ok(name) = unsafe { CStr::from_ptr(name) }.to_str() else {
        return -22;
    };
    match create(name) {
        Ok(region) => {
            unsafe { out.write(Box::into_raw(Box::new(region)).cast()) };
            0
        }
        Err(error) => error_code(&error),
    }
}

/// # Safety
/// Non-null `name` must be a readable, NUL-terminated string and `out` writable.
#[no_mangle]
pub unsafe extern "C" fn zinc_create(
    name: *const c_char,
    capacity: usize,
    out: *mut ZincHandle,
) -> i32 {
    unsafe { make_handle(name, out, |name| SharedRegion::create(name, capacity)) }
}

/// # Safety
/// Non-null `name` must be a readable, NUL-terminated string and `out` writable.
#[no_mangle]
pub unsafe extern "C" fn zinc_open(name: *const c_char, out: *mut ZincHandle) -> i32 {
    unsafe { make_handle(name, out, SharedRegion::open) }
}

/// # Safety
/// Non-null `h` must be a live Zinc handle. The returned pointer expires on close.
#[no_mangle]
pub unsafe extern "C" fn zinc_ptr(h: ZincHandle) -> *mut u8 {
    if h.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { to_region(h).as_ptr() }
}

/// # Safety
/// Non-null `h` must be a live Zinc handle.
#[no_mangle]
pub unsafe extern "C" fn zinc_capacity(h: ZincHandle) -> usize {
    if h.is_null() {
        return 0;
    }
    unsafe { to_region(h).capacity() }
}

/// # Safety
/// Non-null `h` must be a live Zinc handle, closed exactly once after all uses finish.
#[no_mangle]
pub unsafe extern "C" fn zinc_close(h: ZincHandle) {
    if !h.is_null() {
        drop(unsafe { Box::from_raw(h.cast::<SharedRegion>()) });
    }
}

/// # Safety
/// Non-null `h` must be a live Zinc handle.
#[no_mangle]
pub unsafe extern "C" fn zinc_notify(h: ZincHandle) {
    if !h.is_null() {
        unsafe { to_region(h).notify() };
    }
}

/// # Safety
/// Non-null `h` must be a live Zinc handle for the duration of the wait.
#[no_mangle]
pub unsafe extern "C" fn zinc_wait(h: ZincHandle, timeout_ms: u32) -> i32 {
    if h.is_null() {
        return -22;
    }
    match unsafe { to_region(h).wait(timeout_ms) } {
        Ok(()) => 0,
        Err(error) => error_code(&error),
    }
}

/// Return 0 for a pending notification, -11 otherwise, or -22 for a null handle.
/// # Safety
/// Non-null `h` must be a live Zinc handle.
#[no_mangle]
pub unsafe extern "C" fn zinc_try_wait(h: ZincHandle) -> i32 {
    if h.is_null() {
        return -22;
    }
    if unsafe { to_region(h).try_wait() } {
        0
    } else {
        error_code(&ZincError::WouldBlock)
    }
}

#[no_mangle]
pub extern "C" fn zinc_version() -> u32 {
    u32::from(header::VERSION) << 16 | 0x0001
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
        unsafe {
            let name = c"test_cabi".as_ptr();
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
    }

    #[test]
    fn zinc_null_handle_safety() {
        unsafe {
            let ptr = zinc_ptr(ptr::null_mut());
            assert!(ptr.is_null());
            let cap = zinc_capacity(ptr::null_mut());
            assert_eq!(cap, 0);
            zinc_notify(ptr::null_mut()); // should not crash
            zinc_close(ptr::null_mut()); // should not crash
        }
    }

    #[test]
    fn zinc_version_check() {
        let v = zinc_version();
        assert!(v > 0);
    }

    #[test]
    fn invalid_ffi_inputs_clear_output() {
        unsafe {
            let mut handle = std::ptr::dangling_mut::<c_void>();
            assert_eq!(zinc_create(ptr::null(), page_size(), &mut handle), -22);
            assert!(handle.is_null());
            assert_eq!(zinc_open(c"bad/name".as_ptr(), &mut handle), -22);
            assert!(handle.is_null());
            assert_eq!(
                zinc_create(c"valid".as_ptr(), page_size(), ptr::null_mut()),
                -22
            );
            assert_eq!(zinc_open(ptr::null(), ptr::null_mut()), -22);
            assert_eq!(zinc_wait(ptr::null_mut(), 0), -22);
            assert_eq!(zinc_try_wait(ptr::null_mut()), -22);
            let invalid_utf8 = [0xff_u8, 0];
            assert_eq!(zinc_open(invalid_utf8.as_ptr().cast(), &mut handle), -22);
            assert!(handle.is_null());
        }
    }

    #[test]
    fn ffi_try_wait() {
        unsafe {
            let mut handle = ptr::null_mut();
            assert_eq!(
                zinc_create(c"test_cabi_try_wait".as_ptr(), page_size(), &mut handle),
                0
            );
            assert_eq!(zinc_try_wait(handle), -11);
            zinc_notify(handle);
            assert_eq!(zinc_try_wait(handle), 0);
            assert_eq!(zinc_try_wait(handle), -11);
            zinc_close(handle);
        }
    }
}
