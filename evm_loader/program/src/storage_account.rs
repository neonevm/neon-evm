use crate::{
    account_data::{ Storage, AccountData, FinalizedStorage},
    error::EvmLoaderError
};
use evm::{ H160 };
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    program_error::ProgramError,
    sysvar::Sysvar,
    clock::Clock,
};
use serde::{ Serialize, de::DeserializeOwned };
use std::convert::TryInto;
use crate::config::OPERATOR_PRIORITY_SLOTS;

pub struct StorageAccount<'a> {
    info: &'a AccountInfo<'a>,
    data: AccountData
}

#[allow(clippy::too_many_arguments)]
impl<'a> StorageAccount<'a> {
    pub fn new(info: &'a AccountInfo<'a>, operator: &AccountInfo, accounts: &[AccountInfo], caller: H160, nonce: u64, gas_limit: u64, gas_price: u64, sign: &[u8]) -> Result<Self, ProgramError> {
       let mut account_data = info.try_borrow_mut_data()?;

       match AccountData::unpack(&account_data)? {
            AccountData::FinalizedStorage(_) => {AccountData::pack(&AccountData::Empty, &mut account_data)?;},
            AccountData::Empty => {},
            _ => return Err!(ProgramError::InvalidAccountData; "storage account is not empty and is not finalized key={:?}", info.key)
        };

        let mut sign_:[u8; 65] =[0; 65];
        sign_.copy_from_slice(sign);
        let data = AccountData::Storage(
            Storage {
                caller,
                nonce,
                gas_limit,
                gas_price,
                slot: Clock::get()?.slot,
                operator: *operator.key,
                accounts_len: accounts.len(),
                executor_data_size: 0,
                evm_data_size: 0,
                gas_used_and_paid: 0,
                number_of_payments: 0,
                sign: sign_
            }
        );

        Ok(Self { info, data })
    }

    pub fn restore(info: &'a AccountInfo<'a>, operator: &AccountInfo) -> Result<Self, ProgramError> {
        let mut account_data = info.try_borrow_mut_data()?;

        match AccountData::unpack(&account_data)? {
            AccountData::Storage(mut data) => {
                let clock = Clock::get()?;
                if (*operator.key != data.operator) && ((clock.slot - data.slot) <= OPERATOR_PRIORITY_SLOTS) {
                    return Err!(ProgramError::InvalidAccountData);
                }

                if data.operator != *operator.key {
                    data.operator = *operator.key;
                    data.slot = clock.slot;
                }

                let data = AccountData::Storage(data);
                AccountData::pack(&data, &mut account_data)?;

                Ok(Self { info, data })
            }
            AccountData::Empty =>  Err!(EvmLoaderError::StorageAccountUninitialized.into()),
            AccountData::FinalizedStorage(_) => { Err!(EvmLoaderError::StorageAccountFinalized.into()) },
            _ =>  Err!(ProgramError::InvalidAccountData)
        }
    }

    pub fn finalized_is_outdated(info: &'a AccountInfo<'a>, sign : &[u8], caller: &H160)  -> Result<bool, ProgramError> {
        let account_data = info.try_borrow_data()?;

        match AccountData::unpack(&account_data)? {
            AccountData::FinalizedStorage(storage) => {
                if storage.sender != *caller || !storage.sign.eq(sign) {
                    return Ok(true);
                }
                Ok(false)
            }
            _ =>  Err!(ProgramError::InvalidAccountData)
        }
    }

    pub fn check_for_blocked_accounts(program_id: &Pubkey, accounts: &[AccountInfo], required_exclusive_access : bool) -> Result<(), ProgramError> {
        for account_info in accounts.iter().filter(|a| a.owner == program_id) {
            let data = account_info.try_borrow_data()?;
            if let AccountData::Account(account) = AccountData::unpack(&data)? {
                if account.rw_blocked_acc.is_some() {
                    return Err!(ProgramError::InvalidAccountData; "trying to execute transaction on rw locked account {}", account_info.key);
                }
                if required_exclusive_access && account.ro_blocked_cnt > 0{
                    return Err!(ProgramError::InvalidAccountData; "trying to execute transaction on ro locked account {}", account_info.key);
                }
            }
        }

        Ok(())
    }

    pub fn unblock_accounts_and_finalize(&self, program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), ProgramError> {

        for account_info in accounts.iter().filter(|a| a.owner == program_id) {
            let mut data = account_info.try_borrow_mut_data()?;
            if let AccountData::Account(mut account) = AccountData::unpack(&data)? {
                if let Some(rw_blocked_acc) = account.rw_blocked_acc {
                    if *self.info.unsigned_key() == rw_blocked_acc {
                        account.rw_blocked_acc = None;
                    }
                    else if account.ro_blocked_cnt > 0 {
                            account.ro_blocked_cnt -= 1;
                        }
                        else {
                            return Err!(ProgramError::NotEnoughAccountKeys; "trying to unlock account without ro locking {}", account_info.key);
                        }
                }
                else if account.ro_blocked_cnt > 0 {
                        account.ro_blocked_cnt -= 1;
                    }
                    else {
                        return Err!(ProgramError::NotEnoughAccountKeys; "trying to unlock account without ro locking {}", account_info.key);
                    }

                AccountData::pack(&AccountData::Account(account), &mut data)?;
            }
        }

        let mut account_data = self.info.try_borrow_mut_data()?;
        let finalized_storage = FinalizedStorage{sender :self.caller_and_nonce()?.0, sign: self.get_sign()?};
        AccountData::pack(&AccountData::FinalizedStorage(finalized_storage), &mut account_data)?;

        debug_print!("Destroying {:?}", self.info.key);

        Ok(())
    }

