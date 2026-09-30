use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::ops::Range;
use crate::common::byte_stream::ByteStream;

/// Reassembles out-of-order, overlapping, and duplicate segments into an ordered `ByteStream`.
///
/// In-order segments go straight to the `ByteStream`. Early segments are held in the fixed size
/// ring buffer `Box<[u8]>` for staging.
///
/// An interval map `BTreeMap` tracks which ranges are present and which gaps exist.
#[derive(Debug)]
pub struct Reassembler {
    output: ByteStream,
    buffer: Box<[u8]>,              // Ring buffer staging area for out-of-order bytes only
    staged: BTreeMap<usize, usize>, // Disjoint, non-touching intervals: start -> end (absolute)
    next_byte_idx: usize,
    last_byte_idx: Option<usize>,
    bytes_pending: usize,
}

impl Reassembler {
    pub fn new(output: ByteStream) -> Self {
        let capacity = output.remaining_capacity();
        Reassembler {
            output,
            buffer: vec![0u8; capacity].into_boxed_slice(),
            staged: BTreeMap::new(),
            next_byte_idx: 0,
            last_byte_idx: None,
            bytes_pending: 0,
        }
    }

    /// Insert a new data segment at a certain index. The segment is trimmed to the size of the
    /// remaining capacity or the stream end size.
    pub fn insert(&mut self, first_idx: usize, data: &[u8], is_last: bool) -> io::Result<()> {
        if is_last {
            let end = first_idx + data.len();
            self.last_byte_idx = Some(self.last_byte_idx.map_or(end, |old| old.max(end)));
        }
        let room = self.output.remaining_capacity().min(self.buffer.len());
        let stream_end = self.last_byte_idx.unwrap_or(usize::MAX);
        let start = first_idx.max(self.next_byte_idx);
        let end = (first_idx + data.len())
            .min(self.next_byte_idx + room)
            .min(stream_end);

        if start < end {
            let data = &data[start - first_idx..end - first_idx];
            if start == self.next_byte_idx {
                self.output.write_all(data)?;
                self.advance_to(end);
            } else {
                self.stash(start, data);
            }
            self.flush()?;
        }
        if self.is_done() && !self.output.is_closed() {
            self.output.close();
        }
        Ok(())
    }

    /// Get the number of bytes pending in the reassembler.
    pub fn bytes_pending(&self) -> usize {
        self.bytes_pending
    }

    /// The output ByteStream of assembled data.
    pub fn get_output(&self) -> &ByteStream {
        &self.output
    }

    /// Get the next contiguous byte index.
    pub fn next_byte_idx(&self) -> usize {
        self.next_byte_idx
    }

    /// Maps the absolute stream range [start, start + len) to onto the ring buffer.
    /// Returns at most two slot ranges:
    ///
    /// 1. The range to the end of the buffer
    /// 2. The wraparound range from idx 0 (empty if no wrap)
    fn ring(&self, start: usize, len: usize) -> (Range<usize>, Range<usize>) {
        let cap = self.buffer.len();
        let s = start % cap;
        let first = len.min(cap - s);
        (s..s + first, 0..len - first)
    }

    /// Copy bytes into the ring (overwriting is fine: TCP bytes at an index never change),
    /// then merge [start, end) with every interval it overlaps or touches.
    fn stash(&mut self, start: usize, data: &[u8]) {
        let (a, b) = self.ring(start, data.len());
        let (data_a, data_b) = data.split_at(a.len());
        self.buffer[a].copy_from_slice(data_a);
        self.buffer[b].copy_from_slice(data_b);

        let (mut s, mut e) = (start, start + data.len());
        // Left neighbor that reaches us.
        if let Some((&ls, &le)) = self.staged.range(..=s).next_back() {
            if le >= s {
                s = ls;
                e = e.max(le);
                self.staged.remove(&ls);
                self.bytes_pending -= le - ls;
            }
        }
        // Right neighbors we reach.
        while let Some((&rs, &re)) = self.staged.range(s..=e).next() {
            e = e.max(re);
            self.staged.remove(&rs);
            self.bytes_pending -= re - rs;
        }
        self.staged.insert(s, e);
        self.bytes_pending += e - s;
    }

    /// Move the cursor to `new_next`, dropping staged bytes it passed over.
    fn advance_to(&mut self, new_next: usize) {
        while let Some((&s, &e)) = self.staged.first_key_value() {
            if s >= new_next { break; }
            self.staged.pop_first();
            self.bytes_pending -= e - s;
            if e > new_next {
                self.staged.insert(new_next, e);
                self.bytes_pending += e - new_next;
            }
        }
        self.next_byte_idx = new_next;
    }

