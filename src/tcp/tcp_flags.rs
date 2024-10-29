use bitflags::bitflags;

bitflags! {
    // Bit positions [ CWR, ECE, URG, ACK, PSH, RST, SYN, FIN ]
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct TcpFlags: u8 {
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
    use crate::tcp::tcp_flags::TcpFlags;

    #[test]
    fn test_tcp_flags() {
        assert_eq!(TcpFlags::FIN.bits(), 0b00000001);
        assert_eq!(TcpFlags::SYN.bits(), 0b00000010);
        assert_eq!(TcpFlags::RST.bits(), 0b00000100);
        assert_eq!(TcpFlags::PSH.bits(), 0b00001000);
        assert_eq!(TcpFlags::ACK.bits(), 0b00010000);
        assert_eq!(TcpFlags::URG.bits(), 0b00100000);
        assert_eq!(TcpFlags::ECE.bits(), 0b01000000);
        assert_eq!(TcpFlags::CWR.bits(), 0b10000000);

        let combined = TcpFlags::FIN
            | TcpFlags::SYN
            | TcpFlags::RST
            | TcpFlags::PSH
            | TcpFlags::ACK
            | TcpFlags::URG
            | TcpFlags::ECE
            | TcpFlags::CWR;
        assert_eq!(combined.bits(), 0b11111111);
    }
}
