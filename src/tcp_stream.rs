//! Yaoi TcpStream

use crate::cmaps::MapConnected;

use core::net::SocketAddr;
use std::os::fd::RawFd;

/// Yaoi TcpStream
#[derive(Debug)]
pub struct TcpStream {
    raw_fd: Option<RawFd>,
    fixed_fd: Option<u32>,
}

impl TcpStream {
    /// Mut be valid Fixed Fd
    pub fn from_fixed(f_fd: u32) -> Self {
        Self {
            raw_fd: None,
            fixed_fd: Some(f_fd),
        }
    }
}
