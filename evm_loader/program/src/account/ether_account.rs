use std::mem::size_of;

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use evm::{H160, U256};
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;

use super::Packable;

/// Ethereum account data v3
#[derive(Debug, Default)]
pub struct Data {
    /// Ethereum address
    pub address: H160,
    /// Solana account nonce
    pub bump_seed: u8,
    /// Ethereum account nonce
    pub trx_count: u64,
    /// Neon token balance
    pub balance: U256,
    /// Account generation, increment on suicide
    pub generation: u32,
    /// Contract code size
    pub code_size: u32,
    /// Read-write lock
    pub rw_blocked: bool,
}

impl Data {
    const ADDRESS_SIZE: usize = size_of::<H160>();
    const BUMP_SEED_SIZE: usize = size_of::<u8>();
    const TRX_COUNT_SIZE: usize = size_of::<u64>();
    const BALANCE_SIZE: usize = size_of::<U256>();
    const GENERATION_SIZE: usize = size_of::<u32>();
    const CODE_SIZE_SIZE: usize = size_of::<u32>();
    const RW_BLOCKED_SIZE: usize = size_of::<bool>();

    pub fn check_blocked(&self) -> ProgramResult {
        if self.rw_blocked {
            // error message is parsed in proxy, do not change
            return Err!(ProgramError::InvalidAccountData; "trying to execute transaction on rw locked account {}", self.address);
        }

        Ok(())
    }
}

impl Packable for Data {
    /// `AccountV3` struct tag
    const TAG: u8 = super::TAG_ACCOUNT_V3;

    /// `AccountV3` struct serialized size
    const SIZE: usize = Data::ADDRESS_SIZE +
        Data::BUMP_SEED_SIZE +
        Data::TRX_COUNT_SIZE +
        Data::BALANCE_SIZE +
        Data::GENERATION_SIZE +
        Data::CODE_SIZE_SIZE +
        Data::RW_BLOCKED_SIZE;

    /// Deserialize `AccountV3` struct from input data
    #[must_use]
    fn unpack(input: &[u8]) -> Self {
        let data = array_ref![input, 0, Data::SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            address,
            bump_seed,
            trx_count,
            balance,
            generation,
            code_size,
            rw_blocked,
        ) = array_refs![
            data,
            Data::ADDRESS_SIZE,
            Data::BUMP_SEED_SIZE,
            Data::TRX_COUNT_SIZE,
            Data::BALANCE_SIZE,
            Data::GENERATION_SIZE,
            Data::CODE_SIZE_SIZE,
            Data::RW_BLOCKED_SIZE
        ];

        Self {
            address: H160::from_slice(address),
            bump_seed: bump_seed[0],
            trx_count: u64::from_le_bytes(*trx_count),
            balance: U256::from_little_endian(balance),
            generation: u32::from_le_bytes(*generation),
            code_size: u32::from_le_bytes(*code_size),
            rw_blocked: rw_blocked[0] != 0,
        }
    }

    /// Serialize `AccountV3` struct into given destination
    fn pack(&self, dst: &mut [u8]) {
        let data = array_mut_ref![dst, 0, Data::SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            address,
            bump_seed,
            trx_count,
            balance,
            generation,
            code_size,
            rw_blocked,
        ) = mut_array_refs![
            data,
            Data::ADDRESS_SIZE,
            Data::BUMP_SEED_SIZE,
            Data::TRX_COUNT_SIZE,
            Data::BALANCE_SIZE,
            Data::GENERATION_SIZE,
            Data::CODE_SIZE_SIZE,
            Data::RW_BLOCKED_SIZE
        ];

        *address = self.address.to_fixed_bytes();
        bump_seed[0] = self.bump_seed;
        *trx_count = self.trx_count.to_le_bytes();
        self.balance.to_little_endian(balance);
        *generation = self.generation.to_le_bytes();
        *code_size = self.code_size.to_le_bytes();
        rw_blocked[0] = u8::from(self.rw_blocked);
    }
}
