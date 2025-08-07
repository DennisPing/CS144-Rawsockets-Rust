use crate::tcp::wrap32::Wrap32;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum TcpError {
    #[error("Bad checksum")]
    BadChecksum,

    #[error("Invalid SEQ number: {expected} != {got}")]
    InvalidSeqNumber { expected: Wrap32, got: Wrap32 },

    #[error("Invalid ACK number: {expected} != {got}")]
    InvalidAckNumber { expected: Wrap32, got: Wrap32 },

    #[error("Resource temporarily unavailable")]
    ResourceUnavailable, // EAGAIN

    #[error("Invalid state")]
    InvalidState(&'static str), // EINVAL

    #[error("Invalid memory or resources")]
    InvalidBuffer, // ENOBUFS

    #[error("Connection already in use")]
    ConnectionInUse, // EADDRINUSE

    #[error("Socket is already connected")]
    IsConnected, // EISCONN

    #[error("Socket is not connected")]
    NotConnected, // ENOTCONN

    #[error("Connection timeout")]
    ConnectionTimeout, // ETIMEDOUT

    #[error("Connection reset")]
    ConnectionReset, // ECONNRESET

    #[error("Operation not supported")]
    OperationNotSupported, // ENOTSUP

    #[error("Address in use")]
    AddressInUse, // EADDRINUSE

    #[error("Address not available")]
    AddressNotAvailable, // EADDRNOTAVAIL

    #[error("Operation would block")]
    WouldBlock, // EWOULDBLOCK
}
