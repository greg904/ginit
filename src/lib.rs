#![feature(setgroups)]

pub mod config;
pub mod shutdown;
pub mod sysctl;
pub mod ui;

use std::{ffi::CString, io, ptr};

/// Wraps a return value from a `libc` function into an `io::Result`.
pub fn libc_check_error(ret: libc::c_int) -> io::Result<libc::c_int> {
    if ret == -1 {
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
    libc_check_error(unsafe {
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
