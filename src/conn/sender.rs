use crate::conn::segment::Segment;
use crate::conn::tcb::Tcb;

pub struct Sender {
    outgoing: Vec<Segment>,
}

impl Sender {
    pub fn new() -> Self {
        Self {
            outgoing: Vec::new(),
        }
    }

    pub fn push_data(&mut self, tcb: &mut Tcb, data: &[u8]) -> Vec<Segment> {
        Vec::new()
    }

    pub fn open(tcb: &mut Tcb) {}

    pub fn close(tcb: &mut Tcb) {}
}
