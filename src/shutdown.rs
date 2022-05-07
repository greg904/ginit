//! Powering off the system gracefully is not an easy task. This module provides
//! routines to help.

use std::{ffi::CString, fs::File, io::Read, ptr};

use crate::libc_wrapper;

/// Tell processes to exit and wait for them to do so.
///
/// Errors are printed to stderr unlike most other functions. This is because
/// there can be multiple non critical errors that happen and will still want
/// to continue.
pub fn end_all_processes() {
    // A pid of -1 is used to broadcast the SIGTERM signal to all processes.
    if let Err(err) = libc_wrapper::check_error_int(unsafe { libc::kill(-1, libc::SIGTERM) }) {
        let no_process_found = err
            .raw_os_error()
            .map(|code| code == libc::ESRCH)
            .unwrap_or(false);
        if !no_process_found {
            eprintln!("failed to broadcast SIGTERM: {:?}", err);
            // If we get an error here, don't wait for processes to exit
            // because they don't know that they have to...
        }
        return;
    }

    loop {
        // `libc::wait` will collect the exit status of any process.
        if let Err(err) = libc_wrapper::check_error_int(unsafe { libc::wait(ptr::null_mut()) }) {
            match err.raw_os_error() {
                // There are no processes left.
                Some(libc::ECHILD) => break,
                // The function was interrupted by a signal.
                Some(libc::EINTR) => continue,
                _ => {
                    eprintln!("failed to wait for processes to exit: {:?}", err);
                    // This should not happen. If it does, then we better break now
                    // because if we don't we might be stuck in the loop with the
                    // same error over and over again.
                    break;
                }
            }
        }
    }
}

/// Unmounts all filesystems known to the init process.
///
/// Errors are printed to stderr unlike most other functions. This is because
/// there can be multiple non critical errors that happen and will still want
/// to continue.
pub fn unmount_all() {
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
    // we cannot unmount /dev before /dev/pts). As the /proc/self/mounts file
    // is ordered by mount order, we have to iterate it in reverse order.
    for line in lines.lines().rev() {
        let mountpoint = match line.split_ascii_whitespace().nth(5) {
            Some(val) => val,
            None => {
                eprintln!("invalid /proc/self/mounts format");
                return;
            }
        };
        let mountpoint_cstr = CString::new(mountpoint).unwrap();
        if let Err(err) =
            libc_wrapper::check_error_int(unsafe { libc::umount(mountpoint_cstr.as_ptr()) })
        {
            eprintln!("failed to unmount {}: {:?}", mountpoint, err);
        }
    }
}

/// Actually powers off the system.
///
/// This is not safe to do before having unmounted all filesystems.
pub fn power_off() {
    unsafe { libc::reboot(libc::RB_POWER_OFF) };
}
