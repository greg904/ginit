//! This module contains functions related to the set up of the graphical user
//! interface.

use core::convert::TryFrom;
use core::fmt::Write;
use core::ptr;

use crate::config;
use crate::linux;

const SEAT_COMPOSITOR_FD: u32 = 3;

/// Creates the XDG_RUNTIME_DIR directory.
fn create_xdg_runtime_dir() -> i32 {
    let ret = unsafe { linux::mkdir(config::XDG_RUNTIME_DIR, 0o700) };
    if ret < 0 {
        return ret;
    }
    unsafe { linux::chown(config::XDG_RUNTIME_DIR, config::USER_UID, config::USER_GID) }
}

fn ui_process_pre_exec(seat_compositor_fd: usize) -> bool {
    let mut ret;
    let seat_compositor_fd = u32::try_from(seat_compositor_fd).unwrap();
    if seat_compositor_fd != SEAT_COMPOSITOR_FD {
        ret = linux::dup2(seat_compositor_fd, SEAT_COMPOSITOR_FD);
        if ret < 0 {
            writeln!(linux::Stderr, "failed to dup2 seat compositor FD: {ret}").unwrap();
            return false;
        }
        ret = linux::close(seat_compositor_fd);
        if ret < 0 {
            writeln!(linux::Stderr, "failed to close seat compositor FD: {ret}").unwrap();
        }
    }
    ret = linux::setgid(config::USER_GID);
    if ret < 0 {
        writeln!(linux::Stderr, "failed to setgid: {ret}").unwrap();
        return false;
    }
    ret = linux::setgroups(config::USER_GROUPS);
    if ret < 0 {
        writeln!(linux::Stderr, "failed to setgroups: {ret}").unwrap();
        return false;
    }
    ret = linux::setuid(config::USER_UID);
    if ret < 0 {
        writeln!(linux::Stderr, "failed to setuid: {ret}").unwrap();
        return false;
    }
    ret = unsafe { linux::chdir(config::USER_HOME) };
    if ret < 0 {
        writeln!(linux::Stderr, "failed to chdir: {ret}").unwrap();
        return false;
    }
    true
}

/// Starts the user interface process and returns its PID so that the caller can wait until it
/// dies.
///
/// To understand what `seat_compositor_fd` refers to, please look at the documentation for the
/// `seat` module.
pub fn start_ui_process(seat_compositor_fd: u32) -> i32 {
    let ret = create_xdg_runtime_dir();
    if ret < 0 {
        return ret;
    }

    unsafe {
        linux::spawn_with_pre_exec(
            b"/usr/bin/sway\0" as *const u8,
            &[b"/usr/bin/sway\0" as *const u8, ptr::null()] as *const *const u8,
            config::SWAY_ENVP,
            ui_process_pre_exec,
            usize::try_from(seat_compositor_fd).unwrap(),
        )
    }
}
