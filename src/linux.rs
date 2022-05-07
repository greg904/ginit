use core::arch::asm;
use core::fmt::Write;
use core::{fmt, ptr};

pub const AF_UNSPEC: i32 = 0;
pub const AF_INET: i32 = 2;
pub const AF_NETLINK: i32 = 16;

pub const ARPHRD_NONE: u16 = 0xFFFE;

pub const CLONE_VM: u64 = 0x100;
pub const CLONE_VFORK: u64 = 0x4000;

pub const ESRCH: i32 = 3;
pub const EINTR: i32 = 4;
pub const ECHILD: i32 = 10;
pub const ENOMEM: i32 = 12;
pub const EINVAL: i32 = 22;

pub const IFA_ADDRESS: u16 = 1;
pub const IFA_LOCAL: u16 = 2;
pub const IFA_BROADCAST: u16 = 4;

pub const IFF_UP: i32 = 0x1;

pub const LINUX_REBOOT_MAGIC1: i32 = 0xfee1deadu32 as i32;
pub const LINUX_REBOOT_MAGIC2: i32 = 672274793;

pub const MS_NOSUID: u64 = 2;
pub const MS_NODEV: u64 = 4;
pub const MS_NOEXEC: u64 = 8;
pub const MS_NOATIME: u64 = 1024;

pub const NETLINK_ROUTE: i32 = 0;

pub const NLMSG_ERROR: i32 = 0x2;

pub const NLM_F_REQUEST: i32 = 1;
pub const NLM_F_ACK: i32 = 4;
pub const NLM_F_EXCL: i32 = 0x200;
pub const NLM_F_CREATE: i32 = 0x400;

pub const O_RDONLY: u32 = 0o0;
pub const O_WRONLY: u32 = 0o1;
pub const O_CREAT: u32 = 0o100;
pub const O_TRUNC: u32 = 0o1000;

pub const RB_POWER_OFF: u32 = 0x4321FEDC;

pub const RTA_OIF: u16 = 4;
pub const RTA_GATEWAY: u16 = 5;

pub const RTM_SETLINK: u16 = 19;
pub const RTM_NEWADDR: u16 = 20;
pub const RTM_NEWROUTE: u16 = 24;

pub const RTN_UNICAST: u8 = 1;

pub const RTPROT_BOOT: u8 = 3;

pub const RT_SCOPE_UNIVERSE: u8 = 0;

pub const RT_TABLE_MAIN: u8 = 254;

pub const SIGTERM: i32 = 15;
pub const SIGCHLD: i32 = 17;

pub const SOCK_RAW: i32 = 3;

#[repr(C)]
pub struct nlmsgerr {
    pub error: i32,
    pub msg: nlmsghdr,
}

#[repr(C)]
pub struct nlmsghdr {
    pub nlmsg_len: u32,
    pub nlmsg_type: u16,
    pub nlmsg_flags: u16,
    pub nlmsg_seq: u32,
    pub nlmsg_pid: u32,
}

unsafe fn syscall_0(num: u64) -> i64 {
    let ret;
    asm!(
        "syscall",
        in("rax") num,
        lateout("rax") ret,
        out("rcx") _,
        out("r11") _,
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
        out("rcx") _,
        out("r11") _,
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
        out("rcx") _,
        out("r11") _,
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
        out("rcx") _,
        out("r11") _,
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
        out("rcx") _,
        out("r11") _,
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
        out("rcx") _,
        out("r11") _,
    );
    ret
}

pub fn read(fd: u32, buf: &mut [u8]) -> i64 {
    unsafe { syscall_3(0, fd.into(), buf.as_mut_ptr() as u64, buf.len() as u64) }
}

