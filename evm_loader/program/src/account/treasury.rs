use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use std::ops::Deref;
use crate::config::collateral_pool_base;

pub struct Treasury<'a> {
    info: &'a AccountInfo<'a>
}

pub struct MainTreasury<'a> {
    info: &'a AccountInfo<'a>
}

impl<'a> Treasury<'a> {
    pub fn from_account(program_id: &Pubkey, index: u32, info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if info.owner != program_id {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not program owned", info.key);
        }

        let seed = format!("{}{}", collateral_pool_base::PREFIX, index);
        let expected_key = Pubkey::create_with_seed(&collateral_pool_base::id(), &seed, program_id)?;
        if *info.key != expected_key {
            return Err!(ProgramError::InvalidArgument; "Account {} - invalid treasure account", info.key);
        }

        Ok(Self { info })
    }
}

impl<'a> Deref for Treasury<'a> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        self.info
    }
}

impl<'a> MainTreasury<'a> {
    pub fn from_account(info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        let expected_key = Pubkey::create_with_seed(
            &collateral_pool_base::id(),
            collateral_pool_base::MAIN_BALANCE_SEED,
            &spl_token::id())?;

        if *info.key != expected_key {
            return Err!(ProgramError::InvalidArgument; "Account {} - invalid main treasure account", info.key);
        }

        Ok(Self { info })
    }
}

impl<'a> Deref for MainTreasury<'a> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        self.info
    }
}

