//! Error types

use num_derive::FromPrimitive;
use solana_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

/// Errors that may be returned by the EVM Loader program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum EVMLoaderError {
    /// Storage Account is uninitialized.
    #[error("Storage Account is uninitialized")]
    StorageAccountUninitialized,
    /// SomeError.
    #[error("Some error")]
    SomeError,
}

impl From<EVMLoaderError> for ProgramError {
    fn from(e: EVMLoaderError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl<T> DecodeError<T> for EVMLoaderError {
    fn type_of() -> &'static str {
        "EVMLoaderError"
    }
}