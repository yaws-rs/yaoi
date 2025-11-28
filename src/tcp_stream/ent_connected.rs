//! Yaoi TcpStream Connected Entity

use crate::YaoiError;
use core::net::SocketAddr;
use std::os::fd::RawFd;

/// Connected entity
#[derive(Debug)]
pub struct EntConnected {
    raw_fd: Option<RawFd>,
    fixed_fd: Option<u32>,
    peer_addr: Option<SocketAddr>,
}

impl EntConnected {
    /// Mut be valid Fixed Fd
    pub fn from_fixed(f_fd: u32) -> Self {
        Self {
            raw_fd: None,
            fixed_fd: Some(f_fd),
            peer_addr: None,
        }
    }
    /// With a known Peer Address
    pub fn and_peer_addr(mut self, addr: SocketAddr) -> Self {
        self.peer_addr = Some(addr);
        self
    }
    pub fn fixed_fd(&self) -> Option<u32> {
        self.fixed_fd
    }
}
