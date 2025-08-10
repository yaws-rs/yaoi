//! Completion Maps for Type Conversions

// Usages:
// 1: TcpPool (Client) creates TcpStream upon Connect completion
#[derive(Clone, Debug, Default)]
pub(crate) struct MapConnected {
    pub(crate) fixed_fd: u32,
    pub(crate) result: i32,
}
