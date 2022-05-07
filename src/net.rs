//! Networking related code is put in this module. The init system has to set
//! up the networking stack so that the user has access to the internet. On
//! Linux, this is done using the `rtnetlink` interface.

use core::slice;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::{io, net::Ipv4Addr};
use std::{mem, ptr};

use crate::config;
use crate::libc_wrapper;

/// A netlink socket FD with automatic cleanup and that keeps track of the
/// current sequence number for messages.
struct NetlinkSocket {
    fd: libc::c_int,
    seq: u32,
}

impl NetlinkSocket {
    /// Creates a new netlink route socket.
    ///
    /// `protocol` is used to tell the kernel what the socket will be used for.
    /// For instance, to listen to and modify networking configuration, use
    /// `libc::NETLINK_ROUTE`.
    fn new(protocol: libc::c_int) -> io::Result<NetlinkSocket> {
        let fd = unsafe { libc::socket(libc::AF_NETLINK, libc::SOCK_RAW, protocol) };
        let fd = libc_wrapper::check_error(fd)?;
        Ok(NetlinkSocket { fd, seq: 0 })
    }

    /// Returns and increments the next sequence number to use to send a
    /// message.
    fn next_seq(&mut self) -> u32 {
        let seq = self.seq;
        self.seq += 1;
        seq
    }

    /// Sends a message through the socket.
    fn send(&self, msg: &[u8]) -> io::Result<()> {
        let ret = unsafe { libc::write(self.fd, msg.as_ptr() as *const libc::c_void, msg.len()) };
        libc_wrapper::check_error(ret)?;
        Ok(())
    }

    /// Receives a message from the socket.
    fn recv(&self, msg: &mut [u8]) -> io::Result<libc::ssize_t> {
        let ret = unsafe { libc::read(self.fd, msg.as_mut_ptr() as *mut libc::c_void, msg.len()) };
        libc_wrapper::check_error(ret)
    }

    /// Drains the socket until a `nmsgerr` message is available. That message
    /// is then read and depending on the error code inside of it, either a
    /// Ok or Err is returned.
    fn ack_error(&self) -> io::Result<()> {
        loop {
            let mut buf = [0u8; 8192];
            let len = self.recv(&mut buf)?.try_into().unwrap();

            let mut i = 0;
            loop {
                if i + mem::size_of::<libc::nlmsghdr>() > len {
                    break;
                }
                let hdr =
                    unsafe { ptr::read_unaligned(buf[i..].as_ptr() as *const libc::nlmsghdr) };
                if i32::from(hdr.nlmsg_type) == libc::NLMSG_ERROR {
                    let payload = unsafe {
                        ptr::read(buf[i + mem::size_of::<libc::nlmsghdr>()..].as_ptr()
                            as *const libc::nlmsgerr)
                    };
                    return match payload.error {
                        0 => Ok(()),
                        err => Err(io::Error::new(
                            io::ErrorKind::Other,
                            format!("netlink error {}", err),
                        )),
                    };
                }
                i += usize::try_from(hdr.nlmsg_len).unwrap();
            }
        }
    }
}

impl Drop for NetlinkSocket {
    fn drop(&mut self) {
        let ret = unsafe { libc::close(self.fd) };
        if let Err(err) = libc_wrapper::check_error(ret) {
            eprintln!("failed to close netlink socket: {:?}", err);
        }
    }
}

fn serialize_addr(addr: Ipv4Addr) -> u32 {
    u32::from(addr).to_be()
}

#[repr(C)]
struct ifaddrmsg {
    ifa_family: libc::c_uchar,
    ifa_prefixlen: libc::c_uchar,
    ifa_flags: libc::c_uchar,
    ifa_scope: libc::c_uchar,
    ifa_index: libc::c_uint,
}

/// This is just the header for a rtnetlink attribute.
#[repr(C)]
struct rtattr {
    rta_len: libc::c_ushort,
    rta_type: libc::c_ushort,
}

#[repr(C)]
struct RtAttr<T> {
    hdr: rtattr,
    val: T,
}

impl<T> RtAttr<T> {
    fn new(ty: libc::c_ushort, val: T) -> RtAttr<T> {
        RtAttr {
            hdr: rtattr {
                rta_len: libc::c_ushort::try_from(mem::size_of::<RtAttr<T>>()).unwrap(),
                rta_type: ty,
            },
            val,
        }
    }
}

#[repr(C)]
struct AddAddrRequest {
    hdr: libc::nlmsghdr,
    payload: ifaddrmsg,
    local: RtAttr<u32>,
    addr: RtAttr<u32>,
    broadcast: RtAttr<u32>,
}

fn add_addr_to_interface(
    socket: &mut NetlinkSocket,
    interface_index: libc::c_uint,
    addr: Ipv4Addr,
    broadcast: Ipv4Addr,
) -> io::Result<()> {
    let req = AddAddrRequest {
        hdr: libc::nlmsghdr {
            nlmsg_len: u32::try_from(mem::size_of::<AddAddrRequest>()).unwrap(),
            nlmsg_type: libc::RTM_NEWADDR,
            nlmsg_flags: u16::try_from(
                libc::NLM_F_REQUEST | libc::NLM_F_CREATE | libc::NLM_F_EXCL | libc::NLM_F_ACK,
            )
            .unwrap(),
            nlmsg_seq: socket.next_seq(),
            nlmsg_pid: 0,
        },
        payload: ifaddrmsg {
            ifa_family: libc::c_uchar::try_from(libc::AF_INET).unwrap(),
            ifa_prefixlen: 24,
            ifa_flags: 0,
            ifa_scope: 0,
            ifa_index: interface_index,
        },
        local: RtAttr::new(libc::IFA_LOCAL, serialize_addr(addr)),
        addr: RtAttr::new(libc::IFA_ADDRESS, serialize_addr(addr)),
        broadcast: RtAttr::new(libc::IFA_BROADCAST, serialize_addr(broadcast)),
    };
    let req_bytes = unsafe {
        slice::from_raw_parts(
            (&req as *const AddAddrRequest) as *const u8,
            mem::size_of::<AddAddrRequest>(),
        )
    };
    socket.send(req_bytes)?;
    socket.ack_error()
}

