//! Zero-copy TCP segment view for parsing and inspection.
//!
//! `TcpView` borrows a byte slice and provides accessor methods to read
//! header fields without allocation. Validation happens at construction time.

use crate::common::checksum::tcp_checksum;
use crate::common::wrap32::Wrap32;
use crate::ip::datagram::IpProtocol;
use crate::tcp::flags::TcpFlags;
use crate::tcp::{TcpOptions, WireError};
use std::net::Ipv4Addr;

pub const TCP_HDR_MIN_SIZE: usize = 20;

/// A validated, zero-copy view into a TCP segment.
///
/// # Invariants (established at construction)
/// - Buffer is at least 20 bytes (minimum TCP header)
/// - Data offset is >= 5 (valid header length)
/// - Buffer contains at least `header_len()` bytes
/// - Checksum is valid (when parsed with IPv4 endpoints)
#[derive(Clone, Copy)]
pub struct TcpView<'a> {
    data: &'a [u8],
    header_len: usize,
}

impl<'a> TcpView<'a> {
    /// Parse and validate a TCP segment from a byte slice against IPv4 endpoints.
    pub fn parse(data: &'a [u8], src: Ipv4Addr, dst: Ipv4Addr) -> Result<Self, WireError> {
        let view = Self::parse_unchecked(data)?;

        if tcp_checksum(src, dst, IpProtocol::Tcp, data) != 0 {
            return Err(WireError::BadChecksum);
        }

        Ok(view)
    }

    /// Parse without checksum validation.
    ///
    /// Useful for testing or when checksum is handled elsewhere.
    pub fn parse_unchecked(data: &'a [u8]) -> Result<Self, WireError> {
        if data.len() < TCP_HDR_MIN_SIZE {
            return Err(WireError::Truncated {
                needed: TCP_HDR_MIN_SIZE,
                got: data.len(),
            });
        }

        let data_offset = data[12] >> 4;
        if data_offset < 5 {
            return Err(WireError::InvalidTcpDataOffset(data_offset));
        }

        let header_len = (data_offset as usize) * 4;
        if data.len() < header_len {
            return Err(WireError::Truncated {
                needed: header_len,
                got: data.len(),
            });
        }

        Ok(Self { data, header_len })
    }

    // ─────────────────────────────────────────────────────────────
    // Header field accessors
    // ─────────────────────────────────────────────────────────────

    #[inline]
    pub fn src_port(&self) -> u16 {
        u16::from_be_bytes([self.data[0], self.data[1]])
    }

    #[inline]
    pub fn dst_port(&self) -> u16 {
        u16::from_be_bytes([self.data[2], self.data[3]])
    }

    #[inline]
    pub fn seq(&self) -> Wrap32 {
        Wrap32::new(u32::from_be_bytes([
            self.data[4],
            self.data[5],
            self.data[6],
            self.data[7],
        ]))
    }

    #[inline]
    pub fn ack(&self) -> Wrap32 {
        Wrap32::new(u32::from_be_bytes([
            self.data[8],
            self.data[9],
            self.data[10],
            self.data[11],
        ]))
    }

    #[inline]
    pub const fn header_len(&self) -> usize {
        self.header_len
    }

    #[inline]
    pub fn flags(&self) -> TcpFlags {
        TcpFlags::from_bits_truncate(self.data[13])
    }

    #[inline]
    pub fn window(&self) -> u16 {
        u16::from_be_bytes([self.data[14], self.data[15]])
    }

    #[inline]
    pub fn checksum(&self) -> u16 {
        u16::from_be_bytes([self.data[16], self.data[17]])
    }

    #[inline]
    pub fn urgent_ptr(&self) -> u16 {
        u16::from_be_bytes([self.data[18], self.data[19]])
    }

    // ─────────────────────────────────────────────────────────────
    // TCP Options accessors
    // ─────────────────────────────────────────────────────────────

    /// TCP options as a zero-copy view.
    #[inline]
    pub fn options(&self) -> Result<TcpOptions, WireError> {
        TcpOptions::parse(&self.data[TCP_HDR_MIN_SIZE..self.header_len])
    }

    /// Find MSS option value.
    #[inline]
    pub fn mss(&self) -> Option<u16> {
        self.options().ok().and_then(|opts| opts.mss)
    }

    /// Find Window Scale option value.
    #[inline]
    pub fn window_scale(&self) -> Option<u8> {
        self.options().ok().and_then(|opts| opts.wscale)
    }

    /// Find Timestamp option value (ts_val, ts_ecr).
    #[inline]
    pub fn timestamp(&self) -> Option<(u32, u32)> {
        self.options().ok().and_then(|opts| opts.timestamp)
    }

