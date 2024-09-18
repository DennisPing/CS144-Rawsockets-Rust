use std::{fmt, io};

#[derive(Debug)]
pub enum ConnError {
    IoError(io::Error),
    IpMismatch,
    TCPReceiverError(String),
}

impl From<io::Error> for ConnError {
    fn from(err: io::Error) -> Self {
        ConnError::IoError(err)
    }
}

impl fmt::Display for ConnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnError::IoError(err) => write!(f, "{}", err),
            ConnError::IpMismatch => write!(f, "IP address mismatch"),
            ConnError::TCPReceiverError(msg) => write!(f, "TCP receiver error: {}", msg),
        }
    }
}

impl std::error::Error for ConnError {}
