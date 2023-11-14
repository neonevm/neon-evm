use std::mem::size_of;

use ethnum::U256;
use solana_program::account_info::AccountInfo;
use solana_program::pubkey::Pubkey;

use crate::account::ether_storage::Cell;
use crate::error::Result;
use crate::types::Address;

pub struct LegacyStorageData {
    pub address: Address,
    pub generation: u32,
    pub index: U256,
}

impl LegacyStorageData {
    /// Storage struct tag
    pub const TAG: u8 = super::TAG_STORAGE_CELL_DEPRECATED;
    /// Storage struct serialized size
    pub const SIZE: usize = 20 + 4 + 32;

    #[must_use]
    pub fn unpack(input: &[u8]) -> Self {
        let data = arrayref::array_ref![input, 0, LegacyStorageData::SIZE];
        let (address, generation, index) = arrayref::array_refs![data, 20, 4, 32];

        Self {
            address: Address(*address),
            generation: u32::from_le_bytes(*generation),
            index: U256::from_le_bytes(*index),
        }
    }

    pub fn from_account(program_id: &Pubkey, account: &AccountInfo) -> Result<Self> {
        crate::account::validate_tag(program_id, account, Self::TAG)?;

        let data = account.try_borrow_data()?;
        Ok(Self::unpack(&data[1..]))
    }

    #[allow(clippy::unused_self)]
    #[must_use]
    pub fn read_cells(&self, account: &AccountInfo) -> Vec<Cell> {
        let cells_offset = 1 + Self::SIZE;

        let data = account.data.borrow();
        let data = &data[cells_offset..];

        static_assertions::assert_eq_align!(Cell, u8);
        assert_eq!(data.len() % size_of::<Cell>(), 0);

        // SAFETY: Cell has the same alignment as data
        let cells = unsafe {
            let ptr = data.as_ptr().cast::<Cell>();
            let len = data.len() / size_of::<Cell>();
            std::slice::from_raw_parts(ptr, len)
        };

        cells.to_vec()
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn read_value(&self, subindex: u8, account: &AccountInfo) -> [u8; 32] {
        let cells = self.read_cells(account);

        for cell in cells {
            if cell.subindex != subindex {
                continue;
            }

            return cell.value;
        }

        [0_u8; 32]
    }
}
