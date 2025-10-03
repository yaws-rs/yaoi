//! Yaoi TcpStream Streaming HugeTlb entity

use super::EntConnected;

use hugepage::HugePageBytes;

use crate::Blueprints;
use crate::YaoiError;
use blueprint::Orbit;
use blueprint::{Left, NoRight, Right};

/// Yaoi HugeTlb TcpStream
#[derive(Debug)]
pub struct EntHugeTlb {
    connection: EntConnected,
    hugetlb_out: HugePageBytes,
    hugetlb_in: HugePageBytes,
    cursor: CursorLeft,
    is_ready: bool,
}

#[derive(Debug, Default)]
struct CursorLeft {
    pos_in_start: usize,
    pos_in_end: usize,
    pos_out_start: usize,
    pos_out_end: usize,
}

impl EntHugeTlb {
    fn impl_left(&mut self) -> impl Left + use<'_> {
        self
    }
}

// I/O side is always left
impl Left for &mut EntHugeTlb {
    fn left_lens(&self) -> (usize, usize) {
        let len_in = self.cursor.pos_in_end - self.cursor.pos_in_start;
        let len_out = self.cursor.pos_out_end - self.cursor.pos_out_start;
        (len_in, len_out)
    }
    fn left_set_lens(&mut self, new_len_in: usize, new_len_out: usize) -> () {
        let (cur_len_in, cur_len_out) = self.left_lens();

        if new_len_in + self.cursor.pos_in_start >= self.hugetlb_in.capacity() {
            panic!("Setting Left In len over capacity."); // TODO: error (dep trait).
        }
        if new_len_out + self.cursor.pos_out_start >= self.hugetlb_in.capacity() {
            panic!("Setting Left Out len over capacity."); // TODO: error (dep trait)
        }

        if new_len_in != cur_len_in {
            self.cursor.pos_in_end = self.cursor.pos_in_start + new_len_in;
        }
        if new_len_out != cur_len_out {
            self.cursor.pos_out_end = self.cursor.pos_out_start + new_len_out;
        }
    }
    fn left_bufs_mut(&mut self) -> (&mut [u8], &mut [u8]) {
        let buf_in = self.hugetlb_in.as_slice_mut();
        let buf_out = self.hugetlb_out.as_slice_mut();

        let buf_in_brw = &mut buf_in[self.cursor.pos_in_start..self.cursor.pos_in_end];
        let buf_out_brw = &mut buf_out[self.cursor.pos_out_start..self.cursor.pos_out_end];

        (buf_in, buf_out)
    }
    fn set_ready(&mut self, r: bool) -> bool {
        self.is_ready = r;
        self.is_ready
    }
    fn is_ready(&self) -> bool {
        self.is_ready
    }
}

// Intermediate Left
#[derive(Debug)]
struct IntermedLeft {
    buf_in: [u8; 8192],
    buf_in_len: usize,
    buf_out: [u8; 8192],
    buf_out_len: usize,
    is_ready: bool,
}

impl Default for IntermedLeft {
    fn default() -> Self {
        Self {
            buf_in: [0u8; 8192],
            buf_in_len: 0,
            buf_out: [0u8; 8192],
            buf_out_len: 0,
            is_ready: false,
        }
    }
}

impl Left for IntermedLeft {
    fn left_lens(&self) -> (usize, usize) {
        (self.buf_in_len, self.buf_out_len)
    }
    fn left_set_lens(&mut self, len_in: usize, len_out: usize) -> () {
        self.buf_in_len = len_in;
        self.buf_out_len = len_out;
    }
    fn left_bufs_mut(&mut self) -> (&mut [u8], &mut [u8]) {
        (&mut self.buf_in[0..self.buf_in_len], &mut self.buf_out)
    }
    fn set_ready(&mut self, r: bool) -> bool {
        self.is_ready = r;
        self.is_ready
    }
    fn is_ready(&self) -> bool {
        self.is_ready
    }
}

// Intermediate Right
#[derive(Debug)]
struct IntermedRight {
    buf_in: [u8; 8192],
    buf_in_len: usize,
    buf_out: [u8; 8192],
    buf_out_len: usize,
    wants_right_next_in: bool,
}

impl Default for IntermedRight {
    fn default() -> Self {
        Self {
            buf_in: [0u8; 8192],
            buf_in_len: 0,
            buf_out: [0u8; 8192],
            buf_out_len: 0,
            wants_right_next_in: false,
        }
    }
}

impl Right for IntermedRight {
    fn out_len(&self) -> usize {
        self.buf_out_len
    }
    fn buf_right_out(&self) -> &[u8] {
        &self.buf_out[0..self.buf_out_len]
    }
    fn wants_right_next_in(&self) -> bool {
        self.wants_right_next_in
    }
    fn set_wants_right_next_in(&mut self, s: bool) -> () {
        self.wants_right_next_in = s
    }
    fn all_sent_right_out(&mut self) -> () {
        self.buf_out_len = 0;
        self.buf_out = [0u8; 8192];
    }
    fn add_right_out(&mut self, bs: &[u8]) -> () {
        let start_pos = self.buf_out_len;
        let end_pos = self.buf_out_len + bs.len();
        self.buf_out[start_pos..end_pos].copy_from_slice(bs);
        self.buf_out_len += bs.len();
    }
    fn add_right_in(&mut self, bs: &[u8]) -> () {
        let start_pos = self.buf_in_len;
        let end_pos = self.buf_in_len + bs.len();
        self.buf_in[start_pos..end_pos].copy_from_slice(bs);
        self.buf_in_len += bs.len();
    }
}

// Left side processing
#[derive(Debug)]
enum RotatingLeft<'ci> {
    Io(&'ci mut EntHugeTlb),
    Other(IntermedLeft),
}

impl EntHugeTlb {
    /// From TcpStream with hugetlb's
    pub fn from_connected(
        connection: EntConnected,
        hugetlb_in: HugePageBytes,
        hugetlb_out: HugePageBytes,
    ) -> Self {
        Self {
            connection,
            hugetlb_in,
            hugetlb_out,
            cursor: CursorLeft::default(),
            is_ready: false,
        }
    }
    /// Borrow the underlying EntConnected
    pub fn connection(&self) -> &EntConnected {
        &self.connection
    }
    /// Fixed fd if any
    pub fn fixed_fd(&self) -> Option<u32> {
        self.connection.fixed_fd()
    }
    /// Run blueprints through this Ent
    #[inline]
    pub fn run_blueprints<const Layers: usize, O: Orbit>(
        &mut self,
        bp: &mut Blueprints<Layers, O>,
    ) -> Result<(), YaoiError> {
        let layers = bp.layers_as_mut();

        let mut left = self.impl_left();
        let mut intermed_left = IntermedLeft::default();
        let mut intermed_right = IntermedRight::default();

        struct NothingBurger;

        let mut cur_level = 0;

        loop {
            let layer = &mut layers[cur_level];

            let _pos = match cur_level {
                0 => {
                    let _pos =
                        layer.advance_with(&mut NothingBurger, &mut left, &mut intermed_right);

                    if left.is_ready() == false {
                        break;
                    }

                    _pos
                }
                _ => {
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
