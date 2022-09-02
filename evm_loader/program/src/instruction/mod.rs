#![allow(deprecated)]
//! `EvmInstruction` serialization/deserialization

use solana_program::{ program_error::ProgramError };


/// `EvmInstruction` serialized in instruction data
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum EvmInstruction {
    /// Creates an ERC20 token account for the given Ethereum wallet address, contract address and token mint
    ///
    /// ### Account references
    ///   0. `[writeable,signer]` Funding account (must be a system account)
    ///   1. `[writeable]` ERC20 token account address to be created
    ///   2. `[]` Wallet address for the new ERC20 token account
    ///   3. '[]' Contract address
    ///   4. `[]` The token mint for the new ERC20 token account
    ///   5. `[]` System program
    ///   6. `[]` SPL Token program
    ///   7. '[]' Rent sysvar
    ERC20CreateTokenAccount,


    /// copying the content of the one code_account to the new code_account
    /// # Account references
    ///   0. [WRITE] contract account
    ///   1. [WRITE] current code account
    ///   2. [WRITE] new code account
    ///   3. [READ] operator account
    ResizeContractAccount,


    /// Recompute Valids Table
    ///
    /// # Account references
    ///   0. \[WRITE\] Code account
    UpdateValidsTable,

    /// Create Ethereum account
    /// # Account references
    ///   0. [WRITE, SIGNER] Funding account
    ///   1. [] System Program
    ///   2. [WRITE] New account (program_address(version, ether, bump_seed))
    ///   3. (for contract creation) [WRITE] Code account for new contract account
    CreateAccountV02,

    /// Deposits NEON tokens to a Ether account.
    /// Requires previously executed SPL-Token.Approve which
    /// delegates the deposit amount to the NEON destination account.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable]` NEON token source account.
    ///   1. `[writable]` NEON token pool (destination) account.
    ///   2. `[writable]` Ether account to store balance of NEONs.
    ///   3. `[]` EVM Loader authority account (PDA, seeds = \[b"Deposit"\]).
    ///   5. `[]` SPL Token program id.
    Deposit,
    Deposit2,

    /// Migrates Ethereum account's internal structure from v1 to current.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable]` Operator (to close associated token account).
    ///   1. `[writable]` Ether account to migrate.
    ///   2. `[writable]` NEON token account associated with the ether account.
    ///   3. `[writable]` NEON token pool account.
    ///   4. `[]` EVM Loader authority account (PDA, seeds = \[b"Deposit"\]).
    ///   5. `[]` SPL Token program id.
    MigrateAccount,

    /// Writes value to Ethereum account's distributed practically infinite storage.
    WriteValueToDistributedStorage,

    /// Converts data account from V1 (HAMT) to V2 (distributed storage).
    ConvertDataAccountFromV1ToV2,

    /// Collect lamports from treasury pool accounts to main pool balance
    CollectTreasure,

    // Create Holder Account
    HolderCreate,

    // Delete Holder Account
    HolderDelete,

    // Write Transaction into Holder Account
    HolderWrite,

    // Execute Transaction from Instruction in single iteration
    TransactionExecuteFromInstruction,

    // Execute Iterative Transaction from Instruction
    TransactionStepFromInstruction,

    // Execute Iterative Transaction from Account
    TransactionStepFromAccount,

    // Execute Iterative Transaction without ChainId from Account
    TransactionStepFromAccountNoChainId,

    // Cancel Transaction
    Cancel
}

impl EvmInstruction {
    /// Parse `EvmInstruction`
    ///
    /// # Errors
    /// Will return `ProgramError::InvalidInstructionData` if can't parse `tag`
    pub const fn parse(tag: &u8) -> Result<Self, ProgramError> {
        Ok(match tag {
            15 => Self::ERC20CreateTokenAccount,
            17 => Self::ResizeContractAccount,
            23 => Self::UpdateValidsTable,
            24 => Self::CreateAccountV02,
            25 => Self::Deposit,
            26 => Self::MigrateAccount,
            28 => Self::WriteValueToDistributedStorage,
            29 => Self::ConvertDataAccountFromV1ToV2,
            30 => Self::CollectTreasure,
            31 => Self::TransactionExecuteFromInstruction,
            32 => Self::TransactionStepFromInstruction,
            33 => Self::TransactionStepFromAccount,
            34 => Self::TransactionStepFromAccountNoChainId,
            35 => Self::Cancel,
            36 => Self::HolderCreate,
            37 => Self::HolderDelete,
            38 => Self::HolderWrite,
            39 => Self::Deposit2,

            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }
}


pub mod account_create;
pub mod account_holder_create;
pub mod account_holder_delete;
pub mod account_holder_write;
pub mod account_resize;
pub mod erc20_account_create;
pub mod neon_tokens_deposit;
pub mod neon_tokens_deposit_2;
pub mod migrate_account;
pub mod transaction_cancel;
pub mod transaction_execute_from_instruction;
pub mod transaction_step_from_instruction;
pub mod transaction_step_from_account;
pub mod transaction_step_from_account_no_chainid;
pub mod update_valids_table;
pub mod transaction;
pub mod storage_to_v2;
pub mod collect_treasury;