    /// Check if SACK permitted option is present.
    #[inline]
    pub fn sack_permitted(&self) -> bool {
        self.options().ok().map(|opts| opts.sack_permitted).unwrap_or(false)
    }

    /// Payload bytes after the header.
    #[inline]
    pub fn payload(&self) -> &'a [u8] {
        &self.data[self.header_len..]
    }

    /// The entire segment as bytes.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    // ─────────────────────────────────────────────────────────────
    // TCP Flags convenience methods
    // ─────────────────────────────────────────────────────────────

    #[inline]
    pub fn is_syn(&self) -> bool {
        self.flags().contains(TcpFlags::SYN)
    }

    #[inline]
    pub fn is_ack(&self) -> bool {
        self.flags().contains(TcpFlags::ACK)
    }

    #[inline]
    pub fn is_fin(&self) -> bool {
        self.flags().contains(TcpFlags::FIN)
    }

    #[inline]
    pub fn is_rst(&self) -> bool {
        self.flags().contains(TcpFlags::RST)
    }

    #[inline]
    pub fn is_psh(&self) -> bool {
        self.flags().contains(TcpFlags::PSH)
    }

    #[inline]
    pub fn is_syn_ack(&self) -> bool {
        let f = self.flags();
        f.contains(TcpFlags::SYN) && f.contains(TcpFlags::ACK)
    }

    // ─────────────────────────────────────────────────────────────
    // Derived methods
    // ─────────────────────────────────────────────────────────────

    #[inline]
    pub fn has_options(&self) -> bool {
        self.header_len > TCP_HDR_MIN_SIZE
    }

    #[inline]
    pub fn has_payload(&self) -> bool {
        !self.payload().is_empty()
    }

    #[inline]
    pub fn payload_len(&self) -> usize {
        self.data.len() - self.header_len
    }

    /// Segment length for sequence number accounting.
    /// SYN and FIN each consume one sequence number.
    pub fn seq_len(&self) -> u32 {
        let mut len = self.payload_len() as u32;
        if self.is_syn() {
            len += 1;
        }
        if self.is_fin() {
            len += 1;
        }
        len
    }

    /// Convert to an owned `TcpSegment`.
    pub fn to_owned(&self) -> super::TcpSegment {
        super::TcpSegment::from_view(self)
    }
}

impl<'a> From<&TcpView<'a>> for super::TcpSegment {
    fn from(view: &TcpView<'a>) -> Self {
        super::TcpSegment::from_view(view)
    }
}

impl<'a> From<TcpView<'a>> for super::TcpSegment {
    fn from(view: TcpView<'a>) -> Self {
        super::TcpSegment::from_view(&view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::wireshark;
    use std::net::Ipv4Addr;

    #[test]
    fn test_tcp_view_parse() {
        let tcp_bytes = hex::decode(wireshark::tcp_hex()).unwrap();
        let src = Ipv4Addr::new(10, 110, 208, 106);
        let dst = Ipv4Addr::new(204, 44, 192, 60);

        let view = TcpView::parse(&tcp_bytes, src, dst).unwrap();

        assert_eq!(view.src_port(), 50871);
        assert_eq!(view.dst_port(), 80);
        assert_eq!(view.seq(), Wrap32::new(2753993875));
        assert_eq!(view.ack(), Wrap32::new(0));
        assert_eq!(view.header_len(), 44);
        assert_eq!(view.flags(), TcpFlags::SYN);
        assert_eq!(view.window(), 65535);
        assert_eq!(view.checksum(), 37527);
        assert_eq!(view.urgent_ptr(), 0);
        assert!(view.is_syn());
        assert!(!view.is_ack());
        assert!(view.payload().is_empty());
        assert_eq!(view.mss(), Some(1460));
        assert_eq!(view.window_scale(), Some(6));
        assert!(view.sack_permitted());
    }

    #[test]
    fn test_tcp_view_with_payload() {
        let tcp_bytes = wireshark::hex_concat(&[
            wireshark::tcp_with_payload_hex(),
            wireshark::giant_payload_hex(),
        ]);
        let src = Ipv4Addr::new(204, 44, 192, 60);
        let dst = Ipv4Addr::new(10, 110, 208, 106);

        let view = TcpView::parse(&tcp_bytes, src, dst).unwrap();

        assert_eq!(view.src_port(), 80);
        assert_eq!(view.dst_port(), 50871);
        assert!(view.is_ack());
        assert!(view.has_payload());
        assert_eq!(view.payload_len(), 1374);
        assert!(view.payload().starts_with(b"HTTP/1.1 200 OK\r\n"));
    }
}
