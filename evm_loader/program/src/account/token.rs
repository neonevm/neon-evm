use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;
use solana_program::program_pack::{IsInitialized, Pack};
use std::ops::Deref;

pub struct Account<'a, T: Pack + IsInitialized> {
    pub info: &'a AccountInfo<'a>,
    data: T,
}

impl<'a, T: Pack + IsInitialized> Account<'a, T> {
    pub fn from_account(info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if !spl_token::check_id(info.owner) {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not spl token owned", info.key);
        }

        let data = info.try_borrow_data()?;
        let data = T::unpack(&data)?;

        Ok(Self { info, data })
    }
}

impl<'a, T: Pack + IsInitialized> Deref for Account<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

pub type State<'a> = Account<'a, spl_token::state::Account>;
pub type Mint<'a> = Account<'a, spl_token::state::Mint>;
