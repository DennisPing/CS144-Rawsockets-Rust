use crate::conn::byte_stream::ByteStream;
use std::collections::BTreeMap;
use std::io;
use std::io::{Read, Write};

#[derive(Debug)]
pub struct Reassembler {
    segments: BTreeMap<usize, Vec<u8>>, // Out-of-order segments. key = start index
    output: ByteStream,                 // The assembled ByteStream, ready to be read
    next_byte_idx: usize,               // Next byte index expected to write
    last_byte_idx: Option<usize>,       // Index of the last byte, if known
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
    pub fn insert(
        &mut self,
        data: Vec<u8>,
        first_index: u64,
        is_last_segment: bool,
    ) -> io::Result<()> {
        // Common sense assertion
        assert!(
            first_index < usize::MAX as u64,
            "first_index exceeds usize::MAX"
        );
        let first_idx = first_index as usize; // Cast to `usize` for convenience

        if data.is_empty() && !is_last_segment {
            return Ok(());
        }

        // If this is the last segment, set `last_recvd` flag
        if is_last_segment {
            self.last_recvd = true;
            self.last_byte_idx = Some(first_idx + data.len());
        }

        // Buffer in the new segment
        self.insert_buffer(first_idx, data)?;

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
    pub fn next_byte_idx(&self) -> u64 {
        self.next_byte_idx as u64
    }

    /// Insert data into the buffer, merging overlapping segments
    fn insert_buffer(&mut self, first_index: usize, data: Vec<u8>) -> io::Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        // If new segment is entirely before the next expected byte, just ignore
        if first_index + data.len() <= self.next_byte_idx {
            return Ok(());
        }

        // Calculate the effective start and end idx within buffer capacity
        let start = first_index.max(self.next_byte_idx);
        let end =
            (first_index + data.len()).min(self.next_byte_idx + self.output.remaining_capacity());

        let window_len = end.saturating_sub(start);

        if window_len == 0 {
            return Ok(()); // No capacity left to buffer any part of the new data
        }

        // Calculate the offset within the data vec
        let data_offset = start - first_index;

        // Extract the relevant portion of data to buffer
        let logical_data = data
            .into_iter()
            .skip(data_offset)
            .take(window_len)
            .collect::<Vec<u8>>();

        // Create merged segment boundaries
        let mut merged_start = start;
        let mut merged_end = end;
        let mut merged_data = logical_data;

        // The range to search in for overlapping segments
        let search_start = if merged_start == 0 {
            0
        } else {
            merged_start - 1
        };
        let search_end = merged_end;

        let overlapping_keys: Vec<usize> = self
            .segments
            .range(search_start..=search_end)
            .map(|(&seg_start, _)| seg_start)
            .collect();

        // Merge all overlapping keys
        for &seg_start in &overlapping_keys {
            if let Some(seg_data) = self.segments.remove(&seg_start) {
                let seg_end = seg_start + seg_data.len();

                // Update the merged boundaries
                merged_start = merged_start.min(seg_start);
                merged_end = merged_end.max(seg_end);

                // Calculate insertion index relative to the new merged start
                let insert_idx = seg_start - merged_start;

                // Resize merged data if necessary
                let new_len = merged_end - merged_start;
                if merged_data.len() < new_len {
                    merged_data.resize(new_len, 0);
                }

                // Overlay the existing segment data onto the merged data
                merged_data[insert_idx..(insert_idx + seg_data.len())].copy_from_slice(&seg_data);
            }
        }

        // Insert merged segment back into the buffer
        self.segments.insert(merged_start, merged_data);

        Ok(())
    }

