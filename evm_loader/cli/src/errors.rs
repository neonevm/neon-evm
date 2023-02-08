//! Error types
#![allow(clippy::use_self)]

use log::error;
use solana_cli::cli::CliError as SolanaCliError;
use solana_client::client_error::ClientError as SolanaClientError;
use solana_client::tpu_client::TpuSenderError as SolanaTpuSenderError;
use solana_sdk::program_error::ProgramError as SolanaProgramError;
use solana_sdk::pubkey::{Pubkey, PubkeyError as SolanaPubkeyError};
use solana_sdk::signer::SignerError as SolanaSignerError;
use thiserror::Error;

use crate::commands::init_environment::EnvironmentError;

/// Errors that may be returned by the neon-cli program.
#[derive(Debug, Error)]
pub enum NeonCliError {
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
    /// too many steps
    #[error("Too many steps")]
    TooManySteps,

    /// Environment Error
    #[error("Environment error {0:?}")]
    EnvironmentError(EnvironmentError),

    /// Environment incomplete and should be corrected (some item missed or can be fixed)
    #[error("Incomplete environment")]
    IncompleteEnvironment,

    /// Environment in wrong state (some item in wrong state)
    #[error("Wrong environment")]
    WrongEnvironment,
}

impl NeonCliError {
    pub fn error_code(&self) -> i32 {
        match self {
            NeonCliError::IncompleteEnvironment => 50,
            NeonCliError::WrongEnvironment => 51,
            NeonCliError::EnvironmentError(_) => 52,
            NeonCliError::StdIoError(_) => 102,     // => 1002,
            NeonCliError::ProgramError(_) => 111,   // => 1011,
            NeonCliError::SignerError(_) => 112,    // => 1012,
            NeonCliError::ClientError(_) => 113,    // => 1013,
            NeonCliError::CliError(_) => 114,       // => 1014,
            NeonCliError::TpuSenderError(_) => 115, // => 1015,
            NeonCliError::PubkeyError(_) => 116,
            NeonCliError::EvmError(_) => 117,
            NeonCliError::EvmLoaderNotSpecified => 201, // => 4001,
            NeonCliError::KeypairNotSpecified => 202,   // => 4002,
            NeonCliError::IncorrectProgram(_) => 203,
            NeonCliError::AccountNotFound(_) => 205, // => 4005,
            NeonCliError::AccountIsNotBpf(_) => 226, // => 4026,
            NeonCliError::AccountIsNotUpgradeable(_) => 227, // => 4027,
            NeonCliError::AssociatedPdaNotFound(_, _) => 241, // => 4041,
            NeonCliError::InvalidAssociatedPda(_, _) => 242, // => 4042,
            NeonCliError::TooManySteps => 245,
        }
    }
}

impl From<EnvironmentError> for NeonCliError {
    fn from(e: EnvironmentError) -> NeonCliError {
        NeonCliError::EnvironmentError(e)
    }
}
