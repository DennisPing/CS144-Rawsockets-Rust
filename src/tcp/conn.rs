// use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
// use std::io::{Error, ErrorKind};
// use std::marker::PhantomData;
// use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs};
// use crate::tcp::receiver::TcpReceiver;
// use crate::tcp::sender::TcpSender;
//
// #[derive(Debug)]
// pub struct TcpConn<State> {
//     pub sender: TcpSender,
//     pub receiver: TcpReceiver,
//     pub state: PhantomData<State>,
// }
//
// /// Resolve hostname to an IPv4 address.
// fn resolve_hostname(hostname: &str) -> Result<SocketAddrV4, Error> {
//     // DNS lookup
//     let target = (hostname, 80u16);
//     let socket_addrs: Vec<SocketAddr> = target.to_socket_addrs()?.collect();
//
//     // Loop over addresses and filter for IPv4
//     for addr in socket_addrs {
//         if let SocketAddr::V4(v4_addr) = addr {
//             return Ok(v4_addr);
//         }
//     }
//
//     Err(Error::new(
//         ErrorKind::AddrNotAvailable,
//         "IPv4 address not found",
//     ))
// }
//
// /// Lookup the local IPv4 address from network interface.
// fn lookup_local_ip() -> Result<Ipv4Addr, Error> {
//     let interfaces = NetworkInterface::show().unwrap();
//
//     for interface in interfaces {
//         for addr in interface.addr {
//             // Step 3: Filter for non-loopback IPv4 addresses
//             if let Addr::V4(v4_addr) = addr {
//                 if !v4_addr.ip.is_loopback() {
//                     return Ok(v4_addr.ip);
//                 }
//             }
//         }
//     }
//
//     Err(Error::new(
//         ErrorKind::NotFound,
//         "No local IPv4 address found",
//     ))
// }
