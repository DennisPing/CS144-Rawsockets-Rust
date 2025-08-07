use std::cell::RefCell;
use std::io::{Read, Write};
use crate::conn::receiver::Receiver;
use crate::conn::sender::Sender;
use crate::conn::tcb::Tcb;
use crate::tcp::wrap32::Wrap32;
use std::net::SocketAddr;
use std::rc::Rc;

pub struct ConnInner<S>
where S: Read + Write {
    socket: S,
    tcb: Rc<Tcb>,
}

pub struct Conn<S>
where
    S: Read + Write,
{
    inner: Rc<RefCell<S>>,
    // tcb: Tcb,
    // sender: Sender,
    // receiver: Receiver,
}

const RECV_CAPACITY: usize = 65535 * 16; // 1 MB

impl Conn {

}
