//! This module contains the entry point of the init program. For more
//! information about this program, read the `README` file at the root of
//! the project.
#![no_main]
#![no_std]
#![feature(lang_items)]

use core::convert::{TryFrom, TryInto};
use core::fmt::Write;
use core::mem;
use core::{panic::PanicInfo, ptr};

pub mod config;
pub mod linux;
pub mod mounts;
pub mod net;
pub mod seat;
pub mod shutdown;
pub mod sysctl;
pub mod ui;

fn late_init() {
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
    let fd = match u32::try_from(fd) {
        Ok(n) => n,
        Err(_) => return,
    };
    if fd != 1 {
        linux::dup2(fd, 1);
    }
    if fd != 2 {
        linux::dup2(fd, 2);
    }
    if fd != 1 && fd != 2 {
        linux::close(fd);
    }
}

fn dmesg_pre_exec(fd: usize) -> bool {
    // Output into the FD.
    let ret = linux::dup2(fd.try_into().unwrap(), 1);
    if ret < 0 {
        writeln!(linux::Stderr, "failed to dup log FD for dmesg: {ret}").unwrap();
        return false;
    }
    true
}

fn write_kernel_log() {
    let fd = unsafe {
        linux::open(
            b"/var/log/dmesg\0" as *const u8,
            linux::O_CREAT | linux::O_WRONLY | linux::O_TRUNC,
            0o600,
        )
    };
    let fd = match u32::try_from(fd) {
        Ok(n) => linux::Fd(n),
        Err(_) => {
            writeln!(linux::Stderr, "failed to open /var/log/dmesg: {fd}").unwrap();
            return;
        }
    };
    let ret = unsafe {
        linux::spawn_and_wait_with_pre_exec(
            b"/bin/dmesg\0" as *const u8,
            &[b"/bin/dmesg\0" as *const u8, ptr::null()] as *const *const u8,
            &[ptr::null()] as *const *const u8,
            dmesg_pre_exec,
            fd.0.try_into().unwrap(),
        )
    };
    match ret {
        Ok(code) => {
            if code != 0 {
                writeln!(linux::Stderr, "dmesg exited with code {code}").unwrap();
            }
        }
        Err(e) => writeln!(linux::Stderr, "failed to spawn dmesg: {e}").unwrap(),
    }
}

/// Shuts down the system while making sure that no progress will be lost.
fn graceful_shutdown() {
    writeln!(linux::Stdout, "shutting down...").unwrap();

    write_kernel_log();

    // Start writing data to disk so that there is less to write when the
    // processes are killed.
    linux::sync();

    shutdown::end_all_processes();
    shutdown::unmount_all();
    shutdown::power_off();
}

fn create_dev_symlinks() {
    let mut ret =
        unsafe { linux::symlink(b"/proc/self/fd\0" as *const u8, b"/dev/fd\0" as *const u8) };
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
}

fn add_dri_render_permissions() {
    let ret = unsafe {
        linux::chown(
            b"/dev/dri/renderD128\0" as *const u8,
            config::USER_UID,
            config::USER_GID,
        )
    };
    if ret < 0 {
        writeln!(linux::Stderr, "failed to chown /dev/dri/renderD128: {ret}").unwrap();
    }
}

