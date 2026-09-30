use crate::common::checksum::ip_checksum;
use crate::ip::datagram::{IpDatagram, IpProtocol, IPV4, IP_HDR_SIZE};
use crate::ip::flags::IpFlags;
use crate::tcp::view::TcpView;
use crate::tcp::WireError;
use std::net::Ipv4Addr;

/// Borrowed view of an IpDatagram. The "read-only" side.
#[derive(Clone, Copy)]
pub struct IpView<'a> {
    data: &'a [u8],    // The IP datagram (header + payload)
    header_len: usize, // Cached header length
    total_len: usize,
}

impl<'a> IpView<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, WireError> {
        if data.len() < IP_HDR_SIZE {
            return Err(WireError::Truncated {
                needed: IP_HDR_SIZE,
                got: data.len(),
            });
        }

        // Check IP Version = 4
        let version = data[0] >> 4;
        if version != IPV4 {
            return Err(WireError::IpVersionNotSupported(version));
        }

        // Check IHL >= 5
        let ihl = data[0] & 0x0f;
        if ihl < 5 {
            return Err(WireError::InvalidHeader);
        }

        // Check header length
        let header_len = (ihl as usize) * 4;
        if data.len() < header_len {
            return Err(WireError::Truncated {
                needed: header_len,
                got: data.len(),
            });
        }

        // Validate checksum
        if ip_checksum(&data[..header_len]) != 0 {
            return Err(WireError::BadChecksum);
        }

        // Check total length
        let total_len = u16::from_be_bytes([data[2], data[3]]) as usize;
        if total_len < header_len {
            return Err(WireError::InvalidHeader);
        }
        if data.len() < total_len {
            return Err(WireError::Truncated {
                needed: total_len,
                got: data.len(),
            });
        }

        Ok(Self { data, header_len, total_len })
    }

    /// IP version (always 4 for us)
    #[inline]
    pub fn version(&self) -> u8 {
        self.data[0] >> 4
    }

    /// Internet header length as 32-bit words (min value is 5)
    ///
    /// Example: 5 words * 32 bits = 160 bits / 8 = 20 bytes long
    #[inline]
    pub fn ihl(&self) -> u8 {
        self.data[0] & 0x0f
    }

    /// Header length only
    #[inline]
    pub fn header_len(&self) -> usize {
        self.header_len
    }

    /// Type of service (usually 0)
    #[inline]
    pub fn tos(&self) -> u8 {
        self.data[1]
    }

    /// Total length of the IP datagram (header + payload)
    #[inline]
    pub fn total_len(&self) -> u16 {
        u16::from_be_bytes([self.data[2], self.data[3]])
    }

    /// ID for fragmentation
    #[inline]
    pub fn id(&self) -> u16 {
        u16::from_be_bytes([self.data[4], self.data[5]])
    }

    /// IP Flags
    #[inline]
    pub fn flags(&self) -> IpFlags {
        IpFlags::unpack(u16::from_be_bytes([self.data[6], self.data[7]]))
    }

    /// Fragment offset in 8 byte units
    #[inline]
    pub fn frag_offset(&self) -> u16 {
        u16::from_be_bytes([self.data[6], self.data[7]]) & 0x1fff
    }

    /// Time to live
    #[inline]
    pub fn ttl(&self) -> u8 {
        self.data[8]
    }

    /// IP protocol
    #[inline]
    pub fn protocol(&self) -> IpProtocol {
        IpProtocol::from(self.data[9])
    }

    /// IP checksum
    #[inline]
    pub fn checksum(&self) -> u16 {
        u16::from_be_bytes([self.data[10], self.data[11]])
    }

    /// Source address
    #[inline]
    pub fn src(&self) -> Ipv4Addr {
        Ipv4Addr::new(self.data[12], self.data[13], self.data[14], self.data[15])
    }

    /// Destination address
    #[inline]
    pub fn dst(&self) -> Ipv4Addr {
        Ipv4Addr::new(self.data[16], self.data[17], self.data[18], self.data[19])
    }

    /// IP options (if any). Empty slice if IHL = 5
    #[inline]
    pub fn options(&self) -> &'a [u8] {
        &self.data[IP_HDR_SIZE..self.header_len]
    }

    /// Slices to exact IP payload (automatically trims any trailing Ethernet frame padding).
    #[inline]
    pub fn payload(&self) -> &'a [u8] {
        &self.data[self.header_len..self.total_len() as usize]
    }

    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        &self.data[..self.total_len() as usize]
    }

    /// Parse the payload directly as a TCP segment view with checksum validation.
    pub fn tcp(&self) -> Result<TcpView<'a>, WireError> {
        match self.protocol() {
            IpProtocol::Tcp => TcpView::parse(self.payload(), self.src(), self.dst()),
            _ => Err(WireError::ProtocolNotSupported(self.protocol() as u8)),
        }
    }

    pub fn to_owned(&self) -> IpDatagram {
        IpDatagram::from_view(self)
    }
}

impl<'a> From<&IpView<'a>> for IpDatagram {
    fn from(view: &IpView<'a>) -> Self {
        IpDatagram::from_view(view)
    }
}

impl<'a> From<IpView<'a>> for IpDatagram {
    fn from(view: IpView<'a>) -> Self {
        IpDatagram::from_view(&view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::wireshark;

    #[test]
    fn test_ip_view_parse() {
        let ip_bytes = wireshark::hex_concat(&[wireshark::ip_hex(), wireshark::tcp_hex()]);
        let view = IpView::parse(&ip_bytes).unwrap();

        assert_eq!(view.version(), 4);
        assert_eq!(view.header_len(), 20);
        assert_eq!(view.total_len(), 64);
        assert_eq!(view.protocol(), IpProtocol::Tcp);
        assert_eq!(view.src(), Ipv4Addr::new(10, 110, 208, 106));
        assert_eq!(view.dst(), Ipv4Addr::new(204, 44, 192, 60));
    }

    #[test]
    fn test_ip_view_to_tcp() {
        let raw_bytes = wireshark::hex_concat(&[
            wireshark::ip_with_payload_hex(),
            wireshark::tcp_with_payload_hex(),
            wireshark::giant_payload_hex(),
        ]);

        let ip_view = IpView::parse(&raw_bytes).unwrap();
        let tcp_view = ip_view.tcp().unwrap();

        assert_eq!(tcp_view.src_port(), 80);
        assert_eq!(tcp_view.dst_port(), 50871);
        assert!(tcp_view.is_ack());
        assert!(tcp_view.has_payload());
    }
}
