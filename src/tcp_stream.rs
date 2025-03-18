//! Yaoi TcpStream

use std::os::fd::RawFd;
use core::net::SocketAddr;

/// Yaoi TcpStream
#[derive(Debug)]
pub struct TcpStream {
    raw_fd: RawFd,
    addr: SocketAddr,
}

impl TcpStream {
    
}
