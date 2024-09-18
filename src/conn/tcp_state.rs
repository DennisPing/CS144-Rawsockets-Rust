#[derive(Debug, PartialEq)]
pub enum TCPState {
    Listen,      // Waiting for SYN
    SynRcvd,     // SYN received, expecting ACK
    SynSent,     // SYN sent, waiting for SYN-ACK
    Established, // Connection established, exchanging data
    CloseWait,   // FIN received, waiting for application to close
    LastAck,     // FIN sent, waiting for ACK
    FinWait1,    // FIN sent, waiting for ACK of FIN or FIN from peer
    FinWait2,    // FIN acknowledged, waiting for FIN from peer
    Closing,     // Both FINs sent, waiting for final ACK
    TimeWait,    // Connection in TIME-WAIT after both FIN and ACK
    Closed,      // Connection closed normally
    Reset,       // Connection reset
}
