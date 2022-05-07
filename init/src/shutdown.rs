use std::{ffi::CString, fs::File, io::Read, ptr};

use crate::libc_check_err;

/// Tell processes to exit and wait for them to do so. Errors are printed to
/// stderr instead of being returned.
pub(crate) fn kill_all_processes() {
    if let Err(err) = libc_check_err(unsafe { libc::kill(-1, libc::SIGTERM) }) {
        eprintln!("failed to broadcast SIGTERM: {:?}", err);
    }

    loop {
        if let Err(err) = libc_check_err(unsafe { libc::wait(ptr::null_mut()) }) {
            let no_child_left = err
                .raw_os_error()
                .map(|code| code == libc::ECHILD)
                .unwrap_or(false);
            if !no_child_left {
                eprintln!("failed to wait for processes to exit: {:?}", err);
            }
            break;
        }
    }
}

/// Tries to unmount all filesystem known to the init process. Errors are
/// printed to stderr instead of being returned.
pub(crate) fn unmount_all() {
    let lines = {
        let mut file = match File::open("/proc/self/mounts") {
            Ok(val) => val,
            Err(err) => {
                eprintln!("failed to open /proc/self/mounts: {:?}", err);
                return;
            }
        };
        let mut lines = String::new();
        if let Err(err) = file.read_to_string(&mut lines) {
            eprintln!("failed to read /proc/self/mounts: {:?}", err);
            return;
        }
        lines
    };

    // We cannot unmount a tree in which there is another mount (for instance,
    // we cannot unmount /dev before /dev/pts is unmounted), so we have to
    // iterate in reverse order compared to the mount order.
    for line in lines.lines().rev() {
        let mountpoint = match line.split_ascii_whitespace().nth(5) {
            Some(val) => val,
            None => {
                eprintln!("invalid /proc/self/mounts data");
                return;
            }
        };
        let mountpoint_cstr = CString::new(mountpoint).unwrap();
        if let Err(err) = libc_check_err(unsafe { libc::umount(mountpoint_cstr.as_ptr()) }) {
            eprintln!("failed to unmount {}: {:?}", mountpoint, err);
        }
    }
}
