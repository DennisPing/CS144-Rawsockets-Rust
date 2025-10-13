use crate::ip::ip_flags::IpFlags;
use crate::ip::pseudoheader::PseudoHeader;
use crate::tcp::tcp_checksum::ipv4_header_checksum;
use crate::tcp::tcp_error::BuildError;
use std::net::Ipv4Addr;

pub const IPV4: u8 = 4;
pub const IPPROTO_TCP: u8 = 6;
pub const IPPROTO_UDP: u8 = 17;
pub const IP_HDR_MIN_SIZE: usize = 20;
pub const DEFAULT_TTL: u8 = 64;

/// An IPv4 datagram with builder-style construction.
///
/// # Construction
///
/// Use `new()` for minimal construction or chain builder methods:
///
/// ```ignore
/// let dgram = IpDatagram::new(src, dst)
///     .ttl(128)
///     .payload(tcp_bytes);
/// ```
///
/// # Serialization
///
/// ```ignore
/// let bytes = dgram.to_bytes();
/// // or into existing buffer:
/// let len = dgram.encode_into(&mut buf)?;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpDatagram {
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub tos: u8,
    pub id: u16,
    pub flags: IpFlags,
    pub frag_offset: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub payload: Vec<u8>,
}

impl IpDatagram {
    /// Create a new datagram with required addresses.
    ///
    /// Defaults: TTL=64, protocol=TCP, flags=DF, empty payload.
    pub fn new(src: Ipv4Addr, dst: Ipv4Addr) -> Self {
        Self {
            src,
            dst,
            tos: 0,
            id: 0,
            flags: IpFlags::DF,
            frag_offset: 0,
            ttl: DEFAULT_TTL,
            protocol: IPPROTO_TCP,
            payload: Vec::new(),
        }
    }

    /// Create an IPv4 datagram carrying a TCP segment.
    pub fn tcp(src: Ipv4Addr, dst: Ipv4Addr, segment: &crate::tcp::TcpSegment) -> Self {
        let payload = segment.to_bytes(src, dst);
        Self::new(src, dst).payload(payload)
    }

    // ─────────────────────────────────────────────────────────────
    // Builder methods (consume self, return self)
    // ─────────────────────────────────────────────────────────────

    pub fn src(mut self, src: Ipv4Addr) -> Self {
        self.src = src;
        self
    }

    pub fn dst(mut self, dst: Ipv4Addr) -> Self {
        self.dst = dst;
        self
    }

    pub fn tos(mut self, tos: u8) -> Self {
        self.tos = tos;
        self
    }

    pub fn id(mut self, id: u16) -> Self {
        self.id = id;
        self
    }

    pub fn flags(mut self, flags: IpFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn frag_offset(mut self, offset: u16) -> Self {
        self.frag_offset = offset & 0x1FFF;
        self
    }

    pub fn ttl(mut self, ttl: u8) -> Self {
        self.ttl = ttl;
        self
    }

    pub fn protocol(mut self, protocol: u8) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn payload(mut self, data: Vec<u8>) -> Self {
        self.payload = data;
        self
    }

    pub fn payload_from_slice(mut self, data: &[u8]) -> Self {
        self.payload = data.to_vec();
        self
    }

    // ─────────────────────────────────────────────────────────────
    // Derived values
    // ─────────────────────────────────────────────────────────────

    /// Total length of the serialized datagram (header + payload).
    pub fn wire_len(&self) -> usize {
        IP_HDR_MIN_SIZE + self.payload.len()
    }

    /// Create a pseudo-header for TCP/UDP checksum calculation.
    pub fn pseudo_header(&self) -> PseudoHeader {
        PseudoHeader::new(self.src, self.dst, self.protocol, self.payload.len() as u16)
    }

    // ─────────────────────────────────────────────────────────────
    // From parsed view
    // ─────────────────────────────────────────────────────────────

    /// Create an owned datagram from a parsed view.
    pub fn from_view(view: &super::ip_datagram_view::IpView<'_>) -> Self {
        Self {
            src: view.src(),
            dst: view.dst(),
            tos: view.tos(),
            id: view.id(),
            flags: view.flags(),
            frag_offset: view.frag_offset(),
            ttl: view.ttl(),
            protocol: view.protocol(),
            payload: view.payload().to_vec(),
        }
    }

    // ─────────────────────────────────────────────────────────────
    // Serialization
    // ─────────────────────────────────────────────────────────────

    /// Serialize to a new `Vec<u8>`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![0u8; self.wire_len()];
        self.encode_into(&mut buf).expect("buffer sized correctly");
        buf
    }

