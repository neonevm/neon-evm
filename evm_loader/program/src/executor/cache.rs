use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use ethnum::U256;
use maybe_async::maybe_async;
use serde::{Deserialize, Serialize};
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

use crate::{account_storage::AccountStorage, types::Address};

#[derive(Clone, Serialize, Deserialize)]
pub struct OwnedAccountInfo {
    pub key: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
    pub lamports: u64,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub owner: Pubkey,
    pub executable: bool,
    pub rent_epoch: solana_program::clock::Epoch,
}

impl OwnedAccountInfo {
    #[must_use]
    pub fn from_account_info(program_id: &Pubkey, info: &AccountInfo) -> Self {
        Self {
            key: *info.key,
            is_signer: info.is_signer,
            is_writable: info.is_writable,
            lamports: info.lamports(),
            data: if info.executable || (info.owner == program_id) {
                // This is only used to emulate external programs
                // They don't use data in our accounts
                vec![]
            } else {
                info.data.borrow().to_vec()
            },
            owner: *info.owner,
            executable: info.executable,
            rent_epoch: info.rent_epoch,
        }
    }
}

impl<'a> solana_program::account_info::IntoAccountInfo<'a> for &'a mut OwnedAccountInfo {
    fn into_account_info(self) -> AccountInfo<'a> {
        AccountInfo {
            key: &self.key,
            is_signer: self.is_signer,
            is_writable: self.is_writable,
            lamports: Rc::new(RefCell::new(&mut self.lamports)),
            data: Rc::new(RefCell::new(&mut self.data)),
            owner: &self.owner,
            executable: self.executable,
            rent_epoch: self.rent_epoch,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Cache {
    pub solana_accounts: BTreeMap<Pubkey, OwnedAccountInfo>,
    pub native_balances: BTreeMap<(Address, u64), U256>,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub block_number: U256,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub block_timestamp: U256,
}

#[maybe_async]
#[allow(clippy::await_holding_refcell_ref)] // We don't use this RefCell<Cache> in other execution context
pub async fn cache_get_or_insert_account<B: AccountStorage>(
    cache: &RefCell<Cache>,
    key: Pubkey,
    backend: &B,
) -> OwnedAccountInfo {
    use std::collections::btree_map::Entry;

    let mut cache = cache.borrow_mut();
    match cache.solana_accounts.entry(key) {
        Entry::Vacant(entry) => {
            let owned_account_info = backend.clone_solana_account(&key).await;
            entry.insert(owned_account_info).clone()
        }
        Entry::Occupied(entry) => entry.get().clone(),
    }
}

#[maybe_async]
#[allow(clippy::await_holding_refcell_ref)] // We don't use this RefCell<Cache> in other execution context
pub async fn cache_get_or_insert_balance<B: AccountStorage>(
    cache: &RefCell<Cache>,
    key: (Address, u64),
    backend: &B,
) -> U256 {
    use std::collections::btree_map::Entry;

    let mut cache = cache.borrow_mut();

    match cache.native_balances.entry(key) {
        Entry::Vacant(entry) => {
            let balance = backend.balance(key.0, key.1).await;
            *entry.insert(balance)
        }
        Entry::Occupied(entry) => *entry.get(),
    }
}
