use std::net::Ipv4Addr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PseudoHeader {
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub protocol: u8,
    pub payload_len: u16,
}

impl PseudoHeader {
    pub fn new(src: Ipv4Addr, dst: Ipv4Addr, protocol: u8, payload_len: u16) -> PseudoHeader {
        Self {
            src,
            dst,
            protocol,
            payload_len,
        }
    }
}
