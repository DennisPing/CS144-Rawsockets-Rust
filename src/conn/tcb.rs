use crate::tcp::wrap32::Wrap32;
use std::net::Ipv4Addr;

/// Transmission Control Block holds all the state information for a TCP connection.
pub struct Tcb {
    pub local_ip: Ipv4Addr,
    pub local_port: u16,
    pub remote_ip: Ipv4Addr,
    pub remote_port: u16,
 
    // send sequence numbers
    pub send_isn: Wrap32,     // initial send seq num
    pub send_unacked: Wrap32, // oldest unacked seq num
    pub send_next: Wrap32,    // next seq num to send
    pub send_window: u16,

    // receive sequence numbers
    pub recv_isn: Wrap32,  // initial recv seq num
    pub recv_next: Wrap32, // next seq num expected
}

impl Tcb {
    /// Starts in the SYN-SENT state.
    pub fn client(isn: Wrap32) -> Self {
        Tcb {
            send_isn: isn,
            send_unacked: isn,
            send_next: isn + Wrap32::new(1), // SYN consumed 1 seq no
            send_window: 0,                  // Unknown until received SYN-ACK
            recv_isn: Wrap32::new(0),
            recv_next: Wrap32::new(0),
        }
    }

    /// Starts in the LISTEN state.
    pub fn listener() -> Self {
        Tcb {
            send_isn: Wrap32::new(0), // Unknown until received SYN
            send_unacked: Wrap32::new(0),
            send_next: Wrap32::new(0),
            send_window: 0, // Unknown until received SYN-ACK
            recv_isn: Wrap32::new(0),
            recv_next: Wrap32::new(0),
        }
    }
}
