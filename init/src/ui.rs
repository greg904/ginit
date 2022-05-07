use std::borrow::Cow;
use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::process::CommandExt;
use std::process::Child;
use std::process::Command;

use crate::{config, libc_check_err};

fn udev_trigger_add(ty: &str) -> io::Result<()> {
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

pub(crate) fn start_ui() -> io::Result<Child> {
    // Configure all devices and wait for the end of the configuration.
    Command::new("/sbin/udevd")
        .env("PATH", config::EXEC_PATH)
        .spawn()?;
    if let Err(err) = udev_trigger_add("subsystems") {
        eprintln!("failed to add all subsystems to udev: {:?}", err);
    }
    if let Err(err) = udev_trigger_add("devices") {
        eprintln!("failed to add all devices to udev: {:?}", err);
    }

    DirBuilder::new()
        .mode(0o700)
        .create("/run/xdg-runtime-dir")?;
    libc_check_err(unsafe {
        libc::chown(
            b"/run/xdg-runtime-dir\0".as_ptr() as *const libc::c_char,
            config::USER_UID,
            config::USER_GID,
        )
    })?;

    Command::new("/usr/bin/sway")
        .uid(config::USER_UID)
        .gid(config::USER_GID)
        .groups(config::USER_GROUPS)
        .current_dir(config::USER_HOME)
        .env("MOZ_ENABLE_WAYLAND", "1")
        .env("HOME", config::USER_HOME)
        .env("PATH", config::EXEC_PATH)
        .env("WLR_LIBINPUT_NO_DEVICES", "1")
        .env("WLR_SESSION", "direct")
        .env("XDG_RUNTIME_DIR", "/run/xdg-runtime-dir")
        .env("XDG_SEAT", "seat-main")
        .spawn()
}
