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

/// Mostly in-order segments that overlap their neighbors and repeat bytes already received.
fn staggered_overlap(dataset: &[u8]) -> Vec<Segment<'_>> {
    let mut segments = Vec::new();
    for step in (0..dataset.len()).step_by(SEGMENT_SIZE) {
        // Small offsets (+2, +0, +1) to induce overlap
        for offset in [2, 0, 1] {
            let start = step + offset;
            if start < dataset.len() {
                segments.push(segment_at(dataset, start, 2 * SEGMENT_SIZE));
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
fn run(segments: &[Segment], out: &mut Vec<u8>) {
    let mut ra = Reassembler::new(ByteStream::new(CAPACITY));
    let mut buf = [0u8; 8192]; // Reusable buffer
    out.clear();

    for seg in segments {
        ra.insert(seg.start_idx, seg.data, seg.is_last).unwrap();

        // Read out any available bytes into the out buffer
        loop {
            let n = ra.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }
    }

    assert!(ra.output().eof(), "ByteStream should be EOF after finishing");
}

/// Criterion bench test
fn bench_reassembler(c: &mut Criterion) {
    let dataset = gen_dataset();
    let workloads = [
        ("staggered_overlap", staggered_overlap(&dataset)),
        ("gaps_then_fill", gaps_then_fill(&dataset)),
    ];

    let mut group = c.benchmark_group("reassembler");
    group.throughput(Throughput::Bytes(DATASET_SIZE as u64));
    group.noise_threshold(0.05); // Run-to-run noise on a laptop is a few percent

    for (name, segments) in &workloads {
        // Allocated once and reused, so the timing does not include growing the out buffer
        let mut out = Vec::with_capacity(DATASET_SIZE);

        // Validate correctness once, so we don't pollute the timing with repetitive checks
        run(segments, &mut out);
        assert_eq!(out, dataset, "{name}: data content mismatch");

        group.bench_function(*name, |bencher| {
            bencher.iter(|| {
                run(black_box(segments), &mut out);
                black_box(&out);
            })
        });
    }

    group.finish()
}

criterion_group!(benches, bench_reassembler);
criterion_main!(benches);
