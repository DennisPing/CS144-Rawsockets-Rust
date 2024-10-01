use crate::conn::byte_stream::ByteStream;
use crate::conn::reassembler::Reassembler;
use crate::conn::tcp_state::TCPState;
use std::num::Wrapping;

#[derive(Debug)]
pub struct TCPReceiver {
    state: TCPState,            // Current state of the TCP receiver
    reassembler: Reassembler,   // Handles TCP segments
    isn: Option<Wrapping<u32>>, // Initial seq number
}

impl TCPReceiver {
    pub fn new(capacity: usize) -> Self {
        TCPReceiver {
            state: TCPState::Listen,
            reassembler: Reassembler::new(ByteStream::new(capacity)),
            isn: None,
        }
    }
}
