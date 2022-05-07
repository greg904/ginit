//! Collection of useful functions to interface with the standard C library.

use std::{ffi::CString, io, ptr};

/// Wraps a return value from a `libc` function into an `io::Result`.
pub fn check_error<T: From<i8> + PartialEq>(ret: T) -> io::Result<T> {
    if ret == T::from(-1) {
        return Err(io::Error::last_os_error());
    }
    Ok(ret)
}

/// A safe wrapper for `libc::mount`.
pub fn mount(
    src: &str,
    dest: &str,
    ty: &str,
    flags: libc::c_ulong,
    data: Option<&str>,
) -> io::Result<()> {
    let src = CString::new(src).unwrap();
    let dest = CString::new(dest).unwrap();
    let ty = CString::new(ty).unwrap();
    let data = data.map(|val| CString::new(val).unwrap());
    check_error(unsafe {
        libc::mount(
            src.as_ptr(),
            dest.as_ptr(),
            ty.as_ptr(),
            flags,
            data.as_ref()
                .map(|s| s.as_ptr() as *const libc::c_void)
                .unwrap_or(ptr::null()),
        )
    })?;
    Ok(())
}