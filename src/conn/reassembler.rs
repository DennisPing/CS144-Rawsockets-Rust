use crate::conn::byte_stream::ByteStream;
use std::collections::BTreeMap;
use std::io;
use std::io::{Read, Write};

#[derive(Debug)]
pub struct Reassembler {
    segments: BTreeMap<usize, Vec<u8>>, // Out-of-order segments. key = start index
    output: ByteStream,                 // The assembled ByteStream, ready to be read
    next_byte_idx: usize,               // The next byte index expected to write
    last_byte_idx: Option<usize>,       // The last byte index, if known
    last_recvd: bool,                   // Has the last segment been received?
}

impl Reassembler {
    /// New `Reassembler` with the provided `ByteStream` as output
    pub fn new(output: ByteStream) -> Self {
        Reassembler {
            segments: BTreeMap::new(),
            output,
            next_byte_idx: 0,
            last_byte_idx: None,
            last_recvd: false,
        }
    }

    /// Insert a new byte segment into the `Reassembler`
    pub fn insert(&mut self, data: Vec<u8>, seq_num: usize, is_last: bool) -> io::Result<()> {
        if data.is_empty() && !is_last {
            return Ok(());
        }

        // If this is the last segment, set `last_recvd` flag and record `last_byte_idx`
        if is_last {
            self.last_recvd = true;
            self.last_byte_idx = Some(seq_num + data.len());
        }

        if self.is_done() {
            self.output.close();
            return Ok(());
        }

        // Buffer in the new segment
        self.insert_buffer(seq_num, data)?;

        // Write as much as possible to the output stream
        self.try_write()?;

        Ok(())
    }

    /// The total number of bytes pending reassembly in the buffer
    pub fn bytes_pending(&self) -> usize {
        self.segments.values().map(|segment| segment.len()).sum()
    }

    /// Get the underlying `ByteStream` output
    pub fn get_output(&mut self) -> &ByteStream {
        &self.output
    }

    /// Get the index of the next byte
    pub fn next_byte_idx(&self) -> usize {
        self.next_byte_idx
    }

    /// Insert data into the buffer; merging overlapping segments
    fn insert_buffer(&mut self, seq_num: usize, data: Vec<u8>) -> io::Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        let last_idx = seq_num + data.len();

        // Ignore the segment if it's entirely before the next expected byte
        if last_idx <= self.next_byte_idx {
            return Ok(());
        }

        // Calculate the effective range within buffer capacity
        let start = seq_num.max(self.next_byte_idx);
        let end = last_idx.min(self.next_byte_idx + self.output.remaining_capacity());

        if start >= end {
            return Ok(()); // No capacity to buffer
        }

        // Calculate the effective slice of data that fits within [start, end)
        let offset = start - seq_num;
        let window = &data[offset..(end - seq_num)];
        let mut merged = window.to_vec();
        let mut m_start = start;
        let mut m_end = end;

        let overlapping_segments = self.find_overlapping_segments(m_start, m_end);

        // Merge all overlapping segments with the new segment
        for (seg_start, seg_data) in overlapping_segments {
            self.segments.remove(&seg_start);

            let seg_end = seg_start + seg_data.len();

            if seg_end <= m_end {
                // Fully overlapping within [m_start, m_end)
                m_start = m_start.min(seg_start);
                m_end = m_end.max(seg_end);

                let insert_idx = seg_start - m_start;
                let req_len = m_end - m_start;

                // Resize merged data if necessary
                if merged.len() < req_len {
                    merged.resize(req_len, 0);
                }

                // Overlay the existing segment data onto merged data
                merged[insert_idx..(insert_idx + seg_data.len())].copy_from_slice(&seg_data);
            } else {
                // Partial overlap: seg_end > m_end
                m_start = m_start.min(seg_start);

                let overlap_len = m_end - seg_start;
                let insert_idx = seg_start - m_start;
                let req_len = m_end - m_start;

                // Resize merged data if necessary
                if merged.len() < req_len {
                    merged.resize(req_len, 0);
                }

                // Overlay only the overlapping part onto merged_data
                merged[insert_idx..(insert_idx + overlap_len)]
                    .copy_from_slice(&seg_data[..overlap_len]);

                // Preserve the non-overlapping part
                let rem_start = m_end;
                let rem_data = seg_data[overlap_len..].to_vec();
                self.segments.insert(rem_start, rem_data);
            }
        }

