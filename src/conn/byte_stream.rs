use std::collections::VecDeque;
use std::io;
use std::io::{Read, Write};

#[derive(Debug)]
pub struct ByteStream {
    buffer: VecDeque<u8>,
    capacity: usize,
    closed: bool,
}

impl ByteStream {
    pub fn new(capacity: usize) -> Self {
        ByteStream {
            buffer: VecDeque::new(),
            capacity,
            closed: false,
        }
    }

    /// Push N bytes into the byte stream
    pub fn write_bytes(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.closed {
            return Err(io::Error::new(io::ErrorKind::Other, "stream closed"));
        }
        let available = self.capacity - self.buffer.len();
        let to_write = data.len().min(available);
        self.buffer.extend(&data[..to_write]);
        Ok(to_write)
    }

    /// Consume N bytes from the byte stream
    pub fn read_bytes(&mut self, amount: usize) -> Vec<u8> {
        let to_read = amount.min(self.buffer.len());
        self.buffer.drain(0..to_read).collect()
    }

    /// Peak N bytes without consuming them
    pub fn peek(&self, amount: usize) -> Vec<u8> {
        let to_read = amount.min(self.buffer.len());
        self.buffer.iter().take(to_read).cloned().collect()
    }

    /// Close the byte stream
    pub fn close(&mut self) {
        self.closed = true;
    }

    /// The remaining capacity in the underlying buffer
    pub fn remaining_capacity(&self) -> usize {
        self.capacity - self.buffer.len()
    }

    /// The number of bytes still available in the buffer (not consumed yet)
    pub fn bytes_available(&self) -> usize {
        self.buffer.len()
    }
}

impl Read for ByteStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let to_read = buf.len().min(self.buffer.len());
        let drained: Vec<u8> = self.buffer.drain(0..to_read).collect();
        buf[..to_read].copy_from_slice(&drained);
        Ok(to_read)
    }
}

impl Write for ByteStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_bytes(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
