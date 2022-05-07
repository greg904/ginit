use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::env;
use std::ffi::CStr;
use std::fs;
use std::io;
use std::mem::MaybeUninit;
use std::net::Ipv4Addr;
use std::path::Path;
use std::ptr;
use std::str;

use serde::Deserialize;

/// Configuration of a network interface.
#[derive(Deserialize)]
struct NetInterfaceConfig {
    index: usize,
    addr: Option<String>,
    gateway: Option<String>,
    broadcast: Option<String>,
}

/// Configuration of the network.
#[derive(Deserialize)]
struct NetConfig {
    interfaces: Vec<NetInterfaceConfig>,
}

/// Configuration of the user interface that starts automatically on startup.
#[derive(Deserialize)]
struct UiConfig {
    user: String,
    env: HashMap<String, String>,
}

/// Filesystem to mount at boot.
#[derive(Deserialize)]
struct Mount {
    dir: String,
    device: String,
    fs_type: String,

    #[serde(default)]
    flags: u64,

    #[serde(default)]
    data: Option<String>,

    #[serde(default)]
    mkdir: Option<u32>,

    #[serde(default)]
    early: bool,
}

/// Build time configuration of the init system.
#[derive(Deserialize)]
struct Config {
    net: NetConfig,
    ui: UiConfig,
    mounts: Vec<Mount>,
}

impl Config {
    fn read() -> Config {
        let bytes = fs::read("config.toml").unwrap();
        let s = str::from_utf8(&bytes).unwrap();
        toml::from_str(s).unwrap()
    }
}

/// Wraps a return value from a `libc` function into an `io::Result`.
fn libc_check_error<T: From<i8> + PartialEq>(ret: T) -> io::Result<T> {
    if ret == T::from(-1) {
        return Err(io::Error::last_os_error());
    }
    Ok(ret)
}

fn getgrouplist(name: &str, group: libc::gid_t) -> Vec<libc::gid_t> {
    let mut num: libc::c_int = 0;
    unsafe {
        libc::getgrouplist(
            name.as_ptr() as *const i8,
            group,
            ptr::null_mut(),
            &mut num as *mut libc::c_int,
        )
    };
    let mut groups = vec![0; usize::try_from(num).unwrap()];
    libc_check_error(unsafe {
        libc::getgrouplist(
            name.as_ptr() as *const i8,
            group,
            groups.as_mut_ptr(),
            &mut num as *mut libc::c_int,
        )
    })
    .unwrap();
    groups
}

/// A system `/etc/passwd` entry for a user.
struct Passwd {
    _name: String,
    _passwd: String,
    uid: libc::uid_t,
    gid: libc::gid_t,
    _gecos: String,
    dir: String,
    _shell: String,
}

impl Passwd {
    fn get_from_username(name: &str) -> Self {
        let mut pwd: MaybeUninit<libc::passwd> = MaybeUninit::uninit();
        let mut result: *mut libc::passwd = ptr::null_mut();
        let cap =
            usize::try_from(unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) }).unwrap_or(16384);
        let mut buf = vec![0i8; cap];
        unsafe {
            libc::getpwnam_r(
                name.as_ptr() as *const i8,
                pwd.as_mut_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
                &mut result as *mut *mut libc::passwd,
            )
        };
        if result.is_null() {
            panic!("getpwnam_r() failed: {}", io::Error::last_os_error());
        }
        let pwd = unsafe { pwd.assume_init() };
        Self {
            _name: unsafe { CStr::from_ptr(pwd.pw_name) }
                .to_str()
                .unwrap()
                .to_owned(),
            _passwd: unsafe { CStr::from_ptr(pwd.pw_passwd) }
                .to_str()
                .unwrap()
                .to_owned(),
            uid: pwd.pw_uid,
            gid: pwd.pw_gid,
            _gecos: unsafe { CStr::from_ptr(pwd.pw_gecos) }
                .to_str()
                .unwrap()
                .to_owned(),
            dir: unsafe { CStr::from_ptr(pwd.pw_dir) }
                .to_str()
                .unwrap()
                .to_owned(),
            _shell: unsafe { CStr::from_ptr(pwd.pw_shell) }
                .to_str()
                .unwrap()
                .to_owned(),
        }
    }
}

/// Parses a quoted string like `he'llo' "wo\"l'"d` into `hello worl'd`, as a
/// shell would do.
fn unquote(s: &str) -> String {
    let mut res = String::new();
    let mut quote = '\0';
    let mut escaping = false;
    for c in s.chars() {
        if !escaping {
            if c == '\\' {
                escaping = true;
                continue;
            } else if quote == '\0' && (c == '\'' || c == '"') {
                quote = c;
                continue;
            } else if c == quote {
                quote = '\0';
                continue;
            }
        } else {
            escaping = false;
        }
        res.push(c);
    }
    assert!(!escaping, "backslash is not followed by character");
    assert_eq!(quote, '\0', "unfinished quoting");
    res
}

fn get_profile_env() -> HashMap<String, String> {
    fs::read_to_string("/etc/profile.env")
        .unwrap()
        .lines()
        .filter_map(|l| {
            const PREFIX: &str = "export ";
            if !l.starts_with(PREFIX) {
                return None;
            }
            let mut parts = l[PREFIX.len()..].splitn(2, '=');
            let key = parts.next()?.to_string();
            let val = unquote(parts.next()?);
            Some((key, val))
        })
        .collect()
}

