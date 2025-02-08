use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum HeaderError {
    #[error("Invalid buffer: expected {expected} bytes, actual {actual} bytes")]
    InvalidBuffer {expected: usize, actual: usize},

    #[error("Bad checksum")]
    BadChecksum(String),
}