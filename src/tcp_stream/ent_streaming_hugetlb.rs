//! Yaoi TcpStream Streaming HugeTlb entity

use super::EntConnected;

use hugepage::HugePageBytes;

use io_uring_bearer::UringBearer;
use io_uring_opcode_sets::Wrapper;

use crate::cmaps::{MapSentZc, MapRecvMulti};
use crate::Blueprints;
use crate::YaoiError;
use blueprint::Orbit;
use blueprint::{Left, NoRight, Right, InBuffer};

use core::num::NonZero;
use io_uring_bufring::{RingBufChoice, RingBufUnregistered, RingBufRegistered, BufferCount, PerBufferSize};

/// Yaoi HugeTlb TcpStream
#[derive(Debug)]
pub struct EntHugeTlb {
    connection: EntConnected,
    hugetbl_out: HugePageBytes,
//    ring_out: RingBufRegistered,
    // If registered, must be unregistered before dropped
    hugetbl_in: HugePageBytes,
    ring_in: RingBufRegistered,
    cursor: CursorLeft,
    is_ready: bool,
    want_read: bool,
    want_write: bool,
    in_blocked: bool,
}

#[derive(Debug, Default)]
struct CursorLeft {
    pos_in_start: usize,
    pos_in_end: usize,
    idx_in_reused: u16,
    pos_out_start: usize,
    pos_out_end: usize,
    idx_out_cur: u16,
}

impl EntHugeTlb {
    fn impl_left(&mut self) -> impl Left + use<'_> {
        self
    }
}

#[inline]
fn register_out_bufs(bearer: &mut UringBearer<Wrapper>, hugetlb: &mut HugePageBytes) -> Result<(), YaoiError> {
    let base_ptr = hugetlb.as_mut_ptr();

    println!("base_ptr = {:p}", base_ptr);
    
    let iovecs: [libc::iovec; 256] = core::array::from_fn(|x| {
        libc::iovec {
            iov_base: unsafe { base_ptr.add(8192 * x) } as *mut libc::c_void,
            iov_len: 8192,
            
        }
    });

    unsafe { bearer.io_uring().submitter().register_buffers(&iovecs) }
    .unwrap();

    Ok(())
}

#[inline]
fn register_buf_ring(bearer: &mut UringBearer<Wrapper>, hugetbl: &mut HugePageBytes, bgid: u16) -> Result<RingBufRegistered, YaoiError> {
    let buffer_count = BufferCount(unsafe { NonZero::new_unchecked(256) });
    let per_buffer_size = PerBufferSize(unsafe { NonZero::new_unchecked(8192) });
    let choice = RingBufChoice::with_default_pagesize(buffer_count, per_buffer_size).
        map_err(YaoiError::RingBuf)?;
    
    let unreg = unsafe { RingBufUnregistered::with_rawbuf_continuous(choice, hugetbl.as_mut_ptr()) }
    .map_err(YaoiError::RingBuf)?;
    let reg = unreg.register_with_bearer(bearer, bgid)
        .map_err(YaoiError::RingBuf)?;
    Ok(reg)
}

