use crate::net::header;
use crate::net::rawsocket;
use crate::net::{IPFlags, IPHeader, TCPFlags, TCPHeader};
use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
use nix::sys::socket::SockProtocol;
use rand::rngs::ThreadRng;
use rand::Rng;
use std::io::{Error, ErrorKind};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs};
use std::os::fd::OwnedFd;
use std::time::Duration;

#[derive(Debug)]
pub struct Conn {
    pub hostname: String,
    pub local_addr: SocketAddrV4,
    pub remote_addr: SocketAddrV4,
    pub adv_window: u16,  // Advertised window
    pub mss: u16,         // Max segment size
    pub window_scale: u8, // Window scale
    pub send_fd: OwnedFd,
    pub recv_fd: OwnedFd,
    initial_seq_num: u32, // The initial sequence number after connect()
    initial_ack_num: u32, // The initial ack number after connect()
    rng: ThreadRng,
}

impl Conn {
    /// Set up a new Connection with the remote host. Does the 3-way handshake automatically.
    pub fn new(hostname: String, timeout: Duration) -> Result<Self, Error> {
        let local_ip = Self::lookup_local_ip()?;
        let mut rng = rand::thread_rng();
        let random_port = rng.gen_range(49152..65535);

        let local_addr = SocketAddrV4::new(local_ip, random_port);
        let remote_addr = Self::resolve_hostname(&hostname)?;

        let send_fd = rawsocket::new_send_socket(SockProtocol::Raw)?;
        let recv_fd = rawsocket::new_recv_socket(SockProtocol::Tcp)?;

        rawsocket::set_timeout(&recv_fd, timeout)?;

        Ok(Self {
            hostname,
            local_addr,
            remote_addr,
            adv_window: 65535,
            mss: 1460,
            window_scale: 4,
            send_fd,
            recv_fd,
            initial_seq_num: 0,
            initial_ack_num: 0,
            rng,
        })
    }

    fn connect(&mut self) -> Result<(), &'static str> {
        let seq_num: u32 = self.rng.random();
        let ack_num: u32 = 0;

        Ok(())
    }

    fn send(
        &self,
        seq_num: u32,
        ack_num: u32,
        payload: &[u8],
        tcp_flags: TCPFlags,
    ) -> Result<(), &'static str> {
        Ok(())
    }

    fn build_packet(
        &self,
        seq_num: u32,
        ack_num: u32,
        payload: &[u8],
        tcp_flags: TCPFlags,
        tcp_options: &[u8],
    ) -> Vec<u8> {
        let data_offset = (5 + tcp_options.len()) / 4;
        let total_len = 20 + data_offset * 4 + payload.len();

        let iph = IPHeader {
            version: 4,
            ihl: 5,
            tos: 0,
            total_len: total_len as u16,
            id: 0,
            flags: IPFlags::DF,
            frag_offset: 0,
            ttl: 0,
            protocol: 0,
            checksum: 0,
            src_ip: *self.local_addr.ip(),
            dst_ip: *self.remote_addr.ip(),
        };

        let tcph = TCPHeader {
            src_port: self.local_addr.port(),
            dst_port: self.remote_addr.port(),
            seq_num,
            ack_num,
            data_offset: data_offset as u8,
            reserved: 0,
            flags: tcp_flags,
            window: self.adv_window,
            checksum: 0,
            urgent: 0,
            options: tcp_options.to_vec(),
            payload: payload.to_vec(),
        };

        header::pack(&iph, &tcph)
    }

    /// Resolve hostname to an IPv4 address.
    fn resolve_hostname(hostname: &str) -> Result<SocketAddrV4, Error> {
        // DNS lookup
        let target = (hostname, 80u16);
        let socket_addrs: Vec<SocketAddr> = target.to_socket_addrs()?.collect();

        // Loop over addresses and filter for IPv4
        for addr in socket_addrs {
            if let SocketAddr::V4(v4_addr) = addr {
                return Ok(v4_addr);
            }
        }

        Err(Error::new(
            ErrorKind::AddrNotAvailable,
            "IPv4 address not found",
        ))
    }

    /// Lookup the local IPv4 address from network interface.
    fn lookup_local_ip() -> Result<Ipv4Addr, Error> {
        let interfaces = NetworkInterface::show().unwrap();

        for interface in interfaces {
            for addr in interface.addr {
                // Step 3: Filter for non-loopback IPv4 addresses
                if let Addr::V4(v4_addr) = addr {
                    if !v4_addr.ip.is_loopback() {
                        return Ok(v4_addr.ip);
                    }
                }
            }
        }

        Err(Error::new(
            ErrorKind::NotFound,
            "No local IPv4 address found",
        ))
    }
}
