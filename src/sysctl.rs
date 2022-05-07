//! `sysctl` options and code to apply them. Sysctl is Linux specific and
//! information about it can be found on the net.
use std::{fs::OpenOptions, io::Write, path::Path};

/// Opens the file at the given `path` and writes the given `content` to it.
///
/// `write` is used instead of `write_all` so this should only be used for
/// special files like sysfs configuration files.
///
/// Errors are printed to stderr unlike most other functions. This is because
/// there can be multiple non critical errors that happen and will still want
/// to continue.
fn open_and_write<P: AsRef<Path>>(path: P, content: &[u8]) {
    let mut file = match OpenOptions::new().write(true).open(path.as_ref()) {
        Ok(val) => val,
        Err(err) => {
            eprintln!("failed to open {:?}: {:?}", path.as_ref(), err);
            return;
        }
    };
    if let Err(err) = file.write(content) {
        eprintln!("failed to write to {:?}: {:?}", path.as_ref(), err);
    }
}

/// Change sysctl options.
///
/// Errors are printed to stderr unlike most other functions. This is because
/// there can be multiple non critical errors that happen and will still want
/// to continue.
pub fn apply_sysctl() {
    open_and_write("/proc/sys/fs/protected_fifos", b"1");
    open_and_write("/proc/sys/fs/protected_hardlinks", b"1");
    open_and_write("/proc/sys/fs/protected_regular", b"1");
    open_and_write("/proc/sys/fs/protected_symlinks", b"1");
    open_and_write("/proc/sys/net/ipv4/tcp_mtu_probing", b"1");
    open_and_write("/proc/sys/vm/admin_reserve_kbytes", b"0");
    open_and_write("/proc/sys/vm/dirty_background_ratio", b"75");
    open_and_write("/proc/sys/vm/dirty_expire_centisecs", b"90000");
    open_and_write("/proc/sys/vm/dirty_ratio", b"75");
    open_and_write("/proc/sys/vm/dirty_writeback_centisecs", b"90000");
    open_and_write("/proc/sys/vm/overcommit_memory", b"2");
    open_and_write("/proc/sys/vm/overcommit_ratio", b"100");
    open_and_write("/proc/sys/vm/stat_interval", b"10");
    open_and_write("/proc/sys/vm/user_reserve_kbytes", b"0");
}
