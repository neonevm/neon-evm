use serde::{Serialize, Serializer, Deserialize};
use solana_program::{program_error::ProgramError, pubkey::Pubkey, instruction::Instruction};
use std::convert::TryInto;
use primitive_types::{H160, H256};
use evm::backend::Log;

fn serialize_h160<S>(value: &H160, s: S) -> Result<S::Ok, S::Error> where S: Serializer {
    value.as_fixed_bytes().serialize(s)
}

/// Create a new account
#[derive(Serialize, Debug, PartialEq, Eq, Clone)]
pub enum EvmInstruction<'a> {
    /// Write program data into an account
    ///
    /// # Account references
    ///   0. [WRITE] Account to write to
    ///   1. [SIGNER] Signer for Ether account
    Write {
        /// Offset at which to write the given bytes
        offset: u32,
        bytes: &'a [u8],
    },

    /// Finalize an account loaded with program data for execution
    ///
    /// The exact preparation steps is loader specific but on success the loader must set the executable
    /// bit of the account.
    ///
    /// # Account references
    ///   0. [WRITE] The account to prepare for execution
    ///   1. [WRITE] Contract code account (Code account)
    ///   2. [WRITE] Caller (Ether account)
    ///   3. [SIGNER] Signer for Ether account
    ///   4. [] Clock sysvar
    ///   5. [] Rent sysvar
    ///   ... other Ether accounts
    Finalize,

    ///
    /// Create Ethereum account (create program_address account and write data)
    /// # Account references
    ///   0. [WRITE, SIGNER] Funding account
    ///   1. [WRITE] New account (program_address(ether, nonce))
    ///   2. (for contract creation) [WRITE] Code account for new contract account
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
    /// # Account references
    ///   0. [WRITE] Contract account for execution (Ether account)
    ///   1. [WRITE] Contract code account (Code account)
    ///   2. [WRITE] Caller (Ether account)
    ///   3. [SIGNER] Signer for caller
    ///   4. [] Clock sysvar
    ///   ... other Ether accounts
    Call {
        /// Call data
        bytes: &'a [u8],
    },

    /// Execute Ethereum transaction from account data
    /// # Account references
    ///   0. [] The account with transaction for execution
    ///   1. [WRITE] Caller (Ether account)
    ///   ... another accounts
    ///   2. [] Clock sysvar
    ExecuteTrxFromAccountData,

    ///
    /// Create ethereum account with seed
    /// # Account references
    ///   0. [WRITE, SIGNER] Funding account
    ///   1. [WRITE] New account (create_with_seed(base, seed, owner)
    ///   2. [] Base (program_addres(ether, nonce))
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
    },

    /// Call Ethereum-contract action from raw transaction data
    /// # Account references same as in Call
    CallFromRawEthereumTX {
        /// Call data
        from_addr: &'a [u8],
        sign: &'a [u8],
        unsigned_msg: &'a [u8],
    },

    /// Call Ethereum-contract action from raw transaction data
    /// # Account references same as in Call
    CheckEtheriumTX {
        /// Call data
        from_addr: &'a [u8],
        sign: &'a [u8],
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
        address: H160,
        topics: Vec<H256>,
        /// Data
        data: &'a [u8],
    },

    PartialCallFromRawEthereumTX {
        step_count: u64,
        from_addr: &'a [u8],
        sign: &'a [u8],
        unsigned_msg: &'a [u8],
    },

    Continue {
        step_count: u64,
    },

    KeccakSysCall,

    KeccakSHA3,
}


impl<'a> EvmInstruction<'a> {
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
                let (bytes, _) = rest.split_at(length as usize);
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
                let (seed_len, rest) = rest.split_at(4);
                let (_, rest) = rest.split_at(4);  // padding
                let seed_len = seed_len.try_into().ok().map(u32::from_le_bytes).ok_or(InvalidInstructionData)?;
                let (seed, rest) = rest.split_at(seed_len as usize);

                let base = Pubkey::new(base);
                let (lamports, rest) = rest.split_at(8);
                let (space, rest) = rest.split_at(8);

                let (owner, rest) = rest.split_at(32);
                let owner = Pubkey::new(owner);

                let seed = seed.into();
                let lamports = lamports.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                let space = space.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;

                EvmInstruction::CreateAccountWithSeed {base, seed, lamports, space, owner}
            },
            5 => {
                let (from_addr, rest) = rest.split_at(20);
                let (sign, unsigned_msg) = rest.split_at(65);
                EvmInstruction::CallFromRawEthereumTX {from_addr, sign, unsigned_msg}
            },
            0xa1 => {
                let (from_addr, rest) = rest.split_at(20);
                let (sign, unsigned_msg) = rest.split_at(65);
                EvmInstruction::CheckEtheriumTX {from_addr, sign, unsigned_msg}
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
                for i in 1..=topics_cnt {
                    let (topic, rest2) = rest.split_at(32);
                    let topic = H256::from_slice(&*topic);
                    topics.push(topic);
                    rest = rest2;
                }
                EvmInstruction::OnEvent {address, topics, data: rest}
            },
            8 => {
                EvmInstruction::ExecuteTrxFromAccountData
            },
            9 => {
                let (step_count, rest) = rest.split_at(8);
                let step_count = step_count.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                let (from_addr, rest) = rest.split_at(20);
                let (sign, unsigned_msg) = rest.split_at(65);
                EvmInstruction::PartialCallFromRawEthereumTX {step_count, from_addr, sign, unsigned_msg}
            },
            10 => {
                let (step_count, _rest) = rest.split_at(8);
                let step_count = step_count.try_into().ok().map(u64::from_le_bytes).ok_or(InvalidInstructionData)?;
                EvmInstruction::Continue {step_count}
            },

            0xb1 => EvmInstruction::KeccakSysCall,
        
            0xb2 => EvmInstruction::KeccakSHA3,

            _ => return Err(InvalidInstructionData),
        })
    }
}

/// Creates a `OnReturn` instruction.
pub fn on_return(
    myself_program_id: &Pubkey,
    status: u8,
    result: &Vec<u8>
) -> Result<Instruction, ProgramError> {
    let mut data = Vec::with_capacity(2 + result.len());
    data.push(6u8);
    data.push(status);
    data.extend(result);

    Ok(Instruction {
        program_id: *myself_program_id,
        accounts: [].to_vec(),
        data: data,
    })
}

/// Creates a `OnEvent` instruction.
pub fn on_event(
    myself_program_id: &Pubkey,
    log: Log
) -> Result<Instruction, ProgramError> {
    let mut data = Vec::new();
    data.insert(0, 7u8);

    data.extend_from_slice(log.address.as_bytes());

    data.extend_from_slice(&log.topics.len().to_le_bytes());
    for topic in log.topics {
        data.extend_from_slice(topic.as_bytes());
    }

    data.extend(&log.data);

    Ok(Instruction {
        program_id: *myself_program_id,
        accounts: [].to_vec(),
        data,
    })
}
