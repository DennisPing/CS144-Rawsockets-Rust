// use std::io;
// use std::io::Write;
// use std::net::Ipv4Addr;
// use crate::ip::flags::IPFlags;
// use crate::ip::header::IPHeader;
// use crate::packet;
// use crate::tcp::byte_stream::ByteStream;
// use crate::tcp::flags::TCPFlags;
// use crate::tcp::header::TCPHeader;
// use crate::tcp::wrap32::Wrap32;
// 
// /// The sender end of the `TcpConnection`
// #[derive(Debug)]
// pub struct TcpSender {
//     isn: Wrap32,            // Initial seq number
//     unacked_seq_no: Wrap32, // First unack'ed seq number
//     next_seq_no: Wrap32,    // Next seq number to send
//     stream: ByteStream,
//     reused_tcp: TCPHeader,
//     reused_ip: IPHeader,
// }
// 
// impl TcpSender {
//     pub fn new(isn: Wrap32, stream: ByteStream) -> Self {
//         TcpSender {
//             isn,
//             unacked_seq_no: isn,
//             next_seq_no: isn,
//             stream,
//             reused_tcp: TCPHeader{
//                 src_port: 0,
//                 dst_port: 0,
//                 seq_no: Wrap32::new(0),
//                 ack_no: Wrap32::new(0),
//                 data_offset: 0,
//                 reserved: 0,
//                 flags: TCPFlags::FIN,
//                 window: 0,
//                 checksum: 0,
//                 urgent: 0,
//                 options: Box::new([]),
//                 payload: Box::new([]),
//             },
//             reused_ip: IPHeader {
//                 version: 0,
//                 ihl: 0,
//                 tos: 0,
//                 total_len: 0,
//                 id: 0,
//                 flags: IPFlags::DF,
//                 frag_offset: 0,
//                 ttl: 0,
//                 protocol: 0,
//                 checksum: 0,
//                 src_ip: Ipv4Addr::new(0, 0, 0, 0),
//                 dst_ip: Ipv4Addr::new(0, 0, 0, 0),
//             }
//         }
//     }
// 
//     pub fn send(&mut self, data: &[u8]) -> io::Result<()> {
//         self.stream.write_all(data)?;
//         self.next_seq_no = self.next_seq_no + Wrap32::new(data.len() as u32);
//         Ok(())
//     }
// 
//     pub fn window_size(&self) -> usize {
//         self.stream.remaining_capacity()
//     }
// 
//     pub fn acknowledge(&mut self, ack_no: Wrap32) {
//         if ack_no > self.unacked_seq_no {
//             self.unacked_seq_no = ack_no;
//         }
//     }
// 
//     pub fn current_seq_no(&self) -> Wrap32 {
//         self.next_seq_no
//     }
// 
//     pub fn first_unacked_seq_no(&self) -> Wrap32 {
//         self.unacked_seq_no
//     }
// 
//     pub fn send_syn(&mut self) -> io::Result<()> {
//         let data = packet::pack(&self.reused_ip, &self.reused_tcp);
//         self.send(&data)
//     }
// }