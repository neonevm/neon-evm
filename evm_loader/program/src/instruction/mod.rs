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
    CreateAccount,

    /// Deprecated: Create ethereum account with seed
    #[deprecated(note = "Instruction not supported")]
    CreateAccountWithSeed,

    /// Call Ethereum-contract action from raw transaction data
    CallFromRawEthereumTX,

    /// Called action return
    OnReturn,

    /// Called action event
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

    /// Cancel iterative transaction execution
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

    /// Recompute Valids Table
    ///
    /// # Account references
    ///   0. \[WRITE\] Code account
    UpdateValidsTable,

    /// Deprecated: Create Ethereum account V2
    #[deprecated(note = "Instruction not supported")]
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

    /// Deprecated: Migrates Ethereum account's internal structure from v1 to current.
    #[deprecated(note = "Instruction not supported")]
    MigrateAccount,

    /// Same as ExecuteTrxFromAccountDataIterativeOrContinue, but for transactions without chain id
    ExecuteTrxFromAccountDataIterativeOrContinueNoChainId,

    /// Deprecated: Writes value to Ethereum account's distributed practically infinite storage.
    #[deprecated(note = "Instruction not supported")]
    WriteValueToDistributedStorage,

    /// Deprecated: Converts data account from V1 (HAMT) to V2 (distributed storage).
    #[deprecated(note = "Instruction not supported")]
    ConvertDataAccountFromV1ToV2,

    /// Create Ethereum account
    /// # Account references
    ///   0. [WRITE, SIGNER] Funding account
    ///   1. [] System Program
    ///   2. [WRITE] New account (program_address(version, ether, bump_seed))
    CreateAccountV03,
}

impl EvmInstruction {
    /// Parse `EvmInstruction`
    ///
    /// # Errors
    /// Will return `ProgramError::InvalidInstructionData` if can't parse `tag`
    pub const fn parse(tag: &u8) -> Result<Self, ProgramError> {
        Ok(match tag {
            2 => Self::CreateAccount, // deprecated
            5 => Self::CallFromRawEthereumTX,
            6 => Self::OnReturn,
            7 => Self::OnEvent,
            9 => Self::PartialCallFromRawEthereumTX, // deprecated
            10 => Self::Continue, // deprecated
            11 => Self::ExecuteTrxFromAccountDataIterative, // deprecated
            12 => Self::Cancel, // deprecated
            13 => Self::PartialCallOrContinueFromRawEthereumTX,
            14 => Self::ExecuteTrxFromAccountDataIterativeOrContinue,
            15 => Self::ERC20CreateTokenAccount,
            16 => Self::DeleteHolderOrStorageAccount,
            17 => Self::ResizeContractAccount, // deprecated
            18 => Self::WriteHolder,
            19 => Self::PartialCallFromRawEthereumTxV03,
            20 => Self::ContinueV03,
            21 => Self::CancelWithNonce,
            22 => Self::ExecuteTrxFromAccountDataIterativeV03,
            23 => Self::UpdateValidsTable,
            24 => Self::CreateAccountV02, // deprecated
            25 => Self::Deposit,
            26 => Self::MigrateAccount, // deprecated
            27 => Self::ExecuteTrxFromAccountDataIterativeOrContinueNoChainId,
            28 => Self::WriteValueToDistributedStorage, // deprecated
            29 => Self::ConvertDataAccountFromV1ToV2, // deprecated
            30 => Self::CreateAccountV03,

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
pub mod update_valids_table;
pub mod transaction;
