use crate::tcp::flags::TCPFlags;
use crate::tcp::header::TCPHeader;
use crate::tcp::reassembler::Reassembler;
use std::io;
use std::io::{Error, ErrorKind, Read, Write};
use crate::tcp::wrap32::Wrap32;

#[derive(Debug)]
pub struct TCPReceiver {
    syn: bool,
    fin: bool,
    reassembler: Reassembler,   // Handles TCP segments
    isn: Option<Wrap32>, // Initial seq number
}

impl TCPReceiver {
    pub fn new(reassembler: Reassembler) -> Self {
        TCPReceiver {
            syn: false,
            fin: false,
            reassembler,
            isn: None,
        }
    }

    /*
    TCP receiver tells the other end:
        - ack no
        - window size
        - rst flag
    */

    pub fn recv(&mut self, tcph: TCPHeader) -> io::Result<usize> {
        // self.reassembler.insert(1, &tcph.payload, false)?;
        Ok(tcph.payload.len())
    }

    pub fn send(&mut self) -> TCPHeader {
        TCPHeader {
            src_port: 0,
            dst_port: 0,
            seq_num: 0,
            ack_num: 0,
            data_offset: 0,
            reserved: 0,
            flags: TCPFlags::SYN,
            window: 0,
            checksum: 0,
            urgent: 0,
            options: Box::from([]),
            payload: Box::from([]),
        }
    }
}

impl Read for TCPReceiver {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reassembler.read(buf)
    }
}

impl Write for TCPReceiver {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.reassembler.get_output().is_closed() {
            return Err(Error::new(ErrorKind::Other, "stream closed"));
        }
        todo!()
    }

    fn flush(&mut self) -> io::Result<()> {
        todo!()
    }
}
