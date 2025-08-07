use crate::ip::ip_header::IpHeader;
use crate::packet::packet_error::PacketError;
use crate::socket::byte_stream::ByteStream;
use crate::socket::reassembler::Reassembler;
use crate::tcp::tcp_flags::TcpFlags;
use crate::tcp::tcp_header::TcpHeader;
use crate::tcp::wrap32::Wrap32;
use std::io::Read;

/// The receiver end of the `TcpSocket`
#[derive(Debug)]
pub struct TcpReceiver {
    isn: Wrap32,              // Ack number
    reassembler: Reassembler, // Handles incoming TCP segments
    advertised_window: usize, // The window size advertised to the sender
}

impl TcpReceiver {
    pub fn new(isn: Wrap32, capacity: usize) -> Self {
        TcpReceiver {
            isn,
            reassembler: Reassembler::new(ByteStream::new(capacity)),
            advertised_window: capacity, // Start with full window
        }
    }

    pub fn receive_segment(
        &mut self,
        tcph: TcpHeader,
        iph: IpHeader,
    ) -> Result<Option<Vec<u8>>, PacketError> {
        // Handle initial SYN
        if tcph.flags.contains(TcpFlags::SYN) {
            self.reassembler.insert(0, &[], false)?;
            // !todo(Generate SYN-ACK)
            // This requires interaction with TcpSender, which may be handled at the TcpSocket layer.
            // Communicate with TcpSocket to send a SYN-ACK.

            return Ok(None);
        }

        let checkpoint = self.reassembler.next_byte_idx() as u64;
        let mut abs_seq_no = tcph.seq_no.unwrap(self.isn, checkpoint);

        let is_last = tcph.flags.contains(TcpFlags::FIN);
        if is_last {
            abs_seq_no += 1; // FIN consumes one sequence number
        }

        self.reassembler
            .insert(abs_seq_no as usize, &tcph.payload, is_last)?;

        // Update the new advertised window
        self.advertised_window = self.reassembler.get_output().remaining_capacity();

        // Return any new reassembled data
        let mut data = Vec::new();
        self.reassembler.read_to_end(&mut data)?;
        if data.is_empty() {
            Ok(None)
        } else {
            Ok(Some(data))
        }
    }

    pub fn advertised_window(&self) -> usize {
        self.advertised_window
    }

    /// Generate a new ack number and advertised window size
    pub fn generate_ack(&self) -> (Wrap32, usize) {
        let ack_no = Wrap32::new(self.reassembler.next_byte_idx() as u32);
        (ack_no, self.advertised_window)
    }
}
