use crate::{
    config::OPERATOR_PRIORITY_SLOTS,
    error::EvmLoaderError,
    account::{State, FinalizedState, Operator, Incinerator, program, Holder},
    transaction::Transaction,
};
use evm::{H160, U256};
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    program_error::ProgramError,
    sysvar::Sysvar,
    clock::Clock,
};
use std::cell::{RefMut, Ref};


pub enum Deposit<'a> {
    ReturnToOperator(Operator<'a>),
    Burn(Incinerator<'a>),
}


impl <'a> FinalizedState<'a> {
    #[must_use]
    pub fn is_outdated(&self, transaction_hash: &[u8; 32]) -> bool {
        self.transaction_hash.ne(transaction_hash)
    }
}


impl<'a> State<'a> {
    pub fn new(
        program_id: &'a Pubkey,
        info: &'a AccountInfo<'a>,
        accounts: &crate::instruction::transaction::Accounts<'a>,
        caller: H160,
        trx: &Transaction,
    ) -> Result<Self, ProgramError> {
        let owner = match crate::account::tag(program_id, info)? {
            Holder::TAG => {
                let holder = Holder::from_account(program_id, info)?;
                holder.owner
            }
            FinalizedState::TAG => {
                let finalized_storage = FinalizedState::from_account(program_id, info)?;
                if !finalized_storage.is_outdated(&trx.hash) {
                    return Err!(EvmLoaderError::StorageAccountFinalized.into(); "Transaction already finalized")
                }

                finalized_storage.owner
            }
            _ => return Err!(ProgramError::InvalidAccountData; "Account {} - expected finalized storage or holder", info.key)
        };

        if &owner != accounts.operator.key {
            return Err!(ProgramError::InvalidAccountData; "Account {} - invalid state account owner", info.key)
        }

        let data = crate::account::state::Data {
            owner,
            transaction_hash: trx.hash,
            caller,
            gas_limit: trx.gas_limit,
            gas_price: trx.gas_price,
            gas_used: U256::zero(),
            operator: *accounts.operator.key,
            slot: Clock::get()?.slot,
            accounts_len: accounts.remaining_accounts.len(),
        };

        info.data.borrow_mut()[0] = 0_u8;
        let mut storage = State::init(info, data)?;

        storage.make_deposit(&accounts.system_program, &accounts.operator)?;
        storage.write_accounts(accounts.remaining_accounts)?;
        Ok(storage)
    }

    pub fn restore(
        program_id: &Pubkey,
        info: &'a AccountInfo<'a>,
        operator: &Operator,
        accounts: &[AccountInfo],
    ) -> Result<Self, ProgramError> {
        let account_tag = crate::account::tag(program_id, info)?;
        if account_tag == FinalizedState::TAG {
            return Err!(EvmLoaderError::StorageAccountFinalized.into(); "Account {} - Storage Finalized", info.key);
        }
        if account_tag == Holder::TAG {
            return Err!(EvmLoaderError::StorageAccountUninitialized.into(); "Account {} - Storage Uninitialized", info.key);
        }

        let mut storage = State::from_account(program_id, info)?;
        storage.check_accounts(accounts)?;

        let clock = Clock::get()?;
        if (*operator.key != storage.operator) && ((clock.slot - storage.slot) <= OPERATOR_PRIORITY_SLOTS) {
            return Err!(ProgramError::InvalidAccountData; "operator.key != storage.operator");
        }

        if storage.operator != *operator.key {
            storage.operator = *operator.key;
            storage.slot = clock.slot;
        }

        Ok(storage)
    }

    pub fn finalize(self, deposit: Deposit<'a>) -> Result<FinalizedState<'a>, ProgramError> {
        debug_print!("Finalize Storage {}", self.info.key);

        match deposit {
            Deposit::ReturnToOperator(operator) => self.withdraw_deposit(&operator),
            Deposit::Burn(incinerator) => self.withdraw_deposit(&incinerator),
        }?;

        let finalized_data = crate::account::state::FinalizedData {
            owner: self.owner,
            transaction_hash: self.transaction_hash
        };

        let finalized = unsafe { self.replace(finalized_data) }?;
        Ok(finalized)
    }

    fn make_deposit(&self, system_program: &program::System<'a>, source: &Operator<'a>) -> Result<(), ProgramError> {
        system_program.transfer(source, self.info, crate::config::PAYMENT_TO_DEPOSIT)
    }

    fn withdraw_deposit(&self, target: &AccountInfo<'a>) -> Result<(), ProgramError> {
        let source_lamports = self.info.lamports().checked_sub(crate::config::PAYMENT_TO_DEPOSIT)
            .ok_or_else(|| E!(ProgramError::InvalidArgument; "Deposit source lamports underflow"))?;

        let target_lamports = target.lamports().checked_add(crate::config::PAYMENT_TO_DEPOSIT)
            .ok_or_else(|| E!(ProgramError::InvalidArgument; "Deposit target lamports overflow"))?;

        **self.info.lamports.borrow_mut() = source_lamports;
        **target.lamports.borrow_mut() = target_lamports;

        Ok(())
    }

    pub fn accounts(&self) -> Result<Vec<(bool, Pubkey)>, ProgramError> {
        let (begin, end) = self.accounts_region();

        let account_data = self.info.try_borrow_data()?;
        if account_data.len() < end {
            return Err!(ProgramError::AccountDataTooSmall; "Account {} - data too small, required: {}", self.info.key, end);
        }

        let keys_storage = &account_data[begin..end];
        let chunks = keys_storage.chunks_exact(1 + 32);
        let accounts = chunks
            .map(|c| c.split_at(1))
            .map(|(writable, key)| (writable[0] > 0, Pubkey::new(key)))
            .collect();

        Ok(accounts)
    }

    fn write_accounts(&mut self, accounts: &[AccountInfo]) -> Result<(), ProgramError> {
        assert_eq!(self.accounts_len, accounts.len()); // should be always true

        let (begin, end) = self.accounts_region();

        let mut account_data = self.info.try_borrow_mut_data()?;
        if account_data.len() < end {
            return Err!(ProgramError::AccountDataTooSmall; "Account {} - data too small, required: {}", self.info.key, end);
        }

        let accounts_storage = &mut account_data[begin..end];
        let accounts_storage = accounts_storage.chunks_exact_mut(1 + 32);
        for (info, account_storage) in accounts.iter().zip(accounts_storage) {
            account_storage[0] = u8::from(info.is_writable);
            account_storage[1..].copy_from_slice(info.key.as_ref());
        }

        Ok(())
    }

    fn check_accounts(&self, accounts: &[AccountInfo]) -> Result<(), ProgramError> {
        let blocked_accounts = self.accounts()?;
        if blocked_accounts.len() != accounts.len() {
            return Err!(ProgramError::NotEnoughAccountKeys; "Invalid number of accounts");
        }

        for ((writable, key), info) in blocked_accounts.into_iter().zip(accounts) {
            if key != *info.key {
                return Err!(ProgramError::InvalidAccountData; "Expected account {}, found {}", key, info.key);
            }

            if writable != info.is_writable {
                return Err!(ProgramError::InvalidAccountData; "Expected account {} is_writable: {}", info.key, writable);
            }
        }

        Ok(())
    }

    #[must_use]
    pub fn evm_state_data(&self) -> Ref<[u8]> {
        let (_, accounts_region_end) = self.accounts_region();

        let data = self.info.data.borrow();
        Ref::map(data, |d| &d[accounts_region_end..])
    }

    #[must_use]
    pub fn evm_state_mut_data(&mut self) -> RefMut<[u8]> {
        let (_, accounts_region_end) = self.accounts_region();

        let data = self.info.data.borrow_mut();
        RefMut::map(data, |d| &mut d[accounts_region_end..])
    }

    #[must_use]
    fn accounts_region(&self) -> (usize, usize) {
        let begin = Self::SIZE;
        let end = begin + self.accounts_len * (1 + 32);

        (begin, end)
    }
}
