use bitflags::bitflags;

bitflags! {
    // Bit positions [ RF, DF, MF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct IpFlags: u16 {
        const RF = 1 << 15; // Reserved Flag
        const DF = 1 << 14; // Don't Fragment
        const MF = 1 << 13; // More Fragments
    }
}

impl IpFlags {
    /// Pack the flags and fragment offset into a single u16
    pub fn pack(self, frag_offset: u16) -> u16 {
        self.bits() | (frag_offset & 0x1fff)
    }

    /// Unpack the flags from a single u16
    pub fn unpack(bits: u16) -> Self {
        Self::from_bits_truncate(bits & 0xe000)
    }
}

impl Default for IpFlags {
    fn default() -> Self {
        IpFlags::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_flags_bits() {
        assert_eq!(IpFlags::RF.bits(), 0b1000_0000_0000_0000);
        assert_eq!(IpFlags::DF.bits(), 0b0100_0000_0000_0000);
        assert_eq!(IpFlags::MF.bits(), 0b0010_0000_0000_0000);

        let combined = IpFlags::RF | IpFlags::DF | IpFlags::MF;
        assert_eq!(combined.bits(), 0b1110_0000_0000_0000);
    }

    #[test]
    fn test_ip_flags_pack() {
        let flags = IpFlags::MF; // 0b0010_0000_0000_0000
        let frag_offset: u16 = 10; // 0b0000_0000_0000_1010
        let packed = flags.pack(frag_offset);

        // Top 3 bits are flags; bottom 13 bits are offset.
        assert_eq!(packed & 0b1110_0000_0000_0000, flags.bits());
        assert_eq!(packed & 0b0001_1111_1111_1111, frag_offset);

        // Check entire bits
        assert_eq!(packed, 0b0010_0000_0000_1010);
    }

    #[test]
    fn test_ip_flags_unpack() {
        // MF flag with offset of 10
        let bits: u16 = 0b0010_0000_0000_1010;
        let flags= IpFlags::unpack(bits);
        assert_eq!(flags, IpFlags::MF);
    }

    #[test]
    fn test_ip_flags_empty() {
        let flags = IpFlags::default();
        assert!(flags.is_empty());
    }
}