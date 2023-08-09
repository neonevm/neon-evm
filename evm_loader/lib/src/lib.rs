pub mod account_storage;
pub mod commands;
pub mod config;
pub mod context;
pub mod errors;
pub mod rpc;
pub mod syscall_stubs;
pub mod types;

pub use config::Config;
pub use context::Context;
pub use errors::NeonError;

pub type NeonResult<T> = Result<T, NeonError>;
