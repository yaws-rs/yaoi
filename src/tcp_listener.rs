//! Yaoi TcpListener

use crate::error::YaoiError;
use crate::strategy::StrategyListener;
use crate::TcpStream;
use crate::EntHugeTlb;
use core::net::SocketAddr;
use core::marker::PhantomData;

use ysockaddr::YSockAddrC;

use io_uring_bearer::completion::SubmissionRecordStatus;
use io_uring_bearer::Completion;
use io_uring_bearer::TargetFd;
use io_uring_bearer::SubmissionFlags;
use io_uring_bearer::UringBearer;
use io_uring_opcode::{OpCode, OpCompletion};
use io_uring_opcode_sets::Wrapper;

use crate::cmaps::{ServerMapMixed, MapAccepted, MapRecvMulti, MapSentZc};
use crate::EntConnected;

use std::ffi::c_int;

use thingbuf::StaticThingBuf;
use hugepage::HugePageBytes;

use hashbrown::HashMap;
use nohash_hasher::BuildNoHashHasher;

/// Context relating to each individual TcpStream                                                                                                    
#[derive(Debug, Default)]
enum ListenerSlotCtx {
    /// Initial state pre-connect
    #[default]
    Created,
    /// Connected, not Reading or Writing
    Accepted(TcpStream),
    /// Connected and Reading
    Reading(TcpStream),
    /// Connected and Writing
    Writing(TcpStream),
    /// Connected, Reading and Writing
    ReadingAndWriting(TcpStream),
    /// Shut down completed Ok
    ShutdownOk,
}

impl ListenerSlotCtx {
    /// Borrow the underlying TcpStream (if any) as mut
    pub fn tcp_stream_mut(&mut self) -> Option<&mut TcpStream> {
        let s = match self {
            Self::Accepted(ref mut s) => s,
            Self::Reading(ref mut s) => s,
            Self::Writing(ref mut s) => s,
            Self::ReadingAndWriting(ref mut s) => s,
            _ => return None,
        };
        Some(s)
    }
}
    

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
pub struct TcpListener<Cfun, Cdata> {
    local_addr: SocketAddr,
    bearer: UringBearer<Wrapper>,
    listen_fd: u32,
    reg_mapped_acceptfd: i32,
    strategy: StrategyListener,

    pool: HashMap<u32, ListenerSlotCtx, BuildNoHashHasher<u32>>,
    pool_count: usize,
    
    a_fn: Option<Cfun>,
    cfg_hugetlb: Option<hugepage::HugePageChoice>,
    cd: PhantomData<Cdata>,
}

use crate::strategy::StrategyRegister;

fn register_strat_regular(
    bearer: &mut UringBearer<Wrapper>,
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
    bearer: &mut UringBearer<Wrapper>,
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
    bearer: &mut UringBearer<Wrapper>,
    fixed_fds: bool,
    repl_cc: u32,
    addr: &SocketAddr,
    reg_mapped_acceptfd: i32,
) -> Result<(), YaoiError> {
    let target_fd = match fixed_fds {
        true => TargetFd::AutoRegistered,
        false => TargetFd::Unregistered,
    };
    for x in 0..repl_cc {
        match addr {
            // SAFETY: We can only have IPv4 Listener through type
            SocketAddr::V4(_) => {
                unsafe { bearer.add_accept_ipv4(reg_mapped_acceptfd, target_fd) }.map_err(YaoiError::Bearer)?
            }
            // SAFETY: We can only have IPv6 Listener through type
            SocketAddr::V6(_) => {
                unsafe { bearer.add_accept_ipv6(reg_mapped_acceptfd, target_fd) }.map_err(YaoiError::Bearer)?
            }
        }
    }
    bearer.submit().map_err(YaoiError::Bearer)?;

    Ok(())
}

impl<Cfun: for<'a, 'b> Fn(&'a mut Cdata, &'b mut TcpStream), Cdata> TcpListener<Cfun, Cdata> {
    /// Listen at local address SocketAddr with the configured pending accept queue capacity and strategy for the listener.
    pub fn listen_with_strategy(
        addr: SocketAddr,
        q_count: usize,
        strategy: StrategyListener,
    ) -> Result<TcpListener<Cfun, Cdata>, YaoiError> {
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
            StrategyListener::Replenish(repl_cc, StrategyRegister::Regular, _) => {
                let reg_mapped_acceptfd = register_strat_regular(&mut bearer, listener_fd)?;
                accept_replenish_cc(&mut bearer, false, repl_cc, &addr, reg_mapped_acceptfd)?;
                reg_mapped_acceptfd
            }
            StrategyListener::Replenish(repl_cc, StrategyRegister::Fixed(fixed_count), _) => {
                let reg_mapped_acceptfd =
                    register_strat_fixed(&mut bearer, listener_fd, fixed_count)?;
                accept_replenish_cc(&mut bearer, true, repl_cc, &addr, reg_mapped_acceptfd)?;
                reg_mapped_acceptfd
            }
            _ => todo!("Strategy missing: {:?}", strategy),
        };