fn run_event_loop() {
    let mask = linux::sigset_t::try_from(1 << (linux::SIGCHLD - 1)).unwrap();

    let ret = linux::rt_sigprocmask(
        linux::SIG_BLOCK,
        &mask,
        ptr::null_mut(),
        mem::size_of_val(&mask),
    );
    if ret < 0 {
        writeln!(linux::Stderr, "failed to block SIGCHLD: {ret}").unwrap();
    }

    let signalfd = linux::signalfd4(-1, mask, linux::SFD_CLOEXEC | linux::SFD_NONBLOCK);
    if signalfd < 0 {
        writeln!(
            linux::Stderr,
            "failed to create SIGCHLD signalfd: {signalfd}"
        )
        .unwrap();
        return;
    }
    let signalfd = linux::Fd(signalfd.try_into().unwrap());

    let (mut seat_server, seat_compositor_fd) = match seat::SeatServer::new() {
        Ok(t) => t,
        Err(err) => {
            writeln!(linux::Stderr, "failed to create seat server: {err}").unwrap();
            return;
        }
    };

    let ui_child_pid = ui::start_ui_process(seat_compositor_fd.0);
    if ui_child_pid < 0 {
        writeln!(linux::Stderr, "failed to start UI process: {ui_child_pid}").unwrap();
        return;
    }

    late_init();

    loop {
        let mut fds = [
            linux::pollfd {
                fd: i32::try_from(signalfd.0).unwrap(),
                events: linux::POLLIN,
                revents: 0,
            },
            linux::pollfd {
                fd: i32::try_from(seat_server.fd()).unwrap(),
                events: linux::POLLIN,
                revents: 0,
            },
        ];
        let ret = linux::poll(&mut fds, 500);
        if ret < 0 {
            writeln!(linux::Stderr, "failed to poll: {ret}").unwrap();
            break;
        }
        if fds[0].revents & (linux::POLLERR | linux::POLLNVAL) != 0 {
            writeln!(
                linux::Stderr,
                "poll returned error on SIGCHLD signalfd: {}",
                fds[0].revents
            )
            .unwrap();
            break;
        }
        if fds[1].revents & (linux::POLLERR | linux::POLLNVAL) != 0 {
            writeln!(
                linux::Stderr,
                "poll returned error on seat server socket: {}",
                fds[1].revents
            )
            .unwrap();
            break;
        }

        if fds[0].revents & linux::POLLIN != 0 {
            // Drain the signalfd before we reap processes to mark the signals as handled by the
            // kernel so that it doesn't wake up until a new one arrives. If poll was
            // edge-triggered, we would not need to do that, but here we need to do it because it
            // is level-triggered.
            loop {
                let mut buf = [0u8; 128];
                let ret = linux::read(signalfd.0, &mut buf);
                if ret == -i64::from(linux::EAGAIN) {
                    break;
                } else if ret < 0 {
                    writeln!(linux::Stderr, "failed to read from signalfd: {ret}").unwrap();
                    break;
                }
            }

            // Reap zombie processes.
            let mut status: i32 = 0;
            loop {
                let pid = unsafe {
                    linux::wait4(-1, &mut status as *mut i32, linux::WNOHANG, ptr::null_mut())
                };
                if pid < 0 {
                    writeln!(linux::Stderr, "failed to wait for process: {pid}").unwrap();
                    break;
                } else if pid == 0 {
                    break;
                } else if pid == ui_child_pid {
                    writeln!(linux::Stdout, "UI process died: {status}").unwrap();
                    // Consider the system stopped when the UI process dies.
                    return;
                }
            }
        }

        if fds[1].revents & linux::POLLIN != 0 {
            if let Err(err) = seat_server.process_incoming() {
                writeln!(
                    linux::Stderr,
                    "failed to process seat server request: {err}"
                )
                .unwrap();
                return;
            }
        }
    }
}

#[no_mangle]
extern "C" fn _start() -> ! {
    redirect_stdout();

    writeln!(linux::Stdout, "booting...").unwrap();

    let ret = config::mount_early();
    if ret < 0 {
        writeln!(linux::Stderr, "failed to mount early FS: {ret}").unwrap();
    }

    create_dev_symlinks();
    add_dri_render_permissions();

    run_event_loop();

    graceful_shutdown();

    // We should not get here.
    linux::exit(0);
}

#[panic_handler]
fn panic(panic: &PanicInfo<'_>) -> ! {
    let _ = writeln!(linux::Stderr, "{}", panic);
    // Make sure the message is visible in the log file.
    linux::sync();
    loop {}
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
