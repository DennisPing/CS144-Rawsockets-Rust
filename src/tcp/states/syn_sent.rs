use std::marker::PhantomData;
use crate::tcp::conn::TcpConn;
use crate::tcp::errors::TcpError;
use crate::tcp::states::established::Established;
use crate::tcp::wrap32::Wrap32;

pub struct SynSent;

impl TcpConn<SynSent> {
    pub fn handle_syn_ack(mut self, ack_no: Wrap32) -> Result<TcpConn<Established>, TcpError> {
        let next_seq_no = self.sender.pending_seq_no();
        if next_seq_no != ack_no {
            return Err(TcpError::InvalidAckNumber {
                expected: next_seq_no,
                got: ack_no,
            });
        }
        
        // Update the ack number based on conditions here

        Ok(TcpConn {
            sender: self.sender,
            receiver: self.receiver,
            state: PhantomData,
        })
    }
}