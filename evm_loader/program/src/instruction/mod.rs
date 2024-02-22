#![allow(deprecated)]
//! `EvmInstruction` serialization/deserialization

use solana_program::program_error::ProgramError;

/// `EvmInstruction` serialized in instruction data
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum EvmInstruction {
    /// Deposits spl-tokens to an Ether account.
    /// Requires previously executed SPL-Token.Approve which
    /// delegates the deposit amount to the NEON destination account.
    ///
    /// Accounts:
    ///  `[]` spl-token mint account.
    ///  `[WRITE]` spl-token source account.
    ///  `[WRITE]` spl-token pool (destination) account.
    ///  `[WRITE]` NeonEVM user balance account
    ///  `[WRITE]` NeonEVM user contract account
    ///  `[]` SPL Token program id.
    ///  `[writeable,signer]` Funding account (must be a system account).
    ///  `[]` System program.
    /// Instruction data:
    ///  0..20  - destination address
    ///  20..28 - chain id in little endian
    Deposit,

    /// Collect lamports from treasury pool accounts to main pool balance
    ///
    /// Accounts:
    ///  `[WRITE]` Main treasury balance: PDA["treasury_pool"]
    ///  `[WRITE]` Auxiliary treasury balance: PDA["treasury_pool", index.to_le_bytes()]
    ///  `[]` System program
    /// Instruction data:
    ///  0..4 - treasury index in little endian
    CollectTreasure,

    /// Create Holder Account
    ///
    /// Accounts:
    ///  `[WRITE]` Holder Account
    ///  `[SIGNER]` Holder Account Owner
    /// Instruction data:
    ///  0..8          - seed length in little endian
    ///  8..8+seed_len - seed in utf-8
    HolderCreate,

    /// Delete Holder Account
    ///
    /// Accounts:
    ///  `[WRITE]` Holder Account
    ///  `[WRITE,SIGNER]` Holder Account Owner
    /// Instruction data:
    ///   None
    HolderDelete,

    /// Write Transaction into Holder Account
    ///
    /// Accounts:
    ///  `[WRITE]` Holder Account
    ///  `[SIGNER]` Holder Account Owner
    /// Instruction data:
    ///  0..32  - transaction hash
    ///  32..40 - offset in Holder in little endian
    ///  40..   - transaction data
    HolderWrite,

    /// Execute Transaction from Instruction in single iteration
    ///
    /// Accounts:
    ///  `[WRITE,SIGNER]` Operator
    ///  `[WRITE]` Treasury
    ///  `[WRITE]` Operator Balance
    ///  `[]` System program
    ///  `[WRITE?]` Other accounts
    /// Instruction data:
    ///  0..4 - treasury index in little endian
    ///  4..  - transaction data
    TransactionExecuteFromInstruction,

    /// Execute Transaction from Account in single iteration
    ///
    /// Accounts:
    ///  `[]` Holder
    ///  `[WRITE,SIGNER]` Operator
    ///  `[WRITE]` Treasury
    ///  `[WRITE]` Operator Balance
    ///  `[]` System program
    ///  `[WRITE?]` Other accounts
    /// Instruction data:
    ///  0..4 - treasury index in little endian
    TransactionExecuteFromAccount,

    /// Execute Iterative Transaction from Instruction
    ///
    /// Accounts:
    ///  `[WRITE]` Holder/State
    ///  `[WRITE,SIGNER]` Operator
    ///  `[WRITE]` Treasury
    ///  `[WRITE]` Operator Balance
    ///  `[]` System program
    ///  `[WRITE]`  Other accounts
    /// Instruction data:
    ///  0..4 - treasury index in little endian
    ///  4..8 - step count in little endian
    ///  8..  - transaction data
    TransactionStepFromInstruction,

    /// Execute Iterative Transaction from Account
    ///
    /// Accounts:
    ///  `[WRITE]` Holder/State
    ///  `[WRITE,SIGNER]` Operator
    ///  `[WRITE]` Treasury
    ///  `[WRITE]` Operator Balance
    ///  `[]` System program
    ///  `[WRITE]`  Other accounts
    /// Instruction data:
    ///  0..4 - treasury index in little endian
    ///  4..8 - step count in little endian
    TransactionStepFromAccount,

    /// Execute Iterative Transaction without ChainId from Account
    ///
    /// Accounts:
    ///  `[WRITE]` Holder/State
    ///  `[WRITE,SIGNER]` Operator
    ///  `[WRITE]` Treasury
    ///  `[WRITE]` Operator Balance
    ///  `[]` System program
    ///  `[WRITE]`  Other accounts
    /// Instruction data:
    ///  0..4 - treasury index in little endian
    ///  4..8 - step count in little endian
    TransactionStepFromAccountNoChainId,

    /// Cancel Transaction
    ///
    /// Accounts:
    ///  `[WRITE]` State
    ///  `[SIGNER]` Operator
    ///  `[WRITE]` Operator Balance
    /// Instruction data:
    ///   0..32 - transaction hash
    Cancel,

    /// CreateMainTreasury
    ///
    /// Accounts:
    ///  `[WRITE]` Main treasury balance: PDA["treasury_pool"]
    ///  `[]` Program data (to get program upgrade-authority)
    ///  `[SIGNER]` Owner for account (upgrade program authority)
    ///  `[]` SPL token program id
    ///  `[]` System program
    ///  `[]` wSOL mint
    ///  `[WRITE,SIGNER]` Payer
    /// Instruction data:
    ///  None
    CreateMainTreasury,

    /// Create a User Balance account
    ///
    /// Accounts:
    ///  `[WRITE,SIGNER]` Operator
    ///  `[]` System program
    ///  `[WRITE]` NeonEVM user balance account
    ///  `[WRITE]` NeonEVM user contract account
    /// Instruction data:
    ///  0..20  - address
    ///  20..28 - chain id in little endian
    AccountCreateBalance,

    ConfigGetChainCount,
    ConfigGetChainInfo,
    ConfigGetEnvironment,
    ConfigGetPropertyCount,
    ConfigGetPropertyByIndex,
    ConfigGetPropertyByName,
    ConfigGetStatus,
    ConfigGetVersion,
}

