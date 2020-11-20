use crate::hamt::Hamt;
use crate::account_data::AccountData;
use solana_sdk::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
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
        let mut data = acc.data.borrow_mut();
        let (accountData, _rest) = AccountData::unpack(&data)?;
        match accountData {
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

    pub fn get_address(&self) -> H160 {
        match self.accountData {
            AccountData::Empty => solidity_address(&self.accountInfo.key),
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
                let offset = AccountData::size() + code_size as usize;
                let mut hamt = Hamt::new(&mut data[offset..])?;
                return Ok(f(&mut hamt));
            }
        }
        Err(ProgramError::UninitializedAccount)
    }

    pub fn update(&mut self, solidity_address: H160, nonce: U256, lamports: u64, code: &Option<Vec<u8>>) -> Result<(), ProgramError> {
        println!("Update: {}, {}, {}, {:?} for {:?}", solidity_address, nonce, lamports, if let Some(_) = code {"Exist"} else {"Empty"}, self);
        let mut data = self.accountInfo.data.borrow_mut();
        **self.accountInfo.lamports.borrow_mut() = lamports;

        if let AccountData::Account{address,..} = self.accountData {
            if address != solidity_address {
                return Err(ProgramError::IncorrectProgramId);
            }
        };

        let mut current_code_size = match self.accountData {
            AccountData::Empty => 0,
            AccountData::Account{code_size, ..} => code_size as usize,
        };
        if let Some(code) = code {
            if current_code_size != 0 {return Err(ProgramError::AccountAlreadyInitialized);}
            current_code_size = code.len();
            data[AccountData::size()..AccountData::size()+current_code_size].copy_from_slice(&code);
        }

        self.accountData = AccountData::Account {
            nonce: nonce,
            address: solidity_address,
            code_size: current_code_size as u64,
        };

        self.accountData.pack(&mut data);
        
        Ok(())
    }
}


