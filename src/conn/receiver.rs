use crate::conn::segment::Segment;
use crate::conn::tcb::Tcb;
use crate::socket::byte_stream::ByteStream;
use crate::socket::reassembler::Reassembler;
use crate::tcp::wrap32::Wrap32;

pub struct Receiver {
    reassembler: Reassembler,
}

impl Receiver {
    pub fn new() -> Self {
        Self {
            reassembler: Reassembler::new(ByteStream::new(1024 * 1024 * 1024)),
        }
    }

    pub fn on_segment_recv(&mut self, tcb: &mut Tcb, segment: Segment) {}

    pub fn ack_no(self, tcb: &Tcb) -> Wrap32 {
        tcb.recv_next
    }
}
