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

use crate::cmaps::{MapSentZc, MapRecvMulti};
use crate::Blueprints;
use crate::YaoiError;
use blueprint::Orbit;

use io_uring_bearer::UringBearer;
use io_uring_opcode_sets::Wrapper;

#[derive(Debug)]
pub enum TcpStream {
    Connected(EntConnected),
    StreamingHugeTlb(EntHugeTlb),
}

// TODO: this works as long as clientpool ent is <= u16 - use-case limiattion for hugetlb incremental streaming
#[inline]
fn _slot_u16_from_fixed_fd(try_fixed_fd: Option<u32>) -> Result<u16, YaoiError> {
    let test_in_range = 0u32..u16::MAX as u32;
    match try_fixed_fd {
        None => Err(YaoiError::HugeTlbReqFixedId),
        Some(out_range) if !test_in_range.contains(&out_range) => Err(YaoiError::LimitHugeTlbU16),
        Some(in_range) => Ok(in_range as u16),
    }
}

#[inline]
fn _require_fixed_fd(try_some: Option<u32>) -> Result<u32, YaoiError> {
    match try_some {
        Some(fixed_fd) => Ok(fixed_fd),
        None => Err(YaoiError::HugeTlbReqFixedId),
    }
}

impl TcpStream {
    /// Run blueprints end-to-end Left to Right and back
    #[inline]
    pub fn run_blueprints<const Layers: usize, O: Orbit>(
        &mut self,
        bp: &mut Blueprints<Layers, O>,
    ) -> Result<(), YaoiError> {
        match self {
            Self::Connected(_) => Err(YaoiError::BpNeedBuffers),
            Self::StreamingHugeTlb(ref mut huge_tlb) => huge_tlb.run_blueprints(bp),
        }
    }
    #[inline]
    pub fn fixed_fd(&self) -> Option<u32> {
        match self {
            Self::Connected(e) => e.fixed_fd(),
            Self::StreamingHugeTlb(e) => e.fixed_fd(),
        }
    }
    #[inline]
    pub fn left_wants_read(&self) -> bool {
        match self {
            Self::Connected(_) => false,
            Self::StreamingHugeTlb(ref huge_tlb) => huge_tlb.left_wants_read(),
        }
    }
    /// Indicate whehther the Left side wants to write
    #[inline]
    pub fn left_wants_write(&self) -> bool {
        match self {
            Self::Connected(_) => false,
            Self::StreamingHugeTlb(ref huge_tlb) => huge_tlb.left_wants_write(),
        }
    }
    /// Issue Recv to start receiving data
    #[inline]
    pub fn recv_multi(&mut self, bearer: &mut UringBearer<Wrapper>) -> Result<usize, YaoiError> {
        let maybe_fixed_fd = self.fixed_fd();
        match self {
            Self::Connected(_) => Err(YaoiError::BpNeedBuffers),
            Self::StreamingHugeTlb(ref mut huge_tlb) => {
                let fixed_fd = _require_fixed_fd(maybe_fixed_fd)?;
                let slot_u16 =_slot_u16_from_fixed_fd(maybe_fixed_fd)?;
                huge_tlb.recv_multi(fixed_fd, slot_u16, bearer)
            },
        }
    }
    // Attemp to reuse / free any unused buffers
    #[inline]
    pub fn try_free_buffers(&mut self, bearer: &mut UringBearer<Wrapper>) -> Result<(), YaoiError> {
        match self {
            Self::Connected(_) => Err(YaoiError::BpNeedBuffers),
            Self::StreamingHugeTlb(ref mut huge_tlb) => {
                huge_tlb.try_free_buffers(bearer)
            }
        }
    }
    /// Issue Send for all the pending data to be sent out
    // TODO: How do we phase / fragment / limit this (config knob through capacity?)
    #[inline]
    pub fn send_all_out(&mut self, bearer: &mut UringBearer<Wrapper>) -> Result<usize, YaoiError> {
        let maybe_fixed_fd = self.fixed_fd();
        match self {
            Self::Connected(_) => Err(YaoiError::BpNeedBuffers),
            Self::StreamingHugeTlb(ref mut huge_tlb) => {
                let fixed_fd = _require_fixed_fd(maybe_fixed_fd)?;
                huge_tlb.send_all_out(fixed_fd, bearer)
            },
        }
    }
    /// Called for each successfull SentZc completion
    #[inline]
    pub(crate) fn sent_zc(&mut self, sent_zc: &mut MapSentZc) -> Result<(), YaoiError> {
        let maybe_fixed_fd = self.fixed_fd();
        match self {
            Self::Connected(_) => Err(YaoiError::BpNeedBuffers),
            Self::StreamingHugeTlb(ref mut huge_tlb) => {
                let fixed_fd = _require_fixed_fd(maybe_fixed_fd)?;                
                huge_tlb.sent_zc(fixed_fd, sent_zc)
            },
        }
    }
    #[inline]
    pub(crate) fn cb_recv_multi(&mut self, recv_multi: &mut MapRecvMulti) -> Result<(), YaoiError> {
        let maybe_fixed_fd = self.fixed_fd();
        match self {
            Self::Connected(_) => Err(YaoiError::BpNeedBuffers),
            Self::StreamingHugeTlb(ref mut huge_tlb) => {
                let fixed_fd = _require_fixed_fd(maybe_fixed_fd)?;                
                huge_tlb.cb_recv_multi(fixed_fd, recv_multi)
            },
        }        
    }
}
