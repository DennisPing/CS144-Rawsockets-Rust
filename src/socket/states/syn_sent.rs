use crate::packet::packet_error::PacketError;
use crate::socket::socket::TcpSocket;
use crate::socket::states::established::Established;
use crate::tcp::tcp_error::TcpError;
use crate::tcp::tcp_flags::TcpFlags;
use std::marker::PhantomData;

pub struct SynSent;

impl TcpSocket<SynSent> {
    pub fn on_segment(mut self) -> Result<TcpSocket<Established>, PacketError> {
        if let Some(segment) = self.recv_segment() {
            if segment.tcph.flags == (TcpFlags::SYN | TcpFlags::ACK) {
                self.send_segment(TcpFlags::ACK, None)?;

                return Ok(TcpSocket {
                    tcb: self.tcb,
                    sender: self.sender,
                    receiver: self.receiver,
                    timer: self.timer,
                    state: PhantomData,
                });
            }
        }
        Err(PacketError::Tcp(TcpError::InvalidState("Expected SYN|ACK")))
    }
}
