#![allow(deprecated)]
//! `EvmInstruction` serialization/deserialization

use solana_program::{ program_error::ProgramError };


/// `EvmInstruction` serialized in instruction data
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum EvmInstruction {
    /// Deposits NEON tokens to an Ether account (V3).
    /// Requires previously executed SPL-Token.Approve which
    /// delegates the deposit amount to the NEON destination account.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable]` NEON token source account.
    ///   1. `[writable]` NEON token pool (destination) account.
    ///   2. `[writable]` Ether account to store balance of NEONs.
    ///   3. `[]` SPL Token program id.
    ///   4. `[writeable,signer]` Funding account (must be a system account).
    ///   5. `[]` System program.
    DepositV03,

    /// Create Ethereum account V3
    /// # Account references
    ///   0. [WRITE, SIGNER] Funding account
    ///   1. [] System Program
    ///   2. [WRITE] New account (program_address(version, ether, bump_seed))
    CreateAccountV03,

    /// Collect lamports from treasury pool accounts to main pool balance
    CollectTreasure,

    /// Create Holder Account
    HolderCreate,

    /// Delete Holder Account
    HolderDelete,

    /// Write Transaction into Holder Account
    HolderWrite,

    /// Execute Transaction from Instruction in single iteration
    TransactionExecuteFromInstruction,

    /// Execute Iterative Transaction from Instruction
    TransactionStepFromInstruction,

    /// Execute Iterative Transaction from Account
    TransactionStepFromAccount,

    /// Execute Iterative Transaction without ChainId from Account
    TransactionStepFromAccountNoChainId,

    /// Cancel Transaction
    Cancel,
}

impl EvmInstruction {
    /// Parse `EvmInstruction`
    ///
    /// # Errors
    /// Will return `ProgramError::InvalidInstructionData` if can't parse `tag`
    pub const fn parse(tag: &u8) -> Result<Self, ProgramError> {
        Ok(match tag {
            0x1e => Self::CollectTreasure,                          // 30
            0x1f => Self::TransactionExecuteFromInstruction,        // 31
            0x20 => Self::TransactionStepFromInstruction,           // 32
            0x21 => Self::TransactionStepFromAccount,               // 33
            0x22 => Self::TransactionStepFromAccountNoChainId,      // 34
            0x23 => Self::Cancel,                                   // 35
            0x24 => Self::HolderCreate,                             // 36
            0x25 => Self::HolderDelete,                             // 37
            0x26 => Self::HolderWrite,                              // 38
            0x27 => Self::DepositV03,                               // 39
            0x28 => Self::CreateAccountV03,                         // 40

            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }
}


pub mod account_create;
pub mod account_holder_create;
pub mod account_holder_delete;
pub mod account_holder_write;
pub mod neon_tokens_deposit;
pub mod transaction_cancel;
pub mod transaction_execute_from_instruction;
pub mod transaction_step_from_instruction;
pub mod transaction_step_from_account;
pub mod transaction_step_from_account_no_chainid;
pub mod transaction;
pub mod collect_treasury;
