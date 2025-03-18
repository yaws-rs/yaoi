//! Yaoi TcpListener

use crate::Dummy; // TODO: ugh.
use crate::error::YaoiError;
use core::net::SocketAddr;
use crate::TcpStream;
use crate::strategy::StrategyListener;

use ysockaddr::YSockAddrC;

use io_uring_opcode::{OpCode, OpCompletion};
use io_uring_bearer::UringBearer;
use io_uring_bearer::Completion;
use io_uring_bearer::completion::SubmissionRecordStatus;


/// Yaoi TcpListener
pub struct TcpListener {    
    local_addr: SocketAddr,
    bearer: UringBearer<Dummy>,
    listen_fd: u32,
    reg_mapped_acceptfd: i32,
    strategy: StrategyListener,
}

impl TcpListener {    
    /// Listen at local address SocketAddr with the configured pending accept queue capacity and strategy for the listener.
    pub fn listen_with_strategy(addr: SocketAddr, q_count: usize, strategy: StrategyListener) -> Result<TcpListener, YaoiError> {
        let family = match addr {
            SocketAddr::V4(_) => libc::AF_INET,
            SocketAddr::V6(_) => libc::AF_INET6,
        };
        
        let ffi_sa: YSockAddrC = addr.into();
        let fd = syscall!(socket(family, libc::SOCK_STREAM, libc::IPPROTO_TCP))?;

        let (sockaddr, sockaddr_len) = ffi_sa.as_c_sockaddr_len();

        let bind = syscall!(bind(fd, sockaddr, sockaddr_len as _))?;

        syscall!(listen(fd, q_count as i32))?;
        let cap = crate::capacity::TcpListenerCapacity::provide(q_count);
        let mut bearer = UringBearer::with_capacity(cap)        
            .map_err(YaoiError::Bearer)?;

        let reg_mapped_acceptfd = bearer.register_acceptor(fd).map_err(YaoiError::Bearer)? as i32;
        bearer.commit_registered_init().map_err(YaoiError::Bearer)?;

        match addr {
            // SAFETY: We can only have IPv4 Listener through type
            SocketAddr::V4(_) => unsafe { bearer.add_accept_ipv4(reg_mapped_acceptfd) }.map_err(YaoiError::Bearer)?,
            // SAFETY: We can only have IPv6 Listener through type            
            SocketAddr::V6(_) => unsafe { bearer.add_accept_ipv6(reg_mapped_acceptfd) }.map_err(YaoiError::Bearer)?,
        }

        bearer.submit().map_err(YaoiError::Bearer)?;
        
        Ok ( TcpListener { local_addr: addr, bearer, listen_fd: fd as u32, strategy, reg_mapped_acceptfd } )
    }
    /// Accept with callback using userdata.
    #[inline]
    pub fn accept_with_cb<F, U>(&mut self, user: &mut U, func: F) -> Result<(), YaoiError>
    where
        F: Fn(&mut U, i32, Option<SocketAddr>)
    {

        // SAFETY: Assuming we are doing single-shot Accept. This will not be safe with multi-shot (TODO).
        unsafe { self.bearer.handle_completions(user, |u, e, rec| {
            let s_addr = match rec {
                Completion::Accept(a_rec) => a_rec.sockaddr(),
                _ => todo!(), // TODO: Bugs should not happen but we should provide surface for exposing it.
            };
            func(u, e.result(), s_addr);
            SubmissionRecordStatus::Forget
        }).map_err(YaoiError::Bearer)? };
        Ok(())
    }
    /// Listener local address
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
    
}
