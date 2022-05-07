//! `sysctl` options and code to apply them. Sysctl is Linux specific and
//! information about it can be found on the net.
use libc::{c_char, c_void};

macro_rules! lit_cstr {
    ($s:literal) => {
        (concat!($s, "\0").as_bytes().as_ptr() as *const c_char)
    };
}

/// Prints `<function>(<argument>): <current errno description>` to stderr.
///
/// # Safety
///
/// Both `function` and `argument` must be null-terminated.
unsafe fn perror_function_argument(function: *const c_char, argument: *const c_char) {
    let mut buf = [0 as c_char; 64];
    let ret = {
        libc::snprintf(
            buf.as_mut_ptr(),
            buf.len(),
            lit_cstr!("%s(%s)"),
            function,
            argument,
        )
    };
    if ret >= 0 {
        {
            libc::perror(buf.as_mut_ptr())
        };
    }
}

/// Opens the file at the given `path` and writes the given `content` to it.
///
/// Errors are printed to stderr unlike most other functions. This is because
/// there can be multiple non critical errors that happen and will still want
/// to continue.
///
/// # Safety
///
/// `path` must be null-terminated.
unsafe fn open_and_write(path: *const c_char, content: &[u8]) {
    let fd = libc::open(path, libc::O_WRONLY | libc::O_CLOEXEC);
    if fd == -1 {
        perror_function_argument(lit_cstr!("open"), path);
        return;
    }
    let ret = libc::write(fd, content.as_ptr() as *const c_void, content.len());
    if ret == -1 {
        perror_function_argument(lit_cstr!("write"), path);
    }
    let ret = libc::close(fd);
    if ret == -1 {
        perror_function_argument(lit_cstr!("close"), path);
    }
}

/// Change sysctl options to the ones I want.
///
/// Errors are printed to stderr unlike most other functions. This is because
/// there can be multiple non critical errors that happen and will still want
/// to continue.
pub fn apply_sysctl() {
    unsafe {
        open_and_write(lit_cstr!("/proc/sys/fs/protected_fifos"), b"1");
        open_and_write(lit_cstr!("/proc/sys/fs/protected_hardlinks"), b"1");
        open_and_write(lit_cstr!("/proc/sys/fs/protected_regular"), b"1");
        open_and_write(lit_cstr!("/proc/sys/fs/protected_symlinks"), b"1");
        open_and_write(lit_cstr!("/proc/sys/net/ipv4/tcp_mtu_probing"), b"1");
        open_and_write(lit_cstr!("/proc/sys/vm/admin_reserve_kbytes"), b"0");
        open_and_write(lit_cstr!("/proc/sys/vm/dirty_background_ratio"), b"75");
        open_and_write(lit_cstr!("/proc/sys/vm/dirty_expire_centisecs"), b"90000");
        open_and_write(lit_cstr!("/proc/sys/vm/dirty_ratio"), b"75");
        open_and_write(
            lit_cstr!("/proc/sys/vm/dirty_writeback_centisecs"),
            b"90000",
        );
        open_and_write(lit_cstr!("/proc/sys/vm/overcommit_memory"), b"2");
        open_and_write(lit_cstr!("/proc/sys/vm/overcommit_ratio"), b"100");
        open_and_write(lit_cstr!("/proc/sys/vm/stat_interval"), b"10");
        open_and_write(lit_cstr!("/proc/sys/vm/user_reserve_kbytes"), b"0");
    }
}
