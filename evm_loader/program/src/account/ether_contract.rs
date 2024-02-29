use crate::{
    account::TAG_EMPTY,
    account_storage::KeysCache,
    error::{Error, Result},
    types::Address,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::MAX_PERMITTED_DATA_INCREASE, pubkey::Pubkey, rent::Rent,
    system_program,
};
use std::{
    cell::{Ref, RefMut},
    mem::size_of,
};

use crate::config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT;

use super::{
    AccountHeader, AccountsDB, ACCOUNT_PREFIX_LEN, ACCOUNT_SEED_VERSION, TAG_ACCOUNT_CONTRACT,
};

#[derive(Eq, PartialEq)]
pub enum AllocateResult {
    Ready,
    NeedMore,
}

#[repr(C, packed)]
pub struct HeaderV0 {
    pub address: Address,
    pub chain_id: u64,
    pub generation: u32,
}

impl AccountHeader for HeaderV0 {
    const VERSION: u8 = 0;
}

#[repr(C, packed)]
pub struct HeaderWithRevision {
    pub v0: HeaderV0,
    pub revision: u32,
}

impl AccountHeader for HeaderWithRevision {
    const VERSION: u8 = 2;
}

// Set the last version of the Header struct here
// and change the `header_size` and `header_upgrade` functions
pub type Header = HeaderWithRevision;

pub type Storage = [[u8; 32]; STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT];
pub type Code = [u8];

pub struct ContractAccount<'a> {
    account: AccountInfo<'a>,
}

impl<'a> ContractAccount<'a> {
    #[must_use]
    pub fn required_account_size(code: &[u8]) -> usize {
        ACCOUNT_PREFIX_LEN + size_of::<Header>() + size_of::<Storage>() + code.len()
    }

    #[must_use]
    pub fn required_header_realloc(&self) -> usize {
        let allocated_header_size = self.header_size();
        size_of::<Header>().saturating_sub(allocated_header_size)
    }

    pub fn from_account(program_id: &Pubkey, account: AccountInfo<'a>) -> Result<Self> {
        super::validate_tag(program_id, &account, TAG_ACCOUNT_CONTRACT)?;

        Ok(Self { account })
    }

