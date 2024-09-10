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