    /// Attempt to write contiguous data from the buffer to the output `ByteStream`
    fn try_write(&mut self) -> io::Result<()> {
        loop {
            // Attempt to retrieve the segment starting at `next_byte`
            if let Some(data) = self.segments.get(&self.next_byte_idx) {
                // Attempt to write the entire segment to the output
                let bytes_written = self.output.write(data)?;

                if bytes_written == 0 {
                    break;
                }

                // Calculate the new position
                let new_next_byte_idx = self.next_byte_idx + bytes_written;

                if bytes_written < data.len() {
                    // Partial write occurred; store the remaining data
                    let remaining_data = data[bytes_written..].to_vec();
                    self.segments.insert(new_next_byte_idx, remaining_data);

                    // Remove the old segment because it's been partially written
                    self.segments.remove(&self.next_byte_idx);

                    // Update `next_byte_idx` and be done
                    self.next_byte_idx = new_next_byte_idx;
                    break;
                } else {
                    // Entire segment was written; remove it from the buffer
                    self.segments.remove(&self.next_byte_idx);
                    self.next_byte_idx = new_next_byte_idx;
                }

                // Check if all bytes have been written and close the stream if necessary
                if self.last_recvd {
                    if let Some(last_idx) = self.last_byte_idx {
                        if self.next_byte_idx >= last_idx {
                            self.output.close();
                            break;
                        }
                    }
                }
            } else {
                // No contiguous segment found; exit the loop.
                break;
            }
        }

        Ok(())
    }
}

impl Read for Reassembler {
    /// Read data from the assembled `ByteStream` into the buffer
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.output.read(buf)
    }
}

#[cfg(test)]
mod tests {
    use crate::conn::{ByteStream, Reassembler};
    use rand::Rng;
    use std::io::Read;

    fn create_reassembler(capacity: usize) -> Reassembler {
        let stream = ByteStream::new(capacity);
        Reassembler::new(stream)
    }

    fn read_all(reassembler: &mut Reassembler) -> Vec<u8> {
        let mut buf = vec![];
        reassembler.read_to_end(&mut buf).unwrap();
        buf
    }

    // -- Test capacity --

    #[test]
    fn test_insert_within_capacity() {
        let mut ra = create_reassembler(5);

        // Insert first
        ra.insert(b"Hello".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 5);
        assert_eq!(ra.bytes_pending(), 0);

        // Read out all data
        let data = read_all(&mut ra);
        let mut actual = std::str::from_utf8(&data).unwrap();
        assert_eq!("Hello", actual);

        // Insert second
        ra.insert(b"World".to_vec(), 5, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 10);
        assert_eq!(ra.bytes_pending(), 0);

        // Read out all data
        let data = read_all(&mut ra);
        actual = std::str::from_utf8(&data).unwrap();
        assert_eq!("World", actual);

        // Insert third
        ra.insert(b"Honda".to_vec(), 10, true).unwrap();
        assert_eq!(ra.output.bytes_written(), 15);
        assert_eq!(ra.bytes_pending(), 0);

        // Read out all data
        let data = read_all(&mut ra);
        actual = std::str::from_utf8(&data).unwrap();
        assert_eq!("Honda", actual);

        assert!(ra.output.eof());
    }

    #[test]
    fn test_insert_beyond_capacity() {
        let mut ra = create_reassembler(5);

        // Insert first
        ra.insert(b"Hello".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 5);
        assert_eq!(ra.bytes_pending(), 0);

        // Insert second, no-op because capacity exceeded
        ra.insert(b"World".to_vec(), 5, true).unwrap();
        assert_eq!(ra.output.bytes_written(), 5);
        assert_eq!(ra.bytes_pending(), 0);

        // Read out all data
        let data = read_all(&mut ra);
        let mut actual = std::str::from_utf8(&data).unwrap();
        assert_eq!("Hello", actual);

        // Insert third
        ra.insert(b"World".to_vec(), 5, true).unwrap();
        assert_eq!(ra.output.bytes_written(), 10);
        assert_eq!(ra.bytes_pending(), 0);

        // Read out all data
        let data = read_all(&mut ra);
        actual = std::str::from_utf8(&data).unwrap();
        assert_eq!("World", actual);

        assert!(ra.output.eof());
    }