impl EntHugeTlb {
    #[inline]
    pub(crate) fn recv_multi(&mut self, fixed_fd: u32, bgid: u16, bearer: &mut UringBearer<Wrapper>) -> Result<usize, YaoiError> {
        println!("Added RecvMulti with bgid<{bgid}>");
        Ok(bearer.add_recv_multi(fixed_fd, bgid, None)
           .map_err(YaoiError::Bearer)?)
    }
    // TODO phasing / limiting
    #[inline]
    pub(crate) fn send_all_out(&mut self, fixed_fd: u32, bearer: &mut UringBearer<Wrapper>) -> Result<usize, YaoiError> {
        // TOOD: guard u32 - can it overflow?
        let total_out_len = self.cursor.pos_out_end - self.cursor.pos_out_start;

        println!("send_all_out = {}", total_out_len);
        
        if total_out_len == 0 {
            return Ok(0);
        }
        let mut buf_id = (self.cursor.pos_out_start / 8192) as u16; // TODO fixed bufsize
        let mut buf_idx = self.cursor.idx_out_cur;

        let chunks = total_out_len.div_ceil(8192); // TODO fixed bufsize
        for chunk_no in 0..chunks {
            let chunk_len = if chunk_no == 0 {
                total_out_len
            }
            else {
                8192
            };

            println!("chunk no = {}, buf_id = {}, len = {}", chunk_no, buf_id, chunk_len);
            
            let indexed_ptr = if buf_id == 0 {
                self.hugetbl_out.as_mut_ptr() as _
            }
            else {
                let indexed_start = buf_id as usize * 8192;
                unsafe { self.hugetbl_out.as_mut_ptr().add(indexed_start) as _ }
            };
            
            let buf_ref = self.cursor.pos_out_start;
            // SAFETY: We hold the underlying fixed raw buffer valid as long as it is needed
            let sid = unsafe {
                bearer.add_send_zc_rawbuf(
                    fixed_fd,
                    indexed_ptr,
                    chunk_len as u32,
                    None, // Some(buf_id),
                    buf_ref,
                    None, // DestTo
                    None, // SubmisionFlags
                )
            }
            // TODO: what if it's only one chunk failing? granular error handling
            .map_err(YaoiError::Bearer)?;

            if buf_id == 255 {
                self.cursor.idx_out_cur = 0;
                self.cursor.pos_out_start = 0;
                self.cursor.pos_out_end = 0;
                return Ok(chunks);
            }
            
            buf_id += 1;
            
            self.cursor.pos_out_start = buf_id as usize * 8192;
            if self.cursor.pos_out_end < self.cursor.pos_out_start {
                self.cursor.pos_out_end = self.cursor.pos_out_start;
            }
        }
        self.cursor.idx_out_cur = buf_id;
        
        Ok(chunks)
    }
    #[inline]
    pub(crate) fn try_free_buffers(&mut self, bearer: &mut UringBearer<Wrapper>) -> Result<(), YaoiError> {
        //let cur_buf_id = self.cursor.pos_in_start / 8192;

        let cur_buf_id = self.cursor.pos_in_start / 8192;

        // revolved over - leftover data within last buffer
        if cur_buf_id == 0 && self.cursor.idx_in_reused == 255 {
            unsafe { self.ring_in.dropping_bid(256) };
            self.cursor.idx_in_reused = 0;
            return Ok(());                
        }
        
        // revolved over - no leftover data
        if cur_buf_id == 0 && self.cursor.idx_in_reused == 256 {
            self.cursor.idx_in_reused = 0;
            return Ok(());
        }
        if self.cursor.idx_in_reused as usize > cur_buf_id {
            panic!("! self.cursor.idx_in_reused == {} > cur_buf_id == {}", self.cursor.idx_in_reused, cur_buf_id);
        }
        
        let gap = cur_buf_id - self.cursor.idx_in_reused as usize;
        
        if cur_buf_id > 0 && gap > 0 {
            let last_reused = self.cursor.idx_in_reused;
            let cur_tail = self.ring_in.cur_tail();
            for do_free in self.cursor.idx_in_reused .. cur_buf_id as u16 {
                unsafe { self.ring_in.dropping_bid(do_free) };
                self.cursor.idx_in_reused = do_free+1;
            }
            let cur_tail = self.ring_in.cur_tail();
            
        }
        Ok(())
    }
    #[inline]
    pub(crate) fn sent_zc(&mut self, fixed_fd: u32, sent_zc: &mut MapSentZc) -> Result<(), YaoiError> {
        // do nothing given we marked it sent pre-completion
        Ok(())
    }
    pub(crate) fn cb_recv_multi(&mut self, fixed_fd: u32, recv_multi: &mut MapRecvMulti) -> Result<(), YaoiError> {

        println!("cb_recv_multi fixed_fd={} recv_multi={:?}", fixed_fd, recv_multi);
        let recv_buf_start_pos = 8192_usize * recv_multi.buf_id as usize;

        let maybe_new_end_pos =  recv_multi.buf_len + self.cursor.pos_in_end;

        // Aim to update the latest confirmed end position
        if maybe_new_end_pos > self.cursor.pos_in_end {
            self.cursor.pos_in_end = maybe_new_end_pos;
            return Ok(());
        }
        // KTODO
        // If the previous end pos falls to last buffer and
        // the current updates the first buffer also check
        // whether there is revolution
        // (8192 * 256) - 8192 = 2088960 .. 2097152
        let rn_last_buf = 2088960 .. 2097152;
        if rn_last_buf.contains(&self.cursor.pos_in_end) && recv_multi.buf_id == 0 {
            panic!("Revolution detected? <first> cur_end = {}, recv_multi = {:?}", self.cursor.pos_in_end, recv_multi);
        }
        // TODO maybe it can come out of order what then ?        
        if rn_last_buf.contains(&self.cursor.pos_in_end) && recv_multi.buf_id == 0 {
             panic!("Revolution detected? <other> cur_end = {}, recv_multi = {:?}", self.cursor.pos_in_end, recv_multi);
        }

        panic!("We didn't update our end?! cur_end = {}, recv_multi = {:?}", self.cursor.pos_in_end, recv_multi);

        //Ok(())
    }
}

