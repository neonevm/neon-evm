//! Error types
#![allow(clippy::use_self)]
#![allow(clippy::cast_possible_wrap)]

use std::process::exit;
use log::{ error };

use evm::{ H160, U256 };

use solana_sdk::pubkey::Pubkey;
use solana_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

/// Errors that may be returned by the neon-cli program.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum NeonCliError {
    /// Need specify evm_loader
    #[error("EVM loader must be specified")]
    EvmLoaderNotSpecified,
    /// Need specify fee payer
    #[error("Fee payer must be specified")]
    FeePayerNotSpecified,
    /// Changes of incorrect account were found
    #[error("Incorrect account at address {0:?}")]
    IncorrectAccount(H160),
    /// Account is uninitialized.
    #[error("Uninitialized account  account={0:?}, code_account={1:?}")]
    AccountUninitialized(Pubkey,Pubkey),
    /// Account is already initialized.
    #[error("Account is already initialized  account={0:?}, code_account={1:?}")]
    AccountAlreadyInitialized(Pubkey,Pubkey),
    /// Changes to the storage can only be applied to the contract account
    #[error("Contract account expected at address {0:?}")]
    ContractAccountExpected(H160),
    /// Deploy to existing account.
    #[error("Attempt to deploy to existing account at address {0:?}")]
    DeploymentToExistingAccount(H160),
    /// Convert nonce error
    #[error("Nonce conversion error {0:?}")]
    ConvertNonceError(U256),
    /// Invalid message verbosity
    #[error("Invalid verbosity message")]
    InvalidVerbosityMessage,
    /// Unknown Error.
    #[error("Unknown error")]
    UnknownError,
}

impl NeonCliError {
    pub fn error_code(&self) -> u32 {
        match self {
            NeonCliError::EvmLoaderNotSpecified             => 4001,
            NeonCliError::FeePayerNotSpecified              => 4002,
            NeonCliError::IncorrectAccount(_)               => 4010,
            NeonCliError::AccountUninitialized(_,_)         => 4011,
            NeonCliError::AccountAlreadyInitialized(_,_)    => 4012,
            NeonCliError::ContractAccountExpected(_)        => 4015,
            NeonCliError::DeploymentToExistingAccount(_)    => 4021,
            NeonCliError::ConvertNonceError(_)              => 4030,
            NeonCliError::InvalidVerbosityMessage           => 4100,
            NeonCliError::UnknownError                      => 4900,
        }
    }
    pub fn report_and_exit(self) {
        error!("Emulator Error: {}", &self);
        exit(self.error_code() as i32);
    }
}

impl From<NeonCliError> for ProgramError {
    fn from(e: NeonCliError) -> Self {
        Self::Custom(e.error_code())
    }
}

impl<T> DecodeError<T> for NeonCliError {
    fn type_of() -> &'static str {
        "NeonCliError"
    }
}
