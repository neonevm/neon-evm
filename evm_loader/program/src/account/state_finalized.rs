use std::cell::{Ref, RefMut};

use super::{AccountHeader, Operator, StateAccount, TAG_STATE_FINALIZED};
use crate::{
    error::{Error, Result},
    types::Transaction,
};
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

/// Storage data account to store execution metainfo between steps for iterative execution
#[repr(C, packed)]
pub struct Header {
    pub owner: Pubkey,
    pub transaction_hash: [u8; 32],
}

impl AccountHeader for Header {
    const VERSION: u8 = 0;
}

pub struct StateFinalizedAccount<'a> {
    account: AccountInfo<'a>,
}

impl<'a> StateFinalizedAccount<'a> {
    pub fn convert_from_state<'s>(
        program_id: &Pubkey,
        state: StateAccount<'s>,
    ) -> Result<AccountInfo<'s>> {
        let owner = state.owner();
        let transaction_hash = state.trx().hash();

        let account = state.into_account();

        super::set_tag(program_id, &account, TAG_STATE_FINALIZED, Header::VERSION)?;
        {
            let mut header = super::header_mut::<Header>(&account);
            header.owner = owner;
            header.transaction_hash = transaction_hash;
        }

        Ok(account)
    }

    pub fn from_account(program_id: &Pubkey, account: AccountInfo<'a>) -> Result<Self> {
        super::validate_tag(program_id, &account, TAG_STATE_FINALIZED)?;
        Ok(Self { account })
    }

    #[inline]
    #[must_use]
    fn header(&self) -> Ref<Header> {
        super::header(&self.account)
    }

    #[inline]
    #[must_use]
    fn header_mut(&mut self) -> RefMut<Header> {
        super::header_mut(&self.account)
    }

    pub fn update<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Header),
    {
        let mut header = self.header_mut();
        f(&mut header);
    }

    #[must_use]
    pub fn owner(&self) -> Pubkey {
        self.header().owner
    }

    #[must_use]
    pub fn trx_hash(&self) -> [u8; 32] {
        self.header().transaction_hash
    }

    pub fn validate_owner(&self, operator: &Operator) -> Result<()> {
        if &self.owner() != operator.key {
            return Err(Error::HolderInvalidOwner(self.owner(), *operator.key));
        }

        Ok(())
    }

    pub fn validate_trx(&self, transaction: &Transaction) -> Result<()> {
        if self.trx_hash() == transaction.hash {
            return Err(Error::StorageAccountFinalized);
        }

        Ok(())
    }
}
