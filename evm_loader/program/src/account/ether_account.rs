use std::mem::size_of;

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use ethnum::U256;
use solana_program::account_info::AccountInfo;
use solana_program::{entrypoint::ProgramResult, pubkey::Pubkey};
use solana_program::program_error::ProgramError;

use crate::types::Address;

use super::{Operator, ACCOUNT_SEED_VERSION};
use super::{Packable, EthereumAccount, program::System};

/// Ethereum account data v3
#[derive(Debug, Default)]
pub struct Data {
    /// Ethereum address
    pub address: Address,
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
    const ADDRESS_SIZE: usize = size_of::<Address>();
    const BUMP_SEED_SIZE: usize = size_of::<u8>();
    const TRX_COUNT_SIZE: usize = size_of::<u64>();
    const BALANCE_SIZE: usize = size_of::<U256>();
    const GENERATION_SIZE: usize = size_of::<u32>();
    const CODE_SIZE_SIZE: usize = size_of::<u32>();
    const RW_BLOCKED_SIZE: usize = size_of::<bool>();
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
            address: Address::from(*address),
            bump_seed: bump_seed[0],
            trx_count: u64::from_le_bytes(*trx_count),
            balance: U256::from_le_bytes(*balance),
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

        *address = self.address.into();
        bump_seed[0] = self.bump_seed;
        *trx_count = self.trx_count.to_le_bytes();
        *balance = self.balance.to_le_bytes();
        *generation = self.generation.to_le_bytes();
        *code_size = self.code_size.to_le_bytes();
        rw_blocked[0] = u8::from(self.rw_blocked);
    }
}


impl<'a> EthereumAccount<'a> {
    pub fn check_blocked(&self) -> ProgramResult {
        if self.rw_blocked {
            // error message is parsed in proxy, do not change
            return Err!(ProgramError::InvalidAccountData; "trying to execute transaction on rw locked account {}", self.address);
        }

        Ok(())
    }
    
    pub fn create_account(
        system_program: &System<'a>,
        program_id: &Pubkey,
        operator: &Operator<'a>,
        address: &Address,
        info: &'a AccountInfo<'a>,
        bump_seed: u8,
        space: usize,
    ) -> ProgramResult {
        if space < EthereumAccount::SIZE {
            return Err!(
                ProgramError::AccountDataTooSmall;
                "Account {} - account space must be not less than minimal size of {} bytes",
                address,
                EthereumAccount::SIZE
            )
        }

        let program_seeds: &[&[u8]] = &[
            &[ACCOUNT_SEED_VERSION],
            address.as_bytes(),
            &[bump_seed],
        ];
        system_program.create_pda_account(
            program_id,
            operator,
            info,
            program_seeds,
            space,
        )?;

        Ok(())
    }

    pub fn create_and_init_account(
        system_program: &System<'a>,
        program_id: &Pubkey,
        operator: &Operator<'a>,
        address: Address,
        info: &'a AccountInfo<'a>,
        bump_seed: u8,
        space: usize,
    ) -> ProgramResult {
        Self::create_account(system_program, program_id, operator, &address, info, bump_seed, space)?;

        EthereumAccount::init(
            info,
            Data {
                address,
                bump_seed,
                ..Default::default()
            },
        )?;

        Ok(())
    }
}