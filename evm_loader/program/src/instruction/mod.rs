#![allow(deprecated)]
//! `EvmInstruction` serialization/deserialization

use solana_program::{ program_error::ProgramError };


/// `EvmInstruction` serialized in instruction data
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum EvmInstruction {
    /// Deprecated: Write to an account
    #[deprecated(note = "Instruction not supported")]
    Write,

    /// Deprecated: Finalize an account loaded with program data for execution
    #[deprecated(note = "Instruction not supported")]
    Finalise,

    /// Deprecated: Create ethereum account
    #[deprecated(note = "Instruction not supported")]
    CreateAccountV01,

    /// Deprecated: Create ethereum account with seed
    #[deprecated(note = "Instruction not supported")]
    CreateAccountWithSeed,

    /// Call Ethereum-contract action from raw transaction data
    CallFromRawEthereumTX,

    /// Deprecated: Called action return
    #[deprecated(note = "Instruction not supported")]
    OnReturn,

    /// Deprecated: Called action event
    #[deprecated(note = "Instruction not supported")]
    OnEvent,

    /// Deprecated: Partial call Ethereum-contract action from raw transaction data stored in holder account data
    #[deprecated(note = "Instruction not supported")]
    PartialCallFromRawEthereumTX,

    /// Partial call Ethereum-contract action from raw transaction data
    /// ### Account references
    ///   0. \[WRITE\] storage account
    ///   1. ... Account references same as in Call
    PartialCallFromRawEthereumTxV03,

    /// Deprecated: Continue (version 01) Ethereum-contract action from raw transaction data
    #[deprecated(note = "Instruction not supported")]
    Continue,

    /// Continue (version 02) Ethereum-contract action from raw transaction data
    /// ### Account references same as in PartialCallFromRawEthereumTX
    ContinueV03,

    /// Deprecated: Partial call Ethereum-contract action from raw transaction data stored in holder account data
    #[deprecated(note = "Instruction not supported")]
    ExecuteTrxFromAccountDataIterative,

    /// Partial call Ethereum-contract action from raw transaction data stored in holder account data
    ExecuteTrxFromAccountDataIterativeV03,

    /// Deprecated: cancel iterative transaction execution
    #[deprecated(note = "Instruction not supported")]
    Cancel,

    /// Partial call Ethereum-contract action from raw transaction data
    /// or Continue
    /// ### Account references
    ///   0. \[WRITE\] storage account
    ///   1. ... Account references same as in Call
    PartialCallOrContinueFromRawEthereumTX,

    /// Partial call Ethereum-contract action from raw transaction data stored in holder account data
    /// or
    /// Continue
    ExecuteTrxFromAccountDataIterativeOrContinue,

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

    /// Delete Holder or Storage account
    /// # Account references
    ///   0. [WRITE] Deleted account
    ///   1. [WRITE] Deleted account creator
    DeleteHolderOrStorageAccount,

    /// Deprecated: copying the content of the one code_account to the new code_account
    #[deprecated(note = "Instruction not supported")]
    ResizeContractAccount,

    /// Cancel iterative transaction execution providing caller nonce
    CancelWithNonce,

    /// Write program data into a holder account
    ///
    /// # Account references
    ///   0. \[WRITE\] Account to write to
    ///   1. \[SIGNER\] Signer for Ether account
    WriteHolder,

    /// Deprecated: recompute Valids Table
    #[deprecated(note = "Instruction not supported")]
    UpdateValidsTable,

    /// Deprecated: Create Ethereum account V2
    /// Note: Account creation now performed implicitly in most cases.
    CreateAccountV02,

    /// Deprecated: deposits NEON tokens to a Ether account.
    #[deprecated(note = "Use `DepositV03` instead")]
    DepositV02,

    /// Deprecated: migrates Ethereum account's internal structure from v1 to current.
    #[deprecated(note = "Instruction not supported")]
    Migrate01AccountFromV1ToV2,

    /// Same as ExecuteTrxFromAccountDataIterativeOrContinue, but for transactions without chain id
    ExecuteTrxFromAccountDataIterativeOrContinueNoChainId,

    /// Deprecated: writes value to Ethereum account's distributed practically infinite storage.
    #[deprecated(note = "Instruction not supported")]
    Migrate02ContractFromV1ToV2WriteValueToDistributedStorage,

    /// Deprecated: converts data account from V1 (HAMT) to V2 (distributed storage).
    #[deprecated(note = "Instruction not supported")]
    Migrate02ContractFromV1ToV2ConvertDataAccount,

    /// Collect lamports from treasury pool accounts to main pool balance
    CollectTreasure,

    /// Deposits NEON tokens to an Ether account (V3).
    /// Requires previously executed SPL-Token.Approve which
    /// delegates the deposit amount to the NEON destination account.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable]` NEON token source account.
    ///   1. `[writable]` NEON token pool (destination) account.
    ///   2. `[writable]` Ether account to store balance of NEONs.
    ///   3. `[]` EVM Loader authority account (PDA, seeds = \[b"Deposit"\]).
    ///   4. `[]` SPL Token program id.
    ///   5. `[writeable,signer]` Funding account (must be a system account).
    ///   6. `[]` System program.
    DepositV03,

    /// Create Ethereum account V3
    /// # Account references
    ///   0. [WRITE, SIGNER] Funding account
    ///   1. [] System Program
    ///   2. [WRITE] New account (program_address(version, ether, bump_seed))
    CreateAccountV03,

    /// Merges contract with account and converts account to the version 3 format.
    /// # Account references
    ///   0. [WRITE, SIGNER] Operator account.
    ///   1. [] System Program.
    ///   2. [WRITE] Neon account to convert.
    ///   3. [WRITE] (optional) Neon contract to convert.
    Migrate03AccountFromV2ToV3,
}

