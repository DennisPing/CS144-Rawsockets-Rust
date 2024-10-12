use crate::ip::header::IPHeader;
use crate::tcp::header::TCPHeader;
use std::io::{Error, ErrorKind};

/// Pack an `IPHeader` and `TCPHeader` into a byte vector.
pub fn pack(iph: &IPHeader, tcph: &TCPHeader) -> Vec<u8> {
    // Allocate entire vector
    let mut packet = vec![0u8; iph.total_len as usize];

    packet[0..20].copy_from_slice(&iph.to_bytes());
    packet[20..].copy_from_slice(&tcph.to_bytes(iph));
    packet
}

/// Unpack a byte vector into an `IPHeader` and `TCPHeader`.
pub fn unpack(packet: &[u8]) -> Result<(IPHeader, TCPHeader), Error> {
    if packet.len() < 20 {
        return Err(Error::from(ErrorKind::InvalidData));
    }

    let iph_bytes = &packet[0..20];
    if IPHeader::checksum(iph_bytes) != 0 {
        return Err(Error::new(ErrorKind::Other, "Bad IP checksum"));
    }

    let iph = IPHeader::from_bytes(iph_bytes);

    let tcp_bytes = &packet[20..];
    if TCPHeader::checksum(tcp_bytes, &iph) != 0 {
        return Err(Error::new(ErrorKind::Other, "Bad TCP checksum"));
    }

    let tcph = TCPHeader::from_bytes(tcp_bytes);
    Ok((iph, tcph))
}

// -- Unit tests --

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ip::flags::IPFlags;
    use crate::packet::test_utils;
    use crate::tcp::flags::TCPFlags;
    use std::net::Ipv4Addr;

    #[test]
    fn test_pack() {
        let ip_bytes = hex::decode(test_utils::get_ip_hex_with_payload()).unwrap();
        let tcp_bytes = hex::decode(test_utils::get_tcp_hex_with_payload()).unwrap();
        let payload = hex::decode(test_utils::giant_payload()).unwrap();

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
            options: Box::from(hex::decode("0101080abeb95f0abb687a45").unwrap()),
            payload: Box::from(payload.clone()),
        };

        let packet = pack(&iph, &tcph);
        let expected = [ip_bytes, tcp_bytes, payload].concat();
        assert_eq!(expected, packet);
    }

    #[test]
    fn test_unpack() {
        let ip_bytes = hex::decode(test_utils::get_ip_hex_with_payload()).unwrap();
        let tcp_bytes = hex::decode(test_utils::get_tcp_hex_with_payload()).unwrap();
        let payload = hex::decode(test_utils::giant_payload()).unwrap();

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
            *tcph.options,
            hex::decode("0101080abeb95f0abb687a45").unwrap()
        );
        assert_eq!(*tcph.payload, payload)
    }

    #[test]
    fn test_unpack_corrupt_iph() {
        let mut ip_bytes = hex::decode(test_utils::get_ip_hex_with_payload()).unwrap();
        ip_bytes[10] = 0xff; // Corrupt a byte
        let tcp_bytes = hex::decode(test_utils::get_tcp_hex_with_payload()).unwrap();
        let payload = hex::decode(test_utils::giant_payload()).unwrap();

        let packet = [ip_bytes, tcp_bytes, payload.clone()].concat();
        let result = unpack(&packet);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Other);
        assert_eq!(err.into_inner().unwrap().to_string(), "Bad IP checksum")
    }

    #[test]
    fn test_unpack_corrupt_tcph() {
        let ip_bytes = hex::decode(test_utils::get_ip_hex_with_payload()).unwrap();
        let mut tcp_bytes = hex::decode(test_utils::get_tcp_hex_with_payload()).unwrap();
        tcp_bytes[10] = 0xff; // Corrupt a byte
        let payload = hex::decode(test_utils::giant_payload()).unwrap();

        let packet = [ip_bytes, tcp_bytes, payload.clone()].concat();
        let result = unpack(&packet);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Other);
        assert_eq!(err.into_inner().unwrap().to_string(), "Bad TCP checksum")
    }

    // Difficult as fuck
    #[test]
    fn test_odd_tcp_segment_length() {
        let payload = hex::decode(test_utils::giant_payload_odd()).unwrap();

        let iph = IPHeader {
            version: 4,
            ihl: 5,
            tos: 0x20,
            total_len: 845,
            id: 21169,
            flags: IPFlags::DF,
            frag_offset: 0,
            ttl: 38,
            protocol: 6,
            checksum: 45243,
            src_ip: Ipv4Addr::new(204, 44, 192, 60),
            dst_ip: Ipv4Addr::new(192, 168, 1, 13),
        };

        let tcph = TCPHeader {
            src_port: 80,
            dst_port: 47652,
            seq_num: 3280096596,
            ack_num: 1563085193,
            data_offset: 8,
            reserved: 0,
            flags: TCPFlags::ACK | TCPFlags::PSH,
            window: 235,
            checksum: 47864,
            urgent: 0,
            options: Box::from(hex::decode("0101080afdc076540198f657").unwrap()),
            payload: Box::from(payload),
        };

        let packet = pack(&iph, &tcph);
        let result = unpack(&packet);
        assert!(result.is_ok());

        let (iph2, tcph2) = result.unwrap();
        assert_eq!(iph.checksum, iph2.checksum);
        assert_eq!(tcph.checksum, tcph2.checksum);
    }
}
