#![feature(setgroups)]

use std::{
    convert::TryFrom, ffi::CStr, fs::DirBuilder, io, os::unix::fs::DirBuilderExt, ptr, thread,
};

pub(crate) mod config;
pub(crate) mod kernel_opts;
pub(crate) mod net;
pub(crate) mod shutdown;
pub(crate) mod ui;

pub(crate) fn libc_check_err(ret: libc::c_int) -> io::Result<libc::c_int> {
    if ret == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(ret)
}

fn mount(
    src: &[u8],
    dest: &[u8],
    ty: &[u8],
    flags: libc::c_ulong,
    data: Option<&[u8]>,
) -> io::Result<()> {
    let src = CStr::from_bytes_with_nul(src).unwrap().as_ptr();
    let dest = CStr::from_bytes_with_nul(dest).unwrap().as_ptr();
    let ty = CStr::from_bytes_with_nul(ty).unwrap().as_ptr();
    let data = data
        .map(|s| CStr::from_bytes_with_nul(s).unwrap().as_ptr() as *const libc::c_void)
        .unwrap_or(ptr::null());
    libc_check_err(unsafe { libc::mount(src, dest, ty, flags, data) }).map(|_ret| ())
}

fn mount_early() -> io::Result<()> {
    const TMPFS_FLAGS: libc::c_ulong =
        libc::MS_NOATIME | libc::MS_NODEV | libc::MS_NOEXEC | libc::MS_NOSUID;
    mount(
        b"none\0",
        b"/dev\0",
        b"devtmpfs\0",
        libc::MS_NOATIME | libc::MS_NOEXEC | libc::MS_NOSUID,
        None,
    )?;
    DirBuilder::new().mode(0o1744).create("/dev/shm")?;
    mount(b"none\0", b"/dev/shm\0", b"tmpfs\0", TMPFS_FLAGS, None)?;
    DirBuilder::new().mode(0o744).create("/dev/pts")?;
    mount(
        b"none\0",
        b"/dev/pts\0",
        b"devpts\0",
        libc::MS_NOATIME | libc::MS_NOEXEC | libc::MS_NOSUID,
        None,
    )?;
    mount(b"none\0", b"/tmp\0", b"tmpfs\0", TMPFS_FLAGS, None)?;
    mount(b"none\0", b"/run\0", b"tmpfs\0", TMPFS_FLAGS, None)?;
    mount(b"none\0", b"/proc\0", b"proc\0", 0, None)?;
    mount(b"none\0", b"/sys\0", b"sysfs\0", 0, None)?;
    mount(
        b"/dev/nvme0n1p2\0",
        b"/bubble\0",
        b"btrfs\0",
        libc::MS_NOATIME | libc::MS_NODEV,
        Some(b"subvol=/@bubble,commit=900\0"),
    )?;
    Ok(())
}

fn background_init() {
    if let Err(err) = kernel_opts::set_kernel_opts() {
        eprintln!("failed to set a kernel option: {:?}", err);
    }
    if let Err(err) = net::setup_networking() {
        eprintln!("failed to setup networking: {:?}", err);
    }
    if let Err(err) = mount(
        b"/dev/nvme0n1p1\0",
        b"/boot\0",
        b"vfat\0",
        libc::MS_NOATIME,
        Some(b"umask=0077\0"),
    ) {
        eprintln!("failed to mount /boot: {:?}", err);
    }
}

fn unsafe_main() {
    if let Err(err) = mount_early() {
        eprintln!("failed to mount early FS: {:?}", err);
    }

    // We'll let the initialization happen in the background so no need to
    // store the handle here.
    thread::spawn(background_init);

    let ui_child = match ui::start_ui() {
        Ok(val) => val,
        Err(err) => {
            eprintln!("failed to start UI process {:?}", err);
            return;
        }
    };
    let ui_child_pid = i32::try_from(ui_child.id()).unwrap();

    loop {
        // Reap zombie processes.
        let pid = match libc_check_err(unsafe { libc::wait(ptr::null_mut()) }) {
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

fn main() {
    // The actual main code is wrapped to make sure that we sync and shutdown
    // gracefully in every case.
    if let Err(err) = std::panic::catch_unwind(unsafe_main) {
        eprintln!("panic from unsafe_main: {:?}", err);
    }

    // Start writing data to disk so that there is less to write when the
    // processes are killed.
    unsafe { libc::sync() };

    shutdown::kill_all_processes();
    shutdown::unmount_all();
    unsafe { libc::reboot(libc::RB_POWER_OFF) };
}
