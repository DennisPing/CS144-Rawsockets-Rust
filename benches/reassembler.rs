use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use net::common::byte_stream::ByteStream;
use net::common::reassembler::Reassembler;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::hint::black_box;
use std::io::Read;

const CAPACITY: usize = 65_535; // Largest receive window a TCP header can advertise
const SEGMENT_SIZE: usize = 1_460; // Standard TCP maximum segment size on Ethernet
const DATASET_SIZE: usize = 8 * 1024 * 1024; // 8 MB
const SEED: u64 = 1370;

struct Segment<'a> {
    start_idx: usize,
    data: &'a [u8],
    is_last: bool,
}

/// Generate one big random buffer. Each segment borrows its own slice from here.
fn gen_dataset() -> Vec<u8> {
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut dataset = vec![0u8; DATASET_SIZE];
    rng.fill_bytes(&mut dataset);
    dataset
}

/// Cut the segment [start, start + len) out of the dataset, clamped to the end of the dataset.
fn segment_at(dataset: &[u8], start: usize, len: usize) -> Segment<'_> {
    let end = (start + len).min(dataset.len());
    Segment {
        start_idx: start,
        data: &dataset[start..end],
        is_last: end == dataset.len(),
    }
}

/// Baseline: every segment arrives exactly once and in order.
fn in_order(dataset: &[u8]) -> Vec<Segment<'_>> {
    let mut segments = Vec::new();
    for start in (0..dataset.len()).step_by(SEGMENT_SIZE) {
        segments.push(segment_at(dataset, start, SEGMENT_SIZE));
    }
    segments
}

/// Overlapping segments: two arrive early and overlap each other, one fills in, one is stale.
///
/// | Order | Offset | Range       | Test condition         |
/// |-------|--------|-------------|--------------------------------------------|
/// | 1     | S      | [S, 2S)     | Arrives early => stage                     |
/// | 2     | S/2    | [S/2, 3S/2) | Arrives early => stage & merge overlapping |
/// | 3     | 0      | [0, S)      | In order & overlap => trim and write out   |
/// | 4     | S/4    | [S/4, 5S/4) | Entirely stale => drop                     |
fn staggered_overlap(dataset: &[u8]) -> Vec<Segment<'_>> {
    // Second half first; then a segment overlapping it; then the first half; then a duplicate
    const OFFSETS: [usize; 4] = [
        SEGMENT_SIZE,
        SEGMENT_SIZE / 2,
        0,
        SEGMENT_SIZE / 4
    ];

    /*
    bytes:       0         S/2       S         3S/2      2S
                 ^         ^         ^         ^         ^
    #1 early:                        [-------------------)     staged
    #2 early:             [-------------------)                merged with #1
    #3 in order: [-------------------)                         written, staged rest flushed
    #4 stale:         [-------------------)                    dropped
     */

    let mut segments = Vec::new();
    for step in (0..dataset.len()).step_by(2 * SEGMENT_SIZE) {
        for offset in OFFSETS {
            let start = step + offset;
            if start < dataset.len() {
                segments.push(segment_at(dataset, start, SEGMENT_SIZE));
            }
        }
    }
    segments
}

/// Out-of-order segments in groups of 8, sent so that gaps open up and are then filled in.
fn gaps_then_fill(dataset: &[u8]) -> Vec<Segment<'_>> {
    // 1, 3, 5, 7 leave four separate ranges; 6, 4, 2 each join two ranges; 0 fills the last gap
    const SEND_ORDER: [usize; 8] = [1, 3, 5, 7, 6, 4, 2, 0];

    let mut segments = Vec::new();
    for group_start in (0..dataset.len()).step_by(SEND_ORDER.len() * SEGMENT_SIZE) {
        for position in SEND_ORDER {
            let start = group_start + position * SEGMENT_SIZE;
            if start < dataset.len() {
                segments.push(segment_at(dataset, start, SEGMENT_SIZE));
            }
        }
    }
    segments
}

/// The hot path under test. Create a new reassembler, insert all segments, and read out the data.
/// Returns the reassembled bytes if `keep_output` is true, otherwise an empty `Vec`.
fn run(segments: &[Segment], keep_output: bool) -> Vec<u8> {
    let mut ra = Reassembler::new(ByteStream::new(CAPACITY));
    let mut buf = [0u8; 8192]; // Reusable buffer
    let mut out = Vec::new();

    for seg in segments {
        ra.insert(seg.start_idx, seg.data, seg.is_last).unwrap();

        // Read out any available bytes
        loop {
            let n = ra.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            if keep_output {
                out.extend_from_slice(&buf[..n]);
            } else {
                black_box(&buf[..n]);
            }
        }
    }

    assert!(ra.output().eof(), "ByteStream should be EOF after finishing");
    out
}

/// Criterion bench test
fn bench_reassembler(c: &mut Criterion) {
    let dataset = gen_dataset();
    let workloads = [
        ("in_order", in_order(&dataset)),
        ("staggered_overlap", staggered_overlap(&dataset)),
        ("gaps_then_fill", gaps_then_fill(&dataset)),
    ];

    let mut group = c.benchmark_group("reassembler");
    group.throughput(Throughput::Bytes(DATASET_SIZE as u64));
    group.noise_threshold(0.05); // Add 5% noise threshold to prevent false alarms

    for (name, segments) in &workloads {
        // 1. Validate the correctness once. Compare the out Vec with the original dataset.
        let out = run(segments, true);
        assert_eq!(out, dataset, "{name}: data content mismatch");

        // The timed hot runs only look at the bytes read out; they do not collect them
        group.bench_function(*name, |bencher| {
            bencher.iter(|| run(black_box(segments), false))
        });
    }

    group.finish()
}

criterion_group!(benches, bench_reassembler);
criterion_main!(benches);
