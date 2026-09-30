use crate::tcp::WireError;

/// A flat, allocation-free TCP Options. 40 byte struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TcpOptions {
    pub mss: Option<u16>,
    pub wscale: Option<u8>,
    pub sack_permitted: bool,
    /// Up to 4 SACK blocks (left_edge, right_edge)
    pub sack_blocks: [(u32, u32); 4],
    pub sack_blocks_len: u8,
    /// (ts_val, ts_ecr)
    pub timestamp: Option<(u32, u32)>,
    pub user_timeout: Option<u16>,
}

impl TcpOptions {
    /// Create a new, empty set of options.
    pub const fn new() -> Self {
        Self {
            mss: None,
            wscale: None,
            sack_permitted: false,
            sack_blocks: [(0, 0); 4],
            sack_blocks_len: 0,
            timestamp: None,
            user_timeout: None,
        }
    }

    /// Read all options from incoming data stream.
    /// Unknown options are ignored.
    pub fn parse(data: &[u8]) -> Result<Self, WireError> {
        let mut opts = Self::new();
        let mut i = 0;

        while i < data.len() {
            let kind = data[i];

            // 1 byte options
            if kind == 0 {
                break; // EOL
            }

            if kind == 1 {
                i += 1; // NOP
                continue;
            }

            // Multi byte options
            if i+1 >= data.len() {
                return Err(WireError::Truncated { needed: i+2, got: data.len()})
            }

            let len = data[i+1] as usize;
            if len < 2 || i + len > data.len() {
                return Err(WireError::InvalidTcpOption(kind))
            }

            let payload = &data[i + 2 .. i+len];
            match kind {
                2 if len == 4 => opts.mss = Some(u16::from_be_bytes([payload[0], payload[1]])),
                3 if len == 3 => opts.wscale = Some(payload[0]),
                4 if len == 2 => opts.sack_permitted = true,
                5 if len >= 10 && (len - 2).is_multiple_of(8) => {
                    let num_blocks = ((len-2)/8).min(4);
                    opts.sack_blocks_len = num_blocks as u8;
                    for b in 0..num_blocks {
                        let offset = b * 8;
                        opts.sack_blocks[b] = (
                            u32::from_be_bytes(payload[offset..offset + 4].try_into().unwrap()),
                            u32::from_be_bytes(payload[offset + 4..offset + 8].try_into().unwrap()),
                        );
                    }
                }
                8 if len == 10 => {
                    opts.timestamp = Some((
                        u32::from_be_bytes(payload[0..4].try_into().unwrap()),
                        u32::from_be_bytes(payload[4..8].try_into().unwrap()),
                        ));
                }
                28 if len == 4 => opts.user_timeout = Some(u16::from_be_bytes([payload[0], payload[1]])),
                _ => {} // Ignore all other unknown options
            }
            i += len;
        }

        Ok(opts)
    }

    /// Serialize into a 40-byte stack buffer, returning the buffer and written length.
    /// This automatically handles NOP padding for 32-bit alignment!
    pub fn to_bytes(&self) -> ([u8; 40], usize) {
        let mut buf = [0u8; 40];
        let mut idx = 0;

        if let Some(mss) = self.mss {
            buf[idx..idx+4].copy_from_slice(&[2, 4, (mss >> 8) as u8, mss as u8]);
            idx += 4;
        }

        if let Some(ws) = self.wscale {
            buf[idx..idx+4].copy_from_slice(&[1, 3, 3, ws]); // Prepended NOP for alignment
            idx += 4;
        }

        if self.sack_permitted {
            buf[idx..idx+4].copy_from_slice(&[4, 2, 1, 1]); // Appended NOPs for alignment
            idx += 4;
        }

        if let Some((ts_val, ts_ecr)) = self.timestamp {
            buf[idx..idx+2].copy_from_slice(&[1, 1]); // Two NOPs to align the 10-byte timestamp to 12 bytes
            idx += 2;
            buf[idx..idx+10].copy_from_slice(&[
                8, 10,
                (ts_val >> 24) as u8, (ts_val >> 16) as u8, (ts_val >> 8) as u8, ts_val as u8,
                (ts_ecr >> 24) as u8, (ts_ecr >> 16) as u8, (ts_ecr >> 8) as u8, ts_ecr as u8,
            ]);
            idx += 10;
        }

        if self.sack_blocks_len > 0 {
            let n = self.sack_blocks_len as usize;
            buf[idx..idx+2].copy_from_slice(&[1, 1]); // Align
            idx += 2;
            buf[idx] = 5;
            buf[idx+1] = (2 + 8 * n) as u8;
            idx += 2;
            for &(l, r) in self.sack_blocks.iter().take(n) {
                buf[idx..idx+4].copy_from_slice(&l.to_be_bytes());
                buf[idx+4..idx+8].copy_from_slice(&r.to_be_bytes());
                idx += 8;
            }
        }

        // Pad the total length to a 32-bit boundary using EOL (0)
        while idx % 4 != 0 {
            buf[idx] = 0;
            idx += 1;
        }

        (buf, idx)
    }

