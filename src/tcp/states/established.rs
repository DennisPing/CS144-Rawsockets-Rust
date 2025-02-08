use std::marker::PhantomData;
use crate::tcp::conn::TcpConn;
use crate::tcp::errors::TcpError;
use crate::tcp::states::fin_wait1::FinWait1;
use crate::tcp::wrap32::Wrap32;

pub struct Established;

impl TcpConn<Established> {
    pub fn close(mut self) -> Result<TcpConn<FinWait1>, TcpError>{
        self.sender.send_fin()?;
        Ok(TcpConn {
            sender: self.sender,
            receiver: self.receiver,
            state: PhantomData,
        })
    }

    /// Maybe this method should be enhanced. In real life it would be called a lot.
    pub fn send(&mut self, data: &[u8]) -> Result<(), TcpError> {
        self.sender.send_payload(data)?;
        Ok(())
    }

    pub fn handle_rst(mut self, rst_seq_no: Wrap32) -> Result<(), TcpError> {
        // Immediately close the connection
        // Clean up resources, transition to closed state, etc
        Err(TcpError::ConnectionReset)
    }
}