//! This module contains the entry point of the init program. For more
//! information about this program, read the `README.md` file at the root of
//! the project.
#![feature(setgroups)]

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::prelude::{IntoRawFd, OpenOptionsExt};
use std::process::Command;
use std::{convert::TryFrom, fs::DirBuilder, io, os::unix::fs::DirBuilderExt, ptr, thread};
use std::os::unix::fs;

pub mod config;
pub mod libc_wrapper;
pub mod net;
pub mod shutdown;
pub mod sysctl;
pub mod ui;

use libc_wrapper::mount;

fn mount_early() -> io::Result<()> {
    const TMPFS_FLAGS: libc::c_ulong =
        libc::MS_NOATIME | libc::MS_NODEV | libc::MS_NOEXEC | libc::MS_NOSUID;
    mount(
        "none",
        "/dev",
        "devtmpfs",
        libc::MS_NOATIME | libc::MS_NOEXEC | libc::MS_NOSUID,
        None,
    )?;
    DirBuilder::new().mode(0o1744).create("/dev/shm")?;
    mount("none", "/dev/shm", "tmpfs", TMPFS_FLAGS, None)?;
    DirBuilder::new().mode(0o744).create("/dev/pts")?;
    mount(
        "none",
        "/dev/pts",
        "devpts",
        libc::MS_NOATIME | libc::MS_NOEXEC | libc::MS_NOSUID,
        None,
    )?;
    mount("none", "/tmp", "tmpfs", TMPFS_FLAGS, None)?;
    mount("none", "/run", "tmpfs", TMPFS_FLAGS, None)?;
    mount("none", "/proc", "proc", 0, None)?;
    mount("none", "/sys", "sysfs", 0, None)?;
    mount(
        "/dev/nvme0n1p2",
        "/bubble",
        "btrfs",
        libc::MS_NOATIME | libc::MS_NODEV,
        Some("subvol=/@bubble,commit=900"),
    )?;
    Ok(())
}

fn background_init() {
    sysctl::apply_sysctl();
    if let Err(err) = mount(
        "/dev/nvme0n1p1",
        "/boot",
        "vfat",
        libc::MS_NOATIME,
        Some("umask=0077"),
    ) {
        eprintln!("failed to mount /boot: {:?}", err);
    }
    if let Err(err) = net::setup_networking() {
        eprintln!("failed to setup networking: {:?}", err);
    }
}

fn redirect_stdout() -> io::Result<()> {
    let fd = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open("/var/log/boot")?
        .into_raw_fd();
    libc_wrapper::check_error(unsafe { libc::dup2(fd, 1) })?;
    libc_wrapper::check_error(unsafe { libc::dup2(fd, 2) })?;
    Ok(())
}

fn unsafe_main() {
    if let Err(err) = redirect_stdout() {
        eprintln!("failed to redirect stdout: {:?}", err);
    }

    if let Err(err) = mount_early() {
        eprintln!("failed to mount early FS: {:?}", err);
    }

    if let Err(err) = fs::symlink("/proc/self/fd", "/dev/fd") {
        eprintln!("failed to create /dev/fd symlink: {:?}", err);
    }
    if let Err(err) = fs::symlink("/proc/self/fd/0", "/dev/stdin") {
        eprintln!("failed to create /dev/stdin symlink: {:?}", err);
    }
    if let Err(err) = fs::symlink("/proc/self/fd/1", "/dev/stdout") {
        eprintln!("failed to create /dev/stdout symlink: {:?}", err);
    }
    if let Err(err) = fs::symlink("/proc/self/fd/2", "/dev/stderr") {
        eprintln!("failed to create /dev/stderr symlink: {:?}", err);
    }

    // We'll let the initialization happen in the background so no need to
    // store the handle here.
    thread::spawn(background_init);

    let ui_child = match ui::start_ui_process() {
        Ok(val) => val,
        Err(err) => {
            eprintln!("failed to start UI process {:?}", err);
            return;
        }
    };
    let ui_child_pid = i32::try_from(ui_child.id()).unwrap();

    loop {
        // Reap zombie processes.
        let pid = match libc_wrapper::check_error(unsafe { libc::wait(ptr::null_mut()) }) {
            Ok(val) => val,
            Err(err) => {
                eprintln!("wait failed: {:?}", err);
                return;
            }
        };
        if pid == ui_child_pid {
            // Consider the system stopped when the UI process dies.
            break;
        }
    }
}

fn write_kernel_log() {
    match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open("/var/log/dmesg")
    {
        Ok(kern_log) => match Command::new("/bin/dmesg").stdout(kern_log).spawn() {
            Ok(mut cmd) => match cmd.wait() {
                Ok(status) => {
                    if !status.success() {
                        eprintln!("dmesg failed: {:?}", status);
                    }
                }
                Err(err) => eprintln!("failed to wait for dmesg: {:?}", err),
            },
            Err(err) => eprintln!("failed to spawn dmesg: {:?}", err),
        },
        Err(err) => eprintln!("failed to open kernel log file: {:?}", err),
    };
}

/// Shuts down the system while making sure that no progress will be lost.
fn graceful_shutdown() {
    write_kernel_log();

    if let Err(err) = io::stdout().flush() {
        eprintln!("failed to flush stdout: {:?}", err);
    }
    if let Err(err) = io::stderr().flush() {
        eprintln!("failed to flush stderr: {:?}", err);
    }

    // Start writing data to disk so that there is less to write when the
    // processes are killed.
    unsafe { libc::sync() };

    shutdown::end_all_processes();
    shutdown::unmount_all();
    shutdown::power_off();
}

fn main() {
    // The actual main code is wrapped to make sure that we sync and shutdown
    // gracefully in every case.
    if let Err(err) = std::panic::catch_unwind(unsafe_main) {
        eprintln!("panic from unsafe_main: {:?}", err);
    }

    graceful_shutdown();
}
