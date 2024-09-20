use crate::net::ip_header::IPHeader;
use crate::net::tcp_flags::TCPFlags;
use std::io::{Error, ErrorKind};
use std::vec;

#[derive(Debug, Clone)]
pub struct TCPHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset: u8, // Upper 4 bits
    pub reserved: u8,    // Lower 4 bits
    pub flags: TCPFlags,
    pub window: u16,
    pub checksum: u16,
    pub urgent: u16,
    pub options: Vec<u8>,
    pub payload: Vec<u8>, // Append payload to end of TCP header for ease of use
}

impl TCPHeader {
    /// Convert a `TCPHeader` into a byte vector.
    pub fn to_bytes(&self, ip: &IPHeader) -> Vec<u8> {
        let header_len = self.data_offset as usize * 4;
        let mut buf = vec![0u8; header_len + self.payload.len()];

        buf[0..2].copy_from_slice(&self.src_port.to_be_bytes());
        buf[2..4].copy_from_slice(&self.dst_port.to_be_bytes());
        buf[4..8].copy_from_slice(&self.seq_num.to_be_bytes());
        buf[8..12].copy_from_slice(&self.ack_num.to_be_bytes());
        buf[12] = (self.data_offset << 4) | self.reserved;
        buf[13] = self.flags.bits();
        buf[14..16].copy_from_slice(&self.window.to_be_bytes());
        // leave 16..18 as zeros for checksum
        buf[18..20].copy_from_slice(&self.urgent.to_be_bytes());
        buf[20..20 + self.options.len()].copy_from_slice(&self.options);
        buf[header_len..].copy_from_slice(&self.payload);

        let checksum = Self::checksum(&buf, ip);
        buf[16..18].copy_from_slice(&checksum.to_be_bytes());

        buf
    }

    /// Convert a byte vector into a `TCPHeader`.
    pub fn from_bytes(data: &[u8]) -> Result<Self, Error> {
        if data.len() < 20 {
            return Err(Error::from(ErrorKind::InvalidData));
        }

        let src_port = u16::from_be_bytes([data[0], data[1]]);
        let dst_port = u16::from_be_bytes([data[2], data[3]]);
        let seq_num = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let ack_num = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let data_offset = data[12] >> 4;
        let reserved = data[12] & 0x0f;
        let flags = TCPFlags::from_bits_truncate(data[13]);
        let window = u16::from_be_bytes([data[14], data[15]]);
        let checksum = u16::from_be_bytes([data[16], data[17]]);
        let urgent = u16::from_be_bytes([data[18], data[19]]);

        let header_len = data_offset as usize * 4;
        let options = data[20..header_len].to_vec();
        let payload = data[header_len..].to_vec();

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
            payload,
        })
    }

    /// Compute the checksum for a `TCPHeader`.
    pub fn checksum(tcp_bytes: &[u8], iph: &IPHeader) -> u16 {
        let mut sum = 0u32;

        // Add source IP to pseudo header
        let src_bytes = iph.src_ip.octets();
        sum += ((src_bytes[0] as u32) << 8) | (src_bytes[1] as u32);
        sum += ((src_bytes[2] as u32) << 8) | (src_bytes[3] as u32);

        // Add destination IP to pseudo header
        let dst_bytes = iph.dst_ip.octets();
        sum += ((dst_bytes[0] as u32) << 8) | (dst_bytes[1] as u32);
        sum += ((dst_bytes[2] as u32) << 8) | (dst_bytes[3] as u32);

        // Add protocol and segment length
        sum += iph.protocol as u32;
        sum += (tcp_bytes.len()) as u32;

        // Sum the TCP Header and payload
        for i in (0..tcp_bytes.len()).step_by(2) {
            sum += ((tcp_bytes[i] as u32) << 8) | (tcp_bytes[i + 1] as u32);
        }

        // If odd length, add the last byte
        if tcp_bytes.len() % 2 != 0 {
            sum += (tcp_bytes[tcp_bytes.len() - 1] as u32) << 8;
        }

        // Fold the carry bits
        while sum >> 16 != 0 {
            sum = (sum & 0xffff) + (sum >> 16)
        }

        !sum as u16
    }
}

// -- Unit tests --

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::test_utils;

    #[test]
    fn test_tcp_header_to_bytes() {
        let tcp_header = TCPHeader {
            src_port: 50871,
            dst_port: 80,
            seq_num: 2753993875,
            ack_num: 0,
            data_offset: 11,
            reserved: 0,
            flags: TCPFlags::SYN,
            window: 65535,
            checksum: 37527,
            urgent: 0,
            options: hex::decode("020405b4010303060101080abb6879f80000000004020000").unwrap(),
            payload: vec![],
        };

        // Get the IP header in order to build TCP header
        let ip_bytes = hex::decode(test_utils::get_ip_hex()).unwrap();
        let iph = IPHeader::from_bytes(ip_bytes.as_slice()).unwrap();
        let data = tcp_header.to_bytes(&iph);

        // Verify that checksum is 0
        let checksum = TCPHeader::checksum(&data, &iph);
        assert_eq!(checksum, 0);

        // Check that constructed data is equal to wireshark data
        let tcp_bytes = hex::decode(test_utils::get_tcp_hex()).unwrap();
        assert_eq!(data, tcp_bytes.as_slice())
    }

    #[test]
    fn test_tcp_header_from_bytes() {
        let tcp_bytes = hex::decode(test_utils::get_tcp_hex()).unwrap();
        let tcph = TCPHeader::from_bytes(&tcp_bytes).unwrap();

        assert_eq!(tcph.src_port, 50871);
        assert_eq!(tcph.dst_port, 80);
        assert_eq!(tcph.seq_num, 2753993875);
        assert_eq!(tcph.ack_num, 0);
        assert_eq!(tcph.data_offset, 11);
        assert_eq!(tcph.reserved, 0);
        assert_eq!(tcph.flags, TCPFlags::SYN);
        assert_eq!(tcph.window, 65535);
        assert_eq!(tcph.checksum, 37527);
        assert_eq!(tcph.urgent, 0);
        assert_eq!(
            tcph.options,
            hex::decode("020405b4010303060101080abb6879f80000000004020000").unwrap()
        );
        assert_eq!(tcph.payload, &[])
    }
}