    /// Returns the exact logical wire length (in bytes) of these options,
    /// including all necessary NOP padding for 32-bit alignment.
    #[inline]
    pub const fn options_len(&self) -> usize {
        let mut len = 0;

        if self.mss.is_some() {
            len += 4;
        }

        if self.wscale.is_some() {
            // 3 bytes for WScale + 1 byte NOP padding
            len += 4;
        }

        if self.sack_permitted {
            // 2 bytes for SACK Permitted + 2 bytes NOP padding
            len += 4;
        }

        if self.timestamp.is_some() {
            // 10 bytes for Timestamp + 2 bytes NOP padding
            len += 12;
        }

        if self.sack_blocks_len > 0 {
            // 2 bytes for NOP padding + 2 bytes for kind/len + 8 bytes per block
            len += 4 + (self.sack_blocks_len as usize * 8);
        }

        len
    }

    #[inline]
    pub fn mss(mut self, mss: u16) -> Self {
        self.mss = Some(mss);
        self
    }

    #[inline]
    pub fn wscale(mut self, wscale: u8) -> Self {
        self.wscale = Some(wscale);
        self
    }

    #[inline]
    pub fn sack_permitted(mut self, sack_permitted: bool) -> Self {
        self.sack_permitted = sack_permitted;
        self
    }

    #[inline]
    pub fn timestamp(mut self, ts_val: u32, ts_ecr: u32) -> Self {
        self.timestamp = Some((ts_val, ts_ecr));
        self
    }

    #[inline]
    pub fn sack(mut self, blocks: &[(u32, u32)]) -> Self {
        let n = blocks.len().min(4);
        self.sack_blocks_len = n as u8;
        for i in 0..n {
            self.sack_blocks[i] = blocks[i];
        }
        self
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_option_mss() {
        let opts = TcpOptions::new().mss(1460);

        let (bytes, len) = opts.to_bytes();
        assert_eq!(len, 4);
        assert_eq!(&bytes[..len], [2, 4, 5, 180]); // mss kind = 2, len = 4, val = 1460

        let parsed = TcpOptions::parse(&bytes[..len]).unwrap();
        assert_eq!(parsed.mss, Some(1460));
    }

    #[test]
    fn test_tcp_option_wscale() {
        let opts = TcpOptions::new().wscale(7);

        let (bytes, len) = opts.to_bytes();
        assert_eq!(len, 4);
        assert_eq!(&bytes[..len], [1, 3, 3, 7]); // nop, wscale kind = 3, len = 3, val = 7

        let parsed = TcpOptions::parse(&bytes[..len]).unwrap();
        assert_eq!(parsed.wscale, Some(7));
    }

    #[test]
    fn test_tcp_option_mss_and_wscale() {
        let opts = TcpOptions::new().mss(1460).wscale(7);

        let (bytes, len) = opts.to_bytes();
        assert_eq!(len, 8);
        assert_eq!(&bytes[..len], [
            2, 4, 5, 180, // mss
            1, 3, 3, 7 // nop + wscale
        ]);

        let parsed = TcpOptions::parse(&bytes[..len]).unwrap();
        assert_eq!(parsed.mss, Some(1460));
        assert_eq!(parsed.wscale, Some(7));
    }

    #[test]
    fn test_tcp_option_timestamp() {
        let opts = TcpOptions::new().timestamp(3144186360, 0);

        let (bytes, len) = opts.to_bytes();
        assert_eq!(len, 12);
        assert_eq!(&bytes[..len], [
            1, 1, // nop x 2
            8, 10, // timestamp kind = 8, len = 10
            187, 104, 121, 248, // ts_val = 3144186360
            0, 0, 0, 0, // ts_ecr = 0
        ]);

        let parsed = TcpOptions::parse(&bytes[..len]).unwrap();
        assert_eq!(parsed.timestamp, Some((3144186360, 0)));
    }

    #[test]
    fn test_tcp_options_wireshark() {
        let bytes = hex::decode("020405b4010303060101080abb6879f80000000004020000").unwrap();
        assert_eq!(bytes.len() % 4, 0);
        assert_eq!(bytes.len(), 24);

        let parsed = TcpOptions::parse(&bytes).unwrap();
        assert_eq!(parsed.mss, Some(1460));
        assert_eq!(parsed.wscale, Some(6));
        assert_eq!(parsed.timestamp, Some((3144186360, 0)));
        assert!(parsed.sack_permitted);
    }
}
