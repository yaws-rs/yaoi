//! Yaoi Capacities

use io_uring_bearer::BearerCapacityKind;
use capacity::{Capacity, Setting};

#[derive(Clone, Debug)]
pub(crate) struct TcpListenerCapacity {
    q_count: usize,
}

impl Setting<BearerCapacityKind> for TcpListenerCapacity {
    fn setting(&self, v: &BearerCapacityKind) -> usize {
        match v {
            BearerCapacityKind::CoreQueue => self.q_count,
            BearerCapacityKind::RegisteredFd => 1,
            BearerCapacityKind::PendingCompletions => self.q_count,
            BearerCapacityKind::Buffers => 0,
            BearerCapacityKind::Futexes => 0,
        }
    }
}

impl TcpListenerCapacity {
    pub(crate) fn provide(q_count: usize) -> Capacity::<TcpListenerCapacity, BearerCapacityKind> {
        Capacity::<TcpListenerCapacity, BearerCapacityKind>::with_planned(TcpListenerCapacity { q_count })
    }
}
