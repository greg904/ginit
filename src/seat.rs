//! This is a lighter implementation of something like udev and seatd. The goal is for for the
//! Wayland compositor, and only it, be able to request access to GPU and input devices. To do
//! that, it is given a FD to a UNIX socket on which it will send a datagram with the path to a
//! device to that socket followed by a NUL byte, and it will receive a datagram with a FD of the
//! device if the request was allowed, or an empty datagram otherwise.

use core::convert::{TryFrom, TryInto};
use core::fmt::Write;
use core::mem;
use core::ptr;

use crate::linux::{self, Fd};

#[repr(C)]
struct RightsCtrlMsg {
    hdr: linux::cmsghdr,
    fd: i32,
}

impl RightsCtrlMsg {
    fn new(fd: i32) -> Self {
        Self {
            hdr: linux::cmsghdr {
                cmsg_level: linux::SOL_SOCKET,
                cmsg_type: linux::SCM_RIGHTS,
                cmsg_len: mem::size_of::<RightsCtrlMsg>(),
            },
            fd,
        }
    }
}

/// A seat server is an object to process device open requests from the Wayland compositor. It will
/// receive those requests on a anonymous UNIX socket.
pub struct SeatServer {
    fd: Fd,
}

impl SeatServer {
    /// Creates a seat server. On success, the server and a sending FD are returned in a tuple in
    /// that order.
    pub fn new() -> Result<(Self, Fd), i32> {
        // We want to make both FDs `CLOEXEC` and then remove the flag from one of them instead of
        // not making any `CLOEXEC` and then adding the flag to one of them to prevent an `exec`
        // called in another thread to pick up the wrong FD.
        let pair =
            linux::create_socket_pair(linux::AF_UNIX, linux::SOCK_DGRAM | linux::SOCK_CLOEXEC, 0)?;
        let mut ret = linux::fcntl(pair.1 .0, linux::F_GETFD, 0);
        if ret < 0 {
            return Err(ret);
        }
        let new_flags = ret | linux::FD_CLOEXEC;
        ret = linux::fcntl(pair.1 .0, linux::F_SETFD, new_flags as u64);
        if ret < 0 {
            return Err(ret);
        }

        Ok((Self { fd: pair.0 }, pair.1))
    }

    fn process_incoming_one(&mut self) -> Result<bool, i32> {
        let mut buf = [0u8; 48];
        let ret = unsafe {
            linux::recvfrom(
                i32::try_from(self.fd.0).unwrap(),
                &mut buf,
                linux::MSG_DONTWAIT,
                ptr::null_mut(),
                0,
            )
        };
        let n = match usize::try_from(ret) {
            Ok(n) => n,
            Err(_) => {
                if i32::try_from(ret).unwrap() == -linux::EAGAIN {
                    // This means that there are no more datagrams to process.
                    return Ok(false);
                }
                return Err(ret.try_into().unwrap());
            }
        };
        // Datagram should be a NUL-terminated string.
        if n == 0 || buf[n - 1] != b'\0' {
            return Ok(true);
        }
        let dev_fd = unsafe {
            linux::open(
                buf.as_ptr(),
                linux::O_RDWR
                    | linux::O_NOCTTY
                    | linux::O_NOFOLLOW
                    | linux::O_CLOEXEC
                    | linux::O_NONBLOCK,
                0,
            )
        };
        // We cannot send anciliary data without actual data.
        let byte = 0u8;
        let iov = linux::iovec {
            iov_base: &byte as *const u8 as *mut u8,
            iov_len: mem::size_of_val(&byte),
        };
        if dev_fd < 0 {
            // Send a message without an FD to the client to tell it about the error.
            let mut msg = linux::msghdr {
                msg_name: ptr::null_mut(),
                msg_namelen: 0,
                msg_iov: &iov as *const linux::iovec as *mut linux::iovec,
                msg_iovlen: 1,
                msg_control: ptr::null_mut(),
                msg_controllen: 0,
                msg_flags: 0,
            };
            let ret = unsafe { linux::sendmsg(i32::try_from(self.fd.0).unwrap(), &mut msg, 0) };
            if ret < 0 {
                writeln!(
                    linux::Stderr,
                    "failed to send error message to Wayland compositor: {ret}"
                )
                .unwrap();
            }
            return Ok(true);
        }
        let dev_fd = linux::Fd(u32::try_from(dev_fd).unwrap());
        let mut rights = RightsCtrlMsg::new(i32::try_from(dev_fd.0).unwrap());
        let mut msg = linux::msghdr {
            msg_name: ptr::null_mut(),
            msg_namelen: 0,
            msg_iov: &iov as *const linux::iovec as *mut linux::iovec,
            msg_iovlen: 1,
            msg_control: &mut rights as *mut RightsCtrlMsg as *mut u8,
            msg_controllen: mem::size_of_val(&rights),
            msg_flags: 0,
        };
        let ret = unsafe { linux::sendmsg(i32::try_from(self.fd.0).unwrap(), &mut msg, 0) };
        if ret < 0 {
            writeln!(
                linux::Stderr,
                "failed to send device FD to Wayland compositor: {ret}"
            )
            .unwrap();
        }
        Ok(true)
    }

    pub fn process_incoming(&mut self) -> Result<(), i32> {
        while self.process_incoming_one()? {}
        Ok(())
    }

    pub fn fd(&self) -> u32 {
        self.fd.0
    }
}
