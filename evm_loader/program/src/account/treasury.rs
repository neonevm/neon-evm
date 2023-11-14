use crate::config::TREASURY_POOL_SEED;
use crate::error::{Error, Result};
use solana_program::{account_info::AccountInfo, program_pack::Pack, pubkey::Pubkey};
use std::ops::Deref;

pub struct Treasury<'a> {
    info: &'a AccountInfo<'a>,
    bump_seed: u8,
}

pub struct MainTreasury<'a> {
    info: &'a AccountInfo<'a>,
    bump_seed: u8,
}

impl<'a> Treasury<'a> {
    pub fn from_account(
        program_id: &Pubkey,
        index: u32,
        info: &'a AccountInfo<'a>,
    ) -> Result<Self> {
        let (expected_key, bump_seed) = Treasury::address(program_id, index);
        if *info.key != expected_key {
            return Err(Error::AccountInvalidKey(*info.key, expected_key));
        }

        Ok(Self { info, bump_seed })
    }

    #[must_use]
    pub fn address(program_id: &Pubkey, index: u32) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[TREASURY_POOL_SEED.as_bytes(), &index.to_le_bytes()],
            program_id,
        )
    }

    #[must_use]
    pub fn get_bump_seed(&self) -> u8 {
        self.bump_seed
    }
}

impl<'a> Deref for Treasury<'a> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        self.info
    }
}

impl<'a> MainTreasury<'a> {
    pub fn from_account(program_id: &Pubkey, info: &'a AccountInfo<'a>) -> Result<Self> {
        let (expected_key, bump_seed) = MainTreasury::address(program_id);
        if *info.key != expected_key {
            return Err(Error::AccountInvalidKey(*info.key, expected_key));
        }

        if *info.owner != spl_token::id() {
            return Err(Error::AccountInvalidOwner(*info.key, spl_token::id()));
        }

        let account = spl_token::state::Account::unpack(&info.data.borrow())?;
        if account.mint != spl_token::native_mint::id() {
            return Err(Error::Custom(format!(
                "Account {} - not wrapped SOL spl_token account",
                info.key
            )));
        }

        Ok(Self { info, bump_seed })
    }

    #[must_use]
    pub fn address(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[TREASURY_POOL_SEED.as_bytes()], program_id)
    }

    #[must_use]
    pub fn get_bump_seed(&self) -> u8 {
        self.bump_seed
    }
}

impl<'a> Deref for MainTreasury<'a> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        self.info
    }
}
