use std::marker::PhantomData;
use std::net::Ipv4Addr;
use rand::random;
use crate::tcp::conn::TcpConn;
use crate::tcp::errors::TcpError;
use crate::tcp::receiver::TcpReceiver;
use crate::tcp::sender::TcpSender;
use crate::tcp::states::listen::Listen;
use crate::tcp::states::syn_sent::SynSent;
use crate::tcp::wrap32::Wrap32;

pub struct Closed;

impl TcpConn<Closed> {
    pub fn new(src_ip: Ipv4Addr, src_port: u16, dst_ip: Ipv4Addr,  dst_port: u16) -> Self {
        let sender_isn = Wrap32::new(random::<u32>());
        let receiver_isn = Wrap32::new(random::<u32>());
        TcpConn {
            sender: TcpSender::new(sender_isn, 4096, src_ip, src_port, dst_ip, dst_port),
            receiver: TcpReceiver::new(receiver_isn, 4096),
            state: PhantomData,
        }
    }

    /// Connect to a peer by initiating the 3-way handshake
    pub fn connect(mut self) -> Result<TcpConn<SynSent>, TcpError> {
        let x = random::<u32>();
        self.sender.set_seq_no(Wrap32::new(x));
        self.sender.send_syn()?;

        Ok(TcpConn {
            sender: self.sender,
            receiver: self.receiver,
            state: PhantomData,
        })
    }

    /// Listen for an incoming connection
    pub fn listen(self) -> TcpConn<Listen> {
        TcpConn {
            sender: self.sender,
            receiver: self.receiver,
            state: PhantomData,
        }
    }
}

// impl Default for TcpConn<Closed> {
//     fn default() -> Self {
//         TcpConn {
//             sender: TcpSender::new(Wrap32::new(0), 4096),
//             receiver: TcpReceiver::new(Wrap32::new(0), 4096),
//             state: PhantomData,
//         }
//     }
// }