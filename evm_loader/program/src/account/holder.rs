use std::cell::Ref;

use arrayref::{mut_array_refs, array_refs};
use arrayref::{array_mut_ref, array_ref};
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

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
        #[allow(clippy::use_self)]
        let data = array_mut_ref![dst, 0, Data::SIZE];
        let (owner, hash, len) = mut_array_refs![data, 32, 32, 8];

        owner.copy_from_slice(self.owner.as_ref());
        hash.copy_from_slice(&self.transaction_hash);
        *len = self.transaction_len.to_le_bytes();
    }
}


impl<'a> Holder<'a> {
    pub fn clear(&mut self) {
        self.transaction_hash.fill(0);
        self.transaction_len = 0;
        
        let mut data = self.info.data.borrow_mut();
        data[Self::SIZE..].fill(0);
    }

    pub fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), ProgramError>  {
        let mut data = self.info.data.borrow_mut();
        
        let begin = Self::SIZE.checked_add(offset)
            .ok_or_else(|| E!(ProgramError::InvalidArgument; "Holder offset overflow"))?;
        let end = begin.checked_add(bytes.len())
            .ok_or_else(|| E!(ProgramError::InvalidArgument; "Holder offset overflow"))?;

        data[begin..end].copy_from_slice(bytes);
        self.transaction_len = std::cmp::max(self.transaction_len, offset + bytes.len());

        Ok(())
    }

    #[must_use]
    pub fn transaction(&self) -> Ref<'a, [u8]> {
        let data = Ref::map(self.info.data.borrow(), |d| *d);
        Ref::map(data, |d| &d[Self::SIZE..][..self.transaction_len])
    }

    pub fn validate_owner(&self, operator: &Operator) -> Result<(), ProgramError> {
        if &self.owner != operator.key {
            return Err!(ProgramError::InvalidAccountData; "Invalid Holder account owner");
        }

        Ok(())
    }

    pub fn validate_transaction(&self, trx: &Transaction) -> Result<(), ProgramError> {
        if self.transaction_hash != trx.hash {
            return Err!(ProgramError::InvalidAccountData; "Invalid Holder transaction hash");
        }

        Ok(())
    }
}
