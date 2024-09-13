use crate::net::{IPHeader, TCPHeader};

/// Pack an `IPHeader` and `TCPHeader` into a byte vector.
pub fn pack(iph: &IPHeader, tcph: &TCPHeader) -> Vec<u8> {
    // Allocate entire vector
    let mut packet = vec![0u8; iph.total_len as usize];

    packet[0..20].copy_from_slice(&iph.to_bytes());
    packet[20..].copy_from_slice(&tcph.to_bytes(iph));
    packet
}

/// Unpack a byte vector into an `IPHeader` and `TCPHeader`.
pub fn unpack(packet: &[u8]) -> Result<(IPHeader, TCPHeader), &'static str> {
    if packet.len() < 20 {
        return Err("Incomplete TCP/IP packet");
    }

    let iph_bytes = &packet[0..20];
    if IPHeader::checksum(&iph_bytes) != 0 {
        return Err("IP checksum failed");
    }

    let iph = IPHeader::from_bytes(iph_bytes)?;

    let tcp_bytes = &packet[20..];
    if TCPHeader::checksum(tcp_bytes, &iph) != 0 {
        return Err("TCP checksum failed");
    }

    let tcph = TCPHeader::from_bytes(&tcp_bytes)?;
    Ok((iph, tcph))
}

// -- Unit tests --

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::ip_flags::IPFlags;
    use crate::net::tcp_flags::TCPFlags;
    use crate::net::test_utils::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_pack() {
        let ip_bytes = hex::decode(get_ip_hex_with_payload()).unwrap();
        let tcp_bytes = hex::decode(get_tcp_hex_with_payload()).unwrap();
        let payload = hex::decode(giant_payload()).unwrap();

        let iph = IPHeader {
            version: 4,
            ihl: 5,
            tos: 0,
            total_len: 1426,
            id: 17988,
            flags: IPFlags::DF,
            frag_offset: 0,
            ttl: 42,
            protocol: 6,
            checksum: 40416,
            src_ip: Ipv4Addr::new(204, 44, 192, 60),
            dst_ip: Ipv4Addr::new(10, 110, 208, 106),
        };

        let tcph = TCPHeader {
            src_port: 80,
            dst_port: 50871,
            seq_num: 1654659911,
            ack_num: 2753994376,
            data_offset: 8,
            reserved: 0,
            flags: TCPFlags::ACK,
            window: 235,
            checksum: 29098,
            urgent: 0,
            options: hex::decode("0101080abeb95f0abb687a45").unwrap(),
            payload: payload.clone(),
        };

        let packet = pack(&iph, &tcph);
        let expected = [ip_bytes, tcp_bytes, payload].concat();
        assert_eq!(expected, packet);
    }

    #[test]
    fn test_unpack() {
        let ip_bytes = hex::decode(get_ip_hex_with_payload()).unwrap();
        let tcp_bytes = hex::decode(get_tcp_hex_with_payload()).unwrap();
        let payload = hex::decode(giant_payload()).unwrap();

        let packet = [ip_bytes, tcp_bytes, payload.clone()].concat();
        let result = unpack(&packet);
        assert!(result.is_ok());
        let (iph, tcph) = result.unwrap();

        assert_eq!(iph.version, 4);
        assert_eq!(iph.ihl, 5);
        assert_eq!(iph.tos, 0);
        assert_eq!(iph.total_len, 1426);
        assert_eq!(iph.id, 17988);
        assert_eq!(iph.flags, IPFlags::DF);
        assert_eq!(iph.frag_offset, 0);
        assert_eq!(iph.ttl, 42);
        assert_eq!(iph.protocol, 6);
        assert_eq!(iph.checksum, 40416);
        assert_eq!(iph.src_ip, Ipv4Addr::new(204, 44, 192, 60));
        assert_eq!(iph.dst_ip, Ipv4Addr::new(10, 110, 208, 106));

        assert_eq!(tcph.src_port, 80);
        assert_eq!(tcph.dst_port, 50871);
        assert_eq!(tcph.seq_num, 1654659911);
        assert_eq!(tcph.ack_num, 2753994376);
        assert_eq!(tcph.data_offset, 8);
        assert_eq!(tcph.reserved, 0);
        assert_eq!(tcph.flags, TCPFlags::ACK);
        assert_eq!(tcph.window, 235);
        assert_eq!(tcph.checksum, 29098);
        assert_eq!(tcph.urgent, 0);
        assert_eq!(
            tcph.options,
            hex::decode("0101080abeb95f0abb687a45").unwrap()
        );
        assert_eq!(tcph.payload, payload)
    }
}