// I/O side is always left
impl Left for &mut EntHugeTlb {
    fn left_in_blocked(&self) -> bool {
        self.in_blocked
    }
    fn set_left_in_blocked(&mut self, nb: bool) -> () {
        self.in_blocked = nb;
    }
    fn left_lens(&self) -> (usize, usize) {
        let len_in = self.cursor.pos_in_end - self.cursor.pos_in_start;
        let len_out = self.cursor.pos_out_end - self.cursor.pos_out_start;
        (len_in, len_out)
    }
    fn left_set_lens(&mut self, new_len_in: usize, new_len_out: usize) -> () {
        let (cur_len_in, cur_len_out) = self.left_lens();

        if new_len_in + self.cursor.pos_in_start > self.hugetbl_in.capacity() {
            panic!("Setting Left In len over capacity."); // TODO: error (dep trait).
        }
        if new_len_out + self.cursor.pos_out_start > self.hugetbl_in.capacity() {
            panic!("Setting Left Out len over capacity."); // TODO: error (dep trait)
        }

        if new_len_in < cur_len_in {
            self.cursor.pos_in_start = self.cursor.pos_in_end - new_len_in;
        }
        if new_len_in > cur_len_in {
            self.cursor.pos_in_end = self.cursor.pos_in_start + new_len_in;
        }
        if new_len_out != cur_len_out {
            //            self.cursor.pos_out_end = self.cursor.pos_out_start + new_len_out;
            let buf_out_start = self.cursor.idx_out_cur as usize * 8192;
            self.cursor.pos_out_end = buf_out_start + new_len_out;            
        }
    }
    fn left_bufs_mut<'d>(&'d mut self) -> (InBuffer<'d>, &'d mut [u8]) {

        let buf_out = unsafe { self.hugetbl_out.as_slice_mut() };

        // revolve
        if self.cursor.pos_in_start > 2097152 && self.cursor.pos_in_end > 2097152 {
            self.cursor.pos_in_start -= 2097152;
            self.cursor.pos_in_end -= 2097152;
        }
        
        let range_inbuf = 0..2097152;
        let buf_in_brw = if self.cursor.pos_in_start <= 2097152 && self.cursor.pos_in_end > 2097152 {

            let buf_in1_len = 2097152 - self.cursor.pos_in_start;
            let buf_in2_len = self.cursor.pos_in_end - 2097152;
            let (buf_in1, buf_in2) = unsafe { self.hugetbl_in.as_slice_mut_disjointed_2_unchecked(
                self.cursor.pos_in_start, buf_in1_len,
                0, buf_in2_len
            ) };
            InBuffer::Double(buf_in1, buf_in2)
        }
        else {
            let buf_in = unsafe { self.hugetbl_in.as_slice_mut() };
            InBuffer::Single(&mut buf_in[self.cursor.pos_in_start..self.cursor.pos_in_end])
        };

        let buf_out_start = self.cursor.idx_out_cur as usize * 8192;
        let buf_out_brw = &mut buf_out[buf_out_start..];

        (buf_in_brw, buf_out_brw)
    }
    fn set_ready(&mut self, r: bool) -> bool {
        self.is_ready = r;
        self.is_ready
    }
    fn is_ready(&self) -> bool {
        self.is_ready
    }
    fn left_want_read(&self) -> bool {
        self.want_read
    }
    fn set_left_want_read(&mut self, w: bool) -> () {
        self.want_read = w;
    }    
    fn left_want_write(&self) -> bool {
        self.want_write
    }
    fn set_left_want_write(&mut self, w: bool) -> () {
        self.want_write = w;
    }
}

