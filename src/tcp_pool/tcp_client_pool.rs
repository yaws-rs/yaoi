//! Yaoi TcpClientPool

use crate::cmaps::{ClientMapMixed, MapConnected, MapRecvMulti, MapSentZc};
use crate::error::YaoiError;
use crate::Blueprints;
use crate::Dummy;
use crate::{EntConnected, EntHugeTlb, TcpStream};

use core::marker::PhantomData;
use core::net::SocketAddr;

use std::os::fd::RawFd;

use hashbrown::HashMap;
use nohash_hasher::BuildNoHashHasher;

use io_uring_op_socket::Socket;
use io_uring_opcode::OpExtSocket;

use io_uring_opcode_sets::Wrapper;

use io_uring_bearer::SubmissionFlags;
use io_uring_bearer::UringBearer;

use hugepage::HugePageBytes;

/// Context relating to each individual TcpStream
#[derive(Debug, Default)]
enum ClientSlotCtx {
    /// Initial state pre-connect
    #[default]
    Created,
    /// Connecting as fno
    Connecting(usize),
    /// Encountered an I/O Error during Connecting
    ConnectingError(i32),
    /// Connected, not Reading or Writing
    Connected(TcpStream),
    /// Connected and Reading
    Reading(TcpStream),
    /// Connected and Writing
    Writing(TcpStream),
    /// Connected, Reading and Writing
    ReadingAndWriting(TcpStream),
    /// Shut down completed Ok
    ShutdownOk,
}

impl ClientSlotCtx {
    /// Borrow the underlying TcpStream (if any) as mut
    #[inline]
    pub fn tcp_stream_mut(&mut self) -> Option<&mut TcpStream> {
        let s = match self {
            Self::Connected(ref mut s) => s,
            Self::Reading(ref mut s) => s,
            Self::Writing(ref mut s) => s,
            Self::ReadingAndWriting(ref mut s) => s,
            _ => return None,
        };
        Some(s)
    }
}

/// Batched pool of TcpClient entities ran through the same [`UringBearer`] instance.
pub struct TcpClientPool<Cfun, Cdata>
where
    Cfun: Fn(&mut Cdata, &mut TcpStream) -> (),
{
    bearer: UringBearer<Wrapper>,
    pool: HashMap<u32, ClientSlotCtx, BuildNoHashHasher<u32>>,
    c_fn: Option<Cfun>,
    s_fn: Option<Cfun>,

    pool_count: usize,
    state_connecting: usize,
    state_error: usize,
    state_connected: usize,
    state_shutdown: usize,

    cfg_hugetlb: Option<hugepage::HugePageChoice>,

    cd: PhantomData<Cdata>,
}

use io_uring_op_connect::Connect;
use io_uring_opcode::OpExtConnect;
use ysockaddr::YSockAddrR;

use io_uring_bearer::completion::SubmissionRecordStatus;
use io_uring_bearer::Completion;
use io_uring_opcode::{OpCode, OpCompletion};

use thingbuf::StaticThingBuf;

