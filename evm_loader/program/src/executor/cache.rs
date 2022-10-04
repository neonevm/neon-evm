use std::{collections::BTreeMap, rc::Rc, cell::RefCell};

use borsh::{BorshSerialize, BorshDeserialize};
use solana_program::{pubkey::Pubkey, account_info::AccountInfo};
use evm::{U256};

use crate::account_storage::AccountStorage;


#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct OwnedAccountInfo {
    pub key: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
    pub lamports: u64,
    pub data: Vec<u8>,
    pub owner: Pubkey,
    pub executable: bool,
    pub rent_epoch: solana_program::clock::Epoch,
}

#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct OwnedAccountInfoPartial {
    pub key: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
    pub lamports: u64,
    pub data: Vec<u8>,
    pub data_offset: usize,
    pub data_total_len: usize,
    pub owner: Pubkey,
    pub executable: bool,
    pub rent_epoch: solana_program::clock::Epoch,
}

impl OwnedAccountInfoPartial {
    #[must_use]
    pub fn from_account_info(info: &AccountInfo, offset: usize, len: usize) -> Option<Self> {
        let data = info.data.borrow();

        if offset.saturating_add(len) > data.len() {
            return None;
        }

        Some(Self { 
            key: *info.key,
            is_signer: info.is_signer,
            is_writable: info.is_writable,
            lamports: info.lamports(),
            data: data[offset .. offset+len].to_vec(),
            data_offset: offset,
            data_total_len: data.len(),
            owner: *info.owner,
            executable: info.executable,
            rent_epoch: info.rent_epoch,
        })
    }
}

impl OwnedAccountInfo {
    #[must_use]
    pub fn from_account_info(info: &AccountInfo) -> Self {
        Self { 
            key: *info.key,
            is_signer: info.is_signer,
            is_writable: info.is_writable,
            lamports: info.lamports(),
            data: info.data.borrow().to_vec(),
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

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct AccountMeta {
    pub key: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl AccountMeta {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn from_solana_meta(meta: solana_program::instruction::AccountMeta) -> Self {
        Self {
            key: meta.pubkey,
            is_signer: meta.is_signer,
            is_writable: meta.is_writable
        }
    }

    #[must_use]
    pub fn into_solana_meta(self) -> solana_program::instruction::AccountMeta {
        solana_program::instruction::AccountMeta {
            pubkey: self.key,
            is_signer: self.is_signer,
            is_writable: self.is_writable,
        }
    }
}



#[derive(BorshSerialize, BorshDeserialize)]
pub struct Cache {
    pub solana_accounts: BTreeMap<Pubkey, OwnedAccountInfo>,
    pub solana_accounts_partial: BTreeMap<Pubkey, OwnedAccountInfoPartial>,
    pub block_number: U256,
    pub block_timestamp: U256,
}

impl Cache {
    pub fn get_account_or_insert<B: AccountStorage>(&mut self, key: Pubkey, backend: &B) -> &mut OwnedAccountInfo {
        self.solana_accounts.entry(key).or_insert_with(|| backend.clone_solana_account(&key))
    }
}
