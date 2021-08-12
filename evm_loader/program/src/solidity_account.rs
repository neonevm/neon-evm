//! Solidity Account info manipulations
use crate::{
    account_data::AccountData,
    hamt::Hamt,
    utils::{keccak256_h256},
    token, token::token_mint
};
use evm::backend::Basic;
use evm::{H160, H256, U256};
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    program_error::ProgramError,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::convert::{TryInto, TryFrom};

/// Solidity Account info
#[derive(Debug, Clone)]
pub struct SolidityAccount<'a> {
    account_data: AccountData,
    solana_address: &'a Pubkey,
    code_data: Option<(AccountData, Rc<RefCell<&'a mut [u8]>>)>,
    balance: U256,
}

impl<'a> SolidityAccount<'a> {
    /// ### Create `SolidityAccount`
    /// ## Example:
    /// ```
    /// let account_data = AccountData::unpack(&caller_info.data.borrow())?;
    /// account_data.get_account()?;
    /// let caller_acc = SolidityAccount::new(caller_info.key, caller_info.lamports(), account_data, None);
    /// ```
    #[must_use]
    pub fn new(solana_address: &'a Pubkey, balance: u64, account_data: AccountData, code_data: Option<(AccountData, Rc<RefCell<&'a mut [u8]>>)>) -> Self {
        let min_decimals = u32::from(token::eth_decimals() - token_mint::decimals());
        let min_value = U256::from(10_u64.pow(min_decimals));

        let balance = U256::from(balance);
        let balance = balance * min_value;

        debug_print!("  SolidityAccount::new solana_adress={} balance={}", solana_address, balance);
        Self{account_data, solana_address, code_data, balance}
    }

    /// Get ethereum account address
    /// # Panics
    ///
    /// Will panic `account_data` doesn't contain `Account` struct
    #[must_use]
    pub fn get_ether(&self) -> H160 {AccountData::get_account(&self.account_data).unwrap().ether}

    /// Get ethereum account nonce
    /// # Panics
    ///
    /// Will panic `account_data` doesn't contain `Account` struct
    #[must_use]
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
        let contract = AccountData::get_contract(contract_data).unwrap();
        let code_size = contract.code_size as usize;

        if code_size > 0 {
            let data = self.code_data.as_ref().unwrap().1.borrow();
            f(&data[contract_data.size()..contract_data.size()+code_size])
        } else {
            f(&[])
        }
    }

    fn valids<U, F>(&self, f: F) -> U
    where F: FnOnce(&[u8]) -> U {
        if self.code_data.is_none() {
            return f(&[])
        }

        let contract_data = &self.code_data.as_ref().unwrap().0;
        let contract = AccountData::get_contract(contract_data).unwrap();
        let code_size = contract.code_size as usize;
        let valids_size = (code_size / 8) + 1;

        if code_size > 0 {
            let data = self.code_data.as_ref().unwrap().1.borrow();
            let begin = contract_data.size() + code_size;
            let end = contract_data.size() + code_size + valids_size; 
            f(&data[begin..end])
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
        Err!(ProgramError::UninitializedAccount)*/
        if self.code_data.is_none() {
            return Err!(ProgramError::UninitializedAccount)
        }

        let contract_data = &self.code_data.as_ref().unwrap().0;
        let contract = AccountData::get_contract(contract_data)?;
        let code_size = contract.code_size as usize;
        let valids_size = (code_size / 8) + 1;

        if code_size > 0 {
            let mut data = self.code_data.as_ref().unwrap().1.borrow_mut();
            debug_print!("Storage data borrowed");
            let mut hamt = Hamt::new(&mut data[contract_data.size()+code_size+valids_size..], false)?;
            Ok(f(&mut hamt))
        } else {
            Err!(ProgramError::UninitializedAccount)
        }
    }

    /// Get solana address
    #[must_use]
    pub const fn get_solana_address(&self) -> Pubkey {
        *self.solana_address
    }

    /// Get solana account seeds
    /// # Panics
    ///
    /// Will panic `account_data` doesn't contain `Account` struct
    #[must_use]
    pub fn get_seeds(&self) -> (H160, u8) { (AccountData::get_account(&self.account_data).unwrap().ether, AccountData::get_account(&self.account_data).unwrap().nonce) }

    /// Get ethereum account basic info
    /// # Panics
    ///
    /// Will panic `account_data` doesn't contain `Account` struct
    #[must_use]
    pub fn basic(&self) -> Basic {
        Basic {
            balance: self.balance, 
            nonce: U256::from(AccountData::get_account(&self.account_data).unwrap().trx_count), 
        }
    }

    /// Get code hash
    #[must_use]
    pub fn code_hash(&self) -> H256 {
        self.code(|d| {
            debug_print!("{}", &hex::encode(&d[0..32]));
            keccak256_h256(d)
        })
    }

    /// Get code size
    #[must_use]
    pub fn code_size(&self) -> usize {
        self.code(|d| d.len())
    }

    /// Get code data
    #[must_use]
    pub fn get_code(&self) -> Vec<u8> {
        self.code(|d| d.into())
    }

    /// Get code data
    #[must_use]
    pub fn get_valids(&self) -> Vec<u8> {
        self.valids(|d| d.into())
    }

    /// Get storage record data
    pub fn get_storage(&self, index: &U256) -> U256 {
        self.storage(|storage| storage.find(*index))
            .unwrap_or_default()
            .unwrap_or_else(U256::zero)
    }

    /// Update account data
    /// # Errors
    ///
    /// Will return: 
    /// `ProgramError::AccountAlreadyInitialized` if trying to save code to account that already have code
    /// `ProgramError::AccountDataTooSmall` if trying to save code to account with not enough data space
    /// `ProgramError::NotEnoughAccountKeys` if didn't find code account
    /// `ProgramError::UninitializedAccount` if code account have `code_size` equal 0
    #[allow(clippy::too_many_arguments)]
    pub fn update<I>(
        &mut self,
        account_info: &'a AccountInfo<'a>,
        #[allow(unused_variables)]
        solidity_address: H160,
        nonce: U256,
        #[allow(unused_variables)]
        balance: U256,
        code_and_valids: &Option<(Vec<u8>, Vec<u8>)>,
        storage_items: I,
        reset_storage: bool,
    ) -> Result<(), ProgramError>
    where I: IntoIterator<Item = (U256, U256)> 
    {
        debug_print!("Update: {}, {}, {}, {:?}, {}", solidity_address, nonce, balance, if code_and_valids.is_some() {"Exist"} else {"Empty"}, reset_storage);
        let mut data = (*account_info.data).borrow_mut();
        // **account_info.lamports.borrow_mut() = lamports;

        /*let mut current_code_size = match self.account_data {
            AccountData::Empty => 0,
            AccountData::Foreign => 0,
            AccountData::Account{code_size, ..} => code_size as usize,
        };*/
        let nonce = u64::try_from(nonce).map_err(|s| E!(ProgramError::InvalidArgument; "s={:?}", s))?;
        AccountData::get_mut_account(&mut self.account_data)?.trx_count = nonce;

        if let Some((code, valids)) = code_and_valids {
            debug_print!("Write contract");
            if let Some((ref mut contract_data, ref mut code_data)) = self.code_data {
                let mut code_data = code_data.borrow_mut();
                let contract = AccountData::get_mut_contract(contract_data)?;
    
                if contract.code_size != 0 {
                    return Err!(ProgramError::AccountAlreadyInitialized; "contract.code_size={:?}", contract.code_size);
                };
                contract.code_size = code.len().try_into().map_err(|e| E!(ProgramError::AccountDataTooSmall; "TryFromIntError={:?}", e))?;
    
                debug_print!("Write contract header");
                contract_data.pack(&mut code_data)?;

                debug_print!("Write code");
                let code_begin = contract_data.size();
                let code_end = code_begin + code.len();
                code_data[code_begin..code_end].copy_from_slice(code);
                debug_print!("Code written");

                let valids_begin = code_end;
                let valids_end = valids_begin + valids.len();
                code_data[valids_begin..valids_end].copy_from_slice(valids);
                debug_print!("Valids written");
            }
            else {
                return Err!(ProgramError::NotEnoughAccountKeys; "Expected code account");
            }
        }

        debug_print!("Write account data");        
        self.account_data.pack(&mut data)?;

        let mut storage_iter = storage_items.into_iter().peekable();
        let exist_items = matches!(storage_iter.peek(), Some(_));
        if reset_storage || exist_items {
            debug_print!("Update storage");
            if let Some((ref contract_data, ref mut code_data)) = self.code_data {
                let mut code_data = code_data.borrow_mut();
    
                let contract = AccountData::get_contract(contract_data)?;
                if contract.code_size == 0 {return Err!(ProgramError::UninitializedAccount; "contract.code_size={:?}", contract.code_size);};
                let code_size = contract.code_size as usize;
                let valids_size = (code_size / 8) + 1;
    
                let mut storage = Hamt::new(&mut code_data[contract_data.size()+code_size+valids_size..], reset_storage)?;
                debug_print!("Storage initialized");
                for (key, value) in storage_iter {
                    debug_print!("Storage value: {} = {}", &key.to_string(), &value.to_string());
                    storage.insert(key, value)?;
                }
            }
            else {
                return Err!(ProgramError::NotEnoughAccountKeys; "Expected code account");
            }
        }

        debug_print!("Account updated");
        Ok(())
    }
}