    pub fn allocate(
        address: Address,
        code: &[u8],
        rent: &Rent,
        accounts: &AccountsDB,
        keys: Option<&KeysCache>,
    ) -> Result<AllocateResult> {
        let (pubkey, bump_seed) = keys.map_or_else(
            || address.find_solana_address(&crate::ID),
            |keys| keys.contract_with_bump_seed(&crate::ID, address),
        );

        let info = accounts.get(&pubkey);

        let required_size = Self::required_account_size(code);
        if info.data_len() >= required_size {
            return Ok(AllocateResult::Ready);
        }

        let system = accounts.system();
        let operator = accounts.operator();

        if system_program::check_id(info.owner) {
            let seeds: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], address.as_bytes(), &[bump_seed]];
            let space = required_size.min(MAX_PERMITTED_DATA_INCREASE);
            system.create_pda_account(&crate::ID, operator, info, seeds, space, rent)?;
        } else if crate::check_id(info.owner) {
            super::validate_tag(&crate::ID, info, TAG_EMPTY)?;

            let max_size = info.data_len() + MAX_PERMITTED_DATA_INCREASE;
            let space = required_size.min(max_size);
            info.realloc(space, false)?;

            let required_balance = rent.minimum_balance(space);
            if info.lamports() < required_balance {
                let lamports = required_balance - info.lamports();
                system.transfer(operator, info, lamports)?;
            }
        } else {
            return Err(Error::AccountInvalidOwner(pubkey, system_program::ID));
        }

        if info.data_len() >= required_size {
            Ok(AllocateResult::Ready)
        } else {
            Ok(AllocateResult::NeedMore)
        }
    }

    pub fn init(
        address: Address,
        chain_id: u64,
        generation: u32,
        code: &[u8],
        accounts: &AccountsDB<'a>,
        keys: Option<&KeysCache>,
    ) -> Result<Self> {
        let (pubkey, _) = keys.map_or_else(
            || address.find_solana_address(&crate::ID),
            |keys| keys.contract_with_bump_seed(&crate::ID, address),
        );

        let account = accounts.get(&pubkey).clone();

        super::validate_tag(&crate::ID, &account, TAG_EMPTY)?;
        super::set_tag(&crate::ID, &account, TAG_ACCOUNT_CONTRACT, Header::VERSION)?;

        let mut contract = Self { account };
        {
            let mut header = super::header_mut::<Header>(&contract.account);
            header.v0.address = address;
            header.v0.chain_id = chain_id;
            header.v0.generation = generation;
            header.revision = 1;
        }
        {
            let mut contract_code = contract.code_mut();
            contract_code.copy_from_slice(code);
        }

        Ok(contract)
    }

    #[must_use]
    pub fn pubkey(&self) -> &'a Pubkey {
        self.account.key
    }

    fn header_size(&self) -> usize {
        match super::header_version(&self.account) {
            0 | 1 => size_of::<HeaderV0>(),
            HeaderWithRevision::VERSION => size_of::<HeaderWithRevision>(),
            _ => panic!("Unknown header version"),
        }
    }

    fn header_upgrade(&mut self, rent: &Rent, db: &AccountsDB<'a>) -> Result<()> {
        match super::header_version(&self.account) {
            0 | 1 => {
                super::expand_header::<HeaderV0, Header>(&self.account, rent, db)?;
            }
            HeaderWithRevision::VERSION => {
                super::expand_header::<HeaderWithRevision, Header>(&self.account, rent, db)?;
            }
            _ => panic!("Unknown header version"),
        }

        Ok(())
    }

    #[inline]
    #[must_use]
    fn storage_offset(&self) -> usize {
        ACCOUNT_PREFIX_LEN + self.header_size()
    }

    #[inline]
    fn storage(&self) -> Ref<Storage> {
        let offset = self.storage_offset();
        super::section(&self.account, offset)
    }

    #[inline]
    fn storage_mut(&mut self) -> RefMut<Storage> {
        let offset = self.storage_offset();
        super::section_mut(&self.account, offset)
    }

    #[inline]
    #[must_use]
    fn code_offset(&self) -> usize {
        self.storage_offset() + size_of::<Storage>()
    }

    #[inline]
    #[must_use]
    pub fn code(&self) -> Ref<Code> {
        let offset = self.code_offset();

        let data = self.account.data.borrow();
        Ref::map(data, |d| &d[offset..])
    }

    #[inline]
    fn code_mut(&mut self) -> RefMut<Code> {
        let offset = self.code_offset();

        let data = self.account.data.borrow_mut();
        RefMut::map(data, |d| &mut d[offset..])
    }

    #[must_use]
    pub fn code_buffer(&self) -> crate::evm::Buffer {
        let begin = self.code_offset();
        let end = begin + self.code_len();

        unsafe { crate::evm::Buffer::from_account(&self.account, begin..end) }
    }

    #[must_use]
    pub fn code_len(&self) -> usize {
        let offset = self.code_offset();

        self.account.data_len().saturating_sub(offset)
    }

    #[must_use]
    pub fn address(&self) -> Address {
        let header = super::header::<HeaderV0>(&self.account);
        header.address
    }

    #[must_use]
    pub fn chain_id(&self) -> u64 {
        let header = super::header::<HeaderV0>(&self.account);
        header.chain_id
    }

    #[must_use]
    pub fn generation(&self) -> u32 {
        let header = super::header::<HeaderV0>(&self.account);
        header.generation
    }

    #[must_use]
    pub fn revision(&self) -> u32 {
        if super::header_version(&self.account) < HeaderWithRevision::VERSION {
            return 0;
        }

        let header = super::header::<HeaderWithRevision>(&self.account);
        header.revision
    }

    pub fn increment_revision(&mut self, rent: &Rent, db: &AccountsDB<'a>) -> Result<()> {
        if super::header_version(&self.account) < HeaderWithRevision::VERSION {
            self.header_upgrade(rent, db)?;
        }

        let mut header = super::header_mut::<HeaderWithRevision>(&self.account);
        header.revision = header.revision.wrapping_add(1);

        Ok(())
    }

    #[must_use]
    pub fn storage_value(&self, index: usize) -> [u8; 32] {
        assert!(index < STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT);

        let storage = self.storage();
        storage[index]
    }

    pub fn set_storage_value(&mut self, index: usize, value: &[u8; 32]) {
        assert!(index < STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT);

        let mut storage = self.storage_mut();

        let cell: &mut [u8; 32] = &mut storage[index];
        cell.copy_from_slice(value);
    }

    pub fn set_storage_multiple_values(&mut self, offset: usize, values: &[[u8; 32]]) {
        let max = offset.saturating_add(values.len());
        assert!(max <= STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT);

        let mut storage = self.storage_mut();
        storage[offset..][..values.len()].copy_from_slice(values);
    }
}
