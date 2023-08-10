use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use ethnum::U256;
#[cfg(not(feature = "library"))]
use serde::{Deserialize, Serialize};
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

#[derive(Clone)]
#[cfg_attr(not(feature = "library"), derive(Serialize, Deserialize))]
pub struct OwnedAccountInfo {
    pub key: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
    pub lamports: u64,
    #[cfg_attr(not(feature = "library"), serde(with = "serde_bytes"))]
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

#[cfg_attr(not(feature = "library"), derive(Serialize, Deserialize))]
pub struct Cache {
    pub solana_accounts: BTreeMap<Pubkey, OwnedAccountInfo>,
    #[cfg_attr(not(feature = "library"), serde(with = "ethnum::serde::bytes::le"))]
    pub block_number: U256,
    #[cfg_attr(not(feature = "library"), serde(with = "ethnum::serde::bytes::le"))]
    pub block_timestamp: U256,
}
