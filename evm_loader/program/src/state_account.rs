use crate::{
    config::OPERATOR_PRIORITY_SLOTS,
    error::EvmLoaderError,
    account::{State, FinalizedState, Operator, Incinerator, program},
    transaction::UnsignedTransaction,
};
use evm::{H160, U256};
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    program_error::ProgramError,
    sysvar::Sysvar,
    clock::Clock,
};
use serde::{ Serialize, de::DeserializeOwned };
use std::convert::TryInto;


pub enum Deposit<'a> {
    ReturnToOperator(Operator<'a>),
    Burn(Incinerator<'a>),
}


impl <'a> FinalizedState<'a> {
    #[must_use]
    pub fn is_outdated(&self, signature: &[u8; 65], caller: &H160)  -> bool {
        self.sender != *caller || self.signature.ne(signature)
    }
}


impl<'a> State<'a> {
    pub fn new(
        program_id: &'a Pubkey,
        info: &'a AccountInfo<'a>,
        accounts: &crate::instruction::transaction::Accounts<'a>,
        caller: H160,
        trx: &UnsignedTransaction,
        signature: &[u8; 65]
    ) -> Result<Self, ProgramError> {
        let data = crate::account::state::Data {
            caller,
            nonce: trx.nonce,
            gas_limit: trx.gas_limit,
            gas_price: trx.gas_price,
            slot: Clock::get()?.slot,
            operator: *accounts.operator.key,
            accounts_len: accounts.remaining_accounts.len(),
            executor_data_size: 0,
            evm_data_size: 0,
            gas_used_and_paid: U256::zero(),
            number_of_payments: 0,
            signature: *signature,
        };

        let mut storage = match crate::account::tag(program_id, info)? {
            crate::account::TAG_EMPTY => {
                State::init(info, data)
            }
            FinalizedState::TAG => {
                let finalized_storage = FinalizedState::from_account(program_id, info)?;
                assert!(finalized_storage.is_outdated(signature, &caller));

                unsafe { finalized_storage.replace(data) }
            }
            _ => return Err!(ProgramError::InvalidAccountData; "Account {} - expected finalized storage or empty", info.key)
        }?;

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
        if account_tag == crate::account::TAG_EMPTY {
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
        solana_program::msg!("Finalize Storage {}", self.info.key);

        match deposit {
            Deposit::ReturnToOperator(operator) => self.withdraw_deposit(&operator),
            Deposit::Burn(incinerator) => self.withdraw_deposit(&incinerator),
        }?;

        let finalized_data = crate::account::state::FinalizedData {
            sender: self.caller,
            signature: self.signature,
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

    pub fn serialize<T: Serialize, E: Serialize>(&mut self, evm_data: &T, executor_data: &E) -> Result<(), ProgramError> {
        {
            self.evm_data_size = bincode::serialized_size(&evm_data)
                .map_err(|e| E!(ProgramError::InvalidInstructionData; "Error={:?}", e))?
                .try_into()
                .map_err(|e| E!(ProgramError::InvalidInstructionData; "TryFromIntError={:?}", e))?;
            self.executor_data_size = bincode::serialized_size(&executor_data)
                .map_err(|e| E!(ProgramError::InvalidInstructionData; "Error={:?}", e))?
                .try_into()
                .map_err(|e| E!(ProgramError::InvalidInstructionData; "TryFromIntError={:?}", e))?;
        }
        
        let mut account_data = self.info.try_borrow_mut_data()?;
        {
            let (start, mid, end) = self.storage_region();
            if account_data.len() < end {
                return Err!(ProgramError::AccountDataTooSmall; "account_data.len()={:?} < end={:?}", account_data.len(), end);
            }

            {
                let buffer = &mut account_data[start..mid];
                bincode::serialize_into(buffer, &evm_data).map_err(|e| E!(ProgramError::InvalidInstructionData; "Error={:?}", e))?;
            }
            {
                let buffer = &mut account_data[mid..end];
                bincode::serialize_into(buffer, &executor_data).map_err(|e| E!(ProgramError::InvalidInstructionData; "Error={:?}", e))?;
            }
        }

        Ok(())
    }

    pub fn deserialize<T: DeserializeOwned, E: DeserializeOwned>(&self) -> Result<(T, E), ProgramError> {
        let account_data = self.info.try_borrow_data()?;

        let (start, mid, end) = self.storage_region();
        if account_data.len() < end {
            return Err!(ProgramError::AccountDataTooSmall; "account_data.len()={:?}", account_data.len());
        }

        let evm_data: T = {
            let buffer = &account_data[start..mid];
            bincode::deserialize_from(buffer).map_err(|e| E!(ProgramError::InvalidInstructionData; "Error={:?}", e))?
        };
        let executor_data: E = {
            let buffer = &account_data[mid..end];
            bincode::deserialize_from(buffer).map_err(|e| E!(ProgramError::InvalidInstructionData; "Error={:?}", e))?
        };

        Ok((evm_data, executor_data))
    }

    fn accounts_region(&self) -> (usize, usize) {
        let begin = Self::SIZE;
        let end = begin + self.accounts_len * (1 + 32);

        (begin, end)
    }

    fn storage_region(&self) -> (usize, usize, usize) {
        let (_accounts_region_begin, accounts_region_end) = self.accounts_region();

        let begin = accounts_region_end;
        let mid = begin + self.evm_data_size;
        let end = mid + self.executor_data_size;

        (begin, mid, end)
    }
}
