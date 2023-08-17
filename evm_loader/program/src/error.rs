//! Error types
#![allow(clippy::use_self)]

use std::{array::TryFromSliceError, num::TryFromIntError};

use ethnum::U256;
use solana_program::{
    program_error::ProgramError, pubkey::Pubkey, secp256k1_recover::Secp256k1RecoverError,
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

    #[error("RLP error: {0}")]
    RlpError(#[from] rlp::DecoderError),

    #[error("Secp256k1 error: {0}")]
    Secp256k1Error(#[from] Secp256k1RecoverError),

    #[error("Bincode error: {0}")]
    BincodeError(#[from] bincode::Error),

    #[error("FromHexError error: {0}")]
    FromHexError(#[from] hex::FromHexError),

    #[error("TryFromIntError error: {0}")]
    TryFromIntError(#[from] TryFromIntError),

    #[error("TryFromSliceError error: {0}")]
    TryFromSliceError(#[from] TryFromSliceError),

    #[error("Account {0} - not found")]
    AccountMissing(Address),

    #[error("Account {0} - blocked")]
    AccountBlocked(Address),

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

    #[error("Operator is not authorized")]
    UnauthorizedOperator,

    #[error("Storage Account is uninitialized")]
    StorageAccountUninitialized,

    #[error("Storage Account is finalized")]
    StorageAccountFinalized,

    #[error("Unknown extension method selector {1:?}, contract {0}")]
    UnknownPrecompileMethodSelector(Address, [u8; 4]),

    #[error("Insufficient balance for transfer, account = {0}, required = {1}")]
    InsufficientBalance(Address, U256),

    #[error("Out of Gas, limit = {0}, required = {1}")]
    OutOfGas(U256, U256),

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

    #[error("EVM encountered deprecated opcode, contract = {0}, opcode = {1:X}")]
    DeprecatedOpcode(Address, u8),

    #[error("EVM encountered unknown opcode, contract = {0}, opcode = {1:X}")]
    UnknownOpcode(Address, u8),

    #[error("Account {0} nonce overflow")]
    NonceOverflow(Address),

    #[error("Invalid Nonce, origin {0} nonce {1} != Transaction nonce {2}")]
    InvalidTransactionNonce(Address, u64, u64),

    #[error("Invalid Chain ID {0}")]
    InvalidChainId(U256),

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

    #[error("Holder Account - invalid transaction hash {}, expected = {}", hex::encode(.0), hex::encode(.1))]
    HolderInvalidHash([u8; 32], [u8; 32]),

    #[error("Validation: undefined instruction: op {0:X}, pos {1}")]
    ValidationUndefinedInstruction(u8, usize),

    #[error("Validation: truncated immediate: op {0:X}, pos {1}")]
    ValidationTruncatedImmediate(u8, usize),

    #[error("Validation: invalid section argument: arg {0}, last {1}, pos {2}")]
    ValidationInvalidSectionArgument(u16, usize, usize),

    #[error("Validation: invalid jump destination: offset {0}, dest {1}, pos {2}")]
    ValidationInvalidJumpDest(i16, usize, usize),

    #[error("Validation: conflicting stack height: have {0}, want {1}")]
    ValidationConflictingStack(usize, usize),

    #[error("Validation: invalid number of branches in jump table: must not be 0, pos {0}")]
    ValidationInvalidBranchCount(usize),

    #[error("Validation: invalid number of outputs: have {0}, want {1}, at pos {2}")]
    ValidationInvalidOutputs(u8, usize, usize),

    #[error("Validation: invalid max stack height: {0}: have {1}, want {2}")]
    ValidationInvalidMaxStackHeight(usize, usize, u16),

    #[error("Validation: invalid code termination: end with {0}, pos {1}")]
    ValidationInvalidCodeTermination(u8, usize),

    #[error("Validation: unreachable code")]
    ValidationUnreachableCode,

    #[error("EVM Empty stack")]
    EmptyStack,

    #[error("EVM container not found")]
    ContainerNotFound,

    #[error("EVM function metadata not found at index {0}")]
    FunctionMetadataNotFound(usize),

    #[error("Opcode not supported, opcode {0:X}")]
    UnsupportedOpcode(u8),

    #[error("Unexpected end of file")]
    UnexpectedEndOfFile,

    #[error("Invalid magic: want 0xEF00")]
    InvalidMagic,

    #[error("Invalid version: have {0}, want 1")]
    InvalidVersion(u8),

    #[error("Missing section header: found section kind {:?} instead of {:?}", .0, .1)]
    MissingSectionHeader(u8, u8),

    #[error("Invalid type section size: type section size must be divisible by 4, have {0}")]
    InvalidTypeSizeMustBeDivisibleBy4(u16),

    #[error("Invalid type section size: type section must not exceed 4*1024, have {0}")]
    InvalidTypeSizeExceed(u16),

    #[error("Invalid code header")]
    InvalidCodeHeader,

    #[error("Invalid code size: mismatch of code sections cound and type signatures, types {0}, code {1}")]
    MismatchCodeSize(usize, usize),

    #[error("Invalid code size for section {0}: size must not be 0")]
    InvalidCodeSize(usize),

    #[error("Missing header terminator: have ({:#x?})", .0)]
    MissingTerminator(u8),

    #[error("Invalid type content, too many inputs for section {0}: have {1}")]
    TooManyInputs(u8, u8),

    #[error("Invalid type content, too many outputs for section {0}: have {1}")]
    TooManyOutputs(u8, u8),

    #[error("Invalid type content, max stack height exceeds limit for section {0}: have {1}")]
    TooLargeMaxStackHeight(u8, u8),

    #[error("Invalid section 0 type, input and output should be zero: have {0}, {1}")]
    InvalidSection0Type(u8, u8),

    #[error("invalid code: EOF contract must not deploy legacy code")]
    EOFLegacyCode,

    #[error("Invalid container size: have {0}, want {1}")]
    InvalidContainerSize(usize, usize),

    #[error("Unknown section header ({:#x?})", .0)]
    UnknownSectionHeader(u8),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<Error> for ProgramError {
    fn from(e: Error) -> Self {
        solana_program::msg!("{}", e);
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

/// Macro to log a `ProgramError` in the current transaction log.
/// with the source file position like: file.rc:777
/// and additional info if needed
/// See `https://github.com/neonlabsorg/neon-evm/issues/159`
///
/// # Examples
///
/// ```ignore
/// #    map_err(|s| E!(ProgramError::InvalidArgument; "s={:?}", s))
/// ```
///
macro_rules! E {
    ( $n:expr; $($args:expr),* ) => ({
        #[cfg(target_os = "solana")]
        solana_program::msg!("{}:{} : {}", file!(), line!(), &format!($($args),*));

        #[cfg(all(not(target_os = "solana"), feature = "log"))]
        log::error!("{}", &format!($($args),*));

        $n
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
        return solana_program::msg!("Revert");
    }

    if let Some(reason) = format_revert_error(msg) {
        return solana_program::msg!("Revert: Error(\"{}\")", reason);
    }

    if let Some(reason) = format_revert_panic(msg) {
        return solana_program::msg!("Revert: Panic({:#x})", reason);
    }

    solana_program::msg!("Revert: {}", hex::encode(msg));
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
