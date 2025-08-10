//! YAOI Errors

use core::fmt;
use core::fmt::Display;

use io_uring_bearer::error::UringBearerError;

/// Yaoi Errors
#[derive(Debug)]
pub enum YaoiError {
    /// std::io Error e.g from Syscalls
    StdIo(std::io::Error),
    /// UringBearer errors
    Bearer(UringBearerError),
    /// Misc Yaoi Bug that should cause a controlled panic downstream.
    /// This should be reported.
    Bug(String),
    /// Underlying io-uring Originating error
    IoUring(std::io::Error),
}

impl Display for YaoiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StdIo(e) => write!(f, "StdIo: {}", e),
            Self::Bearer(e) => write!(f, "UringBearer: {}", e),
            Self::Bug(e) => write!(f, "Yaoi Bug: {}", e),
            Self::IoUring(e) => write!(f, "Yaoi IoUring: {}", e),
        }
    }
}

impl std::error::Error for YaoiError {}
