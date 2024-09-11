use nix::errno::Errno;
use nix::sys::socket::setsockopt;
use nix::sys::socket::sockopt::{RcvBuf, ReceiveTimeout, ReuseAddr};
use nix::sys::socket::{socket, AddressFamily, SockFlag, SockProtocol, SockType};
use nix::sys::time::{TimeVal, TimeValLike};
use std::os::fd::OwnedFd;
use std::time::Duration;

/// Get a raw send socket. Local address reuse enabled.
pub fn new_send_socket(protocol: SockProtocol) -> Result<OwnedFd, Errno> {
    let sock_fd = socket(
        AddressFamily::Inet,
        SockType::Raw,
        SockFlag::empty(),
        protocol,
    )?;
    setsockopt(&sock_fd, ReuseAddr, &true)?;
    Ok(sock_fd)
}

/// Get a raw recv socket. The buf size is 2 MB.
pub fn new_recv_socket(protocol: SockProtocol) -> Result<OwnedFd, Errno> {
    let sock_fd = socket(
        AddressFamily::Inet,
        SockType::Raw,
        SockFlag::empty(),
        protocol,
    )?;
    let buf_size: usize = 1024 * 1024 * 2;
    setsockopt(&sock_fd, RcvBuf, &buf_size)?;
    Ok(sock_fd)
}

/// Set the receive timeout of a raw socket.
pub fn set_timeout(fd: OwnedFd, duration: Duration) -> Result<(), Errno> {
    let timeout = TimeVal::seconds(duration.as_secs() as i64)
        + TimeVal::microseconds(duration.as_secs() as i64);
    setsockopt(&fd, ReceiveTimeout, &timeout)?;
    Ok(())
}
