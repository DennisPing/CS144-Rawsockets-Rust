use criterion::{
    criterion_group, criterion_main, BenchmarkId, Criterion, SamplingMode, Throughput,
};
use net::common::byte_stream::ByteStream;
use net::common::reassembler::Reassembler;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::hint::black_box;
use std::io::{ErrorKind, Read};
use std::time::Duration;

struct BenchConfig {
    name: &'static str,
    dataset_size: usize,
    capacity: usize,
    seed: u64,
}

#[derive(Clone, Debug)]
struct Segment {
    start_idx: usize,
    data: Vec<u8>,
    is_last: bool,
}

/// Generate one big random buffer. Each segment will copy its own slice from here.
fn gen_dataset(total_bytes: usize, seed: u64) -> Vec<u8> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut dataset = vec![0u8; total_bytes];
    rng.fill_bytes(&mut dataset);
    dataset
}

/// Build staggered overlapping segments.
fn build_segments(data: &[u8], capacity: usize) -> Vec<Segment> {
    let mut segments: Vec<Segment> = Vec::new();

    let mut i = 0;
    while i < data.len() {
        // Small offsets (+2, +0, +1) to induce overlap
        for &offset in &[2usize, 0, 1] {
            let start = i.saturating_add(offset);
            if start >= data.len() {
                continue;
            }
            let end = (start + capacity * 2).min(data.len());
            let is_last = end == data.len();
            if start < end {
                segments.push(Segment {
                    start_idx: start,
                    data: data[start..end].to_vec(),
                    is_last,
                });
            }
        }
        i = i.saturating_add(capacity);
    }

    segments
}

/// The hot path under test. Create a new reassembler, insert all segments, and read out the data.
fn hot_run(segments: &[Segment], capacity: usize, total_len: usize) {
    let mut ra = Reassembler::new(ByteStream::new(capacity));
    let mut out = Vec::with_capacity(total_len);
    let mut buf = [0u8; 8192]; // Reusable buffer

    for seg in segments {
        // Insert segment into reassembler
        ra.insert(seg.start_idx, &seg.data, seg.is_last).unwrap();

        // Read out any available bytes into the out buffer
        loop {
            match ra.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => out.extend_from_slice(&buf[..n]),
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => panic!("read error: {e:?}"),
            }
        }
    }

    // Final drain in case the last insert unblocked a tail region
    loop {
        match ra.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(e) => panic!("final read error: {e:?}"),
        }
    }

    // Debug sanity check
    debug_assert!(
        ra.get_output().eof(),
        "ByteStream should be EOF after finishing"
    );
    debug_assert_eq!(out.len(), total_len, "Data length mismatch");
}

/// Validate the correctness of reassembler once, so we don't pollute the timing with repetitive checks.
fn validate_once(segments: &[Segment], capacity: usize, dataset: &[u8]) {
    let mut ra = Reassembler::new(ByteStream::new(capacity));
    let mut out = Vec::with_capacity(dataset.len());
    let mut buf = [0u8; 8192];

    for seg in segments {
        ra.insert(seg.start_idx, &seg.data, seg.is_last).unwrap();

        loop {
            match ra.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => out.extend_from_slice(&buf[..n]),
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => panic!("read error: {e:?}"),
            }
        }
    }

    loop {
        match ra.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(e) => panic!("final read error: {e:?}"),
        }
    }

    assert!(
        ra.get_output().eof(),
        "ByteStream should be EOF after finishing"
    );
    assert_eq!(out.len(), dataset.len(), "Data length mismatch");
    assert_eq!(out, dataset, "Data content mismatch");
}

/// Criterion bench test
fn bench_reassembler(c: &mut Criterion) {
    let mut group = c.benchmark_group("reassembler");

    group.sample_size(25);
    group.measurement_time(Duration::from_secs(10));
    group.sampling_mode(SamplingMode::Auto);

    let dataset_sizes = [
        8 * 1024 * 1024,  // 8 MB
        32 * 1024 * 1024, // 32 MB
    ];

    let capacities = [1500, 4096, 8192];

    for &dataset_size in &dataset_sizes {
        for &capacity in &capacities {
            let config = BenchConfig {
                name: "Staggered overlap",
                dataset_size,
                capacity,
                seed: 1370,
            };

            // Generate the input data
            let dataset = gen_dataset(config.dataset_size, config.seed);
            let segments = build_segments(&dataset, config.capacity);

            validate_once(&segments, config.capacity, &dataset);

            // Friendly ID with a parameterized name and measurement value
            let id = BenchmarkId::new(
                format!(
                    "{} bench with {} Bytes capacity",
                    config.name, config.capacity
                ),
                format!("{} MB data", config.dataset_size / (1024 * 1024)),
            );

            group.throughput(Throughput::Bytes(config.dataset_size as u64));

            // Register the benchmark case with the ID, bencher handle, and bench config
            group.bench_with_input(id, &config, |bencher, config| {
                bencher.iter(|| {
                    hot_run(
                        black_box(&segments),
                        config.capacity,
                        dataset.len()
                    );
                })
            });
        }
    }

    group.finish()
}

criterion_group!(benches, bench_reassembler);
criterion_main!(benches);