    /// If the first staged interval starts at the cursor, write it out.
    fn flush(&mut self) -> io::Result<()> {
        if let Some((&s, &e)) = self.staged.first_key_value() {
            if s == self.next_byte_idx {
                let (a, b) = self.ring(s, e - s);
                self.output.write_all(&self.buffer[a])?;
                self.output.write_all(&self.buffer[b])?;
                self.staged.pop_first();
                self.bytes_pending -= e - s;
                self.next_byte_idx = e;
            }
        }
        Ok(())
    }

    fn is_done(&self) -> bool {
        self.last_byte_idx.is_some_and(|last| self.next_byte_idx >= last)
    }
}

impl Read for Reassembler {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.output.read(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::seq::SliceRandom;
    use rand::{Rng, RngExt};
    use std::io::Read;

    fn create_reassembler(capacity: usize) -> Reassembler {
        let stream = ByteStream::new(capacity);
        Reassembler::new(stream)
    }

    fn read_all_as_string(reassembler: &mut Reassembler) -> String {
        let mut buf = vec![];
        reassembler.read_to_end(&mut buf).unwrap();
        std::str::from_utf8(&buf).unwrap().to_owned()
    }

    // -- Test insert and capacity --

    #[test]
    fn test_insert_empty_data() {
        let mut ra = create_reassembler(32);
        ra.insert(0, &[], false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        assert!(!ra.output.eof());
    }

    #[test]
    fn test_insert_within_capacity() {
        let mut ra = create_reassembler(5);

        // Insert first
        ra.insert(0, b"Hello", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 5);
        assert_eq!(ra.next_byte_idx(), 5);
        assert_eq!(ra.bytes_pending(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("Hello", actual);

        // Insert second
        ra.insert(5, b"World", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 10);
        assert_eq!(ra.next_byte_idx(), 10);
        assert_eq!(ra.bytes_pending(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("World", actual);

        // Insert third
        ra.insert(10, b"Honda", true).unwrap();
        assert_eq!(ra.output.bytes_written(), 15);
        assert_eq!(ra.next_byte_idx(), 15);
        assert_eq!(ra.bytes_pending(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("Honda", actual);

        let output = ra.get_output();
        assert!(output.is_closed());
        assert!(output.eof());
    }

    #[test]
    fn test_insert_beyond_capacity() {
        let mut ra = create_reassembler(5);

        // Insert first
        ra.insert(0, b"Hello", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 5);
        assert_eq!(ra.bytes_pending(), 0);

        // Insert second; no-op because capacity exceeded
        ra.insert(5, b"World", true).unwrap();
        assert_eq!(ra.output.bytes_written(), 5);
        assert_eq!(ra.bytes_pending(), 0);

        // Read out all data
        let actual = read_all_as_string(&mut ra);
        assert_eq!("Hello", actual);

        // Insert third; success
        ra.insert(5, b"World", true).unwrap();
        assert_eq!(ra.output.bytes_written(), 10);
        assert_eq!(ra.bytes_pending(), 0);

        // Read out all data
        let actual = read_all_as_string(&mut ra);
        assert_eq!("World", actual);

        assert!(ra.output.eof());
    }

    #[test]
    fn test_capacity_overlapping_inserts() {
        let mut ra = create_reassembler(1);

        // Insert first
        ra.insert(0, b"ab", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 1);
        assert_eq!(ra.bytes_pending(), 0);

        // Insert second; no-op because capacity exceeded
        ra.insert(0, b"ab", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 1);
        assert_eq!(ra.bytes_pending(), 0);

        // Read out all data
        let actual = read_all_as_string(&mut ra);
        assert_eq!(ra.output.bytes_read(), 1);
        assert_eq!("a", actual);

        // Insert third
        ra.insert(0, b"abc", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 2);
        assert_eq!(ra.bytes_pending(), 0);

        // Read out all data
        let actual = read_all_as_string(&mut ra);
        assert_eq!(ra.output.bytes_read(), 2);
        assert_eq!("b", actual);
    }

    #[test]
    fn test_insert_beyond_capacity_with_different_data() {
        let mut ra = create_reassembler(2);

        ra.insert(1, b"b", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 1);

        ra.insert(2, b"bX", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 1);

        ra.insert(0, b"a", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 2);
        assert_eq!(ra.bytes_pending(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("ab", actual);

        ra.insert(1, b"bc", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 3);
        assert_eq!(ra.bytes_pending(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("c", actual);
    }

    #[test]
    fn test_insert_last_segment_beyond_capacity() {
        let mut ra = create_reassembler(2);

        ra.insert(1, b"bc", true).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 1);

        ra.insert(0, b"a", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 2);
        assert_eq!(ra.bytes_pending(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("ab", actual);

        ra.insert(1, b"bc", true).unwrap();
        assert_eq!(ra.output.bytes_written(), 3);
        assert_eq!(ra.bytes_pending(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("c", actual);

        assert!(ra.output.eof());
    }

    #[test]
    fn test_insert_junk_after_close() {
        let mut ra = create_reassembler(32);

        ra.insert(0, b"abcd", false).unwrap();
        ra.insert(4, b"efgh", true).unwrap();
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcdefgh", actual);
        assert!(ra.output.eof());

        // Verify code doesn't blow up
        let result = ra.insert(8, b"zzz", false);
        assert!(result.is_ok());

        // Verify nothing gets read
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);
    }

    // -- Test sequential --

    #[test]
    fn test_sequential() {
        let mut ra = create_reassembler(32);

        ra.insert(0, b"abcd", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcd", actual);

        ra.insert(4, b"efgh", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("efgh", actual);
    }

    #[test]
    fn test_sequential_combined() {
        let mut ra = create_reassembler(32);

        ra.insert(0, b"abcd", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);

        ra.insert(4, b"efgh", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);

        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcdefgh", actual);
    }

    #[test]
    fn test_sequential_combined_loop() {
        let mut ra = create_reassembler(4096);
        let mut combined_data = String::new();

        for i in 0..100 {
            let total_writes = 4 * i;
            assert_eq!(ra.output.bytes_written(), total_writes);
            ra.insert(4 * i, b"abcd", false).unwrap();
            combined_data.push_str("abcd");
        }

        let actual = read_all_as_string(&mut ra);
        assert_eq!(combined_data, actual);
    }

    #[test]
    fn test_sequential_immediate_read_loop() {
        let mut ra = create_reassembler(4096);

        for i in 0..100 {
            let total_writes = 4 * i;
            assert_eq!(ra.output.bytes_written(), total_writes);
            ra.insert(4 * i, b"abcd", false).unwrap();
            let actual = read_all_as_string(&mut ra);
            assert_eq!("abcd", actual);
        }
    }

    // -- Test duplicates --

    #[test]
    fn test_dup_at_same_index() {
        let mut ra = create_reassembler(32);

        // Insert new data
        ra.insert(0, b"abcd", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);

        // Read out data
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcd", actual);

        // Insert duplicate data at same index
        ra.insert(0, b"abcd", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);

        // Read out data, should be empty string
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);
    }

    #[test]
    fn test_dup_at_multiple_indexes() {
        let mut ra = create_reassembler(32);

        // Insert new data
        ra.insert(0, b"abcd", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcd", actual);

        // Insert data at index 4
        ra.insert(4, b"abcd", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcd", actual);

        // Insert duplicate data at index 0
        ra.insert(0, b"abcd", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);

        // Insert duplicate data at index 4
        ra.insert(4, b"abcd", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);
    }

    #[test]
    fn test_dup_random_indexes() {
        let mut ra = create_reassembler(32);

        let data = b"abcdefgh";

        ra.insert(0, data, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcdefgh", actual);

        // Perform 1000 random insertions
        let mut rng = rand::rng();
        for _ in 0..1000 {
            let j = rng.random_range(0..8);
            let k = rng.random_range(j..8);

            let chunk = &data[j..k];
            ra.insert(j, chunk, false).unwrap();
            assert_eq!(ra.output.bytes_written(), 8);

            let actual = read_all_as_string(&mut ra);
            assert_eq!("", actual);
            assert!(!ra.output.eof());
        }
    }

    #[test]
    fn test_dup_overlapping_segments_beyond_existing_data() {
        let mut ra = create_reassembler(32);

        ra.insert(0, b"abcd", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcd", actual);

        // Insert overlapping data that goes beyond existing data
        ra.insert(0, b"abcdef", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 6);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("ef", actual);
    }

    // -- Test holes --

    #[test]
    fn test_insert_with_initial_gap() {
        let mut ra = create_reassembler(32);

        ra.insert(1, b"b", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);
    }

    #[test]
    fn test_fill_initial_gap() {
        let mut ra = create_reassembler(32);

        ra.insert(1, b"b", false).unwrap();
        ra.insert(0, b"a", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 2);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("ab", actual);
    }

    #[test]
    fn test_fill_gap_with_last() {
        let mut ra = create_reassembler(32);

        ra.insert(1, b"b", true).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);

        ra.insert(0, b"a", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 2);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("ab", actual);
        assert!(ra.output.eof());
    }

    #[test]
    fn test_fill_gap_with_overlapping_data() {
        let mut ra = create_reassembler(32);

        ra.insert(1, b"b", false).unwrap();
        ra.insert(0, b"ab", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 2);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("ab", actual);
    }

    #[test]
    fn test_fill_multiple_gaps_with_chunks() {
        let mut ra = create_reassembler(32);

        ra.insert(1, b"b", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);

        ra.insert(3, b"d", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);

        ra.insert(0, b"abc", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcd", actual);

        // Insert empty data for last segment
        ra.insert(4, b"", true).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);
    }

    // -- Test overlapping segments --

    #[test]
    fn test_overlap_extend() {
        let mut ra = create_reassembler(32);

        ra.insert(0, b"Hello", false).unwrap();
        ra.insert(0, b"HelloWorld", false).unwrap();

        assert_eq!(ra.output.bytes_written(), 10);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("HelloWorld", actual);
    }

    #[test]
    fn test_overlap_extend_after_read() {
        let mut ra = create_reassembler(32);

        ra.insert(0, b"Hello", false).unwrap();
        let actual = read_all_as_string(&mut ra);
        assert_eq!("Hello", actual);

        ra.insert(0, b"HelloWorld", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 10);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("World", actual);
    }

    #[test]
    fn test_overlap_fill_gap() {
        let mut ra = create_reassembler(32);

        ra.insert(5, b"World", false).unwrap();
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);

        ra.insert(0, b"Hello", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 10);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("HelloWorld", actual);
    }

    #[test]
    fn test_overlap_partial() {
        let mut ra = create_reassembler(32);

        ra.insert(5, b"World", false).unwrap();
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);

        ra.insert(0, b"Hello", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 10);

        ra.insert(8, b"ldHondaCivic", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 20);

        let actual = read_all_as_string(&mut ra);
        assert_eq!("HelloWorldHondaCivic", actual);
    }

    #[test]
    fn test_overlap_between_two_pending() {
        let mut ra = create_reassembler(32);

        ra.insert(1, b"bc", false).unwrap();
        ra.insert(4, b"ef", false).unwrap();
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 4);

        ra.insert(2, b"cde", false).unwrap();
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 5);

        // _bc_ef
        // __cde_ (overlap in the middle between two pending)

        ra.insert(0, b"a", false).unwrap();
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcdef", actual);
        assert_eq!(ra.output.bytes_written(), 6);
        assert_eq!(ra.bytes_pending(), 0);
    }

    #[test]
    fn test_overlap_many_pending() {
        let mut ra = create_reassembler(32);

        ra.insert(4, b"efgh", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 4);

        ra.insert(14, b"op", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 6);

        ra.insert(18, b"s", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 7);

        ra.insert(0, b"a", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 1);
        assert_eq!(ra.bytes_pending(), 7);

        ra.insert(0, b"abcde", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        assert_eq!(ra.bytes_pending(), 3);

        ra.insert(14, b"opqrst", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        assert_eq!(ra.bytes_pending(), 6);

        ra.insert(14, b"op", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        assert_eq!(ra.bytes_pending(), 6);

        ra.insert(8, b"ijklmn", false).unwrap();
        assert_eq!(ra.output.bytes_written(), 20);
        assert_eq!(ra.bytes_pending(), 0);
    }

    #[test]
    fn test_random_shuffle() {
        let n_reps = 32;
        let n_segs = 128;
        let max_seg_len = 2048;
        let max_offset_shift = 1023; // Maximum shift to introduce overlaps

        let mut rng = rand::rng();
        for _ in 0..n_reps {
            let capacity = n_segs * max_seg_len;
            let mut ra = create_reassembler(capacity);

            let mut segments: Vec<(usize, usize)> = Vec::with_capacity(n_segs);
            let mut total_len = 0;

            // Generate segments with possible overlaps
            for _ in 0..n_segs {
                let seg_len = 1 + rng.random_range(0..max_seg_len - 1);
                let shift = total_len.min(1 + rng.random_range(0..max_offset_shift));
                let start = total_len - shift;
                let seg_size = seg_len + shift;
                segments.push((start, seg_size));

                total_len += seg_len;
            }

            // Shuffle segments to simulate out of order receives
            segments.shuffle(&mut rng);

            // Generate random data
            let mut payload = vec![0u8; total_len];
            rng.fill_bytes(&mut payload);

            // Insert each shuffled segment into the Reassembler
            for (start, size) in segments {
                let slice = &payload[start..(start + size)];
                let is_last = start + size == total_len;
                ra.insert(start, slice, is_last)
                    .expect("Insert into Reassembler failed");
            }

            // Read out all data
            let mut buf = vec![];
            ra.read_to_end(&mut buf).expect("Read to end failed");
            assert_eq!(payload.len(), buf.len());
            assert_eq!(payload, buf);
        }
    }
}