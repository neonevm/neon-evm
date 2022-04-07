//! Error types
#![allow(clippy::use_self)]
#![allow(clippy::cast_possible_wrap)]

use log::{ error };

use evm::{ H160, U256 };

use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::SignerError as SolanaSignerError;
use solana_program::{decode_error::DecodeError};
use solana_program::program_error::ProgramError as SolanaProgramError;
use solana_client::client_error::ClientError as SolanaClientError;
use solana_client::tpu_client::TpuSenderError as SolanaTpuSenderError;
use solana_cli::cli::CliError as SolanaCliError;
use thiserror::Error;


/// Errors that may be returned by the neon-cli program.
#[derive(Debug, Error)]
pub enum NeonCliError {
    /// Std IO Error
    #[error("Std I/O error. {0:?}")]
    StdIoError(std::io::Error),
    /// Solana Client Error
    #[error("Solana program error. {0:?}")]
    ProgramError(SolanaProgramError),
    /// Solana Client Error
    #[error("Solana client error. {0:?}")]
    ClientError(SolanaClientError),
    /// Solana Signer Error
    #[error("Solana signer error. {0:?}")]
    SignerError(SolanaSignerError),
    /// Solana Cli Error
    #[error("Solana CLI error. {0:?}")]
    CliError(SolanaCliError),
    /// TPU Sender Error
    #[error("TPU sender error. {0:?}")]
    TpuSenderError(SolanaTpuSenderError),
    /// Need specify evm_loader
    #[error("EVM loader must be specified.")]
    EvmLoaderNotSpecified,
    /// Need specify fee payer
    #[error("Fee payer must be specified.")]
    FeePayerNotSpecified,
    /// Account not found at address
    #[error("Account not found at address {0:?}.")]
    AccountNotFoundAtAddress(H160),
    /// Code account not found
    #[error("Code account not found at address {0:?}.")]
    CodeAccountNotFound(H160),
    /// Code account not found
    #[error("Code account required at address {0:?}.")]
    CodeAccountRequired(H160),
    /// Changes of incorrect account were found
    #[error("Incorrect account at address {0:?}.")]
    IncorrectAccount(H160),
    /// Account already exists
    #[error("Account already exists. {0:?}")]
    AccountAlreadyExists(Account),
    /// Account is already initialized.
    #[error("Account is already initialized.  account={0:?}, code_account={1:?}")]
    AccountAlreadyInitialized(Pubkey,Pubkey),
    /// Changes to the storage can only be applied to the contract account
    #[error("Contract account expected at address {0:?}.")]
    ContractAccountExpected(H160),
    /// Deploy to existing account.
    #[error("Attempt to deploy to existing account at address {0:?}.")]
    DeploymentToExistingAccount(H160),
    /// Invalid storage account owner
    #[error("Invalid storage account owner {0:?}.")]
    InvalidStorageAccountOwner(Pubkey),
    /// Storage account required
    #[error("Storage account required. {0:?}")]
    StorageAccountRequired(Account),
    /// Account incorrect type
    #[error("Account incorrect type. {0:?}")]
    AccountIncorrectType(Account),
    /// Account data too small
    #[error("Account data too small. account_data.len()={0:?} < end={1:?}")]
    AccountDataTooSmall(usize,usize),
    /// Account not found
    #[error("Account not found {0:?}.")]
    AccountNotFound(Pubkey),
    /// Account is not BFP
    #[error("Account is not BPF {0:?}.")]
    AccountIsNotBpf(Pubkey),
    /// Account is not upgradeable
    #[error("Account is not upgradeable {0:?}.")]
    AccountIsNotUpgradeable(Pubkey),
    /// Convert nonce error
    #[error("Nonce conversion error. {0:?}")]
    ConvertNonceError(U256),
    /// Program data account not found
    #[error("Associated PDA not found {0:?} for Program {1:?}.")]
    AssociatedPdaNotFound(Pubkey,Pubkey),
    /// Program data account not found
    #[error("Invalid Associated PDA {0:?} for Program {1:?}.")]
    InvalidAssociatedPda(Pubkey,Pubkey),
    /// Invalid message verbosity
    #[error("Invalid verbosity message.")]
    InvalidVerbosityMessage,
    /// Transaction failed
    #[error("Transaction failed.")]
    TransactionFailed,
    /// too many steps
    #[error("Too many steps")]
    TooManySteps,
    // Account nonce exceeds u64::max
    #[error("Transaction count overflow")]
    TrxCountOverflow,
    /// Unknown Error.
    #[error("Unknown error.")]
    UnknownError
}

