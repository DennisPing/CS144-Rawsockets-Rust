use crate::tcp::tcp_flags::TcpFlags;
use crate::tcp::wrap32::Wrap32;

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub seq_no: Wrap32,
    pub ack_no: Wrap32,
    pub flags: TcpFlags,
    pub window: u16,
    pub payload: Vec<u8>,
}

impl Segment {
    pub fn is_syn(&self) -> bool {
        self.flags == TcpFlags::SYN
    }

    pub fn is_ack(&self) -> bool {
        self.flags == TcpFlags::ACK
    }

    pub fn is_fin(&self) -> bool {
        self.flags == TcpFlags::FIN
    }

    pub fn is_syn_ack(&self) -> bool {
        self.flags == TcpFlags::SYN | TcpFlags::ACK
    }

    pub fn is_fin_ack(&self) -> bool {
        self.flags == TcpFlags::FIN | TcpFlags::ACK
    }

    pub fn is_rst(&self) -> bool {
        self.flags == TcpFlags::RST
    }
}
