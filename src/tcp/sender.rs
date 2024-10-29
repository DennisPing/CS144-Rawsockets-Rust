use std::io;
use std::io::Write;
use crate::ip::ip_header::IpHeader;
use crate::packet;
use crate::tcp::byte_stream::ByteStream;
use crate::tcp::tcp_header::TcpHeader;
use crate::tcp::wrap32::Wrap32;

/// The sender end of the `TcpConnection`
#[derive(Debug)]
pub struct TcpSender {
    isn: Wrap32,            // Initial seq number
    unacked_seq_no: Wrap32, // First unack'ed seq number
    next_seq_no: Wrap32,    // Next seq number to send
    stream: ByteStream,
    reused_tcp: TcpHeader,
    reused_ip: IpHeader,
}

impl TcpSender {
    pub fn new(isn: Wrap32, stream: ByteStream) -> Self {
        TcpSender {
            isn,
            unacked_seq_no: isn,
            next_seq_no: isn,
            stream,
            reused_tcp: TcpHeader::default(),
            reused_ip: IpHeader::default(),
        }
    }

    pub fn send(&mut self, data: &[u8]) -> io::Result<()> {
        self.stream.write_all(data)?;
        self.next_seq_no = self.next_seq_no + Wrap32::new(data.len() as u32);
        Ok(())
    }

    pub fn window_size(&self) -> usize {
        self.stream.remaining_capacity()
    }

    pub fn acknowledge(&mut self, ack_no: Wrap32) {
        if ack_no > self.unacked_seq_no {
            self.unacked_seq_no = ack_no;
        }
    }

    pub fn current_seq_no(&self) -> Wrap32 {
        self.next_seq_no
    }

    pub fn first_unacked_seq_no(&self) -> Wrap32 {
        self.unacked_seq_no
    }

    pub fn send_syn(&mut self) -> io::Result<()> {
        let data = packet::wrap(&self.reused_ip, &self.reused_tcp).unwrap();
        self.send(&data)
    }
}