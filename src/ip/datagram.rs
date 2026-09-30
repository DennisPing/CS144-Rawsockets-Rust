use crate::common::checksum::ip_checksum;
use crate::ip::flags::IpFlags;
use crate::tcp::error::BuildError;
use std::net::Ipv4Addr;
use crate::ip::IpView;
use crate::tcp::TcpSegment;

pub const IPV4: u8 = 4;
pub const IP_HDR_SIZE: usize = 20;
pub const DEFAULT_TTL: u8 = 64;

/// Enum idea borrowed from smoltcp: https://github.com/smoltcp-rs/smoltcp
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpProtocol {
    HopByHop = 0x00, // 0
    Icmp = 0x01, // 1
    Tcp = 0x06, // 6
    Udp = 0x11, // 17
    // Ignore all other protocols for this project
}

impl IpProtocol {
    pub fn from(b: u8) -> IpProtocol {
        match b {
            0x00 => IpProtocol::HopByHop,
            0x01 => IpProtocol::Icmp,
            0x06 => IpProtocol::Tcp,
            0x11 => IpProtocol::Udp,
            _ => IpProtocol::HopByHop // Default to 0x00
        }
    }
}


/// An IPv4 datagram with the builder-style construction. The "write only" side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpDatagram {
    pub version: u8,
    pub ihl: u8,
    pub tos: u8,
    pub id: u16,
    pub flags: IpFlags,
    pub frag_offset: u16,
    pub ttl: u8,
    pub protocol: IpProtocol,
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub payload: Vec<u8>, // Could be anything, but in this case, it's the TCP hdr + TCP payload
}

