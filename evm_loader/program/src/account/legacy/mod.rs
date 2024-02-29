mod legacy_ether;
mod legacy_holder;
mod legacy_storage_cell;

pub use legacy_ether::LegacyEtherData;
pub use legacy_holder::LegacyFinalizedData;
pub use legacy_holder::LegacyHolderData;
pub use legacy_storage_cell::LegacyStorageData;

use solana_program::system_program;
use solana_program::{account_info::AccountInfo, rent::Rent, sysvar::Sysvar};

use super::AccountHeader;
use super::Holder;
use super::HolderHeader;
use super::StateFinalizedAccount;
use super::StateFinalizedHeader;
use super::StorageCellHeader;
use super::TAG_HOLDER;
use super::TAG_STATE_FINALIZED;
use super::{AccountsDB, ContractAccount, TAG_STORAGE_CELL};
use crate::{
    account::{BalanceAccount, StorageCell},
    account_storage::KeysCache,
    error::Result,
};

// First version
pub const TAG_STATE_DEPRECATED: u8 = 22;
pub const TAG_STATE_FINALIZED_DEPRECATED: u8 = 31;
pub const TAG_HOLDER_DEPRECATED: u8 = 51;
pub const TAG_ACCOUNT_CONTRACT_DEPRECATED: u8 = 12;
pub const TAG_STORAGE_CELL_DEPRECATED: u8 = 42;
// Before account revision (Holder and Finalized remain the same)
pub const TAG_STATE_BEFORE_REVISION: u8 = 23;

fn reduce_account_size(account: &AccountInfo, required_len: usize, rent: &Rent) -> Result<u64> {
    assert!(account.data_len() > required_len);

    account.realloc(required_len, false)?;

    // Return excessive lamports to the operator
    let minimum_balance = rent.minimum_balance(account.data_len());
    if account.lamports() > minimum_balance {
        let value = account.lamports() - minimum_balance;
        **account.lamports.borrow_mut() -= value;

        Ok(value)
    } else {
        Ok(0)
    }
}

fn kill_account(account: &AccountInfo) -> Result<u64> {
    let value = account.lamports();

    **account.try_borrow_mut_lamports()? = 0;
    account.realloc(0, false)?;
    account.assign(&system_program::ID);

    Ok(value)
}

fn update_ether_account_from_v1(
    legacy_data: &LegacyEtherData,
    db: &AccountsDB,
    keys: &KeysCache,
    rent: &Rent,
) -> Result<u64> {
    let pubkey = keys.contract(&crate::ID, legacy_data.address);
    let account = db.get(&pubkey);

    let mut lamports_collected = 0_u64;

    if (legacy_data.generation > 0) || (legacy_data.code_size > 0) {
        // This is contract account. Convert it to new format
        super::validate_tag(&crate::ID, account, TAG_ACCOUNT_CONTRACT_DEPRECATED)?;

        // Read existing data
        let storage = legacy_data.read_storage(account);
        let code = legacy_data.read_code(account);

        // Make account smaller
        let required_len = ContractAccount::required_account_size(&code);
        lamports_collected += reduce_account_size(account, required_len, rent)?;

        // Fill it with new data
        account.try_borrow_mut_data()?.fill(0);

        let mut contract = ContractAccount::init(
            legacy_data.address,
            crate::config::DEFAULT_CHAIN_ID,
            legacy_data.generation,
            &code,
            db,
            Some(keys),
        )?;
        contract.set_storage_multiple_values(0, &storage);
    } else {
        // This is not contract. Just kill it.
        lamports_collected += kill_account(account)?;
    }

    if (legacy_data.balance > 0) || (legacy_data.trx_count > 0) {
        // Has balance data. Create a new account
        let mut balance = BalanceAccount::create(
            legacy_data.address,
            crate::config::DEFAULT_CHAIN_ID,
            db,
            Some(keys),
            rent,
        )?;
        balance.mint(legacy_data.balance)?;
        balance.increment_nonce_by(legacy_data.trx_count)?;
    }

    Ok(lamports_collected)
}

