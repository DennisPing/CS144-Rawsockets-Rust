use std::io;
use thiserror::Error;
use crate::ip::ip_error::IpError;
use crate::tcp::tcp_error::TcpError;

#[derive(Error, Debug)]
pub enum PacketError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("IP error")]
    Ip(#[from] IpError),

    #[error("TCP error")]
    Tcp(#[from] TcpError),
}

impl PartialEq for PacketError {
    fn eq(&self, other: &Self) -> bool {
        use PacketError::*;
        match (self, other) {
            // Compare I/O errors by their kind:
            (Io(a), Io(b)) => a.kind() == b.kind(),
            (Ip(a), Ip(b)) => a == b,
            (Tcp(a), Tcp(b)) => a == b,
            _ => false,
        }
    }
}