use std::cell::RefMut;
use std::mem::size_of;

use arrayref::{array_ref, array_refs};
use evm::{U256, Valids};
use solana_program::pubkey::Pubkey;

use crate::account::{EthereumAccount, Packable};
use crate::config::STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT;

/// Ethereum contract data account v2
#[deprecated]
#[derive(Debug)]
pub struct DataV2 {
    /// Solana account with ethereum account data associated with this code data
    pub owner: Pubkey,
    /// Contract code size
    pub code_size: u32,
    /// Contract generation, increment on suicide
    pub generation: u32
}

#[allow(deprecated)]
impl Packable for DataV2 {
    /// Contract struct tag
    const TAG: u8 = super::_TAG_CONTRACT_V2;
    /// Contract struct serialized size
    const SIZE: usize = 32 + 4 + 4;

    /// Deserialize `Contract` struct from input data
    #[must_use]
    fn unpack(input: &[u8]) -> Self {
        #[allow(clippy::use_self)]
            let data = array_ref![input, 0, DataV2::SIZE];
        let (owner, code_size, generation) = array_refs![data, 32, 4, 4];

        Self {
            owner: Pubkey::new_from_array(*owner),
            code_size: u32::from_le_bytes(*code_size),
            generation: u32::from_le_bytes(*generation),
        }
    }

    fn pack(&self, _dst: &mut [u8]) {
        unimplemented!()
    }
}

pub struct ContractData<'this, 'acc> {
    account: &'this EthereumAccount<'acc>,
}

impl<'acc> ContractData<'_, 'acc> {
    pub const INTERNAL_STORAGE_SIZE: usize =
        size_of::<U256>() * STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT as usize;

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
        RefMut::map(self.account.info.data.borrow_mut(), |slice| &mut slice[EthereumAccount::SIZE..])
    }

    #[must_use]
    fn extension_part_borrow_mut(&self, offset: usize, len: usize) -> RefMut<'acc, [u8]> {
        RefMut::map(self.extension_borrow_mut(), |slice| &mut slice[offset..][..len])
    }

}

pub trait ContractExtension<'this, 'acc> {
    fn is_contract(&self) -> bool {
        self.code_size() != 0
    }

    fn code_size(&self) -> usize;
    fn contract_data(&'this self) -> Option<ContractData<'this, 'acc>>;

    #[must_use]
    fn space_needed(code_size: usize) -> usize {
        EthereumAccount::SIZE +
            if code_size > 0 {
                code_size + Valids::size_needed(code_size) + ContractData::INTERNAL_STORAGE_SIZE
            } else {
                0
            }
    }

    #[must_use]
    fn size(&self) -> usize {
        Self::space_needed(self.code_size())
    }
}

impl<'this, 'acc> ContractExtension<'this, 'acc> for EthereumAccount<'acc> {
    fn code_size(&self) -> usize {
        self.code_size as usize
    }

    fn contract_data(&'this self) -> Option<ContractData<'this, 'acc>> {
        if !self.is_contract() {
            return None;
        }
        Some(ContractData { account: self })
    }
}
