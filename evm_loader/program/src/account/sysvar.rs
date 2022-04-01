use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;
use solana_program::sysvar::Sysvar;


pub struct Instructions<'a> {
    pub info: &'a AccountInfo<'a>
}

impl<'a> Instructions<'a> {
    pub fn from_account(info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if !solana_program::sysvar::instructions::check_id(info.key) {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not sysvar instructions", info.key);
        }

        Ok(Self { info })
    }
}



pub struct Rent<'a> {
    pub info: &'a AccountInfo<'a>,
    pub data: solana_program::sysvar::rent::Rent,
}

impl<'a> Rent<'a> {
    pub fn from_account(info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if !solana_program::sysvar::rent::check_id(info.key) {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not sysvar instructions", info.key);
        }

        let data = solana_program::sysvar::rent::Rent::from_account_info(info)?;

        Ok(Self { info, data })
    }
}
