//! Error types
#![allow(clippy::use_self)]

use std::{array::TryFromSliceError, num::TryFromIntError, str::Utf8Error};

use ethnum::U256;
use solana_program::{
    program_error::ProgramError,
    pubkey::{Pubkey, PubkeyError},
    secp256k1_recover::Secp256k1RecoverError,
};
use thiserror::Error;

use crate::types::Address;

/// Errors that may be returned by the EVM Loader program.
#[derive(Error, Debug)]
pub enum Error {
    #[error("Error: {0}")]
    Custom(String),

    #[error("Solana Program Error: {0}")]
    ProgramError(#[from] ProgramError),

    #[error("Solana Pubkey Error: {0}")]
    PubkeyError(#[from] PubkeyError),

    #[error("RLP error: {0}")]
    RlpError(#[from] rlp::DecoderError),

    #[error("Secp256k1 error: {0}")]
    Secp256k1Error(#[from] Secp256k1RecoverError),

    #[error("Bincode error: {0}")]
    BincodeError(#[from] bincode::Error),

    #[error("IO error: {0}")]
    BorshError(#[from] std::io::Error),

    #[error("FromHexError error: {0}")]
    FromHexError(#[from] hex::FromHexError),

    #[error("TryFromIntError error: {0}")]
    TryFromIntError(#[from] TryFromIntError),

    #[error("TryFromSliceError error: {0}")]
    TryFromSliceError(#[from] TryFromSliceError),

    #[error("Utf8Error error: {0}")]
    Utf8Error(#[from] Utf8Error),

    #[error("Account {0} - not found")]
    AccountMissing(Pubkey),

    #[error("Account {0} - blocked, trying to execute transaction on rw locked account")]
    AccountBlocked(Pubkey),

    #[error("Account {0} - was empty, created by another transaction")]
    AccountCreatedByAnotherTransaction(Pubkey),

    #[error("Account {0} - invalid tag, expected {1}")]
    AccountInvalidTag(Pubkey, u8),

    #[error("Account {0} - invalid owner, expected {1}")]
    AccountInvalidOwner(Pubkey, Pubkey),

    #[error("Account {0} - invalid public key, expected {1}")]
    AccountInvalidKey(Pubkey, Pubkey),

    #[error("Account {0} - invalid data")]
    AccountInvalidData(Pubkey),

    #[error("Account {0} - not writable")]
    AccountNotWritable(Pubkey),

    #[error("Account {0} - not signer")]
    AccountNotSigner(Pubkey),

    #[error("Account {0} - not rent exempt")]
    AccountNotRentExempt(Pubkey),

    #[error("Account {0} - already initialized")]
    AccountAlreadyInitialized(Pubkey),

    #[error("Account {0} - in legacy format")]
    AccountLegacy(Pubkey),

    #[error("Operator is not authorized")]
    UnauthorizedOperator,

    #[error("Storage Account is uninitialized")]
    StorageAccountUninitialized,

    #[error("Transaction already finalized")]
    StorageAccountFinalized,

    #[error("Unknown extension method selector {1:?}, contract {0}")]
    UnknownPrecompileMethodSelector(Address, [u8; 4]),

    #[error("Insufficient balance for transfer, account = {0}, chain = {1}, required = {2}")]
    InsufficientBalance(Address, u64, U256),

    #[error("Invalid token for transfer, account = {0}, chain = {1}")]
    InvalidTransferToken(Address, u64),

    #[error("Out of Gas, limit = {0}, required = {1}")]
    OutOfGas(U256, U256),

    #[error("Invalid gas balance account")]
    GasReceiverInvalidChainId,

    #[error("EVM Stack Overflow")]
    StackOverflow,

    #[error("EVM Stack Underflow")]
    StackUnderflow,

    #[error("EVM Push opcode out of bounds, contract = {0}")]
    PushOutOfBounds(Address),

    #[error("EVM Memory Access at offset = {0} with length = {1} is out of limits")]
    MemoryAccessOutOfLimits(usize, usize),

    #[error("EVM RETURNDATACOPY offset = {0} with length = {1} exceeds data size")]
    ReturnDataCopyOverflow(usize, usize),

    #[error("EVM static mode violation, contract = {0}")]
    StaticModeViolation(Address),

    #[error("EVM invalid jump destination = {1}, contract = {0}")]
    InvalidJump(Address, usize),

    #[error("EVM encountered invalid opcode, contract = {0}, opcode = {1:X}")]
    InvalidOpcode(Address, u8),

    #[error("EVM encountered unknown opcode, contract = {0}, opcode = {1:X}")]
    UnknownOpcode(Address, u8),

    #[error("Account {0} - nonce overflow")]
    NonceOverflow(Address),

    #[error("Invalid Nonce, origin {0} nonce {1} != Transaction nonce {2}")]
    InvalidTransactionNonce(Address, u64, u64),

    #[error("Invalid Chain ID {0}")]
    InvalidChainId(u64),

    #[error("Attempt to deploy to existing account {0}, caller = {1}")]
    DeployToExistingAccount(Address, Address),

    #[error("New contract code starting with the 0xEF byte (EIP-3541), contract = {0}")]
    EVMObjectFormatNotSupported(Address),

    #[error("New contract code size exceeds 24kb (EIP-170), contract = {0}, size = {1}")]
    ContractCodeSizeLimit(Address, usize),

    #[error("Transaction is rejected from a sender with deployed code (EIP-3607), contract = {0}")]
    SenderHasDeployedCode(Address),

    #[error("Checked Integer Math Overflow")]
    IntegerOverflow,

    #[error("Index out of bounds")]
    OutOfBounds,

    #[error("Holder Account - invalid owner {0}, expected = {1}")]
    HolderInvalidOwner(Pubkey, Pubkey),

    #[error("Holder Account - insufficient size {0}, required = {1}")]
    HolderInsufficientSize(usize, usize),

    #[error("Holder Account - invalid transaction hash {}, expected = {}", hex::encode(.0), hex::encode(.1))]
    HolderInvalidHash([u8; 32], [u8; 32]),

    #[error(
        "Deployment of contract which needs more than 10kb of account space needs several \
    transactions for reallocation and cannot be performed in a single instruction. \
    That's why you have to use iterative transaction for the deployment."
    )]
    AccountSpaceAllocationFailure,
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<Error> for ProgramError {
    fn from(e: Error) -> Self {
        log_msg!("{}", e);
        match e {
            Error::ProgramError(e) => e,
            _ => Self::Custom(0),
        }
    }
}

impl From<&'static str> for Error {
    fn from(value: &'static str) -> Self {
        Self::Custom(value.to_string())
    }
}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Self::Custom(value)
    }
}

/// Macro to log a `ProgramError` in the current transaction log
/// with the source file position like: file.rc:42
/// and additional info if needed
/// See `https://github.com/neonlabsorg/neon-evm/issues/159`
///
/// # Examples
///
/// ```ignore
/// #    return Err!(ProgramError::InvalidArgument; "Caller pubkey: {} ", &caller_info.key.to_string());
/// ```
///
macro_rules! Err {
    ( $n:expr; $($args:expr),* ) => ({
        #[cfg(target_os = "solana")]
        solana_program::msg!("{}:{} : {}", file!(), line!(), &format!($($args),*));

        #[cfg(all(not(target_os = "solana"), feature = "log"))]
        log::error!("{}", &format!($($args),*));

        Err($n)
    });
}

#[must_use]
fn format_revert_error(msg: &[u8]) -> Option<&str> {
    if msg.starts_with(&[0x08, 0xc3, 0x79, 0xa0]) {
        // Error(string) function selector
        let msg = &msg[4..];
        if msg.len() < 64 {
            return None;
        }

        let offset = U256::from_be_bytes(*arrayref::array_ref![msg, 0, 32]);
        if offset != 32 {
            return None;
        }

        let length = U256::from_be_bytes(*arrayref::array_ref![msg, 32, 32]);
        let length: usize = length.try_into().ok()?;

        let begin = 64_usize;
        let end = begin.checked_add(length)?;

        let reason = msg.get(begin..end)?;
        std::str::from_utf8(reason).ok()
    } else {
        None
    }
}

#[must_use]
fn format_revert_panic(msg: &[u8]) -> Option<U256> {
    if msg.starts_with(&[0x4e, 0x48, 0x7b, 0x71]) {
        // Panic(uint256) function selector
        let msg = &msg[4..];
        if msg.len() != 32 {
            return None;
        }

        let value = arrayref::array_ref![msg, 0, 32];
        Some(U256::from_be_bytes(*value))
    } else {
        None
    }
}

pub fn print_revert_message(msg: &[u8]) {
    if msg.is_empty() {
        return log_msg!("Revert");
    }

    if let Some(reason) = format_revert_error(msg) {
        return log_msg!("Revert: Error(\"{}\")", reason);
    }

    if let Some(reason) = format_revert_panic(msg) {
        return log_msg!("Revert: Panic({:#x})", reason);
    }

    log_msg!("Revert: {}", hex::encode(msg));
}

#[must_use]
pub fn build_revert_message(msg: &str) -> Vec<u8> {
    let data_len = if msg.len() % 32 == 0 {
        std::cmp::max(msg.len(), 32)
    } else {
        ((msg.len() / 32) + 1) * 32
    };

    let capacity = 4 + 32 + 32 + data_len;
    let mut result = Vec::with_capacity(capacity);
    result.extend_from_slice(&[0x08, 0xc3, 0x79, 0xa0]); // Error(string) function selector

    let offset = U256::new(0x20);
    result.extend_from_slice(&offset.to_be_bytes());

    let length = U256::new(msg.len() as u128);
    result.extend_from_slice(&length.to_be_bytes());

    result.extend_from_slice(msg.as_bytes());

    assert!(result.len() <= capacity);
    result.resize(capacity, 0);

    result
}
