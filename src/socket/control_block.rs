use crate::tcp::wrap32::Wrap32;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

/// The TCP Control Block (TCB) holds connection-wide state.
#[derive(Debug)]
pub struct TcpControlBlock {
    pub seq_no: Wrap32,
    pub ack_no: Wrap32,
    pub window_size: u16,
    pub rto: Duration,
    pub src_ip: Ipv4Addr,
    pub src_port: u16,
    pub dst_ip: Option<Ipv4Addr>,
    pub dst_port: Option<u16>,
    pub sent_segments: HashMap<Wrap32, (Vec<u8>, Instant)>,
    pub last_seq_no: Option<Wrap32>,
}
