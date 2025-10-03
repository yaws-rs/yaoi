//! Yaoi TcpStream

mod ent_streaming_hugetlb;
#[doc(inline)]
pub use ent_streaming_hugetlb::*;

mod ent_connected;
#[doc(inline)]
pub use ent_connected::*;

use core::net::SocketAddr;
use std::os::fd::RawFd;

use hugepage::HugePageBytes;

use crate::Blueprints;
use crate::YaoiError;
use blueprint::Orbit;

#[derive(Debug)]
pub enum TcpStream {
    Connected(EntConnected),
    StreamingHugeTlb(EntHugeTlb),
}

impl TcpStream {
    /// Run blueprints end-to-end Left to Right and back
    pub fn run_blueprints<const Layers: usize, O: Orbit>(
        &mut self,
        bp: &mut Blueprints<Layers, O>,
    ) -> Result<(), YaoiError> {
        match self {
            Self::Connected(_) => Err(YaoiError::BpNeedBuffers),
            Self::StreamingHugeTlb(ref mut huge_tlb) => huge_tlb.run_blueprints(bp),
        }
    }
    pub fn fixed_fd(&self) -> Option<u32> {
        match self {
            Self::Connected(e) => e.fixed_fd(),
            Self::StreamingHugeTlb(e) => e.fixed_fd(),
        }
    }
}
