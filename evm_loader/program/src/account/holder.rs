use solana_program::account_info::AccountInfo;
use solana_program::pubkey::Pubkey;
use std::cell::{Ref, RefMut};
use std::mem::size_of;

use crate::account::TAG_STATE_FINALIZED;
use crate::error::{Error, Result};
use crate::types::Transaction;

use super::{Operator, HOLDER_PREFIX_LEN, TAG_EMPTY, TAG_HOLDER};

/// Ethereum holder data account
#[repr(C, packed)]
pub struct Header {
    pub owner: Pubkey,
    pub transaction_hash: [u8; 32],
    pub transaction_len: usize,
}

pub struct Holder<'a> {
    account: AccountInfo<'a>,
}

const HEADER_OFFSET: usize = HOLDER_PREFIX_LEN;
const BUFFER_OFFSET: usize = HEADER_OFFSET + size_of::<Header>();

impl<'a> Holder<'a> {
    pub fn from_account(program_id: &Pubkey, account: AccountInfo<'a>) -> Result<Self> {
        match super::tag(program_id, &account)? {
            TAG_STATE_FINALIZED => {
                super::set_tag(program_id, &account, TAG_HOLDER)?;

                let mut holder = Self { account };
                holder.clear();

                Ok(holder)
            }
            TAG_HOLDER => Ok(Self { account }),
            _ => Err(Error::AccountInvalidTag(*account.key, TAG_HOLDER)),
        }
    }

    pub fn create(
        program_id: &Pubkey,
        account: AccountInfo<'a>,
        seed: &str,
        operator: &Operator,
    ) -> Result<Self> {
        if account.owner != program_id {
            return Err(Error::AccountInvalidOwner(*account.key, *program_id));
        }

        let key = Pubkey::create_with_seed(operator.key, seed, program_id)?;
        if &key != account.key {
            return Err(Error::AccountInvalidKey(*account.key, key));
        }

        super::validate_tag(program_id, &account, TAG_EMPTY)?;
        super::set_tag(&crate::ID, &account, TAG_HOLDER)?;

        let mut holder = Self::from_account(program_id, account)?;
        holder.header_mut().owner = *operator.key;
        holder.clear();

        Ok(holder)
    }

    pub fn update<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Header),
    {
        let mut header = self.header_mut();
        f(&mut header);
    }

    fn header(&self) -> Ref<Header> {
        super::section(&self.account, HEADER_OFFSET)
    }

    fn header_mut(&mut self) -> RefMut<Header> {
        super::section_mut(&self.account, HEADER_OFFSET)
    }

    fn buffer(&self) -> Ref<[u8]> {
        let data = self.account.data.borrow();
        Ref::map(data, |d| &d[BUFFER_OFFSET..])
    }

    fn buffer_mut(&mut self) -> RefMut<[u8]> {
        let data = self.account.data.borrow_mut();
        RefMut::map(data, |d| &mut d[BUFFER_OFFSET..])
    }

    pub fn clear(&mut self) {
        {
            let mut header = self.header_mut();
            header.transaction_hash.fill(0);
            header.transaction_len = 0;
        }
        {
            let mut buffer = self.buffer_mut();
            buffer.fill(0);
        }
    }

    pub fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<()> {
        let begin = offset;
        let end = offset
            .checked_add(bytes.len())
            .ok_or(Error::IntegerOverflow)?;

        {
            let mut header = self.header_mut();
            header.transaction_len = std::cmp::max(header.transaction_len, end);
        }
        {
            let mut buffer = self.buffer_mut();
            let Some(buffer) = buffer.get_mut(begin..end) else {
                return Err(Error::HolderInsufficientSize(buffer.len(), end));
            };

            buffer.copy_from_slice(bytes);
        }

        Ok(())
    }

    #[must_use]
    pub fn transaction_len(&self) -> usize {
        self.header().transaction_len
    }

    #[must_use]
    pub fn transaction(&self) -> Ref<[u8]> {
        let len = self.transaction_len();

        let buffer = self.buffer();
        Ref::map(buffer, |b| &b[..len])
    }

    #[must_use]
    pub fn transaction_hash(&self) -> [u8; 32] {
        self.header().transaction_hash
    }

    pub fn update_transaction_hash(&mut self, hash: [u8; 32]) {
        if self.transaction_hash() == hash {
            return;
        }

        self.clear();
        self.header_mut().transaction_hash = hash;
    }

    #[must_use]
    pub fn owner(&self) -> Pubkey {
        self.header().owner
    }

    pub fn validate_owner(&self, operator: &Operator) -> Result<()> {
        if &self.owner() != operator.key {
            return Err(Error::HolderInvalidOwner(self.owner(), *operator.key));
        }

        Ok(())
    }

    pub fn validate_transaction(&self, trx: &Transaction) -> Result<()> {
        if self.transaction_hash() != trx.hash() {
            return Err(Error::HolderInvalidHash(
                self.transaction_hash(),
                trx.hash(),
            ));
        }

        Ok(())
    }

    /// # Safety
    /// Permanently deletes Holder account and all data in it
    pub unsafe fn suicide(self, operator: &Operator) {
        crate::account::delete(&self.account, operator);
    }
}