impl EvmInstruction {
    /// Parse `EvmInstruction`
    ///
    /// # Errors
    /// Will return `ProgramError::InvalidInstructionData` if can't parse `tag`
    pub const fn parse(tag: &u8) -> Result<Self, ProgramError> {
        Ok(match tag {
            0x02 => Self::CreateAccountV01, // deprecated
            0x05 => Self::CallFromRawEthereumTX,
            0x06 => Self::OnReturn, // deprecated
            0x07 => Self::OnEvent, // deprecated
            0x09 => Self::PartialCallFromRawEthereumTX, // deprecated
            0x0a => Self::Continue, // deprecated
            0x0b => Self::ExecuteTrxFromAccountDataIterative, // deprecated
            0x0c => Self::Cancel, // deprecated
            0x0d => Self::PartialCallOrContinueFromRawEthereumTX,
            0x0e => Self::ExecuteTrxFromAccountDataIterativeOrContinue,
            0x0f => Self::ERC20CreateTokenAccount,
            0x10 => Self::DeleteHolderOrStorageAccount,
            0x11 => Self::ResizeContractAccount, // deprecated
            0x12 => Self::WriteHolder,
            0x13 => Self::PartialCallFromRawEthereumTxV03,
            0x14 => Self::ContinueV03,
            0x15 => Self::CancelWithNonce,
            0x16 => Self::ExecuteTrxFromAccountDataIterativeV03,
            0x17 => Self::UpdateValidsTable, // deprecated
            0x18 => Self::CreateAccountV02, // deprecated, no-op
            0x19 => Self::DepositV02, // deprecated
            0x1a => Self::Migrate01AccountFromV1ToV2, // deprecated
            0x1b => Self::ExecuteTrxFromAccountDataIterativeOrContinueNoChainId,
            0x1c => Self::Migrate02ContractFromV1ToV2WriteValueToDistributedStorage, // deprecated
            0x1d => Self::Migrate02ContractFromV1ToV2ConvertDataAccount, // deprecated
            0x1e => Self::CollectTreasure,
            0x1f => Self::DepositV03,
            0x20 => Self::CreateAccountV03,
            0x21 => Self::Migrate03AccountFromV2ToV3,

            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }
}


pub mod account_create;
pub mod account_delete_holder_storage;
pub mod erc20_account_create;
pub mod neon_tokens_deposit;
pub mod transaction_write_to_holder;
pub mod transaction_cancel;
pub mod transaction_execute_from_instruction;
pub mod transaction_begin_from_instruction;
pub mod transaction_begin_from_account;
pub mod transaction_continue;
pub mod transaction_step_from_instruction;
pub mod transaction_step_from_account;
pub mod transaction_step_from_account_no_chainid;
pub mod transaction;
pub mod collect_treasury;
pub mod migrate_v2_to_v3;
