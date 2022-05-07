//! Networking related code is put in this module. The init system has to set
//! up the networking stack so that the user has access to the internet. On
//! Linux, this is done using the `rtnetlink` interface.

use core::convert::TryFrom;
use core::convert::TryInto;
use core::slice;
use core::{mem, ptr};

use crate::config;
use crate::linux;

pub type Ipv4Addr = u32;

/// A netlink socket FD with automatic cleanup and that keeps track of the
/// current sequence number for messages.
struct NetlinkSocket {
    fd: linux::Fd,
    seq: u32,
}

impl NetlinkSocket {
    /// Creates a new netlink route socket.
    ///
    /// `protocol` is used to tell the kernel what the socket will be used for.
    /// For instance, to listen to and modify networking configuration, use
    /// `linux::NETLINK_ROUTE`.
    fn new(protocol: i32) -> Result<NetlinkSocket, i32> {
        let fd = linux::socket(linux::AF_NETLINK, linux::SOCK_RAW, protocol);
        if fd < 0 {
            return Err(fd);
        }
        Ok(NetlinkSocket {
            fd: linux::Fd(fd.try_into().unwrap()),
            seq: 0,
        })
    }

    /// Returns and increments the next sequence number to use to send a
    /// message.
    fn next_seq(&mut self) -> u32 {
        let seq = self.seq;
        self.seq += 1;
        seq
    }

    /// Sends a message through the socket.
    fn send(&self, msg: &[u8]) -> i64 {
        unsafe { linux::write(self.fd.0, msg.as_ptr(), msg.len()) }
    }

    /// Receives a message from the socket.
    fn recv(&self, msg: &mut [u8]) -> i64 {
        unsafe { linux::read(self.fd.0, msg.as_mut_ptr(), msg.len()) }
    }

    /// Drains the socket until a `nmsgerr` message is available. That message
    /// is then read and depending on the error code inside of it, either a
    /// Ok or Err is returned.
    fn ack_error(&self) -> i32 {
        loop {
            let mut buf = [0u8; 8192];
            let len = self.recv(&mut buf);
            if len < 0 {
                return len.try_into().unwrap();
            }
            let len = usize::try_from(len).unwrap();

            let mut i = 0;
            loop {
                if i + mem::size_of::<linux::nlmsghdr>() > len {
                    break;
                }
                let hdr =
                    unsafe { ptr::read_unaligned(buf[i..].as_ptr() as *const linux::nlmsghdr) };
                if i32::from(hdr.nlmsg_type) == linux::NLMSG_ERROR {
                    let payload = unsafe {
                        ptr::read(buf[i + mem::size_of::<linux::nlmsghdr>()..].as_ptr()
                            as *const linux::nlmsgerr)
                    };
                    return match payload.error {
                        0 => 0,
                        err => err,
                    };
                }
                i += usize::try_from(hdr.nlmsg_len).unwrap();
            }
        }
    }
}

#[repr(C)]
struct ifaddrmsg {
    ifa_family: u8,
    ifa_prefixlen: u8,
    ifa_flags: u8,
    ifa_scope: u8,
    ifa_index: u32,
}

/// This is just the header for a rtnetlink attribute.
#[repr(C)]
struct rtattr {
    rta_len: u16,
    rta_type: u16,
}

#[repr(C)]
struct RtAttr<T> {
    hdr: rtattr,
    val: T,
}

impl<T> RtAttr<T> {
    fn new(ty: u16, val: T) -> RtAttr<T> {
        RtAttr {
            hdr: rtattr {
                rta_len: u16::try_from(mem::size_of::<RtAttr<T>>()).unwrap(),
                rta_type: ty,
            },
            val,
        }
    }
}

#[repr(C)]
struct AddAddrRequest {
    hdr: linux::nlmsghdr,
    payload: ifaddrmsg,
    local: RtAttr<u32>,
    addr: RtAttr<u32>,
    broadcast: RtAttr<u32>,
}

fn add_addr_to_interface(
    socket: &mut NetlinkSocket,
    interface_index: u32,
    addr: Ipv4Addr,
    broadcast: Ipv4Addr,
) -> i32 {
    let req = AddAddrRequest {
        hdr: linux::nlmsghdr {
            nlmsg_len: u32::try_from(mem::size_of::<AddAddrRequest>()).unwrap(),
            nlmsg_type: linux::RTM_NEWADDR,
            nlmsg_flags: u16::try_from(
                linux::NLM_F_REQUEST | linux::NLM_F_CREATE | linux::NLM_F_EXCL | linux::NLM_F_ACK,
            )
            .unwrap(),
            nlmsg_seq: socket.next_seq(),
            nlmsg_pid: 0,
        },
        payload: ifaddrmsg {
            ifa_family: u8::try_from(linux::AF_INET).unwrap(),
            ifa_prefixlen: 24,
            ifa_flags: 0,
            ifa_scope: 0,
            ifa_index: interface_index,
        },
        local: RtAttr::new(linux::IFA_LOCAL, addr.to_be()),
        addr: RtAttr::new(linux::IFA_ADDRESS, addr.to_be()),
        broadcast: RtAttr::new(linux::IFA_BROADCAST, broadcast.to_be()),
    };
    let req_bytes = unsafe {
        slice::from_raw_parts(
            (&req as *const AddAddrRequest) as *const u8,
            mem::size_of::<AddAddrRequest>(),
        )
    };
    let ret = socket.send(req_bytes);
    if ret < 0 {
        return ret.try_into().unwrap();
    }
    socket.ack_error()
}

