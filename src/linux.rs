use core::arch::asm;

pub const MS_NOSUID: u64 = 2;
pub const MS_NODEV: u64 = 4;
pub const MS_NOEXEC: u64 = 8;
pub const MS_NOATIME: u64 = 1024;

unsafe fn syscall_2(num: u64, arg1: u64, arg2: u64) -> i64 {
    let ret;
    asm!(
        "syscall",
        in("rax") num,
        in("rdi") arg1,
        in("rsi") arg2,
        lateout("rax") ret
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
        lateout("rax") ret
    );
    ret
}

pub unsafe fn mkdir(pathname: *const u8, mode: u32) -> i64 {
    syscall_2(83, pathname as u64, mode.into())
}

pub unsafe fn mount(
    dev_name: *const u8,
    dir_name: *const u8,
    fs_type: *const u8,
    flags: u64,
    data: *const u8,
) -> i64 {
    syscall_5(
        165,
        dev_name as u64,
        dir_name as u64,
        fs_type as u64,
        flags,
        data as u64,
    )
}