// Intermediate Buf
#[derive(Debug)]
struct IntermedBuf {
    buf_right_in: [u8; 8192],
    buf_right_in_len: usize,
    buf_right_out: [u8; 8192],
    buf_right_out_len: usize,
    left_ready: bool,
    left_want_read: bool,
    left_want_write: bool,
    left_blocked: bool,
    right_wants_next_in: bool,
}

impl Default for IntermedBuf {
    fn default() -> Self {
        Self {
            buf_right_in: [0u8; 8192],
            buf_right_in_len: 0,
            buf_right_out: [0u8; 8192],
            buf_right_out_len: 0,
            left_ready: false,
            left_want_read: false,
            left_want_write: false,
            left_blocked: false,
            right_wants_next_in: false,
        }
    }
}

impl Left for IntermedBuf {
    fn left_in_blocked(&self) -> bool {
        self.left_blocked
    }
    fn set_left_in_blocked(&mut self, nb: bool) -> () {
        self.left_blocked = nb;
    }
    fn left_lens(&self) -> (usize, usize) {
        (self.buf_right_in_len, self.buf_right_out_len)
    }
    fn left_set_lens(&mut self, len_in: usize, len_out: usize) -> () {
        self.buf_right_in_len = len_in;
        self.buf_right_out_len = len_out;
    }
    fn left_bufs_mut<'d>(&'d mut self) -> (InBuffer<'d>, &mut [u8]) {
        (InBuffer::Single(&mut self.buf_right_in[0..self.buf_right_in_len]), &mut self.buf_right_out)
    }
    fn set_ready(&mut self, r: bool) -> bool {
        self.left_ready = r;
        self.left_ready
    }
    fn is_ready(&self) -> bool {
        self.left_ready
    }
    fn left_want_read(&self) -> bool {
        self.left_want_read
    }
    fn set_left_want_read(&mut self, w: bool) -> () {
        self.left_want_read = w;
    }    
    fn left_want_write(&self) -> bool {
        self.left_want_write
    }
    fn set_left_want_write(&mut self, w: bool) -> () {
        self.left_want_write = w;
    }    
}

impl Right for IntermedBuf {
    fn right_lens(&self) -> (usize, usize) {
        (self.buf_right_in_len, self.buf_right_out_len)
    }
    fn buf_right_out(&self) -> &[u8] {
        &self.buf_right_out[0..self.buf_right_out_len]
    }
    fn wants_right_next_in(&self) -> bool {
        self.right_wants_next_in
    }
    fn set_wants_right_next_in(&mut self, s: bool) -> () {
        self.right_wants_next_in = s
    }
    fn all_sent_right_out(&mut self) -> () {
        self.buf_right_out_len = 0;
        self.buf_right_out = [0u8; 8192];
    }
    fn add_right_out(&mut self, bs: &[u8]) -> () {
        let start_pos = self.buf_right_out_len;
        let end_pos = self.buf_right_out_len + bs.len();
        self.buf_right_out[start_pos..end_pos].copy_from_slice(bs);
        self.buf_right_out_len += bs.len();
    }
    fn add_right_in(&mut self, bs: &[u8]) -> () {
        let start_pos = self.buf_right_in_len;
        let end_pos = self.buf_right_in_len + bs.len();
        self.buf_right_in[start_pos..end_pos].copy_from_slice(bs);
        self.buf_right_in_len += bs.len();
    }
}

// TODO: this works as long as clientpool ent is <= u16 - use-case limiattion for hugetbl incremental streaming
// TOOD nuke this...
#[inline]
fn _slot_u16_from_fixed_fd(try_fixed_fd: Option<u32>) -> Result<u16, YaoiError> {
    let test_in_range = 0u32..u16::MAX as u32;
    match try_fixed_fd {
        None => Err(YaoiError::HugeTlbReqFixedId),
        Some(out_range) if !test_in_range.contains(&out_range) => Err(YaoiError::LimitHugeTlbU16),
        Some(in_range) => Ok(in_range as u16),
    }
}

