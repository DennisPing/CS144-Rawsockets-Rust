use crate::ip::ip_header::IpHeader;
use crate::tcp::tcp_header::TcpHeader;
use crate::packet::errors::HeaderError;

/// Wrap an `IPHeader` and `TCPHeader` into a packet. Zero allocation.
pub fn wrap_into(iph: &IpHeader, tcph: &TcpHeader, packet: &mut [u8]) -> Result<usize, HeaderError> {
    let ip_len = iph.serialize(&mut packet[0..20])?;
    let tcp_length = tcph.serialize(&mut packet[20..], iph)?;
    Ok(ip_len + tcp_length)
}

/// Wrap an `IPHeader` and `TCPHeader` into a packet. Allocs a new `Vec<u8>` for convenience.
pub fn wrap(iph: &IpHeader, tcph: &TcpHeader) -> Result<Vec<u8>, HeaderError> {
    let tcp_len = tcph.data_offset as usize * 4 + tcph.payload.len();
    let total_len = 20 + tcp_len;
    let mut packet = vec![0u8; total_len];

    wrap_into(iph, tcph, &mut packet)?;
    Ok(packet)
}

/// Unwrap a packet into `IPHeader` and `TCPHeader` objects. Zero allocation.
pub fn unwrap_from(packet: &[u8], iph: &mut IpHeader, tcph: &mut TcpHeader) -> Result<usize, HeaderError> {
    let parsed_iph = IpHeader::parse(&packet[0..20])?;
    let total_len = parsed_iph.total_len as usize;
    *iph = parsed_iph;

    let parsed_tcph = TcpHeader::parse(&packet[20..total_len], iph)?;
    *tcph = parsed_tcph;

    Ok(total_len)
}

/// Unpack a byte vector into an `IPHeader` and `TCPHeader`. Allocs new headers for convenience.
pub fn unwrap(packet: &[u8]) -> Result<(IpHeader, TcpHeader), HeaderError> {
    let mut iph = IpHeader::default();
    let mut tcph = TcpHeader::default();

    unwrap_from(packet, &mut iph, &mut tcph)?;
    Ok((iph, tcph))
}

// -- Unit tests --

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ip::ip_flags::IpFlags;
    use crate::packet::test_utils;
    use crate::tcp::tcp_flags::TcpFlags;
    use std::net::Ipv4Addr;
    use crate::tcp::wrap32::Wrap32;

    #[test]
    fn test_pack() {
        let ip_bytes = hex::decode(test_utils::get_ip_hex_with_payload()).unwrap();
        let tcp_bytes = hex::decode(test_utils::get_tcp_hex_with_payload()).unwrap();
        let payload = hex::decode(test_utils::giant_payload()).unwrap();

        let iph = IpHeader {
            version: 4,
            ihl: 5,
            tos: 0,
            total_len: 1426,
            id: 17988,
            flags: IpFlags::DF,
            frag_offset: 0,
            ttl: 42,
            protocol: 6,
            checksum: 40416,
            src_ip: Ipv4Addr::new(204, 44, 192, 60),
            dst_ip: Ipv4Addr::new(10, 110, 208, 106),
        };

        let tcph = TcpHeader {
            src_port: 80,
            dst_port: 50871,
            seq_no: Wrap32::new(1654659911),
            ack_no: Wrap32::new(2753994376),
            data_offset: 8,
            reserved: 0,
            flags: TcpFlags::ACK,
            window: 235,
            checksum: 29098,
            urgent: 0,
            options: hex::decode("0101080abeb95f0abb687a45").unwrap(),
            payload: payload.clone(),
        };

        let packet = wrap(&iph, &tcph).unwrap();
        let expected = [ip_bytes, tcp_bytes, payload].concat();
        assert_eq!(expected, packet);
    }

    #[test]
    fn test_unpack() {
        let ip_bytes = hex::decode(test_utils::get_ip_hex_with_payload()).unwrap();
        let tcp_bytes = hex::decode(test_utils::get_tcp_hex_with_payload()).unwrap();
        let payload = hex::decode(test_utils::giant_payload()).unwrap();

        let packet = [ip_bytes, tcp_bytes, payload.clone()].concat();
        let result = unwrap(&packet);
        assert!(result.is_ok());

        let (iph, tcph) = result.unwrap();
        assert_eq!(iph.version, 4);
        assert_eq!(iph.ihl, 5);
        assert_eq!(iph.tos, 0);
        assert_eq!(iph.total_len, 1426);
        assert_eq!(iph.id, 17988);
        assert_eq!(iph.flags, IpFlags::DF);
        assert_eq!(iph.frag_offset, 0);
        assert_eq!(iph.ttl, 42);
        assert_eq!(iph.protocol, 6);
        assert_eq!(iph.checksum, 40416);
        assert_eq!(iph.src_ip, Ipv4Addr::new(204, 44, 192, 60));
        assert_eq!(iph.dst_ip, Ipv4Addr::new(10, 110, 208, 106));

        assert_eq!(tcph.src_port, 80);
        assert_eq!(tcph.dst_port, 50871);
        assert_eq!(tcph.seq_no, Wrap32::new(1654659911));
        assert_eq!(tcph.ack_no, Wrap32::new(2753994376));
        assert_eq!(tcph.data_offset, 8);
        assert_eq!(tcph.reserved, 0);
        assert_eq!(tcph.flags, TcpFlags::ACK);
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
        let result = unwrap(&packet);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err, HeaderError::BadChecksum("IP".to_string()));
    }

    #[test]
    fn test_unpack_corrupt_tcph() {
        let ip_bytes = hex::decode(test_utils::get_ip_hex_with_payload()).unwrap();
        let mut tcp_bytes = hex::decode(test_utils::get_tcp_hex_with_payload()).unwrap();
        tcp_bytes[10] = 0xff; // Corrupt a byte
        let payload = hex::decode(test_utils::giant_payload()).unwrap();

        let packet = [ip_bytes, tcp_bytes, payload.clone()].concat();
        let result = unwrap(&packet);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err, HeaderError::BadChecksum("TCP".to_string()));
    }

    // Difficult as fuck
    #[test]
    fn test_odd_tcp_segment_length() {
        let payload = hex::decode(test_utils::giant_payload_odd()).unwrap();

        let iph = IpHeader {
            version: 4,
            ihl: 5,
            tos: 0x20,
            total_len: 845,
            id: 21169,
            flags: IpFlags::DF,
            frag_offset: 0,
            ttl: 38,
            protocol: 6,
            checksum: 45243,
            src_ip: Ipv4Addr::new(204, 44, 192, 60),
            dst_ip: Ipv4Addr::new(192, 168, 1, 13),
        };

        let tcph = TcpHeader {
            src_port: 80,
            dst_port: 47652,
            seq_no: Wrap32::new(3280096596),
            ack_no: Wrap32::new(1563085193),
            data_offset: 8,
            reserved: 0,
            flags: TcpFlags::ACK | TcpFlags::PSH,
            window: 235,
            checksum: 47864,
            urgent: 0,
            options: hex::decode("0101080afdc076540198f657").unwrap(),
            payload,
        };

        let packet = wrap(&iph, &tcph).unwrap();
        let result = unwrap(&packet);
        assert!(result.is_ok());

        let (iph2, tcph2) = result.unwrap();
        assert_eq!(iph.checksum, iph2.checksum);
        assert_eq!(tcph.checksum, tcph2.checksum);
    }
}
