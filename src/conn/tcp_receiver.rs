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

    /// Receive a TCP segment and process it based on the current state
    pub fn receive_segment(&mut self, seq_num: Wrapping<u32>, data: Vec<u8>, syn: bool, fin: bool) {
        match self.state {
            TCPState::Listen => {
                if syn {
                    self.handle_syn(seq_num);
                }
            }
            TCPState::SynRcvd | TCPState::Established => {
                if syn {
                    return;
                }
                self.handle_segment(seq_num, data, fin);
            }
            TCPState::CloseWait | TCPState::FinWait1 | TCPState::FinWait2 => {
                // Handle ongoing data and FIN flag during closing connection
                self.handle_segment(seq_num, data, fin);
            }
            // Do nothing if the connection is in a closed or reset state
            _ => {}
        }
    }

    /// Handle the SYN flag and transition to the SYN_RCVD state
    fn handle_syn(&mut self, seq_num: Wrapping<u32>) {
        self.isn = Some(seq_num);
        self.state = TCPState::SynRcvd;
    }

    fn handle_segment(&mut self, seq_num: Wrapping<u32>, data: Vec<u8>, fin: bool) {
        // Unwrap sequence number and pass data onto StreamReassembler
        if let Some(isn) = self.isn {
            let abs_seq_num = self.unwrap_seq_num(seq_num);

            self.reassembler
                .assemble_segment(abs_seq_num as usize, data, fin);

            if fin {
                self.handle_fin();
            } else {
                self.state = TCPState::Established;
            }
        }
    }

    fn handle_fin(&mut self) {
        match self.state {
            TCPState::Established => self.state = TCPState::CloseWait,
            TCPState::FinWait1 => self.state = TCPState::Closing,
            TCPState::FinWait2 => self.state = TCPState::TimeWait,
            _ => {}
        }
    }

    /// Calculates the acknowledgment number to be sent to the sender.
    /// The ack_num is the sequence number of the next byte expected.
    pub fn next_ack_num(&self) -> Option<Wrapping<u32>> {
        if let Some(isn) = self.isn {
            let next_unassembled_byte = self.reassembler.first_unassembled();
            let next_seq_num = next_unassembled_byte as u32;
            let ack_num = isn + Wrapping(next_seq_num) + Wrapping(1);
            Some(ack_num)
        } else {
            None
        }
    }

    /// Calculate the window size to advertise to the sender
    pub fn window_size(&self) -> usize {
        let capacity = self.reassembler.output().remaining_capacity();
        let unassembled = self.reassembler.unassembled_bytes();
        capacity - unassembled
    }

    /// Unwrap the sequence number using the ISN
    fn unwrap_seq_num(&self, seq_num: Wrapping<u32>) -> u32 {
        let isn = self.isn.unwrap_or(Wrapping(0)).0;
        seq_num.0.wrapping_sub(isn)
    }

    /// Get a reference to the ByteStream
    pub fn stream_out(&self) -> &ByteStream {
        self.reassembler.output()
    }

    /// Get the current state (for debugging purposes)
    pub fn state(&self) -> &TCPState {
        &self.state
    }
}
