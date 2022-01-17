//! Error types
#![allow(clippy::use_self)]

use solana_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

/// Errors that may be returned by the neon-cli program.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum NeonCliError {
    /// No Error.
    #[error("no error")]
    NoError,
    /// Unknown Error.
    #[error("Unknown error")]
    UnknownError,
    /// Account is already initialized.
    #[error("Account is already initialized")]
    AccountAlreadyInitialized,
    /// Account is uninitialized.
    #[error("Account is uninitialized")]
    UninitializedAccount,
    /// Deploy to existing account.
    #[error("Deploy to existing account")]
    DeployToExistingAccount,
    /// Changes to the storage can only be applied to the contract account
    #[error("A contract account is expected")]
    ContractAccountIsExpected,
    /// Changes of incorrect account were found
    #[error("Incorrect account")]
    IncorrectAccount,
    /// Convert nonce error
    #[error("convert nonce error")]
    ConvertNonceError,
    /// Invalid message verbosity
    #[error("invalid message verbosity")]
    InvalidMessageVerbosity,
    /// Need specify evm_loader
    #[error("need specify evm_loader")]
    EvmLoaderNotSpecified,
    /// Need specify fee payer
    #[error("need specify fee payer")]
    FeePayerNotSpecified,
}

impl From<NeonCliError> for ProgramError {
    fn from(e: NeonCliError) -> Self {
        Self::Custom(e as u32)
    }
}

impl<T> DecodeError<T> for NeonCliError {
    fn type_of() -> &'static str {
        "NeonCliError"
    }
}
