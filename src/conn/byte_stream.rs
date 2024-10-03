use std::collections::VecDeque;
use std::io::{self, Error, ErrorKind, Read, Write};

/// An in-order byte stream
#[derive(Debug)]
pub struct ByteStream {
    buffer: VecDeque<u8>,
    capacity: usize,
    bytes_written: usize,
    bytes_read: usize,
    closed: bool,
}

impl ByteStream {
    /// New `ByteStream` with capacity `N`
    pub fn new(capacity: usize) -> Self {
        ByteStream {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
            bytes_written: 0,
            bytes_read: 0,
            closed: false, // It's always the producer's job to close the byte stream, never the consumer
        }
    }

    /// Remove `N` bytes from the byte stream and return the actual number of bytes popped
    pub fn pop_output(&mut self, len: usize) -> usize {
        let to_pop = len.min(self.buffer.len());
        self.buffer.drain(..to_pop);
        self.bytes_read += to_pop;
        to_pop
    }

    /// Peek `N` bytes without consuming them and return a new vector of bytes peeked
    pub fn peek_output(&self, amount: usize) -> Vec<u8> {
        let to_peek = amount.min(self.buffer.len());
        self.buffer.iter().take(to_peek).cloned().collect()
    }

    /// The remaining capacity in the byte stream
    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.buffer.len())
    }

    /// Close the byte stream
    pub fn close(&mut self) {
        self.closed = true;
    }

    /// Is the byte stream closed?
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// The length of the buffer (number of bytes not consumed yet)
    pub fn buffer_size(&self) -> usize {
        self.buffer.len()
    }

    /// Is the byte stream empty?
    pub fn is_buffer_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Is the end of the byte stream reached?
    pub fn eof(&self) -> bool {
        self.closed && self.is_buffer_empty()
    }

    /// The total number of bytes written
    pub fn bytes_written(&self) -> usize {
        self.bytes_written
    }

    /// The total number of bytes read
    pub fn bytes_read(&self) -> usize {
        self.bytes_read
    }
}

impl Read for ByteStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let to_read = buf.len().min(self.buffer.len());

        if to_read > 0 {
            // Make ring buffer contiguous if not already
            let contiguous = self.buffer.make_contiguous();
            buf[..to_read].copy_from_slice(&contiguous[..to_read]);
            self.buffer.drain(..to_read);
            self.bytes_read += to_read;
            Ok(to_read)
        } else {
            Ok(0)
        }
    }
}

impl Write for ByteStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.closed {
            return Err(Error::new(ErrorKind::Other, "stream closed"));
        }
        let available = self.remaining_capacity();
        let to_write = buf.len().min(available);
        self.buffer.extend(&buf[..to_write]);
        self.bytes_written += to_write;
        Ok(to_write)
    }
    
    fn flush(&mut self) -> io::Result<()> {
        Ok(()) // no-op because this is an in-memory data structure
    }
}

// -- Unit tests --

#[cfg(test)]
mod tests {
    use crate::conn::byte_stream::ByteStream;
    use std::io::{ErrorKind, Read, Write};

    fn generate_data(size: usize) -> Vec<u8> {
        (0..size as u8).collect()
    }

    #[test]
    fn test_construction() {
        let bs = ByteStream::new(100);
        assert_eq!(bs.remaining_capacity(), 100);
        assert_eq!(bs.buffer_size(), 0);
        assert_eq!(bs.bytes_written(), 0);
        assert_eq!(bs.bytes_read(), 0);
        assert!(!bs.is_closed());
        assert!(bs.is_buffer_empty());
        assert!(!bs.eof());
    }

    #[test]
    fn test_remaining_capacity() {
        let mut bs = ByteStream::new(10);
        assert_eq!(bs.remaining_capacity(), 10);

        let data = generate_data(4);
        bs.write(&data).unwrap();
        assert_eq!(bs.remaining_capacity(), 6);

        let data = generate_data(6);
        bs.write(&data).unwrap();
        assert_eq!(bs.remaining_capacity(), 0);

        assert_eq!(bs.buffer_size(), 10);
    }