    /// Serialize the 20-byte IPv4 header for a given total length.
    pub fn encode_header_only(&self, buf: &mut [u8], total_len: usize) -> Result<usize, BuildError> {
        if buf.len() < IP_HDR_MIN_SIZE {
            return Err(BuildError::BufferTooSmall {
                needed: IP_HDR_MIN_SIZE,
                got: buf.len(),
            });
        }

        // Version (4) + IHL (5)
        buf[0] = (IPV4 << 4) | 5;
        buf[1] = self.tos;
        buf[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());
        buf[4..6].copy_from_slice(&self.id.to_be_bytes());
        buf[6..8].copy_from_slice(&self.flags.pack(self.frag_offset).to_be_bytes());
        buf[8] = self.ttl;
        buf[9] = self.protocol;
        buf[10..12].fill(0); // Checksum placeholder
        buf[12..16].copy_from_slice(&self.src.octets());
        buf[16..20].copy_from_slice(&self.dst.octets());

        // Compute and insert header checksum
        let checksum = ipv4_header_checksum(&buf[..IP_HDR_MIN_SIZE]);
        buf[10..12].copy_from_slice(&checksum.to_be_bytes());

        Ok(IP_HDR_MIN_SIZE)
    }

    /// Serialize into an existing buffer. Returns bytes written.
    pub fn encode_into(&self, buf: &mut [u8]) -> Result<usize, BuildError> {
        let total_len = self.wire_len();

        if buf.len() < total_len {
            return Err(BuildError::BufferTooSmall {
                needed: total_len,
                got: buf.len(),
            });
        }

        self.encode_header_only(&mut buf[..IP_HDR_MIN_SIZE], total_len)?;

        // Copy payload
        buf[IP_HDR_MIN_SIZE..total_len].copy_from_slice(&self.payload);

        Ok(total_len)
    }
}

impl Default for IpDatagram {
    fn default() -> Self {
        Self {
            src: Ipv4Addr::UNSPECIFIED,
            dst: Ipv4Addr::UNSPECIFIED,
            tos: 0,
            id: 0,
            flags: IpFlags::DF,
            frag_offset: 0,
            ttl: DEFAULT_TTL,
            protocol: IPPROTO_TCP,
            payload: Vec::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ip::ip_datagram_view::IpView;
    use crate::tcp::wireshark_sample;
    use crate::tcp::TcpSegment;

    #[test]
    fn test_ip_datagram_encode() {
        let src = Ipv4Addr::new(10, 110, 208, 106);
        let dst = Ipv4Addr::new(204, 44, 192, 60);
        let ip_dgram = IpDatagram::new(src, dst).payload_from_slice(b"hello world");

        let bytes = ip_dgram.to_bytes();
        let view = IpView::parse(&bytes).unwrap();

        assert_eq!(view.src(), src);
        assert_eq!(view.dst(), dst);
        assert_eq!(view.protocol(), IPPROTO_TCP);
        assert_eq!(view.payload(), b"hello world");
    }

    #[test]
    fn test_ip_datagram_decode() {
        let bytes = wireshark_sample::hex_concat(&[
            wireshark_sample::ip_hex(),
            wireshark_sample::tcp_hex(),
        ]);
        let view = IpView::parse(&bytes).unwrap();
        let dgram = IpDatagram::from_view(&view);

        assert_eq!(dgram.src, Ipv4Addr::new(10, 110, 208, 106));
        assert_eq!(dgram.dst, Ipv4Addr::new(204, 44, 192, 60));
        assert_eq!(dgram.ttl, 64);
        assert_eq!(dgram.protocol, IPPROTO_TCP);
    }

    #[test]
    fn test_ip_datagram_tcp_roundtrip() {
        let src = Ipv4Addr::new(10, 110, 208, 106);
        let dst = Ipv4Addr::new(204, 44, 192, 60);

        let seg = TcpSegment::new(12345, 80).syn();
        let dgram = IpDatagram::tcp(src, dst, &seg);

        let bytes = dgram.to_bytes();
        let ip_view = IpView::parse(&bytes).unwrap();
        let tcp_view = ip_view.tcp().unwrap();

        assert_eq!(tcp_view.src_port(), 12345);
        assert_eq!(tcp_view.dst_port(), 80);
        assert!(tcp_view.is_syn());
    }
}
