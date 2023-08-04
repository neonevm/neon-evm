//! Error types
#![allow(clippy::use_self)]

use std::net::AddrParseError;

use log::error;
use solana_cli::cli::CliError as SolanaCliError;
use solana_client::client_error::ClientError as SolanaClientError;
use solana_client::tpu_client::TpuSenderError as SolanaTpuSenderError;
use solana_sdk::program_error::ProgramError as SolanaProgramError;
use solana_sdk::pubkey::{Pubkey, PubkeyError as SolanaPubkeyError};
use solana_sdk::signer::SignerError as SolanaSignerError;
use thiserror::Error;

use crate::commands::init_environment::EnvironmentError;
use crate::types::PgError;

/// Errors that may be returned by the neon-cli program.
#[derive(Debug, Error)]
pub enum NeonError {
    /// Std IO Error
    #[error("Std I/O error. {0:?}")]
    StdIoError(#[from] std::io::Error),
    /// Solana Client Error
    #[error("Solana program error. {0:?}")]
    ProgramError(#[from] SolanaProgramError),
    /// Solana Client Error
    #[error("Solana client error. {0:?}")]
    ClientError(#[from] SolanaClientError),
    /// Solana Signer Error
    #[error("Solana signer error. {0:?}")]
    SignerError(#[from] SolanaSignerError),
    /// Solana Cli Error
    #[error("Solana CLI error. {0:?}")]
    CliError(#[from] SolanaCliError),
    /// TPU Sender Error
    #[error("TPU sender error. {0:?}")]
    TpuSenderError(#[from] SolanaTpuSenderError),
    /// Pubkey Error
    #[error("Pubkey Error. {0:?}")]
    PubkeyError(#[from] SolanaPubkeyError),
    /// EVM Loader Error
    #[error("EVM Error. {0}")]
    EvmError(#[from] evm_loader::error::Error),
    /// Need specify evm_loader
    #[error("EVM loader must be specified.")]
    EvmLoaderNotSpecified,
    /// Need specify fee payer
    #[error("Keypair must be specified.")]
    KeypairNotSpecified,
    /// Incorrect program
    #[error("Incorrect program {0:?}")]
    IncorrectProgram(Pubkey),
    #[error("Account not found {0:?}.")]
    AccountNotFound(Pubkey),
    /// Account is not BFP
    #[error("Account is not BPF {0:?}.")]
    AccountIsNotBpf(Pubkey),
    /// Account is not upgradeable
    #[error("Account is not upgradeable {0:?}.")]
    AccountIsNotUpgradeable(Pubkey),
    /// Program data account not found
    #[error("Associated PDA not found {0:?} for Program {1:?}.")]
    AssociatedPdaNotFound(Pubkey, Pubkey),
    /// Program data account not found
    #[error("Invalid Associated PDA {0:?} for Program {1:?}.")]
    InvalidAssociatedPda(Pubkey, Pubkey),
    #[error("")]
    InvalidChDbConfig,
    /// too many steps
    #[error("Too many steps")]
    TooManySteps,
    #[error("Incorrect address {0:?}.")]
    IncorrectAddress(String),
    #[error("Incorrect index {0:?}.")]
    IncorrectIndex(String),
    #[error("Tx parameters parsing error {0:?}.")]
    TxParametersParsingError(String),
    #[error("AddrParseError. {0:?}")]
    AddrParseError(#[from] AddrParseError),
    #[error("AxumError. {0:?}")]
    AxumError(#[from] axum::Error),
    #[error("SolanaClientError. {0:?}")]
    SolanaClientError(solana_client::client_error::ClientError),
    /// Environment Error
    #[error("Environment error {0:?}")]
    EnvironmentError(#[from] EnvironmentError),
    /// Environment incomplete and should be corrected (some item missed or can be fixed)
    #[error("Incomplete environment")]
    IncompleteEnvironment,
    /// Environment in wrong state (some item in wrong state)
    #[error("Wrong environment")]
    WrongEnvironment,
    #[error("Hex Error. {0}")]
    FromHexError(#[from] hex::FromHexError),
    #[error("PostgreSQL Error: {0}")]
    PostgreError(#[from] PgError),
    #[error("Panic: {0}")]
    Panic(String),
}

impl NeonError {
    pub fn error_code(&self) -> i32 {
        match self {
            NeonError::IncompleteEnvironment => 50,
            NeonError::WrongEnvironment => 51,
            NeonError::EnvironmentError(_) => 52,
            NeonError::Panic(_) => 101,
            NeonError::StdIoError(_) => 102,
            NeonError::ProgramError(_) => 111,
            NeonError::SignerError(_) => 112,
            NeonError::ClientError(_) => 113,
            NeonError::CliError(_) => 114,
            NeonError::TpuSenderError(_) => 115,
            NeonError::PubkeyError(_) => 116,
            NeonError::EvmError(_) => 117,
            NeonError::AddrParseError(_) => 118,
            NeonError::AxumError(_) => 119,
            NeonError::SolanaClientError(_) => 120,
            NeonError::EvmLoaderNotSpecified => 201,
            NeonError::KeypairNotSpecified => 202,
            NeonError::IncorrectProgram(_) => 203,
            NeonError::AccountNotFound(_) => 205,
            NeonError::AccountIsNotBpf(_) => 226,
            NeonError::AccountIsNotUpgradeable(_) => 227,
            NeonError::AssociatedPdaNotFound(_, _) => 241,
            NeonError::InvalidAssociatedPda(_, _) => 242,
            NeonError::TooManySteps => 245,
            NeonError::FromHexError(_) => 246,
            NeonError::InvalidChDbConfig => 247,
            NeonError::IncorrectAddress(_) => 248,
            NeonError::IncorrectIndex(_) => 249,
            NeonError::TxParametersParsingError(_) => 250,
            NeonError::PostgreError(_) => 251,
        }
    }
}

#[derive(Debug, Error)]
pub enum NeonAPIError {
    /// Std IO Error
    #[error("Std I/O error. {0:?}")]
    StdIoError(#[from] std::io::Error),
}
