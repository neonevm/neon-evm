use std::ops::Deref;
use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;


pub struct Incinerator<'a> {
    info: &'a AccountInfo<'a>,
}

impl<'a> Incinerator<'a> {
    pub fn from_account(info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if !solana_program::incinerator::check_id(info.key) {
            return Err!(ProgramError::InvalidArgument; "Account {} - expected incinerator", info.key)
        }

        Ok(Self { info })
    }
}

impl<'a> Deref for Incinerator<'a> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        self.info
    }
}