pub fn write(fd: u32, buf: &[u8]) -> i64 {
    unsafe { syscall_3(1, fd.into(), buf.as_ptr() as u64, buf.len() as u64) }
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn open(filename: *const u8, flags: u32, mode: u32) -> i32 {
    syscall_3(2, filename as u64, flags.into(), mode.into()) as i32
}

pub fn close(fd: u32) -> i32 {
    unsafe { syscall_1(3, fd.into()) as i32 }
}

pub fn dup2(old_fd: u32, new_fd: u32) -> i32 {
    unsafe { syscall_2(33, old_fd.into(), new_fd.into()) as i32 }
}

pub fn socket(family: i32, sock_type: i32, protocol: i32) -> i32 {
    unsafe { syscall_3(41, family as u64, sock_type as u64, protocol as u64) as i32 }
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn clone(
    flags: u64,
    sp: *mut u8,
    tid_parent: *mut i32,
    tid_child: *mut i32,
    tls: *mut u8,
    f: unsafe fn(data: usize),
    arg: usize,
) -> i32 {
    let ret;
    asm!(
        "syscall",
        // If we are the new thread...
        "test rax, rax",
        "jnz 1f",
        // Mark the new outer frame.
        "xor ebp, ebp",
        // Call `f`.
        "mov rdi, r12",
        "call r9",
        "1:",
        // Registers we use in the new thread.
        in("r9") f,
        in("r12") arg,
        // System call parameters.
        in("rax") 56,
        in("rdi") flags,
        in("rsi") sp,
        in("rdx") tid_parent,
        in("r10") tid_child,
        in("r8") tls,
        lateout("rax") ret,
        out("rcx") _,
        out("r11") _,
    );
    ret
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn execve(filename: *const u8, argv: *const *const u8, envp: *const *const u8) -> i32 {
    syscall_3(59, filename as u64, argv as u64, envp as u64) as i32
}

pub fn exit(code: i32) -> ! {
    unsafe { syscall_1(60, code as u64) };
    unreachable!();
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn wait4(pid: i32, status: *mut i32, options: i32, rusage: *mut u8) -> i32 {
    syscall_4(61, pid as u64, status as u64, options as u64, rusage as u64) as i32
}

pub fn kill(pid: i32, signal: i32) -> i32 {
    unsafe { syscall_2(62, pid as u64, signal as u64) as i32 }
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn chdir(filename: *const u8) -> i32 {
    syscall_1(80, filename as u64) as i32
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn mkdir(pathname: *const u8, mode: u32) -> i32 {
    syscall_2(83, pathname as u64, mode.into()) as i32
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn symlink(old_name: *const u8, new_name: *const u8) -> i32 {
    syscall_2(88, old_name as u64, new_name as u64) as i32
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn chown(filename: *const u8, uid: u32, gid: u32) -> i32 {
    syscall_3(92, filename as u64, uid as u64, gid as u64) as i32
}

pub fn setuid(uid: u32) -> i32 {
    unsafe { syscall_1(105, uid as u64) as i32 }
}

pub fn setgid(gid: u32) -> i32 {
    unsafe { syscall_1(106, gid as u64) as i32 }
}

pub fn setgroups(groups: &[u32]) -> i32 {
    unsafe { syscall_2(116, groups.len() as u64, groups.as_ptr() as u64) as i32 }
}

pub fn sync() {
    unsafe { syscall_0(162) };
}

#[allow(clippy::missing_safety_doc)]
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

#[allow(clippy::missing_safety_doc)]
pub unsafe fn umount(name: *const u8, flags: i32) -> i32 {
    syscall_2(166, name as u64, flags as u64) as i32
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn reboot(magic1: i32, magic2: i32, cmd: u32, arg: *const u8) -> i32 {
    syscall_4(169, magic1 as u64, magic2 as u64, cmd as u64, arg as u64) as i32
}

pub struct Fd(pub u32);

impl Drop for Fd {
    fn drop(&mut self) {
        let ret = close(self.0);
        if ret < 0 {
            writeln!(Stderr, "failed to close FD: {ret}").unwrap();
        }
    }
}

pub struct Stdout;

impl fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if write(1, s.as_bytes()) < 0 {
            return Err(fmt::Error);
        }
        Ok(())
    }
}

pub struct Stderr;

impl fmt::Write for Stderr {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if write(2, s.as_bytes()) < 0 {
            return Err(fmt::Error);
        }
        Ok(())
    }
}

struct SpawnHelperData {
    filename: *const u8,
    argv: *const *const u8,
    envp: *const *const u8,
    pre_exec: unsafe fn(data: usize) -> bool,
    pre_exec_data: usize,
}

unsafe fn spawn_helper(arg: usize) {
    let arg = &*(arg as *const SpawnHelperData);
    if (arg.pre_exec)(arg.pre_exec_data) {
        let ret = execve(arg.filename, arg.argv, arg.envp);
        if ret < 0 {
            // Do not panic.
            let _ = writeln!(Stderr, "failed to execve: {ret}");
        }
    }
    exit(1);
}

/// Spawns a new process and returns its PID. The `pre_exec` function is called with the
/// `pre_exec_data` argument before `execve` is called. This allows the caller to change the
/// environment for the new process.
///
/// # Safety
///
/// `filename` must be a NUL-terminated string.
/// `argv` must be an array of NUL-terminated strings, with a null pointer at the end.
/// `envp` must be an array of NUL-terminated strings, with a null pointer at the end.
/// `pre_exec` must not introduce UB.
pub unsafe fn spawn_with_pre_exec(
    filename: *const u8,
    argv: *const *const u8,
    envp: *const *const u8,
    pre_exec: unsafe fn(data: usize) -> bool,
    pre_exec_data: usize,
) -> i32 {
    let mut stack = [0u8; 512];
    // The stack grows downwards.
    let mut sp = stack.as_mut_ptr().add(stack.len());
    // The stack must be 16-byte aligned.
    sp = (sp as usize & !0xf) as *mut u8;
    let data = SpawnHelperData {
        filename,
        argv,
        envp,
        pre_exec,
        pre_exec_data,
    };
    clone(
        CLONE_VM | CLONE_VFORK | SIGCHLD as u64,
        sp,
        ptr::null_mut(),
        ptr::null_mut(),
        ptr::null_mut(),
        spawn_helper,
        &data as *const _ as usize,
    )
}

fn dummy_pre_exec(_data: usize) -> bool {
    true
}

/// Spawns a new process and returns its PID.
///
/// # Safety
///
/// `filename` must be a NUL-terminated string.
/// `argv` must be an array of NUL-terminated strings, with a null pointer at the end.
/// `envp` must be an array of NUL-terminated strings, with a null pointer at the end.
pub unsafe fn spawn(filename: *const u8, argv: *const *const u8, envp: *const *const u8) -> i32 {
    spawn_with_pre_exec(filename, argv, envp, dummy_pre_exec, 0)
}

/// Spawns a new process, waits for it to die and returns its status code. The `pre_exec` function
/// is called with the `pre_exec_data` argument before `execve` is called. This allows the caller
/// to change the environment for the new process.
///
/// # Safety
///
/// `filename` must be a NUL-terminated string.
/// `argv` must be an array of NUL-terminated strings, with a null pointer at the end.
/// `envp` must be an array of NUL-terminated strings, with a null pointer at the end.
/// `pre_exec` must not introduce UB.
pub unsafe fn spawn_and_wait_with_pre_exec(
    filename: *const u8,
    argv: *const *const u8,
    envp: *const *const u8,
    pre_exec: unsafe fn(data: usize) -> bool,
    pre_exec_data: usize,
) -> Result<i32, i32> {
    let pid = spawn_with_pre_exec(filename, argv, envp, pre_exec, pre_exec_data);
    if pid < 0 {
        return Err(pid);
    }
    let mut status = 0;
    let ret = wait4(pid, &mut status as *mut i32, 0, ptr::null_mut());
    if ret < 0 {
        return Err(ret);
    }
    Ok(status)
}

/// Spawns a new process, waits for it to die and returns its status code.
///
/// # Safety
///
/// `filename` must be a NUL-terminated string.
/// `argv` must be an array of NUL-terminated strings, with a null pointer at the end.
/// `envp` must be an array of NUL-terminated strings, with a null pointer at the end.
pub unsafe fn spawn_and_wait(
    filename: *const u8,
    argv: *const *const u8,
    envp: *const *const u8,
) -> Result<i32, i32> {
    spawn_and_wait_with_pre_exec(filename, argv, envp, dummy_pre_exec, 0)
}