        let pool: HashMap<u32, ListenerSlotCtx, BuildNoHashHasher<u32>> =
            HashMap::<u32, ListenerSlotCtx, BuildNoHashHasher<u32>>::with_capacity_and_hasher(
                strategy.cap_pool() as usize,
                BuildNoHashHasher::default(),
            );
        
        Ok(TcpListener {
            local_addr: addr,
            bearer,
            listen_fd: listener_fd as u32,
            pool,
            a_fn: None,
            cfg_hugetlb: None,
            pool_count: strategy.cap_pool() as usize,
            strategy,
            reg_mapped_acceptfd,
            cd: PhantomData,
        })
    }
    /// Setup Accept with callback using userdata.
    #[inline]
    pub fn accept_with_cb(&mut self, user: &mut Cdata, func: Cfun) -> Result<(), YaoiError> {
        self.a_fn = Some(func);
        Ok(())
    }
    /// Check for max N events
    #[inline]
    pub fn check<const N: usize>(&mut self, udata: &mut Cdata) -> Result<(), YaoiError> {

        #[derive(Debug)]
        struct UserData<const N: usize> {
            e: u32,
            bundle: StaticThingBuf<ServerMapMixed, { N }>,
        }

        let mut user = UserData::<N> {
            e: 0,
            bundle: StaticThingBuf::<ServerMapMixed, N>::new(),
        };

        
        // SAFETY: Assuming we are doing single-shot Accept. This will not be safe with multi-shot (TODO).
        unsafe {
            self.bearer
                .handle_completions(&mut user, Some(N as u32), |u, e, rec| {
                    match rec {
                        Completion::Accept(a_rec) => {
                            u.bundle
                                .push(ServerMapMixed::Accepted(MapAccepted {
                                    s_addr: a_rec.sockaddr(),
			                        result: e.result(),
                                }))
                                .unwrap();
                            u.e += 1;
                            SubmissionRecordStatus::Forget
                        },
                        Completion::SendZc(sz) => {
                            if e.result() < 0 {
                                // TODO: errors
                                println!("Listener/SendZc failed/entry<{:?}> rec<{:?}>", e, rec);
                            }
                            else {
                                let buf_ref = match sz.buf_ref() {
                                    Some(buf_ref) => buf_ref,
                                    None => unreachable!(), // TODO: individual errors
	                            };
                                u.bundle.push(ServerMapMixed::SentZc(
                                    MapSentZc{ fixed_fd: sz.fixed_fd(),
                                               sent_out: e.result() as usize,
                                               buf_ref: buf_ref }
                                ));

                            }
                            match io_uring::cqueue::more(e.flags()) {
                                false => SubmissionRecordStatus::Forget,
                                true => SubmissionRecordStatus::Retain,
                            }
                        },
                        Completion::RecvMulti(rcv_multi) => {
                            if e.result() < 0 {
                                panic!("Server / Error - recv_multi: {:?}, e = {:?}, rec = {:?}", rcv_multi, e, rec);
                            }

                            if !io_uring::cqueue::more(e.flags()) {
                                println!("RecvMulti No-More triggered.");
                                // KTODO no more recv
                                SubmissionRecordStatus::Forget
                            }
                            else {
                                println!("RecvMulti = {:?}, e = {:?}, rec = {:?}", rcv_multi, e, rec);
                                let buf_len = e.result() as usize;
                                let buf_id = match io_uring::cqueue::buffer_select(e.flags()) {
                                    Some(id) => id,
                                    None => panic!("RecvMulti must have buffer id... but it didn.t"),
                                };
                                
                                u.bundle
                                    .push(ServerMapMixed::RecvMulti(MapRecvMulti {
                                        fixed_fd: rcv_multi.fixed_fd(),
                                        buf_id,
                                        buf_len,
                                        buf_grp: rcv_multi.buf_grp_id(),
                                    }))
                                    .unwrap();
                                SubmissionRecordStatus::Retain
                            }
                        },
                        // TODO: Bugs should not happen but we should provide surface for exposing it.                        
                        _ => panic!("Server - Missing handle_completion for e = {:?}, rec = {:?}", e, rec),
                    }
                })
                .map_err(YaoiError::Bearer)?
        };

        while let Some(mixed) = user.bundle.pop() {
            match mixed {
                ServerMapMixed::Nothing => {
                    unreachable!()
                },
                ServerMapMixed::SentZc(mut sent_zc) => {
                    let slot_u32 = sent_zc.fixed_fd as u32;

                    let p_entry = match self.pool.get_mut(&slot_u32) {
                        Some(p_entry) => p_entry,
                        None => todo!("BUG: {slot_u32} not exist? - pool: {:?}", self.pool),
                    };

                    if let Some(tcp_stream) = p_entry.tcp_stream_mut() {
                        tcp_stream.sent_zc(&mut sent_zc)?;
                    }
                    else {
                        unreachable!();
                    }                    
                },
                ServerMapMixed::RecvMulti(mut rcv_multi) => {
                    let slot_u32 = rcv_multi.fixed_fd;
                    
                    let p_entry = match self.pool.get_mut(&slot_u32) {
                        Some(p_entry) => p_entry,
                        None => todo!("BUG: {slot_u32} not exist? - pool: {:?}", self.pool),
                    };

                    if let Some(tcp_stream) = p_entry.tcp_stream_mut() {
                        tcp_stream.cb_recv_multi(&mut rcv_multi)?;

                        loop {
                            match &self.a_fn {
                                Some(f) => f(udata, tcp_stream),
                                None => {}
                            }

                            let sm_send = tcp_stream.send_all_out(&mut self.bearer)?;
                            if sm_send != 0 {
                                self.bearer.submit().map_err(YaoiError::Bearer)?;
                            }
                            else {
                                break;
                            }
                        }
                        tcp_stream.try_free_buffers(&mut self.bearer)?;
                    }
                    else {
                        unreachable!();
                    }                    
                },
                ServerMapMixed::Accepted(accepted) => {
                    println!("Accepted = {:?}", accepted);
                    let s_addr = accepted.s_addr;
                    let slot_u32 = match accepted.result {
                        0..i32::MAX => accepted.result as u32,
                        _ => {
                            // TODO: signal errors through closure
                            // TODO: replenish Accept
                            continue;
                        },
                    };
                    
                    // TODO: regular fd's
                    let mut tcp_stream = match s_addr {
                        Some(s_addr) => TcpStream::Connected(EntConnected::from_fixed(slot_u32).and_peer_addr(s_addr)),
                        None => TcpStream::Connected(EntConnected::from_fixed(slot_u32)),
                    };
                    
                    let mut wants_write = false;
                    let mut wants_read = false;
                    
                    if let Some(tlb_choice) = self.cfg_hugetlb {
                        let hugetlb_in = HugePageBytes::new(tlb_choice).map_err(YaoiError::HugeTlb)?;
                        let hugetlb_out = HugePageBytes::new(tlb_choice).map_err(YaoiError::HugeTlb)?;
                        match tcp_stream {
                            TcpStream::Connected(ent_connected) => {
                                // TODO error in one of many
                                tcp_stream = TcpStream::StreamingHugeTlb(EntHugeTlb::from_connected(
                                    &mut self.bearer,
                                    ent_connected,
                                    hugetlb_in,
                                    hugetlb_out,
                                )?);
                                wants_write = tcp_stream.left_wants_write();
                                wants_read = tcp_stream.left_wants_read();
                            }
                            _ => return Err(YaoiError::Bug("Type error. Expected EntConnected?")),
                        }
                    }
                    match &self.a_fn {
                        Some(f) => f(udata, &mut tcp_stream),
                        None => {}
                    }
                    
                    let sm_recv = tcp_stream.recv_multi(&mut self.bearer)?;
                    let sm_send = tcp_stream.send_all_out(&mut self.bearer)?;
                    
                    let slot = match (wants_write, wants_read) {
                        (false, false) => ListenerSlotCtx::Accepted(tcp_stream),
                        (false, true) => ListenerSlotCtx::Reading(tcp_stream),
                        (true, false) => ListenerSlotCtx::Writing(tcp_stream),
                        (true, true) => ListenerSlotCtx::ReadingAndWriting(tcp_stream),
                    };
                    
                    // TODO: existing (errored? occupied!?) slot
                    println!("self.pool.insert({})", slot_u32);
                    self.pool.insert(slot_u32, slot);
                    
                    if sm_recv != 0 || sm_send != 0 {
                        self.bearer.submit().map_err(YaoiError::Bearer)?;
                    }
                }
            }
        }
        
        Ok(())
    }
    /// Set HugeTLB as buffers backend with the given choice of hugetlb size.
    pub fn set_hugetlb(&mut self, tlb_choice: hugepage::HugePageChoice) -> Result<(), YaoiError> {
        match self.cfg_hugetlb {
            None => {
                self.cfg_hugetlb = Some(tlb_choice);
                Ok(())
            }
            _ => Err(YaoiError::HugeTlbAlreadySet),
        }
	}    
    /// Listener local address
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}
