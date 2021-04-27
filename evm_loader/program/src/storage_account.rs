use crate::{
    account_data::Storage,
};
use primitive_types::{ 
    H160 
};
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    program_error::ProgramError,
};
use serde::{ Serialize, de::DeserializeOwned };


pub struct StorageAccount<'a> {
    info: &'a AccountInfo<'a>,
    data: Storage
}

impl<'a> StorageAccount<'a> {
    pub fn new(info: &'a AccountInfo<'a>, caller: H160, nonce: u64) -> Result<Self, ProgramError> {
        // TODO check storage is empty

        let data = Storage { caller, nonce, accounts_len: 0, executor_data_size: 0, evm_data_size: 0 };
        Ok(Self { info, data })
    }

    pub fn restore(info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        let account_data = info.try_borrow_data()?;
        let data = Storage::unpack(&account_data)?.0;

        // TODO check storage is not empty

        Ok(Self { info, data })
    }

    pub fn caller_and_nonce(&self) -> (H160, u64) {
        (self.data.caller, self.data.nonce)
    }

    pub fn accounts(&self) -> Result<Vec<Pubkey>, ProgramError> {
        let account_data = self.info.try_borrow_data()?;
        let account_data = &account_data[Storage::SIZE..];
        let chunks = account_data.chunks_exact(32).take(self.data.accounts_len);
        let keys = chunks.map(|c| Pubkey::new(c)).collect();

        Ok(keys)
    }

    pub fn check_accounts(&self, accounts: &[AccountInfo]) -> Result<(), ProgramError> {
        if self.data.accounts_len != accounts.len() {
            return Err(ProgramError::NotEnoughAccountKeys);
        }

        let keys = accounts.iter().map(|a| a.unsigned_key().clone());
        if self.accounts()?.into_iter().eq(keys) {
            Ok(())
        } else {
            Err(ProgramError::NotEnoughAccountKeys)
        }
    }

    pub fn write_accounts(&mut self, accounts: &[AccountInfo]) -> Result<(), ProgramError> {
        let mut account_data = self.info.try_borrow_mut_data()?;
        {
            let mut keys_storage = &mut account_data[Storage::SIZE..];
            let mut chunks = keys_storage.chunks_exact_mut(32);

            let keys = accounts.iter().map(|a| a.unsigned_key().to_bytes());
            for (key, storage) in keys.zip(chunks) {
                storage.copy_from_slice(&key);
            }
        }

        {
            self.data.accounts_len = accounts.len();
            self.data.pack(&mut account_data)?;
        }

        Ok(())
    }

    pub fn serialize<T: Serialize, E: Serialize>(&mut self, evm_data: &T, executor_data: &E) -> Result<(), ProgramError> {
        let evm_data_size = bincode::serialized_size(&evm_data).unwrap() as usize;
        let executor_data_size = bincode::serialized_size(&executor_data).unwrap() as usize;

        let expected_storage_size = Storage::SIZE + evm_data_size + executor_data_size;
        if self.info.data_len() < expected_storage_size {
            return Err(ProgramError::InvalidAccountData)
        }

        let mut account_data = self.info.try_borrow_mut_data()?;
        {
            let data_start = Storage::SIZE + (self.data.accounts_len * 32usize);
            let data_end = data_start + evm_data_size + executor_data_size;
            let (_, buffer) = account_data.split_at_mut(data_start);
            let (buffer, _) = buffer.split_at_mut(data_end);

            let (evm_data_buffer, executor_data_buffer) = buffer.split_at_mut(evm_data_size);
            bincode::serialize_into(evm_data_buffer, &evm_data).map_err(|_| ProgramError::InvalidInstructionData)?;
            bincode::serialize_into(executor_data_buffer, &executor_data).map_err(|_| ProgramError::InvalidInstructionData)?;
        }

        {
            self.data.evm_data_size = evm_data_size;
            self.data.executor_data_size = executor_data_size;
            self.data.pack(&mut account_data)?;
        }

        Ok(())
    }

    pub fn deserialize<T: DeserializeOwned, E: DeserializeOwned>(&self) -> Result<(T, E), ProgramError> {
        let expected_storage_size = Storage::SIZE + self.data.evm_data_size + self.data.executor_data_size;
        if self.info.data_len() < expected_storage_size {
            return Err(ProgramError::InvalidAccountData)
        }

        let account_data = self.info.try_borrow_data()?;

        let data_start = Storage::SIZE + (self.data.accounts_len * 32usize);
        let data_end = data_start + self.data.evm_data_size + self.data.executor_data_size;
        let (_, buffer) = account_data.split_at(data_start);
        let (buffer, _) = buffer.split_at(data_end);

        let (evm_data_buffer, executor_data_buffer) = buffer.split_at(self.data.evm_data_size);
        let evm_data: T = bincode::deserialize_from(evm_data_buffer).map_err(|_| ProgramError::InvalidInstructionData)?;
        let executor_data: E = bincode::deserialize_from(executor_data_buffer).map_err(|_| ProgramError::InvalidInstructionData)?;

        Ok((evm_data, executor_data))
    }
}