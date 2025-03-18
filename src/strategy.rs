//! Yaoi Strategies

#[derive(Debug)]
pub enum StrategyRegister {
    /// Appropriate Regular non-Fixed filehandles which are not registered with io_uring.
    Regular,
    /// Appropriate Fixed filehandles registered with io_uring.
    Fixed,
}

/// TcpListener Strategies
#[derive(Debug)]
pub enum StrategyListener {
    /// Replenish single-shot Accept requests upto q_count capacity.
    /// Best used when the source address and / or port is required without requiring a separate syscall to obtain.
    Replenish(StrategyRegister),
    /// Send a single Multi-shot Accept request without needing to replenish the queue.
    /// Best used when the source address and / or port is not required that would require a separate syscall to obtain.
    Multi(StrategyRegister),
}
