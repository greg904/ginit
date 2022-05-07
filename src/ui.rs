//! This module contains functions related to the set up of the graphical user
//! interface.

use core::ptr;
use core::fmt::Write;

use crate::config;
use crate::linux;

fn udev_trigger_add_action(action_type: *const u8) -> bool {
    let ret = unsafe {
        linux::spawn_and_wait(
            b"/bin/udevadm\0" as *const u8,
            &[
                b"/bin/udevadm\0" as *const u8,
                b"trigger\0" as *const u8,
                b"--type\0" as *const u8,
                action_type,
                b"--action\0" as *const u8,
                b"add\0" as *const u8,
                ptr::null(),
            ] as *const *const u8,
            &[config::SYSTEM_PATH, ptr::null()] as *const *const u8,
            || true,
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
            || true,
        )
    };
    if ret < 0 {
        writeln!(linux::Stderr, "failed to start udev: {ret}").unwrap();
        return false;
    }
    if udev_trigger_add_action(b"subsystems\0" as *const u8) {
        writeln!(linux::Stderr, "failed to add udev subsystems: {ret}").unwrap();
    }
    if udev_trigger_add_action(b"devices\0" as *const u8) {
        writeln!(linux::Stderr, "failed to add udev devices: {ret}").unwrap();
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
        linux::spawn(
            b"/usr/bin/sway\0" as *const u8,
            &[b"/usr/bin/sway\0" as *const u8, ptr::null()] as *const *const u8,
            config::SWAY_ENVP,
            || {
                linux::setgid(config::USER_GID) >= 0
                    && linux::setgroups(&config::USER_GROUPS) >= 0
                    && linux::setuid(config::USER_UID) >= 0
                    && linux::chdir(config::USER_HOME) >= 0
            },
        )
    }
}