    #[test]
    fn test_overlapping_inserts() {
        let mut ra = create_reassembler(1);

        // Insert first
        ra.insert(b"ab".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 1);
        assert_eq!(ra.bytes_pending(), 0);

        // Insert second, no-op because capacity exceeded
        ra.insert(b"ab".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 1);
        assert_eq!(ra.bytes_pending(), 0);

        // Read out all data
        let data = read_all(&mut ra);
        let mut actual = std::str::from_utf8(&data).unwrap();
        assert_eq!(ra.output.bytes_read(), 1);
        assert_eq!("a", actual);

        // Insert third
        ra.insert(b"abc".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 2);
        assert_eq!(ra.bytes_pending(), 0);

        // Read out all data
        let data = read_all(&mut ra);
        actual = std::str::from_utf8(&data).unwrap();
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
        let data = read_all(&mut ra);
        let actual = std::str::from_utf8(&data).unwrap();
        assert_eq!("ab", actual);

        ra.insert(b"bc".to_vec(), 1, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 3);
        assert_eq!(ra.bytes_pending(), 0);
        let data = read_all(&mut ra);
        let actual = std::str::from_utf8(&data).unwrap();
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
        let data = read_all(&mut ra);
        let actual = std::str::from_utf8(&data).unwrap();
        assert_eq!("ab", actual);

        assert!(!ra.output.eof());

        ra.insert(b"bc".to_vec(), 1, true).unwrap();
        assert_eq!(ra.output.bytes_written(), 3);
        assert_eq!(ra.bytes_pending(), 0);
        let data = read_all(&mut ra);
        let actual = std::str::from_utf8(&data).unwrap();
        assert_eq!("c", actual);

        assert!(ra.output.eof());
    }

    // -- Test duplicates --

    #[test]
    fn test_dup_at_same_index() {
        let mut ra = create_reassembler(64);

        // Insert new data
        ra.insert(b"abcd".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);

        // Read out data
        let data = read_all(&mut ra);
        let actual = std::str::from_utf8(&data).unwrap();
        assert_eq!("abcd", actual);
        assert!(!ra.output.eof());

        // Insert duplicate data at same index
        ra.insert(b"abcd".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);

        // Read out data, should be empty string
        let data = read_all(&mut ra);
        let actual = std::str::from_utf8(&data).unwrap();
        assert_eq!("", actual);
        assert!(!ra.output.eof());
    }

    #[test]
    fn test_dup_at_multiple_indexes() {
        let mut ra = create_reassembler(64);

        // Insert new data
        ra.insert(b"abcd".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 4);

        // Read out data
        let data = read_all(&mut ra);
        let actual = std::str::from_utf8(&data).unwrap();
        assert_eq!("abcd", actual);
        assert!(!ra.output.eof());

        // Insert data at index 4
        ra.insert(b"abcd".to_vec(), 4, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);

        // Read out data
        let data = read_all(&mut ra);
        let actual = std::str::from_utf8(&data).unwrap();
        assert_eq!("abcd", actual);
        assert!(!ra.output.eof());

        // Insert duplicate data at index 0
        ra.insert(b"abcd".to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);

        // Read out data
        let data = read_all(&mut ra);
        let actual = std::str::from_utf8(&data).unwrap();
        assert_eq!("", actual);
        assert!(!ra.output.eof());
    }

    #[test]
    fn test_dup_random_indexes() {
        let mut ra = create_reassembler(64);

        let data = b"abcdefgh";

        ra.insert(data.to_vec(), 0, false).unwrap();
        assert_eq!(ra.output.bytes_written(), 8);

        // Read out data
        let data = read_all(&mut ra);
        let mut actual = std::str::from_utf8(&data).unwrap();
        assert_eq!("abcdefgh", actual);
        assert!(!ra.output.eof());

        // Perform 1000 random insertions
        let mut rng = rand::thread_rng();
        for i in 0..1000 {
            let j = rng.gen_range(0..8);
            let k = rng.gen_range(j..8);

            let chunk = &data[j..k];
            ra.insert(chunk.to_vec(), j as u64, false).unwrap();
            assert_eq!(ra.output.bytes_written(), 8);

            let data = read_all(&mut ra);
            actual = std::str::from_utf8(&data).unwrap();
            assert_eq!("", actual);
            assert!(!ra.output.eof());
        }
    }
}
