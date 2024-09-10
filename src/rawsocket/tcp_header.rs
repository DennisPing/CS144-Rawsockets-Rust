use std::vec;
use crate::rawsocket::ip_header::IPHeader;
use crate::rawsocket::tcp_flags::TCPFlags;

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
        buf[12] = (self.data_offset << 4) | self.reserved;
        buf[13] = self.flags.bits();
        buf[14..16].copy_from_slice(&self.window.to_be_bytes());
        buf[16..18].copy_from_slice(&[0, 0]);
        buf[18..20].copy_from_slice(&self.urgent.to_be_bytes());
        buf[20..20 + self.options.len()].copy_from_slice(&self.options);
        buf[header_len..].copy_from_slice(payload);
        let checksum = Self::checksum(&buf, ip, payload);
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
        let reserved = data[12] & 0x0f;
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
        let mut sum = 0u32;

        // Add source IP to pseudo header
        let src_bytes = ip.src_ip.octets();
        sum += ((src_bytes[0] as u32) << 8) | (src_bytes[1] as u32);
        sum += ((src_bytes[2] as u32) << 8) | (src_bytes[3] as u32);

        // Add destination IP to pseudo header
        let dst_bytes = ip.dst_ip.octets();
        sum += ((dst_bytes[0] as u32) << 8) | (dst_bytes[1] as u32);
        sum += ((dst_bytes[2] as u32) << 8) | (dst_bytes[3] as u32);

        // Add protocol and segment length
        sum += ip.protocol as u32;
        sum += (tcp_bytes.len() + payload.len()) as u32;

        // Sum the TCP Header
        for i in (0..tcp_bytes.len()).step_by(2) {
            if i + 1 < tcp_bytes.len() {
                sum += ((tcp_bytes[i] as u32) << 8) | (tcp_bytes[i+1] as u32);
            } else {
                sum += (tcp_bytes[i] as u32) << 8;
            }
        }

        // Sum the payload
        for i in (0..payload.len()).step_by(2) {
            if i + 1 < payload.len() {
                sum += ((payload[i] as u32) << 8) | (payload[i+1] as u32);
            } else {
                sum += (payload[i] as u32) << 8;
            }
        }

        // Fold the carry bits
        while sum >> 16 != 0 {
            sum = (sum & 0xffff) + (sum >> 16)
        }

        !sum as u16
    }
}

// Unit tests *****************************************************************

#[cfg(test)]
mod tests {
    use crate::rawsocket::test_utils;
    use super::*;
    
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
        };

        // Get the IP header in order to build TCP header
        let ip_bytes = hex::decode(test_utils::get_ip_hex()).unwrap();
        let ip_header = IPHeader::from_bytes(ip_bytes.as_slice()).unwrap();
        let data = tcp_header.to_bytes(&ip_header, &[]);

        // Verify that checksum is 0
        let checksum = TCPHeader::checksum(&data, &ip_header, &[]);
        assert_eq!(checksum, 0);

        // Check that constructed data is equal to wireshark data
        let tcp_bytes = hex::decode(test_utils::get_tcp_hex()).unwrap();
        assert_eq!(data, tcp_bytes.as_slice())
    }

    #[test]
    fn test_tcp_header_from_bytes() {
        let tcp_bytes = hex::decode(test_utils::get_tcp_hex()).unwrap();
        let tcp_header = TCPHeader::from_bytes(&tcp_bytes).unwrap();

        assert_eq!(tcp_header.src_port, 50871);
        assert_eq!(tcp_header.dst_port, 80);
        assert_eq!(tcp_header.seq_num, 2753993875);
        assert_eq!(tcp_header.ack_num, 0);
        assert_eq!(tcp_header.data_offset, 11);
        assert_eq!(tcp_header.reserved, 0);
        assert_eq!(tcp_header.flags, TCPFlags::SYN);
        assert_eq!(tcp_header.window, 65535);
        assert_eq!(tcp_header.checksum, 37527);
        assert_eq!(tcp_header.urgent, 0);
        assert_eq!(tcp_header.options, hex::decode("020405b4010303060101080abb6879f80000000004020000").unwrap());
    }
    
    
}