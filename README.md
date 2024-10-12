# CS144-Rawsockets-Rust

![Build Status](https://github.com/DennisPing/TCP-IP-Raw-Sockets-Rust/actions/workflows/rust.yml/badge.svg)
[![codecov](https://codecov.io/gh/DennisPing/CS144-Rawsockets-Rust/graph/badge.svg?token=Z0XXSP5MGP)](https://codecov.io/gh/DennisPing/CS144-Rawsockets-Rust)

## Overview

An HTTP/1 and HTTP/2 Cleartext (H2C) implementation on top of a custom TCP/IP network stack based on the
[Stanford CS144](https://cs144.github.io/) `libsponge` library (now called `minnow`) and
the [Northeastern CS5700](https://david.choffnes.com/classes/cs4700sp22/project4.php) `rawsockets` project.

Done for self-learning purposes.

| OSI Layer             | Description                 | Implemented |
|-----------------------|-----------------------------|-------------|
| 7. Application Layer  | HTTP/1 and HTTP/2 H2C       | ✔️          |
| 6. Presentation Layer | Gzip and Brotli compression | ✔️          |
| 5. Session Layer      | Connection and Session      | ✔️          |
| 4. Transport Layer    | TCP Protocol                | ✔️          |
| 3. Network Layer      | IP Protocol and Router      | ✔️          |
| 2. Data Link Layer    | Ethernet                    |             |
| 1. Physical Layer     | Network card                |             |

## Requirements

- Linux
- Rust 1.69+

## Required System Changes

1. Modify iptables rule

    ```bash
    sudo iptables -A OUTPUT -p tcp --tcp-flags RST RST -j DROP
    ```
   
2. Find your "network interface" logical name
   ```bash
   sudo lshw -class network
   
   *-network                 
    description: Wireless interface
    product: Wi-Fi 6 AX200
    vendor: Intel Corporation
    physical id: 0
    bus info: pci@0000:04:00.0
    logical name: wlp4s0 <--- Use this name
    version: 1a
    serial: 50:e0:85:89:ca:b5
    width: 64 bits
    clock: 33MHz
    ... (omitted)
   ```

3. Disable `gro, tx, rx`

    ```bash
    sudo ethtool -K <network interface> gro off
    sudo ethtool -K <network interface> tx off rx off
    ```

4. Example

    ```bash
    sudo iptables -A OUTPUT -p tcp --tcp-flags RST RST -j DROP
    sudo ethtool -K wlp4s0 gro off
    sudo ethtool -K wlp4s0 tx off rx off
    ```

## How to Build

```bash
cargo build --release
```

## How to Run

```bash
sudo ./target/release/rawhttpget [http://exampleurl.com]
```

**Note:** Does not work with `HTTPS` URL's because encryption is not a goal of this project.

## Run Unit Tests

```bash
cargo test
```

## Run Benchmarks

```bash
cd target/release/
./byte_stream_speed_test
./reassembler_speed_test
```

## Generate Documentation

```bash
cargo doc
```
