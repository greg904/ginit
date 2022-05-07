//! This module contains the entry point of the init program. For more
//! information about this program, read the `README.md` file at the root of
//! the project.
#![no_main]
#![no_std]
#![feature(lang_items)]

use core::convert::TryInto;
use core::fmt::Write;
use core::{panic::PanicInfo, ptr};

pub mod config;
pub mod linux;
pub mod mounts;
pub mod net;
pub mod shutdown;
pub mod sysctl;
pub mod ui;

fn background_init() {
    sysctl::apply_sysctl();

    let mut ret = config::mount_late();
    if ret < 0 {
        writeln!(linux::Stderr, "failed to mount late FS: {ret}").unwrap();
    }

    ret = net::setup_networking();
    if ret < 0 {
        writeln!(linux::Stderr, "failed to setup networking: {ret}").unwrap();
    }
}

fn redirect_stdout() {
    let fd = unsafe {
        linux::open(
            b"/var/log/boot\0" as *const u8,
            linux::O_WRONLY | linux::O_CREAT | linux::O_TRUNC,
            0o600,
        )
    };
    if fd < 0 {
        return;
    }
    let fd = linux::Fd(fd.try_into().unwrap());
    linux::dup2(fd.0, 1);
    linux::dup2(fd.0, 2);
}

fn unsafe_main() {
    redirect_stdout();

    writeln!(linux::Stdout, "booting...").unwrap();

    let mut ret = config::mount_early();
    if ret < 0 {
        writeln!(linux::Stderr, "failed to mount early FS: {ret}").unwrap();
    }

    ret = unsafe { linux::symlink(b"/proc/self/fd\0" as *const u8, b"/dev/fd\0" as *const u8) };
    if ret < 0 {
        writeln!(linux::Stderr, "failed to symlink /dev/fd: {ret}").unwrap();
    }
    ret = unsafe {
        linux::symlink(
            b"/proc/self/fd/0\0" as *const u8,
            b"/dev/stdin\0" as *const u8,
        )
    };
    if ret < 0 {
        writeln!(linux::Stderr, "failed to symlink /dev/stdin: {ret}").unwrap();
    }
    ret = unsafe {
        linux::symlink(
            b"/proc/self/fd/1\0" as *const u8,
            b"/dev/stdout\0" as *const u8,
        )
    };
    if ret < 0 {
        writeln!(linux::Stderr, "failed to symlink /dev/stdout: {ret}").unwrap();
    }
    ret = unsafe {
        linux::symlink(
            b"/proc/self/fd/2\0" as *const u8,
            b"/dev/stderr\0" as *const u8,
        )
    };
    if ret < 0 {
        writeln!(linux::Stderr, "failed to symlink /dev/stderr: {ret}").unwrap();
    }

    /*
    // We'll let the initialization happen in the background so no need to
    // store the handle here.
    thread::spawn(background_init);
    */
    background_init();

    let ui_child_pid = ui::start_ui_process();
    if ui_child_pid < 0 {
        writeln!(linux::Stderr, "failed to start UI process: {ui_child_pid}").unwrap();
        return;
    }

    loop {
        // Reap zombie processes.
        let pid = unsafe { linux::wait4(-1, ptr::null_mut(), 0, ptr::null_mut()) };
        if pid < 0 {
            writeln!(linux::Stderr, "failed to wait for process: {pid}").unwrap();
            return;
        }
        if pid == ui_child_pid {
            writeln!(linux::Stdout, "UI process died").unwrap();
            // Consider the system stopped when the UI process dies.
            break;
        }
    }
}

/*
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
*/

/// Shuts down the system while making sure that no progress will be lost.
fn graceful_shutdown() {
    writeln!(linux::Stdout, "shutting down...").unwrap();

    // write_kernel_log();

    // Start writing data to disk so that there is less to write when the
    // processes are killed.
    linux::sync();

    shutdown::end_all_processes();
    shutdown::unmount_all();
    shutdown::power_off();
}

#[no_mangle]
extern "C" fn _start() -> ! {
    /*
    // The actual main code is wrapped to make sure that we sync and shutdown
    // gracefully in every case.
    if let Err(err) = std::panic::catch_unwind(unsafe_main) {
        eprintln!("panic from unsafe_main: {:?}", err);
    }
    */
    unsafe_main();

    graceful_shutdown();

    linux::exit(0);
}

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    loop {}
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
