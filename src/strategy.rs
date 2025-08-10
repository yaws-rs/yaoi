//! Yaoi Strategies

#[derive(Debug)]
pub enum StrategyRegister {
    /// Appropriate Regular non-Fixed filehandles which are not registered with io_uring.
    Regular,
    /// Appropriate Fixed filehandles registered with io_uring.
    Fixed(u32),
}

/// TcpListener Strategies
#[derive(Debug)]
pub enum StrategyListener {
    /// Replenish single-shot Accept requests upto q_count capacity.
    /// Best used when the source address and / or port is required without requiring a separate syscall to obtain.
    Replenish(u32, StrategyRegister),
    /// Send a single Multi-shot Accept request without needing to replenish the queue.
    /// Best used when the source address and / or port is not required that would require a separate syscall to obtain.
    Multi(StrategyRegister),
}

impl StrategyListener {
    /// Construct new TcpListener Strategy using Replenishing (of Single-Shots) strategy
    pub fn replenishing(flux_count: u32) -> NeedRegisterStrategy {
        NeedRegisterStrategy::Replenishing(flux_count)
    }
    /// Construct new TcpListener Strategy using Multi-Shot Accept strategy    
    pub fn multi() -> NeedRegisterStrategy {
        NeedRegisterStrategy::Multi
    }
}

/// We need to understand how to register Filehandles, whether io_uring Fixed or Regular non io_uring mapped Fds.
pub enum NeedRegisterStrategy {
    Replenishing(u32),
    Multi,
}

impl NeedRegisterStrategy {
    /// All Accepted sockets are registered with regular filehandles that are not mapped fixed into io_uring.
    pub fn regular_fds(&self) -> StrategyListener {
        match self {
            Self::Replenishing(c) => StrategyListener::Replenish(*c, StrategyRegister::Regular),
            Self::Multi => StrategyListener::Multi(StrategyRegister::Regular),
        }
    }
    /// All Accepted sockets are registered as fixed filehandles that are mapped directly in io_uring.
    pub fn fixed_fds(&self, fixed_fd_capacity: u32) -> StrategyListener {
        match self {
            Self::Replenishing(c) => {
                StrategyListener::Replenish(*c, StrategyRegister::Fixed(fixed_fd_capacity))
            }
            Self::Multi => StrategyListener::Multi(StrategyRegister::Fixed(fixed_fd_capacity)),
        }
    }
}
