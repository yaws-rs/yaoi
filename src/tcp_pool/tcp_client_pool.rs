//! Yaoi TcpClientPool

use crate::cmaps::MapConnected;
use crate::error::YaoiError;
use crate::Dummy;
use crate::TcpStream;

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

#[derive(Debug, Default)]
pub enum SlotCtx {
    #[default]
    Created,
    Connecting(usize),
    Error(i32),
    Connected(TcpStream),
    Shutdown,
}

pub struct TcpClientPool<Cfun, Cdata>
where
    Cfun: Fn(&mut Cdata, &TcpStream) -> (),
{
    bearer: UringBearer<Wrapper>,
    pool: HashMap<u32, SlotCtx, BuildNoHashHasher<u32>>,
    c_fn: Option<Cfun>,
    s_fn: Option<Cfun>,

    pool_count: usize,
    state_connecting: usize,
    state_error: usize,
    state_connected: usize,
    state_shutdown: usize,

    cd: PhantomData<Cdata>,
}

use io_uring_op_connect::Connect;
use io_uring_opcode::OpExtConnect;
use ysockaddr::YSockAddrR;

use io_uring_bearer::completion::SubmissionRecordStatus;
use io_uring_bearer::Completion;
use io_uring_opcode::{OpCode, OpCompletion};

use thingbuf::StaticThingBuf;

impl<Cfun: for<'a, 'b> Fn(&'a mut Cdata, &'b TcpStream), Cdata> TcpClientPool<Cfun, Cdata> {
    /// Create a new TcpClientPool with pool_cap count of streams.
    pub fn with_capacity(pool_cap: usize) -> Result<Self, YaoiError> {
        let cap = crate::capacity::TcpPoolCapacity::provide(pool_cap);
        let mut bearer = UringBearer::with_capacity(cap).map_err(YaoiError::Bearer)?;

        let pool: HashMap<u32, SlotCtx, BuildNoHashHasher<u32>> =
            HashMap::<u32, SlotCtx, BuildNoHashHasher<u32>>::with_capacity_and_hasher(
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
        })
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
                *p_entry = SlotCtx::Connecting(op_idx);
            } else {
                self.pool.insert(x_u32, SlotCtx::Connecting(op_idx));
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
            bundle: StaticThingBuf<MapConnected, { N }>,
        }

        let mut user = UserData::<N> {
            e: 0,
            bundle: StaticThingBuf::<MapConnected, N>::new(),
        };

        // SAFETY: Completion rec does not need to live post-completion for Connect
        unsafe {
            self.bearer
                .handle_completions(&mut user, Some(N as u32), |user, entry, rec| {
                    match rec {
                        Completion::Socket(s) => {
                            println!("Rec<{:?}>, Entry<{:?}> Socketed = {:?}", rec, entry, s);
                            SubmissionRecordStatus::Forget
                        }
                        Completion::Connect(c) => {
                            let connect = c.unwrap_connect();
                            user.bundle
                                .push(MapConnected {
                                    fixed_fd: connect.fixed_fd(),
                                    result: entry.result(),
                                })
                                .unwrap();
                            user.e += 1;
                            SubmissionRecordStatus::Forget
                        }
                        _ => panic!("Queue had something else than expected?"), // TODO: handle better
                    }
                })
        };

        while let Some(connected) = user.bundle.pop() {
            let slot_u32 = connected.fixed_fd as u32;

            let p_entry = match self.pool.get_mut(&slot_u32) {
                Some(p_entry) => p_entry,
                None => todo!("BUG: {slot_u32} not exist? - pool: {:?}", self.pool),
            };

            let tcp_stream = match connected.result {
                0 => TcpStream::from_fixed(connected.fixed_fd),
                _ => {
                    self.state_connecting -= 1;
                    self.state_error += 1;
                    *p_entry = SlotCtx::Error(connected.result);
                    continue;
                }
            };

            self.state_connecting -= 1;
            self.state_connected += 1;

            match &self.c_fn {
                Some(f) => f(udata, &tcp_stream),
                None => {}
            }

            *p_entry = SlotCtx::Connected(tcp_stream);
        }
        Ok(user.e as usize)
    }
}
