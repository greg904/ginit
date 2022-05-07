//! Machine specific configuration is stored here. This means that it cannot be
//! changed during runtime but has the benefit that we don't have to do any
//! parsing at runtime which is easier and faster.

use crate::linux;
use crate::net::Ipv4Addr;
use core::ptr;

pub struct NetInterface {
    pub index: u32,
    pub addr: Option<Ipv4Addr>,
    pub gateway: Option<Ipv4Addr>,
    pub broadcast: Option<Ipv4Addr>,
}

include!(concat!(env!("OUT_DIR"), "/config.rs"));
