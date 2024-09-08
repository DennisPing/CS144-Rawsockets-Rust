use crate::rawsocket::header_ip::IPHeader;
use bitflags::bitflags;
use std::f32::consts::E;
use std::net::Ipv4Addr;
use std::vec;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    struct TCPFlags: u8 {
        const CWR = 0b0000_0001;
        const ECE = 0b0000_0010;
        const URG = 0b0000_0100;
        const ACK = 0b0000_1000;
        const PSH = 0b0001_0000;
        const RST = 0b0010_0000;
        const SYN = 0b0100_0000;
        const FIN = 0b1000_0000;
    }
}

#[derive(Debug, Clone)]
struct TCPHeader {
    src_port: u16,
    dst_port: u16,
    seq_num: u32,
    ack_num: u32,
    data_offset: u8, // Upper 4 bits
    reserved: u8,    // Lower 4 bits
    flags: TCPFlags,
    window: u16,
    checksum: u16,
    urgent: u16,
    options: Vec<u8>,
}

impl TCPHeader {
    fn to_bytes(&self, ip: &IPHeader, payload: &[u8]) -> Vec<u8> {
        let header_len = self.data_offset as usize * 4;
        let mut buf = vec![0u8; header_len + payload.len()];

        buf[0..2].copy_from_slice(&self.src_port.to_be_bytes());
        buf[2..4].copy_from_slice(&self.dst_port.to_be_bytes());
        buf[4..8].copy_from_slice(&self.seq_num.to_be_bytes());
        buf[8..12].copy_from_slice(&self.ack_num.to_be_bytes());
        buf[12] = (self.data_offset << 4) | self.data_offset;
        buf[13] = self.flags.bits();
        buf[14..16].copy_from_slice(&self.window.to_be_bytes());
        buf[18..20].copy_from_slice(&self.urgent.to_be_bytes());
        buf[20..20 + self.options.len()].copy_from_slice(&self.options);
        buf[header_len..].copy_from_slice(payload);
        let checksum = Self::checksum(&buf, ip);
        buf[16..18].copy_from_slice(&checksum.to_be_bytes());

        buf
    }

    fn from_bytes(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 20 {
            return Err("Not enough bytes to parse TCP header");
        }

        let src_port = u16::from_be_bytes([data[0], data[1]]);
        let dst_port = u16::from_be_bytes([data[2], data[3]]);
        let seq_num = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let ack_num = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let data_offset = data[12] >> 4;
        let reserved = data[12] & 0x0F;
        let flags = TCPFlags::from_bits_truncate(data[13]);
        let window = u16::from_be_bytes([data[14], data[15]]);
        let checksum = u16::from_be_bytes([data[16], data[17]]);
        let urgent = u16::from_be_bytes([data[18], data[19]]);
        let options = data[20..(data_offset as usize * 4)].to_vec();

        Ok(Self {
            src_port,
            dst_port,
            seq_num,
            ack_num,
            data_offset,
            reserved,
            flags,
            window,
            checksum,
            urgent,
            options,
        })
    }

    fn checksum(tcp_bytes: &[u8], ip: &IPHeader, payload: &[u8]) -> u16 {
        0
    }
}
