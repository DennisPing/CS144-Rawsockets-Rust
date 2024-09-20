# TCP-IP-Raw-Sockets-Rust

![Build Status](https://github.com/DennisPing/TCP-IP-Raw-Sockets-Rust/actions/workflows/rust.yml/badge.svg)
[![codecov](https://codecov.io/gh/DennisPing/TCP-IP-Raw-Sockets-Rust/graph/badge.svg?token=Z0XXSP5MGP)](https://codecov.io/gh/DennisPing/TCP-IP-Raw-Sockets-Rust)

## Overview

An HTTP/1 and HTTP/2 Cleartext (H2C) implementation on top of a custom TCP/IP network stack implementation based on the
[Stanford CS144](https://cs144.github.io/) `libsponge` library and
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

2. Find your "network interface" name using: `ifconfig -a` and disable gro, tx, rx

    ```bash
    sudo ethtool -K <network interface> gro off
    sudo ethtool -K <network interface> tx off rx off
    ```

3. Example

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

**Note:** Does not work withs `HTTPS` URL's because encryption is not a goal of this project.

## Run Unit Tests

```bash
cargo test
```

## Generate Documentation

```bash
cargo doc
```
