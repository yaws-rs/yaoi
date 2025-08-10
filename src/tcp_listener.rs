//! Yaoi TcpListener

use crate::error::YaoiError;
use crate::strategy::StrategyListener;
use crate::Dummy; // TODO: ugh.
use crate::TcpStream;
use core::net::SocketAddr;

use ysockaddr::YSockAddrC;

use io_uring_bearer::completion::SubmissionRecordStatus;
use io_uring_bearer::Completion;
use io_uring_bearer::SubmissionFlags;
use io_uring_bearer::UringBearer;
use io_uring_opcode::{OpCode, OpCompletion};

use std::ffi::c_int;

/// Caller must ensure `T` is the correct type for `opt` and `val`.
pub(crate) unsafe fn setsockopt<T>(
    fd: c_int,
    opt: c_int,
    val: c_int,
    payload: T,
) -> Result<(), YaoiError> {
    let payload = core::ptr::addr_of!(payload).cast();
    syscall!(setsockopt(
        fd,
        opt,
        val,
        payload,
        size_of::<T>() as libc::socklen_t,
    ))
    .map(|_| ())
}

/// Yaoi TcpListener
pub struct TcpListener {
    local_addr: SocketAddr,
    bearer: UringBearer<Dummy>,
    listen_fd: u32,
    reg_mapped_acceptfd: i32,
    strategy: StrategyListener,
}

use crate::strategy::StrategyRegister;

fn register_strat_regular(
    bearer: &mut UringBearer<Dummy>,
    listener_fd: i32,
) -> Result<i32, YaoiError> {
    let reg_mapped_acceptfd = bearer
        .register_acceptor(listener_fd)
        .map_err(YaoiError::Bearer)? as i32;
    bearer.commit_registered_init().map_err(YaoiError::Bearer)?;
    Ok(reg_mapped_acceptfd)
}

use std::os::fd::RawFd;

fn register_strat_fixed(
    bearer: &mut UringBearer<Dummy>,
    listener_fd: i32,
    listen_count: u32,
) -> Result<i32, YaoiError> {
    let mut reg_map: Vec<RawFd> = vec![-1; listen_count as usize];
    reg_map[0] = listener_fd;

    bearer
        .io_uring()
        .submitter()
        .register_files(&reg_map)
        .map_err(YaoiError::IoUring)?;

    Ok(0)
}

fn accept_replenish_cc(
    bearer: &mut UringBearer<Dummy>,
    fixed_fds: bool,
    repl_cc: u32,
    addr: &SocketAddr,
    reg_mapped_acceptfd: i32,
) -> Result<(), YaoiError> {
    for x in 0..repl_cc {
        match addr {
            // SAFETY: We can only have IPv4 Listener through type
            SocketAddr::V4(_) => {
                unsafe { bearer.add_accept_ipv4(reg_mapped_acceptfd) }.map_err(YaoiError::Bearer)?
            }
            // SAFETY: We can only have IPv6 Listener through type
            SocketAddr::V6(_) => {
                unsafe { bearer.add_accept_ipv6(reg_mapped_acceptfd) }.map_err(YaoiError::Bearer)?
            }
        }
    }
    bearer.submit().map_err(YaoiError::Bearer)?;

    Ok(())
}

impl TcpListener {
    /// Listen at local address SocketAddr with the configured pending accept queue capacity and strategy for the listener.
    pub fn listen_with_strategy(
        addr: SocketAddr,
        q_count: usize,
        strategy: StrategyListener,
    ) -> Result<TcpListener, YaoiError> {
        let family = match addr {
            SocketAddr::V4(_) => libc::AF_INET,
            SocketAddr::V6(_) => libc::AF_INET6,
        };

        let ffi_sa: YSockAddrC = addr.into();

        // TODO: replace with io_uring socket()
        let listener_fd = syscall!(socket(family, libc::SOCK_STREAM, libc::IPPROTO_TCP))?;

        // TODO: replace with io_uring setosckopt
        unsafe { setsockopt(listener_fd, libc::SOL_SOCKET, libc::SO_REUSEADDR, 1) }.unwrap();

        let (sockaddr, sockaddr_len) = ffi_sa.as_c_sockaddr_len();
        // TODO: replace with io_uring bind
        let bind = syscall!(bind(listener_fd, sockaddr, sockaddr_len as _))?;

        // TODO: replace with io_uring listen
        syscall!(listen(listener_fd, q_count as i32))?;

        let cap = crate::capacity::TcpListenerCapacity::provide(q_count);
        let mut bearer = UringBearer::with_capacity(cap).map_err(YaoiError::Bearer)?;

        let reg_mapped_acceptfd = match strategy {
            StrategyListener::Replenish(repl_cc, StrategyRegister::Regular) => {
                let reg_mapped_acceptfd = register_strat_regular(&mut bearer, listener_fd)?;
                accept_replenish_cc(&mut bearer, false, repl_cc, &addr, reg_mapped_acceptfd)?;
                reg_mapped_acceptfd
            }
            StrategyListener::Replenish(repl_cc, StrategyRegister::Fixed(fixed_count)) => {
                let reg_mapped_acceptfd =
                    register_strat_fixed(&mut bearer, listener_fd, fixed_count)?;
                accept_replenish_cc(&mut bearer, true, repl_cc, &addr, reg_mapped_acceptfd)?;
                reg_mapped_acceptfd
            }
            _ => todo!("Strategy missing: {:?}", strategy),
        };

        Ok(TcpListener {
            local_addr: addr,
            bearer,
            listen_fd: listener_fd as u32,
            strategy,
            reg_mapped_acceptfd,
        })
    }
    /// Accept with callback using userdata.
    #[inline]
    pub fn accept_with_cb<F, U>(&mut self, user: &mut U, func: F) -> Result<(), YaoiError>
    where
        F: Fn(&mut U, i32, Option<SocketAddr>),
    {
        // SAFETY: Assuming we are doing single-shot Accept. This will not be safe with multi-shot (TODO).
        unsafe {
            self.bearer
                .handle_completions(user, None, |u, e, rec| {
                    let s_addr = match rec {
                        Completion::Accept(a_rec) => a_rec.sockaddr(),
                        _ => todo!(), // TODO: Bugs should not happen but we should provide surface for exposing it.
                    };
                    func(u, e.result(), s_addr);
                    SubmissionRecordStatus::Forget
                })
                .map_err(YaoiError::Bearer)?
        };
        Ok(())
    }
    /// Listener local address
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}
