// use std::collections::BTreeMap;
// use std::io;
// use std::io::{Read, Write};
// use crate::common::byte_stream::ByteStream;
//
// #[derive(Debug)]
// pub struct Reassembler {
//     segments: BTreeMap<usize, Vec<u8>>, // Out-of-order segments. key = start index
//     output: ByteStream,                 // The assembled ByteStream, ready to be read
//     next_byte_idx: usize,               // The next contiguous byte offset expected to write
//     last_byte_idx: Option<usize>,       // The index after the end offset, if known
// }
//
// impl Reassembler {
//     /// New `Reassembler` with the provided `ByteStream` as output.
//     pub fn new(output: ByteStream) -> Self {
//         Reassembler {
//             segments: BTreeMap::new(),
//             output,
//             next_byte_idx: 0,
//             last_byte_idx: None,
//         }
//     }
//
//     /// Insert a new segment into the `Reassembler`. Optionally mark it as the last segment.
//     pub fn insert(&mut self, first_idx: usize, data: &[u8], is_last: bool) -> io::Result<()> {
//         if data.is_empty() && !is_last {
//             return Ok(());
//         }
//
//         // If this is the last segment, set `last_byte_idx`
//         if is_last {
//             let end = first_idx + data.len();
//             self.last_byte_idx = Some(self.last_byte_idx.map_or(end, |old| old.max(end)));
//         }
//
//         if self.is_done() {
//             self.output.close();
//             return Ok(());
//         }
//
//         // Buffer in the new segment and merge overlaps
//         self.insert_buffer(first_idx, data)?;
//
//         // Write as much as possible to the output stream
//         self.write_output()?;
//
//         Ok(())
//     }
//
//     /// The total number of bytes pending reassembly in the buffer.
//     pub fn bytes_pending(&self) -> usize {
//         self.segments.values().map(|segment| segment.len()).sum()
//     }
//
//     /// Get the underlying assembled `ByteStream` output.
//     pub fn get_output(&mut self) -> &ByteStream {
//         &self.output
//     }
//
//     /// Get the next contiguous byte offset.
//     pub fn next_byte_idx(&self) -> usize {
//         self.next_byte_idx
//     }
//
//     /// Insert data into the buffer and merging any overlapping segments.
//     fn insert_buffer(&mut self, first_idx: usize, data: &[u8]) -> io::Result<()> {
//         // 1) Clamp incoming segment to write cursor and available output capacity
//         let last_idx = first_idx + data.len();
//         if last_idx <= self.next_byte_idx {
//             return Ok(()); // Incoming segment's end is entirely before the write cursor
//         }
//
//         let start = first_idx.max(self.next_byte_idx);
//         let cap_end = self.next_byte_idx + self.output.remaining_capacity();
//         if start >= cap_end {
//             return Ok(()); // No capacity left
//         }
//
//         let end = last_idx.min(cap_end);
//         let window_offset = start - first_idx;
//         let window = &data[window_offset..window_offset + (end - start)];
//
//         // 2) Split map at `start`: left (< start) stays in map; right (>= start) splits off.
//         let mut right = self.segments.split_off(&start);
//
//         // Current merge range.
//         let mut merge_start = start;
//         let mut merge_end = end;
//
//         // Vector of neighbor pieces that will be layered into the merged result.
//         let mut pieces: Vec<(usize, Vec<u8>)> = Vec::new();
//
//         // 3) Check if left neighbor overlaps or touches the merge start.
//         if let Some((&l_start, l_seg)) = self.segments.last_key_value() {
//             let l_end = l_start + l_seg.len();
//             if l_end >= merge_start {
//                 let l_seg = self.segments.remove(&l_start).unwrap();
//                 merge_start = l_start.min(merge_start);
//                 merge_end = merge_end.max(l_end); // Expand merge window
//                 pieces.push((l_start, l_seg)); // Re-add left neighbor
//             }
//         }
//
//         // 4) Pull in right neighbors (nb) while they overlap or touch.
//         while let Some((&r_start, _)) = right.first_key_value() {
//             if r_start > merge_end {
//                 break; // No more overlaps
//             }
//             let (nb_start, nb_seg) = right.pop_first().unwrap();
//             let nb_end = nb_start + nb_seg.len();
//             merge_end = merge_end.max(nb_end);
//             pieces.push((nb_start, nb_seg));
//         }
//
//         // 5) Append the right-hand map (any non-overlapping segments) into our buffer.
//         self.segments.append(&mut right);
//
//         // 6) Build the merged segment. Overlay old pieces and then the new window.
//         let mut merged = vec![0u8; merge_end - merge_start];
//         for (start_idx, segment) in &pieces { // Overlay all left + right segments
//             let offset = *start_idx - merge_start;
//             merged[offset..offset + segment.len()].copy_from_slice(segment);
//         }
//         let new_offset = start - merge_start;
//         merged[new_offset..new_offset + window.len()].copy_from_slice(window);
//
//         // 7) Insert the merged result.
//         self.segments.insert(merge_start, merged);
//
//         // Time complexity:
//         // Worse case: O(k log n + k * m)
//         //      where k = number of overlapping segments and m = avg segment size
//         // Avg case: O(log n)
//         //      when most segments arrive in-order
//         Ok(())
//     }
//
//     /// Write contiguous data from the buffer to the output `ByteStream`.
//     fn write_output(&mut self) -> io::Result<()> {
//         while let Some(mut seg) = self.segments.remove(&self.next_byte_idx) {
//             let n = self.output.write(&seg)?;
//             if n == 0 {
//                 // Unable to write; reinsert and stop.
//                 self.segments.insert(self.next_byte_idx, seg);
//                 break;
//             }
//             if n < seg.len() {
//                 // Partial write: keep remainder by splitting without copying the prefix.
//                 let rem = seg.split_off(n);
//                 self.segments.insert(self.next_byte_idx + n, rem);
//                 self.next_byte_idx += n;
//                 break;
//             }
//             self.next_byte_idx += n;
//
//             if self.is_done() {
//                 self.output.close();
//                 break;
//             }
//         }
//         Ok(())
//     }
//
//     /// Check if all the data has been received and written out.
//     fn is_done(&self) -> bool {
//         if let Some(last_idx) = self.last_byte_idx {
//             self.next_byte_idx >= last_idx
//         } else {
//             false
//         }
//     }
// }
//
// impl Read for Reassembler {
//     fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
//         self.output.read(buf)
//     }
// }
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use rand::seq::SliceRandom;
//     use rand::{Rng, RngCore};
//     use std::io::Read;
//
//     fn create_reassembler(capacity: usize) -> Reassembler {
//         let stream = ByteStream::new(capacity);
//         Reassembler::new(stream)
//     }
//
//     fn read_all_as_string(reassembler: &mut Reassembler) -> String {
//         let mut buf = vec![];
//         reassembler.read_to_end(&mut buf).unwrap();
//         std::str::from_utf8(&buf).unwrap().to_owned()
//     }
//
//     // -- Test insert and capacity --
//
//     #[test]
//     fn test_insert_empty_data() {
//         let mut ra = create_reassembler(32);
//         ra.insert(0, &[], false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 0);
//         assert!(!ra.output.eof());
//     }
//
//     #[test]
//     fn test_insert_within_capacity() {
//         let mut ra = create_reassembler(5);
//
//         // Insert first
//         ra.insert(0, b"Hello", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 5);
//         assert_eq!(ra.next_byte_idx(), 5);
//         assert_eq!(ra.bytes_pending(), 0);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("Hello", actual);
//
//         // Insert second
//         ra.insert(5, b"World", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 10);
//         assert_eq!(ra.next_byte_idx(), 10);
//         assert_eq!(ra.bytes_pending(), 0);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("World", actual);
//
//         // Insert third
//         ra.insert(10, b"Honda", true).unwrap();
//         assert_eq!(ra.output.bytes_written(), 15);
//         assert_eq!(ra.next_byte_idx(), 15);
//         assert_eq!(ra.bytes_pending(), 0);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("Honda", actual);
//
//         let output = ra.get_output();
//         assert!(output.is_closed());
//         assert!(output.eof());
//     }
//
//     #[test]
//     fn test_insert_beyond_capacity() {
//         let mut ra = create_reassembler(5);
//
//         // Insert first
//         ra.insert(0, b"Hello", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 5);
//         assert_eq!(ra.bytes_pending(), 0);
//
//         // Insert second; no-op because capacity exceeded
//         ra.insert(5, b"World", true).unwrap();
//         assert_eq!(ra.output.bytes_written(), 5);
//         assert_eq!(ra.bytes_pending(), 0);
//
//         // Read out all data
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("Hello", actual);
//
//         // Insert third; success
//         ra.insert(5, b"World", true).unwrap();
//         assert_eq!(ra.output.bytes_written(), 10);
//         assert_eq!(ra.bytes_pending(), 0);
//
//         // Read out all data
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("World", actual);
//
//         assert!(ra.output.eof());
//     }
//
//     #[test]
//     fn test_capacity_overlapping_inserts() {
//         let mut ra = create_reassembler(1);
//
//         // Insert first
//         ra.insert(0, b"ab", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 1);
//         assert_eq!(ra.bytes_pending(), 0);
//
//         // Insert second; no-op because capacity exceeded
//         ra.insert(0, b"ab", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 1);
//         assert_eq!(ra.bytes_pending(), 0);
//
//         // Read out all data
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!(ra.output.bytes_read(), 1);
//         assert_eq!("a", actual);
//
//         // Insert third
//         ra.insert(0, b"abc", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 2);
//         assert_eq!(ra.bytes_pending(), 0);
//
//         // Read out all data
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!(ra.output.bytes_read(), 2);
//         assert_eq!("b", actual);
//     }
//
//     #[test]
//     fn test_insert_beyond_capacity_with_different_data() {
//         let mut ra = create_reassembler(2);
//
//         ra.insert(1, b"b", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 0);
//         assert_eq!(ra.bytes_pending(), 1);
//
//         ra.insert(2, b"bX", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 0);
//         assert_eq!(ra.bytes_pending(), 1);
//
//         ra.insert(0, b"a", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 2);
//         assert_eq!(ra.bytes_pending(), 0);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("ab", actual);
//
//         ra.insert(1, b"bc", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 3);
//         assert_eq!(ra.bytes_pending(), 0);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("c", actual);
//     }
//
//     #[test]
//     fn test_insert_last_segment_beyond_capacity() {
//         let mut ra = create_reassembler(2);
//
//         ra.insert(1, b"bc", true).unwrap();
//         assert_eq!(ra.output.bytes_written(), 0);
//         assert_eq!(ra.bytes_pending(), 1);
//
//         ra.insert(0, b"a", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 2);
//         assert_eq!(ra.bytes_pending(), 0);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("ab", actual);
//
//         ra.insert(1, b"bc", true).unwrap();
//         assert_eq!(ra.output.bytes_written(), 3);
//         assert_eq!(ra.bytes_pending(), 0);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("c", actual);
//
//         assert!(ra.output.eof());
//     }
//
//     #[test]
//     fn test_insert_junk_after_close() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(0, b"abcd", false).unwrap();
//         ra.insert(4, b"efgh", true).unwrap();
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("abcdefgh", actual);
//         assert!(ra.output.eof());
//
//         // Verify code doesn't blow up
//         let result = ra.insert(8, b"zzz", false);
//         assert!(result.is_ok());
//
//         // Verify nothing gets read
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("", actual);
//     }
//
//     // -- Test sequential --
//
//     #[test]
//     fn test_sequential() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(0, b"abcd", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 4);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("abcd", actual);
//
//         ra.insert(4, b"efgh", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 8);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("efgh", actual);
//     }
//
//     #[test]
//     fn test_sequential_combined() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(0, b"abcd", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 4);
//
//         ra.insert(4, b"efgh", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 8);
//
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("abcdefgh", actual);
//     }
//
//     #[test]
//     fn test_sequential_combined_loop() {
//         let mut ra = create_reassembler(4096);
//         let mut combined_data = String::new();
//
//         for i in 0..100 {
//             let total_writes = 4 * i;
//             assert_eq!(ra.output.bytes_written(), total_writes);
//             ra.insert(4 * i, b"abcd", false).unwrap();
//             combined_data.push_str("abcd");
//         }
//
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!(combined_data, actual);
//     }
//
//     #[test]
//     fn test_sequential_immediate_read_loop() {
//         let mut ra = create_reassembler(4096);
//
//         for i in 0..100 {
//             let total_writes = 4 * i;
//             assert_eq!(ra.output.bytes_written(), total_writes);
//             ra.insert(4 * i, b"abcd", false).unwrap();
//             let actual = read_all_as_string(&mut ra);
//             assert_eq!("abcd", actual);
//         }
//     }
//
//     // -- Test duplicates --
//
//     #[test]
//     fn test_dup_at_same_index() {
//         let mut ra = create_reassembler(32);
//
//         // Insert new data
//         ra.insert(0, b"abcd", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 4);
//
//         // Read out data
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("abcd", actual);
//
//         // Insert duplicate data at same index
//         ra.insert(0, b"abcd", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 4);
//
//         // Read out data, should be empty string
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("", actual);
//     }
//
//     #[test]
//     fn test_dup_at_multiple_indexes() {
//         let mut ra = create_reassembler(32);
//
//         // Insert new data
//         ra.insert(0, b"abcd", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 4);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("abcd", actual);
//
//         // Insert data at index 4
//         ra.insert(4, b"abcd", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 8);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("abcd", actual);
//
//         // Insert duplicate data at index 0
//         ra.insert(0, b"abcd", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 8);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("", actual);
//
//         // Insert duplicate data at index 4
//         ra.insert(4, b"abcd", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 8);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("", actual);
//     }
//
//     #[test]
//     fn test_dup_random_indexes() {
//         let mut ra = create_reassembler(32);
//
//         let data = b"abcdefgh";
//
//         ra.insert(0, data, false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 8);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("abcdefgh", actual);
//
//         // Perform 1000 random insertions
//         let mut rng = rand::rng();
//         for _ in 0..1000 {
//             let j = rng.random_range(0..8);
//             let k = rng.random_range(j..8);
//
//             let chunk = &data[j..k];
//             ra.insert(j, chunk, false).unwrap();
//             assert_eq!(ra.output.bytes_written(), 8);
//
//             let actual = read_all_as_string(&mut ra);
//             assert_eq!("", actual);
//             assert!(!ra.output.eof());
//         }
//     }
//
//     #[test]
//     fn test_dup_overlapping_segments_beyond_existing_data() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(0, b"abcd", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 4);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("abcd", actual);
//
//         // Insert overlapping data that goes beyond existing data
//         ra.insert(0, b"abcdef", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 6);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("ef", actual);
//     }
//
//     // -- Test holes --
//
//     #[test]
//     fn test_insert_with_initial_gap() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(1, b"b", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 0);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("", actual);
//     }
//
//     #[test]
//     fn test_fill_initial_gap() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(1, b"b", false).unwrap();
//         ra.insert(0, b"a", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 2);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("ab", actual);
//     }
//
//     #[test]
//     fn test_fill_gap_with_last() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(1, b"b", true).unwrap();
//         assert_eq!(ra.output.bytes_written(), 0);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("", actual);
//
//         ra.insert(0, b"a", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 2);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("ab", actual);
//         assert!(ra.output.eof());
//     }
//
//     #[test]
//     fn test_fill_gap_with_overlapping_data() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(1, b"b", false).unwrap();
//         ra.insert(0, b"ab", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 2);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("ab", actual);
//     }
//
//     #[test]
//     fn test_fill_multiple_gaps_with_chunks() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(1, b"b", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 0);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("", actual);
//
//         ra.insert(3, b"d", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 0);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("", actual);
//
//         ra.insert(0, b"abc", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 4);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("abcd", actual);
//
//         // Insert empty data for last segment
//         ra.insert(4, b"", true).unwrap();
//         assert_eq!(ra.output.bytes_written(), 4);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("", actual);
//     }
//
//     // -- Test overlapping segments --
//
//     #[test]
//     fn test_overlap_extend() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(0, b"Hello", false).unwrap();
//         ra.insert(0, b"HelloWorld", false).unwrap();
//
//         assert_eq!(ra.output.bytes_written(), 10);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("HelloWorld", actual);
//     }
//
//     #[test]
//     fn test_overlap_extend_after_read() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(0, b"Hello", false).unwrap();
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("Hello", actual);
//
//         ra.insert(0, b"HelloWorld", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 10);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("World", actual);
//     }
//
//     #[test]
//     fn test_overlap_fill_gap() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(5, b"World", false).unwrap();
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("", actual);
//
//         ra.insert(0, b"Hello", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 10);
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("HelloWorld", actual);
//     }
//
//     #[test]
//     fn test_overlap_partial() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(5, b"World", false).unwrap();
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("", actual);
//
//         ra.insert(0, b"Hello", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 10);
//
//         ra.insert(8, b"ldHondaCivic", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 20);
//
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("HelloWorldHondaCivic", actual);
//     }
//
//     #[test]
//     fn test_overlap_between_two_pending() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(1, b"bc", false).unwrap();
//         ra.insert(4, b"ef", false).unwrap();
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("", actual);
//         assert_eq!(ra.output.bytes_written(), 0);
//         assert_eq!(ra.bytes_pending(), 4);
//
//         ra.insert(2, b"cde", false).unwrap();
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("", actual);
//         assert_eq!(ra.output.bytes_written(), 0);
//         assert_eq!(ra.bytes_pending(), 5);
//
//         // _bc_ef
//         // __cde_ (overlap in the middle between two pending)
//
//         ra.insert(0, b"a", false).unwrap();
//         let actual = read_all_as_string(&mut ra);
//         assert_eq!("abcdef", actual);
//         assert_eq!(ra.output.bytes_written(), 6);
//         assert_eq!(ra.bytes_pending(), 0);
//     }
//
//     #[test]
//     fn test_overlap_many_pending() {
//         let mut ra = create_reassembler(32);
//
//         ra.insert(4, b"efgh", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 0);
//         assert_eq!(ra.bytes_pending(), 4);
//
//         ra.insert(14, b"op", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 0);
//         assert_eq!(ra.bytes_pending(), 6);
//
//         ra.insert(18, b"s", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 0);
//         assert_eq!(ra.bytes_pending(), 7);
//
//         ra.insert(0, b"a", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 1);
//         assert_eq!(ra.bytes_pending(), 7);
//
//         ra.insert(0, b"abcde", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 8);
//         assert_eq!(ra.bytes_pending(), 3);
//
//         ra.insert(14, b"opqrst", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 8);
//         assert_eq!(ra.bytes_pending(), 6);
//
//         ra.insert(14, b"op", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 8);
//         assert_eq!(ra.bytes_pending(), 6);
//
//         ra.insert(8, b"ijklmn", false).unwrap();
//         assert_eq!(ra.output.bytes_written(), 20);
//         assert_eq!(ra.bytes_pending(), 0);
//     }
//
//     #[test]
//     fn test_random_shuffle() {
//         let n_reps = 32;
//         let n_segs = 128;
//         let max_seg_len = 2048;
//         let max_offset_shift = 1023; // Maximum shift to introduce overlaps
//
//         let mut rng = rand::rng();
//         for _ in 0..n_reps {
//             let capacity = n_segs * max_seg_len;
//             let mut ra = create_reassembler(capacity);
//
//             let mut segments: Vec<(usize, usize)> = Vec::with_capacity(n_segs);
//             let mut total_len = 0;
//
//             // Generate segments with possible overlaps
//             for _ in 0..n_segs {
//                 let seg_len = 1 + rng.random_range(0..max_seg_len - 1);
//                 let shift = total_len.min(1 + rng.random_range(0..max_offset_shift));
//                 let start = total_len - shift;
//                 let seg_size = seg_len + shift;
//                 segments.push((start, seg_size));
//
//                 total_len += seg_len;
//             }
//
//             // Shuffle segments to simulate out of order receives
//             segments.shuffle(&mut rng);
//
//             // Generate random data
//             let mut payload = vec![0u8; total_len];
//             rng.fill_bytes(&mut payload);
//
//             // Insert each shuffled segment into the Reassembler
//             for (start, size) in segments {
//                 let slice = &payload[start..(start + size)];
//                 let is_last = start + size == total_len;
//                 ra.insert(start, slice, is_last)
//                     .expect("Insert into Reassembler failed");
//             }
//
//             // Read out all data
//             let mut buf = vec![];
//             ra.read_to_end(&mut buf).expect("Read to end failed");
//             assert_eq!(payload.len(), buf.len());
//             assert_eq!(payload, buf);
//         }
//     }
// }
