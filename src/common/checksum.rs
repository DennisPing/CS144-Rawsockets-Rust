use std::net::Ipv4Addr;
use crate::ip::datagram::IpProtocol;

/// IPv4 header checksum: one's complement sum over 16-bit words.
/// Returns 0 for a valid header (when run over a header with its checksum field filled).
pub fn ip_checksum(ip_bytes: &[u8]) -> u16 {
    ones_complement_sum(0, ip_bytes)
}

/// TCP checksum using IPv4 pseudo-header.
///
/// The pseudo-header is prepended virtually for checksum calculation
/// but never transmitted on the wire.
pub fn tcp_checksum(src: Ipv4Addr, dst: Ipv4Addr, protocol: IpProtocol, tcp_bytes: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    // Pseudo-header
    let src = src.octets();
    let dst = dst.octets();

    sum += u16::from_be_bytes([src[0], src[1]]) as u32;
    sum += u16::from_be_bytes([src[2], src[3]]) as u32;
    sum += u16::from_be_bytes([dst[0], dst[1]]) as u32;
    sum += u16::from_be_bytes([dst[2], dst[3]]) as u32;
    sum += u16::from_be_bytes([0, protocol as u8]) as u32;
    sum += tcp_bytes.len() as u32;

    ones_complement_sum(sum, tcp_bytes)
}

/// One's complement sum over 16-bit words with carry folding.
fn ones_complement_sum(mut sum: u32, data: &[u8]) -> u16 {
    for chunk in data.chunks(2) {
        if chunk.len() == 2 {
            sum += u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
        } else {
            sum += u16::from_be_bytes([chunk[0], 0]) as u32;
        }
    }

    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }

    !(sum as u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_tcp_checksum_validation() {
        // From Wireshark capture - a valid TCP SYN segment
        let tcp_bytes = hex::decode(
            "c6b70050a4269c9300000000b002ffff92970000020405b4010303060101080abb6879f80000000004020000"
        ).unwrap();

        let src = Ipv4Addr::new(10, 110, 208, 106);
        let dst = Ipv4Addr::new(204, 44, 192, 60);

        // Valid segment should checksum to 0
        assert_eq!(tcp_checksum(src, dst, IpProtocol::Tcp, &tcp_bytes), 0);
    }

    #[test]
    fn test_ipv4_header_checksum_validation() {
        // From Wireshark capture - a valid IP header
        let ip_hdr = hex::decode("45000040000040004006d3760a6ed06acc2cc03c").unwrap();

        // Valid header should checksum to 0
        assert_eq!(ip_checksum(&ip_hdr), 0);
    }
}
