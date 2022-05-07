//! Powering off the system gracefully is not an easy task. This module provides
//! routines to help.
use core::ptr;
use core::fmt::Write;

use crate::{linux, mounts};

/// Tell processes to exit and wait for them to do so.
///
/// Errors are ignored unlike most other functions. This is because there can
/// be multiple non critical errors that happen and will still want to
/// continue.
pub fn end_all_processes() {
    // A pid of -1 is used to broadcast the SIGTERM signal to all processes.
    let ret = linux::kill(-1, linux::SIGTERM);
    if ret < 0 {
        if ret != -linux::ESRCH {
            writeln!(linux::Stderr, "failed to broadcast SIGTERM: {ret}").unwrap();
            // If we get an error here, don't wait for processes to exit
            // because they don't know that they have to...
        }
        return;
    }

    loop {
        // `wait4` will collect the exit status of any process.
        let ret = unsafe { linux::wait4(-1, ptr::null_mut(), 0, ptr::null_mut()) };
        if ret == -linux::ECHILD {
            // There are no processes left.
            break;
        } else if ret == -linux::EINTR {
            // The function was interrupted by a signal.
            continue;
        } else if ret < 0 {
            writeln!(linux::Stderr, "failed to kill processes: {ret}").unwrap();
            // This should not happen. If it does, then we better break now
            // because if we don't we might be stuck in the loop with the
            // same error over and over again.
            break;
        }
    }
}

static mut MOUNTS: [u8; 256] = [0; 256];

/// Unmounts all filesystems known to the init process.
///
/// Errors are printed to stderr unlike most other functions. This is because
/// there can be multiple non critical errors that happen and will still want
/// to continue.
pub unsafe fn unmount_all() {
    let n = mounts::read_mounts(&mut MOUNTS);
    if n < 0 {
        writeln!(linux::Stderr, "failed to read mounts: {n}").unwrap();
        return;
    }

    // We cannot unmount a tree in which there is another mount (for instance,
    // we cannot unmount /dev before /dev/pts). As the mounts file is ordered
    // by mount order, we have to iterate it in reverse order.
    let mut end = n as usize;
    while end >= 2 {
        // Find string start.
        let mut start = end - 2;
        loop {
            if MOUNTS[start] == b'\0' {
                start += 1;
                break;
            } else if start == 0 {
                break;
            }
            start -= 1;
        }

        let m = &MOUNTS[start..end];
        let ret = linux::umount(m.as_ptr(), 0);
        if ret < -1 {
            writeln!(linux::Stderr, "failed unmount FS: {ret}").unwrap();
        }

        end = start;
    }
}

/// Actually powers off the system.
///
/// This is not safe to do before having unmounted all filesystems.
pub fn power_off() {
    unsafe {
        linux::reboot(
            linux::LINUX_REBOOT_MAGIC1,
            linux::LINUX_REBOOT_MAGIC2,
            linux::RB_POWER_OFF,
            ptr::null(),
        )
    };
}
