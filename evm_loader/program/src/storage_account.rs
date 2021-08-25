use crate::{
    account_data::{ Storage, AccountData }
};
use evm::{ H160 };
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    program_error::ProgramError,
    sysvar,
    sysvar::Sysvar,
    clock::Clock,
};
use serde::{ Serialize, de::DeserializeOwned };
use std::convert::TryInto;

const OPERATOR_PRIORITY_SLOTS: u64 = 3;

pub struct StorageAccount<'a> {
    info: &'a AccountInfo<'a>,
    data: AccountData
}

impl<'a> StorageAccount<'a> {
    pub fn new(info: &'a AccountInfo<'a>, operator: &AccountInfo, accounts: &[AccountInfo], caller: H160, nonce: u64, gas_limit: u64, gas_price: u64) -> Result<Self, ProgramError> {
        let clock_info = accounts.iter().find(|a| sysvar::clock::check_id(a.key)).ok_or_else(||E!(ProgramError::InvalidAccountData))?;
        let clock = Clock::from_account_info(clock_info)?;

        let account_data = info.try_borrow_data()?;

        if let AccountData::Empty = AccountData::unpack(&account_data)? {
            let data = AccountData::Storage(
                Storage { 
                    caller,
                    nonce,
                    gas_limit,
                    gas_price,
                    slot: clock.slot,
                    operator: *operator.key,
                    accounts_len: accounts.len(),
                    executor_data_size: 0,
                    evm_data_size: 0
                }
            );
            Ok(Self { info, data })
        } else {
            Err!(ProgramError::InvalidAccountData; "storage account is not empty. account_data.len()={:?}", &account_data.len())
        }
    }

    pub fn restore(info: &'a AccountInfo<'a>, operator: &AccountInfo, accounts: &[AccountInfo]) -> Result<Self, ProgramError> {
        let clock_info = accounts.iter().find(|a| sysvar::clock::check_id(a.key)).ok_or_else(||E!(ProgramError::InvalidAccountData))?;
        let clock = Clock::from_account_info(clock_info)?;

        let mut account_data = info.try_borrow_mut_data()?;

        if let AccountData::Storage(mut data) = AccountData::unpack(&account_data)? {
            if (*operator.key != data.operator) && ((clock.slot - data.slot) <= OPERATOR_PRIORITY_SLOTS) {
                return Err!(ProgramError::InvalidAccountData);
            }

            data.operator = *operator.key;
            data.slot = clock.slot;

            let data = AccountData::Storage(data);
            AccountData::pack(&data, &mut account_data)?;

            Ok(Self { info, data })
        } else {
            Err!(ProgramError::InvalidAccountData)
        }
    }

    pub fn unblock_accounts_and_destroy(self, program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), ProgramError> {
        for account_info in accounts.iter().filter(|a| a.owner == program_id) {
            let mut data = account_info.try_borrow_mut_data()?;
            if let AccountData::Account(mut account) = AccountData::unpack(&data)? {
                account.blocked = None;
                AccountData::pack(&AccountData::Account(account), &mut data)?;
            }
        }

        let mut account_data = self.info.try_borrow_mut_data()?;
        AccountData::pack(&AccountData::Empty, &mut account_data)?;

        Ok(())
    }

    pub fn caller_and_nonce(&self) -> Result<(H160, u64), ProgramError> {
        let storage = AccountData::get_storage(&self.data)?;
        Ok((storage.caller, storage.nonce))
    }

    pub fn get_gas_params(&self) -> Result<(u64, u64), ProgramError> {
        let storage = AccountData::get_storage(&self.data)?;
        Ok((storage.gas_limit, storage.gas_price))
    }

    pub fn accounts(&self) -> Result<Vec<Pubkey>, ProgramError> {
        let (begin, end) = self.accounts_region()?;

        let account_data = self.info.try_borrow_data()?;
        if account_data.len() < end {
            return Err!(ProgramError::AccountDataTooSmall; "account_data.len()={:?} < end={:?}", account_data.len(), end);
        }

        let keys_storage = &account_data[begin..end];
        let chunks = keys_storage.chunks_exact(32);
        let keys = chunks.map(|c| Pubkey::new(c)).collect();

        Ok(keys)
    }

