use crate::packet::packet_error::PacketError;
use crate::socket::socket::TcpSocket;
use crate::socket::states::syn_rcvd::SynRcvd;
use crate::tcp::tcp_error::TcpError;
use crate::tcp::tcp_flags::TcpFlags;
use std::marker::PhantomData;

pub struct Listen;

impl TcpSocket<Listen> {
    pub fn accept(mut self) -> Result<TcpSocket<SynRcvd>, PacketError> {
        if let Some(segment) = self.recv_segment() {
            if segment.tcph.flags == TcpFlags::SYN {
                // Update remote endpoint info
                self.tcb.dst_ip = Some(segment.iph.src_ip);
                self.tcb.dst_port = Some(segment.tcph.dst_port);

                self.send_segment(TcpFlags::SYN | TcpFlags::ACK, None)?;

                self.timer.start(self.tcb.rto);

                return Ok(TcpSocket {
                    tcb: self.tcb,
                    sender: self.sender,
                    receiver: self.receiver,
                    timer: self.timer,
                    state: PhantomData,
                });
            }
        }

        Err(PacketError::Tcp(TcpError::WouldBlock))
    }
}
