use std::marker::PhantomData;
use std::net::{IpAddr, Ipv4Addr};
use crate::tcp::errors::TcpError;
use crate::tcp::socket::states::syn_rcvd::SynRcvd;
use crate::tcp::socket::TcpSocket;
use crate::tcp::tcp_flags::TcpFlags;

pub struct Listen;

impl TcpSocket<Listen> {
    pub fn accept(mut self) -> Result<TcpSocket<SynRcvd>, TcpError> {
        if let Some(segment) = self.receiver.recv() {
            if segment.tcph.flags.contains(TcpFlags::SYN) {
                // Update remote endpoint info
                self.tcb.dst_ip = Some(Ipv4Addr::from(segment.iph.src_ip));
                self.tcb.dst_port = Some(segment.tcph.dst_port);

                self.send_syn_ack()?;

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

        Err(TcpError::WouldBlock)
    }
}