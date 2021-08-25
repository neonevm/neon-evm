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
    /// Write program data into an account
    ///
    /// # Account references
    ///   0. \[WRITE\] Account to write to
    ///   1. \[SIGNER\] Signer for Ether account
    Write {
        /// Offset at which to write the given bytes
        offset: u32,
        /// Data to write
        bytes: &'a [u8],
    },

    /// Finalize an account loaded with program data for execution
    ///
    /// The exact preparation steps is loader specific but on success the loader must set the executable
    /// bit of the account.
    ///
    /// # Account references
    ///   0. \[WRITE\] The account to prepare for execution
    ///   1. \[WRITE\] Contract code account (Code account)
    ///   2. \[WRITE\] Caller (Ether account)
    ///   3. \[SIGNER\] Signer for Ether account
    ///   4. \[\] Clock sysvar
    ///   5. \[\] Rent sysvar
    ///   ... other Ether accounts
    Finalize,

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

    /// Call Ethereum-contract action
    /// ### Account references
    ///   0. \[WRITE\] Contract account for execution (Ether account)
    ///   1. \[WRITE\] Contract code account (Code account)
    ///   2. \[WRITE\] Caller (Ether account)
    ///   3. \[SIGNER\] Signer for caller
    ///   4. \[\] Clock sysvar
    ///   ... other Ether accounts
    Call {
        /// Call data
        bytes: &'a [u8],
    },

    ///
    /// Create ethereum account with seed
    /// # Account references
    ///   0. [WRITE, SIGNER] Funding account
    ///   1. \[WRITE\] New account (create_with_seed(base, seed, owner)
    ///   2. \[\] Base (program_address(ether, nonce))
    CreateAccountWithSeed {
        /// Base public key
        base: Pubkey,

        /// String of ASCII chars, no longer than `Pubkey::MAX_SEED_LEN`
        seed: Vec<u8>,

        /// Number of lamports to transfer to the new account
        lamports: u64,

        /// Number of bytes of memory to allocate
        space: u64,

        /// Owner program account address
        owner: Pubkey,

        /// Associated token address to create
        token: Option<Pubkey>,
    },

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

    /// Partial call Ethereum-contract action from raw transaction data
    /// ### Account references
    ///   0. \[WRITE\] storage account
    ///   1. ... Account references same as in Call
    PartialCallFromRawEthereumTX {
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

    /// Partial call Ethereum-contract action from raw transaction data
    /// ### Account references same as in PartialCallFromRawEthereumTX
    Continue {
        /// Steps of ethereum contract to execute
        step_count: u64,
    },

    /// Partial call Ethereum-contract action from raw transaction data stored in holder account data
    ExecuteTrxFromAccountDataIterative {
        /// Seed index for a collateral pool account
        collateral_pool_index: u32,
        /// Steps of ethereum contract to execute
        step_count: u64,
    },

    /// Partial call Ethereum-contract action from raw transaction data
    /// ### Account references same as in PartialCallFromRawEthereumTX
    Cancel,
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
            0 => {
                let (_, rest) = rest.split_at(3);
                let (offset, rest) = rest.split_at(4);
                let (length, rest) = rest.split_at(8);
                let offset = offset.try_into().ok().map(u32::from_le_bytes).ok_or(InvalidInstructionData)?;
                let length = length.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                let length = usize::try_from(length).map_err(|_| InvalidInstructionData)?;
                let (bytes, _) = rest.split_at(length);
                EvmInstruction::Write {offset, bytes}
            },
            1 => {
                let (_, _rest) = rest.split_at(3);
                EvmInstruction::Finalize
            },
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
            3 => {
                EvmInstruction::Call {bytes: rest}
            },
            4 => {
                let (_, rest) = rest.split_at(3);
                let (base, rest) = rest.split_at(32);
                let (seed_len, rest) = rest.split_at(8);
                let seed_len = seed_len.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                let (seed, rest) = rest.split_at(seed_len as usize);

                let base = Pubkey::new(base);
                let (lamports, rest) = rest.split_at(8);
                let (space, rest) = rest.split_at(8);

                let (owner, rest) = rest.split_at(32);
                let owner = Pubkey::new(owner);

                let token = if rest.len() >= 32 {
                    let (token, _rest) = rest.split_at(32);
                    let token = Pubkey::new(token);
                    Some(token)
                } else {
                    None
                };

                let seed = seed.into();
                let lamports = lamports.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                let space = space.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;

                EvmInstruction::CreateAccountWithSeed {base, seed, lamports, space, owner, token}
            },
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
            9 => {
                let (collateral_pool_index, rest) = rest.split_at(4);
                let collateral_pool_index = collateral_pool_index.try_into().ok().map(u32::from_le_bytes).ok_or(InvalidInstructionData)?;
                let (step_count, rest) = rest.split_at(8);
                let step_count = step_count.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                let (from_addr, rest) = rest.split_at(20);
                let (sign, unsigned_msg) = rest.split_at(65);
                EvmInstruction::PartialCallFromRawEthereumTX {collateral_pool_index, step_count, from_addr, sign, unsigned_msg}
            },
            10 => {
                let (step_count, _rest) = rest.split_at(8);
                let step_count = step_count.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                EvmInstruction::Continue {step_count}
            },
            11 => {
                let (collateral_pool_index, rest) = rest.split_at(4);
                let collateral_pool_index = collateral_pool_index.try_into().ok().map(u32::from_le_bytes).ok_or(InvalidInstructionData)?;
                let (step_count, _rest) = rest.split_at(8);
                let step_count = step_count.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                EvmInstruction::ExecuteTrxFromAccountDataIterative {collateral_pool_index, step_count}
            },
            12 => {
                EvmInstruction::Cancel
            },
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
