use std::net::Ipv4Addr;
use crate::ip::ip_flags::IpFlags;
use crate::ip::ip_header::IpHeader;
use crate::packet;
use crate::packet::errors::HeaderError;
use crate::tcp::tcp_flags::TcpFlags;
use crate::tcp::tcp_header::TcpHeader;
use crate::tcp::wrap32::Wrap32;

/// A builder for creating TCP messages. Single threaded only.
#[derive(Debug)]
pub struct TcpSegment {
    pub iph: IpHeader,   // IP header template
    pub tcph: TcpHeader, // TCP header template
}

impl TcpSegment {
    pub fn new(src_ip: Ipv4Addr, src_port: u16, dst_ip: Ipv4Addr, dst_port: u16) -> Self {
         let mut builder = TcpSegment {
            iph: Default::default(),
            tcph: Default::default(),
        };

         // Set the fixed fields of the IP header
        builder.iph.version = 4;
        builder.iph.ihl = 5;
        builder.iph.protocol = 6;
        builder.iph.src_ip = src_ip;
        builder.iph.dst_ip = dst_ip;

        // Set the fixed fields of the TCP header
        builder.tcph.src_port = src_port;
        builder.tcph.dst_port = dst_port;

        builder
    }

    pub fn build(&mut self) -> Result<Vec<u8>, HeaderError> {
        self.tcph.data_offset = 5 + (self.tcph.options.len() as u8) / 4;
        let total_len = 20 + (self.tcph.data_offset as usize) * 4 + self.tcph.payload.len();
        self.iph.total_len = total_len as u16;
        packet::wrap(&self.iph, &self.tcph)
    }

    pub fn ttl(&mut self, ttl: u8) -> &mut Self {
        self.iph.ttl = ttl;
        self
    }

    pub fn seq_no(&mut self, seq_no: Wrap32) -> &mut Self {
        self.tcph.seq_no = seq_no;
        self
    }

    pub fn ack_no(&mut self, ack_no: Wrap32) -> &mut Self {
        self.tcph.ack_no = ack_no;
        self
    }

    pub fn ip_flags(&mut self, flags: IpFlags) -> &mut Self {
        self.iph.flags = flags;
        self
    }

    pub fn tcp_flags(&mut self, flags: TcpFlags) -> &mut Self {
        self.tcph.flags = flags;
        self
    }

    pub fn window_size(&mut self, window_size: u16) -> &mut Self {
        self.tcph.window = window_size;
        self
    }

    pub fn tcp_options(&mut self, options: &[u8]) -> &mut Self {
        self.tcph.options = options.to_vec();
        self
    }

    pub fn payload(&mut self, payload: &[u8]) -> &mut Self {
        self.tcph.payload = payload.to_vec();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tcp::tcp_flags::TcpFlags;
    use std::net::Ipv4Addr;
    use crate::tcp::wrap32::Wrap32;

    #[test]
    fn test_build() {
        let src_ip = Ipv4Addr::new(192, 168, 1, 1);
        let dst_ip = Ipv4Addr::new(192, 168, 1, 2);
        let src_port = 12345;
        let dst_port = 80;

        let mut builder = TcpSegment::new(src_ip, src_port, dst_ip, dst_port);
        let packet = builder
            .ttl(64)
            .ip_flags(IpFlags::DF)
            .seq_no(Wrap32::new(12345))
            .ack_no(Wrap32::new(67890))
            .tcp_flags(TcpFlags::SYN)
            .window_size(65535)
            .tcp_options(&[])
            .payload(&[])
            .build()
            .unwrap();

        let iph = IpHeader::parse(&packet[0..20]).unwrap();
        let tcph = TcpHeader::parse(&packet[20..], &iph).unwrap();

        assert_eq!(iph.src_ip, src_ip);
        assert_eq!(iph.dst_ip, dst_ip);
        assert_eq!(iph.ttl, 64);
        assert_eq!(iph.flags, IpFlags::DF);
        assert_eq!(tcph.src_port, src_port);
        assert_eq!(tcph.dst_port, dst_port);
        assert_eq!(tcph.seq_no, Wrap32::new(12345));
        assert_eq!(tcph.ack_no, Wrap32::new(67890));
        assert_eq!(tcph.flags, TcpFlags::SYN);
        assert_eq!(tcph.window, 65535);
        assert_eq!(tcph.options.len(), 0);
        assert_eq!(tcph.payload.len(), 0);

        // Check that the unwrapped packet is the same as the original
        let result = packet::unwrap(packet.as_ref());
        assert!(result.is_ok());
        let (iph2, tcph2) = result.unwrap();
        assert_eq!(iph, iph2);
        assert_eq!(tcph, tcph2);
    }

    #[test]
    fn test_build_with_options_and_payload() {
        let src_ip = Ipv4Addr::new(192, 168, 1, 1);
        let dst_ip = Ipv4Addr::new(192, 168, 1, 2);
        let src_port = 12345;
        let dst_port = 80;

        let mut builder = TcpSegment::new(src_ip, src_port, dst_ip, dst_port);
        let packet = builder
            .ttl(64)
            .ip_flags(IpFlags::DF)
            .seq_no(Wrap32::new(12345))
            .ack_no(Wrap32::new(67890))
            .tcp_flags(TcpFlags::SYN | TcpFlags::ACK)
            .window_size(65535)
            .tcp_options(&[1, 2, 3, 4])
            .payload(&[5, 6, 7, 8])
            .build()
            .unwrap();

        let iph = IpHeader::parse(&packet[0..20]).unwrap();
        let tcph = TcpHeader::parse(&packet[20..], &iph).unwrap();

        assert_eq!(iph.src_ip, src_ip);
        assert_eq!(iph.dst_ip, dst_ip);
        assert_eq!(iph.ttl, 64);
        assert_eq!(iph.flags, IpFlags::DF);
        assert_eq!(tcph.src_port, src_port);
        assert_eq!(tcph.dst_port, dst_port);
        assert_eq!(tcph.seq_no, Wrap32::new(12345));
        assert_eq!(tcph.ack_no, Wrap32::new(67890));
        assert_eq!(tcph.flags, TcpFlags::SYN | TcpFlags::ACK);
        assert_eq!(tcph.window, 65535);
        assert_eq!(tcph.options, vec![1, 2, 3, 4]);
        assert_eq!(tcph.payload, vec![5, 6, 7, 8]);

        // Check that the unwrapped packet is the same as the original
        let result = packet::unwrap(packet.as_ref());
        assert!(result.is_ok());
        let (iph2, tcph2) = result.unwrap();
        assert_eq!(iph, iph2);
        assert_eq!(tcph, tcph2);
    }
}