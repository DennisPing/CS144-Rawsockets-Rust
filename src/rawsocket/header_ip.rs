use bitflags::bitflags;
use std::net::Ipv4Addr;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct IPFlags: u16 {
        const RF = 0b1000000000000000; // Reserved Flag
        const DF = 0b0100000000000000; // Don't Fragment
        const MF = 0b0010000000000000; // More Fragments
    }
}

impl IPFlags {
    /// Pack the flags and fragment offset into a single u16
    fn pack(self, frag_offset: u16) -> u16 {
        self.bits() | (frag_offset & 0x1fff)
    }

    /// Unpack the flags and fragment offset from a single u16
    fn unpack(bits: u16) -> (Self, u16) {
        let top3 = Self::from_bits_truncate(bits & 0xe000);
        let bottom13 = bits & 0x1fff;
        (top3, bottom13)
    }
}

#[derive(Debug, Copy, Clone)]
struct IPHeader {
    version: u8, // Always 4
    ihl: u8,     // Always 5 since we have no options
    tos: u8,     // Always 0 when we send out, can be 8 when receiving from server
    tot_len: u16,
    id: u16,
    flags: IPFlags,   // 3 bits, part of u16
    frag_offset: u16, // 13 bits, part of u16
    ttl: u8,          // Always 64 when we send out
    protocol: u8,     // Always 6 for TCP
    checksum: u16,
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
}

impl IPHeader {
    /// Convert the IPHeader to a byte array.
    fn to_bytes(&self) -> [u8; 20] {
        let mut buf = [0u8; 20];
        buf[0] = (self.version << 4) | self.ihl;
        buf[1] = self.tos;
        buf[2..4].copy_from_slice(&self.tot_len.to_be_bytes());
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

    fn from_bytes(packet: &[u8]) -> Result<Self, &'static str> {
        if packet.len() < 20 {
            return Err("Packet is too short for IPv4 header");
        }

        let version = packet[0] >> 4;
        let ihl = packet[0] & 0x0f;
        let tos = packet[1];
        let tot_len = u16::from_be_bytes([packet[2], packet[3]]);
        let id = u16::from_be_bytes([packet[4], packet[5]]);
        let combo_flags = u16::from_be_bytes([packet[6], packet[7]]);
        let (flags, frag_offset) = IPFlags::unpack(combo_flags);
        let ttl = packet[8];
        let protocol = packet[9];
        let checksum = u16::from_be_bytes([packet[10], packet[11]]);
        let src_ip = Ipv4Addr::new(packet[12], packet[13], packet[14], packet[15]);
        let dst_ip = Ipv4Addr::new(packet[16], packet[17], packet[18], packet[19]);

        Ok(Self {
            version,
            ihl,
            tos,
            tot_len,
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

    /// Compute the checksum for an IPv4 header.
    /// Wiki: https://en.wikipedia.org/wiki/IPv4_header_checksum.
    fn checksum(data: &[u8]) -> u16 {
        // Sum every 2 bytes as a 16-bit value
        let mut sum: u32 = data
            .chunks(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]) as u32)
            .sum();

        // Fold the carry bits
        while sum > 0xffff {
            sum = (sum & 0xffff) + (sum >> 16);
        }

        !(sum as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_header_to_bytes() {
        let header = IPHeader {
            version: 4,
            ihl: 5,
            tos: 0,
            tot_len: 64,
            id: 0,
            flags: IPFlags::DF,
            frag_offset: 0,
            ttl: 64,
            protocol: 6,
            checksum: 54134,
            src_ip: Ipv4Addr::new(10, 110, 208, 106),
            dst_ip: Ipv4Addr::new(204, 44, 192, 60),
        };

        let packet = header.to_bytes();

        // Verify that checksum is 0
        let checksum = IPHeader::checksum(&packet);
        assert_eq!(checksum, 0);

        let wireshark_hex = "45000040000040004006d3760a6ed06acc2cc03c";
        let wireshark_bytes = hex::decode(wireshark_hex).unwrap();
        assert_eq!(packet, wireshark_bytes.as_slice());
    }

    #[test]
    fn test_ip_header_from_bytes() {
        let wireshark_hex = "45000040000040004006d3760a6ed06acc2cc03c";
        let wireshark_bytes = hex::decode(wireshark_hex).unwrap();
        let header = IPHeader::from_bytes(&wireshark_bytes).unwrap();

        assert_eq!(header.version, 4);
        assert_eq!(header.ihl, 5);
        assert_eq!(header.tos, 0);
        assert_eq!(header.tot_len, 64);
        assert_eq!(header.id, 0);
        assert_eq!(header.flags, IPFlags::DF);
        assert_eq!(header.frag_offset, 0);
        assert_eq!(header.ttl, 64);
        assert_eq!(header.protocol, 6);
        assert_eq!(header.checksum, 54134);
        assert_eq!(header.src_ip, Ipv4Addr::new(10, 110, 208, 106));
        assert_eq!(header.dst_ip, Ipv4Addr::new(204, 44, 192, 60));
    }

    #[test]
    fn test_ip_flags() {
        assert_eq!(IPFlags::RF.bits(), 0b1000000000000000);
        assert_eq!(IPFlags::DF.bits(), 0b0100000000000000);
        assert_eq!(IPFlags::MF.bits(), 0b0010000000000000);

        let combined = IPFlags::RF | IPFlags::DF | IPFlags::MF;
        assert_eq!(combined.bits(), 0b1110000000000000);
    }
}
