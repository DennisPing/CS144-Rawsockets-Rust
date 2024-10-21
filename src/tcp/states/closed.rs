// use std::marker::PhantomData;
// use crate::tcp::conn::TcpConn;
// use crate::tcp::receiver::TcpReceiver;
// use crate::tcp::sender::TcpSender;
// use crate::tcp::states::listen::Listen;
// use crate::tcp::states::syn_sent::SynSent;
// 
// pub struct Closed;
// 
// impl TcpConn<Closed> {
//     pub fn new(sender: TcpSender, receiver: TcpReceiver) -> Self {
//         TcpConn {
//             sender,
//             receiver,
//             state: PhantomData,
//         }
//     }
// 
//     /// Listen passively for an incoming connection
//     pub fn listen(self) -> TcpConn<Listen> {
//         TcpConn {
//             sender: self.sender,
//             receiver: self.receiver,
//             state: PhantomData,
//         }
//     }
// 
//     /// Initiate the 3-way handshake
//     pub fn connect(mut self) -> TcpConn<SynSent> {
//         // TODO: Send SYN segment using TcpSender
//         let payload: Vec<u8> = Vec::new();
//         let result = self.sender.send(&payload);
// 
//         TcpConn {
//             sender: self.sender,
//             receiver: self.receiver,
//             state: PhantomData,
//         }
//     }
// }