impl NeonCliError {
    pub fn error_code(&self) -> u32 {
        match self {
            NeonCliError::StdIoError(_)                     => 102, // => 1002,
            NeonCliError::ProgramError(_)                   => 111, // => 1011,
            NeonCliError::SignerError(_)                    => 112, // => 1012,
            NeonCliError::ClientError(_)                    => 113, // => 1013,
            NeonCliError::CliError(_)                       => 114, // => 1014,
            NeonCliError::TpuSenderError(_)                 => 115, // => 1015,
            NeonCliError::EvmLoaderNotSpecified             => 201, // => 4001,
            NeonCliError::FeePayerNotSpecified              => 202, // => 4002,
            NeonCliError::AccountNotFound(_)                => 205, // => 4005,
            NeonCliError::AccountNotFoundAtAddress(_)       => 206, // => 4006,
            NeonCliError::CodeAccountNotFound(_)            => 207, // => 4007,
            NeonCliError::CodeAccountRequired(_)            => 208, // => 4008,
            NeonCliError::IncorrectAccount(_)               => 209, // => 4009,
            NeonCliError::AccountAlreadyExists(_)           => 210, // => 4010,
            NeonCliError::AccountAlreadyInitialized(_,_)    => 213, // => 4013,
            NeonCliError::ContractAccountExpected(_)        => 215, // => 4015,
            NeonCliError::DeploymentToExistingAccount(_)    => 221, // => 4021,
            NeonCliError::InvalidStorageAccountOwner(_)     => 222, // => 4022,
            NeonCliError::StorageAccountRequired(_)         => 223, // => 4023,
            NeonCliError::AccountIncorrectType(_)           => 224, // => 4024,
            NeonCliError::AccountDataTooSmall(_,_)          => 225, // => 4025,
            NeonCliError::AccountIsNotBpf(_)                => 226, // => 4026,
            NeonCliError::AccountIsNotUpgradeable(_)        => 227, // => 4027,
            NeonCliError::ConvertNonceError(_)              => 230, // => 4030,
            NeonCliError::AssociatedPdaNotFound(_,_)        => 241, // => 4041,
            NeonCliError::InvalidAssociatedPda(_,_)         => 242, // => 4042,
            NeonCliError::InvalidVerbosityMessage           => 243, // => 4100,
            NeonCliError::TransactionFailed                 => 244, // => 4200,
            NeonCliError::TooManySteps                      => 245,
            NeonCliError::TrxCountOverflow                  => 246,
            NeonCliError::UnknownError                      => 249, // => 4900,
        }
    }
}

impl From<std::io::Error> for NeonCliError {
    fn from(e: std::io::Error) -> NeonCliError {
        NeonCliError::StdIoError(e)
    }
}

impl From<SolanaClientError> for NeonCliError {
    fn from(e: SolanaClientError) -> NeonCliError {
        NeonCliError::ClientError(e)
    }
}

impl From<SolanaProgramError> for NeonCliError {
    fn from(e: SolanaProgramError) -> NeonCliError {
        NeonCliError::ProgramError(e)
    }
}

impl From<SolanaSignerError> for NeonCliError {
    fn from(e: SolanaSignerError) -> NeonCliError {
        NeonCliError::SignerError(e)
    }
}

impl From<SolanaCliError> for NeonCliError {
    fn from(e: SolanaCliError) -> NeonCliError {
        NeonCliError::CliError(e)
    }
}

impl From<SolanaTpuSenderError> for NeonCliError {
    fn from(e: SolanaTpuSenderError) -> NeonCliError {
        NeonCliError::TpuSenderError(e)
    }
}

impl From<NeonCliError> for SolanaProgramError {
    fn from(e: NeonCliError) -> Self {
        Self::Custom(e.error_code())
    }
}

impl<T> DecodeError<T> for NeonCliError {
    fn type_of() -> &'static str {
        "NeonCliError"
    }
}
