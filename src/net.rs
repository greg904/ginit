//! Networking related code is put in this module. The init system has to set
//! up the networking stack so that the user has access to the internet. On
//! Linux, this is done using the `rtnetlink` interface.

use std::fmt::Debug;
use std::{io, net::Ipv4Addr};

use neli::consts::rtnl::{Arphrd, Iff, IffFlags, RtScope, RtTable, Rta, RtmFFlags, Rtn, Rtprot};
use neli::rtnl::{Ifinfomsg, Rtmsg};
use neli::{
    consts::{
        nl::{NlTypeWrapper, NlmF, NlmFFlags},
        rtnl::{Ifa, IfaFFlags, RtAddrFamily, Rtm},
        socket::NlFamily,
    },
    err::NlError,
    nl::{NlPayload, Nlmsghdr},
    rtnl::{Ifaddrmsg, Rtattr},
    socket::NlSocketHandle,
    types::RtBuffer,
    Nl,
};

use crate::config;

#[derive(Debug)]
pub enum SetupNetworkingError {
    Io(io::Error),
    NlSocket(NlError),
    Rtnl(libc::c_int),
}

fn rtnl_check_error<P>(rtnl: &mut NlSocketHandle) -> Result<(), SetupNetworkingError>
where
    P: Nl + Debug,
{
    for resp in rtnl.iter(false) {
        let resp: Nlmsghdr<NlTypeWrapper, P> = resp.map_err(SetupNetworkingError::NlSocket)?;
        if let NlPayload::Err(err) = resp.nl_payload {
            if err.error != 0 {
                return Err(SetupNetworkingError::Rtnl(err.error));
            }
            break;
        }
    }
    Ok(())
}

fn rtnl_addr(addr: Ipv4Addr) -> u32 {
    u32::from(addr).to_be()
}

fn set_eth0_addr(rtnl: &mut NlSocketHandle) -> Result<(), SetupNetworkingError> {
    let mut attrs = RtBuffer::new();
    attrs.push(Rtattr::new(None, Ifa::Local, rtnl_addr(config::ETH0_ADDR)).unwrap());
    attrs.push(Rtattr::new(None, Ifa::Address, rtnl_addr(config::ETH0_ADDR)).unwrap());
    attrs.push(Rtattr::new(None, Ifa::Broadcast, rtnl_addr(config::ETH0_BROADCAST)).unwrap());
    let payload = Ifaddrmsg {
        ifa_family: RtAddrFamily::Inet,
        ifa_prefixlen: 24,
        ifa_flags: IfaFFlags::empty(),
        ifa_scope: 0,
        ifa_index: config::ETH0_INDEX,
        rtattrs: attrs,
    };
    let hdr = Nlmsghdr::new(
        None,
        Rtm::Newaddr,
        NlmFFlags::new(&[NlmF::Request, NlmF::Create, NlmF::Excl, NlmF::Ack]),
        None,
        None,
        NlPayload::Payload(payload),
    );
    rtnl.send(hdr).map_err(SetupNetworkingError::NlSocket)?;
    rtnl_check_error::<Ifaddrmsg>(rtnl)
}

fn set_eth0_route(rtnl: &mut NlSocketHandle) -> Result<(), SetupNetworkingError> {
    let mut attrs = RtBuffer::new();
    attrs.push(Rtattr::new(None, Rta::Gateway, rtnl_addr(config::ETH0_GATEWAY)).unwrap());
    attrs.push(Rtattr::new(None, Rta::Oif, config::ETH0_INDEX).unwrap());
    let payload = Rtmsg {
        rtm_family: RtAddrFamily::Inet,
        rtm_dst_len: 0,
        rtm_src_len: 0,
        rtm_tos: 0,
        rtm_table: RtTable::Main,
        rtm_protocol: Rtprot::Boot,
        rtm_scope: RtScope::Universe,
        rtm_type: Rtn::Unicast,
        rtm_flags: RtmFFlags::empty(),
        rtattrs: attrs,
    };
    let hdr = Nlmsghdr::new(
        None,
        Rtm::Newroute,
        NlmFFlags::new(&[NlmF::Request, NlmF::Create, NlmF::Excl, NlmF::Ack]),
        None,
        None,
        NlPayload::Payload(payload),
    );
    rtnl.send(hdr).map_err(SetupNetworkingError::NlSocket)?;
    rtnl_check_error::<Ifaddrmsg>(rtnl)
}

/// Sets a network interface's status to "admin up".
fn bring_up(rtnl: &mut NlSocketHandle, interface_index: i32) -> Result<(), SetupNetworkingError> {
    let payload = Ifinfomsg::new(
        RtAddrFamily::Unspecified,
        Arphrd::None,
        interface_index,
        IffFlags::new(&[Iff::Up]),
        IffFlags::new(&[Iff::Up]),
        RtBuffer::new(),
    );
    let hdr = Nlmsghdr::new(
        None,
        Rtm::Setlink,
        NlmFFlags::new(&[NlmF::Request, NlmF::Ack]),
        None,
        None,
        NlPayload::Payload(payload),
    );
    rtnl.send(hdr).map_err(SetupNetworkingError::NlSocket)?;
    rtnl_check_error::<Ifinfomsg>(rtnl)
}

pub fn setup_networking() -> Result<(), SetupNetworkingError> {
    let mut rtnl =
        NlSocketHandle::connect(NlFamily::Route, None, &[]).map_err(SetupNetworkingError::Io)?;
    set_eth0_addr(&mut rtnl)?;
    bring_up(&mut rtnl, config::LO_INDEX)?;
    bring_up(&mut rtnl, config::ETH0_INDEX)?;
    set_eth0_route(&mut rtnl)?;
    Ok(())
}