    pub fn caller_and_nonce(&self) -> Result<(H160, u64), ProgramError> {
        let storage = AccountData::get_storage(&self.data)?;
        Ok((storage.caller, storage.nonce))
    }

    pub fn get_sign(&self) -> Result<([u8; 65]), ProgramError> {
        let storage = AccountData::get_storage(&self.data)?;
        Ok(storage.sign)
    }

    pub fn get_gas_params(&self) -> Result<(u64, u64), ProgramError> {
        let storage = AccountData::get_storage(&self.data)?;
        Ok((storage.gas_limit, storage.gas_price))
    }

    pub fn add_gas_has_been_paid(&mut self, gas: u64) -> Result<(), ProgramError> {
        let mut account_data = self.info.try_borrow_mut_data()?;

        let mut storage = AccountData::get_mut_storage(&mut self.data)?;
        storage.gas_used_and_paid += gas;
        storage.number_of_payments += 1;
        AccountData::pack(&self.data, &mut account_data)?;

        Ok(())
    }

    pub fn get_payments_info(&self) -> Result<(u64, u64), ProgramError> {
        let storage = AccountData::get_storage(&self.data)?;
        Ok((storage.gas_used_and_paid, storage.number_of_payments))
    }

    pub fn accounts(&self) -> Result<Vec<Pubkey>, ProgramError> {
        let (begin, end) = self.accounts_region()?;

        let account_data = self.info.try_borrow_data()?;
        if account_data.len() < end {
            return Err!(ProgramError::AccountDataTooSmall; "account_data.len()={:?} < end={:?}", account_data.len(), end);
        }

        let keys_storage = &account_data[begin..end];
        let chunks = keys_storage.chunks_exact(32);
        let keys = chunks.map(Pubkey::new).collect();

        Ok(keys)
    }

    pub fn check_accounts(&self, program_id: &Pubkey, accounts: &[AccountInfo], required_exclusive_access : bool) -> Result<(), ProgramError> {
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
                    if let Some(rw_blocked_acc) = account.rw_blocked_acc {
                        if *self.info.unsigned_key() == rw_blocked_acc {
                            if required_exclusive_access && account.ro_blocked_cnt > 0 {
                                // read-only locks found, wait for unlock
                                return Err(EvmLoaderError::ExclusiveAccessUnvailable.into());
                            }
                        }
                        else if account.ro_blocked_cnt == 0 {
                                return Err!(ProgramError::NotEnoughAccountKeys; "there are no read-only locks");
                        }
                    }
                    else if account.ro_blocked_cnt == 0 {
                            return Err!(ProgramError::NotEnoughAccountKeys; "there are no read-only locks");
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


        let is_writable_code_acc = |code_acc: & Pubkey| -> bool {
            for meta in accounts.iter().filter(|a| a.owner == program_id) {
                if *meta.key == *code_acc && meta.is_writable {
                    return true
                }
            }
            false
        };

        for account_info in accounts.iter().filter(|a| a.owner == program_id) {
            let mut write_block: bool = false;
            let mut read_block: bool = false;
            {
                let data = account_info.try_borrow_data()?;

                if let AccountData::Account(account) = AccountData::unpack(&data)? {
                    if account.rw_blocked_acc.is_some() {
                        return Err!(ProgramError::InvalidAccountData; "trying to lock rw-locked account {}", account_info.key);
                    }
                    if account.code_account == Pubkey::new_from_array([0_u8; 32]) {
                        if is_writable_code_acc(account_info.key) {
                            write_block = true;
                        } else {
                            read_block = true;
                        }
                    }
                    else{
                        // rw lock found
                        if is_writable_code_acc(&account.code_account) {
                            write_block = true;
                        } else {
                            read_block = true;
                        }
                    }
                }
            }
            // lock is needed
            if write_block || read_block {
                debug_print!("lock account {}", account_info.key);
                let mut data = account_info.try_borrow_mut_data()?;

                if let AccountData::Account(mut account) = AccountData::unpack(&data)? {
                    if write_block {
                        account.rw_blocked_acc = Some(*self.info.unsigned_key());
                        debug_print!("set lock rw");
                    }
                    else {
                        account.ro_blocked_cnt += 1;
                        debug_print!("set lock ro");
                    }
                    AccountData::pack(&AccountData::Account(account), &mut data)?;
                }
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
