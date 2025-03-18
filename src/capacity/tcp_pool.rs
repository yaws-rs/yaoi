//! Translate Yaoi Capacities for TcpPool

use io_uring_bearer::BearerCapacityKind;
use capacity::{Capacity, Setting};

#[derive(Clone, Debug)]
pub(crate) struct TcpPoolCapacity {
    pool_count: usize,
}

impl Setting<BearerCapacityKind> for TcpPoolCapacity {
    fn setting(&self, v: &BearerCapacityKind) -> usize {
        match v {
            // Assume every TcpPool item requires 3* queue capacity
            BearerCapacityKind::CoreQueue => self.pool_count * 3,
            // Assume every TcpPool item requires one RegisteredFd            
            BearerCapacityKind::RegisteredFd => self.pool_count,
            // Assume every TcpPool item requires 3* pending completions capacity
            BearerCapacityKind::PendingCompletions => self.pool_count * 3,
            // TODO: how do we determine this? just enough I guess? provide a knob via API probably best?
            BearerCapacityKind::Buffers => self.pool_count * 3,
            BearerCapacityKind::Futexes => 0,
        }
    }
}

impl TcpPoolCapacity {
    pub(crate) fn provide(pool_count: usize) -> Capacity::<TcpPoolCapacity, BearerCapacityKind> {
        Capacity::<TcpPoolCapacity, BearerCapacityKind>::with_planned(TcpPoolCapacity { pool_count })
    }
}
