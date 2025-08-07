use crate::ip::ip_flags::IpFlags;
use crate::packet::packet_error::PacketError;
use crate::socket::control_block::TcpControlBlock;
use crate::tcp::tcp_error::TcpError;
use crate::tcp::tcp_flags::TcpFlags;
use crate::tcp::tcp_segment::TcpSegment;
use crate::tcp::wrap32::Wrap32;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

/// A Timer trait to abstract RTO and retransmission behavior.
pub trait Timer {
    fn start(&mut self, duration: Duration);
    fn cancel(&mut self);
}

pub(crate) trait Sender {
    fn send(&self, segment: TcpSegment);
}

pub(crate) trait Receiver {
    fn recv(&mut self) -> Option<TcpSegment>;
}

/// An orchestrator for the Sender, Receiver, and ControlBlock
pub struct TcpSocket<State> {
    pub tcb: TcpControlBlock,        // Holds connection-wide state
    pub sender: Box<dyn Sender>,     // Sends data to the network
    pub receiver: Box<dyn Receiver>, // Receives data
    pub timer: Box<dyn Timer>,       // Handles retransmissions and timeout (RTO)
    pub state: PhantomData<State>,
}

/// Shared helper methods
impl<State> TcpSocket<State> {
    pub fn retransmit_last_segment(&mut self) -> Result<(), PacketError> {
        if let Some(seq_no) = self.tcb.last_seq_no {
            if let Some((payload, _timestamp)) = self.tcb.sent_segments.remove(&seq_no) {
                // return self.send_segment(&payload);
            }
        }
        Err(PacketError::Tcp(TcpError::InvalidState(
            "Tried to resend a segment that doesn't exist",
        )))
    }

    pub fn recv_segment(&mut self) -> Option<TcpSegment> {
        self.receiver.recv()
    }

    /// Send a `TcpSegment` with the specified flags and payload.
    pub fn send_segment(
        &mut self,
        flags: TcpFlags,
        payload: Option<&[u8]>,
    ) -> Result<(), PacketError> {
        let payload = payload.unwrap_or(&[]);
        let dst_ip = self
            .tcb
            .dst_ip
            .ok_or_else(|| TcpError::InvalidState("Destination IP address not set"))?;
        let dst_port = self
            .tcb
            .dst_port
            .ok_or_else(|| TcpError::InvalidState("Destination IP port not set"))?;

        let mut segment = TcpSegment::new(self.tcb.src_ip, self.tcb.src_port, dst_ip, dst_port);
        segment.ttl(64);
        segment.ip_flags(IpFlags::DF);
        segment.ack_no(self.tcb.ack_no);
        segment.seq_no(self.tcb.seq_no);
        segment.tcp_flags(flags);
        segment.window_size(self.tcb.window_size);
        segment.tcp_options(&[]);
        segment.payload(payload);
        segment.build()?;

        self.sender.send(segment);

        // Track the segment for possible retransmission
        self.tcb
            .sent_segments
            .insert(self.tcb.seq_no, (payload.to_vec(), Instant::now()));

        // Start or reset the retransmission timer
        if self.tcb.sent_segments.len() == 1 {
            self.timer.start(self.tcb.rto)
        }

        // Update seq_no
        let bit_mask = flags & (TcpFlags::SYN | TcpFlags::FIN);
        if bit_mask.bits() != 0 {
            let bytes_consumed = 1 + payload.len() as u32;
            self.tcb.seq_no += Wrap32::new(bytes_consumed);
        }

        Ok(())
    }

    fn prune_old_segments(&mut self) {
        let ack_no = self.tcb.ack_no;
        self.tcb.sent_segments.retain(|&seq_no, _| seq_no >= ack_no);
    }

    fn prune_expired_segments(&mut self) {
        let now = Instant::now();
        let rto = self.tcb.rto;

        self.tcb
            .sent_segments
            .retain(|_, &mut (_, timestamp)| now.duration_since(timestamp) < rto);
    }
}
