use std::net::AddrParseError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum IpError {
    #[error("Bad checksum")]
    BadChecksum,

    #[error("Invalid memory or resources")]
    InvalidBuffer,

    #[error("Invalid IP address: {0}")]
    InvalidAddress(#[from] AddrParseError),

    #[error("Time to live exceeded")]
    TtlExceeded,

    #[error("IP version not supported: {0}")]
    VersionNotSupported(u8),

    #[error("IP protocol not supported: {0}")]
    ProtocolNotSupported(u8),
}