use std::mem::size_of;

use ethnum::U256;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

use crate::{config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT, error::Result, types::Address};

pub struct LegacyEtherData {
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

impl LegacyEtherData {
    const ADDRESS_SIZE: usize = size_of::<Address>();
    const BUMP_SEED_SIZE: usize = size_of::<u8>();
    const TRX_COUNT_SIZE: usize = size_of::<u64>();
    const BALANCE_SIZE: usize = size_of::<U256>();
    const GENERATION_SIZE: usize = size_of::<u32>();
    const CODE_SIZE_SIZE: usize = size_of::<u32>();
    const RW_BLOCKED_SIZE: usize = size_of::<bool>();

    /// `AccountV3` struct tag
    pub const TAG: u8 = super::TAG_ACCOUNT_CONTRACT_DEPRECATED;

    /// `AccountV3` struct serialized size
    pub const SIZE: usize = Self::ADDRESS_SIZE
        + Self::BUMP_SEED_SIZE
        + Self::TRX_COUNT_SIZE
        + Self::BALANCE_SIZE
        + Self::GENERATION_SIZE
        + Self::CODE_SIZE_SIZE
        + Self::RW_BLOCKED_SIZE;

    #[must_use]
    pub fn unpack(input: &[u8]) -> Self {
        let data = arrayref::array_ref![input, 0, LegacyEtherData::SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (address, bump_seed, trx_count, balance, generation, code_size, rw_blocked) = arrayref::array_refs![
            data,
            LegacyEtherData::ADDRESS_SIZE,
            LegacyEtherData::BUMP_SEED_SIZE,
            LegacyEtherData::TRX_COUNT_SIZE,
            LegacyEtherData::BALANCE_SIZE,
            LegacyEtherData::GENERATION_SIZE,
            LegacyEtherData::CODE_SIZE_SIZE,
            LegacyEtherData::RW_BLOCKED_SIZE
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

    pub fn from_account(program_id: &Pubkey, account: &AccountInfo) -> Result<Self> {
        crate::account::validate_tag(program_id, account, Self::TAG)?;

        let data = account.try_borrow_data()?;
        Ok(Self::unpack(&data[1..]))
    }

    #[allow(clippy::unused_self)]
    #[must_use]
    pub fn read_storage(&self, account: &AccountInfo) -> Vec<[u8; 32]> {
        if self.code_size == 0 {
            return vec![[0; 32]; STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT];
        }

        let data = account.data.borrow();

        let storage_offset = 1 + Self::SIZE;
        let storage_len = 32 * STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT;

        let storage = &data[storage_offset..][..storage_len];
        let storage = unsafe {
            // storage_len is multiple of STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT
            // [u8; 32] has the same alignment as u8
            let ptr: *const [u8; 32] = storage.as_ptr().cast();
            std::slice::from_raw_parts(ptr, STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT)
        };

        storage.to_vec()
    }

    #[must_use]
    pub fn read_code(&self, account: &AccountInfo) -> Vec<u8> {
        if self.code_size == 0 {
            return Vec::new();
        }

        let data = account.data.borrow();

        let storage_offset = 1 + Self::SIZE;
        let storage_len = 32 * STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT;

        let code_offset = storage_offset + storage_len;
        let code_len = self.code_size as usize;

        let code = &data[code_offset..][..code_len];
        code.to_vec()
    }
}
