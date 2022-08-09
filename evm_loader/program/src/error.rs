//! Error types
#![allow(clippy::use_self)]

use solana_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

/// Errors that may be returned by the EVM Loader program.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EvmLoaderError {
    /// Unknown Error.
    #[error("Unknown error. Attention required.")]
    UnknownError,
    /// Storage Account is uninitialized.
    #[error("Storage Account is uninitialized")]
    StorageAccountUninitialized,
    /// Exclusive access to the account is not available
    #[error("Exclusive access to the account is not available")]
    ExclusiveAccessUnvailable,
    /// Operator is not authorized
    #[error("Operator is not authorized")]
    UnauthorizedOperator,
    #[error("Storage Account is finalized")]
    StorageAccountFinalized,
}

impl From<EvmLoaderError> for ProgramError {
    fn from(e: EvmLoaderError) -> Self {
        Self::Custom(e as u32)
    }
}

impl<T> DecodeError<T> for EvmLoaderError {
    fn type_of() -> &'static str {
        "EVMLoaderError"
    }
}


/// Macro to log a `ProgramError` in the current transaction log
/// with the source file position like: file.rc:42
/// and additional info if needed
/// See `https://github.com/neonlabsorg/neon-evm/issues/159`
///
/// # Examples
///
/// ```
/// #    return Err!(ProgramError::InvalidArgument; "Caller pubkey: {} ", &caller_info.key.to_string());
/// ```
///
macro_rules! Err {
    ( $n:expr; $($args:expr),* ) => ({
        #[cfg(target_arch = "bpf")]
        solana_program::msg!("{}:{} : {}", file!(), line!(), &format!($($args),*));

        #[cfg(not(target_arch = "bpf"))]
        log::error!("{}", &format!($($args),*));

        Err($n)
    });
}


/// Macro to log a `ProgramError` in the current transaction log.
/// with the source file position like: file.rc:777
/// and additional info if needed
/// See `https://github.com/neonlabsorg/neon-evm/issues/159`
///
/// # Examples
///
/// ```
/// #    map_err(|s| E!(ProgramError::InvalidArgument; "s={:?}", s))
/// ```
///
macro_rules! E {
    ( $n:expr; $($args:expr),* ) => ({
        #[cfg(target_arch = "bpf")]
        solana_program::msg!("{}:{} : {}", file!(), line!(), &format!($($args),*));

        #[cfg(not(target_arch = "bpf"))]
        log::error!("{}", &format!($($args),*));

        $n
    });
}