impl<Cfun: for<'a, 'b> Fn(&'a mut Cdata, &'b mut TcpStream), Cdata> TcpClientPool<Cfun, Cdata> {
    /// Create a new TcpClientPool with pool_cap count of streams.
    pub fn with_capacity(pool_cap: usize) -> Result<Self, YaoiError> {
        let cap = crate::capacity::TcpPoolCapacity::provide(pool_cap);
        let mut bearer = UringBearer::with_capacity(cap).map_err(YaoiError::Bearer)?;

        let pool: HashMap<u32, ClientSlotCtx, BuildNoHashHasher<u32>> =
            HashMap::<u32, ClientSlotCtx, BuildNoHashHasher<u32>>::with_capacity_and_hasher(
                pool_cap,
                BuildNoHashHasher::default(),
            );

        Ok(Self {
            bearer,
            pool,
            c_fn: None,
            s_fn: None,
            cd: PhantomData,
            pool_count: pool_cap,
            state_connecting: 0,
            state_connected: 0,
            state_shutdown: 0,
            state_error: 0,
            cfg_hugetlb: None,
        })
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
    /// Connect the whole TcpClientPool with calback cb upon connection established.
    pub fn connect_with_cb(
        &mut self,
        addr: SocketAddr,
        c: &mut Cdata,
        cb: Cfun,
    ) -> Result<usize, YaoiError> {
        // TOOD: guard current self.c_fn - what happens if there is previous connect ?
        self.c_fn = Some(cb);

        let ysaddr = YSockAddrR::from_sockaddr(addr);

        let flags_connect: Option<SubmissionFlags> = None;
        let flags_socket = Some(SubmissionFlags::default().on_io_link());

        let mut sock_list: Vec<RawFd> = vec![-1; self.pool_count];
        self.bearer
            .io_uring()
            .submitter()
            .register_files(&sock_list)
            .unwrap();

        let mut submitted = 0;

        for x in 0..self.pool_count {
            let op_idx = self
                .bearer
                .push_socket(
                    Socket::with_fixed_fd(
                        Some(x as u32),
                        libc::AF_INET,
                        libc::SOCK_STREAM,
                        libc::IPPROTO_TCP,
                    )
                    .unwrap(),
                    flags_socket,
                )
                .unwrap();

            let op_idx = self
                .bearer
                .push_connect(
                    Connect::with_ysockaddr_c(x as u32, ysaddr.as_c()).unwrap(),
                    flags_connect,
                )
                .unwrap();
            submitted += 1;
            self.state_connecting += 1;
            let x_u32 = x as u32;
            if let Some(p_entry) = self.pool.get_mut(&x_u32) {
                *p_entry = ClientSlotCtx::Connecting(op_idx);
            } else {
                self.pool.insert(x_u32, ClientSlotCtx::Connecting(op_idx));
            }
        }

        self.bearer.submit().unwrap();

        Ok(submitted)
    }
    /// Check-in next N completed connections
    pub fn check<const N: usize>(&mut self, udata: &mut Cdata) -> Result<usize, YaoiError> {
        #[derive(Debug)]
        struct UserData<const N: usize> {
            e: u32,
            bundle: StaticThingBuf<ClientMapMixed, { N }>,
        }

        let mut user = UserData::<N> {
            e: 0,
            bundle: StaticThingBuf::<ClientMapMixed, N>::new(),
        };

        // SAFETY: Completion rec does not need to live post-completion for Connect
        unsafe {
            self.bearer
                .handle_completions(&mut user, Some(N as u32), |user, entry, rec| {
                    match rec {
                        Completion::Socket(s) => {
                            // We can just forget given we linked it.
                            SubmissionRecordStatus::Forget
                        }
                        Completion::Connect(c) => {
                            let connect = c.unwrap_connect();
                            user.bundle
                                .push(ClientMapMixed::Connected(MapConnected {
                                    fixed_fd: connect.fixed_fd(),
                                    result: entry.result(),
                                }))
                                .unwrap();
                            user.e += 1;
                            SubmissionRecordStatus::Forget
                        }
                        Completion::RecvMulti(rcv_multi) => {
                            if entry.result() < 0 {
                                panic!(
                                    "Client / Error - recv_multi: {:?}, e = {:?}, rec = {:?}",
                                    rcv_multi, entry, rec
                                );
                            }
                            let buf_len = entry.result() as usize;
                            let buf_id = match io_uring::cqueue::buffer_select(entry.flags()) {
                                Some(id) => id,
                                None => {
                                    panic!("Client/RecvMulti must have buffer id... but it didn.t")
                                }
                            };

                            user.bundle
                                .push(ClientMapMixed::RecvMulti(MapRecvMulti {
                                    fixed_fd: rcv_multi.fixed_fd(),
                                    buf_id,
                                    buf_len,
                                    buf_grp: rcv_multi.buf_grp_id(),
                                }))
                                .unwrap();
                            SubmissionRecordStatus::Retain
                        }
                        Completion::SendZc(sz) => {
                            if entry.result() < 0 {
                                // TODO: errors
                                println!("SendZc failed/entry<{:?}> rec<{:?}>", entry, rec);
                            } else {
                                let buf_ref = match sz.buf_ref() {
                                    Some(buf_ref) => buf_ref,
                                    None => unreachable!(), // TODO: individual errors
                                };
                                user.bundle
                                    .push(ClientMapMixed::SentZc(MapSentZc {
                                        fixed_fd: sz.fixed_fd(),
                                        sent_out: entry.result() as usize,
                                        buf_ref: buf_ref,
                                    }))
                                    .unwrap();
                            }
                            SubmissionRecordStatus::Forget
                        }
                        _ => panic!("Queue had something else than expected? {:?}", rec), // TODO: handle better
                    }
                })
                .unwrap();
        };

        while let Some(mixed) = user.bundle.pop() {
            match mixed {
                ClientMapMixed::Nothing => {
                    unreachable!()
                }
                ClientMapMixed::RecvMulti(mut rcv_multi) => {
                    let slot_u32 = rcv_multi.fixed_fd;
                    let p_entry = match self.pool.get_mut(&slot_u32) {
                        Some(p_entry) => p_entry,
                        None => todo!("BUG: {slot_u32} not exist? - pool: {:?}", self.pool),
                    };

                    if let Some(tcp_stream) = p_entry.tcp_stream_mut() {
                        tcp_stream.cb_recv_multi(&mut rcv_multi)?;

                        match &self.c_fn {
                            Some(f) => f(udata, tcp_stream),
                            None => {}
                        }

                        let sm_send = tcp_stream.send_all_out(&mut self.bearer)?;
                        //                        if sm_send != 0 {
                        self.bearer.submit().map_err(YaoiError::Bearer)?;
                        //                        }

                        tcp_stream.try_free_buffers(&mut self.bearer)?;
                    } else {
                        unreachable!();
                    }
                }
                ClientMapMixed::SentZc(mut sent_zc) => {
                    let slot_u32 = sent_zc.fixed_fd as u32;

                    let p_entry = match self.pool.get_mut(&slot_u32) {
                        Some(p_entry) => p_entry,
                        None => todo!("BUG: {slot_u32} not exist? - pool: {:?}", self.pool),
                    };

                    if let Some(tcp_stream) = p_entry.tcp_stream_mut() {
                        tcp_stream.sent_zc(&mut sent_zc)?;
                    } else {
                        unreachable!();
                    }
                }
                ClientMapMixed::Connected(connected) => {
                    let slot_u32 = connected.fixed_fd as u32;

                    let p_entry = match self.pool.get_mut(&slot_u32) {
                        Some(p_entry) => p_entry,
                        None => todo!("BUG: {slot_u32} not exist? - pool: {:?}", self.pool),
                    };

                    let mut tcp_stream = match connected.result {
                        0 => TcpStream::Connected(EntConnected::from_fixed(connected.fixed_fd)),
                        _ => {
                            self.state_connecting -= 1;
                            self.state_error += 1;
                            *p_entry = ClientSlotCtx::ConnectingError(connected.result);
                            continue;
                        }
                    };

                    let mut wants_write = false;
                    let mut wants_read = false;

                    if let Some(tlb_choice) = self.cfg_hugetlb {
                        let hugetlb_in =
                            HugePageBytes::new(tlb_choice).map_err(YaoiError::HugeTlb)?;
                        let hugetlb_out =
                            HugePageBytes::new(tlb_choice).map_err(YaoiError::HugeTlb)?;
                        match tcp_stream {
                            TcpStream::Connected(ent_connected) => {
                                tcp_stream =
                                    TcpStream::StreamingHugeTlb(EntHugeTlb::from_connected(
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

                    self.state_connecting -= 1;
                    self.state_connected += 1;

                    match &self.c_fn {
                        Some(f) => f(udata, &mut tcp_stream),
                        None => {}
                    }

                    // TODO: these should not be ? as one of many may fail.
                    let sm_recv = tcp_stream.recv_multi(&mut self.bearer)?;
                    // TODO: these should not be ? as one of many may fail.
                    let sm_send = tcp_stream.send_all_out(&mut self.bearer)?;

                    println!("Client sm_recv<{sm_recv} sm_send<{sm_send}> wants_read/{wants_read} wants_write/{wants_write}");

                    *p_entry = match (wants_write, wants_read) {
                        (false, false) => ClientSlotCtx::Connected(tcp_stream),
                        (false, true) => ClientSlotCtx::Reading(tcp_stream),
                        (true, false) => ClientSlotCtx::Writing(tcp_stream),
                        (true, true) => ClientSlotCtx::ReadingAndWriting(tcp_stream),
                    };

                    //                    if sm_recv != 0 || sm_send != 0 {
                    self.bearer.submit().map_err(YaoiError::Bearer)?;
                    //                    }
                }
            }
        }
        Ok(user.e as usize)
    }
}
