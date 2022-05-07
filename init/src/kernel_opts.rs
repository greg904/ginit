use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::Path,
};

fn open_write<P: AsRef<Path>>(path: P, content: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).open(path)?;
    file.write(content)?;
    Ok(())
}

pub(crate) fn set_kernel_opts() -> io::Result<()> {
    open_write("/proc/sys/fs/protected_fifos", b"1")?;
    open_write("/proc/sys/fs/protected_hardlinks", b"1")?;
    open_write("/proc/sys/fs/protected_regular", b"1")?;
    open_write("/proc/sys/fs/protected_symlinks", b"1")?;
    open_write("/proc/sys/vm/admin_reserve_kbytes", b"0")?;
    open_write("/proc/sys/vm/dirty_background_ratio", b"75")?;
    open_write("/proc/sys/vm/dirty_expire_centisecs", b"90000")?;
    open_write("/proc/sys/vm/dirty_ratio", b"75")?;
    open_write("/proc/sys/vm/dirty_writeback_centisecs", b"90000")?;
    open_write("/proc/sys/vm/overcommit_memory", b"2")?;
    open_write("/proc/sys/vm/overcommit_ratio", b"100")?;
    open_write("/proc/sys/vm/stat_interval", b"10")?;
    open_write("/proc/sys/vm/user_reserve_kbytes", b"0")?;
    open_write("/sys/class/backlight/nv_backlight/brightness", b"80")?;
    open_write(
        "/sys/class/power_supply/BAT0/charge_control_end_threshold",
        b"80",
    )?;
    Ok(())
}