#[repr(C)]
struct rtmsg {
    rtm_family: libc::c_uchar,
    rtm_dst_len: libc::c_uchar,
    rtm_src_len: libc::c_uchar,
    rtm_tos: libc::c_uchar,
    rtm_table: libc::c_uchar,
    rtm_protocol: libc::c_uchar,
    rtm_scope: libc::c_uchar,
    rtm_type: libc::c_uchar,
    rtm_flags: libc::c_uint,
}

#[repr(C)]
struct AddRouteRequest {
    hdr: libc::nlmsghdr,
    payload: rtmsg,
    gateway: RtAttr<u32>,
    interface: RtAttr<u32>,
}

fn add_route_to_interface(
    socket: &mut NetlinkSocket,
    interface_index: libc::c_uint,
    gateway: Ipv4Addr,
) -> io::Result<()> {
    let req = AddRouteRequest {
        hdr: libc::nlmsghdr {
            nlmsg_len: u32::try_from(mem::size_of::<AddRouteRequest>()).unwrap(),
            nlmsg_type: libc::RTM_NEWROUTE,
            nlmsg_flags: u16::try_from(
                libc::NLM_F_REQUEST | libc::NLM_F_CREATE | libc::NLM_F_EXCL | libc::NLM_F_ACK,
            )
            .unwrap(),
            nlmsg_seq: socket.next_seq(),
            nlmsg_pid: 0,
        },
        payload: rtmsg {
            rtm_family: libc::c_uchar::try_from(libc::AF_INET).unwrap(),
            rtm_dst_len: 0,
            rtm_src_len: 0,
            rtm_tos: 0,
            rtm_table: libc::RT_TABLE_MAIN,
            rtm_protocol: libc::RTPROT_BOOT,
            rtm_scope: libc::RT_SCOPE_UNIVERSE,
            rtm_type: libc::RTN_UNICAST,
            rtm_flags: 0,
        },
        gateway: RtAttr::new(libc::RTA_GATEWAY, serialize_addr(gateway)),
        interface: RtAttr::new(libc::RTA_OIF, interface_index),
    };
    let req_bytes = unsafe {
        slice::from_raw_parts(
            (&req as *const AddRouteRequest) as *const u8,
            mem::size_of::<AddRouteRequest>(),
        )
    };
    socket.send(req_bytes)?;
    socket.ack_error()
}

#[repr(C)]
struct ifinfomsg {
    ifi_family: libc::c_uchar,
    ifi_type: libc::c_ushort,
    ifi_index: libc::c_int,
    ifi_flags: libc::c_uint,
    ifi_change: libc::c_uint,
}

#[repr(C)]
struct ChangeInterfaceRequest {
    hdr: libc::nlmsghdr,
    payload: ifinfomsg,
}

/// Sets a network interface's status to "admin up".
fn bring_interface_admin_up(socket: &mut NetlinkSocket, interface_index: i32) -> io::Result<()> {
    let req = ChangeInterfaceRequest {
        hdr: libc::nlmsghdr {
            nlmsg_len: u32::try_from(mem::size_of::<ChangeInterfaceRequest>()).unwrap(),
            nlmsg_type: libc::RTM_SETLINK,
            nlmsg_flags: u16::try_from(libc::NLM_F_REQUEST | libc::NLM_F_ACK).unwrap(),
            nlmsg_seq: socket.next_seq(),
            nlmsg_pid: 0,
        },
        payload: ifinfomsg {
            ifi_family: libc::c_uchar::try_from(libc::AF_UNSPEC).unwrap(),
            ifi_type: libc::ARPHRD_NONE,
            ifi_index: interface_index,
            ifi_flags: libc::c_uint::try_from(libc::IFF_UP).unwrap(),
            ifi_change: libc::c_uint::try_from(libc::IFF_UP).unwrap(),
        },
    };
    let req_bytes = unsafe {
        slice::from_raw_parts(
            (&req as *const ChangeInterfaceRequest) as *const u8,
            mem::size_of::<ChangeInterfaceRequest>(),
        )
    };
    socket.send(req_bytes)?;
    socket.ack_error()
}

pub fn setup_networking() -> io::Result<()> {
    let mut socket = NetlinkSocket::new(libc::NETLINK_ROUTE)?;
    add_addr_to_interface(
        &mut socket,
        config::ETH0_INDEX,
        config::ETH0_ADDR,
        config::ETH0_BROADCAST,
    )?;
    bring_interface_admin_up(&mut socket, i32::try_from(config::LO_INDEX).unwrap())?;
    bring_interface_admin_up(&mut socket, i32::try_from(config::ETH0_INDEX).unwrap())?;
    add_route_to_interface(&mut socket, config::ETH0_INDEX, config::ETH0_GATEWAY)?;
    Ok(())
}
