//! YAOI Errors

use core::fmt;
use core::fmt::Display;

use io_uring_bearer::error::UringBearerError;

use hugepage::HugePageBytesError;
use io_uring_bufring::RingBufError;

/// Yaoi Errors
#[derive(Debug)]
pub enum YaoiError {
    /// std::io Error e.g from Syscalls
    StdIo(std::io::Error),
    /// UringBearer errors
    Bearer(UringBearerError),
    /// UringBearer errors
    Bearer2(UringBearerError),    
    /// Misc Yaoi Bug that should cause a controlled panic downstream.
    /// This should be reported.
    Bug(&'static str),
    /// Underlying io-uring Originating error
    IoUring(std::io::Error),
    /// HugeTlb can be set only once
    HugeTlbAlreadySet,
    /// HugeTlb related error, mostly OOM etc.
    HugeTlb(HugePageBytesError),
    /// Need buffers to run blueprints
    BpNeedBuffers,
    /// Can only create StreamingHugeTlb on Fno less than u16 as it relies on buffer grouping by fixed Fd id.
    // TODO: re-think this after figuring out the overall group id assignment but this is easy way out for now
    LimitHugeTlbU16,
    /// Streaming HugeTLB requires fixed ID for buffer group id which was not assigned.
    HugeTlbReqFixedId,
    /// RingBuf Related error
    RingBuf(RingBufError),
}

impl Display for YaoiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StdIo(e) => write!(f, "StdIo: {}", e),
            Self::Bearer(e) => write!(f, "UringBearer: {}", e),
            Self::Bearer2(e) => write!(f, "UringBearer: {}", e),            
            Self::Bug(e) => write!(f, "Yaoi Bug: {}", e),
            Self::IoUring(e) => write!(f, "Yaoi IoUring: {}", e),
            Self::HugeTlbAlreadySet => write!(f, "HugeTLB already set once."),
            Self::HugeTlb(e) => write!(f, "HugeTlb error: {}", e),
            Self::BpNeedBuffers => write!(f, "Need buffers to run blueprints."),
            Self::LimitHugeTlbU16 => write!(f, "Streaming incremental HugeTLB buffers can be created only on u16 TcpSteam Ids."),
            Self::HugeTlbReqFixedId => write!(f, "Streaming incremental HugeTLB buffers can be only created on fixed id TcpStreams."),
            Self::RingBuf(re) => write!(f, "RingBuf: {}", re),
        }
    }
}

impl core::error::Error for YaoiError {}
