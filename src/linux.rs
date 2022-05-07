use core::arch::asm;

pub const AF_NETLINK: i32 = 16;

pub const ESRCH: i32 = 3;
pub const EINTR: i32 = 4;
pub const ECHILD: i32 = 10;
pub const ENOMEM: i32 = 12;

pub const LINUX_REBOOT_MAGIC1: i32 = 0xfee1deadu32 as i32;
pub const LINUX_REBOOT_MAGIC2: i32 = 672274793;

pub const MS_NOSUID: u64 = 2;
pub const MS_NODEV: u64 = 4;
pub const MS_NOEXEC: u64 = 8;
pub const MS_NOATIME: u64 = 1024;

pub const NETLINK_ROUTE: i32 = 0;

pub const O_RDONLY: u32 = 0;
pub const O_WRONLY: u32 = 1;

pub const RB_POWER_OFF: u32 = 0x4321FEDC;

pub const SIGTERM: i32 = 15;

pub const SOCK_RAW: i32 = 3;

unsafe fn syscall_0(num: u64) -> i64 {
    let ret;
    asm!(
        "syscall",
        in("rax") num,
        lateout("rax") ret,
    );
    ret
}

unsafe fn syscall_1(num: u64, arg1: u64) -> i64 {
    let ret;
    asm!(
        "syscall",
        in("rax") num,
        in("rdi") arg1,
        lateout("rax") ret,
    );
    ret
}

unsafe fn syscall_2(num: u64, arg1: u64, arg2: u64) -> i64 {
    let ret;
    asm!(
        "syscall",
        in("rax") num,
        in("rdi") arg1,
        in("rsi") arg2,
        lateout("rax") ret,
    );
    ret
}

unsafe fn syscall_3(num: u64, arg1: u64, arg2: u64, arg3: u64) -> i64 {
    let ret;
    asm!(
        "syscall",
        in("rax") num,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        lateout("rax") ret,
    );
    ret
}

unsafe fn syscall_4(num: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64) -> i64 {
    let ret;
    asm!(
        "syscall",
        in("rax") num,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        in("r10") arg4,
        lateout("rax") ret,
    );
    ret
}

unsafe fn syscall_5(num: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i64 {
    let ret;
    asm!(
        "syscall",
        in("rax") num,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        in("r10") arg4,
        in("r8") arg5,
        lateout("rax") ret,
    );
    ret
}

pub unsafe fn read(fd: u32, buf: *mut u8, count: usize) -> i64 {
    syscall_3(0, fd.into(), buf as u64, count as u64)
}

pub unsafe fn write(fd: u32, buf: *const u8, count: usize) -> i64 {
    syscall_3(1, fd.into(), buf as u64, count as u64)
}

pub unsafe fn open(filename: *const u8, flags: u32, mode: u32) -> i32 {
    syscall_3(2, filename as u64, flags.into(), mode.into()) as i32
}

pub fn close(fd: u32) -> i32 {
    unsafe { syscall_1(3, fd.into()) as i32 }
}

pub fn socket(family: i32, sock_type: i32, protocol: i32) -> i32 {
    unsafe { syscall_3(41, family as u64, sock_type as u64, protocol as u64) as i32 }
}

pub unsafe fn wait4(pid: i32, status: *mut i32, options: i32, rusage: *mut u8) -> i32 {
    syscall_4(61, pid as u64, status as u64, options as u64, rusage as u64) as i32
}

pub fn kill(pid: i32, signal: i32) -> i32 {
    unsafe { syscall_2(64, pid as u64, signal as u64) as i32 }
}

pub unsafe fn mkdir(pathname: *const u8, mode: u32) -> i32 {
    syscall_2(83, pathname as u64, mode.into()) as i32
}

pub unsafe fn symlink(old_name: *const u8, new_name: *const u8) -> i32 {
    syscall_2(88, old_name as u64, new_name as u64) as i32
}

pub unsafe fn mount(
    dev_name: *const u8,
    dir_name: *const u8,
    fs_type: *const u8,
    flags: u64,
    data: *const u8,
) -> i32 {
    syscall_5(
        165,
        dev_name as u64,
        dir_name as u64,
        fs_type as u64,
        flags,
        data as u64,
    ) as i32
}

pub fn sync() {
    unsafe { syscall_0(162) };
}

pub unsafe fn umount(name: *const u8, flags: i32) -> i32 {
    syscall_2(166, name as u64, flags as u64) as i32
}

pub unsafe fn reboot(magic1: i32, magic2: i32, cmd: u32, arg: *const u8) -> i32 {
    syscall_4(169, magic1 as u64, magic2 as u64, cmd as u64, arg as u64) as i32
}

pub struct Fd(pub u32);

impl Drop for Fd {
    fn drop(&mut self) {
        if close(self.0) < 0 {
            // TODO: Print an error.
        }
    }
}
