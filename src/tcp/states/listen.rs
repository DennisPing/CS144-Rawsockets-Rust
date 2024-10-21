// use std::marker::PhantomData;
// use crate::tcp::conn::TcpConn;
// use crate::tcp::states::syn_rcvd::SynRcvd;
// 
// pub struct Listen;
// 
// impl TcpConn<Listen> {
//     pub fn accept(mut self) -> TcpConn<SynRcvd> {
//         // TODO: Wait for SYN, send SYN-ACK
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