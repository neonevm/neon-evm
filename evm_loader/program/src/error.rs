//! Error types
#![allow(clippy::use_self)]

use num_derive::FromPrimitive;
use solana_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

/// Errors that may be returned by the EVM Loader program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum EvmLoaderError {
    /// Unknown Error.
    #[error("Unknown error. Attention required.")]
    UnknownError,
    /// Storage Account is uninitialized.
    #[error("Storage Account is uninitialized")]
    StorageAccountUninitialized,
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

/// Function for macro Err! to log an err.
pub fn err_fn_without_info<T:>(err: ProgramError, fl: &str, ln: u32) -> Result<T, ProgramError> {
    solana_program::msg!("{}:{:?}", fl, ln);
    Err(err)
}

/// Function for macro Err! to log an err and add additional info.
pub fn err_fn<T:>(err: ProgramError, fl: &str, ln: u32, info: &str) -> Result<T, ProgramError> {
    solana_program::msg!("{}:{:?} : {}", fl, ln, info);
    Err(err)
}

/// Macro to log a ProgramError in the current transaction log
/// with the source file position like: file.rc:42
/// and additional info if needed
/// See https://github.com/neonlabsorg/neon-evm/issues/159
///
/// # Examples
///
/// ```
/// #    return Err!(ProgramError::InvalidArgument; "Caller pubkey: {} ", &caller_info.key.to_string());
/// ```
///
macro_rules! Err {
    ( $n:expr; $($args:expr),* ) => ( crate::error::err_fn($n, file!(), line!(), &format!($($args),*)) );
    ( $n:expr ) => ( crate::error::err_fn_without_info($n, file!(), line!()) )
}

/// Function for macro E! to log an err.
pub fn e_fn_without_info(e: ProgramError, fl: &str, ln: u32) -> ProgramError {
    solana_program::msg!("{}:{:?}", fl, ln);
    e
}

/// Function for macro E! to log an err and add additional info.
pub fn e_fn(e: ProgramError, fl: &str, ln: u32, info: &str) -> ProgramError {
    solana_program::msg!("{}:{:?} : {}", fl, ln, info);
    e
}

/// Macro to log a ProgramError in the current transaction log.
/// with the source file position like: file.rc:777
/// and additional info if needed
/// See https://github.com/neonlabsorg/neon-evm/issues/159
///
/// # Examples
///
/// ```
/// #    map_err(|s| E!(ProgramError::InvalidArgument; "s={:?}", s))
/// ```
///
macro_rules! E {
    ( $n:expr; $($args:expr),* ) => ( crate::error::e_fn($n, file!(), line!(), &format!($($args),*)) );
    ( $n:expr ) => ( crate::error::e_fn_without_info($n, file!(), line!()) )
}
