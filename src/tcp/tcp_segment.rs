//! Owned TCP segment with builder-style construction.
//!
//! Two-layer design:
//! - `TcpSegment` - Owned data with builder methods and serialization
//! - `TcpView` - Zero-copy view for parsing (see tcp_segment_view)

use crate::common::wrap32::Wrap32;
use crate::ip::ip_datagram::IpDatagram;
use crate::tcp::tcp_checksum::tcp_checksum_for_segment;
use crate::tcp::tcp_error::BuildError;
use crate::tcp::tcp_flags::TcpFlags;
use crate::tcp::tcp_options::TcpOptions;
use crate::tcp::tcp_segment_view::{TcpView, TCP_HDR_MIN_SIZE};
use std::net::Ipv4Addr;

pub const TCP_DEFAULT_WINDOW: u16 = 65535;

/// An owned TCP segment with builder-style construction.
///
/// # Construction
///
/// Use `new()` for minimal construction or chain builder methods:
///
/// ```ignore
/// let seg = TcpSegment::new(src_port, dst_port)
///     .seq(Wrap32::new(1000))
///     .syn()
///     .options(TcpOptions::builder().mss(1460).into_options());
/// ```
///
/// # Serialization
///
/// ```ignore
/// let bytes = seg.to_bytes(src_ip, dst_ip);
/// // or into existing buffer:
/// let len = seg.encode_into(&mut buf, src_ip, dst_ip)?;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpSegment {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq: Wrap32,
    pub ack: Wrap32,
    pub flags: TcpFlags,
    pub window: u16,
    pub urgent_ptr: u16,
    pub options: TcpOptions,
    pub payload: Vec<u8>,
}

impl TcpSegment {
    /// Create a new segment with required ports.
    ///
    /// Defaults: seq=0, ack=0, flags=empty, window=65535, no options/payload.
    pub fn new(src_port: u16, dst_port: u16) -> Self {
        Self {
            src_port,
            dst_port,
            seq: Wrap32::new(0),
            ack: Wrap32::new(0),
            flags: TcpFlags::empty(),
            window: TCP_DEFAULT_WINDOW,
            urgent_ptr: 0,
            options: TcpOptions::default(),
            payload: Vec::new(),
        }
    }

    // ─────────────────────────────────────────────────────────────
    // Builder methods (consume self, return self)
    // ─────────────────────────────────────────────────────────────

    pub fn seq(mut self, seq: Wrap32) -> Self {
        self.seq = seq;
        self
    }

    /// Set the acknowledgment number and automatically insert the ACK flag.
    pub fn ack(mut self, ack: Wrap32) -> Self {
        self.ack = ack;
        self.flags.insert(TcpFlags::ACK);
        self
    }