impl IpDatagram {
    /// Create a new datagram with required addresses.
    ///
    /// Defaults: Version=4, IHL=5, total_len=20, TTL=64, protocol=TCP, flags=DF, empty payload.
    pub fn new() -> Self {
        Self::default()
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

    pub fn protocol(mut self, protocol: IpProtocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn payload(mut self, data: Vec<u8>) -> Self {
        self.payload = data;
        self
    }

    /// Total length of the serialized datagram (header + payload).
    pub fn total_len(&self) -> usize {
        IP_HDR_SIZE + self.payload.len()
    }

    // ─────────────────────────────────────────────────────────────
    // From parsed view
    // ─────────────────────────────────────────────────────────────

    /// Create an owned datagram from a parsed view.
    pub fn from_view(view: &IpView<'_>) -> Self {
        Self {
            version: view.version(),
            ihl: view.ihl(),
            tos: view.tos(),
            id: view.id(),
            flags: view.flags(),
            frag_offset: view.frag_offset(),
            ttl: view.ttl(),
            protocol: view.protocol(),
            src: view.src(),
            dst: view.dst(),
            payload: view.payload().to_vec(),
        }
    }

    // ─────────────────────────────────────────────────────────────
    // Serialization
    // ─────────────────────────────────────────────────────────────

    /// Serialize to a new `Vec<u8>`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![0u8; self.total_len()];
        self.encode_into(&mut buf).expect("buffer sized correctly");
        buf
    }

    /// Serialize IP datagram into an existing buffer. Returns bytes written.
    pub fn encode_into(&self, buf: &mut [u8]) -> Result<usize, BuildError> {
        let total_len = self.total_len();
        if buf.len() < total_len {
            return Err(BuildError::BufferTooSmall {
                needed: total_len,
                got: buf.len(),
            });
        }

        self.encode_header_into(&mut buf[..IP_HDR_SIZE], total_len)?;

        // Copy payload
        buf[IP_HDR_SIZE..total_len].copy_from_slice(&self.payload);

        Ok(total_len)
    }

    /// Encode only the IP header into the provided buffer using a known total length.
    pub fn encode_header_into(&self, buf: &mut [u8], total_len: usize) -> Result<(), BuildError> {
        if buf.len() < IP_HDR_SIZE {
            return Err(BuildError::BufferTooSmall {
                needed: IP_HDR_SIZE,
                got: buf.len(),
            });
        }

        buf[0] = (self.version << 4) | self.ihl; // version + ihl
        buf[1] = self.tos; // tos
        buf[2..4].copy_from_slice(&(total_len as u16).to_be_bytes()); // total length
        buf[4..6].copy_from_slice(&self.id.to_be_bytes()); // id
        buf[6..8].copy_from_slice(&self.flags.pack(self.frag_offset).to_be_bytes());
        buf[8] = self.ttl;
        buf[9] = self.protocol as u8;
        buf[10..12].fill(0); // Checksum placeholder
        buf[12..16].copy_from_slice(&self.src.octets());
        buf[16..20].copy_from_slice(&self.dst.octets());

        // Compute and insert header checksum
        let checksum = crate::common::checksum::ip_checksum(&buf[..IP_HDR_SIZE]);
        buf[10..12].copy_from_slice(&checksum.to_be_bytes());

        Ok(())
    }
}

impl Default for IpDatagram {
    fn default() -> Self {
        Self {
            version: 4,
            ihl: 5,
            tos: 0,
            id: 0,
            flags: IpFlags::DF,
            frag_offset: 0,
            ttl: DEFAULT_TTL,
            protocol: IpProtocol::Tcp,
            src: Ipv4Addr::UNSPECIFIED,
            dst: Ipv4Addr::UNSPECIFIED,
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
    use crate::ip::view::IpView;
    use crate::tcp::TcpSegment;
    use crate::testing::wireshark;

    #[test]
    fn test_ip_datagram_encode() {
        let src = Ipv4Addr::new(10, 110, 208, 106);
        let dst = Ipv4Addr::new(204, 44, 192, 60);

        let ip = IpDatagram::new()
            .tos(0)
            .id(0)
            .flags(IpFlags::DF)
            .frag_offset(0)
            .ttl(64)
            .protocol(IpProtocol::Tcp)
            .src(src)
            .dst(dst);

        // Need to attach the TCP Segment so that the total length is correct
        let ip = ip.payload(hex::decode(wireshark::tcp_hex()).unwrap());
        let bytes = ip.to_bytes();

        // Just check the IP Header from 0..20
        assert_eq!(bytes[..IP_HDR_SIZE], hex::decode(wireshark::ip_hex()).unwrap());

        // let view = IpView::parse(&bytes).unwrap();
        //
        // assert_eq!(view.version(), 4);
        // assert_eq!(view.ihl(), 5);
        // assert_eq!(view.tos(), 0);
        // assert_eq!(view.total_len(), 31);
        // assert_eq!(view.id(), 12345);
        // assert_eq!(view.flags(), IpFlags::RF | IpFlags::DF);
        // assert_eq!(view.ttl(), 64);
        // assert_eq!(view.protocol(), IpProtocol::Tcp);
        // assert_eq!(view.src(), src);
        // assert_eq!(view.dst(), dst);
        // assert_eq!(view.payload(), b"hello world");
    }

    #[test]
    fn test_ip_datagram_decode() {
        let bytes = wireshark::hex_concat(&[
            wireshark::ip_hex(),
            wireshark::tcp_hex(),
        ]);

        let ip_view = IpView::parse(&bytes).unwrap();
        let ip = IpDatagram::from_view(&ip_view);

        assert_eq!(ip.version, 4);
        assert_eq!(ip.ihl, 5);
        assert_eq!(ip.tos, 0);
        assert_eq!(ip.total_len(), 64);
        assert_eq!(ip.id, 0);
        assert_eq!(ip.flags, IpFlags::DF);
        assert_eq!(ip.ttl, 64);
        assert_eq!(ip.protocol, IpProtocol::Tcp);
        assert_eq!(ip.src, Ipv4Addr::new(10, 110, 208, 106));
        assert_eq!(ip.dst, Ipv4Addr::new(204, 44, 192, 60));
        assert_eq!(ip.payload, hex::decode(wireshark::tcp_hex()).unwrap())
    }

    #[test]
    fn test_ip_datagram_tcp_roundtrip() {
        let src = Ipv4Addr::new(10, 110, 208, 106);
        let dst = Ipv4Addr::new(204, 44, 192, 60);

        let writer = crate::protocol::packet::PacketWriter::new()
            .ip(|ip| ip.src(src).dst(dst))
            .tcp(|_| crate::tcp::TcpSegment::new(12345, 80).syn());

        let bytes = writer.to_bytes();
        let ip_view = IpView::parse(&bytes).unwrap();
        let tcp_view = ip_view.tcp().unwrap();

        assert_eq!(tcp_view.src_port(), 12345);
        assert_eq!(tcp_view.dst_port(), 80);
        assert!(tcp_view.is_syn());
    }
}
