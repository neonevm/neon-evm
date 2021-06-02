//! Error types

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
        ProgramError::Custom(e as u32)
    }
}

impl<T> DecodeError<T> for EvmLoaderError {
    fn type_of() -> &'static str {
        "EVMLoaderError"
    }
}