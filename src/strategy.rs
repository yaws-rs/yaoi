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
    Replenish(u32, StrategyRegister, u32),
    /// Send a single Multi-shot Accept request without needing to replenish the queue.
    /// Best used when the source address and / or port is not required that would require a separate syscall to obtain.
    Multi(StrategyRegister, u32),
}

impl StrategyListener {
    /// Construct new TcpListener Strategy using Replenishing (of Single-Shots) strategy
    pub fn replenishing(flux_count: u32) -> NeedRegisterStrategy {
        NeedRegisterStrategy::Replenish(flux_count)
    }
    /// Construct new TcpListener Strategy using Multi-Shot Accept strategy    
    pub fn multi() -> NeedRegisterStrategy {
        NeedRegisterStrategy::Multi
    }
    /// Current bounded pool capacity
    pub fn cap_pool(&self) -> u32 {
        match self {
            Self::Replenish(_, _, cap_pool) => *cap_pool,
            Self::Multi(_, cap_pool) => *cap_pool,
        }
    }
    /// Current fixed capacity - zero means not fixed
    pub fn cap_fixed(&self) -> u32 {
        let strat_reg = match self {
            Self::Replenish(_, ref strat_reg, _) => strat_reg,
            Self::Multi(ref strat_reg, _) => strat_reg,
        };
        match strat_reg {
            StrategyRegister::Fixed(r) => *r,
            StrategyRegister::Regular => 0,
        }
    }
}

/// We need to understand how to register Filehandles, whether io_uring Fixed or Regular non io_uring mapped Fds.
pub enum NeedRegisterStrategy {
    Replenish(u32),
    Multi,
}

impl NeedRegisterStrategy {
    /// All Accepted sockets are registered with regular filehandles that are not mapped fixed into io_uring.
    pub fn regular_fds(&self) -> NeedCapBounded {
        match self {
            Self::Replenish(c) => NeedCapBounded(StrategyListener::Replenish(
                *c,
                StrategyRegister::Regular,
                0,
            )),
            Self::Multi => NeedCapBounded(StrategyListener::Multi(StrategyRegister::Regular, 0)),
        }
    }
    /// All Accepted sockets are registered as fixed filehandles that are mapped directly in io_uring.
    pub fn fixed_fds(&self, fixed_fd_capacity: u32) -> StrategyListener {
        match self {
            Self::Replenish(c) => StrategyListener::Replenish(
                *c,
                StrategyRegister::Fixed(fixed_fd_capacity),
                fixed_fd_capacity,
            ),
            Self::Multi => StrategyListener::Multi(
                StrategyRegister::Fixed(fixed_fd_capacity),
                fixed_fd_capacity,
            ),
        }
    }
}

pub struct NeedCapBounded(StrategyListener);

impl NeedCapBounded {
    /// In case of Regular Fds we need to supply the bounded capacity (concurent connections) for the listener pool
    pub fn pool_capacity(mut self, pool_cap: u32) -> StrategyListener {
        match self.0 {
            StrategyListener::Replenish(replenish_c, strat_reg, _) => {
                StrategyListener::Replenish(replenish_c, strat_reg, pool_cap)
            }
            StrategyListener::Multi(strat_reg, _) => StrategyListener::Multi(strat_reg, pool_cap),
        }
    }
}
