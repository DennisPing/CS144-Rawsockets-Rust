use std::marker::PhantomData;
use crate::tcp::errors::TcpError;
use crate::tcp::socket::states::established::Established;
use crate::tcp::socket::TcpSocket;
use crate::tcp::tcp_flags::TcpFlags;
use crate::tcp::wrap32::Wrap32;

pub struct SynRcvd;

impl TcpSocket<SynRcvd> {
    pub fn confirm(mut self) -> Result<TcpSocket<Established>, TcpError> {
        if let Some(segment) = self.receiver.recv() {
            if segment.tcph.flags.contains(TcpFlags::ACK) {
                // Validate the ACK number
                if segment.tcph.ack_no == self.tcb.seq_no + Wrap32::new(1) {
                    // Move to the Established state
                    self.tcb.ack_no = segment.tcph.seq_no + Wrap32::new(1);
                    self.tcb.seq_no = segment.tcph.ack_no;
                    self.tcb.window_size = segment.tcph.window;

                    // Send an ACK to confirm the connection
                    self.send_ack()?;

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
        Err(TcpError::WouldBlock)
    }
}