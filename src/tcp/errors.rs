use std::io;
use thiserror::Error;
use crate::packet::errors::HeaderError;
use crate::tcp::wrap32::Wrap32;

#[derive(Error, Debug)]
pub enum TcpError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error), // Wrapper around std::io::Error
    
    #[error("Header error: {0}")]
    HeaderError(#[from] HeaderError), // Wrapper around HeaderError

    #[error("Invalid SEQ number: {expected} != {got}")]
    InvalidSeqNumber {
        expected: Wrap32,
        got: Wrap32,
    },

    #[error("Invalid ACK number: {expected} != {got}")]
    InvalidAckNumber {
        expected: Wrap32,
        got: Wrap32,
    },

    #[error("Resource temporarily unavailable")]
    ResourceUnavailable, // EAGAIN

    #[error("Invalid state")]
    InvalidState(String), // EINVAL

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