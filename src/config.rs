//! Machine specific configuration is stored here. This means that it cannot be
//! changed during runtime but has the benefit that we don't have to do any
//! parsing at runtime which is easier and faster.

use std::net::Ipv4Addr;

/// The index of the `lo` interface.
pub const LO_INDEX: i32 = 1;
/// The index of the `eth0` interface.
pub const ETH0_INDEX: i32 = 2;
pub const ETH0_ADDR: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 26);
pub const ETH0_GATEWAY: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 254);
pub const ETH0_BROADCAST: Ipv4Addr = Ipv4Addr::new(255, 255, 255, 0);

pub const USER_HOME: &'static str = "/home/greg";
pub const USER_UID: u32 = 1000;
pub const USER_GID: u32 = 1000;
pub const USER_GROUPS: &'static [u32] = &[1000, 10, 18, 27, 78, 97, 272];

/// This is what is set as the PATH environment variable.
pub const EXEC_PATH: &'static str =
    "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/opt/bin:/usr/lib/llvm/12/bin";