impl EvmInstruction {
    /// Parse `EvmInstruction`
    ///
    /// # Errors
    /// Will return `ProgramError::InvalidInstructionData` if can't parse `tag`
    pub const fn parse(tag: &u8) -> Result<Self, ProgramError> {
        Ok(match tag {
            0x1e => Self::CollectTreasure,    // 30
            0x24 => Self::HolderCreate,       // 36
            0x25 => Self::HolderDelete,       // 37
            0x26 => Self::HolderWrite,        // 38
            0x29 => Self::CreateMainTreasury, // 41

            0x30 => Self::AccountCreateBalance,              // 48
            0x31 => Self::Deposit,                           // 49
            0x32 => Self::TransactionExecuteFromInstruction, // 50
            0x33 => Self::TransactionExecuteFromAccount,     // 51
            0x34 => Self::TransactionStepFromInstruction,    // 52
            0x35 => Self::TransactionStepFromAccount,        // 53
            0x36 => Self::TransactionStepFromAccountNoChainId, // 54
            0x37 => Self::Cancel,                            // 55

            0xA0 => Self::ConfigGetChainCount, // 160
            0xA1 => Self::ConfigGetChainInfo,
            0xA2 => Self::ConfigGetEnvironment,
            0xA3 => Self::ConfigGetPropertyCount,
            0xA4 => Self::ConfigGetPropertyByIndex,
            0xA5 => Self::ConfigGetPropertyByName,
            0xA6 => Self::ConfigGetStatus,
            0xA7 => Self::ConfigGetVersion,
            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }
}

pub mod account_create_balance;
pub mod account_holder_create;
pub mod account_holder_delete;
pub mod account_holder_write;
pub mod collect_treasury;
pub mod config_get_chain_count;
pub mod config_get_chain_info;
pub mod config_get_environment;
pub mod config_get_property_by_index;
pub mod config_get_property_by_name;
pub mod config_get_property_count;
pub mod config_get_status;
pub mod config_get_version;
pub mod create_main_treasury;
pub mod neon_tokens_deposit;
pub mod transaction_cancel;
pub mod transaction_execute;
pub mod transaction_execute_from_account;
pub mod transaction_execute_from_instruction;
pub mod transaction_step;
pub mod transaction_step_from_account;
pub mod transaction_step_from_account_no_chainid;
pub mod transaction_step_from_instruction;
