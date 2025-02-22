use std::collections::HashMap;
use std::marker::PhantomData;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;
use rand::random;
use crate::tcp::errors::TcpError;
use crate::tcp::socket::{Receiver, Sender, TcpControlBlock, TcpSocket, Timer};
use crate::tcp::socket::states::listen::Listen;
use crate::tcp::socket::states::syn_sent::SynSent;
use crate::tcp::wrap32::Wrap32;

pub struct Closed;

impl TcpSocket<Closed> {
    pub fn new(
        sender: Box<dyn Sender>,
        receiver: Box<dyn Receiver>,
        timer: Box<dyn Timer>,
        src_ip: Ipv4Addr,
        src_port: u16,
    ) -> Self {
        let sender_isn = Wrap32::new(random::<u32>());
        let receiver_isn = Wrap32::new(random::<u32>());
        TcpSocket {
            tcb: TcpControlBlock {
                seq_no: sender_isn,
                ack_no: receiver_isn,
                window_size: 1024, // Default window size; configurable as needed.
                rto: Duration::from_secs(1),
                src_ip,
                src_port,
                dst_ip: None,
                dst_port: None,
                sent_segments: HashMap::new(),
                last_seq_no: None,
            },
            sender,
            receiver,
            timer,
            state: PhantomData,

        }
    }

    /// Connect to a peer by initiating the 3-way handshake
    pub fn connect(mut self, dst_ip: Ipv4Addr, dst_port: u16) -> Result<TcpSocket<SynSent>, TcpError> {
        self.tcb.dst_ip = Some(dst_ip);
        self.tcb.dst_port = Some(dst_port);

        self.send_syn()?;

        Ok(TcpSocket {
            tcb: self.tcb,
            sender: self.sender,
            receiver: self.receiver,
            timer: self.timer,
            state: PhantomData,
        })
    }

    /// Listen for an incoming connection
    pub fn listen(self) -> TcpSocket<Listen> {
        TcpSocket {
            tcb: self.tcb,
            sender: self.sender,
            receiver: self.receiver,
            timer: self.timer,
            state: PhantomData,
        }
    }
}