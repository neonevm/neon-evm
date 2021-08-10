//! # Neon EVM
//!
//! Neon EVM is an implementation of Ethereum Virtual Machine on Solana.

#![deny(missing_docs)]
#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::module_name_repetitions)]

#[macro_use]
mod debug;
mod error;
pub mod entrypoint;
mod hamt;
pub mod solana_backend;
pub mod account_data;
pub mod account_storage;
pub mod solidity_account;
mod storage_account;
pub mod instruction;
mod transaction;
pub mod executor;
pub mod executor_state;
pub mod utils;
pub mod token;

// Export current solana-sdk types for downstream users who may also be building with a different
// solana-sdk version
pub use solana_program;

//solana_sdk::declare_id!("EVM1111111111111111111111111111111111111111");
