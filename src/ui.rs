//! This module contains functions related to the set up of the graphical user
//! interface.

use core::fmt::Write;
use core::ptr;

use crate::config;
use crate::linux;

fn udev_trigger_add_action(action_type: *const u8) -> bool {
    let argv = [
        b"/bin/udevadm\0" as *const u8,
        b"trigger\0" as *const u8,
        b"--type\0" as *const u8,
        action_type,
        b"--action\0" as *const u8,
        b"add\0" as *const u8,
        ptr::null(),
    ];
    let ret = unsafe {
        linux::spawn_and_wait(
            b"/bin/udevadm\0" as *const u8,
            &argv as *const *const u8,
            &[config::SYSTEM_PATH, ptr::null()] as *const *const u8,
        )
    };
    match ret {
        Ok(status) if status != 0 => false,
        Err(_) => false,
        _ => true,
    }
}

/// Starts the udev deamon, configure all devices and wait for the end of the
/// configuration.
fn start_udev() -> bool {
    let ret = unsafe {
        linux::spawn(
            b"/lib/systemd/systemd-udevd\0" as *const u8,
            &[b"/lib/systemd/systemd-udevd\0" as *const u8, ptr::null()] as *const *const u8,
            &[config::SYSTEM_PATH, ptr::null()] as *const *const u8,
        )
    };
    if ret < 0 {
        writeln!(linux::Stderr, "failed to start udev: {ret}").unwrap();
        return false;
    }
    if !udev_trigger_add_action(b"subsystems\0" as *const u8) {
        writeln!(linux::Stderr, "failed to add udev subsystems").unwrap();
    }
    if !udev_trigger_add_action(b"devices\0" as *const u8) {
        writeln!(linux::Stderr, "failed to add udev devices").unwrap();
    }
    true
}

/// Creates the XDG_RUNTIME_DIR directory.
fn create_xdg_runtime_dir() -> i32 {
    let ret = unsafe { linux::mkdir(config::XDG_RUNTIME_DIR, 0o700) };
    if ret < 0 {
        return ret;
    }
    unsafe { linux::chown(config::XDG_RUNTIME_DIR, config::USER_UID, config::USER_GID) }
}

fn ui_process_pre_exec(_data: usize) -> bool {
    let mut ret = linux::setgid(config::USER_GID);
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

/// Starts the user interface process and returns a handle to it so that the
/// caller can wait until it dies.
pub fn start_ui_process() -> i32 {
    if !start_udev() {
        return -linux::EINVAL;
    }

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
            0,
        )
    }
}