impl EntHugeTlb {
    /// From TcpStream with hugetlb's
    pub fn from_connected(
        bearer: &mut UringBearer<Wrapper>,
        connection: EntConnected,
        mut hugetbl_in: HugePageBytes,
        mut hugetbl_out: HugePageBytes,
    ) -> Result<Self, YaoiError> {

        // TODO: we should map buffers in u16 pool instead of doing fixed shenanigsna like this
        let bufgrp_idx_in = _slot_u16_from_fixed_fd(connection.fixed_fd())?;
        
        let ring_in = register_buf_ring(bearer, &mut hugetbl_in, bufgrp_idx_in)?;
        //register_out_bufs(bearer, &mut hugetbl_out)?;
        
        Ok(Self {
            connection,
            hugetbl_in,
            ring_in,
            hugetbl_out,
            cursor: CursorLeft::default(),
            is_ready: false,
            want_write: false,
            want_read: false,
            in_blocked: false,
        })
    }
    /// Borrow the underlying EntConnected
    #[inline]
    pub fn connection(&self) -> &EntConnected {
        &self.connection
    }
    /// Fixed fd if any
    #[inline]
    pub fn fixed_fd(&self) -> Option<u32> {
        self.connection.fixed_fd()
    }
    #[inline]
    pub fn left_wants_read(&self) -> bool {
        self.want_read
    }    
    #[inline]
    pub fn left_wants_write(&self) -> bool {
        self.want_write
    }
    /// Run blueprints through this Ent
    #[inline]
    pub fn run_blueprints<const Layers: usize, O: Orbit>(
        &mut self,
        bp: &mut Blueprints<Layers, O>,
    ) -> Result<(), YaoiError> {
        let count_layers = bp.count_all_layers();

        let mut intermed = IntermedBuf::default();
        let mut intermed_left = IntermedBuf::default();
        let mut intermed_right = IntermedBuf::default();

        struct NothingBurger;

        let mut cur_level = 0;

        if Layers == 0 {
            let mut left = self.impl_left();
            let app = bp.app_as_mut();
            let _pos =
                app.advance_with(&mut NothingBurger, &mut left, &mut NoRight);
            return Ok(());
        }

        loop {
            let _pos = match cur_level {
                0 => {
                    println!("------- Match: 0");
                    let mut left = self.impl_left();
                    let layers = bp.layers_as_mut(); 
                    let layer = &mut layers[cur_level];
                    let pre_lens = left.left_lens();
                    println!("Pre/is_ready<{}>, New lens = {:?}", left.is_ready(), pre_lens);
                    
                    let _pos =
                        layer.advance_with(&mut NothingBurger, &mut left, &mut intermed);
                    let lens = left.left_lens();


                    println!("Post/is_ready<{}>, is_blocked<{}> New lens = {:?}", left.is_ready(), left.left_in_blocked(), lens);

                    if lens.1 != 0 {
                        println!("Sending out..");
                        return Ok(());
                    }
                    
                    if left.is_ready() == false && !left.left_in_blocked() && pre_lens.0 != lens.0 {
                        println!("Left is not ready & not blocked with lens.0 changed w/ no output - looping..");
                        continue;
                    }
                    
                    if left.is_ready() == false {
                        return Ok(());
                    }
                    cur_level += 1;
                    println!(" ** Upgrade cur_level >> {cur_level}**");
                    _pos
                }
                // Terminal layer - App processing
                count_layers => {
                    println!("------- Match: Layers {cur_level}/{Layers}");

                    
                    let app = bp.app_as_mut();
                    let _pos = match cur_level {
                        1 => {
                            let pos = app.advance_with(&mut NothingBurger, &mut intermed, &mut NoRight);

                            let (left_len_in, left_len_out) = intermed.left_lens();
                            if left_len_in == 0 && left_len_out == 0 {
                                println!("No output at App, breaking.");
                                return Ok(());
                            }
                            cur_level = 0;
                            pos
                        },
                        _ => {
                            todo!();
                            app.advance_with(&mut NothingBurger, &mut intermed_left, &mut intermed_right);
                        },
                    };
                    _pos
                },
                _ => {
                    println!("------- Match: _default {cur_level}");
                    todo!();
                    let layers = bp.layers_as_mut();
                    let layer = &mut layers[cur_level];
                    let _pos = layer.advance_with(
                        &mut NothingBurger,
                        &mut intermed_left,
                        &mut intermed_right,
                    );
                    _pos
                }
            };
        }
        todo!()
    }
}
