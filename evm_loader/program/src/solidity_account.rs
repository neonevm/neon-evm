use crate::{
    account_data::AccountData,
    hamt::Hamt,
    utils::{keccak256_h256, u256_to_h256},
};
use evm::backend::Basic;
use evm::Code;
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    program_error::ProgramError,
};
use primitive_types::{H160, H256, U256};
use std::cell::RefCell;
use std::rc::Rc;
use std::convert::TryInto;


#[derive(Debug, Clone)]
pub struct SolidityAccount<'a> {
    account_data: AccountData,
    solana_address: &'a Pubkey,
    code_data: Option<(AccountData, Rc<RefCell<&'a mut [u8]>>)>,
    lamports: u64,
}

impl<'a> SolidityAccount<'a> {
    pub fn new(solana_address: &'a Pubkey, lamports: u64, account_data: AccountData, code_data: Option<(AccountData, Rc<RefCell<&'a mut [u8]>>)>) -> Result<Self, ProgramError> {
        debug_print!("  SolidityAccount::new");
        Ok(Self{account_data, solana_address, code_data, lamports})
    }

    pub fn get_signer(&self) -> Pubkey {AccountData::get_account(&self.account_data).unwrap().signer}

    pub fn get_ether(&self) -> H160 {AccountData::get_account(&self.account_data).unwrap().ether}

    pub fn get_nonce(&self) -> u64 {AccountData::get_account(&self.account_data).unwrap().trx_count}

    fn code<U, F>(&self, f: F) -> U
    where F: FnOnce(&[u8]) -> U {
        /*if let AccountData::Account{code_size,..} = self.account_data {
            if code_size > 0 {
                let data = self.account_info.data.borrow();
                let offset = AccountData::size();
                return f(&data[offset..offset+code_size as usize])
            }
        }*/
        if self.code_data.is_none() {
            return f(&[])
        }

        let contract_data = &self.code_data.as_ref().unwrap().0;
        let contract = AccountData::get_contract(&contract_data).unwrap();
        let code_size = contract.code_size as usize;

        if code_size > 0 {
            let data = self.code_data.as_ref().unwrap().1.borrow();
            f(&data[contract_data.size()..contract_data.size()+code_size])
        } else {
            f(&[])
        }
    }

    fn storage<U, F>(&self, f: F) -> Result<U, ProgramError>
    where F: FnOnce(&mut Hamt) -> U {
        /*if let AccountData::Account{code_size,..} = self.account_data {
            if code_size > 0 {
                let mut data = self.account_info.data.borrow_mut();
                debug_print!("Storage data borrowed");
                let offset = AccountData::size() + code_size as usize;
                let mut hamt = Hamt::new(&mut data[offset..], false)?;
                return Ok(f(&mut hamt));
            }
        }
        Err(ProgramError::UninitializedAccount)*/
        if self.code_data.is_none() {
            return Err(ProgramError::UninitializedAccount)
        }

        let contract_data = &self.code_data.as_ref().unwrap().0;
        let contract = AccountData::get_contract(&contract_data)?;
        let code_size = contract.code_size as usize;

        if code_size > 0 {
            let mut data = self.code_data.as_ref().unwrap().1.borrow_mut();
            debug_print!("Storage data borrowed");
            let mut hamt = Hamt::new(&mut data[contract_data.size()+code_size..], false)?;
            Ok(f(&mut hamt))
        } else {
            Err(ProgramError::UninitializedAccount)
        }
    }

    pub fn get_solana_address(&self) -> Pubkey {
        *self.solana_address
    }

    pub fn get_seeds(&self) -> (H160, u8) { (AccountData::get_account(&self.account_data).unwrap().ether, AccountData::get_account(&self.account_data).unwrap().nonce) }
    
    pub fn basic(&self) -> Basic {
        Basic { 
            balance: self.lamports.into(), 
            nonce: U256::from(AccountData::get_account(&self.account_data).unwrap().trx_count), }
        
    }
    
    pub fn code_hash(&self) -> H256 {
        self.code(|d| {
            debug_print!("{}", &hex::encode(&d[0..32]));
            keccak256_h256(d)
        })
    }
    
    pub fn code_size(&self) -> usize {
        self.code(|d| d.len())
    }
    
    pub fn get_code(&self, account: H160) -> Code {
        self.code(|d| Code::AccountRef{ ptr: d.as_ptr(), len: d.len(), account })
    }
    
    pub fn get_storage(&self, index: &U256) -> U256 {
        let value = self.storage(|storage| storage.find(*index)).unwrap_or_default();
        if let Some(v) = value { v } else { U256::zero() }
    }

    pub fn update<I>(
        &mut self,
        account_info: &'a AccountInfo<'a>,
        solidity_address: H160,
        nonce: U256,
        lamports: u64,
        code: &Option<Vec<u8>>,
        storage_items: I,
        reset_storage: bool,
    ) -> Result<(), ProgramError>
    where I: IntoIterator<Item = (U256, U256)> 
    {
        debug_print!("Update: {}, {}, {}, {:?}, {}", solidity_address, nonce, lamports, if let Some(_) = code {"Exist"} else {"Empty"}, reset_storage);
        let mut data = (*account_info.data).borrow_mut();
        **(*account_info.lamports).borrow_mut() = lamports;

        /*let mut current_code_size = match self.account_data {
            AccountData::Empty => 0,
            AccountData::Foreign => 0,
            AccountData::Account{code_size, ..} => code_size as usize,
        };*/
        AccountData::get_mut_account(&mut self.account_data)?.trx_count = nonce.as_u64();

        if let Some(code) = code {
            debug_print!("Write contract");
            match self.code_data {
                Some((ref mut contract_data, ref mut code_data)) => {
                    let mut code_data = code_data.borrow_mut();
                    let contract = AccountData::get_mut_contract(contract_data)?;
        
                    if contract.code_size != 0 {
                        return Err(ProgramError::AccountAlreadyInitialized);
                    };
                    contract.code_size = code.len().try_into().map_err(|_| ProgramError::AccountDataTooSmall)?;
        
                    debug_print!("Write contract header");
                    contract_data.pack(&mut code_data)?;
                    debug_print!("Write code");
                    code_data[contract_data.size()..contract_data.size()+code.len()].copy_from_slice(&code);
                    debug_print!("Code written");
                },
                None => {
                    debug_print!("Expected code account");
                    return Err(ProgramError::NotEnoughAccountKeys);
                }
            }
        }

        debug_print!("Write account data");        
        self.account_data.pack(&mut data)?;

        let mut storage_iter = storage_items.into_iter().peekable();
        let exist_items = if let Some(_) = storage_iter.peek() {true} else {false};
        if reset_storage || exist_items {
            debug_print!("Update storage");
            match self.code_data {
                Some((ref contract_data, ref mut code_data)) => {
                    let mut code_data = code_data.borrow_mut();
        
                    let contract = AccountData::get_contract(&contract_data)?;
                    if contract.code_size == 0 {return Err(ProgramError::UninitializedAccount);};
        
                    let mut storage = Hamt::new(&mut code_data[contract_data.size()+(contract.code_size as usize)..], reset_storage)?;
                    debug_print!("Storage initialized");
                    for (key, value) in storage_iter {
                        debug_print!("Storage value: {} = {}", &key.to_string(), &value.to_string());
                        storage.insert(key, value)?;
                    }
                },
                None => {
                    debug_print!("Expected code account");
                    return Err(ProgramError::NotEnoughAccountKeys);
                }
            }
        }

        debug_print!("Account updated");
        Ok(())
    }
}
