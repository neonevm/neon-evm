use std::convert::From;
use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::log::sol_log_data;
use solana_program::{
    program::{invoke, invoke_signed}, system_instruction,
    rent::Rent, sysvar::Sysvar
};
use super::{Operator, EthereumAccount, sysvar, token};
use std::ops::Deref;
use evm::{ExitError, ExitFatal, ExitReason, ExitSucceed, H160, H256, U256};

use crate::account::ACCOUNT_SEED_VERSION;


pub struct Neon<'a> (&'a AccountInfo<'a>);

impl<'a> Neon<'a> {
    pub fn from_account(program_id: &Pubkey, info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if program_id != info.key {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not Neon program", info.key);
        }

        Ok(Self ( info ))
    }

    #[allow(clippy::unused_self)]
    pub fn on_return(&self, exit_reason: ExitReason, used_gas: U256, result: &[u8])
    {
        let (exit_message, exit_status) = match exit_reason {
            ExitReason::Succeed(success_code) => {
                match success_code {
                    ExitSucceed::Stopped => {("ExitSucceed: Machine encountered an explict stop.", 0x11_u8)},
                    ExitSucceed::Returned => {("ExitSucceed: Machine encountered an explict return.", 0x12)},
                    ExitSucceed::Suicided => {("ExitSucceed: Machine encountered an explict suicide.", 0x13)},
                }
            },
            ExitReason::Error(error_code) => {
                match error_code {
                    ExitError::StackUnderflow => {("ExitError: Trying to pop from an empty stack.", 0xe1)},
                    ExitError::StackOverflow => {("ExitError: Trying to push into a stack over stack limit.", 0xe2)},
                    ExitError::InvalidJump => {("ExitError: Jump destination is invalid.", 0xe3)},
                    ExitError::InvalidRange => {("ExitError: An opcode accesses memory region, but the region is invalid.", 0xe4)},
                    ExitError::DesignatedInvalid => {("ExitError: Encountered the designated invalid opcode.", 0xe5)},
                    ExitError::CallTooDeep => {("ExitError: Call stack is too deep (runtime).", 0xe6)},
                    ExitError::CreateCollision => {("ExitError: Create opcode encountered collision (runtime).", 0xe7)},
                    ExitError::CreateContractLimit => {("ExitError: Create init code exceeds limit (runtime).", 0xe8)},
                    ExitError::OutOfOffset => {("ExitError: An opcode accesses external information, but the request is off offset limit (runtime).", 0xe9)},
                    ExitError::OutOfGas => {("ExitError: Execution runs out of gas (runtime).", 0xea)},
                    ExitError::OutOfFund => {("ExitError: Not enough fund to start the execution (runtime).", 0xeb)},
                    ExitError::PCUnderflow => {("ExitError: PC underflowed (unused).", 0xec)},
                    ExitError::CreateEmpty => {("ExitError: Attempt to create an empty account (runtime, unused).", 0xed)},
                    ExitError::StaticModeViolation => {("ExitError: STATICCALL tried to change state", 0xee)}
                }
            },
            ExitReason::Revert(_) => {("Revert", 0xd0)},
            ExitReason::Fatal(fatal_code) => {
                match fatal_code {
                    ExitFatal::NotSupported => {("Fatal: The operation is not supported.", 0xf1)},
                    ExitFatal::UnhandledInterrupt => {("Fatal: The trap (interrupt) is unhandled.", 0xf2)},
                    ExitFatal::CallErrorAsFatal(_) => {("Fatal: The environment explictly set call errors as fatal error.", 0xf3)},
                }
            },
            ExitReason::StepLimitReached => unreachable!(),
        };

        solana_program::msg!("{} exit_status={:#04X}", exit_message, exit_status);
        debug_print!("used gas {}", used_gas);
        debug_print!("result {}", &hex::encode(&result));

        let used_gas = if used_gas > U256::from(u64::MAX) { // Convert to u64 to not break ABI
            solana_program::msg!("Error: used gas {} exceeds u64::MAX", used_gas);
            u64::MAX
        } else {
            used_gas.as_u64()
        };

        let mnemonic = b"RETURN";
        let exit_status = exit_status.to_le_bytes();
        let used_gas = used_gas.to_le_bytes();
        let fields = [mnemonic.as_slice(),
                      exit_status.as_slice(),
                      used_gas.as_slice(),
                      result];
        sol_log_data(&fields);
    }

    #[allow(clippy::unused_self)]
    pub fn on_event(&self, address: H160, topics: &[H256], data: &[u8]) -> Result<(), ProgramError> {
        assert!(topics.len() < 5);
        #[allow(clippy::cast_possible_truncation)]
        let nt = topics.len() as u8;
        let count_topics = topics.len().to_le_bytes();
        let empty = [] as [u8; 0];

        let mnemonic = [b'L', b'O', b'G', b'0' + nt];
        let t1 = if nt < 1 { &empty } else { topics[0].as_bytes() };
        let t2 = if nt < 2 { &empty } else { topics[1].as_bytes() };
        let t3 = if nt < 3 { &empty } else { topics[2].as_bytes() };
        let t4 = if nt < 4 { &empty } else { topics[3].as_bytes() };
        let fields = [mnemonic.as_slice(),
                      address.as_bytes(),
                      count_topics.as_slice(),
                      t1,
                      t2,
                      t3,
                      t4,
                      data];
        sol_log_data(&fields);

        Ok(())
    }
}

