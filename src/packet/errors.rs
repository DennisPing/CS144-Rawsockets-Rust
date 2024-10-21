use std::io;
use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum HeaderError {
    #[error("Buffer too small: expected at least {expected} bytes, actual {found} bytes")]
    BufferTooSmall {expected: usize, found: usize},

    #[error("Bad checksum")]
    BadChecksum(String),
}