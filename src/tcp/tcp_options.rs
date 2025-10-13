//! TCP Options parsing and building.
//!
//! Two-layer design:
//! - `TcpOptionsView` - Zero-copy view for reading options from wire bytes
//! - `TcpOptions` - Owned representation with builder pattern for writing

use crate::tcp::WireError;
use smallvec::SmallVec;

// ─────────────────────────────────────────────────────────────────────────────
// TcpOption enum (shared between view and owned)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TcpOption {
    Eol,                                    // 0
    Nop,                                    // 1
    Mss(u16),                               // 2
    WScale(u8),                             // 3
    SackPermitted,                          // 4
    Sack(Vec<(u32, u32)>),                  // 5 (1..=4 blocks)
    Timestamp { ts_val: u32, ts_ecr: u32 }, // 8
    UserTimeout(u16),                       // 28
    TcpAo(Vec<u8>),                         // 29
    MpTcp(Vec<u8>),                         // 30
    Unknown { kind: u8, data: Vec<u8> },
}

impl TcpOption {
    /// Get the option kind byte.
    #[inline]
    pub fn kind(&self) -> u8 {
        match self {
            TcpOption::Eol => 0,
            TcpOption::Nop => 1,
            TcpOption::Mss(..) => 2,
            TcpOption::WScale(..) => 3,
            TcpOption::SackPermitted => 4,
            TcpOption::Sack(..) => 5,
            TcpOption::Timestamp { .. } => 8,
            TcpOption::UserTimeout(..) => 28,
            TcpOption::TcpAo(..) => 29,
            TcpOption::MpTcp(..) => 30,
            TcpOption::Unknown { kind, .. } => *kind,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TcpOptionsView - Zero-copy read-only view
// ─────────────────────────────────────────────────────────────────────────────

/// Zero-copy view into TCP options bytes.
///
/// Provides an iterator over options without allocation.
#[derive(Clone, Copy)]
pub struct TcpOptionsView<'a> {
    data: &'a [u8],
}

impl<'a> TcpOptionsView<'a> {
    /// Create a view over options bytes.
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Raw options bytes.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns true if there are no options.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Length of options in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Parse into owned `TcpOptions`.
    pub fn to_owned(&self) -> Result<TcpOptions, WireError> {
        TcpOptions::parse(self.data)
    }

    /// Iterate over options (allocates for each option).
    pub fn iter(&self) -> TcpOptionsIter<'a> {
        TcpOptionsIter {
            data: self.data,
            pos: 0,
        }
    }

    // ─────────────────────────────────────────────────────────────
    // Quick accessors for common options (no allocation)
    // ─────────────────────────────────────────────────────────────

    /// Find MSS option value.
    pub fn mss(&self) -> Option<u16> {
        for opt in self.iter() {
            if let Ok(TcpOption::Mss(v)) = opt {
                return Some(v);
            }
        }
        None
    }

    /// Find Window Scale option value.
    pub fn window_scale(&self) -> Option<u8> {
        for opt in self.iter() {
            if let Ok(TcpOption::WScale(v)) = opt {
                return Some(v);
            }
        }
        None
    }

    /// Check if SACK Permitted is present.
    pub fn sack_permitted(&self) -> bool {
        for opt in self.iter() {
            if let Ok(TcpOption::SackPermitted) = opt {
                return true;
            }
        }
        false
    }

    /// Find Timestamp option.
    pub fn timestamp(&self) -> Option<(u32, u32)> {
        for opt in self.iter() {
            if let Ok(TcpOption::Timestamp { ts_val, ts_ecr }) = opt {
                return Some((ts_val, ts_ecr));
            }
        }
        None
    }
}

/// Iterator over TCP options.
pub struct TcpOptionsIter<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Iterator for TcpOptionsIter<'a> {
    type Item = Result<TcpOption, WireError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.data.len() {
            return None;
        }

