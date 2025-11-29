//! Completion Maps for Type Conversions

use core::net::SocketAddr;

#[derive(Clone, Debug, Default)]
pub(crate) enum ClientMapMixed {
    #[default]
    Nothing,
    Connected(MapConnected),
    RecvMulti(MapRecvMulti),
    SentZc(MapSentZc),
}

#[derive(Clone, Debug, Default)]
pub(crate) enum ServerMapMixed {
    #[default]
    Nothing,
    Accepted(MapAccepted),
    RecvMulti(MapRecvMulti),
    SentZc(MapSentZc),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MapSentZc {
    pub(crate) fixed_fd: u32,
    pub(crate) sent_out: usize,
    pub(crate) buf_ref: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MapRecvMulti {
    pub(crate) fixed_fd: u32,
    pub(crate) buf_id: u16,
    pub(crate) buf_len: usize,
    pub(crate) buf_grp: u16,
}

// Usages:
// 1: TcpPool (Client) creates TcpStream upon Connect completion
#[derive(Clone, Debug, Default)]
pub(crate) struct MapConnected {
    pub(crate) fixed_fd: u32,
    pub(crate) result: i32,
}

// Usage:
// 1. TcpListener (Server) creates TcpStream upon Accept completion
#[derive(Clone, Debug, Default)]
pub(crate) struct MapAccepted {
    pub(crate) result: i32,
    pub(crate) s_addr: Option<SocketAddr>,
}
