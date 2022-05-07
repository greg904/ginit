//! This module contains functions related to the set up of the graphical user
//! interface.

use std::borrow::Cow;
use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::process::CommandExt;
use std::process::Child;
use std::process::Command;

use crate::{config, libc_check_error};

fn udev_trigger_add_action(ty: &str) -> io::Result<()> {
    let mut cmd = Command::new("/sbin/udevadm")
        .args(&["trigger", "--type", ty, "--action", "add"])
        .env("PATH", config::EXEC_PATH)
        .spawn()?;
    cmd.wait().and_then(|status| {
        if status.success() {
            Ok(())
        } else {
            let code: Cow<str> = status
                .code()
                .map(|code| code.to_string().into())
                .unwrap_or("(none)".into());
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("bad exit code: {}", code),
            ))
        }
    })
}

/// Starts the udev deamon, configure all devices and wait for the end of the
/// configuration.
///
/// Non critical errors are printed to stderr.
fn start_udev() -> io::Result<()> {
    Command::new("/sbin/udevd")
        .env("PATH", config::EXEC_PATH)
        .spawn()?;
    if let Err(err) = udev_trigger_add_action("subsystems") {
        eprintln!("failed to add all subsystems to udev: {:?}", err);
    }
    if let Err(err) = udev_trigger_add_action("devices") {
        eprintln!("failed to add all devices to udev: {:?}", err);
    }
    Ok(())
}

/// Creates the XDG_RUNTIME_DIR directory.
///
/// Non critical errors are printed to stderr.
fn create_xdg_runtime_dir() -> io::Result<()> {
    DirBuilder::new()
        .mode(0o700)
        .create("/run/xdg-runtime-dir")?;
    libc_check_error(unsafe {
        libc::chown(
            b"/run/xdg-runtime-dir\0".as_ptr() as *const libc::c_char,
            config::USER_UID,
            config::USER_GID,
        )
    })?;
    Ok(())
}

/// Starts the user interface process and returns a handle to it so that the
/// caller can wait until it dies.
///
/// Non critical errors are printed to stderr.
pub fn start_ui_process() -> io::Result<Child> {
    start_udev()?;
    create_xdg_runtime_dir()?;

    Command::new("/usr/bin/dbus-run-session")
        .uid(config::USER_UID)
        .gid(config::USER_GID)
        .groups(config::USER_GROUPS)
        .current_dir(config::USER_HOME)
        .arg("/usr/bin/sway")
        .env("HOME", config::USER_HOME)
        .env("MOZ_ENABLE_WAYLAND", "1")
        .env("PATH", config::EXEC_PATH)
        .env("LIBSEAT_BACKEND", "builtin")
        .env("SEATD_VTBOUND", "0")
        .env("QT_QPA_PLATFORM", "wayland")
        .env("WLR_DRM_DEVICES", "/dev/dri/card0")
        .env("WLR_LIBINPUT_NO_DEVICES", "1")
        .env("XDG_RUNTIME_DIR", "/run/xdg-runtime-dir")
        .env("XDG_SEAT", "seat0")
        .env("XDG_SESSION_DESKTOP", "sway")
        .env("XDG_SESSION_TYPE", "wayland")
        .env("_JAVA_AWT_WM_NONREPARENTING", "1")
        .spawn()
}
