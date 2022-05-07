//! `sysctl` options and code to apply them. Sysctl is Linux specific and
//! information about it can be found on the net.
use crate::linux;
use core::convert::TryInto;

/// Opens the file at the given `path` and writes the given `content` to it.
///
/// `write` is used instead of `write_all` so this should only be used for
/// special files like sysfs configuration files.
///
/// Errors are printed to stderr unlike most other functions. This is because
/// there can be multiple non critical errors that happen and will still want
/// to continue.
unsafe fn open_and_write(path: *const u8, content: &[u8]) {
    let fd = linux::open(path, 0, 0);
    if fd < 0
        || linux::write(
            fd.try_into().unwrap(),
            content.as_ptr(),
            content.len().try_into().unwrap(),
        ) < 0
    {
        // TODO: Print an error message.
        return;
    }
    linux::close(fd.try_into().unwrap());
}

/// Change sysctl options.
///
/// Errors are not returned unlike most other functions. This is because there
/// can be multiple non critical errors that happen and will still want to
/// continue.
pub fn apply_sysctl() {
    unsafe {
        open_and_write(b"/proc/sys/fs/protected_fifos\0" as *const u8, b"1");
        open_and_write(b"/proc/sys/fs/protected_hardlinks\0" as *const u8, b"1");
        open_and_write(b"/proc/sys/fs/protected_regular\0" as *const u8, b"1");
        open_and_write(b"/proc/sys/fs/protected_symlinks\0" as *const u8, b"1");
        open_and_write(b"/proc/sys/net/ipv4/tcp_mtu_probing\0" as *const u8, b"1");
        open_and_write(b"/proc/sys/vm/admin_reserve_kbytes\0" as *const u8, b"0");
        open_and_write(b"/proc/sys/vm/dirty_background_ratio\0" as *const u8, b"75");
        open_and_write(
            b"/proc/sys/vm/dirty_expire_centisecs\0" as *const u8,
            b"90000",
        );
        open_and_write(b"/proc/sys/vm/dirty_ratio\0" as *const u8, b"75");
        open_and_write(
            b"/proc/sys/vm/dirty_writeback_centisecs\0" as *const u8,
            b"90000",
        );
        open_and_write(b"/proc/sys/vm/overcommit_memory\0" as *const u8, b"2");
        open_and_write(b"/proc/sys/vm/overcommit_ratio\0" as *const u8, b"100");
        open_and_write(b"/proc/sys/vm/stat_interval\0" as *const u8, b"10");
        open_and_write(b"/proc/sys/vm/user_reserve_kbytes\0" as *const u8, b"0");
    }
}
