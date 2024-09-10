use std::os::fd::OwnedFd;
use nix::sys::socket::{socket, AddressFamily, SockType, SockFlag, SockProtocol};
use nix::errno::Errno;
use nix::sys::socket::setsockopt;
use nix::sys::socket::sockopt::{ReuseAddr, RcvBuf, ReceiveTimeout};
use nix::sys::time::{TimeVal, TimeValLike};
use std::time::Duration;

pub fn new_send_socket(protocol: SockProtocol) -> Result<OwnedFd, Errno> {
    let sock_fd = socket(AddressFamily::Inet, SockType::Raw, SockFlag::empty(), protocol)?;
    setsockopt(&sock_fd, ReuseAddr, &true)?;
    Ok(sock_fd)
}

pub fn new_recv_socket(protocol: SockProtocol) -> Result<OwnedFd, Errno> {
    let sock_fd = socket(AddressFamily::Inet, SockType::Raw, SockFlag::empty(), protocol)?;
    let buf_size: usize = 1024 * 1024 * 2;
    setsockopt(&sock_fd, RcvBuf, &buf_size)?;
    Ok(sock_fd)
}

pub fn set_timeout(fd: OwnedFd, duration: Duration) -> Result<(), Errno> {
    let timeout = TimeVal::seconds(duration.as_secs() as i64) + TimeVal::microseconds(duration.as_secs() as i64);
    setsockopt(&fd, ReceiveTimeout, &timeout)?;
    Ok(())
}