impl<'a> Deref for Neon<'a> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}


pub struct System<'a> (&'a AccountInfo<'a>);

impl<'a> From<&'a AccountInfo<'a>> for System<'a> {
    fn from(info: &'a AccountInfo<'a>) -> Self {
        Self( info )
    }
}

impl<'a> From<& System<'a>> for &'a AccountInfo<'a> {
    fn from(f:& System<'a>) -> Self {
        f.0
    }
}

impl<'a> System<'a> {
    pub fn from_account(info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if !solana_program::system_program::check_id(info.key) {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not system program", info.key);
        }

        Ok(Self ( info ))
    }

    pub fn create_pda_account(
        &self,
        program_id: &Pubkey,
        payer: &Operator<'a>,
        new_account: &AccountInfo<'a>,
        new_account_seeds: &[&[u8]],
        space: usize
    ) -> Result<(), ProgramError> {
        let rent = Rent::get()?;
        let minimum_balance = rent.minimum_balance(space).max(1);

        if new_account.lamports() > 0 {
            let required_lamports = minimum_balance.saturating_sub(new_account.lamports());

            if required_lamports > 0 {
                invoke(
                    &system_instruction::transfer(payer.key, new_account.key, required_lamports),
                    &[(*payer).clone(), new_account.clone(), self.0.clone()]
                )?;
            }

            invoke_signed(
                &system_instruction::allocate(new_account.key, space as u64),
                &[new_account.clone(), self.0.clone()],
                &[new_account_seeds],
            )?;

            invoke_signed(
                &system_instruction::assign(new_account.key, program_id),
                &[new_account.clone(), self.0.clone()],
                &[new_account_seeds]
            )
        } else {
            invoke_signed(
                &system_instruction::create_account(
                    payer.key,
                    new_account.key,
                    minimum_balance,
                    space as u64,
                    program_id,
                ),
                &[(*payer).clone(), new_account.clone(), self.0.clone()],
                &[new_account_seeds],
            )
        }
    }

    pub fn transfer(
        &self,
        source: &Operator<'a>,
        target: &AccountInfo<'a>,
        lamports: u64
    ) -> Result<(), ProgramError> {
        crate::debug_print!("system transfer {} lamports from {} to {}", lamports, source.key, target.key);

        invoke(
            &system_instruction::transfer(source.key, target.key, lamports),
            &[(*source).clone(), target.clone(), self.0.clone()]
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

        Ok(Self ( info ))
    }

    pub fn initialize_account(
        &self,
        account: &AccountInfo<'a>,
        mint: &token::Mint<'a>,
        owner: &EthereumAccount<'a>,
        rent: &sysvar::Rent<'a>,
    ) -> Result<(), ProgramError> {
        let instruction = spl_token::instruction::initialize_account(
            &spl_token::id(),
            account.key,
            mint.info.key,
            owner.info.key
        )?;
        let accounts = &[
            account.clone(),
            mint.info.clone(),
            owner.info.clone(),
            self.0.clone(),
            rent.info.clone(),
        ];

        invoke(&instruction, accounts)
    }

    pub fn transfer(
        &self,
        authority: &EthereumAccount<'a>,
        source: &AccountInfo<'a>,
        target: &AccountInfo<'a>,
        value: u64
    ) -> Result<(), ProgramError> {
        let instruction = spl_token::instruction::transfer(
            &spl_token::id(),
            source.key,
            target.key,
            authority.info.key,
            &[],
            value
        )?;
        let accounts = &[
            source.clone(),
            target.clone(),
            authority.info.clone(),
            self.0.clone(),
        ];
        let seeds: &[&[u8]] = &[
            &[ACCOUNT_SEED_VERSION],
            authority.address.as_bytes(),
            &[authority.bump_seed]
        ];

        invoke_signed(&instruction, accounts, &[seeds])
    }

    pub fn approve(
        &self,
        authority: &EthereumAccount<'a>,
        source: &AccountInfo<'a>,
        delegate: &AccountInfo<'a>,
        value: u64
    ) -> Result<(), ProgramError> {
        let instruction = spl_token::instruction::approve(
            &spl_token::id(),
            source.key,
            delegate.key,
            authority.info.key,
            &[],
            value
        )?;
        let accounts = &[
            source.clone(),
            delegate.clone(),
            authority.info.clone(),
            self.0.clone(),
        ];
        let seeds: &[&[u8]] = &[
            &[ACCOUNT_SEED_VERSION],
            authority.address.as_bytes(),
            &[authority.bump_seed]
        ];

        invoke_signed(&instruction, accounts, &[seeds])
    }

    pub fn close_account(
        &self,
        authority: &EthereumAccount<'a>,
        account: &AccountInfo<'a>,
        destination: &AccountInfo<'a>,
    ) -> Result<(), ProgramError> {
        let instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            account.key,
            destination.key,
            authority.info.key,
            &[]
        )?;
        let accounts = &[
            destination.clone(),
            account.clone(),
            authority.info.clone(),
            self.0.clone(),
        ];
        let seeds: &[&[u8]] = &[
            &[ACCOUNT_SEED_VERSION],
            authority.address.as_bytes(),
            &[authority.bump_seed]
        ];

        invoke_signed(&instruction, accounts, &[seeds])
    }
}

impl<'a> Deref for Token<'a> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}