        // Overlay the new incoming data into merged data
        let new_idx = start - m_start;
        merged[new_idx..(new_idx + window.len())].copy_from_slice(window);

        // Insert merged segment back into the BTreeMap
        self.segments.insert(m_start, merged);

        Ok(())
    }

    fn find_overlapping_segments(&self, start: usize, end: usize) -> Vec<(usize, Vec<u8>)> {
        // Thanks OpenAI :)
        self.segments
            .range(..end)
            .filter_map(|(&seg_start, seg_data)| {
                let seg_end = seg_start + seg_data.len();
                if seg_end > start {
                    Some((seg_start, seg_data.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Attempt to write contiguous data from the buffer to the output `ByteStream`
    fn try_write(&mut self) -> io::Result<()> {
        while let Some(mut data) = self.segments.remove(&self.next_byte_idx) {
            let n = self.output.write(&data)?;

            if n == 0 {
                // If unable to write to ByteStream, then re-insert the segment and break
                self.segments.insert(self.next_byte_idx, data);
                break;
            }

            if n < data.len() {
                // Partial write occurred; store the remaining data
                let rem_data = data.split_off(n);
                self.segments.insert(self.next_byte_idx + n, rem_data);

                // Update `next_byte_idx` to the new position
                self.next_byte_idx += n;
                break;
            } else {
                // Full write occurred
                self.next_byte_idx += n;
            }

            if self.is_done() {
                self.output.close();
                break;
            }
        }

        Ok(())
    }

    /// Check if all the data has been received and written out
    fn is_done(&self) -> bool {
        if self.last_recvd {
            if let Some(last_idx) = self.last_byte_idx {
                if self.next_byte_idx >= last_idx {
                    return true;
                }
            }
        }
        false
    }
}

impl Read for Reassembler {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.output.read(buf)
    }
}

#[cfg(test)]
mod tests {
    use crate::conn::{ByteStream, Reassembler};
    use rand::seq::SliceRandom;
    use rand::{Rng, RngCore};
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
        ra.insert(vec![], 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        assert!(!ra.output.eof())
    }

    #[test]
    fn test_insert_within_capacity() {
        let mut ra = create_reassembler(5);

        // Insert first
        ra.insert(b"Hello".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 5);
        assert_eq!(ra.next_byte_idx(), 5);
        assert_eq!(ra.bytes_pending(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("Hello", actual);

        // Insert second
        ra.insert(b"World".to_vec(), 5, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 10);
        assert_eq!(ra.next_byte_idx(), 10);
        assert_eq!(ra.bytes_pending(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("World", actual);

        // Insert third
        ra.insert(b"Honda".to_vec(), 10, true).unwrap();
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
        ra.insert(b"Hello".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 5);
        assert_eq!(ra.bytes_pending(), 0);

        // Insert second; no-op because capacity exceeded
        ra.insert(b"World".to_vec(), 5, true).unwrap();
        assert_eq!(ra.output.bytes_written(), 5);
        assert_eq!(ra.bytes_pending(), 0);

        // Read out all data
        let actual = read_all_as_string(&mut ra);
        assert_eq!("Hello", actual);

        // Insert third; success
        ra.insert(b"World".to_vec(), 5, true).unwrap();
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
        ra.insert(b"ab".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 1);
        assert_eq!(ra.bytes_pending(), 0);

        // Insert second; no-op because capacity exceeded
        ra.insert(b"ab".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 1);
        assert_eq!(ra.bytes_pending(), 0);

        // Read out all data
        let actual = read_all_as_string(&mut ra);
        assert_eq!(ra.output.bytes_read(), 1);
        assert_eq!("a", actual);

        // Insert third
        ra.insert(b"abc".to_vec(), 0, false).unwrap();
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

        ra.insert(b"b".to_vec(), 1, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 1);

        ra.insert(b"bX".to_vec(), 2, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 1);

        ra.insert(b"a".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 2);
        assert_eq!(ra.bytes_pending(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("ab", actual);

        ra.insert(b"bc".to_vec(), 1, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 3);
        assert_eq!(ra.bytes_pending(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("c", actual);
    }

    #[test]
    fn test_insert_last_segment_beyond_capacity() {
        let mut ra = create_reassembler(2);

        ra.insert(b"bc".to_vec(), 1, true).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 1);

        ra.insert(b"a".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 2);
        assert_eq!(ra.bytes_pending(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("ab", actual);

        ra.insert(b"bc".to_vec(), 1, true).unwrap();
        assert_eq!(ra.output.bytes_written(), 3);
        assert_eq!(ra.bytes_pending(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("c", actual);

        assert!(ra.output.eof());
    }

    #[test]
    fn test_insert_junk_after_close() {
        let mut ra = create_reassembler(32);

        ra.insert(b"abcd".to_vec(), 0, false).unwrap();
        ra.insert(b"efgh".to_vec(), 4, true).unwrap();
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcdefgh", actual);
        assert!(ra.output.eof());

        // Verify code doesn't blow up
        let result = ra.insert(b"zzz".to_vec(), 8, false);
        assert!(result.is_ok());

        // Verify nothing gets read
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);
    }

    // -- Test sequential --

    #[test]
    fn test_sequential() {
        let mut ra = create_reassembler(32);

        ra.insert(b"abcd".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcd", actual);

        ra.insert(b"efgh".to_vec(), 4, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("efgh", actual);
    }

    #[test]
    fn test_sequential_combined() {
        let mut ra = create_reassembler(32);

        ra.insert(b"abcd".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);

        ra.insert(b"efgh".to_vec(), 4, false).unwrap();
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
            ra.insert(b"abcd".to_vec(), 4 * i, false).unwrap();
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
            ra.insert(b"abcd".to_vec(), 4 * i, false).unwrap();
            let actual = read_all_as_string(&mut ra);
            assert_eq!("abcd", actual);
        }
    }

    // -- Test duplicates --

    #[test]
    fn test_dup_at_same_index() {
        let mut ra = create_reassembler(32);

        // Insert new data
        ra.insert(b"abcd".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);

        // Read out data
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcd", actual);

        // Insert duplicate data at same index
        ra.insert(b"abcd".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);

        // Read out data, should be empty string
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);
    }

    #[test]
    fn test_dup_at_multiple_indexes() {
        let mut ra = create_reassembler(32);

        // Insert new data
        ra.insert(b"abcd".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcd", actual);

        // Insert data at index 4
        ra.insert(b"abcd".to_vec(), 4, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcd", actual);

        // Insert duplicate data at index 0
        ra.insert(b"abcd".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);

        // Insert duplicate data at index 4
        ra.insert(b"abcd".to_vec(), 4, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);
    }

    #[test]
    fn test_dup_random_indexes() {
        let mut ra = create_reassembler(32);

        let data = b"abcdefgh";

        ra.insert(data.to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcdefgh", actual);

        // Perform 1000 random insertions
        let mut rng = rand::thread_rng();
        for _ in 0..1000 {
            let j = rng.gen_range(0..8);
            let k = rng.gen_range(j..8);

            let chunk = &data[j..k];
            ra.insert(chunk.to_vec(), j, false).unwrap();
            assert_eq!(ra.output.bytes_written(), 8);

            let actual = read_all_as_string(&mut ra);
            assert_eq!("", actual);
            assert!(!ra.output.eof());
        }
    }

    #[test]
    fn test_dup_overlapping_segments_beyond_existing_data() {
        let mut ra = create_reassembler(32);

        ra.insert(b"abcd".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcd", actual);

        // Insert overlapping data that goes beyond existing data
        ra.insert(b"abcdef".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 6);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("ef", actual);
    }

    // -- Test holes --

    #[test]
    fn test_insert_with_initial_gap() {
        let mut ra = create_reassembler(32);

        ra.insert(b"b".to_vec(), 1, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);
    }

    #[test]
    fn test_fill_initial_gap() {
        let mut ra = create_reassembler(32);

        ra.insert(b"b".to_vec(), 1, false).unwrap();
        ra.insert(b"a".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 2);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("ab", actual);
    }

    #[test]
    fn test_fill_gap_with_last() {
        let mut ra = create_reassembler(32);

        ra.insert(b"b".to_vec(), 1, true).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);

        ra.insert(b"a".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 2);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("ab", actual);
        assert!(ra.output.eof());
    }

    #[test]
    fn test_fill_gap_with_overlapping_data() {
        let mut ra = create_reassembler(32);

        ra.insert(b"b".to_vec(), 1, false).unwrap();
        ra.insert(b"ab".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 2);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("ab", actual);
    }

    #[test]
    fn test_fill_multiple_gaps_with_chunks() {
        let mut ra = create_reassembler(32);

        ra.insert(b"b".to_vec(), 1, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);

        ra.insert(b"d".to_vec(), 3, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);

        ra.insert(b"abc".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcd", actual);

        // Insert empty data for last segment
        ra.insert(b"".to_vec(), 4, true).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);
    }

    // -- Test overlapping segments --

    #[test]
    fn test_overlap_extend() {
        let mut ra = create_reassembler(32);

        ra.insert(b"Hello".to_vec(), 0, false).unwrap();
        ra.insert(b"HelloWorld".to_vec(), 0, false).unwrap();

        assert_eq!(ra.output.bytes_written(), 10);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("HelloWorld", actual);
    }

    #[test]
    fn test_overlap_extend_after_read() {
        let mut ra = create_reassembler(32);

        ra.insert(b"Hello".to_vec(), 0, false).unwrap();
        let actual = read_all_as_string(&mut ra);
        assert_eq!("Hello", actual);

        ra.insert(b"HelloWorld".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 10);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("World", actual);
    }

    #[test]
    fn test_overlap_fill_gap() {
        let mut ra = create_reassembler(32);

        ra.insert(b"World".to_vec(), 5, false).unwrap();
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);

        ra.insert(b"Hello".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 10);
        let actual = read_all_as_string(&mut ra);
        assert_eq!("HelloWorld", actual);
    }

    #[test]
    fn test_overlap_partial() {
        let mut ra = create_reassembler(32);

        ra.insert(b"World".to_vec(), 5, false).unwrap();
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);

        ra.insert(b"Hello".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 10);

        ra.insert(b"ldHondaCivic".to_vec(), 8, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 20);

        let actual = read_all_as_string(&mut ra);
        assert_eq!("HelloWorldHondaCivic", actual);
    }

    #[test]
    fn test_overlap_between_two_pending() {
        let mut ra = create_reassembler(32);

        ra.insert(b"bc".to_vec(), 1, false).unwrap();
        ra.insert(b"ef".to_vec(), 4, false).unwrap();
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 4);

        ra.insert(b"cde".to_vec(), 2, false).unwrap();
        let actual = read_all_as_string(&mut ra);
        assert_eq!("", actual);
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 5);

        // _bc_ef
        // __cde_ (overlap in the middle between two pending)

        ra.insert(b"a".to_vec(), 0, false).unwrap();
        let actual = read_all_as_string(&mut ra);
        assert_eq!("abcdef", actual);
        assert_eq!(ra.output.bytes_written(), 6);
        assert_eq!(ra.bytes_pending(), 0);
    }

    #[test]
    fn test_overlap_many_pending() {
        let mut ra = create_reassembler(32);

        ra.insert(b"efgh".to_vec(), 4, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 4);

        ra.insert(b"op".to_vec(), 14, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 6);

        ra.insert(b"s".to_vec(), 18, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 0);
        assert_eq!(ra.bytes_pending(), 7);

        ra.insert(b"a".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 1);
        assert_eq!(ra.bytes_pending(), 7);

        ra.insert(b"abcde".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        assert_eq!(ra.bytes_pending(), 3);

        ra.insert(b"opqrst".to_vec(), 14, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        assert_eq!(ra.bytes_pending(), 6);

        ra.insert(b"op".to_vec(), 14, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);
        assert_eq!(ra.bytes_pending(), 6);

        ra.insert(b"ijklmn".to_vec(), 8, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 20);
        assert_eq!(ra.bytes_pending(), 0);
    }

    #[test]
    fn test_random_shuffle() {
        let n_reps = 32;
        let n_segs = 128;
        let max_seg_len = 2048;
        let max_offset_shift = 1023; // Maximum shift to introduce overlaps

        let mut rng = rand::thread_rng();
        for _ in 0..n_reps {
            let capacity = n_segs * max_seg_len;
            let mut ra = create_reassembler(capacity);

            let mut segments: Vec<(usize, usize)> = Vec::with_capacity(n_segs);
            let mut total_len = 0;

            // Generate segments with possible overlaps
            for _ in 0..n_segs {
                let seg_len = 1 + rng.gen_range(0..max_seg_len - 1);
                let shift = total_len.min(1 + rng.gen_range(0..max_offset_shift));
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
                ra.insert(slice.to_vec(), start, is_last)
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
