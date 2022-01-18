#![allow(deprecated)]
//! `EvmInstruction` serialization/deserialization

use serde::{Serialize, Serializer};
use solana_program::{program_error::ProgramError, pubkey::Pubkey, instruction::Instruction};
use std::convert::{TryInto, TryFrom};
use evm::{H160, H256};
use evm::backend::Log;

fn serialize_h160<S>(value: &H160, s: S) -> Result<S::Ok, S::Error> where S: Serializer {
    value.as_fixed_bytes().serialize(s)
}

/// `EvmInstruction` serialized in instruction data
#[derive(Serialize, Debug, PartialEq, Eq, Clone)]
pub enum EvmInstruction<'a> {
    /// Deprecated: Write to an account
    #[deprecated(note = "Instruction not supported")]
    Write,

    /// Deprecated: Finalize an account loaded with program data for execution
    #[deprecated(note = "Instruction not supported")]
    Finalise,

    ///
    /// Create Ethereum account (create program_address account and write data)
    /// # Account references
    ///   0. [WRITE, SIGNER] Funding account
    ///   1. \[WRITE\] New account (program_address(ether, nonce))
    ///   2. (for contract creation) \[WRITE\] Code account for new contract account
    CreateAccount {
        /// Number of lamports to transfer to the new account
        lamports: u64,

        /// Number of bytes of memory to allocate
        space: u64,

        /// Ethereum address of account
        #[serde(serialize_with="serialize_h160")]
        ether: H160,

        /// Nonce for create valid program_address from ethereum address
        nonce: u8,
    },

    // TODO: EvmInstruction::Call
    // https://github.com/neonlabsorg/neon-evm/issues/188
    // Does not fit in current vision.
    // It is needed to update behavior for all system in whole.
    // /// Call Ethereum-contract action
    // /// ### Account references
    // ///   0. \[WRITE\] Contract account for execution (Ether account)
    // ///   1. \[WRITE\] Contract code account (Code account)
    // ///   2. \[WRITE\] Caller (Ether account)
    // ///   3. \[SIGNER\] Signer for caller
    // ///   4. \[\] Clock sysvar
    // ///   ... other Ether accounts
    // Call {
    //     /// Seed index for a collateral pool account
    //     collateral_pool_index: u32,
    //     /// Call data
    //     bytes: &'a [u8],
    // },

    /// Deprecated: Create ethereum account with seed
    #[deprecated(note = "Instruction not supported")]
    CreateAccountWithSeed,

    /// Call Ethereum-contract action from raw transaction data
    /// #### Account references same as in Call
    CallFromRawEthereumTX {
        /// Seed index for a collateral pool account
        collateral_pool_index: u32,
        /// Ethereum transaction sender address
        from_addr: &'a [u8],
        /// Ethereum transaction sign
        sign: &'a [u8],
        /// Unsigned ethereum transaction
        unsigned_msg: &'a [u8],
    },

    /// Called action return
    OnReturn {
        /// Contract execution status 
        /// Success - 0x11, 0x12 or 0x13 otherwise Error
        status: u8,
        /// Returned data
        bytes: &'a [u8],
    },

    /// Called action event
    OnEvent {
        /// Address
        address: H160,
        /// Topics
        topics: Vec<H256>,
        /// Data
        data: &'a [u8],
    },

    /// Deprecated: Partial call Ethereum-contract action from raw transaction data stored in holder account data
    #[deprecated(note = "Instruction not supported")]
    PartialCallFromRawEthereumTX,

    /// Partial call Ethereum-contract action from raw transaction data
    /// ### Account references
    ///   0. \[WRITE\] storage account
    ///   1. ... Account references same as in Call
    PartialCallFromRawEthereumTXv02 {
        /// Seed index for a collateral pool account
        collateral_pool_index: u32,
        /// Steps of ethereum contract to execute
        step_count: u64,
        /// Ethereum transaction sender address
        from_addr: &'a [u8],
        /// Ethereum transaction sign
        sign: &'a [u8],
        /// Unsigned ethereum transaction
        unsigned_msg: &'a [u8],
    },

    /// Deprecated: Continue (version 01) Ethereum-contract action from raw transaction data
    #[deprecated(note = "Instruction not supported")]
    Continue,

    /// Continue (version 02) Ethereum-contract action from raw transaction data
    /// ### Account references same as in PartialCallFromRawEthereumTX
    ContinueV02 {
        /// Seed index for a collateral pool account
        collateral_pool_index: u32,
        /// Steps of ethereum contract to execute
        step_count: u64,
    },
    /// Deprecated: Partial call Ethereum-contract action from raw transaction data stored in holder account data
    #[deprecated(note = "Instruction not supported")]
    ExecuteTrxFromAccountDataIterative,

    /// Partial call Ethereum-contract action from raw transaction data stored in holder account data
    ExecuteTrxFromAccountDataIterativeV02 {
        /// Seed index for a collateral pool account
        collateral_pool_index: u32,
        /// Steps of ethereum contract to execute
        step_count: u64,
    },

    /// Cancel iterative transaction execution
    #[deprecated(note = "Instruction not supported")]
    Cancel,

    /// Partial call Ethereum-contract action from raw transaction data
    /// or Continue
    /// ### Account references
    ///   0. \[WRITE\] storage account
    ///   1. ... Account references same as in Call
    PartialCallOrContinueFromRawEthereumTX {
        /// Seed index for a collateral pool account
        collateral_pool_index: u32,
        /// Steps of ethereum contract to execute
        step_count: u64,
        /// Ethereum transaction sender address
        from_addr: &'a [u8],
        /// Ethereum transaction sign
        sign: &'a [u8],
        /// Unsigned ethereum transaction
        unsigned_msg: &'a [u8],
    },

    /// Partial call Ethereum-contract action from raw transaction data stored in holder account data
    /// or
    /// Continue
    ExecuteTrxFromAccountDataIterativeOrContinue {
        /// Seed index for a collateral pool account
        collateral_pool_index: u32,
        /// Steps of ethereum contract to execute
        step_count: u64,
    },

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

    /// Delete Ethereum account
    /// # Account references
    ///   0. [WRITE] Deleted account
    ///   1. [WRITE] Deleted account creator
    DeleteAccount {
        /// seed used to create account
        seed:  &'a [u8],
    },
    
    /// copying the content of the one code_account to the new code_account
    /// # Account references
    ///   0. [WRITE] contract account
    ///   1. [WRITE] current code account
    ///   2. [WRITE] new code account
    ///   3. [READ] operator account
    ResizeStorageAccount {
        /// seed used to create account
        seed:  &'a [u8],
    },

    /// Cancel iterative transaction execution providing caller nonce
    CancelWithNonce {
        /// Nonce of caller in canceled transaction
        nonce: u64,
    },

    /// Write program data into a holder account
    ///
    /// # Account references
    ///   0. \[WRITE\] Account to write to
    ///   1. \[SIGNER\] Signer for Ether account
    WriteHolder {
        /// Magical number
        holder_id: u64,
        /// Offset at which to write the given bytes
        offset: u32,
        /// Data to write
        bytes: &'a [u8],
    },

    /// Recompute Valids Table
    /// 
    /// # Account references
    ///   0. \[WRITE\] Code account
    UpdateValidsTable,

    /// Deposits NEON tokens to a Ether account.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable]` The NEON source account.
    ///   1. `[writable]` The NEON destination account.
    ///   2. `[writable]` The Ether account to store balance.
    ///   2. `[]` The EVM Loader program id.
    Deposit,
}

