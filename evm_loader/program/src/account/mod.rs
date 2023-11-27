use crate::error::{Error, Result};
use solana_program::account_info::AccountInfo;
use solana_program::pubkey::Pubkey;
use std::cell::{Ref, RefMut};

pub use crate::config::ACCOUNT_SEED_VERSION;

pub use ether_balance::BalanceAccount;
pub use ether_contract::{AllocateResult, ContractAccount};
pub use ether_storage::{StorageCell, StorageCellAddress};
pub use holder::Holder;
pub use incinerator::Incinerator;
pub use operator::Operator;
pub use state::StateAccount;
pub use state_finalized::StateFinalizedAccount;
pub use treasury::{MainTreasury, Treasury};

use self::program::System;

mod ether_balance;
mod ether_contract;
mod ether_storage;
mod holder;
mod incinerator;
pub mod legacy;
mod operator;
pub mod program;
mod state;
mod state_finalized;
pub mod token;
mod treasury;

pub const TAG_EMPTY: u8 = 0;
pub const TAG_STATE: u8 = 23;
pub const TAG_STATE_FINALIZED: u8 = 32;
pub const TAG_HOLDER: u8 = 52;

pub const TAG_ACCOUNT_BALANCE: u8 = 60;
pub const TAG_ACCOUNT_CONTRACT: u8 = 70;
pub const TAG_STORAGE_CELL: u8 = 43;

const ACCOUNT_PREFIX_LEN: usize = 2;

#[inline]
fn section<'r, T>(account: &'r AccountInfo<'_>, offset: usize) -> Ref<'r, T> {
    let begin = offset;
    let end = begin + std::mem::size_of::<T>();

    let data = account.data.borrow();
    Ref::map(data, |d| {
        let bytes = &d[begin..end];

        assert_eq!(std::mem::align_of::<T>(), 1);
        assert_eq!(std::mem::size_of::<T>(), bytes.len());
        unsafe { &*(bytes.as_ptr().cast()) }
    })
}

#[inline]
fn section_mut<'r, T>(account: &'r AccountInfo<'_>, offset: usize) -> RefMut<'r, T> {
    let begin = offset;
    let end = begin + std::mem::size_of::<T>();

    let data = account.data.borrow_mut();
    RefMut::map(data, |d| {
        let bytes = &mut d[begin..end];

        assert_eq!(std::mem::align_of::<T>(), 1);
        assert_eq!(std::mem::size_of::<T>(), bytes.len());
        unsafe { &mut *(bytes.as_mut_ptr().cast()) }
    })
}

pub fn tag(program_id: &Pubkey, info: &AccountInfo) -> Result<u8> {
    if info.owner != program_id {
        return Err(Error::AccountInvalidOwner(*info.key, *program_id));
    }

    let data = info.try_borrow_data()?;
    if data.len() < ACCOUNT_PREFIX_LEN {
        return Err(Error::AccountInvalidData(*info.key));
    }

    Ok(data[0])
}

pub fn set_tag(program_id: &Pubkey, info: &AccountInfo, tag: u8) -> Result<()> {
    assert_eq!(info.owner, program_id);

    let mut data = info.try_borrow_mut_data()?;
    assert!(data.len() >= ACCOUNT_PREFIX_LEN);

    data[0] = tag;

    Ok(())
}

pub fn validate_tag(program_id: &Pubkey, info: &AccountInfo, tag: u8) -> Result<()> {
    let account_tag = crate::account::tag(program_id, info)?;

    if account_tag == tag {
        Ok(())
    } else {
        Err(Error::AccountInvalidTag(*info.key, tag))
    }
}

pub fn is_blocked(program_id: &Pubkey, info: &AccountInfo) -> Result<bool> {
    if info.owner != program_id {
        return Err(Error::AccountInvalidOwner(*info.key, *program_id));
    }

    let data = info.try_borrow_data()?;
    if data.len() < ACCOUNT_PREFIX_LEN {
        return Err(Error::AccountInvalidData(*info.key));
    }

    if legacy::is_legacy_tag(data[0]) {
        return Err(Error::AccountLegacy(*info.key));
    }

    Ok(data[1] == 1)
}

