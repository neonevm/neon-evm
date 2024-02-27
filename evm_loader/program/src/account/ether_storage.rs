use std::cell::{Ref, RefMut};
use std::mem::size_of;

use super::{AccountsDB, ACCOUNT_PREFIX_LEN, TAG_STORAGE_CELL};
use crate::error::Result;
use ethnum::U256;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey, rent::Rent};

#[derive(Copy, Clone)]
pub struct StorageCellAddress {
    base: Pubkey,
    seed: [u8; 32],
    pubkey: Pubkey,
}

impl StorageCellAddress {
    #[must_use]
    fn make_seed(index: &U256) -> [u8; 32] {
        let mut buffer = [0_u8; 32];

        let index_bytes = index.to_be_bytes();
        let index_bytes = &index_bytes[3..31];

        for i in 0..28 {
            buffer[i] = index_bytes[i] & 0x7F;
        }

        #[allow(clippy::needless_range_loop)]
        for i in 0..7 {
            buffer[28] |= (index_bytes[i] & 0x80) >> (1 + i);
        }
        for i in 0..7 {
            buffer[29] |= (index_bytes[7 + i] & 0x80) >> (1 + i);
        }
        for i in 0..7 {
            buffer[30] |= (index_bytes[14 + i] & 0x80) >> (1 + i);
        }
        for i in 0..7 {
            buffer[31] |= (index_bytes[21 + i] & 0x80) >> (1 + i);
        }

        buffer
    }

    #[must_use]
    pub fn new(program_id: &Pubkey, base: &Pubkey, index: &U256) -> Self {
        let seed_buffer = Self::make_seed(index);
        let seed = unsafe { std::str::from_utf8_unchecked(&seed_buffer) };

        let pubkey = Pubkey::create_with_seed(base, seed, program_id).unwrap();

        Self {
            base: *base,
            seed: seed_buffer,
            pubkey,
        }
    }

    #[must_use]
    pub fn seed(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.seed) }
    }

    #[must_use]
    pub fn pubkey(&self) -> &Pubkey {
        &self.pubkey
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct Cell {
    pub subindex: u8,
    pub value: [u8; 32],
}

pub struct StorageCell<'a> {
    account: AccountInfo<'a>,
}

const CELLS_OFFSET: usize = ACCOUNT_PREFIX_LEN;

impl<'a> StorageCell<'a> {
    #[must_use]
    pub fn required_account_size(cells: usize) -> usize {
        ACCOUNT_PREFIX_LEN + cells * size_of::<Cell>()
    }

    pub fn from_account(program_id: &Pubkey, account: AccountInfo<'a>) -> Result<Self> {
        super::validate_tag(program_id, &account, TAG_STORAGE_CELL)?;

        Ok(Self { account })
    }

    pub fn create(
        address: StorageCellAddress,
        allocate_cells: usize,
        accounts: &AccountsDB<'a>,
        signer_seeds: &[&[u8]],
        rent: &Rent,
    ) -> Result<Self> {
        let base_account = accounts.get(&address.base);
        let cell_account = accounts.get(&address.pubkey);

        assert!(allocate_cells <= u8::MAX.into());
        let space = ACCOUNT_PREFIX_LEN + (allocate_cells * size_of::<Cell>());

        let system = accounts.system();

        system.create_account_with_seed(
            &crate::ID,
            accounts.operator(),
            base_account,
            signer_seeds,
            cell_account,
            address.seed(),
            space,
            rent,
        )?;

        super::set_tag(&crate::ID, cell_account, TAG_STORAGE_CELL)?;
        Ok(Self {
            account: cell_account.clone(),
        })
    }

    #[must_use]
    pub fn pubkey(&self) -> &'a Pubkey {
        self.account.key
    }

    #[must_use]
    pub fn cells(&self) -> Ref<[Cell]> {
        let data = self.account.data.borrow();
        let data = Ref::map(data, |d| &d[CELLS_OFFSET..]);

        Ref::map(data, |bytes| {
            static_assertions::assert_eq_align!(Cell, u8);
            assert_eq!(bytes.len() % size_of::<Cell>(), 0);

            // SAFETY: Cell has the same alignment as bytes
            unsafe {
                let ptr = bytes.as_ptr().cast::<Cell>();
                let len = bytes.len() / size_of::<Cell>();
                std::slice::from_raw_parts(ptr, len)
            }
        })
    }

    #[must_use]
    pub fn cells_mut(&mut self) -> RefMut<[Cell]> {
        let data = self.account.data.borrow_mut();
        let data = RefMut::map(data, |d| &mut d[CELLS_OFFSET..]);

        RefMut::map(data, |bytes| {
            static_assertions::assert_eq_align!(Cell, u8);
            assert_eq!(bytes.len() % size_of::<Cell>(), 0);

            // SAFETY: Cell has the same alignment as bytes
            unsafe {
                let ptr = bytes.as_mut_ptr().cast::<Cell>();
                let len = bytes.len() / size_of::<Cell>();
                std::slice::from_raw_parts_mut(ptr, len)
            }
        })
    }

    #[must_use]
    pub fn get(&self, subindex: u8) -> [u8; 32] {
        for cell in &*self.cells() {
            if cell.subindex != subindex {
                continue;
            }

            return cell.value;
        }

        [0_u8; 32]
    }

    pub fn update(&mut self, subindex: u8, value: &[u8; 32]) -> Result<()> {
        // todo: if value is zero - destroy cell

        for cell in &mut *self.cells_mut() {
            if cell.subindex != subindex {
                continue;
            }

            cell.value.copy_from_slice(value);
            return Ok(());
        }

        let new_len = self.account.data_len() + std::mem::size_of::<Cell>(); // new_len <= 8.25 kb
        self.account.realloc(new_len, false)?;

        let mut cells = self.cells_mut();

        let last_cell = cells.last_mut().unwrap();
        last_cell.subindex = subindex;
        last_cell.value.copy_from_slice(value);

        Ok(())
    }

    pub fn sync_lamports(&mut self, rent: &Rent, accounts: &AccountsDB<'a>) -> Result<()> {
        let original_data_len = unsafe { self.account.original_data_len() };
        if original_data_len == self.account.data_len() {
            return Ok(());
        }

        let minimum_balance = rent.minimum_balance(self.account.data_len());
        if self.account.lamports() >= minimum_balance {
            return Ok(());
        }

        let system = accounts.system();
        let operator = accounts.operator();

        let lamports = minimum_balance - self.account.lamports();
        system.transfer(operator, &self.account, lamports)?;

        Ok(())
    }
}
