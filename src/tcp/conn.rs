use crate::ip::flags::IPFlags;
use crate::ip::header::IPHeader;
use crate::packet;
use crate::socket::rawsocket;
use crate::tcp::byte_stream::ByteStream;
use crate::tcp::flags::TCPFlags;
use crate::tcp::header::TCPHeader;
use crate::tcp::reassembler::Reassembler;
use crate::tcp::receiver::TCPReceiver;
use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
use nix::sys::socket::SockProtocol;
use rand::rngs::ThreadRng;
use rand::Rng;
use std::io::{Error, ErrorKind};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs};
use std::os::fd::{AsRawFd, OwnedFd};
use std::time::Duration;

#[derive(Debug)]
pub struct TCPConn {
    pub hostname: String,
    pub local_addr: SocketAddrV4,
    pub remote_addr: SocketAddrV4,
    pub send_fd: OwnedFd,
    pub recv_fd: OwnedFd,
    tcp_receiver: TCPReceiver,
    rng: ThreadRng,
}

impl TCPConn {
    /// Set up a new `TcpConn` with the remote host. Does the 3-way handshake automatically.
    pub fn new(hostname: String, timeout: Duration) -> Result<Self, Error> {
        let local_ip = Self::lookup_local_ip()?;
        let mut rng = rand::thread_rng();
        let random_port = rng.gen_range(49152..65535);

        let local_addr = SocketAddrV4::new(local_ip, random_port);
        let remote_addr = Self::resolve_hostname(&hostname)?;

        let send_fd = rawsocket::new_send_socket(SockProtocol::Raw)?;
        let recv_fd = rawsocket::new_recv_socket(SockProtocol::Tcp)?;

        rawsocket::set_timeout(&recv_fd, timeout)?;

        let tcp_receiver = TCPReceiver::new(Reassembler::new(ByteStream::new(1024 * 1024 * 1)));

        Ok(Self {
            hostname,
            local_addr,
            remote_addr,
            send_fd,
            recv_fd,
            tcp_receiver,
            rng,
        })
    }

    fn connect(&mut self) -> Result<(), &'static str> {
        let seq_num: u32 = self.rng.gen();
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

    // pub fn recv_data(&mut self) -> Result<Vec<u8>, Error> {
    //     let mut buf = vec![0u8; 1500];
    //     let recv_fd = self.recv_fd.as_raw_fd();
    //
    //     loop {
    //         match read(recv_fd, &mut buf) {
    //             Ok(n) => {
    //                 buf.truncate(n);
    //
    //                 let (iph, tcph) = match header::unpack(&buf) {
    //                     Ok(result) => result,
    //                     Err(err) => {
    //                         return Err(Error::new(ErrorKind::InvalidData, err.to_string()))
    //                     }
    //                 };
    //
    //                 if !iph.src_ip.eq(self.remote_addr.ip()) {
    //                     return Err(Error::new(ErrorKind::Other, "mismatch ip address"));
    //                 }
    //
    //                 let payload_len = tcph.payload.len();
    //
    //                 self.tcp_receiver.receive_segment(
    //                     Wrapping(tcph.seq_num),
    //                     tcph.payload,
    //                     tcph.flags.contains(TCPFlags::SYN),
    //                     tcph.flags.contains(TCPFlags::FIN),
    //                 );
    //
    //                 let data = self.tcp_receiver.stream_out().peek_output(payload_len);
    //                 return Ok(data);
    //             }
    //             Err(Errno::EINTR) => {
    //                 continue; // Retry reading
    //             }
    //             Err(Errno::EAGAIN) | Err(Errno::EWOULDBLOCK) => {
    //                 return Err(Error::new(ErrorKind::TimedOut, "timeout")); // Timeout or would-block
    //             }
    //             Err(e) => {
    //                 return Err(Error::new(ErrorKind::Other, e.to_string()));
    //             }
    //         }
    //     }
    // }

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
            window: 65500u16,
            checksum: 0,
            urgent: 0,
            options: tcp_options.to_vec(),
            payload: payload.to_vec(),
        };

        packet::pack(&iph, &tcph)
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
