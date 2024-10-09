use bitflags::bitflags;

bitflags! {
    // Bit positions [ CWR, ECE, URG, ACK, PSH, RST, SYN, FIN ]
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct TCPFlags: u8 {
        const CWR = 1 << 7;
        const ECE = 1 << 6;
        const URG = 1 << 5;
        const ACK = 1 << 4;
        const PSH = 1 << 3;
        const RST = 1 << 2;
        const SYN = 1 << 1;
        const FIN = 1 << 0;
    }
}

// -- Unit tests --

#[cfg(test)]
mod tests {
    use crate::tcp::flags::TCPFlags;

    #[test]
    fn test_tcp_flags() {
        assert_eq!(TCPFlags::FIN.bits(), 0b00000001);
        assert_eq!(TCPFlags::SYN.bits(), 0b00000010);
        assert_eq!(TCPFlags::RST.bits(), 0b00000100);
        assert_eq!(TCPFlags::PSH.bits(), 0b00001000);
        assert_eq!(TCPFlags::ACK.bits(), 0b00010000);
        assert_eq!(TCPFlags::URG.bits(), 0b00100000);
        assert_eq!(TCPFlags::ECE.bits(), 0b01000000);
        assert_eq!(TCPFlags::CWR.bits(), 0b10000000);

        let combined = TCPFlags::FIN
            | TCPFlags::SYN
            | TCPFlags::RST
            | TCPFlags::PSH
            | TCPFlags::ACK
            | TCPFlags::URG
            | TCPFlags::ECE
            | TCPFlags::CWR;
        assert_eq!(combined.bits(), 0b11111111);
    }
}
