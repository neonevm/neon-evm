use crate::hamt::Hamt;
use crate::account_data::AccountData;
use solana_sdk::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    info,
};
use primitive_types::{H160, H256, U256};
use std::convert::TryInto;

#[derive(Debug,Clone)]
pub struct SolidityAccount {
    //pub key: H160,
    pub account_data: AccountData,
    pub data: [u8],
    pub lamports: u64,
}

impl SolidityAccount {
    pub fn new(data: [u8], lamports: u64) -> Result<Self, ProgramError> {
        println!("  SolidityAccount::new");
        println!("  Get data with length {}", data.len());
        let (account_data, _) = AccountData::unpack(&data)?;
        Ok(Self{account_data, data, lamports})
    }

    pub fn get_ether(&self) -> H160 {self.account_data.ether}

    pub fn get_nonce(&self) -> u64 {self.account_data.trx_count}

    pub fn code<U, F>(&self, f: F) -> U
    where F: FnOnce(&[u8]) -> U {
        /*if let AccountData::Account{code_size,..} = self.account_data {
            if code_size > 0 {
                let data = self.account_info.data.borrow();
                let offset = AccountData::size();
                return f(&data[offset..offset+code_size as usize])
            }
        }*/
        if self.account_data.code_size > 0 {
            let offset = AccountData::SIZE;
            let code_size = self.account_data.code_size as usize;
            f(&self.data[offset..offset + code_size])
        } else {
            f(&[])
        }
    }

    pub fn storage<U, F>(&self, f: F) -> Result<U, ProgramError>
    where F: FnOnce(&mut Hamt) -> U {
        /*if let AccountData::Account{code_size,..} = self.account_data {
            if code_size > 0 {
                let mut data = self.account_info.data.borrow_mut();
                println!("Storage data borrowed");
                let offset = AccountData::size() + code_size as usize;
                let mut hamt = Hamt::new(&mut data[offset..], false)?;
                return Ok(f(&mut hamt));
            }
        }
        Err(ProgramError::UninitializedAccount)*/
        if self.account_data.code_size > 0 {
            let mut mdata = self.data.borrow_mut();
            println!("Storage data borrowed");
            let code_size = self.account_data.code_size as usize;
            let offset = AccountData::SIZE + code_size;
            let mut hamt = Hamt::new(&mut mdata[offset..], false)?;
            Ok(f(&mut hamt))
        } else {
            Err(ProgramError::UninitializedAccount)
        }
    }

    pub fn update<I>(&mut self, solidity_address: H160, nonce: U256, lamports: u64, code: &Option<Vec<u8>>,
            storage_items: I, reset_storage: bool) -> Result<(), ProgramError>
        where I: IntoIterator<Item=(H256, H256)>,
    {
        println!("Update: {}, {}, {}, {:?} for {:?}", solidity_address, nonce, lamports, if let Some(_) = code {"Exist"} else {"Empty"}, self);
        let mut mdata = self.data.borrow_mut();
        **self.lamports.borrow_mut() = lamports;

        /*let mut current_code_size = match self.account_data {
            AccountData::Empty => 0,
            AccountData::Foreign => 0,
            AccountData::Account{code_size, ..} => code_size as usize,
        };*/
        self.account_data.trx_count = nonce.as_u64();
        if let Some(code) = code {
            if self.account_data.code_size != 0 {
                return Err(ProgramError::AccountAlreadyInitialized);
            };
            self.account_data.code_size = code.len().try_into().map_err(|_| ProgramError::AccountDataTooSmall)?;
            println!("Write code");
            mdata[AccountData::SIZE..AccountData::SIZE+code.len()].copy_from_slice(&code);
            println!("Code written");
        }


        println!("Write account data");
        self.account_data.pack(&mut mdata)?;

        let mut storage_iter = storage_items.into_iter().peekable();
        let exist_items = if let Some(_) = storage_iter.peek() {true} else {false};
        if reset_storage || exist_items {
            println!("Update storage");
            let code_size = self.account_data.code_size as usize;
            if code_size == 0 {return Err(ProgramError::UninitializedAccount);};

            let mut storage = Hamt::new(&mut mdata[AccountData::SIZE+code_size..], reset_storage)?;
            println!("Storage initialized");
            for (key, value) in storage_iter {
                println!("Storage value: {} = {}", &key.to_string() , &value.to_string());
                storage.insert(key.as_fixed_bytes().into(), value.as_fixed_bytes().into())?;
            }
        }

        println!("Account updated");
        
        Ok(())
    }
}


