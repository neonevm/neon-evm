use std::cell::RefMut;
use std::mem::size_of;
use std::ops::Range;

use ethnum::U256;

use crate::account::EthereumAccount;
use crate::config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT;

const INTERNAL_STORAGE_SIZE: usize =
    size_of::<U256>() * STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT as usize;

pub struct ContractData<'this, 'acc> {
    account: &'this EthereumAccount<'acc>,
}

impl<'acc> ContractData<'_, 'acc> {
    #[must_use]
    pub fn code(&self) -> RefMut<'acc, [u8]> {
        let offset = INTERNAL_STORAGE_SIZE;
        let len = self.account.code_size as usize;

        self.extension_part_borrow_mut(offset, len)
    }

    #[must_use]
    pub fn storage(&self) -> RefMut<'acc, [u8]> {
        let offset = 0;
        let len = INTERNAL_STORAGE_SIZE;

        self.extension_part_borrow_mut(offset, len)
    }

    #[must_use]
    pub fn extension_borrow_mut(&self) -> RefMut<'acc, [u8]> {
        RefMut::map(self.account.info.data.borrow_mut(), |slice| {
            &mut slice[EthereumAccount::SIZE..]
        })
    }

    #[must_use]
    fn extension_part_borrow_mut(&self, offset: usize, len: usize) -> RefMut<'acc, [u8]> {
        RefMut::map(self.extension_borrow_mut(), |slice| {
            &mut slice[offset..][..len]
        })
    }
}

impl<'this, 'acc> EthereumAccount<'acc> {
    #[must_use]
    pub fn is_contract(&self) -> bool {
        self.code_size() != 0
    }

    #[must_use]
    pub fn code_size(&self) -> usize {
        self.code_size as usize
    }

    #[must_use]
    pub fn code_location(&self) -> Range<usize> {
        let begin = Self::SIZE + INTERNAL_STORAGE_SIZE;
        let end = begin.saturating_add(self.code_size());

        begin..end
    }

    #[must_use]
    pub fn contract_data(&'this self) -> Option<ContractData<'this, 'acc>> {
        if !self.is_contract() {
            return None;
        }
        Some(ContractData { account: self })
    }

    #[must_use]
    pub fn space_needed(code_size: usize) -> usize {
        EthereumAccount::SIZE
            + if code_size > 0 {
                code_size + INTERNAL_STORAGE_SIZE
            } else {
                0
            }
    }

    #[must_use]
    pub fn size(&self) -> usize {
        Self::space_needed(self.code_size())
    }
}
