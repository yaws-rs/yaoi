#![warn(
    clippy::unwrap_used,
    missing_docs,
    rust_2018_idioms,
    unused_lifetimes,
    unused_qualifications
)]
#![doc = include_str!("../README.md")]
#![allow(unused_imports)]

//-----------------------------------------------
// Internal Macros
//-----------------------------------------------
#[macro_use]
pub(crate) mod macros;

//-----------------------------------------------
// Internal Types
//-----------------------------------------------
pub(crate) mod cmaps; // Conversion maps

//-----------------------------------------------
// All Errors
//-----------------------------------------------
pub mod error;

pub(crate) mod capacity;
pub mod strategy;

//-----------------------------------------------
// TCP Pools
//-----------------------------------------------
mod tcp_pool;
pub use tcp_pool::tcp_client_pool::TcpClientPool;

//-----------------------------------------------
// TcpStream
//-----------------------------------------------
mod tcp_stream;
pub use tcp_stream::TcpStream;

//-----------------------------------------------
// TcpListener
//-----------------------------------------------
mod tcp_listener;
pub use tcp_listener::TcpListener;

//-----------------------------------------------
// W/a until the Bearer generic <C> is addressed
//-----------------------------------------------
mod dummy;
pub(crate) use dummy::Dummy;
