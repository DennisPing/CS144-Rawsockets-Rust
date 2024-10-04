use rand::prelude::StdRng;
use rand::{RngCore, SeedableRng};
use rawhttpget::conn::ByteStream;
use std::collections::VecDeque;
use std::io;
use std::io::{Error, ErrorKind, Read, Write};
use std::time::Instant;

fn speed_test(
    input_len: usize,
    capacity: usize,
    random_seed: usize,
    write_size: usize,
    read_size: usize,
) -> io::Result<()> {
    // Generate random data
    let mut rng = StdRng::seed_from_u64(random_seed as u64);
    let mut data = vec![0u8; input_len];
    rng.fill_bytes(&mut data);

    // Split data into chunks
    let mut chunks = VecDeque::new();
    let mut i = 0;
    while i < data.len() {
        let end = usize::min(i + write_size, data.len());
        let chunk = data[i..end].to_vec();
        chunks.push_back(chunk);
        i = end;
    }

    // Set up ByteStream and output buffer
    let mut stream = ByteStream::new(capacity);
    let mut output_buffer = Vec::with_capacity(input_len);

    // Start timer
    let t0 = Instant::now();

    // Run simulation
    while !stream.eof() {
        if chunks.is_empty() {
            if !stream.is_closed() {
                stream.close();
            }
        } else if let Some(front) = chunks.front() {
            if front.len() <= stream.remaining_capacity() {
                let chunk = chunks.pop_front().unwrap();
                stream.write_all(&chunk)?;
            }
        }

        stream.read_to_end(&mut output_buffer)?;
    }

    // Stop timer
    let duration = t0.elapsed();

    // Validate data
    if data != output_buffer {
        return Err(Error::new(
            ErrorKind::Other,
            "Data written does not equal data read :(",
        ));
    }

    // Calculate throughput
    let duration_secs = duration.as_secs_f64();
    let bytes_per_sec = input_len as f64 / duration_secs;
    let bits_per_sec = bytes_per_sec * 8.0;
    let gigabits_per_sec = bits_per_sec / 1e9;

    println!(
        "ByteStream with capacity={capacity}, write_size={write_size}, \
        read_size={read_size} reached {gigabits_per_sec:.2 } Gbit/s",
    );

    Ok(())
}

fn main() {
    let input_len = 1e7 as usize; // 10 MB
    let capacity = 32768; // 32 KB
    let random_seed = 789;
    let write_size = 1500; // MTU 1500 bytes
    let read_size = 128;

    if let Err(e) = speed_test(input_len, capacity, random_seed, write_size, read_size) {
        eprintln!("Speed test failed: {e}");
        std::process::exit(1);
    };

    // Result:
    // ByteStream with capacity=32768, write_size=1500, read_size=128 reached 15.40 Gbit/s
}