    pub fn check_accounts(&self, program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), ProgramError> {
        let storage = AccountData::get_storage(&self.data)?;
        
        if storage.accounts_len != accounts.len() {
            return Err!(ProgramError::NotEnoughAccountKeys; "storage.accounts_len={:?} != accounts.len()={:?}", storage.accounts_len, accounts.len());
        }

        let keys = accounts.iter().map(|a| *a.unsigned_key());
        if !self.accounts()?.into_iter().eq(keys) {
            return Err!(ProgramError::NotEnoughAccountKeys);
        }

        for account_info in accounts.iter().filter(|a| a.owner == program_id) {
            let data = account_info.try_borrow_data()?;
            if let AccountData::Account(account) = AccountData::unpack(&data)? {
                if Some(self.info.unsigned_key()) != account.blocked.as_ref() {
                    return Err!(ProgramError::NotEnoughAccountKeys);
                }
            }
        }

        Ok(())
    }

    pub fn block_accounts(&mut self, program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), ProgramError> {
        {
            let storage = AccountData::get_storage(&self.data)?;
            if storage.accounts_len != accounts.len() {
                return Err!(ProgramError::InvalidInstructionData; "storage.accounts_len={:?} != accounts.len()={:?}", storage.accounts_len, accounts.len());
            }

            let (begin, end) = self.accounts_region()?;

            let mut account_data = self.info.try_borrow_mut_data()?;
            if account_data.len() < end {
                return Err!(ProgramError::AccountDataTooSmall; "account_data.len()={:?} < end={:?}", account_data.len(), end);
            }

            let keys_storage = &mut account_data[begin..end];
            let keys_storage = keys_storage.chunks_exact_mut(32);

            let keys = accounts.iter().map(|a| a.unsigned_key().to_bytes());
            for (key, key_storage) in keys.zip(keys_storage) {
                key_storage.copy_from_slice(&key);
            }
        }


        for account_info in accounts.iter().filter(|a| a.owner == program_id) {
            let mut data = account_info.try_borrow_mut_data()?;
            if let AccountData::Account(mut account) = AccountData::unpack(&data)? {
                account.blocked = Some(*self.info.unsigned_key());
                AccountData::pack(&AccountData::Account(account), &mut data)?;
            }
        }

        Ok(())
    }

    pub fn serialize<T: Serialize, E: Serialize>(&mut self, evm_data: &T, executor_data: &E) -> Result<(), ProgramError> {
        {
            let storage = AccountData::get_mut_storage(&mut self.data)?;
            storage.evm_data_size = bincode::serialized_size(&evm_data)
                .map_err(|e| E!(ProgramError::InvalidInstructionData; "Error={:?}", e))?
                .try_into()
                .map_err(|e| E!(ProgramError::InvalidInstructionData; "TryFromIntError={:?}", e))?;
            storage.executor_data_size = bincode::serialized_size(&executor_data)
                .map_err(|e| E!(ProgramError::InvalidInstructionData; "Error={:?}", e))?
                .try_into()
                .map_err(|e| E!(ProgramError::InvalidInstructionData; "TryFromIntError={:?}", e))?;
        }
        
        let mut account_data = self.info.try_borrow_mut_data()?;
        {
            let (start, mid, end) = self.storage_region()?;
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

        AccountData::pack(&self.data, &mut account_data)?;

        Ok(())
    }

    pub fn deserialize<T: DeserializeOwned, E: DeserializeOwned>(&self) -> Result<(T, E), ProgramError> {
        let account_data = self.info.try_borrow_data()?;

        let (start, mid, end) = self.storage_region()?;
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

    fn accounts_region(&self) -> Result<(usize, usize), ProgramError> {
        let storage = AccountData::get_storage(&self.data)?;

        let begin = AccountData::size(&self.data);
        let end = begin + storage.accounts_len * 32;

        Ok((begin, end))
    }

    fn storage_region(&self) -> Result<(usize, usize, usize), ProgramError> {
        let storage = AccountData::get_storage(&self.data)?;

        let begin = AccountData::size(&self.data) + (storage.accounts_len * 32);
        let mid = begin + storage.evm_data_size;
        let end = mid + storage.executor_data_size;

        Ok((begin, mid, end))
    }
}