    pub fn flags(mut self, flags: TcpFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn syn(mut self) -> Self {
        self.flags.insert(TcpFlags::SYN);
        self
    }

    pub fn ack_flag(mut self) -> Self {
        self.flags.insert(TcpFlags::ACK);
        self
    }

    pub fn fin(mut self) -> Self {
        self.flags.insert(TcpFlags::FIN);
        self
    }

    pub fn rst(mut self) -> Self {
        self.flags.insert(TcpFlags::RST);
        self
    }

    pub fn psh(mut self) -> Self {
        self.flags.insert(TcpFlags::PSH);
        self
    }

    pub fn window(mut self, window: u16) -> Self {
        self.window = window;
        self
    }

    pub fn urgent_ptr(mut self, ptr: u16) -> Self {
        self.urgent_ptr = ptr;
        self
    }

    pub fn options(mut self, options: TcpOptions) -> Self {
        self.options = options;
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
    // From parsed view
    // ─────────────────────────────────────────────────────────────

    /// Create an owned segment from a parsed view.
    pub fn from_view(view: &TcpView<'_>) -> Self {
        Self {
            src_port: view.src_port(),
            dst_port: view.dst_port(),
            seq: view.seq(),
            ack: view.ack(),
            flags: view.flags(),
            window: view.window(),
            urgent_ptr: view.urgent_ptr(),
            options: view.options().to_owned().unwrap_or_default(),
            payload: view.payload().to_vec(),
        }
    }

    // ─────────────────────────────────────────────────────────────
    // Derived values
    // ─────────────────────────────────────────────────────────────

    /// Header length in bytes (20 + options).
    pub fn header_len(&self) -> usize {
        TCP_HDR_MIN_SIZE + self.options.wire_len()
    }

    /// Total wire length (header + payload).
    pub fn wire_len(&self) -> usize {
        self.header_len() + self.payload.len()
    }

    /// Segment length for sequence number accounting.
    /// SYN and FIN each consume one sequence number.
    pub fn seq_len(&self) -> u32 {
        let mut len = self.payload.len() as u32;
        if self.flags.contains(TcpFlags::SYN) {
            len += 1;
        }
        if self.flags.contains(TcpFlags::FIN) {
            len += 1;
        }
        len
    }

    // ─────────────────────────────────────────────────────────────
    // Serialization
    // ─────────────────────────────────────────────────────────────

    /// Serialize to a new `Vec<u8>`.
    pub fn to_bytes(&self, src: Ipv4Addr, dst: Ipv4Addr) -> Vec<u8> {
        let mut buf = vec![0u8; self.wire_len()];
        self.encode_into(&mut buf, src, dst)
            .expect("buffer sized correctly");
        buf
    }

    /// Serialize into an existing buffer. Returns bytes written.
    pub fn encode_into(
        &self,
        buf: &mut [u8],
        src: Ipv4Addr,
        dst: Ipv4Addr,
    ) -> Result<usize, BuildError> {
        let options_bytes = self.options.to_bytes_padded();
        let header_len = TCP_HDR_MIN_SIZE + options_bytes.len();
        let total_len = header_len + self.payload.len();

        if buf.len() < total_len {
            return Err(BuildError::BufferTooSmall {
                needed: total_len,
                got: buf.len(),
            });
        }

        // Validate options alignment
        if options_bytes.len() % 4 != 0 {
            return Err(BuildError::OptionsNotAligned {
                len: options_bytes.len(),
            });
        }

        let data_offset = (header_len / 4) as u8;
        if data_offset > 15 {
            return Err(BuildError::OptionsTooLong {
                len: options_bytes.len(),
            });
        }

        // Write header
        buf[0..2].copy_from_slice(&self.src_port.to_be_bytes());
        buf[2..4].copy_from_slice(&self.dst_port.to_be_bytes());
        buf[4..8].copy_from_slice(&self.seq.to_be_bytes());
        buf[8..12].copy_from_slice(&self.ack.to_be_bytes());
        buf[12] = data_offset << 4;
        buf[13] = self.flags.bits();
        buf[14..16].copy_from_slice(&self.window.to_be_bytes());
        buf[16..18].fill(0); // Checksum placeholder
        buf[18..20].copy_from_slice(&self.urgent_ptr.to_be_bytes());

        // Write options
        if !options_bytes.is_empty() {
            buf[TCP_HDR_MIN_SIZE..header_len].copy_from_slice(&options_bytes);
        }

        // Write payload
        buf[header_len..total_len].copy_from_slice(&self.payload);

        // Compute and insert checksum
        let checksum = tcp_checksum_for_segment(src, dst, &buf[..total_len]);
        buf[16..18].copy_from_slice(&checksum.to_be_bytes());

        Ok(total_len)
    }

    /// Wrap this TCP segment into an owned `IpDatagram`.
    pub fn into_ip(self, src: Ipv4Addr, dst: Ipv4Addr) -> IpDatagram {
        let payload = self.to_bytes(src, dst);
        IpDatagram::new(src, dst).payload(payload)
    }
}

impl Default for TcpSegment {
    fn default() -> Self {
        Self {
            src_port: 0,
            dst_port: 0,
            seq: Wrap32::new(0),
            ack: Wrap32::new(0),
            flags: TcpFlags::empty(),
            window: TCP_DEFAULT_WINDOW,
            urgent_ptr: 0,
            options: TcpOptions::default(),
            payload: Vec::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tcp::wireshark_sample;
    use crate::tcp::wireshark_sample::hex_to_bytes;
    use std::net::Ipv4Addr;

    #[test]
    fn test_tcp_segment_encode() {
        let src_ip = Ipv4Addr::new(10, 110, 208, 106);
        let dst_ip = Ipv4Addr::new(204, 44, 192, 60);

        // Build with our own options (not parsed from Wireshark)
        let seg = TcpSegment::new(50871, 80)
            .seq(Wrap32::new(2753993875))
            .syn()
            .options(
                TcpOptions::builder()
                    .mss(1460)
                    .wscale(6)
                    .timestamp(3144186360, 0)
                    .sack_permitted()
                    .into_options(),
            );

        let encoded = seg.to_bytes(src_ip, dst_ip);

        // Verify checksum is valid (sum to 0)
        assert_eq!(tcp_checksum_for_segment(src_ip, dst_ip, &encoded), 0);

        // Verify we can parse it back
        let view = TcpView::parse(&encoded, src_ip, dst_ip).unwrap();
        assert_eq!(view.src_port(), 50871);
        assert_eq!(view.dst_port(), 80);
        assert_eq!(view.seq(), Wrap32::new(2753993875));
        assert!(view.is_syn());
    }

    #[test]
    fn test_tcp_segment_decode() {
        let tcp_bytes = hex_to_bytes(wireshark_sample::tcp_hex());
        let src_ip = Ipv4Addr::new(10, 110, 208, 106);
        let dst_ip = Ipv4Addr::new(204, 44, 192, 60);

        let view = TcpView::parse(&tcp_bytes, src_ip, dst_ip).unwrap();
        let seg = TcpSegment::from_view(&view);

        assert_eq!(seg.src_port, 50871);
        assert_eq!(seg.dst_port, 80);
        assert_eq!(seg.seq, Wrap32::new(2753993875));
        assert_eq!(seg.ack, Wrap32::new(0));
        assert_eq!(seg.flags, TcpFlags::SYN);
        assert_eq!(seg.window, 65535);
        assert_eq!(seg.urgent_ptr, 0);
        assert!(seg.payload.is_empty());
    }

    #[test]
    fn test_tcp_segment_roundtrip() {
        let src_ip = Ipv4Addr::new(10, 110, 208, 106);
        let dst_ip = Ipv4Addr::new(204, 44, 192, 60);

        let seg = TcpSegment::new(12345, 80)
            .seq(Wrap32::new(1000))
            .ack(Wrap32::new(2000))
            .syn()
            .options(TcpOptions::builder().mss(1460).wscale(7).into_options())
            .payload_from_slice(b"hello");

        let encoded = seg.to_bytes(src_ip, dst_ip);

        let view = TcpView::parse(&encoded, src_ip, dst_ip).unwrap();
        let decoded = TcpSegment::from_view(&view);

        assert_eq!(decoded.src_port, 12345);
        assert_eq!(decoded.dst_port, 80);
        assert_eq!(decoded.seq, Wrap32::new(1000));
        assert_eq!(decoded.ack, Wrap32::new(2000));
        assert!(decoded.flags.contains(TcpFlags::SYN));
        assert!(decoded.flags.contains(TcpFlags::ACK));
        assert_eq!(decoded.payload, b"hello");
    }

    #[test]
    fn test_tcp_segment_into_ip() {
        let src_ip = Ipv4Addr::new(10, 110, 208, 106);
        let dst_ip = Ipv4Addr::new(204, 44, 192, 60);

        let seg = TcpSegment::new(12345, 80)
            .seq(Wrap32::new(1000))
            .ack(Wrap32::new(2000))
            .payload_from_slice(b"test");

        let ip = seg.into_ip(src_ip, dst_ip);
        assert_eq!(ip.src, src_ip);
        assert_eq!(ip.dst, dst_ip);
        assert_eq!(ip.protocol, crate::ip::ip_datagram::IPPROTO_TCP);
    }
}
