use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct TCPFlags: u8 {
        const CWR = 0b1000_0000;
        const ECE = 0b0100_0000;
        const URG = 0b0010_0000;
        const ACK = 0b0001_0000;
        const PSH = 0b0000_1000;
        const RST = 0b0000_0100;
        const SYN = 0b0000_0010;
        const FIN = 0b0000_0001;
    }
}

// Unit tests *****************************************************************

#[test]
fn test_tcp_flags() {
    assert_eq!(TCPFlags::FIN.bits(),0b00000001);
    assert_eq!(TCPFlags::SYN.bits(),0b00000010);
    assert_eq!(TCPFlags::RST.bits(),0b00000100);
    assert_eq!(TCPFlags::PSH.bits(),0b00001000);
    assert_eq!(TCPFlags::ACK.bits(),0b00010000);
    assert_eq!(TCPFlags::URG.bits(),0b00100000);
    assert_eq!(TCPFlags::ECE.bits(),0b01000000);
    assert_eq!(TCPFlags::CWR.bits(),0b10000000);

    let combined = TCPFlags::FIN | TCPFlags::SYN | TCPFlags::RST | TCPFlags::PSH |
        TCPFlags::ACK | TCPFlags::URG | TCPFlags::ECE | TCPFlags::CWR;
    assert_eq!(combined.bits(), 0b11111111);
}