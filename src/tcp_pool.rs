//! Yaoi TcpPool

use crate::Dummy;
use crate::error::YaoiError;
use crate::TcpStream;

use core::marker::PhantomData;

use nohash_hasher::BuildNoHashHasher;
use hashbrown::HashMap;

use io_uring_bearer::UringBearer;

pub struct TcpPool<Cfun, Cdata>
where
    Cfun: Fn(&mut Cdata, &TcpStream) -> ()
{
    bearer: UringBearer<Dummy>,
    pool: HashMap<u32, TcpStream, BuildNoHashHasher<u32>>,
    c_fn: Option<Cfun>,
    s_fn: Option<Cfun>,    

    pool_count: usize,
    cd: PhantomData<Cdata>,
}

impl<Cfun: for<'a, 'b> Fn(&'a mut Cdata, &'b TcpStream), Cdata> TcpPool<Cfun, Cdata> {
    /// Create a new TcpPool with pool_cap count of streams.
    pub fn with_capacity(pool_cap: usize) -> Result<Self, YaoiError> {

        let cap = crate::capacity::TcpPoolCapacity::provide(pool_cap);
        let mut bearer = UringBearer::with_capacity(cap)
            .map_err(YaoiError::Bearer)?;        

        let pool: HashMap<u32, TcpStream, BuildNoHashHasher<u32>> =
            HashMap::<u32, TcpStream, BuildNoHashHasher<u32>>::with_capacity_and_hasher(
                pool_cap,
                BuildNoHashHasher::default(),
            );

        Ok(Self { bearer, pool, c_fn: None, s_fn: None, cd: PhantomData, pool_count: pool_cap })
    }
    /// Connect the whole TcpPool with calback cb upon connection established.
    pub fn connect_with_cb(&mut self, d: &mut Cdata, cb: Cfun) -> Result<usize, YaoiError> {
        // TOOD: guard current self.c_fn - what happens if there is previous connect ?
        self.c_fn = Some(cb);
        self.bearer.
        
    }
    /// Use the TcpPoo to serve a callback cb.
    pub fn serve_with_cb(&mut self, d: &mut Cdata, cb: Cfun) -> Result<usize, YaoiError> {
        todo!()
    }    
}
