use crate::conn::byte_stream::ByteStream;
use std::collections::BTreeMap;
use std::io::Write;

#[derive(Debug)]
pub struct StreamReassembler {
    buffer: BTreeMap<usize, Vec<u8>>, // Stores out-of-order segments indexed by their starting byte
    next_idx: usize,
    output: ByteStream,
    eof_received: bool,
    unassembled_bytes_count: usize,
}

impl StreamReassembler {
    pub fn new(output: ByteStream) -> Self {
        StreamReassembler {
            buffer: BTreeMap::new(),
            next_idx: 0,
            output,
            eof_received: false,
            unassembled_bytes_count: 0,
        }
    }

    /// Push a TCP segment into the reassembler.
    pub fn assemble_segment(&mut self, idx: usize, mut data: Vec<u8>, eof: bool) {
        if eof {
            self.eof_received = true;
        }

        // Handle overlapping segments
        if idx < self.next_idx {
            let overlap = self.next_idx - idx;
            if overlap < data.len() {
                data.drain(0..overlap);
            } else {
                return;
            }
        }

        // Insert remaining non-overlapping segments into buffer
        self.unassembled_bytes_count += data.len();
        self.buffer.insert(idx, data);

        // Assemble in-order data
        self.assemble();

        // If the end-of-stream has been reached, and we've assembled all data, close the stream
        if self.eof_received && self.buffer.is_empty() {
            self.output.close();
        }
    }

    /// Return a reference to the output `ByteStream`
    pub fn output(&self) -> &ByteStream {
        &self.output
    }

    pub fn unassembled_bytes(&self) -> usize {
        self.unassembled_bytes_count
    }

    pub fn empty(&self) -> bool {
        self.unassembled_bytes_count == 0
    }

    pub fn first_unassembled(&self) -> usize {
        self.next_idx
    }

    /// Try to assemble in-order segments
    fn assemble(&mut self) {
        while let Some((_, data)) = self.buffer.remove_entry(&self.next_idx) {
            // Only write in-order data
            let len = data.len();
            self.output.write(&data).unwrap();
            self.next_idx += len;
            self.unassembled_bytes_count -= len;
        }
    }
}
