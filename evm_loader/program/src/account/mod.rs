use std::cell::RefMut;
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use evm::H160;

use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::sysvar::Sysvar;

pub use incinerator::Incinerator;
pub use operator::Operator;
pub use treasury::{MainTreasury, Treasury};
use crate::account::program::System;

mod treasury;
mod operator;
mod incinerator;
pub mod ether_account;
pub mod ether_contract;
pub mod ether_storage;
pub mod state;
pub mod holder;
pub mod program;
pub mod token;
pub mod sysvar;

/// Ethereum account version
pub const ACCOUNT_SEED_VERSION: u8 = if cfg!(feature = "alpha") {
    // Special case for alpha configuration (it is needed in order to separate the accounts created for
    // testing this version)
    255_u8
} else {
    2_u8
};

/*
Deprecated tags:

const TAG_ACCOUNT_V1: u8 = 1;
const TAG_ACCOUNT_V2: u8 = 10;
const TAG_CONTRACT_V1: u8 = 2;
const TAG_CONTRACT_V2: u8 = 20;
const TAG_CONTRACT_STORAGE: u8 = 6;
const TAG_STATE_V1: u8 = 3;
const TAG_STATE: u8 = 30;
const TAG_ERC20_ALLOWANCE: u8 = 4;
const TAG_FINALIZED_STATE: u8 = 5;
const TAG_HOLDER: u8 = 6;
*/

const TAG_EMPTY: u8 = 0;
const TAG_ACCOUNT_V3: u8 = 11;
const TAG_STATE: u8 = 21;
const TAG_FINALIZED_STATE: u8 = 31;
const TAG_CONTRACT_STORAGE: u8 = 41;
const TAG_HOLDER: u8 = 51;

pub type EthereumAccount<'a> = AccountData<'a, ether_account::Data>;
pub type EthereumStorage<'a> = AccountData<'a, ether_storage::Data>;
pub type State<'a> = AccountData<'a, state::Data>;
pub type FinalizedState<'a> = AccountData<'a, state::FinalizedData>;
pub type Holder<'a> = AccountData<'a, holder::Data>;

pub trait Packable {
    const TAG: u8;
    const SIZE: usize;

    fn unpack(data: &[u8]) -> Self;
    fn pack(&self, data: &mut [u8]);
}

struct AccountParts<'a> {
    tag: RefMut<'a, u8>,
    data: RefMut<'a, [u8]>,
    remaining: RefMut<'a, [u8]>,
}

#[derive(Debug)]
pub struct AccountData<'a, T>
where
    T: Packable + Debug,
{
    dirty: bool,
    data: T,
    pub info: &'a AccountInfo<'a>,
}


fn split_account_data<'a>(info: &'a AccountInfo<'a>, data_len: usize) -> Result<AccountParts, ProgramError>
{
    if info.data_len() < 1 + data_len {
        return Err!(ProgramError::InvalidAccountData; "Account {} - invalid data len, expected = {} found = {}", info.key, data_len, info.data_len());
    }

    let account_data = info.try_borrow_mut_data()?;
    let (tag, bytes) = RefMut::map_split(account_data,
        |d| d.split_first_mut().expect("data is not empty")
    );
    let (data, remaining) = RefMut::map_split(bytes,
        |d| d.split_at_mut(data_len)
    );

    Ok(AccountParts{ tag, data, remaining })
}

