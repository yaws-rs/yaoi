//! Internal Capacity translation types translating public API capacities to upstream required.

mod tcp_listener;
pub(crate) use tcp_listener::TcpListenerCapacity;

mod tcp_pool;
pub(crate) use tcp_pool::TcpPoolCapacity;
