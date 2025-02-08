use std::marker::PhantomData;
use crate::tcp::conn::TcpConn;
use crate::tcp::errors::TcpError;
use crate::tcp::states::syn_rcvd::SynRcvd;

pub struct Listen;

impl TcpConn<Listen> {
    pub fn accept(mut self) -> Result<TcpConn<SynRcvd>, TcpError> {
        // Implement logic to wait for SYN and validate it
        // For now, just send SYN-ACK back
        self.sender.send_syn_ack()?;

        Ok(TcpConn {
            sender: self.sender,
            receiver: self.receiver,
            state: PhantomData,
        })
    }
}