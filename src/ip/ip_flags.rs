use bitflags::bitflags;

bitflags! {
    // Bit positions [ RF, DF, MF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ]
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct IPFlags: u16 {
        const RF = 1 << 15; // Reserved Flag
        const DF = 1 << 14; // Don't Fragment
        const MF = 1 << 13; // More Fragments
    }
}

impl IPFlags {
    /// Pack the flags and fragment offset into a single u16
    pub fn pack(self, frag_offset: u16) -> u16 {
        self.bits() | (frag_offset & 0x1fff)
    }

    /// Unpack the flags and fragment offset from a single u16
    pub fn unpack(bits: u16) -> (Self, u16) {
        let top3 = Self::from_bits_truncate(bits & 0xe000);
        let bottom13 = bits & 0x1fff;
        (top3, bottom13)
    }
}

// -- Unit tests --

#[cfg(test)]
mod tests {
    use crate::ip::ip_flags::IPFlags;

    #[test]
    fn test_ip_flags() {
        assert_eq!(IPFlags::RF.bits(), 0b1000000000000000);
        assert_eq!(IPFlags::DF.bits(), 0b0100000000000000);
        assert_eq!(IPFlags::MF.bits(), 0b0010000000000000);

        let combined = IPFlags::RF | IPFlags::DF | IPFlags::MF;
        assert_eq!(combined.bits(), 0b1110000000000000);
    }
}