fn update_storage_account_from_v1(
    legacy_data: &LegacyStorageData,
    db: &AccountsDB,
    keys: &KeysCache,
    rent: &Rent,
) -> Result<u64> {
    let mut lamports_collected = 0_u64;

    let cell_pubkey = keys.storage_cell(&crate::ID, legacy_data.address, legacy_data.index);
    let cell_account = db.get(&cell_pubkey).clone();

    let contract_pubkey = keys.contract(&crate::ID, legacy_data.address);
    let contract_account = db.get(&contract_pubkey).clone();
    let contract = ContractAccount::from_account(&crate::ID, contract_account)?;

    if contract.generation() != legacy_data.generation {
        // Cell is out of date. Kill it.
        lamports_collected += kill_account(&cell_account)?;
        return Ok(lamports_collected);
    }

    let cells = legacy_data.read_cells(&cell_account);

    // Make account smaller
    let required_len = StorageCell::required_account_size(cells.len());
    lamports_collected += reduce_account_size(&cell_account, required_len, rent)?;

    // Fill it with new data
    cell_account.try_borrow_mut_data()?.fill(0);
    super::set_tag(
        &crate::ID,
        &cell_account,
        TAG_STORAGE_CELL,
        StorageCellHeader::VERSION,
    )?;

    let mut storage = StorageCell::from_account(&crate::ID, cell_account)?;
    storage.cells_mut().copy_from_slice(&cells);
    storage.increment_revision(rent, db)?;

    Ok(lamports_collected)
}

pub fn update_holder_account(account: &AccountInfo) -> Result<u8> {
    match super::tag(&crate::ID, account)? {
        TAG_HOLDER_DEPRECATED => {
            let legacy_data = LegacyHolderData::from_account(&crate::ID, account)?;

            super::set_tag(&crate::ID, account, TAG_HOLDER, HolderHeader::VERSION)?;

            let mut holder = Holder::from_account(&crate::ID, account.clone())?;
            holder.update(|data| {
                data.owner = legacy_data.owner;
                data.transaction_hash.fill(0);
                data.transaction_len = 0;
            });

            Ok(TAG_HOLDER)
        }
        TAG_STATE_FINALIZED_DEPRECATED => {
            let legacy_data = LegacyFinalizedData::from_account(&crate::ID, account)?;

            super::set_tag(
                &crate::ID,
                account,
                TAG_STATE_FINALIZED,
                StateFinalizedHeader::VERSION,
            )?;

            let mut state = StateFinalizedAccount::from_account(&crate::ID, account.clone())?;
            state.update(|data| {
                data.owner = legacy_data.owner;
                data.transaction_hash = legacy_data.transaction_hash;
            });

            Ok(TAG_STATE_FINALIZED)
        }
        tag => Ok(tag),
    }
}

pub fn update_legacy_accounts(accounts: &AccountsDB) -> Result<u64> {
    let keys = KeysCache::new();

    let mut lamports_collected = 0_u64;
    let mut legacy_storage = Vec::with_capacity(accounts.accounts_len());

    let rent = Rent::get()?;

    for account in accounts {
        if !crate::check_id(account.owner) {
            continue;
        }

        if account.data_is_empty() {
            continue;
        }

        let tag = account.try_borrow_data()?[0];
        match tag {
            LegacyEtherData::TAG => {
                let legacy_data = LegacyEtherData::from_account(&crate::ID, account)?;
                lamports_collected +=
                    update_ether_account_from_v1(&legacy_data, accounts, &keys, &rent)?;
            }
            LegacyStorageData::TAG => {
                let legacy_data = LegacyStorageData::from_account(&crate::ID, account)?;
                legacy_storage.push(legacy_data);
            }
            _ => {}
        }
    }

    for data in legacy_storage {
        lamports_collected += update_storage_account_from_v1(&data, accounts, &keys, &rent)?;
    }

    Ok(lamports_collected)
}

#[must_use]
pub fn is_legacy_tag(tag: u8) -> bool {
    matches!(
        tag,
        TAG_ACCOUNT_CONTRACT_DEPRECATED
            | TAG_STORAGE_CELL_DEPRECATED
            | TAG_HOLDER_DEPRECATED
            | TAG_STATE_FINALIZED_DEPRECATED
            | TAG_STATE_DEPRECATED
            | TAG_STATE_BEFORE_REVISION
    )
}
