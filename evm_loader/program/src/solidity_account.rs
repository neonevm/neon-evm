use crate::hamt::Hamt;
use crate::account_data::AccountData;
use solana_sdk::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    info,
};
use primitive_types::{H160, H256, U256};

fn solidity_address<'a>(key: &Pubkey) -> H160 {
    H256::from_slice(key.as_ref()).into()
}

#[derive(Debug,Clone)]
pub struct SolidityAccount<'a> {
    //pub key: H160,
    pub accountData: AccountData,
    pub accountInfo: &'a AccountInfo<'a>,
}

impl<'a> SolidityAccount<'a> {
    pub fn new(acc: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        info!("  SolidityAccount::new");
        let mut data = acc.data.borrow_mut();
        info!(&("  Get data with length ".to_owned() + &data.len().to_string()));
        let (accountData, _rest) = 
                if data.len() > 0 {AccountData::unpack(&data)?}
                else {(AccountData::Foreign, &data[..])};

        info!("  Unpack account data");
        match accountData {
            AccountData::Foreign =>
                Ok(SolidityAccount{
                        accountData: accountData,
                        accountInfo: acc,
                    }),
            AccountData::Empty => 
                Ok(SolidityAccount{
                        //key: solidity_address(&acc.key),
                        accountData: accountData,
                        accountInfo: acc,
                    }),
            AccountData::Account{nonce, address, code_size} =>
                Ok(SolidityAccount{
                        //key: address,
                        accountData: accountData,
                        accountInfo: acc,
                }),
        }
    }

    pub fn foreign(acc: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        Ok(SolidityAccount{
                accountData: AccountData::Foreign,
                accountInfo: acc,
            })
    }

    pub fn get_address(&self) -> H160 {
        match self.accountData {
            AccountData::Empty => solidity_address(&self.accountInfo.key),
            AccountData::Foreign => solidity_address(&self.accountInfo.key),
            AccountData::Account{address,..} => address,
        }
    }

    pub fn code<U, F>(&self, f: F) -> U
    where F: FnOnce(&[u8]) -> U {
        if let AccountData::Account{code_size,..} = self.accountData {
            if code_size > 0 {
                let data = self.accountInfo.data.borrow();
                let offset = AccountData::size();
                return f(&data[offset..offset+code_size as usize])
            }
        }
        f(&[])
    }

    pub fn storage<U, F>(&self, f: F) -> Result<U, ProgramError>
    where F: FnOnce(&mut Hamt) -> U {
        if let AccountData::Account{code_size,..} = self.accountData {
            if code_size > 0 {
                let mut data = self.accountInfo.data.borrow_mut();
                info!("Storage data borrowed");
                let offset = AccountData::size() + code_size as usize;
                let mut hamt = Hamt::new(&mut data[offset..], false)?;
                return Ok(f(&mut hamt));
            }
        }
        Err(ProgramError::UninitializedAccount)
    }

    pub fn update<I>(&mut self, solidity_address: H160, nonce: U256, lamports: u64, code: &Option<Vec<u8>>,
            storage_items: I, reset_storage: bool) -> Result<(), ProgramError>
        where I: IntoIterator<Item=(H256, H256)>,
    {
        println!("Update: {}, {}, {}, {:?} for {:?}", solidity_address, nonce, lamports, if let Some(_) = code {"Exist"} else {"Empty"}, self);
        let mut data = self.accountInfo.data.borrow_mut();
        **self.accountInfo.lamports.borrow_mut() = lamports;

        if let AccountData::Foreign = self.accountData {
            info!(&("Don't update data for foreign accounts".to_owned() + &solidity_address.to_string()));
            return Ok(());
        }

        if let AccountData::Account{address,..} = self.accountData {
            if address != solidity_address {
                return Err(ProgramError::IncorrectProgramId);
            }
        };

        let mut current_code_size = match self.accountData {
            AccountData::Empty => 0,
            AccountData::Foreign => 0,
            AccountData::Account{code_size, ..} => code_size as usize,
        };
        if let Some(code) = code {
            if current_code_size != 0 {return Err(ProgramError::AccountAlreadyInitialized);}
            current_code_size = code.len();
            info!("Write code");
            data[AccountData::size()..AccountData::size()+current_code_size].copy_from_slice(&code);
            info!("Code written");
        }

        self.accountData = AccountData::Account {
            nonce: nonce,
            address: solidity_address,
            code_size: current_code_size as u64,
        };

        info!("Write account data");

        self.accountData.pack(&mut data);

        let mut storageIter = storage_items.into_iter().peekable();
        let exist_items = if let Some(_) = storageIter.peek() {true} else {false};
        if reset_storage || exist_items {
            info!("Update storage");
            if current_code_size == 0 {return Err(ProgramError::UninitializedAccount);};

            let mut storage = Hamt::new(&mut data[AccountData::size()+current_code_size..], reset_storage)?;
            info!("Storage initialized");
            for (key, value) in storageIter {
                info!(&("Storage value: ".to_owned() + &key.to_string() + " = " + &value.to_string()));
                storage.insert(key.as_fixed_bytes().into(), value.as_fixed_bytes().into())?;
            }
        }

        info!("Account updated");
        
        Ok(())
    }
}


