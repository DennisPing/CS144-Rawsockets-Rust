use net::tcp::byte_stream::ByteStream;
use net::tcp::reassembler::Reassembler;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use std::collections::VecDeque;
use std::io;
use std::io::{Error, ErrorKind, Read};
use std::time::Instant;

fn speed_test(num_chunks: usize, capacity: usize, random_seed: usize) -> io::Result<()> {
    // Generate random data
    let mut rng = StdRng::seed_from_u64(random_seed as u64);
    let mut data = vec![0u8; num_chunks * capacity];
    rng.fill_bytes(&mut data);

    // Split data up into segments
    let mut chunks: VecDeque<(usize, &[u8], bool)> = VecDeque::new();
    for i in (0..data.len()).step_by(capacity) {
        for offset in 0..3 {
            let start = i + offset;
            if start > data.len() {
                continue; // Skip if start exceeds data length
            }
            let end = usize::min(start + capacity * 2, data.len());
            let segment = data.get(start..end).unwrap_or(&[]);
            let is_last = end >= data.len();
            chunks.push_back((start, segment, is_last));
        }
    }

    // Set up Reassembler and output buffer
    let mut ra = Reassembler::new(ByteStream::new(capacity));
    let mut output_buffer = Vec::with_capacity(data.len());
    let mut buf = [0u8; 8192]; // Reusable buffer

    // Start timer
    let t0 = Instant::now();

    // Run simulation
    while let Some((seq_num, segment, is_last)) = chunks.pop_front() {
        ra.insert(seq_num, segment, is_last)?;

        loop {
            match ra.read(&mut buf) {
                Ok(0) => break, // Done
                Ok(n) => {
                    output_buffer.extend_from_slice(&buf[..n]);
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    break; // ByteStream not ready to be read
                }
                Err(e) => return Err(e),
            }
        }
    }

    let duration = t0.elapsed();

    if !ra.get_output().eof() {
        return Err(Error::new(
            ErrorKind::Other,
            "Reassembler did not close ByteStream when finished",
        ));
    }

    if data != output_buffer {
        return Err(Error::new(
            ErrorKind::Other,
            "Mismatch between data written and data read",
        ));
    }

    // Calculate throughput
    let duration_secs = duration.as_secs_f64();
    let bytes_per_sec = (num_chunks * capacity) as f64 / duration_secs;
    let bits_per_sec = bytes_per_sec * 8.0;
    let gigabits_per_sec = bits_per_sec / 1e9;

    println!(
        "Reassembler to ByteStream with capacity={capacity} reached {gigabits_per_sec:.2} Gbit/s"
    );

    Ok(())
}

fn main() {
    let num_chunks = 10_000;
    let capacity = 1500;
    let random_seed = 1370;

    if let Err(e) = speed_test(num_chunks, capacity, random_seed) {
        eprintln!("Speed test failed: {e}");
        std::process::exit(1);
    }

    // Result:
    // Reassembler to ByteStream with capacity=1500 reached 12.04 Gbit/s
}