#[repr(C)]
struct rtmsg {
    rtm_family: u8,
    rtm_dst_len: u8,
    rtm_src_len: u8,
    rtm_tos: u8,
    rtm_table: u8,
    rtm_protocol: u8,
    rtm_scope: u8,
    rtm_type: u8,
    rtm_flags: u32,
}

#[repr(C)]
struct AddRouteRequest {
    hdr: linux::nlmsghdr,
    payload: rtmsg,
    gateway: RtAttr<u32>,
    interface: RtAttr<u32>,
}

fn add_route_to_interface(
    socket: &mut NetlinkSocket,
    interface_index: u32,
    gateway: Ipv4Addr,
) -> i32 {
    let req = AddRouteRequest {
        hdr: linux::nlmsghdr {
            nlmsg_len: u32::try_from(mem::size_of::<AddRouteRequest>()).unwrap(),
            nlmsg_type: linux::RTM_NEWROUTE,
            nlmsg_flags: u16::try_from(
                linux::NLM_F_REQUEST | linux::NLM_F_CREATE | linux::NLM_F_EXCL | linux::NLM_F_ACK,
            )
            .unwrap(),
            nlmsg_seq: socket.next_seq(),
            nlmsg_pid: 0,
        },
        payload: rtmsg {
            rtm_family: u8::try_from(linux::AF_INET).unwrap(),
            rtm_dst_len: 0,
            rtm_src_len: 0,
            rtm_tos: 0,
            rtm_table: linux::RT_TABLE_MAIN,
            rtm_protocol: linux::RTPROT_BOOT,
            rtm_scope: linux::RT_SCOPE_UNIVERSE,
            rtm_type: linux::RTN_UNICAST,
            rtm_flags: 0,
        },
        gateway: RtAttr::new(linux::RTA_GATEWAY, gateway.to_be()),
        interface: RtAttr::new(linux::RTA_OIF, interface_index),
    };
    let req_bytes = unsafe {
        slice::from_raw_parts(
            (&req as *const AddRouteRequest) as *const u8,
            mem::size_of::<AddRouteRequest>(),
        )
    };
    let ret = socket.send(req_bytes);
    if ret < 0 {
        return ret.try_into().unwrap();
    }
    socket.ack_error()
}

#[repr(C)]
struct ifinfomsg {
    ifi_family: u8,
    ifi_type: u16,
    ifi_index: i32,
    ifi_flags: u32,
    ifi_change: u32,
}

#[repr(C)]
struct ChangeInterfaceRequest {
    hdr: linux::nlmsghdr,
    payload: ifinfomsg,
}

/// Sets a network interface's status to "admin up".
fn bring_interface_admin_up(socket: &mut NetlinkSocket, interface_index: i32) -> i32 {
    let req = ChangeInterfaceRequest {
        hdr: linux::nlmsghdr {
            nlmsg_len: u32::try_from(mem::size_of::<ChangeInterfaceRequest>()).unwrap(),
            nlmsg_type: linux::RTM_SETLINK,
            nlmsg_flags: u16::try_from(linux::NLM_F_REQUEST | linux::NLM_F_ACK).unwrap(),
            nlmsg_seq: socket.next_seq(),
            nlmsg_pid: 0,
        },
        payload: ifinfomsg {
            ifi_family: u8::try_from(linux::AF_UNSPEC).unwrap(),
            ifi_type: linux::ARPHRD_NONE,
            ifi_index: interface_index,
            ifi_flags: u32::try_from(linux::IFF_UP).unwrap(),
            ifi_change: u32::try_from(linux::IFF_UP).unwrap(),
        },
    };
    let req_bytes = unsafe {
        slice::from_raw_parts(
            (&req as *const ChangeInterfaceRequest) as *const u8,
            mem::size_of::<ChangeInterfaceRequest>(),
        )
    };
    let ret = socket.send(req_bytes);
    if ret < 0 {
        return ret.try_into().unwrap();
    }
    socket.ack_error()
}

pub fn setup_networking() -> i32 {
    let mut socket = match NetlinkSocket::new(linux::NETLINK_ROUTE) {
        Ok(s) => s,
        Err(e) => return e,
    };
    for interface in config::NET_INTERFACES.iter() {
        let addr = match interface.addr {
            Some(val) => val,
            None => continue,
        };
        let broadcast = interface
            .broadcast
            .unwrap_or_else(|| u32::from_be_bytes([255, 255, 255, 0]));
        let ret = add_addr_to_interface(&mut socket, interface.index, addr, broadcast);
        if ret < 0 {
            return ret;
        }
    }
    for interface in config::NET_INTERFACES.iter() {
        let ret = bring_interface_admin_up(&mut socket, i32::try_from(interface.index).unwrap());
        if ret < 0 {
            return ret;
        }
    }
    for interface in config::NET_INTERFACES.iter() {
        let gateway = match interface.gateway {
            Some(val) => val,
            None => continue,
        };
        let ret = add_route_to_interface(&mut socket, interface.index, gateway);
        if ret < 0 {
            return ret;
        }
    }
    0
}
