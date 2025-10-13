# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A custom TCP/IP network stack implementation in Rust, based on Stanford CS144 and Northeastern CS5700 coursework. Implements HTTP/1 and HTTP/2 Cleartext (H2C) on top of raw sockets. Linux-only (uses TUN device; requires iptables RST suppression and ethtool offload disabling). The main binary (`rawhttpget`) must run with `sudo`.

## Build Commands

```bash
cargo build --release          # Build release binary
cargo test                     # Run all tests
cargo test <test_name>         # Run specific test
cargo bench                    # Run criterion benchmarks (reassembler)
cargo doc                      # Generate documentation
```

Speed test binaries (after release build):
```bash
./target/release/byte_stream_speed_test
./target/release/reassembler_speed_test
```

## Architecture

### Module Structure

- **`common/`** - Core data structures shared across layers
  - `byte_stream.rs` - Ring buffer (`VecDeque`) with `std::io::Read/Write` traits for in-order byte delivery
  - `reassembler.rs` - Out-of-order segment reassembly using `BTreeMap` for efficient gap tracking
  - `wrap32.rs` - 32-bit wrapping sequence numbers (TCP seq/ack handling)

- **`tcp/`** - Transport layer
  - `segment.rs` - Owned `TcpSegment` with fluent builder methods (consume-and-return-self pattern)
  - `segment_view.rs` - Zero-copy `TcpView` for parsing raw bytes without allocation
  - `checksum.rs` - TCP checksum with IPv4 pseudo-header
  - `flags.rs` - TCP flags (SYN, ACK, FIN, etc.) using `bitflags`
  - `options.rs` - `TcpOptions` (owned) and `TcpOptionsView` (zero-copy) for TCP options parsing

- **`ip/`** - Network layer
  - `datagram.rs` - Owned `IpDatagram` with fluent builder methods
  - `datagram_view.rs` - Zero-copy `IpView` for parsing raw bytes
  - `pseudoheader.rs` - IPv4 pseudo-header for TCP/UDP checksum calculation
  - `flags.rs` - IP flags (DF, MF) with fragment offset packing

- **`http/`** - Application layer (client, request, response)

- **`device/`** - Network device abstraction (TUN)

### Key Patterns

**Two-layer owned/view design**: Each protocol layer has an owned type for building/storing and a view type for zero-copy parsing:
- `TcpSegment` (owned, builder) ↔ `TcpView` (zero-copy parse)
- `IpDatagram` (owned, builder) ↔ `IpView` (zero-copy parse)
- `TcpOptions` (owned) ↔ `TcpOptionsView` (zero-copy parse)

Use `from_view()` on owned types to materialize a parsed view into an owned value.

**Builder Pattern**: `TcpSegment` and `IpDatagram` use fluent builder methods directly (no separate builder struct). `TcpOptions` has a dedicated sub-builder via `TcpOptions::builder()`:
```rust
let seg = TcpSegment::new(src_port, dst_port)
    .seq(Wrap32::new(1000))
    .syn()
    .options(TcpOptions::builder().mss(1460).wscale(6).into_options());
```

**Serialization requires PseudoHeader**: `TcpSegment::to_bytes()` and `encode_into()` take a `&PseudoHeader` for checksum calculation. `TcpView::parse()` also validates the checksum against a pseudo-header.

**Wrap32 Arithmetic**: TCP sequence numbers use `Wrap32` for 32-bit wrapping arithmetic. Key methods:
- `Wrap32::wrap(abs_seq, isn)` - Convert absolute to wrapped
- `wrap32.unwrap(isn, checkpoint)` - Convert wrapped to absolute (nearest to checkpoint)

**Reassembler Algorithm**: Uses `BTreeMap` to track gaps. Merges overlapping segments on insert. Time complexity: O(log n) typical, O(k log n + k*m) worst case for k overlapping segments.

### Test Data

`tcp/mod.rs` contains `wireshark_sample` module with hex-encoded packets captured from real network traffic, used for validating encode/decode correctness.
