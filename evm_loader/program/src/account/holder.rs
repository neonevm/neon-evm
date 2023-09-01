#![allow(clippy::use_self)] // Can't use generic parameter from outer function

use std::cell::Ref;

use arrayref::{array_mut_ref, array_ref};
use arrayref::{array_refs, mut_array_refs};
use solana_program::pubkey::Pubkey;

use crate::error::{Error, Result};
use crate::types::Transaction;

use super::Holder;
use super::Operator;
use super::Packable;

/// Ethereum holder data account
#[derive(Default, Debug)]
pub struct Data {
    pub owner: Pubkey,
    pub transaction_hash: [u8; 32],
    pub transaction_len: usize,
}

impl Packable for Data {
    /// Holder struct tag
    const TAG: u8 = super::TAG_HOLDER;
    /// Holder struct serialized size
    const SIZE: usize = 32 + 32 + 8;

    /// Deserialize `Holder` struct from input data
    #[must_use]
    fn unpack(src: &[u8]) -> Self {
        let data = array_ref![src, 0, Data::SIZE];
        let (owner, hash, len) = array_refs![data, 32, 32, 8];

        Self {
            owner: Pubkey::new_from_array(*owner),
            transaction_hash: *hash,
            transaction_len: usize::from_le_bytes(*len),
        }
    }

    /// Serialize `Holder` struct into given destination
    fn pack(&self, dst: &mut [u8]) {
        let data = array_mut_ref![dst, 0, Data::SIZE];
        let (owner, hash, len) = mut_array_refs![data, 32, 32, 8];

        owner.copy_from_slice(self.owner.as_ref());
        hash.copy_from_slice(&self.transaction_hash);
        *len = self.transaction_len.to_le_bytes();
    }
}

impl<'a> Holder<'a> {
    pub fn clear(&mut self) -> Result<()> {
        self.transaction_hash.fill(0);
        self.transaction_len = 0;

        let mut data = self.info.try_borrow_mut_data()?;
        data[Self::SIZE..].fill(0);

        Ok(())
    }

    pub fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<()> {
        let mut data = self.info.try_borrow_mut_data()?;

        let begin = Self::SIZE
            .checked_add(offset)
            .ok_or(Error::IntegerOverflow)?;
        let end = begin
            .checked_add(bytes.len())
            .ok_or(Error::IntegerOverflow)?;

        data[begin..end].copy_from_slice(bytes);
        self.transaction_len = std::cmp::max(self.transaction_len, offset + bytes.len());

        Ok(())
    }

    #[must_use]
    pub fn transaction(&self) -> Ref<'a, [u8]> {
        let data = Ref::map(self.info.data.borrow(), |d| *d);
        Ref::map(data, |d| &d[Self::SIZE..][..self.transaction_len])
    }

    pub fn validate_owner(&self, operator: &Operator) -> Result<()> {
        if &self.owner != operator.key {
            return Err(Error::HolderInvalidOwner(self.owner, *operator.key));
        }

        Ok(())
    }

    pub fn validate_transaction(&self, trx: &Transaction) -> Result<()> {
        if self.transaction_hash != trx.hash() {
            return Err(Error::HolderInvalidHash(self.transaction_hash, trx.hash()));
        }

        Ok(())
    }
}
