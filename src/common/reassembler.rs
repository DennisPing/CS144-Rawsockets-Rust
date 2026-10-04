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
///
/// # How memory is laid out
///
/// The diagram represents a stream.
///
/// ```text
/// capacity = 8, next_byte_idx = 2, staged_ranges = { 4 => 7, 9 => 10 }
///
/// Absolute stream view (what staged_ranges describes):
///
///   index:     0   1 | 2   3   4   5   6   7   8   9 | 10 ...
///   byte:      a   b | .   .   e   f   g   .   .   j |
///            written | <------- window (8) --------> | not accepted
///                      ^ next_byte_idx
///                              [4, 7)              [9, 10)
///
/// Ring buffer view (where staged bytes physically live, slot = index % 8):
///
///   slot:      0   1   2   3   4   5   6   7
///   byte:      .   j   .   .   e   f   g   .
///                  ^ index 9 wrapped around to slot 1
/// ```
///
/// The window is never wider than the buffer, so two bytes in the window never share a slot.
#[derive(Debug)]
pub struct Reassembler {
    output: ByteStream,
    buffer: Box<[u8]>,                     // Ring buffer staging area for out-of-order bytes only
    staged_ranges: BTreeMap<usize, usize>, // Disjoint, non-touching ranges: start -> end (absolute)
    next_byte_idx: usize,                  // Absolute index of the next byte the output needs
    stream_end_idx: Option<usize>,         // Absolute index one past the final byte, once known
    bytes_pending: usize,                  // Total length of all staged ranges
}

impl Reassembler {
    pub fn new(output: ByteStream) -> Self {
        // The window can never be wider than the output's capacity, so the ring matches it
        let capacity = output.remaining_capacity();
        Reassembler {
            output,
            buffer: vec![0u8; capacity].into_boxed_slice(),
            staged_ranges: BTreeMap::new(),
            next_byte_idx: 0,
            stream_end_idx: None,
            bytes_pending: 0,
        }
    }

    /// Insert a new data segment at a certain index. The segment is trimmed to the size of the
    /// remaining window or the stream end index.
    ///
    /// ```text
    /// next_byte_idx = 2, window_len = 8, insert(0, "abcdefghijkl")
    ///
    ///   index:     0   1 | 2   3   4   5   6   7   8   9 | 10  11
    ///   segment:   a   b | c   d   e   f   g   h   i   j | k   l
    ///            already |      kept: [start, end)       | no room,
    ///            written |                               | dropped
    /// ```
    pub fn insert(&mut self, first_idx: usize, data: &[u8], is_last: bool) -> io::Result<()> {
        let segment_end = first_idx + data.len();
        if is_last {
            // Remember where the stream ends; max guards against a conflicting retransmit
            let stream_end = self.stream_end_idx.map_or(segment_end, |old| old.max(segment_end));
            self.stream_end_idx = Some(stream_end);
        }
        // How many bytes past the cursor we are allowed to accept?
        let window_len = self.output.remaining_capacity().min(self.buffer.len());
        let stream_end = self.stream_end_idx.unwrap_or(usize::MAX);
        // Skip any prefix that was already written
        let start = first_idx.max(self.next_byte_idx);
        // Cut any suffix that is past the window or past the end of the stream
        let end = segment_end
            .min(self.next_byte_idx + window_len)
            .min(stream_end);

        // Something survived the trimming
        if start < end {
            // Re-slice so that data[0] is the byte at absolute index `start`
            let data = &data[start - first_idx..end - first_idx];
            if start == self.next_byte_idx {
                // In-order bytes skip the ring buffer and go straight to the output
                self.output.write_all(data)?;
                self.advance_to(end);
            } else {
                // A gap sits before these bytes, so park them in the ring buffer
                self.stage(start, data);
            }
            // The cursor may now sit at the start of a staged range
            self.write_staged_at_cursor()?;
        }
        // Close the output once the cursor reaches the end of the stream
        if self.is_done() && !self.output.is_closed() {
            self.output.close();
        }
        Ok(())
    }

    /// Get the number of bytes pending in the reassembler.
    pub fn bytes_pending(&self) -> usize {
        self.bytes_pending
    }

    /// Get the output ByteStream of assembled data.
    pub fn output(&self) -> &ByteStream {
        &self.output
    }

    /// Get the next contiguous byte index.
    pub fn next_byte_idx(&self) -> usize {
        self.next_byte_idx
    }

    /// Maps the absolute stream range [start, start + len) onto the ring buffer.
    /// Returns at most two slot ranges:
    ///
    /// 1. The range to the end of the buffer
    /// 2. The wraparound range from idx 0 (empty if no wrap)
    ///
    /// ```text
    /// slot_ranges(start = 6, len = 4), capacity = 8  ->  (6..8, 0..2)
    ///
    ///   index:     0   1   2   3   4   5   6   7   8   9
    ///   stream:    a   b   c   d   e   f   g   h   i   j
    ///                                      [--- len 4 --)
    ///
    ///   slot:      0   1   2   3   4   5   6   7
    ///   byte:      i   j   .   .   .   .   g   h
    ///              [0..2)                  [6..8)
    ///              after_wrap              before_wrap
    /// ```
    fn slot_ranges(&self, start: usize, len: usize) -> (Range<usize>, Range<usize>) {
        let capacity = self.buffer.len();
        // Buffer slot of the first byte
        let first_slot = start % capacity;
        // How many bytes fit before running off the end of the buffer?
        let len_before_wrap = len.min(capacity - first_slot);
        // The part up to the buffer end, then the leftover part starting again at slot 0
        (first_slot..first_slot + len_before_wrap, 0..len - len_before_wrap)
    }

