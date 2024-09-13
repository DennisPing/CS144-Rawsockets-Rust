use crate::net::ip_flags::IPFlags;
use std::net::Ipv4Addr;

#[derive(Debug, Clone)]
pub struct IPHeader {
    pub version: u8, // Always 4 for IPv4
    pub ihl: u8,     // Always 5 since we have no options
    pub tos: u8,     // Always 0 when we send out, can be 8 when receiving from server
    pub total_len: u16,
    pub id: u16,
    pub flags: IPFlags,   // 3 bits, part of u16
    pub frag_offset: u16, // 13 bits, part of u16
    pub ttl: u8,          // Always 64 when we send out
    pub protocol: u8,     // Always 6 for TCP
    pub checksum: u16,
    pub src_ip: Ipv4Addr,
    pub dst_ip: Ipv4Addr,
}

impl IPHeader {
    /// Convert an `IPHeader` into a byte array of size 20.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![0u8; 20];

        buf[0] = (self.version << 4) | self.ihl;
        buf[1] = self.tos;
        buf[2..4].copy_from_slice(&self.total_len.to_be_bytes());
        buf[4..6].copy_from_slice(&self.id.to_be_bytes());
        let flags = self.flags.pack(self.frag_offset);
        buf[6..8].copy_from_slice(&flags.to_be_bytes());
        buf[8] = self.ttl;
        buf[9] = self.protocol;
        buf[10..12].copy_from_slice(&[0, 0]);
        buf[12..16].copy_from_slice(&self.src_ip.octets());
        buf[16..20].copy_from_slice(&self.dst_ip.octets());
        let checksum = Self::checksum(&buf);
        buf[10..12].copy_from_slice(&checksum.to_be_bytes());

        buf
    }

    /// Convert a byte array into an `IPHeader`.
    pub fn from_bytes(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 20 {
            return Err("Not enough bytes to parse IP header");
        }

        let version = data[0] >> 4;
        let ihl = data[0] & 0x0f;
        let tos = data[1];
        let tot_len = u16::from_be_bytes([data[2], data[3]]);
        let id = u16::from_be_bytes([data[4], data[5]]);
        let combo_flags = u16::from_be_bytes([data[6], data[7]]);
        let (flags, frag_offset) = IPFlags::unpack(combo_flags);
        let ttl = data[8];
        let protocol = data[9];
        let checksum = u16::from_be_bytes([data[10], data[11]]);
        let src_ip = Ipv4Addr::new(data[12], data[13], data[14], data[15]);
        let dst_ip = Ipv4Addr::new(data[16], data[17], data[18], data[19]);

        Ok(Self {
            version,
            ihl,
            tos,
            total_len: tot_len,
            id,
            flags,
            frag_offset,
            ttl,
            protocol,
            checksum,
            src_ip,
            dst_ip,
        })
    }

    /// Compute the checksum for an `IPHeader` (Ipv4).
    /// Wiki: https://en.wikipedia.org/wiki/IPv4_header_checksum.
    pub fn checksum(data: &[u8]) -> u16 {
        // Sum every 2 bytes as a 16-bit value
        let mut sum: u32 = data
            .chunks(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]) as u32)
            .sum();

        // Fold the carry bits
        while sum >> 16 != 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }

        !(sum as u16)
    }
}

// -- Unit tests --

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::test_utils;

    #[test]
    fn test_ip_header_to_bytes() {
        let header = IPHeader {
            version: 4,
            ihl: 5,
            tos: 0,
            total_len: 64,
            id: 0,
            flags: IPFlags::DF,
            frag_offset: 0,
            ttl: 64,
            protocol: 6,
            checksum: 54134,
            src_ip: Ipv4Addr::new(10, 110, 208, 106),
            dst_ip: Ipv4Addr::new(204, 44, 192, 60),
        };

        let data = header.to_bytes();

        // Verify that checksum is 0
        let checksum = IPHeader::checksum(&data);
        assert_eq!(checksum, 0);

        let ip_bytes = hex::decode(test_utils::get_ip_hex()).unwrap();
        assert_eq!(data, ip_bytes.as_slice());
    }

    #[test]
    fn test_ip_header_from_bytes() {
        let ip_bytes = hex::decode(test_utils::get_ip_hex()).unwrap();
        let iph = IPHeader::from_bytes(&ip_bytes).unwrap();

        assert_eq!(iph.version, 4);
        assert_eq!(iph.ihl, 5);
        assert_eq!(iph.tos, 0);
        assert_eq!(iph.total_len, 64);
        assert_eq!(iph.id, 0);
        assert_eq!(iph.flags, IPFlags::DF);
        assert_eq!(iph.frag_offset, 0);
        assert_eq!(iph.ttl, 64);
        assert_eq!(iph.protocol, 6);
        assert_eq!(iph.checksum, 54134);
        assert_eq!(iph.src_ip, Ipv4Addr::new(10, 110, 208, 106));
        assert_eq!(iph.dst_ip, Ipv4Addr::new(204, 44, 192, 60));
    }
}
