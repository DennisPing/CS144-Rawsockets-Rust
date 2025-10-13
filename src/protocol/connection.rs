use std::collections::VecDeque;
use crate::common::byte_stream::ByteStream;
use crate::common::reassembler::Reassembler;
use crate::common::wrap32::Wrap32;
use crate::tcp::{TcpSegment, TcpView};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

pub struct Connection {
    pub state: TcpState,

    // -- Send side --
    pub send_una: Wrap32, // oldest unacked seq num
    pub send_nxt: Wrap32, // next seq num to send
    pub send_window: u16, // send window
    pub iss: Wrap32, // initial send seq num

    // --- Recv side --
    pub recv_nxt: Wrap32, // next seq num expected
    pub recv_window: u16, // recv window
    pub irs: Wrap32, // initial recv seq num

    // -- Buffers --
    pub outbound: ByteStream,
    pub inbound: Reassembler,

    pub segments_out: VecDeque<TcpSegment>,

    pub retransmit_timer: u64,
}

impl Connection {
    pub fn new() -> Self {
        unimplemented!()
    }

    pub fn on_recv_segment(&mut self, tcp_view: &TcpView<'_>) {
        self.state = match self.state {
            TcpState::Closed => self.handle_closed(tcp_view),
            TcpState::Listen => self.handle_listen(tcp_view),
            TcpState::SynSent => self.handle_syn_sent(tcp_view),
            TcpState::SynReceived => self.handle_syn_received(tcp_view),
            TcpState::Established => self.handle_established(tcp_view),
            TcpState::FinWait1 => self.handle_fin_wait1(tcp_view),
            TcpState::FinWait2 =>self.handle_fin_wait2(tcp_view),
            TcpState::CloseWait => self.handle_close_wait(tcp_view),
            TcpState::Closing => self.handle_closing(tcp_view),
            TcpState::LastAck => self.handle_last_ack(tcp_view),
            TcpState::TimeWait => self.handle_time_wait(tcp_view),
        }
    }

    fn handle_closed(&mut self, tcp_view: &TcpView<'_>) -> TcpState {
        !unimplemented!()
    }

    fn handle_listen(&mut self, tcp_view: &TcpView<'_>) -> TcpState {
        !unimplemented!()
    }

    fn handle_syn_sent(&mut self, tcp_view: &TcpView<'_>) -> TcpState {
        !unimplemented!()
    }

    fn handle_syn_received(&mut self, tcp_view: &TcpView<'_>) -> TcpState {
        !unimplemented!()
    }

    fn handle_established(&mut self, tcp_view: &TcpView<'_>) -> TcpState {
        !unimplemented!()
    }

    fn handle_fin_wait1(&mut self, tcp_view: &TcpView<'_>) -> TcpState {
        !unimplemented!()
    }

    fn handle_fin_wait2(&mut self, tcp_view: &TcpView<'_>) -> TcpState {
        !unimplemented!()
    }

    fn handle_close_wait(&mut self, tcp_view: &TcpView<'_>) -> TcpState {
        !unimplemented!()
    }

    fn handle_closing(&mut self, tcp_view: &TcpView<'_>) -> TcpState {
        !unimplemented!()
    }

    fn handle_last_ack(&mut self, tcp_view: &TcpView<'_>) -> TcpState {
        !unimplemented!()
    }


    fn handle_time_wait(&mut self, tcp_view: &TcpView<'_>) -> TcpState {
        !unimplemented!()
    }
}