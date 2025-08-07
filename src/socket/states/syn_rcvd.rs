use crate::packet::packet_error::PacketError;
use crate::socket::socket::TcpSocket;
use crate::socket::states::established::Established;
use crate::tcp::tcp_error::TcpError;
use crate::tcp::tcp_flags::TcpFlags;
use crate::tcp::wrap32::Wrap32;
use std::marker::PhantomData;

pub struct SynRcvd;

impl TcpSocket<SynRcvd> {
    pub fn confirm(mut self) -> Result<TcpSocket<Established>, PacketError> {
        if let Some(segment) = self.recv_segment() {
            if segment.tcph.flags.contains(TcpFlags::ACK) {
                // Validate the ACK number
                if segment.tcph.ack_no == self.tcb.seq_no + Wrap32::new(1) {
                    // Move to the Established state
                    self.tcb.ack_no = segment.tcph.seq_no + Wrap32::new(1);
                    self.tcb.seq_no = segment.tcph.ack_no;
                    self.tcb.window_size = segment.tcph.window;

                    // Send an ACK to confirm the connection
                    self.send_segment(TcpFlags::ACK, None)?;

                    // Cancel the timer
                    self.timer.cancel();

                    return Ok(TcpSocket {
                        tcb: self.tcb,
                        sender: self.sender,
                        receiver: self.receiver,
                        timer: self.timer,
                        state: PhantomData,
                    });
                }
            }
        }
        Err(PacketError::Tcp(TcpError::WouldBlock))
    }
}