fn format_addr(s: Option<&str>) -> Cow<str> {
    s.map(|val| {
        let addr: Ipv4Addr = val.parse().unwrap();
        let octets = addr.octets();
        Cow::Owned(format!(
            "Some(u32::from_be_bytes([{}, {}, {}, {}]))",
            octets[0], octets[1], octets[2], octets[3]
        ))
    })
    .unwrap_or(Cow::Borrowed("None"))
}

fn format_mount_function<'a, I: Iterator<Item = &'a Mount>>(fn_name: &str, mounts: I) -> String {
    let body = mounts
        .map(|m| {
            let mkdir = match m.mkdir {
                Some(mode) => format!("        ret = linux::mkdir(b\"{}\\0\" as *const u8, {mode});\n        if ret < 0 {{\n            return ret;\n        }}\n", m.dir),
                None => "".to_owned(),
            };
            let data = match &m.data {
                Some(d) => format!("b\"{}\\0\" as *const u8", d),
                None => "ptr::null()".to_owned(),
            };
            format!("{mkdir}        ret = linux::mount(b\"{}\\0\" as *const u8, b\"{}\\0\" as *const u8, b\"{}\\0\" as *const u8, {}, {});\n        if ret < 0 {{\n            return ret;\n        }}\n", m.device, m.dir, m.fs_type, m.flags, data)
        })
        .collect::<Vec<String>>()
        .concat();
    format!(
        "pub fn {fn_name}() -> i32 {{
    #[allow(unused)]
    let mut ret;
    unsafe {{
{body}    }}
    0
}}"
    )
}

fn main() {
    let profile_env = get_profile_env();
    let system_path = profile_env.get("ROOTPATH").unwrap();
    let cfg = Config::read();

    let net_interfaces_str = cfg
        .net
        .interfaces
        .iter()
        .map(|i| {
            let addr = format_addr(i.addr.as_deref());
            let gateway = format_addr(i.gateway.as_deref());
            let broadcast = format_addr(i.broadcast.as_deref());
            format!(
                "    NetInterface {{
        index: {index},
        addr: {addr},
        gateway: {gateway},
        broadcast: {broadcast},
    }},\n",
                index = i.index,
                addr = addr,
                gateway = gateway,
                broadcast = broadcast
            )
        })
        .collect::<Vec<String>>()
        .concat();

    let passwd = Passwd::get_from_username(&cfg.ui.user);
    let user_groups = getgrouplist(&cfg.ui.user, passwd.gid);
    let user_groups_str = user_groups
        .iter()
        .map(|g| g.to_string())
        .collect::<Vec<String>>()
        .join(", ");

    let xdg_runtime_dir = format!("/run/{}", passwd.uid);
    let mut sway_env: Vec<(&str, &str)> = profile_env
        .iter()
        .filter(|(k, _)| k != &"ROOTPATH")
        .chain(cfg.ui.env.iter())
        .map(|(k, v)| (&**k, &**v))
        .collect();
    sway_env.push(("HOME", &passwd.dir));
    sway_env.push(("MOZ_ENABLE_WAYLAND", "1"));
    sway_env.push(("LIBSEAT_BACKEND", "builtin"));
    sway_env.push(("SEATD_VTBOUND", "0"));
    sway_env.push(("QT_QPA_PLATFORM", "wayland"));
    sway_env.push(("WLR_DRM_DEVICES", "/dev/dri/card0"));
    sway_env.push(("WLR_LIBINPUT_NO_DEVICES", "1"));
    sway_env.push(("XDG_RUNTIME_DIR", &xdg_runtime_dir));
    sway_env.push(("XDG_SEAT", "seat0"));
    sway_env.push(("XDG_SESSION_DESKTOP", "sway"));
    sway_env.push(("XDG_SESSION_TYPE", "wayland"));
    sway_env.push(("_JAVA_AWT_WM_NONREPARENTING", "1"));
    sway_env.sort();
    let sway_envp_str = sway_env
        .iter()
        .map(|(k, v)| format!("    b\"{k}={v}\\0\" as *const u8,\n"))
        .collect::<Vec<String>>()
        .concat();

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_cfg_file = Path::new(&out_dir).join("config.rs");
    fs::write(
        out_cfg_file,
        format!(
            "// This file was generated by the `build.rs` script. You should edit the
// `config.toml` file instead of editing this file directly.

pub const SYSTEM_PATH: *const u8 = b\"PATH={system_path}\\0\" as *const u8;

pub const NET_INTERFACES: [NetInterface; {net_interfaces_len}] = [
{net_interfaces_str}];

pub const USER_HOME: *const u8 = b\"{user_home}\\0\" as *const u8;
pub const USER_UID: u32 = {user_uid};
pub const USER_GID: u32 = {user_gid};
pub const USER_GROUPS: [u32; {user_groups_len}] = [{user_groups_str}];

pub const SWAY_ENVP: *const *const u8 = &[
{sway_envp_str}] as *const *const u8;

pub const XDG_RUNTIME_DIR: *const u8 = b\"{xdg_runtime_dir}\\0\" as *const u8;

{mount_early}

{mount_late}
",
            net_interfaces_len = cfg.net.interfaces.len(),
            user_home = passwd.dir,
            user_uid = passwd.uid,
            user_gid = passwd.gid,
            user_groups_len = user_groups.len(),
            mount_early =
                format_mount_function("mount_early", cfg.mounts.iter().filter(|m| m.early)),
            mount_late =
                format_mount_function("mount_late", cfg.mounts.iter().filter(|m| !m.early)),
        ),
    )
    .unwrap();
}
