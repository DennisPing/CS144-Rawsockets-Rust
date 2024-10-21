use crate::tcp::tcp_flags::TCPFlags;
use crate::tcp::tcp_header::TCPHeader;
use crate::tcp::reassembler::Reassembler;
use std::io;
use crate::tcp::wrap32::Wrap32;

/// The receiver end of the `TcpConnection`
#[derive(Debug)]
pub struct TcpReceiver {
    isn: Wrap32,                // Initial seq number
    reassembler: Reassembler,   // Handles TCP segments
}

impl TcpReceiver {
    pub fn new(isn: Wrap32, reassembler: Reassembler) -> Self {
        TcpReceiver {
            isn,
            reassembler,
        }
    }

    pub fn recv(&mut self, tcph: TCPHeader) -> io::Result<()> {
        let checkpoint = self.reassembler.next_byte_idx() as u64;
        let abs_seq_no = tcph.seq_no.unwrap(self.isn, checkpoint);
        
        let is_last = tcph.flags.contains(TCPFlags::FIN);
        self.reassembler.insert(abs_seq_no as usize, &tcph.payload, is_last)
    }
    
    pub fn next_expected_seq_no(&self) -> u64 {
        self.reassembler.next_byte_idx() as u64
    }
}
