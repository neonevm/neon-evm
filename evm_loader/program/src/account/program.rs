use std::ops::Deref;

use evm::{ExitError, ExitFatal, ExitReason, ExitSucceed, U256};
use evm::backend::Log;
use solana_program::{
    program::{invoke, invoke_signed}, rent::Rent,
    system_instruction, sysvar::Sysvar
};
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::instruction::Instruction;
use solana_program::log::sol_log_data;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

use crate::account::ACCOUNT_SEED_VERSION;

use super::{EthereumAccount, Operator, sysvar, token};

pub struct Neon<'a> {
    info: &'a AccountInfo<'a>
}

impl<'a> Neon<'a> {
    pub fn from_account(program_id: &Pubkey, info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if program_id != info.key {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not Neon program", info.key);
        }

        Ok(Self { info })
    }

    pub fn on_return(&self, exit_reason: ExitReason, used_gas: U256, result: &[u8]) -> ProgramResult
    {
        debug_print!("on_return {:?}", exit_reason);

        let (exit_message, exit_status) = match exit_reason {
            ExitReason::Succeed(success_code) => {
                match success_code {
                    ExitSucceed::Stopped => {("ExitSucceed: Machine encountered an explict stop.", 0x11)},
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
            solana_program::msg!("Error: used gas {} exceeds u64::max", used_gas);
            u64::MAX
        } else {
            used_gas.as_u64()
        };


        let instruction = {
            use core::mem::size_of;
            let capacity = 2 * size_of::<u8>() + size_of::<u64>() + result.len();

            let mut data = Vec::with_capacity(capacity);
            data.push(6_u8);
            data.push(exit_status);
            data.extend(&used_gas.to_le_bytes());
            data.extend(result);

            Instruction { program_id: *self.info.key, accounts: Vec::new(), data }
        };
        invoke(&instruction, &[ self.info.clone() ])
    }

    #[allow(clippy::unused_self)]
    pub fn on_event(&self, log: &Log) {
        let capacity = 1 + 1 + log.topics.len() + 1;
        let mut fields = Vec::with_capacity(capacity);
        let mnemonic = format!("LOG{}", log.topics.len());
        fields.push(mnemonic.as_bytes());
        fields.push(log.address.as_bytes());
        for topic in &log.topics {
            fields.push(topic.as_bytes());
        }
        fields.push(&log.data);
        sol_log_data(&fields);
    }
}

impl<'a> Deref for Neon<'a> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        self.info
    }
}


pub struct System<'a> {
    info: &'a AccountInfo<'a>
}

impl<'a> System<'a> {
    pub fn from_account(info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if !solana_program::system_program::check_id(info.key) {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not system program", info.key);
        }

        Ok(Self { info })
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
                    &[(*payer).clone(), new_account.clone(), self.info.clone()]
                )?;
            }

            invoke_signed(
                &system_instruction::allocate(new_account.key, space as u64),
                &[new_account.clone(), self.info.clone()],
                &[new_account_seeds],
            )?;

            invoke_signed(
                &system_instruction::assign(new_account.key, program_id),
                &[new_account.clone(), self.info.clone()],
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
                &[(*payer).clone(), new_account.clone(), self.info.clone()],
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
            &[(*source).clone(), target.clone(), self.info.clone()]
        )
    }
}

impl<'a> Deref for System<'a> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        self.info
    }
}


pub struct Token<'a> {
    info: &'a AccountInfo<'a>
}

impl<'a> Token<'a> {
    pub fn from_account(info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if !spl_token::check_id(info.key) {
            return Err!(ProgramError::InvalidArgument; "Account {} - is not token program", info.key);
        }

        Ok(Self { info })
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
            self.info.clone(),
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
            self.info.clone(),
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
            self.info.clone(),
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
            self.info.clone(),
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
        self.info
    }
}