        let kind = self.data[self.pos];
        match kind {
            // EOL
            0 => {
                self.pos = self.data.len(); // Stop iteration
                Some(Ok(TcpOption::Eol))
            }
            // NOP
            1 => {
                self.pos += 1;
                Some(Ok(TcpOption::Nop))
            }
            _ => {
                if self.pos + 1 >= self.data.len() {
                    self.pos = self.data.len();
                    return Some(Err(WireError::Truncated {
                        needed: self.pos + 2,
                        got: self.data.len(),
                    }));
                }

                let len = self.data[self.pos + 1] as usize;
                if len < 2 {
                    self.pos = self.data.len();
                    return Some(Err(WireError::InvalidTcpOption(kind)));
                }
                if self.pos + len > self.data.len() {
                    self.pos = self.data.len();
                    return Some(Err(WireError::Truncated {
                        needed: self.pos + len,
                        got: self.data.len(),
                    }));
                }

                let data = &self.data[self.pos + 2..self.pos + len];
                self.pos += len;

                let opt = match kind {
                    2 if len == 4 => TcpOption::Mss(u16::from_be_bytes([data[0], data[1]])),
                    3 if len == 3 => TcpOption::WScale(data[0]),
                    4 if len == 2 => TcpOption::SackPermitted,
                    5 if (10..=34).contains(&len) && (len - 2) % 8 == 0 => {
                        let mut blocks = Vec::with_capacity((len - 2) / 8);
                        for chunk in data.chunks_exact(8) {
                            let l = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                            let r = u32::from_be_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
                            blocks.push((l, r));
                        }
                        TcpOption::Sack(blocks)
                    }
                    8 if len == 10 => {
                        let ts_val = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                        let ts_ecr = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                        TcpOption::Timestamp { ts_val, ts_ecr }
                    }
                    28 if len == 4 => {
                        TcpOption::UserTimeout(u16::from_be_bytes([data[0], data[1]]))
                    }
                    29 => TcpOption::TcpAo(data.to_vec()),
                    30 => TcpOption::MpTcp(data.to_vec()),
                    _ => TcpOption::Unknown {
                        kind,
                        data: data.to_vec(),
                    },
                };

                Some(Ok(opt))
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TcpOptions - Owned representation with builder
// ─────────────────────────────────────────────────────────────────────────────

/// Owned list of TCP options with builder pattern.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TcpOptions(SmallVec<[TcpOption; 8]>);

impl TcpOptions {
    /// Create empty options.
    pub fn new() -> Self {
        Self(SmallVec::new())
    }

    /// Start building options.
    pub fn builder() -> TcpOptionsBuilder {
        TcpOptionsBuilder {
            opts: SmallVec::new(),
        }
    }

    /// Parse options from wire bytes.
    pub fn parse(data: &[u8]) -> Result<Self, WireError> {
        let view = TcpOptionsView::new(data);
        let mut opts = SmallVec::new();
        for result in view.iter() {
            opts.push(result?);
        }
        Ok(TcpOptions(opts))
    }

    pub fn iter(&self) -> impl Iterator<Item = &TcpOption> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Serialize to wire bytes with proper 32-bit padding.
    pub fn to_bytes_padded(&self) -> Vec<u8> {
        let mut out = self.to_bytes_unpadded();
        if out.is_empty() {
            return out;
        }

        // Pad to 4-byte boundary with EOL (0)
        while out.len() % 4 != 0 {
            out.push(0);
        }

        debug_assert!(out.len() <= 40, "TCP options exceeded 40 bytes");
        out
    }

    /// Wire length after padding.
    pub fn wire_len(&self) -> usize {
        if self.is_empty() {
            return 0;
        }
        let unpadded = self.to_bytes_unpadded().len();
        (unpadded + 3) & !3 // Round up to 4-byte boundary
    }

    /// Encode without padding (internal).
    fn to_bytes_unpadded(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for opt in &self.0 {
            if matches!(opt, TcpOption::Eol) {
                out.push(0);
                break;
            }

            // Insert NOPs to align certain options
            let kind = opt.kind();
            if Self::needs_alignment(kind) && out.len() % 4 != 0 {
                let padding = 4 - (out.len() % 4);
                for _ in 0..padding {
                    out.push(1); // NOP
                }
            }

            match opt {
                TcpOption::Eol => unreachable!(),
                TcpOption::Nop => out.push(1),
                TcpOption::Mss(mss) => {
                    out.extend_from_slice(&[2, 4]);
                    out.extend_from_slice(&mss.to_be_bytes());
                }
                TcpOption::WScale(scale) => {
                    out.extend_from_slice(&[3, 3, *scale]);
                }
                TcpOption::SackPermitted => {
                    out.extend_from_slice(&[4, 2]);
                }
                TcpOption::Sack(blocks) => {
                    let n = blocks.len().min(4);
                    let len = 2 + 8 * n;
                    out.push(5);
                    out.push(len as u8);
                    for &(l, r) in blocks.iter().take(n) {
                        out.extend_from_slice(&l.to_be_bytes());
                        out.extend_from_slice(&r.to_be_bytes());
                    }
                }
                TcpOption::Timestamp { ts_val, ts_ecr } => {
                    out.extend_from_slice(&[8, 10]);
                    out.extend_from_slice(&ts_val.to_be_bytes());
                    out.extend_from_slice(&ts_ecr.to_be_bytes());
                }
                TcpOption::UserTimeout(v) => {
                    out.extend_from_slice(&[28, 4]);
                    out.extend_from_slice(&v.to_be_bytes());
                }
                TcpOption::TcpAo(data) => {
                    let len = (2 + data.len()).min(255);
                    out.push(29);
                    out.push(len as u8);
                    out.extend_from_slice(&data[..len - 2]);
                }
                TcpOption::MpTcp(data) => {
                    let len = (2 + data.len()).min(255);
                    out.push(30);
                    out.push(len as u8);
                    out.extend_from_slice(&data[..len - 2]);
                }
                TcpOption::Unknown { kind, data } => {
                    let len = (2 + data.len()).min(255);
                    out.push(*kind);
                    out.push(len as u8);
                    out.extend_from_slice(&data[..len - 2]);
                }
            }
        }
        out
    }

    /// Options that benefit from 32-bit alignment.
    #[inline]
    fn needs_alignment(kind: u8) -> bool {
        matches!(kind, 5 | 8 | 29 | 30)
    }

    pub(crate) fn from_smallvec(opts: SmallVec<[TcpOption; 8]>) -> Self {
        TcpOptions(opts)
    }
}

impl core::ops::Deref for TcpOptions {
    type Target = [TcpOption];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> IntoIterator for &'a TcpOptions {
    type Item = &'a TcpOption;
    type IntoIter = core::slice::Iter<'a, TcpOption>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TcpOptionsBuilder - Fluent builder
// ─────────────────────────────────────────────────────────────────────────────

pub struct TcpOptionsBuilder {
    opts: SmallVec<[TcpOption; 8]>,
}

impl TcpOptionsBuilder {
    pub fn nop(mut self) -> Self {
        self.opts.push(TcpOption::Nop);
        self
    }

    pub fn mss(mut self, mss: u16) -> Self {
        self.opts.push(TcpOption::Mss(mss));
        self
    }

    pub fn wscale(mut self, s: u8) -> Self {
        self.opts.push(TcpOption::WScale(s));
        self
    }

    pub fn sack_permitted(mut self) -> Self {
        self.opts.push(TcpOption::SackPermitted);
        self
    }

    pub fn sack(mut self, blocks: Vec<(u32, u32)>) -> Self {
        self.opts.push(TcpOption::Sack(blocks));
        self
    }

    pub fn timestamp(mut self, ts_val: u32, ts_ecr: u32) -> Self {
        self.opts.push(TcpOption::Timestamp { ts_val, ts_ecr });
        self
    }

    pub fn user_timeout(mut self, v: u16) -> Self {
        self.opts.push(TcpOption::UserTimeout(v));
        self
    }

    pub fn tcp_ao(mut self, bytes: Vec<u8>) -> Self {
        self.opts.push(TcpOption::TcpAo(bytes));
        self
    }

    pub fn mptcp(mut self, bytes: Vec<u8>) -> Self {
        self.opts.push(TcpOption::MpTcp(bytes));
        self
    }

    /// Sort options in conventional order.
    pub fn normalize(mut self) -> Self {
        fn rank(kind: u8) -> u8 {
            match kind {
                2 => 10,  // MSS
                4 => 20,  // SACK Permitted
                5 => 25,  // SACK
                8 => 30,  // Timestamp
                3 => 40,  // WScale
                28 => 50, // UserTimeout
                29 => 60, // TCP-AO
                30 => 70, // MPTCP
                _ => 100,
            }
        }
        self.opts.sort_by_key(|o| rank(o.kind()));
        self
    }

    /// Build into padded wire bytes.
    pub fn build(self) -> Vec<u8> {
        TcpOptions::from_smallvec(self.opts).to_bytes_padded()
    }

    /// Build into owned TcpOptions.
    pub fn into_options(self) -> TcpOptions {
        TcpOptions::from_smallvec(self.opts)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_option_nop() {
        let opt_bytes = TcpOptions::builder().nop().build();
        assert_eq!(vec![1, 0, 0, 0], opt_bytes);

        let opts = TcpOptions::parse(&opt_bytes).unwrap();
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0], TcpOption::Nop);
        assert_eq!(opts[1], TcpOption::Eol)
    }

    #[test]
    fn test_tcp_option_mss() {
        let opt_bytes = TcpOptions::builder().mss(1460).build();
        assert_eq!(vec![2, 4, 5, 180], opt_bytes);

        let opts = TcpOptions::parse(&opt_bytes).unwrap();
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0], TcpOption::Mss(1460));
    }

    #[test]
    fn test_tcp_option_wscale() {
        let opt_bytes = TcpOptions::builder().wscale(7).build();
        assert_eq!(vec![3, 3, 7, 0], opt_bytes);

        let opts = TcpOptions::parse(&opt_bytes).unwrap();
        assert_eq!(2, opts.len());
        assert_eq!(opts[0], TcpOption::WScale(7));
        assert_eq!(opts[1], TcpOption::Eol)
    }

    #[test]
    fn test_tcp_option_mss_and_wscale() {
        let opt_bytes = TcpOptions::builder().mss(1460).wscale(7).build();
        assert_eq!(vec![2, 4, 5, 180, 3, 3, 7, 0], opt_bytes);

        let opts = TcpOptions::parse(&opt_bytes).unwrap();
        assert_eq!(3, opts.len());
        assert_eq!(opts[0], TcpOption::Mss(1460));
        assert_eq!(opts[1], TcpOption::WScale(7));
        assert_eq!(opts[2], TcpOption::Eol);
    }

    #[test]
    fn test_wireshark_options() {
        let opt_bytes = hex::decode("020405b4010303060101080abb6879f80000000004020000").unwrap();
        assert_eq!(opt_bytes.len() % 4, 0);

        let opts = TcpOptions::parse(&opt_bytes).unwrap();
        assert_eq!(8, opts.len());
        assert_eq!(opts[0], TcpOption::Mss(1460));
        assert_eq!(opts[1], TcpOption::Nop);
        assert_eq!(opts[2], TcpOption::WScale(6));
        assert_eq!(opts[3], TcpOption::Nop);
        assert_eq!(opts[4], TcpOption::Nop);
        assert_eq!(opts[5], TcpOption::Timestamp { ts_val: 3144186360, ts_ecr: 0 });
        assert_eq!(opts[6], TcpOption::SackPermitted);
        assert_eq!(opts[7], TcpOption::Eol);
    }

    #[test]
    fn test_options_view_accessors() {
        let opt_bytes = hex::decode("020405b4010303060101080abb6879f80000000004020000").unwrap();
        let view = TcpOptionsView::new(&opt_bytes);

        assert_eq!(view.mss(), Some(1460));
        assert_eq!(view.window_scale(), Some(6));
        assert_eq!(view.timestamp(), Some((3144186360, 0)));
        assert!(view.sack_permitted());
    }
}
