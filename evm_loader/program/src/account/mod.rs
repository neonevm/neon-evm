use std::cell::RefMut;
use std::fmt::Debug;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};

use solana_program::account_info::AccountInfo;
use solana_program::msg;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::sysvar::Sysvar;

pub use holder::Holder;
pub use incinerator::Incinerator;
pub use operator::Operator;
pub use treasury::Treasury;

mod treasury;
mod operator;
mod holder;
mod incinerator;
pub mod ether_account;
pub mod ether_contract;
pub mod ether_storage;
pub mod erc20_allowance;
pub mod state;
pub mod program;
pub mod token;
pub mod sysvar;

/// Ethereum account version
pub const ACCOUNT_SEED_VERSION: u8 = if cfg!(feature = "alpha") {
    // Special case for alpha configuration (it is needed in order to separate the accounts created for
    // testing this version)
    255_u8
} else {
    1_u8
};

pub const TAG_EMPTY: u8 = 0;
#[deprecated]
const TAG_ACCOUNT_V1: u8 = 1;
const TAG_ACCOUNT: u8 = 10;
#[deprecated]
const _TAG_CONTRACT_V1: u8 = 2;
const TAG_CONTRACT: u8 = 20;
const TAG_CONTRACT_STORAGE: u8 = 6;
#[deprecated]
const _TAG_STATE_V1: u8 = 3;
const TAG_STATE: u8 = 30;
const TAG_ERC20_ALLOWANCE: u8 = 4;
const TAG_FINALIZED_STATE: u8 = 5;

pub type EthereumAccount<'a> = AccountData<'a, ether_account::Data>;
pub type EthereumContract<'a> = AccountData<'a, ether_contract::Data, ether_contract::Extension<'a>>;
pub type EthereumStorage<'a> = AccountData<'a, ether_storage::Data>;
pub type State<'a> = AccountData<'a, state::Data>;
pub type FinalizedState<'a> = AccountData<'a, state::FinalizedData>;
pub type ERC20Allowance<'a> = AccountData<'a, erc20_allowance::Data>;


pub trait AccountExtension<'a, T> {
    fn unpack(data: &T, remaining: RefMut<'a, [u8]>) -> Result<Self, ProgramError> where Self: Sized;
}

impl<'a, T> AccountExtension<'a, T> for () {
    fn unpack(_data: &T, _remaining: RefMut<'a, [u8]>) -> Result<Self, ProgramError> { Ok(()) }
}


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
pub struct AccountData<'a, T, E = ()>
where
    T: Packable + Debug,
    E: AccountExtension<'a, T>
{
    dirty: bool,
    data: T,
    pub extension: ManuallyDrop<E>,
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

impl<'a, T, E> AccountData<'a, T, E>
where
    T: Packable + Debug,
    E: AccountExtension<'a, T>
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
        let extension = E::unpack(&data, parts.remaining)?;
        let extension = ManuallyDrop::new(extension);

        Ok(Self { dirty: false, data, extension, info })
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
        let extension = E::unpack(&data, parts.remaining)?;
        let extension = ManuallyDrop::new(extension);

        Ok(Self { dirty: false, data, extension, info })
    }

    pub fn reload_extension(&mut self) -> Result<(), ProgramError> {
        debug_print!("reload extension {:?}", &self.data);

        unsafe { ManuallyDrop::drop(&mut self.extension) }; // Release borrowed account data

        let parts = split_account_data(self.info, T::SIZE)?;

        let extension = E::unpack(&self.data, parts.remaining)?;
        self.extension = ManuallyDrop::new(extension);

        Ok(())
    }

    /// # Safety
    /// *Delete account*. Transfer lamports to the operator.
    /// All data stored in the account will be lost
    pub unsafe fn suicide(mut self, operator: &Operator<'a>) -> Result<(), ProgramError> {
        let info = self.info;

        self.dirty = false; // Do not save data into solana account
        core::mem::drop(self); // Release borrowed account data

        crate::account::delete(info, operator)
    }

    /// # Safety
    /// Should be used with care. Can corrupt account data
    pub unsafe fn replace<U, R>(mut self, data: U) -> Result<AccountData<'a, U, R>, ProgramError>
        where
            U: Packable + Debug,
            R: AccountExtension<'a, U>,
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
        let extension = R::unpack(&data, parts.remaining)?;
        let extension = ManuallyDrop::new(extension);

        Ok(AccountData { dirty: false, data, extension, info })
    }
}

impl<'a, T, E> Deref for AccountData<'a, T, E>
where
    T: Packable + Debug,
    E: AccountExtension<'a, T>
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'a, T, E> DerefMut for AccountData<'a, T, E>
where
    T: Packable + Debug,
    E: AccountExtension<'a, T>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.dirty = true;
        &mut self.data
    }
}

impl<'a, T, E> Drop for AccountData<'a, T, E>
where
    T: Packable + Debug,
    E: AccountExtension<'a, T>
{
    fn drop(&mut self) {
        // Release borrowed account data
        unsafe { ManuallyDrop::drop(&mut self.extension) };

        if !self.dirty {
            return;
        }

        debug_print!("Save into solana account {:?}", self.data);
        assert!(self.info.is_writable);

        let mut parts = split_account_data(self.info, T::SIZE)
            .expect("Account have correct size");

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
pub unsafe fn delete(account: &AccountInfo, operator: &Operator) -> Result<(), ProgramError> {
    msg!("DELETE ACCOUNT {}", account.key);

    let operator_lamports = operator.lamports().checked_add(account.lamports())
        .ok_or_else(|| E!(ProgramError::InvalidArgument; "Operator lamports overflow"))?;

    **operator.lamports.borrow_mut() = operator_lamports;
    **account.lamports.borrow_mut() = 0;

    let mut data = account.data.borrow_mut();
    data.fill(0);

    Ok(())
}