impl<'a, T> AccountData<'a, T>
where
    T: Packable + Debug,
{
    pub const SIZE: usize = 1 + T::SIZE;
    pub const TAG: u8 = T::TAG;

    pub fn from_account(program_id: &Pubkey, info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if info.owner != program_id {
            return Err!(ProgramError::InvalidArgument; "Account {} - expected program owned", info.key);
        }

        let parts = split_account_data(info, T::SIZE)?;
        if *parts.tag != T::TAG {
            return Err!(ProgramError::InvalidAccountData; "Account {} - invalid tag, expected = {} found = {}", info.key, T::TAG, parts.tag);
        }

        let data = T::unpack(&parts.data);

        Ok(Self { dirty: false, data, info })
    }

    pub fn init(info: &'a AccountInfo<'a>, data: T) -> Result<Self, ProgramError> {
        if !info.is_writable {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not writable", info.key);
        }

        let rent = Rent::get()?;
        if !rent.is_exempt(info.lamports(), info.data_len()) {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not rent exempt", info.key);
        }

        let mut parts = split_account_data(info, T::SIZE)?;
        if *parts.tag != TAG_EMPTY {
            return Err!(ProgramError::AccountAlreadyInitialized; "Account {} - already initialized", info.key);
        }

        *parts.tag = T::TAG;
        data.pack(&mut parts.data);

        parts.remaining.fill(0);

        Ok(Self { dirty: false, data, info })
    }

    pub fn create_account(
        system_program: &System<'a>,
        program_id: &Pubkey,
        operator: &Operator<'a>,
        address: H160,
        info: &'a AccountInfo<'a>,
        bump_seed: u8,
        space: usize,
    ) -> ProgramResult {
        if space < EthereumAccount::SIZE {
            return Err!(
                ProgramError::AccountDataTooSmall;
                "Account {} - account space must be not less than minimal size of {} bytes",
                address,
                EthereumAccount::SIZE
            )
        }

        let program_seeds = &[
            &[ACCOUNT_SEED_VERSION],
            address.as_bytes(),
            &[bump_seed],
        ];
        system_program.create_pda_account(
            program_id,
            operator,
            info,
            program_seeds,
            space,
        )?;

        EthereumAccount::init(
            info,
            ether_account::Data {
                address,
                bump_seed,
                ..Default::default()
            },
        )?;

        Ok(())
    }

    /// # Safety
    /// *Delete account*. Transfer lamports to the operator.
    /// All data stored in the account will be lost
    pub unsafe fn suicide(mut self, operator: &Operator<'a>) -> ProgramResult {
        let info = self.info;

        self.dirty = false; // Do not save data into solana account
        core::mem::drop(self); // Release borrowed account data

        crate::account::delete(info, operator)
    }

    /// # Safety
    /// Should be used with care. Can corrupt account data
    pub unsafe fn replace<U>(mut self, data: U) -> Result<AccountData<'a, U>, ProgramError>
    where
        U: Packable + Debug,
    {
        debug_print!("replace account data from {:?} to {:?}", &self.data, &data);
        let info = self.info;

        if !info.is_writable {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not writable", self.info.key);
        }

        self.dirty = false; // Do not save data into solana account
        core::mem::drop(self); // Release borrowed account data

        let mut parts = split_account_data(info, U::SIZE)?;

        *parts.tag = U::TAG;
        data.pack(&mut parts.data);

        parts.remaining.fill(0);

        Ok(AccountData { dirty: false, data, info })
    }
}

impl<'a, T> Deref for AccountData<'a, T>
where
    T: Packable + Debug,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'a, T> DerefMut for AccountData<'a, T>
where
    T: Packable + Debug,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.dirty = true;
        &mut self.data
    }
}

impl<'a, T> Drop for AccountData<'a, T>
where
    T: Packable + Debug,
{
    fn drop(&mut self) {
        if !self.dirty {
            return;
        }

        debug_print!("Save into solana account {:?}", self.data);
        assert!(self.info.is_writable);

        let mut parts = split_account_data(self.info, T::SIZE)
            .expect("Account have incorrect size");

        self.data.pack(&mut parts.data);
    }
}

pub fn tag(program_id: &Pubkey, info: &AccountInfo) -> Result<u8, ProgramError> {
    if info.owner != program_id {
        return Err!(ProgramError::InvalidAccountData; "Account {} - expected program owned", info.key);
    }

    let data = info.try_borrow_data()?;
    if data.is_empty() {
        return Err!(ProgramError::InvalidAccountData; "Account {} - expected not empty", info.key);
    }

    Ok(data[0])
}

/// # Safety
/// *Permanently delete all data* in the account. Transfer lamports to the operator.
pub unsafe fn delete(account: &AccountInfo, operator: &Operator) -> ProgramResult {
    debug_print!("DELETE ACCOUNT {}", account.key);

    let operator_lamports = operator.lamports().checked_add(account.lamports())
        .ok_or_else(|| E!(ProgramError::InvalidArgument; "Operator lamports overflow"))?;

    **operator.lamports.borrow_mut() = operator_lamports;
    **account.lamports.borrow_mut() = 0;

    let mut data = account.data.borrow_mut();
    data.fill(0);

    Ok(())
}