#[inline]
fn set_block(program_id: &Pubkey, info: &AccountInfo, block: bool) -> Result<()> {
    assert_eq!(info.owner, program_id);

    let mut data = info.try_borrow_mut_data()?;
    if data.len() < ACCOUNT_PREFIX_LEN {
        return Err(Error::AccountInvalidData(*info.key));
    }

    if legacy::is_legacy_tag(data[0]) {
        return Err(Error::AccountLegacy(*info.key));
    }

    if block && (data[1] != 0) {
        return Err(Error::AccountBlocked(*info.key));
    }

    data[1] = block.into();

    Ok(())
}

pub fn block(program_id: &Pubkey, info: &AccountInfo) -> Result<()> {
    set_block(program_id, info, true)
}

pub fn unblock(program_id: &Pubkey, info: &AccountInfo) -> Result<()> {
    set_block(program_id, info, false)
}

/// # Safety
/// *Permanently delete all data* in the account. Transfer lamports to the operator.
pub unsafe fn delete(account: &AccountInfo, operator: &Operator) {
    debug_print!("DELETE ACCOUNT {}", account.key);

    **operator.lamports.borrow_mut() += account.lamports();
    **account.lamports.borrow_mut() = 0;

    let mut data = account.data.borrow_mut();
    data.fill(0);
}

pub struct AccountsDB<'a> {
    sorted_accounts: Vec<AccountInfo<'a>>,
    operator: Operator<'a>,
    operator_balance: Option<BalanceAccount<'a>>,
    system: Option<System<'a>>,
    treasury: Option<Treasury<'a>>,
}

impl<'a> AccountsDB<'a> {
    #[must_use]
    pub fn new(
        accounts: &[AccountInfo<'a>],
        operator: Operator<'a>,
        operator_balance: Option<BalanceAccount<'a>>,
        system: Option<System<'a>>,
        treasury: Option<Treasury<'a>>,
    ) -> Self {
        let mut sorted_accounts = accounts.to_vec();
        sorted_accounts.sort_unstable_by_key(|a| a.key);
        sorted_accounts.dedup_by_key(|a| a.key);

        Self {
            sorted_accounts,
            operator,
            operator_balance,
            system,
            treasury,
        }
    }

    #[must_use]
    pub fn accounts_len(&self) -> usize {
        self.sorted_accounts.len()
    }

    #[must_use]
    pub fn system(&self) -> &System<'a> {
        if let Some(system) = &self.system {
            return system;
        }

        panic!("System Account must be present in the transaction");
    }

    #[must_use]
    pub fn treasury(&self) -> &Treasury<'a> {
        if let Some(treasury) = &self.treasury {
            return treasury;
        }

        panic!("Treasury Account must be present in the transaction");
    }

    #[must_use]
    pub fn operator(&self) -> &Operator<'a> {
        &self.operator
    }

    #[must_use]
    pub fn operator_balance(&mut self) -> &mut BalanceAccount<'a> {
        if let Some(operator_balance) = &mut self.operator_balance {
            return operator_balance;
        }

        panic!("Operator Balance Account must be present in the transaction");
    }

    #[must_use]
    pub fn operator_key(&self) -> Pubkey {
        *self.operator.key
    }

    #[must_use]
    pub fn operator_info(&self) -> &AccountInfo<'a> {
        &self.operator
    }

    #[must_use]
    pub fn get(&self, pubkey: &Pubkey) -> &AccountInfo<'a> {
        let Ok(index) = self.sorted_accounts.binary_search_by_key(&pubkey, |a| a.key) else {
            panic!("address {pubkey} must be present in the transaction");
        };

        // We just got an 'index' from the binary_search over this vector.
        unsafe { self.sorted_accounts.get_unchecked(index) }
    }
}

impl<'a, 'r> IntoIterator for &'r AccountsDB<'a> {
    type Item = &'r AccountInfo<'a>;
    type IntoIter = std::slice::Iter<'r, AccountInfo<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.sorted_accounts.iter()
    }
}
