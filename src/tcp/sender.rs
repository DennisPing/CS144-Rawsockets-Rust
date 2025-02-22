use std::collections::BTreeMap;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};
use crate::ip::ip_flags::IpFlags;
use crate::packet;
use crate::tcp::byte_stream::ByteStream;
use crate::tcp::errors::TcpError;
use crate::tcp::tcp_segment::TcpSegment;
use crate::tcp::tcp_flags::TcpFlags;
use crate::tcp::wrap32::Wrap32;

/// The sender end of the `TcpConnection`
#[derive(Debug)]
pub struct TcpSender {
    seq_no: Wrap32,                           // Current seq number
    pending_seq_no: Wrap32,                   // Next seq number to be acknowledged
    out_stream: ByteStream,                   // Stream of outgoing data
    tcp_builder: TcpSegment,                  // Message builder for outgoing data
    window_size: usize,                       // Advertised window from receiver
    sent_segments: BTreeMap<Wrap32, Vec<u8>>, // Sent but un'acked segments
}

// impl TcpSender {
//     pub fn new(isn: Wrap32, capacity: usize, src_ip: Ipv4Addr, src_port: u16, dst_ip: Ipv4Addr, dst_port: u16) -> Self {
//         TcpSender {
//             seq_no: isn,
//             pending_seq_no: isn + Wrap32::new(1),
//             out_stream: ByteStream::new(capacity),
//             tcp_builder: TcpSegment::new(src_ip, src_port, dst_ip, dst_port),
//             window_size: capacity,
//             sent_segments: BTreeMap::new(),
//         }
//     }
// 
//     pub fn window_size(&self) -> usize {
//         self.out_stream.remaining_capacity()
//     }
// 
//     pub fn update_ack(&mut self, ack_no: Wrap32, window_size: usize) -> Result<(), TcpError> {
//         if ack_no > self.pending_seq_no {
//             self.pending_seq_no = ack_no;
//             self.window_size = window_size;
// 
//             // Remove acknowledged segments from segments map
//             self.sent_segments.retain(|seq_no, _| *seq_no >= ack_no);
// 
//             Ok(())
//         } else {
//             Err(TcpError::InvalidAckNumber {
//                 expected: self.pending_seq_no,
//                 got: ack_no,
//             })
//         }
//     }
// 
//     pub fn set_seq_no(&mut self, seq_no: Wrap32) {
//         self.seq_no = seq_no;
//     }
// 
//     pub fn next_seq_no(&self) -> Wrap32 {
//         self.seq_no
//     }
// 
//     pub fn pending_seq_no(&self) -> Wrap32 {
//         self.pending_seq_no
//     }
// 
//     pub fn get_outgoing_stream(&self) -> &ByteStream {
//         &self.out_stream
//     }
//     
//     pub fn remaining_window(&self) -> usize {
//         self.window_size.saturating_sub(self.out_stream.bytes_written() - self.out_stream.bytes_read())
//     }
// 
//     pub fn send_syn(&mut self) -> Result<(), TcpError> {
//         let data = self.tcp_builder
//             .ttl(64)
//             .ip_flags(IpFlags::DF)
//             .seq_no(self.seq_no)
//             .ack_no(self.pending_seq_no)
//             .tcp_flags(TcpFlags::SYN)
//             .window_size(self.window_size as u16)
//             .tcp_options(&[])
//             .payload(&[])
//             .build()?;
//         
//         self.out_stream.write_all(data)?;
//         self.seq_no += Wrap32::new(1);
//         Ok(())
//     }
// 
//     pub fn send_ack(&mut self) -> Result<(), TcpError>  {
//         let data = self.tcp_builder
//             .ttl(64)
//             .ip_flags(IpFlags::DF)
//             .seq_no(self.seq_no)
//             .ack_no(self.pending_seq_no)
//             .tcp_flags(TcpFlags::ACK)
//             .window_size(self.window_size as u16)
//             .tcp_options(&[])
//             .payload(&[])
//             .build()?;
// 
//         self.out_stream.write_all(data)?;
//         self.seq_no += Wrap32::new(1);
//         Ok(())
//     }
// 
//     pub fn send_fin(&mut self) -> Result<(), TcpError>  {
//         self.tcph.flags = TcpFlags::FIN | TcpFlags::ACK;
//         self.tcph.seq_no = self.seq_no;
//         self.tcph.payload.clear();
// 
//         let n = packet::wrap_into(&self.iph, &self.tcph, &mut self.buffer)?;
//         self.out_stream.write_all(&self.buffer[..n])?;
//         self.seq_no += Wrap32::new(1);
//         Ok(())
//     }
// 
//     pub fn send_syn_ack(&mut self) -> Result<(), TcpError>  {
//         self.tcph.flags = TcpFlags::SYN | TcpFlags::ACK;
//         self.tcph.seq_no = self.seq_no;
//         self.tcph.payload.clear();
// 
//         let n = packet::wrap_into(&self.iph, &self.tcph, &mut self.buffer)?;
//         self.out_stream.write_all(&self.buffer[..n])?;
//         self.seq_no += Wrap32::new(1);
//         Ok(())
//     }
// 
//     pub fn send_payload(&mut self, payload: &[u8]) -> Result<(), TcpError> {
//         if payload.len() > self.remaining_window() {
//             return Err(TcpError::InvalidBuffer);
//         }
// 
//         self.tcph.flags = TcpFlags::ACK;
//         self.tcph.seq_no = self.seq_no;
//         self.tcph.payload.clear();
//         self.tcph.payload.extend_from_slice(payload);
// 
//         let n = packet::wrap_into(&self.iph, &self.tcph, &mut self.buffer)?;
//         self.out_stream.write_all(&self.buffer[..n])?;
// 
//         // Store payload into the sent segments map for retransmission if needed
//         self.sent_segments.insert(self.seq_no, payload.to_vec());
// 
//         self.seq_no += Wrap32::new(payload.len() as u32);
//         Ok(())
//     }
// }