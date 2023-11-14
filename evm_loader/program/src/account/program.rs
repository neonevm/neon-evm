use super::Operator;
use solana_program::account_info::AccountInfo;
use solana_program::program::{invoke_signed_unchecked, invoke_unchecked};
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::{rent::Rent, system_instruction, sysvar::Sysvar};
use std::convert::From;
use std::ops::Deref;

pub struct Neon<'a>(&'a AccountInfo<'a>);

impl<'a> Neon<'a> {
    pub fn from_account(
        program_id: &Pubkey,
        info: &'a AccountInfo<'a>,
    ) -> Result<Self, ProgramError> {
        if program_id != info.key {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not Neon program", info.key);
        }

        Ok(Self(info))
    }
}

impl<'a> Deref for Neon<'a> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

pub struct System<'a>(&'a AccountInfo<'a>);

impl<'a> From<&System<'a>> for &'a AccountInfo<'a> {
    fn from(f: &System<'a>) -> Self {
        f.0
    }
}

impl<'a> System<'a> {
    pub fn from_account(info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if !solana_program::system_program::check_id(info.key) {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not system program", info.key);
        }

        Ok(Self(info))
    }

    pub fn create_pda_account(
        &self,
        owner: &Pubkey,
        payer: &Operator<'a>,
        new_account: &AccountInfo<'a>,
        new_account_seeds: &[&[u8]],
        space: usize,
    ) -> Result<(), ProgramError> {
        let rent = Rent::get()?;
        let minimum_balance = rent.minimum_balance(space).max(1);

        if new_account.lamports() > 0 {
            let required_lamports = minimum_balance.saturating_sub(new_account.lamports());

            if required_lamports > 0 {
                invoke_unchecked(
                    &system_instruction::transfer(payer.key, new_account.key, required_lamports),
                    &[payer.info.clone(), new_account.clone(), self.0.clone()],
                )?;
            }

            invoke_signed_unchecked(
                &system_instruction::allocate(new_account.key, space as u64),
                &[new_account.clone(), self.0.clone()],
                &[new_account_seeds],
            )?;

            invoke_signed_unchecked(
                &system_instruction::assign(new_account.key, owner),
                &[new_account.clone(), self.0.clone()],
                &[new_account_seeds],
            )
        } else {
            invoke_signed_unchecked(
                &system_instruction::create_account(
                    payer.key,
                    new_account.key,
                    minimum_balance,
                    space as u64,
                    owner,
                ),
                &[payer.info.clone(), new_account.clone(), self.0.clone()],
                &[new_account_seeds],
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_account_with_seed(
        &self,
        owner: &Pubkey,
        payer: &Operator<'a>,
        base: &AccountInfo<'a>,
        signer_seeds: &[&[u8]],
        new_account: &AccountInfo<'a>,
        seed: &str,
        space: usize,
    ) -> Result<(), ProgramError> {
        let minimum_balance = Rent::get()?.minimum_balance(space).max(1);

        if new_account.lamports() > 0 {
            let required_lamports = minimum_balance.saturating_sub(new_account.lamports());

            if required_lamports > 0 {
                invoke_unchecked(
                    &system_instruction::transfer(payer.key, new_account.key, required_lamports),
                    &[payer.info.clone(), new_account.clone(), self.0.clone()],
                )?;
            }

            invoke_signed_unchecked(
                &system_instruction::allocate_with_seed(
                    new_account.key,
                    base.key,
                    seed,
                    space as u64,
                    owner,
                ),
                &[new_account.clone(), base.clone(), self.0.clone()],
                &[signer_seeds],
            )?;

            invoke_signed_unchecked(
                &system_instruction::assign_with_seed(new_account.key, base.key, seed, owner),
                &[new_account.clone(), base.clone(), self.0.clone()],
                &[signer_seeds],
            )
        } else {
            invoke_signed_unchecked(
                &system_instruction::create_account_with_seed(
                    payer.key,
                    new_account.key,
                    base.key,
                    seed,
                    minimum_balance,
                    space as u64,
                    owner,
                ),
                &[
                    payer.info.clone(),
                    new_account.clone(),
                    base.clone(),
                    self.0.clone(),
                ],
                &[signer_seeds],
            )
        }
    }

    pub fn transfer(
        &self,
        source: &Operator<'a>,
        target: &AccountInfo<'a>,
        lamports: u64,
    ) -> Result<(), ProgramError> {
        crate::debug_print!(
            "system transfer {} lamports from {} to {}",
            lamports,
            source.key,
            target.key
        );

        invoke_unchecked(
            &system_instruction::transfer(source.key, target.key, lamports),
            &[source.info.clone(), target.clone(), self.0.clone()],
        )
    }
}

impl<'a> Deref for System<'a> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

pub struct Token<'a>(&'a AccountInfo<'a>);

impl<'a> Token<'a> {
    pub fn from_account(info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if !spl_token::check_id(info.key) {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not token program", info.key);
        }

        Ok(Self(info))
    }

    pub fn create_account(
        &self,
        account: &AccountInfo<'a>,
        mint: &AccountInfo<'a>,
        owner: &AccountInfo<'a>,
    ) -> Result<(), ProgramError> {
        invoke_unchecked(
            &spl_token::instruction::initialize_account3(
                self.0.key,
                account.key,
                mint.key,
                owner.key,
            )?,
            &[account.clone(), mint.clone()],
        )
    }
}

impl<'a> Deref for Token<'a> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}
