use crate::ip::ip_header::IPHeader;
use crate::tcp::tcp_flags::TCPFlags;
use crate::packet::errors::HeaderError;
use crate::tcp::wrap32::Wrap32;

#[derive(Debug, Clone, PartialEq)]
pub struct TCPHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_no: Wrap32,
    pub ack_no: Wrap32,
    pub data_offset: u8, // Upper 4 bits
    pub reserved: u8,    // Lower 4 bits
    pub flags: TCPFlags,
    pub window: u16,
    pub checksum: u16,
    pub urgent: u16,
    pub options: Vec<u8>,
    pub payload: Vec<u8>, // Append payload to end of TCP header for ease of use
}

impl Default for TCPHeader {
    fn default() -> Self {
        TCPHeader {
            src_port: 0,
            dst_port: 0,
            seq_no: Wrap32::new(0),
            ack_no: Wrap32::new(0),
            data_offset: 0,
            reserved: 0,
            flags: TCPFlags::ACK,
            window: 0,
            checksum: 0,
            urgent: 0,
            options: vec![],
            payload: vec![],
        }
    }
}

impl TCPHeader {
    /// Convert a `TCPHeader` into a byte vector.
    pub fn serialize(&self, buf: &mut [u8], iph: &IPHeader) -> Result<usize, HeaderError> {
        let header_len = self.data_offset as usize * 4; // 20 + options
        let total_len = header_len + self.payload.len(); // 20 + options + payload

        if buf.len() < total_len {
            return Err(HeaderError::BufferTooSmall { expected: total_len, found: buf.len() })
        }

        buf[0..2].copy_from_slice(&self.src_port.to_be_bytes());
        buf[2..4].copy_from_slice(&self.dst_port.to_be_bytes());
        buf[4..8].copy_from_slice(&self.seq_no.value().to_be_bytes());
        buf[8..12].copy_from_slice(&self.ack_no.value().to_be_bytes());
        buf[12] = (self.data_offset << 4) | self.reserved;
        buf[13] = self.flags.bits();
        buf[14..16].copy_from_slice(&self.window.to_be_bytes());
        buf[16..18].fill(0); // Set checksum to 0 initially
        buf[18..20].copy_from_slice(&self.urgent.to_be_bytes());

        if !self.options.is_empty() {
            buf[20..header_len].copy_from_slice(&self.options);
        }

        if !self.payload.is_empty() {
            buf[header_len..total_len].copy_from_slice(&self.payload);
        }

        let checksum = Self::checksum(&buf[..total_len], iph);
        buf[16..18].copy_from_slice(&checksum.to_be_bytes());

        Ok(total_len)
    }

    /// Convert a byte vector into a `TCPHeader`.
    pub fn parse(buf: &[u8], iph: &IPHeader) -> Result<Self, HeaderError> {
        if buf.len() < 20 {
            return Err(HeaderError::BufferTooSmall { expected: 20, found: buf.len() })
        }

        let src_port = u16::from_be_bytes([buf[0], buf[1]]);
        let dst_port = u16::from_be_bytes([buf[2], buf[3]]);
        let seq_no = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let ack_no = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let data_offset = buf[12] >> 4;
        let reserved = buf[12] & 0x0f;
        let flags = TCPFlags::from_bits_truncate(buf[13]);
        let window = u16::from_be_bytes([buf[14], buf[15]]);
        let checksum = u16::from_be_bytes([buf[16], buf[17]]);
        let urgent = u16::from_be_bytes([buf[18], buf[19]]);

        let header_len = data_offset as usize * 4;
        if buf.len() < header_len {
            return Err(HeaderError::BufferTooSmall { expected: header_len, found: buf.len() })
        }

        let options = if header_len > 20 {
            buf[20..header_len].to_vec()
        } else {
            Vec::new()
        };

        let payload = if buf.len() > header_len {
            buf[header_len..].to_vec()
        } else {
            Vec::new()
        };

        if Self::checksum(&buf[..(header_len + payload.len())], iph) != 0 {
            return Err(HeaderError::BadChecksum("TCP".to_string()))
        }

        Ok(TCPHeader {
            src_port,
            dst_port,
            seq_no: Wrap32::new(seq_no),
            ack_no: Wrap32::new(ack_no),
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
    pub fn checksum(data: &[u8], iph: &IPHeader) -> u16 {
        let mut sum: u32 = 0;

        // Pseudo-header
        let src_bytes = iph.src_ip.octets();
        let dst_bytes = iph.dst_ip.octets();

        sum += ((src_bytes[0] as u32) << 8) | (src_bytes[1] as u32);
        sum += ((src_bytes[2] as u32) << 8) | (src_bytes[3] as u32);
        sum += ((dst_bytes[0] as u32) << 8) | (dst_bytes[1] as u32);
        sum += ((dst_bytes[2] as u32) << 8) | (dst_bytes[3] as u32);

        // Add protocol and TCP segment length
        sum += iph.protocol as u32;
        sum += data.len() as u32;

        // Sum the TCP Header and payload
        for i in (0..data.len() - 1).step_by(2) {
            sum += ((data[i] as u32) << 8) | (data[i + 1] as u32);
        }

        // If odd length, add the last byte
        if data.len() % 2 != 0 {
            sum += (data[data.len() - 1] as u32) << 8;
        }

        // Fold the carry bits
        let folded = (sum & 0xffff) + (sum >> 16);
        !folded as u16
    }
}

// -- Unit tests --

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::test_utils;

    #[test]
    fn test_tcp_header_to_bytes() {
        let tcp_header = TCPHeader {
            src_port: 50871,
            dst_port: 80,
            seq_no: Wrap32::new(2753993875),
            ack_no: Wrap32::new(0),
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
        let iph = IPHeader::parse(&ip_bytes).unwrap();
        let mut buf = vec![0u8; 1024];
        let n = tcp_header.serialize(&mut buf, &iph).unwrap();

        // Verify that checksum is 0
        let checksum = TCPHeader::checksum(&buf[..n], &iph);
        assert_eq!(checksum, 0);

        // Check that constructed data is equal to wireshark data
        let tcp_bytes = hex::decode(test_utils::get_tcp_hex()).unwrap();
        assert_eq!(buf[..n], tcp_bytes)
    }

    #[test]
    fn test_tcp_header_from_bytes() {
        let ip_bytes = hex::decode(test_utils::get_ip_hex()).unwrap();
        let iph = IPHeader::parse(&ip_bytes).unwrap();

        let tcp_bytes = hex::decode(test_utils::get_tcp_hex()).unwrap();
        let tcph = TCPHeader::parse(&tcp_bytes, &iph).unwrap();

        assert_eq!(tcph.src_port, 50871);
        assert_eq!(tcph.dst_port, 80);
        assert_eq!(tcph.seq_no, Wrap32::new(2753993875));
        assert_eq!(tcph.ack_no, Wrap32::new(0));
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
        assert_eq!(tcph.payload, [])
    }
}
