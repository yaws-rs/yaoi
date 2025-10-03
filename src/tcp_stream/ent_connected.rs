//! Yaoi TcpStream Connected Entity

use crate::YaoiError;
use core::net::SocketAddr;
use std::os::fd::RawFd;

/// Connected entity
#[derive(Debug)]
pub struct EntConnected {
    raw_fd: Option<RawFd>,
    fixed_fd: Option<u32>,
}

impl EntConnected {
    /// Mut be valid Fixed Fd
    pub fn from_fixed(f_fd: u32) -> Self {
        Self {
            raw_fd: None,
            fixed_fd: Some(f_fd),
        }
    }
    pub fn fixed_fd(&self) -> Option<u32> {
        self.fixed_fd
    }
}
