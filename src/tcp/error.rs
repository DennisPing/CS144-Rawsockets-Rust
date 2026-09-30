//! TCP/IP wire errors for parsing and building packets.

use thiserror::Error;

/// Errors during packet parsing/validation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WireError {
    #[error("truncated: need {needed} bytes, got {got}")]
    Truncated { needed: usize, got: usize },

    #[error("bad checksum")]
    BadChecksum,

    #[error("IP version {0} not supported")]
    IpVersionNotSupported(u8),

    #[error("protocol {0} not supported")]
    ProtocolNotSupported(u8),

    #[error("invalid TCP data offset: {0}")]
    InvalidTcpDataOffset(u8),

    #[error("invalid TCP option kind {0}")]
    InvalidTcpOption(u8),

    #[error("invalid header")]
    InvalidHeader,
}

/// Errors during packet building/encoding.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BuildError {
    #[error("buffer too small: need {needed} bytes, got {got}")]
    BufferTooSmall { needed: usize, got: usize },

    #[error("truncated: need {needed} bytes, got {got}")]
    Truncated { needed: usize, got: usize },

    #[error("options not 32-bit aligned: {len} bytes")]
    OptionsNotAligned { len: usize },

    #[error("options too long: {len} bytes (max 40)")]
    OptionsTooLong { len: usize },
}
