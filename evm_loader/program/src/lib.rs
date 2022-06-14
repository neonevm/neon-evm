//! # Neon EVM
//!
//! Neon EVM is an implementation of Ethereum Virtual Machine on Solana.
#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::module_name_repetitions, clippy::missing_const_for_fn, clippy::use_self)]
#![allow(missing_docs, clippy::missing_panics_doc, clippy::missing_errors_doc)]


mod allocator;
#[macro_use]
mod debug;
#[macro_use]
pub mod error;
pub mod account;
pub mod account_storage;
pub mod config;
pub mod config_macro;
pub mod entrypoint;
pub mod executor;
pub mod executor_state;
pub mod gasometer;
pub mod hamt;
pub mod instruction;
pub mod precompile_contracts;
pub mod query;
pub mod state_account;
pub mod transaction;
pub mod utils;

// Export current solana-sdk types for downstream users who may also be building with a different
// solana-sdk version
pub use solana_program;

#[cfg(feature = "tracing")]
pub use transaction::UnsignedTransaction;

//solana_sdk::declare_id!("EVM1111111111111111111111111111111111111111");

#[cfg(feature = "tracing")]
pub mod tracing;

#[cfg(feature = "tracing")]
macro_rules! event {
    ($x:expr) => {
        // use crate::tracing;
        // tracing::send(&$x);
        use solana_program::{tracer_api, compute_meter_remaining, compute_meter_set_remaining};

        let mut remaining: u64 = 0;
        compute_meter_remaining::compute_meter_remaining(&mut remaining);

        // let mut message = vec![];
        // bincode::serialize_into(&mut message, event).map_err(|e| E!(ProgramError::InvalidInstructionData; "Error={:?}", e)).unwrap();
        let ptr = &$x  as *const _ as *const u8;
        tracer_api::send_trace_message(ptr);
        // solana_program::msg!("{}", remaining);
        compute_meter_set_remaining::compute_meter_set_remaining(remaining + 12);

    };
}

#[cfg(feature = "tracing")]
macro_rules! emit_exit {
    ($reason:expr) => {{
        use evm::tracing::{EventOnStack::*, ExitTrace};
        let reason = $reason;
        event!(Exit(ExitTrace {
            reason: reason.clone().into(),
            return_value: vec![].as_slice(),
            return_value_len: 0,
        }));
        reason
    }};
    ($return_value:expr, $reason:expr) => {{
        let reason = $reason;
        let return_value = $return_value;
        event!((ExitTrace {
            reason: reason.clone(),
            return_value: return_value.as_slice(),
            return_value_len: return_value.len()
        }));
        (return_value, reason)
    }};
}

#[cfg(not(feature = "tracing"))]
macro_rules! emit_exit {
    ($reason:expr) => {
        $reason
    };
    ($return_value:expr, $reason:expr) => {
        ($return_value, $reason)
    };
}

#[cfg(not(feature = "tracing"))]
macro_rules! event {
    ($x:expr) => {};
}

pub(crate) use emit_exit;
pub(crate) use event;
