use crate::ip::datagram::{IpDatagram, IpProtocol, IP_HDR_SIZE};
use crate::tcp::segment::TcpSegment;
use crate::tcp::error::BuildError;
use crate::ip::view::IpView;
use crate::tcp::view::TcpView;
use crate::tcp::WireError;

/// The writer (RNA Polymerase) that constructs a unified TCP/IP packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketWriter {
    pub ip: IpDatagram,
    pub tcp: TcpSegment,
}

impl PacketWriter {
    pub fn new() -> Self {
        Self {
            ip: IpDatagram::new(),
            tcp: TcpSegment::default(),
        }
    }

    pub fn ip(mut self, f: impl FnOnce(IpDatagram) -> IpDatagram) -> Self {
        self.ip = f(self.ip);
        self
    }

    pub fn tcp(mut self, f: impl FnOnce(TcpSegment) -> TcpSegment) -> Self {
        self.tcp = f(self.tcp);
        self
    }

    /// Total length of the combined IP + TCP packet on the wire
    pub fn total_len(&self) -> usize {
        IP_HDR_SIZE + self.tcp.segment_len()
    }

    /// Zips the headers and payload into the final outbound stream.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![0u8; self.total_len()];
        self.encode_into(&mut buf).expect("Buffer correctly sized");
        buf
    }

    /// Encode both IP and TCP directly into the buffer, avoiding double-allocation
    pub fn encode_into(&self, buf: &mut [u8]) -> Result<usize, BuildError> {
        let ip_len = IP_HDR_SIZE;
        let total_len = self.total_len();

        if buf.len() < total_len {
            return Err(BuildError::BufferTooSmall { needed: total_len, got: buf.len() });
        }

        // 1. We encode the TCP segment first into the back of the buffer.
        self.tcp.encode_into(&mut buf[ip_len..], self.ip.src, self.ip.dst)?;

        // 2. Encode the IP header into the front of the buffer.
        self.ip.encode_header_into(&mut buf[..ip_len], total_len)?;

        Ok(total_len)
    }
}

impl Default for PacketWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// The reader (RNA Polymerase) that unzips and reads the network stream.
#[derive(Clone, Copy)]
pub struct PacketReader<'a> {
    pub ip: IpView<'a>,
    pub tcp: TcpView<'a>,
}

impl<'a> PacketReader<'a> {
    /// Attempt to parse the incoming byte stream as a valid TCP/IP packet.
    pub fn parse(data: &'a [u8]) -> Result<Self, WireError> {
        // 1. Unzip IP Header
        let ip = IpView::parse(data)?;
        
        // Ensure it's TCP
        if ip.protocol() != IpProtocol::Tcp {
            return Err(WireError::ProtocolNotSupported(ip.protocol() as u8));
        }

        // 2. Unzip TCP Header
        let tcp = ip.tcp()?;

        Ok(Self { ip, tcp })
    }

    /// Access the raw TCP payload directly
    pub fn payload(&self) -> &'a [u8] {
        self.tcp.payload()
    }
}
