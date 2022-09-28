use std::cell::RefMut;
use std::mem::size_of;

use evm::{U256, Valids};

use crate::account::EthereumAccount;
use crate::config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT;

pub struct ContractData<'this, 'acc> {
    account: &'this EthereumAccount<'acc>,
}

impl<'acc> ContractData<'_, 'acc> {
    pub const INTERNAL_STORAGE_SIZE: usize =
        size_of::<U256>() * STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT as usize;

    #[must_use]
    pub fn code(&self) -> RefMut<'acc, [u8]> {
        self.extension_part_borrow_mut(0, self.account.code_size as usize)
    }

    #[must_use]
    pub fn valids(&self) -> RefMut<'acc, [u8]> {
        self.extension_part_borrow_mut(
            self.account.code_size as usize,
            Valids::size_needed(self.account.code_size as usize),
        )
    }

    #[must_use]
    pub fn storage(&self) -> RefMut<'acc, [u8]> {
        let valids_size = Valids::size_needed(self.account.code_size as usize);
        self.extension_part_borrow_mut(
            self.account.code_size as usize + valids_size,
            Self::INTERNAL_STORAGE_SIZE,
        )
    }

    #[must_use]
    pub fn extension_borrow_mut(&self) -> RefMut<'acc, [u8]> {
        RefMut::map(
            self.account.info.data.borrow_mut(),
            |slice| &mut slice[EthereumAccount::SIZE..],
        )
    }

    #[must_use]
    fn extension_part_borrow_mut(&self, offset: usize, len: usize) -> RefMut<'acc, [u8]> {
        RefMut::map(
            self.extension_borrow_mut(),
            |slice| &mut slice[offset..][..len],
        )
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
    pub fn contract_data(&'this self) -> Option<ContractData<'this, 'acc>> {
        if !self.is_contract() {
            return None;
        }
        Some(ContractData { account: self })
    }

    #[must_use]
    pub fn space_needed(code_size: usize) -> usize {
        EthereumAccount::SIZE +
            if code_size > 0 {
                code_size + Valids::size_needed(code_size) + ContractData::INTERNAL_STORAGE_SIZE
            } else {
                0
            }
    }

    #[must_use]
    pub fn size(&self) -> usize {
        Self::space_needed(self.code_size())
    }
}
