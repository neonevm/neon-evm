#![deny(warnings)]
#![deny(missing_docs)]
#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(deprecated)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::semicolon_if_nothing_returned)]

//! An ERC20-like Token program for the Solana blockchain
#[macro_use]
mod debug;
#[macro_use]
mod error;
pub mod macrorules;
pub mod config;
pub mod entrypoint;
/// hamt impl
pub mod hamt;
pub mod solana_backend;
pub mod account_data;
pub mod account_storage;
pub mod solidity_account;
mod storage_account;
pub mod instruction;
mod transaction;
/// Todo: document
pub mod executor;
/// Todo: document
pub mod executor_state;
pub mod utils;
pub mod operator;
pub mod payment;
pub mod token;
pub mod system;
pub mod precompile_contracts;

// Export current solana-sdk types for downstream users who may also be building with a different
// solana-sdk version
pub use solana_program;

//solana_sdk::declare_id!("EVM1111111111111111111111111111111111111111");
