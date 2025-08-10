//! Dummy - will fix the Genreic in the bearer some day.

use io_uring_opcode::OpCompletion;

#[derive(Clone, Debug)]
pub(crate) enum DummyError {}

#[derive(Clone, Debug)]
pub(crate) struct Dummy {}

impl OpCompletion for Dummy {
    type Error = DummyError;
    fn entry(&self) -> io_uring_bearer::io_uring::squeue::Entry {
        todo!()
    }
    fn owner(&self) -> io_uring_owner::Owner {
        todo!()
    }
    fn force_owner_kernel(&mut self) -> bool {
        todo!()
    }
}
