//! Error types
#![allow(clippy::use_self)]

use std::process::exit;
use log::{ error };

use evm::{ U256 };

use solana_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

/// Errors that may be returned by the neon-cli program.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum NeonCliError {
    /// Need specify evm_loader
    #[error("need specify evm_loader")]
    EvmLoaderNotSpecified,
    /// Need specify fee payer
    #[error("need specify fee payer")]
    FeePayerNotSpecified,
    /// Changes of incorrect account were found
    #[error("Incorrect account")]
    IncorrectAccount,
    /// Account is uninitialized.
    #[error("Account is uninitialized")]
    AccountUninitialized,
    /// Account is already initialized.
    #[error("Account is already initialized")]
    AccountAlreadyInitialized,
    /// Changes to the storage can only be applied to the contract account
    #[error("A contract account is expected")]
    ContractAccountExpected,
    /// Deploy to existing account.
    #[error("Deploy to existing account")]
    DeploymentToExistingAccount,
    /// Convert nonce error
    #[error("convert nonce error")]
    ConvertNonceError(U256),
    /// Invalid message verbosity
    #[error("invalid message verbosity")]
    InvalidVerbosityMessage,
    /// Unknown Error.
    #[error("Unknown error")]
    UnknownError,
}

impl NeonCliError {
    pub fn error_code(&self) -> i32 {
        match self {
            NeonCliError::EvmLoaderNotSpecified       => 4001,
            NeonCliError::FeePayerNotSpecified        => 4002,
            NeonCliError::IncorrectAccount            => 4010,
            NeonCliError::AccountUninitialized        => 4011,
            NeonCliError::AccountAlreadyInitialized   => 4012,
            NeonCliError::ContractAccountExpected     => 4015,
            NeonCliError::DeploymentToExistingAccount => 4021,
            NeonCliError::ConvertNonceError(_)        => 4030,
            NeonCliError::InvalidVerbosityMessage     => 4100,
            NeonCliError::UnknownError                => 4900,
        }
    }
    pub fn report_and_exit(self) {
        error!("Emulator Error: {}", &self);
        exit(self.error_code());
    }
}

impl From<NeonCliError> for ProgramError {
    fn from(e: NeonCliError) -> Self {
        Self::Custom(e.error_code() as u32)
    }
}

impl<T> DecodeError<T> for NeonCliError {
    fn type_of() -> &'static str {
        "NeonCliError"
    }
}