    /// Copy bytes into the ring. Overwriting is fine.
    /// Then merge [start, end) with every range it overlaps or touches.
    ///
    /// ```text
    /// stage(5, "fgh")
    ///
    ///   index:     0   1   2   3   4   5   6   7   8   9
    ///   before:    .   .   .   d   e   .   .   .   i   j    { 3 => 5, 8 => 10 }
    ///   new:       .   .   .   .   .   f   g   h   .   .    touches both neighbors
    ///   after:     .   .   .   d   e   f   g   h   i   j    { 3 => 10 }
    /// ```
    fn stage(&mut self, start: usize, data: &[u8]) {
        // Split the data at the wrap point so each half is one contiguous copy
        let (before_wrap, after_wrap) = self.slot_ranges(start, data.len());
        let (data_before_wrap, data_after_wrap) = data.split_at(before_wrap.len());
        self.buffer[before_wrap].copy_from_slice(data_before_wrap);
        self.buffer[after_wrap].copy_from_slice(data_after_wrap);

        // The range to record, which grows as neighboring ranges are absorbed
        let (mut merged_start, mut merged_end) = (start, start + data.len());
        // Left neighbor: the range with the greatest start <= ours (only one can reach us)
        if let Some((&left_start, &left_end)) =
            self.staged_ranges.range(..=merged_start).next_back()
        {
            // It overlaps or touches our start, so absorb it
            if left_end >= merged_start {
                merged_start = left_start;
                merged_end = merged_end.max(left_end);
                self.remove_range(left_start);
            }
        }
        // Right neighbors: absorb every range that starts inside ours (merged_end may grow)
        while let Some((&right_start, &right_end)) =
            self.staged_ranges.range(merged_start..=merged_end).next()
        {
            merged_end = merged_end.max(right_end);
            self.remove_range(right_start);
        }
        // Absorbed ranges were removed above, so overlapping bytes are counted only once
        self.add_range(merged_start, merged_end);
    }

    /// Move the cursor to `new_next_idx`, dropping staged bytes it passed over.
    ///
    /// ```text
    /// insert(0, "abcd") wrote a..d straight to the output, then called advance_to(4)
    ///
    ///   index:     0   1   2   3   4   5   6   7   8   9
    ///   before:    .   b   .   d   e   .   .   .   i   j    { 1 => 2, 3 => 5, 8 => 10 }
    ///   cursor:                    ^ new_next_idx = 4
    ///   after:     -   -   -   -   e   .   .   .   i   j    { 4 => 5, 8 => 10 }
    ///
    ///   b is dropped, d is trimmed off the front of [3, 5), and [8, 10) is untouched
    /// ```
    fn advance_to(&mut self, new_next_idx: usize) {
        while let Some((&range_start, &range_end)) = self.staged_ranges.first_key_value() {
            // Ranges are sorted, so stop at the first one the cursor has not reached
            if range_start >= new_next_idx { break; }
            self.remove_range(range_start);
            // A range straddling the cursor keeps its unwritten tail, now starting at the cursor
            if range_end > new_next_idx {
                self.add_range(new_next_idx, range_end);
            }
        }
        self.next_byte_idx = new_next_idx;
    }

    /// If the first staged range starts at the cursor, write it out.
    ///
    /// ```text
    ///   index:     0   1   2   3   4   5   6   7   8   9
    ///   before:    -   -   -   -   e   .   .   .   i   j    { 4 => 5, 8 => 10 }
    ///                              ^ next_byte_idx = 4, a staged range starts here
    ///   after:     -   -   -   -   -   .   .   .   i   j    { 8 => 10 }
    ///                                  ^ next_byte_idx = 5, e is now in the output
    /// ```
    fn write_staged_at_cursor(&mut self) -> io::Result<()> {
        // Only the first range can start at the cursor, and the one after it has a gap before it
        if let Some((&range_start, &range_end)) = self.staged_ranges.first_key_value() {
            if range_start == self.next_byte_idx {
                // Two writes because the range may wrap around the end of the buffer
                let (before_wrap, after_wrap) =
                    self.slot_ranges(range_start, range_end - range_start);
                self.output.write_all(&self.buffer[before_wrap])?;
                self.output.write_all(&self.buffer[after_wrap])?;
                self.remove_range(range_start);
                self.next_byte_idx = range_end;
            }
        }
        Ok(())
    }

    /// Record [start, end) as staged and count its bytes as pending.
    fn add_range(&mut self, start: usize, end: usize) {
        self.staged_ranges.insert(start, end);
        self.bytes_pending += end - start;
    }

    /// Forget the staged range that begins at `start` and stop counting its bytes as pending.
    fn remove_range(&mut self, start: usize) {
        if let Some(end) = self.staged_ranges.remove(&start) {
            self.bytes_pending -= end - start;
        }
    }

    /// The stream end is known and the cursor has reached it.
    fn is_done(&self) -> bool {
        self.stream_end_idx.is_some_and(|end| self.next_byte_idx >= end)
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

        let output = ra.output();
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