    #[test]
    fn test_single_write_and_read() {
        let mut bs = ByteStream::new(20);
        let data = b"hello world";
        let n_written = bs.write(data).unwrap();
        assert_eq!(n_written, data.len());
        assert_eq!(bs.bytes_written(), data.len());
        assert_eq!(bs.buffer_size(), data.len());

        let mut buf = vec![0; data.len()];
        let n_read = bs.read(&mut buf).unwrap();
        assert_eq!(n_written, data.len());
        assert_eq!(buf, data);
        assert_eq!(bs.bytes_read(), n_read);
        assert!(bs.is_buffer_empty());
    }

    #[test]
    fn test_many_writes_and_reads() {
        let mut bs = ByteStream::new(1024);
        let chunk_size = 64;
        let num_chunks = 10;

        // Check write
        for i in 1..num_chunks {
            let data = generate_data(chunk_size);
            let n_written = bs.write(&data).unwrap();

            // Check write
            assert_eq!(n_written, chunk_size);
            assert_eq!(bs.bytes_written(), i * chunk_size);
            assert_eq!(bs.buffer_size(), i * chunk_size);
        }

        // Check read
        for i in 1..num_chunks {
            let mut buf = vec![0; chunk_size];
            let n_read = bs.read(&mut buf).unwrap();
            assert_eq!(n_read, chunk_size);

            let expected_data: Vec<u8> = (0..chunk_size as u8).collect();
            assert_eq!(buf, expected_data);
            assert_eq!(bs.bytes_read(), i * chunk_size);
        }

        assert!(bs.is_buffer_empty())
    }

    #[test]
    fn test_write_over_capacity() {
        let capacity = 20;
        let mut bs = ByteStream::new(capacity);
        let data = generate_data(50);
        let n_written = bs.write(&data).unwrap();
        assert_eq!(n_written, capacity);
        assert_eq!(bs.bytes_written(), capacity);
        assert_eq!(bs.buffer_size(), capacity);

        // Write again to overflow
        let n_written = bs.write(&data).unwrap();
        assert_eq!(n_written, 0);
    }

    #[test]
    fn test_pop_output() {
        let mut bs = ByteStream::new(20);
        let data = b"hello world";
        bs.write(data).unwrap();
        assert_eq!(bs.buffer_size(), data.len());

        let n_popped = bs.pop_output(5);
        assert_eq!(n_popped, 5);
        assert_eq!(bs.bytes_read(), 5);
        assert_eq!(bs.buffer_size(), 6);

        let n_popped = bs.pop_output(99); // Request more than available
        assert_eq!(n_popped, 6);
        assert_eq!(bs.bytes_read(), 11);
        assert!(bs.is_buffer_empty());
    }

    #[test]
    fn test_peek_output() {
        let mut bs = ByteStream::new(20);
        let data = b"hello world";
        bs.write(data).unwrap();
        assert_eq!(bs.buffer_size(), data.len());

        let peeked = bs.peek_output(5);
        assert_eq!(peeked, b"hello");

        let peeked = bs.peek_output(15); // Peek more than available
        assert_eq!(peeked, b"hello world");
    }

    #[test]
    fn test_close() {
        let mut bs = ByteStream::new(20);
        bs.close();
        assert!(bs.is_closed());

        // Attempt to write after closing
        let data = b"hello world";
        let result = bs.write(data);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::Other);
    }

    #[test]
    fn test_eof() {
        let mut bs = ByteStream::new(20);
        assert!(!bs.eof());

        // Write and read all data without closing
        let data = b"hello world";
        bs.write(data).unwrap();

        let mut buf = vec![0; data.len()];
        bs.read(&mut buf).unwrap();
        assert!(!bs.eof());

        bs.close();
        assert!(bs.eof());
    }

    #[test]
    fn test_make_contiguous() {
        let mut bs = ByteStream::new(20);
        let data1 = b"abc";
        let data2 = b"defg";
        bs.write(data1).unwrap();
        bs.write(data2).unwrap();
        assert_eq!(bs.buffer_size(), 7);

        // Read 2 bytes
        let mut read_buf = vec![0; 2];
        bs.read(&mut read_buf).unwrap();
        assert_eq!(read_buf, b"ab");
        assert_eq!(bs.buffer_size(), 5);

        // Write more bytes
        let data3 = b"hi";
        bs.write(data3).unwrap();
        assert_eq!(bs.buffer_size(), 7);

        // Now make contiguous and read all
        let mut read_buf = vec![0; 7];
        bs.read(&mut read_buf).unwrap();
        assert_eq!(read_buf, b"cdefghi");

        assert!(bs.flush().is_ok()); // No-op flush
    }
}