impl<'a> EvmInstruction<'a> {
    /// Unpack `EvmInstruction`
    /// ```
    /// let instruction = EvmInstruction::unpack(instruction_data)?;
    /// ```
    /// # Errors
    ///
    /// Will return `ProgramError::InvalidInstructionData` if can't parse `input`
    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
    pub fn unpack(input: &'a[u8]) -> Result<Self, ProgramError> {
        use ProgramError::InvalidInstructionData;

        let (&tag, rest) = input.split_first().ok_or(InvalidInstructionData)?;

        Ok(match tag {
            2 => {
                let (_, rest) = rest.split_at(3);
                let (lamports, rest) = rest.split_at(8);
                let (space, rest) = rest.split_at(8);

                let lamports = lamports.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                let space = space.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;

                let (ether, rest) = rest.split_at(20);
                let ether = H160::from_slice(&*ether); //ether.try_into().map_err(|_| InvalidInstructionData)?;
                let (nonce, _rest) = rest.split_first().ok_or(InvalidInstructionData)?;
                EvmInstruction::CreateAccount {lamports, space, ether, nonce: *nonce}
            },
            // TODO: EvmInstruction::Call
            // https://github.com/neonlabsorg/neon-evm/issues/188
            // Does not fit in current vision.
            // It is needed to update behavior for all system in whole.
            // 3 => {
            //     let (collateral_pool_index, rest) = rest.split_at(4);
            //     let collateral_pool_index = collateral_pool_index.try_into().ok().map(u32::from_le_bytes).ok_or(InvalidInstructionData)?;
            //     EvmInstruction::Call {collateral_pool_index, bytes: rest}
            // },
            5 => {
                let (collateral_pool_index, rest) = rest.split_at(4);
                let collateral_pool_index = collateral_pool_index.try_into().ok().map(u32::from_le_bytes).ok_or(InvalidInstructionData)?;
                let (from_addr, rest) = rest.split_at(20);
                let (sign, unsigned_msg) = rest.split_at(65);
                EvmInstruction::CallFromRawEthereumTX {collateral_pool_index, from_addr, sign, unsigned_msg}
            },
            6 => {
                let (&status, bytes) = input.split_first().ok_or(InvalidInstructionData)?;
                EvmInstruction::OnReturn {status, bytes}
            },
            7 => {
                let (address, rest) = rest.split_at(20);
                let address = H160::from_slice(&*address); //address.try_into().map_err(|_| InvalidInstructionData)?;

                let (topics_cnt, mut rest) = rest.split_at(8);
                let topics_cnt = topics_cnt.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                let mut topics = Vec::new();
                for _ in 1..=topics_cnt {
                    let (topic, rest2) = rest.split_at(32);
                    let topic = H256::from_slice(&*topic);
                    topics.push(topic);
                    rest = rest2;
                }
                EvmInstruction::OnEvent {address, topics, data: rest}
            },
            9 => EvmInstruction::PartialCallFromRawEthereumTX,
            10 => EvmInstruction::Continue,
            11 => EvmInstruction::ExecuteTrxFromAccountDataIterative,
            12 => EvmInstruction::Cancel,
            13 => {
                let (collateral_pool_index, rest) = rest.split_at(4);
                let collateral_pool_index = collateral_pool_index.try_into().ok().map(u32::from_le_bytes).ok_or(InvalidInstructionData)?;
                let (step_count, rest) = rest.split_at(8);
                let step_count = step_count.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                let (from_addr, rest) = rest.split_at(20);
                let (sign, unsigned_msg) = rest.split_at(65);
                EvmInstruction::PartialCallOrContinueFromRawEthereumTX {collateral_pool_index, step_count, from_addr, sign, unsigned_msg}
            },
            14 => {
                let (collateral_pool_index, rest) = rest.split_at(4);
                let collateral_pool_index = collateral_pool_index.try_into().ok().map(u32::from_le_bytes).ok_or(InvalidInstructionData)?;
                let (step_count, _rest) = rest.split_at(8);
                let step_count = step_count.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                EvmInstruction::ExecuteTrxFromAccountDataIterativeOrContinue {collateral_pool_index, step_count}
            },
            15 => EvmInstruction::ERC20CreateTokenAccount,
            16 => EvmInstruction::DeleteAccount { seed: rest },
            17 => EvmInstruction::ResizeStorageAccount { seed: rest },
            18 => {
                let (holder_id, rest) = rest.split_at(8);
                let (offset, rest) = rest.split_at(4);
                let (length, rest) = rest.split_at(8);
                let holder_id = holder_id.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                let offset = offset.try_into().ok().map(u32::from_le_bytes).ok_or(InvalidInstructionData)?;
                let length = length.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                let length = usize::try_from(length).map_err(|_| InvalidInstructionData)?;
                let (bytes, _) = rest.split_at(length);
                EvmInstruction::WriteHolder { holder_id, offset, bytes}
            },
            19 => {
                let (collateral_pool_index, rest) = rest.split_at(4);
                let collateral_pool_index = collateral_pool_index.try_into().ok().map(u32::from_le_bytes).ok_or(InvalidInstructionData)?;
                let (step_count, rest) = rest.split_at(8);
                let step_count = step_count.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                let (from_addr, rest) = rest.split_at(20);
                let (sign, unsigned_msg) = rest.split_at(65);
                EvmInstruction::PartialCallFromRawEthereumTXv02 {collateral_pool_index, step_count, from_addr, sign, unsigned_msg}
            },
            20 => {
                let (collateral_pool_index, rest) = rest.split_at(4);
                let collateral_pool_index = collateral_pool_index.try_into().ok().map(u32::from_le_bytes).ok_or(InvalidInstructionData)?;
                let (step_count, _rest) = rest.split_at(8);
                let step_count = step_count.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                EvmInstruction::ContinueV02 {collateral_pool_index, step_count}
            },
            21 => {
                let (nonce, _rest) = rest.split_at(8);
                let nonce = nonce.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                EvmInstruction::CancelWithNonce {nonce}
            },
            22 => {
                let (collateral_pool_index, rest) = rest.split_at(4);
                let collateral_pool_index = collateral_pool_index.try_into().ok().map(u32::from_le_bytes).ok_or(InvalidInstructionData)?;
                let (step_count, _rest) = rest.split_at(8);
                let step_count = step_count.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                EvmInstruction::ExecuteTrxFromAccountDataIterativeV02 {collateral_pool_index, step_count}
            },
            23 => EvmInstruction::UpdateValidsTable,
            24 => EvmInstruction::Deposit,

            _ => return Err(InvalidInstructionData),
        })
    }
}

/// Creates a `OnReturn` instruction.
#[must_use]
pub fn on_return(
    myself_program_id: &Pubkey,
    status: u8,
    used_gas: u64,
    result: &[u8]
) -> Instruction {
    use core::mem;

    let cap = 2 * mem::size_of::<u8>() + mem::size_of::<u64>() + result.len();
    let mut data = Vec::with_capacity(cap);
    data.push(6_u8);
    data.push(status);
    data.extend(&used_gas.to_le_bytes());
    data.extend(result);

    Instruction {
        program_id: *myself_program_id,
        accounts: Vec::new(),
        data,
    }
}

/// Creates a `OnEvent` instruction.
#[must_use]
pub fn on_event(
    myself_program_id: &Pubkey,
    log: Log
) -> Instruction {
    let mut data = Vec::new();
    data.insert(0, 7_u8);

    data.extend_from_slice(log.address.as_bytes());

    data.extend_from_slice(&log.topics.len().to_le_bytes());
    for topic in log.topics {
        data.extend_from_slice(topic.as_bytes());
    }

    data.extend(&log.data);

    Instruction {
        program_id: *myself_program_id,
        accounts: Vec::new(),
        data,
    }
}
